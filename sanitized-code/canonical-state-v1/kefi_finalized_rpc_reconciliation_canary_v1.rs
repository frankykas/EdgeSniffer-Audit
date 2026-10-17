//! Isolated, bounded fallback canary. It subscribes only to Pump/PumpSwap
//! transaction identities, appends each identity before semantic decoding, and
//! releases no slot unless finalized getBlock proves an exact set/index match.

use std::{collections::{BTreeMap, BTreeSet, HashMap}, env, fs, path::PathBuf, process::Command, time::Duration};
use anyhow::{bail, Context, Result};
use chrono::Utc;
use futures::{stream, StreamExt};
use prost::Message;
use serde::Serialize;
use serde_json::{json, Value};
use tonic::{metadata::AsciiMetadataValue, transport::{Channel, ClientTlsConfig}, Request, Status};
mod finalized_rpc_dispatcher_v1;

use finalized_rpc_dispatcher_v1::{Dispatcher, SlotState};
use yellowstone_grpc_proto::geyser::{geyser_client::GeyserClient, subscribe_update::UpdateOneof, CommitmentLevel, SlotStatus, SubscribeRequest, SubscribeRequestFilterBlocks, SubscribeRequestFilterSlots, SubscribeRequestFilterTransactions};
use pump_exact_evidence_ledger::{canonical_capture_run_v1::{CanonicalRelevancePolicy, CaptureLifecycle, CaptureRunJournal, ExpectedIdentity, RawRpcEvidence, ResourceAccounting, SourceFinalityRecord}, canonical_state_v1::{CanonicalState, CausalOrder, CausalOrderResolver, JournalWriter, SemanticEvent}, filtered_identity_manifest_v1::{reconcile_finalized_block_with_policy, FilteredIdentity, FilteredIdentityManifestWriter, FinalizedBlockInput, FinalizedTransactionInput, ReconciliationReceipt, RpcBudget, StoragePolicy}, finalized_slot_inventory_v1::{FinalizedSlotInventory, ReconciledProducedSlot}, forward_cohort_receipts_v1::{CreationReceipt, IdentityReceipt as FinalizedIdentityReceipt, PathReceipt, SlotReceiptBundle}, pump_instruction_path_decoder_v1::{decode_instruction_paths, CompiledInstructionInput, FullTransactionInput, InnerInstructionGroupInput, InnerInstructionInput, SemanticEventInput}, source_block_closure_v1::{SourceBlockClosure, SourceBlockIdentity}};
#[cfg(test)] use pump_exact_evidence_ledger::filtered_identity_manifest_v1::reconcile_finalized_block;
use pumpfun_sidecar::kefi_pumpswap_semantics_v1::{decode_events, Event, BUY_DISC, BUY_EXACT_DISC, SELL_DISC, PUMPSWAP_PROGRAM};
use pumpfun_sidecar::pumpswap_creation_witness_v1::{classify_finalized_pump_migration_candidate, validate_finalized_pump_migration, FinalizedCreationCandidate, CREATE_POOL};

const PUMP: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const ROUTER: &str = "6Vo3245eszAb5wuqEMw8mGdbfRUdKbHhDHP5LcaGuTAB";
const MAX_MESSAGE: usize = 128 * 1024 * 1024;
const MAX_RSS: u64 = 8 * 1024 * 1024 * 1024;
const MANIFEST_QUOTA: u64 = 64 * 1024 * 1024;
const OPERATIONAL_BUFFER: u64 = 15 * 1024 * 1024 * 1024;
// The kEFi hot volume currently has about 189 GiB free. This leaves a 160 GiB
// protected reserve while still permitting this 64 MiB manifest-only canary.
const PROTECTED_RESERVE: u64 = 160 * 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DiagnosticRpcReservation { range_slots:u64, inventory_requests:u64, dispatcher_limit:u64 }

#[derive(Clone)]
struct PendingFinalizedRange {
    start: u64,
    end: u64,
    get_blocks_response: Value,
    blocks: Vec<(FinalizedBlockInput, Vec<Value>)>,
    raw_rpc_evidence: Vec<RawRpcEvidence>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PendingRangeReadiness {
    AwaitingSourceFinalizedStatus,
    AwaitingSourceBlockClosure,
    AwaitingSourceIdentities,
    ReadyToSeal,
}

#[derive(Serialize)] #[serde(rename_all="camelCase")]
struct Report { schema:String, status:String, started_at_utc:String, finished_at_utc:String, slots_observed:usize, slots_reconciled:usize, slots_rejected:usize, exact_passes:usize, failures:usize, rpc_calls:u64, dispatcher_rpc_calls:u64, dispatcher_retry_rpc_calls:u64, inventory_get_blocks_rpc_calls:u64, inventory_get_block_rpc_calls:u64, retries:u64, pending_slot_peak:usize, max_rss_bytes:u64, max_message_bytes:usize, disk_growth_bytes:u64, decoder_transactions:u64, decoder_events:u64, decoder_rejections:u64, source_gap_or_epoch_failure:bool, unresolved:String, reconciled_slot_produced:bool, scientific_interval_produced:bool, failure:Option<String> }

fn free(path:&PathBuf)->Result<u64>{ let out=Command::new("df").args(["-B1","--output=avail",path.to_str().context("PATH")?]).output()?; Ok(String::from_utf8(out.stdout)?.lines().nth(1).context("DF")?.trim().parse()?) }
fn rss()->u64 { fs::read_to_string("/proc/self/status").ok().and_then(|s|s.lines().find_map(|l|l.strip_prefix("VmHWM:")?.split_whitespace().next()?.parse::<u64>().ok())).unwrap_or(0)*1024 }
fn validate_storage_policy(start_free:u64,reserve:u64,start_floor:u64,total_cap:u64)->Result<()> { if total_cap != 6_u64*1024*1024*1024 || start_floor < reserve.saturating_add(total_cap).saturating_add(OPERATIONAL_BUFFER) { bail!("STORAGE_POLICY_INVALID") } if start_free<start_floor {bail!("PROTECTED_STARTUP_FLOOR")}Ok(()) }
fn ensure_fresh_output_root(root:&PathBuf)->Result<()> { if root.join("capture-run.checkpoint.json").exists(){CaptureRunJournal::void_existing(root,"PROCESS_RESTART_REQUIRES_FRESH_RUN")?;bail!("DURABLE_PRIOR_RUN_VOIDED_REQUIRES_NEW_ROOT")}if root.join("run-binding.json").exists(){bail!("DURABLE_PRIOR_RUN_REQUIRES_NEW_ROOT")}Ok(()) }
fn diagnostic_rpc_reservation(budget:&RpcBudget,start:u64,end:u64)->Result<DiagnosticRpcReservation>{let range_slots=end.checked_sub(start).and_then(|value|value.checked_add(1)).context("FINALIZED_RANGE_INVALID")?;if range_slots==0||range_slots>15{bail!("FINALIZED_RANGE_DIAGNOSTIC_CAP")}let inventory_requests=range_slots.checked_add(1).context("FINALIZED_RANGE_BUDGET_OVERFLOW")?;if inventory_requests>budget.maximum_requests{bail!("FINALIZED_RANGE_RPC_BUDGET_INSUFFICIENT")}let dispatcher_limit=budget.maximum_requests-inventory_requests;if budget.used_requests()>dispatcher_limit{bail!("FINALIZED_RANGE_RESERVATION_ALREADY_STARVED")}Ok(DiagnosticRpcReservation{range_slots,inventory_requests,dispatcher_limit})}
fn consume_dispatcher_rpc(budget:&mut RpcBudget,reservation:Option<DiagnosticRpcReservation>)->Result<()> {if let Some(reservation)=reservation {if budget.used_requests()>=reservation.dispatcher_limit{bail!("DISPATCHER_RPC_RESERVATION_EXHAUSTED")}}budget.consume_one()}
fn parse_diagnostic_range_slots(value:Option<&str>)->Result<Option<u64>> { let Some(value)=value else{return Ok(None)};let slots:u64=value.parse().context("FINALIZED_RANGE_SLOT_COUNT")?;if !(3..=5).contains(&slots){bail!("FINALIZED_RANGE_SLOT_COUNT_OUT_OF_BOUNDS")}Ok(Some(slots)) }
#[cfg(test)] fn finalized_range_ready(start:u64,slots:u64,highest_finalized_slot:Option<u64>)->Result<bool>{let end=start.checked_add(slots.checked_sub(1).context("FINALIZED_RANGE_INVALID")?).context("FINALIZED_RANGE_OVERFLOW")?;Ok(highest_finalized_slot.is_some_and(|highest|highest>=end))}
// Both diagnostic modes start collecting. Range mode completes only after its
// inventory seals; no-range mode completes after one exact dispatcher match.
fn initial_range_inventory_complete(_:Option<u64>)->bool { false }
fn req(policy:&CanonicalRelevancePolicy)->Result<SubscribeRequest> {
    policy.validate()?;
    let mut transactions=HashMap::new();
    let mut slots=HashMap::new();
    let mut blocks=HashMap::new();
    transactions.insert("canonicalRelevant".into(), SubscribeRequestFilterTransactions { vote:Some(policy.include_votes), failed:Some(policy.include_failed), signature:None, account_include:policy.program_ids.clone(), account_exclude:vec![], account_required:vec![], token_accounts:None });
    // This is a closure witness for the same account filter, not a request for
    // an unfiltered full block. It never supplies canonical ordering.
    blocks.insert("canonicalRelevantClosure".into(), SubscribeRequestFilterBlocks { account_include:policy.program_ids.clone(), include_transactions:Some(true), include_accounts:Some(false), include_entries:Some(false), cuckoo_account_include:None });
    slots.insert("finalizedProgress".into(), SubscribeRequestFilterSlots { filter_by_commitment:Some(true), interslot_updates:Some(false) });
    Ok(SubscribeRequest { accounts:HashMap::new(), slots, transactions, transactions_status:HashMap::new(), blocks, blocks_meta:HashMap::new(), entry:HashMap::new(), accounts_data_slice:vec![], commitment:Some(CommitmentLevel::Finalized as i32), ping:None, from_slot:None })
}
fn programs()->BTreeSet<String>{ [PUMP.to_owned(),ROUTER.to_owned(),PUMPSWAP_PROGRAM.to_owned()].into_iter().collect() }
#[cfg(test)] fn default_relevance_policy()->Result<CanonicalRelevancePolicy>{CanonicalRelevancePolicy::new("PUMPSWAP_FINALIZED_RELEVANCE_V1",programs().into_iter().collect(),1)}
fn relevant(tx:&Value)->Result<FinalizedTransactionInput>{ let signature=tx.pointer("/transaction/signatures/0").and_then(Value::as_str).context("RPC_SIGNATURE")?.to_owned(); let mut ids=BTreeSet::new(); for key in tx.pointer("/transaction/message/accountKeys").and_then(Value::as_array).context("RPC_KEYS")? { if let Some(s)=key.as_str(){ids.insert(s.to_owned());} else if let Some(s)=key.get("pubkey").and_then(Value::as_str){ids.insert(s.to_owned());} else {bail!("RPC_KEY_LAYOUT")} } for p in ["/meta/loadedAddresses/writable","/meta/loadedAddresses/readonly"] { if let Some(keys)=tx.pointer(p).and_then(Value::as_array){ for key in keys { ids.insert(key.as_str().context("RPC_LOADED_KEY")?.to_owned()); } } } let failed = tx.pointer("/meta/err").map(|v| !v.is_null()).unwrap_or(true); if failed { ids.clear(); } let supported_version = match tx.get("version") { None => true, Some(value) if value.as_str()==Some("legacy") => true, Some(value) => value.as_u64().is_some_and(|version| version <= 1) }; Ok(FinalizedTransactionInput { transaction_index:String::new(), signature, account_or_program_ids:ids.into_iter().collect(), supported_version }) }

#[derive(Clone)]
struct RawSemanticEvent { event_id: String, log_index: usize, event: Event }

/// Minimal decoded representation of an actual Yellowstone update.  The live
/// gRPC loop maps protobuf messages here and the offline harness drives this
/// same transition function, so range control cannot drift from its test path.
enum SourceObservation {
    Transaction { slot:u64, signature:String, source_transaction_index:u64, received_at_unix_ns:u128, source_frame_sha256:String, semantic_events:Vec<RawSemanticEvent>, decoder_rejected:bool },
    FinalizedSlot { slot:u64, source_frame_sha256:String },
    BlockClosure { closure:SourceBlockClosure },
}

fn apply_source_observation(observation:SourceObservation, source:&str, run:&mut CaptureRunJournal, manifest:&mut FilteredIdentityManifestWriter, dispatcher:&mut Dispatcher, finalized_slots:&mut BTreeSet<u64>, source_block_closures:&mut BTreeMap<u64,SourceBlockClosure>, all_records:&mut Vec<FilteredIdentity>, raw_events:&mut HashMap<String,Vec<RawSemanticEvent>>, report:&mut Report, finality_receive_sequence:&mut u64, use_legacy_dispatcher:bool)->Result<()> {
    match observation {
        SourceObservation::Transaction { slot, signature, source_transaction_index, received_at_unix_ns, source_frame_sha256, semantic_events, decoder_rejected } => {
            let identity=manifest.append(slot,&signature,received_at_unix_ns,Some(source_transaction_index),&source_frame_sha256)?;
            all_records.push(identity);run.bind_manifest(manifest.tail_record_sha256(),manifest.next_manifest_sequence())?;
            if use_legacy_dispatcher { dispatcher.observe(slot,now_ms())?; if finalized_slots.contains(&slot) { dispatcher.finalized_progress(slot,now_ms())?; } }
            report.decoder_transactions+=1;
            if decoder_rejected { report.decoder_rejections+=1; } else { report.decoder_events+=semantic_events.len() as u64; raw_events.insert(signature,semantic_events); }
        }
        SourceObservation::FinalizedSlot { slot, source_frame_sha256 } => {
            finalized_slots.insert(slot);*finality_receive_sequence=finality_receive_sequence.checked_add(1).context("SOURCE_FINALITY_SEQUENCE_OVERFLOW")?;
            run.observe_finality(SourceFinalityRecord{schema:String::new(),source_instance_id:source.into(),continuity_epoch:"1".into(),slot:slot.to_string(),receive_sequence:finality_receive_sequence.to_string(),source_frame_sha256,record_sha256:String::new()}.seal()?)?;
            if use_legacy_dispatcher && dispatcher.slots.contains_key(&slot) { dispatcher.finalized_progress(slot,now_ms())?; }
        }
        SourceObservation::BlockClosure { closure } => {
            closure.validate()?;
            let slot: u64=closure.slot.parse()?;
            if let Some(existing)=source_block_closures.get(&slot) {
                if existing.closure_sha256!=closure.closure_sha256 { bail!("SOURCE_BLOCK_CLOSURE_CONFLICT") }
            } else {
                closure.write_atomic(run.root())?;
                source_block_closures.insert(slot,closure);
            }
        }
    }
    Ok(())
}

fn source_block_closure_from_update(source:&str, epoch:&str, policy:&CanonicalRelevancePolicy, block:&yellowstone_grpc_proto::geyser::SubscribeUpdateBlock, source_block_update_sha256:String)->Result<SourceBlockClosure> {
    let mut filtered_identities=Vec::new();
    for transaction in &block.transactions {
        if !policy.include_votes && transaction.is_vote { continue }
        let meta=transaction.meta.as_ref().context("SOURCE_BLOCK_META_MISSING")?;
        if !policy.include_failed && meta.err.is_some() { continue }
        filtered_identities.push(SourceBlockIdentity { signature:bs58::encode(&transaction.signature).into_string(), source_transaction_index:transaction.index.to_string() });
    }
    SourceBlockClosure { schema:String::new(), source_instance_id:source.into(), continuity_epoch:epoch.into(), relevance_policy_sha256:policy.policy_sha256.clone(), slot:block.slot.to_string(), blockhash:block.blockhash.clone(), executed_transaction_count:block.executed_transaction_count.to_string(), source_block_update_sha256, filtered_identities, closure_sha256:String::new() }.seal()
}


fn json_keys(tx:&Value, path:&str)->Result<Vec<String>> { tx.pointer(path).and_then(Value::as_array).context("RPC_ACCOUNT_KEYS")?.iter().map(|key| key.as_str().or_else(||key.get("pubkey").and_then(Value::as_str)).map(str::to_owned).context("RPC_ACCOUNT_KEY_LAYOUT")).collect() }
fn json_instruction(value:&Value)->Result<CompiledInstructionInput> { Ok(CompiledInstructionInput { program_id_index:value.get("programIdIndex").and_then(Value::as_u64).context("RPC_PROGRAM_INDEX")? as usize, account_indexes:value.get("accounts").and_then(Value::as_array).context("RPC_ACCOUNTS")?.iter().map(|value|value.as_u64().map(|n|n as usize).context("RPC_ACCOUNT_INDEX")).collect::<Result<Vec<_>>>()?, data:bs58::decode(value.get("data").and_then(Value::as_str).context("RPC_INSTRUCTION_DATA")?).into_vec().context("RPC_INSTRUCTION_DATA_BASE58")? }) }
fn finalized_input(slot:u64, index:usize, tx:&Value)->Result<FullTransactionInput> { let signature=tx.pointer("/transaction/signatures/0").and_then(Value::as_str).context("RPC_SIGNATURE")?.to_owned(); let outer=tx.pointer("/transaction/message/instructions").and_then(Value::as_array).context("RPC_OUTER_INSTRUCTIONS")?.iter().map(json_instruction).collect::<Result<Vec<_>>>()?; let mut inner=Vec::new(); if let Some(groups)=tx.pointer("/meta/innerInstructions").and_then(Value::as_array) { for group in groups { let parent=group.get("index").and_then(Value::as_u64).context("RPC_INNER_PARENT_INDEX")? as usize; let instructions=group.get("instructions").and_then(Value::as_array).context("RPC_INNER_INSTRUCTIONS")?.iter().map(|value| Ok(InnerInstructionInput { program_id_index:value.get("programIdIndex").and_then(Value::as_u64).context("RPC_INNER_PROGRAM_INDEX")? as usize, account_indexes:value.get("accounts").and_then(Value::as_array).context("RPC_INNER_ACCOUNTS")?.iter().map(|value|value.as_u64().map(|n|n as usize).context("RPC_INNER_ACCOUNT_INDEX")).collect::<Result<Vec<_>>>()?, data:bs58::decode(value.get("data").and_then(Value::as_str).context("RPC_INNER_DATA")?).into_vec().context("RPC_INNER_DATA_BASE58")?, stack_height:value.get("stackHeight").and_then(Value::as_u64).map(|n|n as usize) })).collect::<Result<Vec<_>>>()?; inner.push(InnerInstructionGroupInput { parent_outer_index:parent, instructions }); } }
    Ok(FullTransactionInput { slot, finalized_transaction_index:Some(index as u64), signature, succeeded:tx.pointer("/meta/err").map(Value::is_null).unwrap_or(false), static_account_keys:json_keys(tx,"/transaction/message/accountKeys")?, loaded_writable_account_keys:tx.pointer("/meta/loadedAddresses/writable").map(|_|json_keys(tx,"/meta/loadedAddresses/writable")).transpose()?.unwrap_or_default(), loaded_readonly_account_keys:tx.pointer("/meta/loadedAddresses/readonly").map(|_|json_keys(tx,"/meta/loadedAddresses/readonly")).transpose()?.unwrap_or_default(), outer_instructions:outer, inner_instruction_groups:inner, log_messages:tx.pointer("/meta/logMessages").and_then(Value::as_array).context("RPC_LOG_MESSAGES")?.iter().map(|value|value.as_str().map(str::to_owned).context("RPC_LOG_ENTRY")).collect::<Result<Vec<_>>>()? }) }
fn path_receipt(source:&str, epoch:&str, slot:u64, blockhash:&str, transaction_index:usize, event_id:String, semantic_kind:String, pool:String, signature:String, source_frame_sha256:String, decoded:Option<pump_exact_evidence_ledger::pump_instruction_path_decoder_v1::DecodedSemanticEvent>, reason:Option<String>)->Result<PathReceipt> {
    let (instruction_path, canonical_order, disposition, reason_code) = match decoded { Some(event)=>{let order=&event.canonical_order;(order.instruction_path.render(),json!({"slot":order.slot.to_string(),"finalizedTransactionIndex":order.finalized_transaction_index.to_string(),"outerInstructionIndex":order.instruction_path.outer_index.to_string(),"innerInstructionIndex":order.instruction_path.inner_index.map(|v|v.to_string()),"stackHeight":order.instruction_path.stack_height.map(|v|v.to_string()),"semanticOrdinal":order.semantic_ordinal.to_string()}),"PATH_RESOLVED_QUARANTINED_UNIVERSE_INCOMPLETE".into(),Some("CANDIDATE_UNIVERSE_INCOMPLETE".into()))},None=>(String::new(),Value::Null,"QUARANTINED".into(),reason.or(Some("PATH_UNRESOLVED".into()))) };
    PathReceipt {schema:"CANONICAL_FINALIZED_INSTRUCTION_PATH_RECEIPT_V2".into(),source_instance_id:source.into(),continuity_epoch:epoch.into(),slot:slot.to_string(),finalized_blockhash:blockhash.into(),signature,transaction_index:transaction_index.to_string(),event_id,instruction_path,semantic_kind,pool,source_frame_sha256,canonical_order,disposition,reason_code,receipt_sha256:String::new()}.seal()
}
fn source_frame(source_frames:&HashMap<String,String>,signature:&str)->Result<String>{source_frames.get(signature).cloned().context("SEMANTIC_SOURCE_FRAME_MISSING")}
fn finalized_relevant_transaction(tx:&Value,index:usize,policy:&CanonicalRelevancePolicy)->Result<Option<FinalizedTransactionInput>> {
    let mut finalized=relevant(tx)?;
    finalized.transaction_index=index.to_string();
    if !policy.matches_program_ids(&finalized)? { return Ok(None) }
    finalized.require_supported_version()?;
    Ok(Some(finalized))
}
fn path_receipts(source:&str, epoch:&str, slot:u64, blockhash:&str, txs:&[Value], events:&HashMap<String,Vec<RawSemanticEvent>>, source_frames:&HashMap<String,String>, policy:&CanonicalRelevancePolicy)->Result<Vec<PathReceipt>> {
    let mut out=Vec::new();
    for (index,tx) in txs.iter().enumerate() {
        let Some(finalized)=finalized_relevant_transaction(tx,index,policy)? else { continue };
        let signature=finalized.signature;
        let frame=source_frame(source_frames,&signature)?;
        let Some(raw)=events.get(&signature) else {out.push(path_receipt(source,epoch,slot,blockhash,index,format!("{signature}:SOURCE_SEMANTIC_ABSENT"),"SOURCE_SEMANTIC_ABSENT".into(),"UNKNOWN".into(),signature,frame,None,Some("SOURCE_SEMANTIC_ABSENT".into()))?);continue};
        let input=finalized_input(slot,index,tx);
        let semantic=raw.iter().map(|raw| SemanticEventInput { event_id:raw.event_id.clone(), expected_program_id:PUMPSWAP_PROGRAM.into(), pool:raw.event.pool.clone(), log_index:raw.log_index, accepted_instruction_discriminators:if raw.event.side=="BUY" {vec![BUY_DISC.to_vec(),BUY_EXACT_DISC.to_vec()]} else {vec![SELL_DISC.to_vec()]}, semantic_kind:format!("PUMPSWAP_{}",raw.event.side) }).collect::<Vec<_>>();
        match input.and_then(|input|decode_instruction_paths(&input,&semantic)) { Ok(decoded) => for event in decoded { let signature=event.signature.clone(); let kind=event.semantic_kind.clone(); let pool=event.pool.clone(); let event_id=event.event_id.clone();out.push(path_receipt(source,epoch,slot,blockhash,index,event_id,kind,pool,signature,frame.clone(),Some(event),None)?); }, Err(error)=> for raw in raw { out.push(path_receipt(source,epoch,slot,blockhash,index,raw.event_id.clone(),format!("PUMPSWAP_{}",raw.event.side),raw.event.pool.clone(),signature.clone(),frame.clone(),None,Some(error.to_string()))?); }
    }} Ok(out) }
fn creation_path_receipts(source:&str, epoch:&str, slot:u64, blockhash:&str, txs:&[Value],source_frames:&HashMap<String,String>,policy:&CanonicalRelevancePolicy)->Result<Vec<PathReceipt>> { let mut out=Vec::new(); for (index,tx) in txs.iter().enumerate() { if finalized_relevant_transaction(tx,index,policy)?.is_none() || classify_finalized_pump_migration_candidate(tx)?!=FinalizedCreationCandidate::PumpMigrationCandidate {continue} let witnesses=match validate_finalized_pump_migration(slot,index as u64,tx){Ok(witnesses)=>witnesses,Err(_)=>continue}; for witness in witnesses { let frame=source_frame(source_frames,&witness.signature)?; let log_index=witness.event_id.rsplit(':').next().context("CREATION_EVENT_ID")?.parse()?; let semantic=vec![SemanticEventInput{event_id:witness.event_id.clone(),expected_program_id:PUMPSWAP_PROGRAM.into(),pool:witness.pool.clone(),log_index,accepted_instruction_discriminators:vec![CREATE_POOL.to_vec()],semantic_kind:"PUMP_MIGRATE_CREATE_POOL".into()}]; match finalized_input(slot,index,tx).and_then(|input|decode_instruction_paths(&input,&semantic)) { Ok(mut decoded)=>{ if decoded.len()!=1 || decoded[0].canonical_order.instruction_path.render()!=witness.instruction_path { bail!("CREATION_PATH_WITNESS_MISMATCH") }; let event=decoded.remove(0);out.push(path_receipt(source,epoch,slot,blockhash,index,event.event_id.clone(),event.semantic_kind.clone(),event.pool.clone(),event.signature.clone(),frame,Some(event),None)?);},Err(error)=>out.push(path_receipt(source,epoch,slot,blockhash,index,witness.event_id,"PUMP_MIGRATE_CREATE_POOL".into(),witness.pool,witness.signature,frame,None,Some(error.to_string()))?) } } } Ok(out) }
fn identity_receipts_with_policy(source:&str,epoch:&str,slot:u64,blockhash:&str,block_reconciliation_receipt_sha256:&str,txs:&[Value],policy:&CanonicalRelevancePolicy)->Result<Vec<FinalizedIdentityReceipt>> { let mut out=Vec::new();for(index,tx) in txs.iter().enumerate(){let Some(finalized)=finalized_relevant_transaction(tx,index,policy)? else {continue};out.push(FinalizedIdentityReceipt{schema:"EDGESNIFFER_FINALIZED_RECONCILIATION_SLOT_RECEIPT_V1".into(),source_instance_id:source.into(),continuity_epoch:epoch.into(),slot:slot.to_string(),signature:finalized.signature,transaction_index:finalized.transaction_index,block_identity:blockhash.into(),block_reconciliation_receipt_sha256:block_reconciliation_receipt_sha256.into(),disposition:"MATCHED".into(),reason_code:None,receipt_sha256:String::new()}.seal()?)}Ok(out) }
#[cfg(test)] fn identity_receipts(source:&str,epoch:&str,slot:u64,blockhash:&str,block_reconciliation_receipt_sha256:&str,txs:&[Value])->Result<Vec<FinalizedIdentityReceipt>> {identity_receipts_with_policy(source,epoch,slot,blockhash,block_reconciliation_receipt_sha256,txs,&default_relevance_policy()?)}
fn creation_receipts(source:&str,epoch:&str,slot:u64, blockhash:&str, txs:&[Value], identities:&[FinalizedIdentityReceipt], paths:&[PathReceipt],policy:&CanonicalRelevancePolicy)->Result<Vec<CreationReceipt>> { let mut out=Vec::new(); for (index,tx) in txs.iter().enumerate() { if finalized_relevant_transaction(tx,index,policy)?.is_none() || classify_finalized_pump_migration_candidate(tx)?!=FinalizedCreationCandidate::PumpMigrationCandidate {continue} match validate_finalized_pump_migration(slot,index as u64,tx) { Ok(witnesses)=>for witness in witnesses { let identity=identities.iter().find(|item|item.signature==witness.signature&&item.transaction_index==witness.transaction_index).context("CREATION_IDENTITY_RECEIPT_MISSING")?;let path=paths.iter().find(|item|item.event_id==witness.event_id&&item.disposition=="PATH_RESOLVED_QUARANTINED_UNIVERSE_INCOMPLETE").context("CREATION_PATH_RECEIPT_MISSING")?;if path.instruction_path!=witness.instruction_path {bail!("CREATION_PATH_RECEIPT_MISMATCH")}; out.push(CreationReceipt{schema:"CANONICAL_PUMP_MIGRATE_CREATION_RECEIPT_V2".into(),source_instance_id:source.into(),continuity_epoch:epoch.into(),slot:slot.to_string(),finalized_blockhash:blockhash.into(),signature:witness.signature.clone(),transaction_index:witness.transaction_index.clone(),pool:witness.pool.clone(),mint:witness.mint.clone(),event_id:witness.event_id.clone(),instruction_path:witness.instruction_path.clone(),identity_receipt_sha256:identity.receipt_sha256.clone(),path_receipt_sha256:path.receipt_sha256.clone(),witness:serde_json::to_value(witness)?,disposition:"CREATION_VERIFIED_RESTRICTED_COHORT_QUARANTINED".into(),reason_code:Some("FORWARD_COHORT_NOT_OPENED".into()),receipt_sha256:String::new()}.seal()?);}, Err(error)=>{let signature=tx.pointer("/transaction/signatures/0").and_then(Value::as_str).context("CREATION_SIGNATURE")?;let identity=identities.iter().find(|item|item.signature==signature&&item.transaction_index==index.to_string()).context("CREATION_QUARANTINE_IDENTITY_MISSING")?;out.push(CreationReceipt{schema:"CANONICAL_PUMP_MIGRATE_CREATION_RECEIPT_V2".into(),source_instance_id:source.into(),continuity_epoch:epoch.into(),slot:slot.to_string(),finalized_blockhash:blockhash.into(),signature:signature.into(),transaction_index:index.to_string(),pool:String::new(),mint:String::new(),event_id:String::new(),instruction_path:String::new(),identity_receipt_sha256:identity.receipt_sha256.clone(),path_receipt_sha256:String::new(),witness:json!({"candidateKind":"PUMP_MIGRATE_CREATE_POOL","signature":signature,"transactionIndex":index.to_string()}),disposition:"CREATION_QUARANTINED".into(),reason_code:Some(error.to_string()),receipt_sha256:String::new()}.seal()?);} } } Ok(out) }
async fn rpc_raw(http:&reqwest::Client,url:&str,root:&PathBuf,run:&mut CaptureRunJournal,method:&str,slot:Option<u64>,request:Value)->Result<(Value,RawRpcEvidence)>{
    let bytes=http.post(url).json(&request).send().await?.error_for_status()?.bytes().await?.to_vec();
    // Reserve before a durable write.  The range journal is the one quota
    // owner for raw finalized bytes and receipt output.
    run.checkpoint.resources.reserve_evidence(bytes.len() as u64)?;run.persist()?;
    let suffix=slot.map(|value|value.to_string()).unwrap_or_else(||"range".into());let name=format!("raw-rpc-{method}-{suffix}.json");let temporary=root.join(format!(".{name}.tmp"));fs::write(&temporary,&bytes)?;fs::rename(&temporary,root.join(&name))?;let evidence=RawRpcEvidence::new(method,slot,&name,&bytes)?;evidence.validate()?;let response:Value=serde_json::from_slice(&bytes).context("RPC_RAW_JSON_PARSE")?;Ok((response,evidence))
}
async fn block(http:&reqwest::Client,url:&str,root:&PathBuf,run:&mut CaptureRunJournal,slot:u64)->Result<(FinalizedBlockInput,Vec<Value>,RawRpcEvidence)>{ let (response,evidence)=rpc_raw(http,url,root,run,"getBlock",Some(slot),json!({"jsonrpc":"2.0","id":slot,"method":"getBlock","params":[slot,{"commitment":"finalized","encoding":"json","transactionDetails":"full","rewards":false,"maxSupportedTransactionVersion":1}]})).await?; if response.get("error").is_some(){bail!("RPC_ERROR")} let result=response.get("result").context("RPC_RESULT")?; if result.is_null(){bail!("RPC_BLOCK_UNAVAILABLE")} let raw_transactions=result.get("transactions").and_then(Value::as_array).context("RPC_TRANSACTIONS")?.clone(); let mut transactions=Vec::new(); for (index,tx) in raw_transactions.iter().enumerate(){let mut parsed=relevant(tx)?; parsed.transaction_index=index.to_string(); transactions.push(parsed);} let proof=FinalizedBlockInput {slot:slot.to_string(),block_identity:result.get("blockhash").and_then(Value::as_str).context("RPC_BLOCKHASH")?.to_owned(),finalized:true,executed_transaction_count:transactions.len().to_string(),transactions}; Ok((proof,raw_transactions,evidence)) }
fn producer_slot_bundle_with_policy(source:&str,epoch:&str,reconciliation:ReconciliationReceipt,source_closure:&SourceBlockClosure,transactions:&[Value],events:&HashMap<String,Vec<RawSemanticEvent>>,records:&[FilteredIdentity],policy:&CanonicalRelevancePolicy)->Result<SlotReceiptBundle>{
    if reconciliation.relevance_policy_sha256 != policy.policy_sha256 { bail!("RECEIPT_POLICY_HASH_MISMATCH") }
    let slot:u64=reconciliation.slot.parse()?;let blockhash=reconciliation.block_identity.clone();let source_frames=records.iter().filter(|record|record.slot==reconciliation.slot).map(|record|(record.signature.clone(),record.source_frame_sha256.clone())).collect::<HashMap<_,_>>();let identities=identity_receipts_with_policy(source,epoch,slot,&blockhash,&reconciliation.receipt_sha256,transactions,policy)?;let expected=identities.iter().map(|identity|ExpectedIdentity{slot:identity.slot.clone(),signature:identity.signature.clone(),transaction_index:identity.transaction_index.clone()}).collect::<Vec<_>>();source_closure.validate_against_finalized(source,epoch,&policy.policy_sha256,slot,&blockhash,&expected)?;let mut paths=path_receipts(source,epoch,slot,&blockhash,transactions,events,&source_frames,policy)?;paths.extend(creation_path_receipts(source,epoch,slot,&blockhash,transactions,&source_frames,policy)?);let creations=creation_receipts(source,epoch,slot,&blockhash,transactions,&identities,&paths,policy)?;let bundle=SlotReceiptBundle{reconciliation,source_block_closure_sha256:source_closure.closure_sha256.clone(),identities,paths,creations};bundle.validate()?;Ok(bundle)
}
fn canonicalize_committed_bundles(root:&PathBuf,source:&str,epoch:&str,inventory:&FinalizedSlotInventory)->Result<()> {
    let journal_root=root.join("canonical-semantic-journal");
    // Recovery is deterministic: validate the existing canonical chain from
    // its original first sequence, rebuild state, then append only unseen
    // receipt-backed semantic identities.  A corrupted journal is an error,
    // never an excuse to silently begin a second chain.
    let existing=if journal_root.exists(){pump_exact_evidence_ledger::canonical_state_v1::scan_journal(&journal_root,epoch)?}else{Vec::new()};
    let mut state=CanonicalState::default();state.epoch=epoch.into();
    for event in &existing {state.apply(event)?;}
    let mut writer=JournalWriter::open(&journal_root,epoch,0,4096)?;
    let mut resolver=CausalOrderResolver::default();
    for entry in &inventory.entries {if entry.disposition!="PRODUCED_RECONCILED"{continue}let slot:u64=entry.slot.parse()?;let bundle=SlotReceiptBundle::read(root,slot)?;
        for path in bundle.paths.iter().filter(|path|path.disposition=="PATH_RESOLVED_QUARANTINED_UNIVERSE_INCOMPLETE"){
            if state.seen.contains(&path.event_id){continue}
            let order=&path.canonical_order;let outer=order.get("outerInstructionIndex").and_then(Value::as_str).context("CANONICAL_BRIDGE_OUTER")?;let inner=order.get("innerInstructionIndex").and_then(Value::as_str);let semantic=order.get("semanticOrdinal").and_then(Value::as_str).context("CANONICAL_BRIDGE_ORDINAL")?;let instruction_path=match inner{Some(inner)=>format!("{outer}/{inner}"),None=>outer.to_owned()};
            resolver.push(SemanticEvent{schema:String::new(),canonical_sequence:"0".into(),source_instance_id:source.into(),epoch_id:epoch.into(),event_identity:path.event_id.clone(),causal_order:CausalOrder{slot:path.slot.clone(),transaction_index:Some(path.transaction_index.clone()),instruction_path:Some(instruction_path),semantic_ordinal:Some(semantic.to_owned())},causal_order_status:"RESOLVED".into(),kind:path.semantic_kind.clone(),signature:path.signature.clone(),pool:Some(path.pool.clone()),mint:None,actor:None,side:if path.semantic_kind.ends_with("BUY"){Some("BUY".into())}else if path.semantic_kind.ends_with("SELL"){Some("SELL".into())}else{None},pool_base_raw:None,pool_quote_raw:None,quote_delta_raw:None,fee_tuple:None,observed_at_unix_ns:"0".into(),parser_version:"FINALIZED_PATH_BRIDGE_V1".into(),raw_frame_sha256:Some(path.source_frame_sha256.clone()),previous_record_sha256:String::new(),record_sha256:String::new()})?;
        }
        for mut semantic in resolver.flush_closed_slot(slot)? { semantic.canonical_sequence=state.last_sequence.map(|value|value+1).unwrap_or(0).to_string();let sealed=writer.append(semantic)?;state.apply(&sealed)?; }
    }
    writer.seal_segment()?;
    let receipt=serde_json::json!({"schema":"CANONICAL_PRODUCTION_BRIDGE_RECEIPT_V1","sourceInstanceId":source,"continuityEpoch":epoch,"inventorySha256":inventory.inventory_sha256,"semanticCount":state.last_sequence.map(|value|value+1).unwrap_or(0).to_string(),"universeDisposition":"UNIVERSE_INCOMPLETE","scientificAdmission":"PROHIBITED"});let bytes=pump_exact_evidence_ledger::canonical_state_v1::jcs_bytes(&receipt)?;let temporary=root.join(".canonical-production-bridge-receipt.tmp");fs::write(&temporary,&bytes)?;fs::rename(temporary,root.join("canonical-production-bridge-receipt.json"))?;Ok(())
}
fn diagnostic_range_preflight(budget:&RpcBudget,start:u64,end:u64)->Result<()> { let span=end.checked_sub(start).and_then(|n|n.checked_add(1)).context("FINALIZED_RANGE_INVALID")?;if span>15||budget.used_requests().checked_add(1+span).context("FINALIZED_RANGE_BUDGET_OVERFLOW")?>budget.maximum_requests{bail!("FINALIZED_RANGE_RPC_BUDGET_INSUFFICIENT")}Ok(()) }
fn finalized_slot_inventory_from_mocked_rpc_with_policy(root:&PathBuf,source:&str,epoch:&str,start:u64,end:u64,get_blocks_response:&Value,blocks:Vec<(FinalizedBlockInput,Vec<Value>)>,source_block_closures:&BTreeMap<u64,SourceBlockClosure>,records:&[FilteredIdentity],events:&HashMap<String,Vec<RawSemanticEvent>>,policy:&CanonicalRelevancePolicy)->Result<FinalizedSlotInventory>{
    if end<start||end-start+1>15{bail!("FINALIZED_RANGE_DIAGNOSTIC_CAP")}if get_blocks_response.get("error").is_some(){bail!("GET_BLOCKS_RPC_ERROR")}let result=get_blocks_response.get("result").and_then(Value::as_array).context("GET_BLOCKS_RESULT")?;let mut produced=Vec::new();for value in result{let slot=value.as_u64().context("GET_BLOCKS_SLOT")?;if slot<start||slot>end||produced.contains(&slot){bail!("GET_BLOCKS_INVENTORY_INVALID")}produced.push(slot)}
    // A live range has already persisted exact HTTP bytes at this path before
    // JSON parsing. Synthetic tests deliberately lack it and use a local
    // canonical fixture encoding instead; production never replaces raw
    // provider evidence with a serde reserialization.
    let raw_path=root.join("raw-rpc-getBlocks-range.json");let response_sha=if raw_path.exists(){pump_exact_evidence_ledger::sha256_hex(&fs::read(&raw_path)?)}else{let response_bytes=serde_json::to_vec(get_blocks_response)?;let tmp=root.join(".synthetic-finalized-getblocks-response.tmp");fs::write(&tmp,&response_bytes)?;fs::rename(tmp,root.join("synthetic-finalized-getblocks-response.json"))?;pump_exact_evidence_ledger::sha256_hex(&response_bytes)};let mut blocks_by_slot=BTreeMap::new();for(block,transactions)in blocks{let slot:u64=block.slot.parse()?;if blocks_by_slot.insert(slot,(block,transactions)).is_some(){bail!("FINALIZED_RANGE_DUPLICATE_BLOCK_RESPONSE")}}let mut proofs=Vec::new();for slot in &produced{let(proof,transactions)=blocks_by_slot.remove(slot).context("FINALIZED_RANGE_BLOCK_RESPONSE_MISSING")?;let closure=source_block_closures.get(slot).context("FINALIZED_RANGE_SOURCE_BLOCK_CLOSURE_MISSING")?;let mut receipt=reconcile_finalized_block_with_policy(epoch,source,"1",proof.clone(),records,policy)?;let raw_block_path=root.join(format!("raw-rpc-getBlock-{slot}.json"));if raw_block_path.exists(){receipt=receipt.bind_raw_provider_response(&pump_exact_evidence_ledger::sha256_hex(&fs::read(raw_block_path)?))?;}if receipt.disposition!="MATCHED"||receipt.reason_code.is_some(){bail!("FINALIZED_RANGE_RECONCILIATION_INCOMPLETE")}let bundle=producer_slot_bundle_with_policy(source,epoch,receipt,closure,&transactions,events,records,policy)?;bundle.write_atomic(root)?;proofs.push(ReconciledProducedSlot{slot:slot.to_string(),block_identity:proof.block_identity,slot_reconciliation_receipt_sha256:bundle.reconciliation.receipt_sha256.clone(),source_block_closure_sha256:Some(bundle.source_block_closure_sha256.clone())});}if !blocks_by_slot.is_empty(){bail!("FINALIZED_RANGE_UNEXPECTED_BLOCK_RESPONSE")}let inventory=FinalizedSlotInventory::seal(source,"1",start,end,&response_sha,&produced,proofs)?;inventory.write_atomic(root)?;canonicalize_committed_bundles(root,source,epoch,&inventory)?;Ok(inventory)
}
#[cfg(test)] fn finalized_slot_inventory_from_mocked_rpc(root:&PathBuf,source:&str,epoch:&str,start:u64,end:u64,get_blocks_response:&Value,blocks:Vec<(FinalizedBlockInput,Vec<Value>)>,records:&[FilteredIdentity],events:&HashMap<String,Vec<RawSemanticEvent>>)->Result<FinalizedSlotInventory>{
    let policy=default_relevance_policy()?;let closures=blocks.iter().map(|(block,_)|{let identities=expected_identities_for_block(block,&policy)?.into_iter().map(|identity|SourceBlockIdentity{signature:identity.signature,source_transaction_index:"0".into()}).collect();let slot=block.slot.parse()?;Ok((slot,SourceBlockClosure{schema:String::new(),source_instance_id:source.into(),continuity_epoch:epoch.into(),relevance_policy_sha256:policy.policy_sha256.clone(),slot:block.slot.clone(),blockhash:block.block_identity.clone(),executed_transaction_count:block.executed_transaction_count.clone(),source_block_update_sha256:"0".repeat(64),filtered_identities:identities,closure_sha256:String::new()}.seal()?))}).collect::<Result<BTreeMap<_,_>>>()?;for closure in closures.values(){closure.write_atomic(root)?;}finalized_slot_inventory_from_mocked_rpc_with_policy(root,source,epoch,start,end,get_blocks_response,blocks,&closures,records,events,&policy)
}
async fn fetch_pending_finalized_range(http:&reqwest::Client,url:&str,root:&PathBuf,start:u64,end:u64,budget:&mut RpcBudget,run:&mut CaptureRunJournal,report:&mut Report)->Result<PendingFinalizedRange>{
    diagnostic_range_preflight(budget,start,end)?; run.checkpoint.resources.consume_rpc()?;run.persist()?;budget.consume_one()?; report.inventory_get_blocks_rpc_calls+=1;
    let (response,get_blocks_evidence)=rpc_raw(http,url,root,run,"getBlocks",None,json!({"jsonrpc":"2.0","id":"bounded-finalized-slot-inventory","method":"getBlocks","params":[start,end,{"commitment":"finalized"}]})).await?;
    if response.get("error").is_some(){bail!("GET_BLOCKS_RPC_ERROR")}
    let result=response.get("result").and_then(Value::as_array).context("GET_BLOCKS_RESULT")?;
    let produced=result.len();
    if budget.used_requests().saturating_add(produced as u64)>budget.maximum_requests {bail!("FINALIZED_RANGE_RPC_BUDGET_INSUFFICIENT")}
    let mut blocks=Vec::new();let mut raw_rpc_evidence=vec![get_blocks_evidence];for value in result {let slot=value.as_u64().context("GET_BLOCKS_SLOT")?;run.checkpoint.resources.consume_rpc()?;run.persist()?;budget.consume_one()?;report.inventory_get_block_rpc_calls+=1;let (proof,transactions,evidence)=block(http,url,root,run,slot).await?;raw_rpc_evidence.push(evidence);blocks.push((proof,transactions))}
    Ok(PendingFinalizedRange {start,end,get_blocks_response:response,blocks,raw_rpc_evidence})
}
fn expected_identities_for_block(block:&FinalizedBlockInput,policy:&CanonicalRelevancePolicy)->Result<Vec<ExpectedIdentity>> {
    block.transactions.iter().filter_map(|transaction| match policy.supports_and_matches(transaction) { Ok(true)=>Some(Ok(ExpectedIdentity { slot:block.slot.clone(),signature:transaction.signature.clone(),transaction_index:transaction.transaction_index.clone() })), Ok(false)=>None, Err(error)=>Some(Err(error)) }).collect()
}

fn pending_range_readiness_with_policy(pending:&PendingFinalizedRange,source_finalized_slots:&BTreeSet<u64>,source_block_closures:&BTreeMap<u64,SourceBlockClosure>,source:&str,epoch:&str,records:&[FilteredIdentity],policy:&CanonicalRelevancePolicy)->Result<PendingRangeReadiness>{
    for (block,_) in &pending.blocks {let slot:u64=block.slot.parse()?;if !source_finalized_slots.contains(&slot){return Ok(PendingRangeReadiness::AwaitingSourceFinalizedStatus)}}
    for (block,_) in &pending.blocks {
        let slot:u64=block.slot.parse()?;
        let Some(closure)=source_block_closures.get(&slot) else { return Ok(PendingRangeReadiness::AwaitingSourceBlockClosure) };
        // The closure proves source-side completion for the exact program
        // filter. Its index is intentionally not compared with finalized RPC.
        closure.validate_against_finalized(source,epoch,&policy.policy_sha256,slot,&block.block_identity,&expected_identities_for_block(block,policy)?)?;
    }
    for (block,_) in &pending.blocks {let receipt=reconcile_finalized_block_with_policy(epoch,source,"1",block.clone(),records,policy)?;match (receipt.disposition.as_str(),receipt.reason_code.as_deref()) {("MATCHED",None)=>{},("REJECTED",Some("MISSING_FILTERED_IDENTITY"))=>return Ok(PendingRangeReadiness::AwaitingSourceIdentities),_=>bail!("FINALIZED_RANGE_SOURCE_RECONCILIATION_REJECTED:{:?}",receipt.reason_code)}}
    Ok(PendingRangeReadiness::ReadyToSeal)
}
#[cfg(test)] fn pending_range_readiness(pending:&PendingFinalizedRange,source_finalized_slots:&BTreeSet<u64>,source_block_closures:&BTreeMap<u64,SourceBlockClosure>,source:&str,epoch:&str,records:&[FilteredIdentity])->Result<PendingRangeReadiness>{pending_range_readiness_with_policy(pending,source_finalized_slots,source_block_closures,source,epoch,records,&default_relevance_policy()?)}
fn seal_pending_finalized_range_with_policy(root:&PathBuf,source:&str,epoch:&str,pending:&PendingFinalizedRange,source_finalized_slots:&BTreeSet<u64>,source_block_closures:&BTreeMap<u64,SourceBlockClosure>,records:&[FilteredIdentity],events:&HashMap<String,Vec<RawSemanticEvent>>,policy:&CanonicalRelevancePolicy)->Result<Option<FinalizedSlotInventory>>{
    if pending_range_readiness_with_policy(pending,source_finalized_slots,source_block_closures,source,epoch,records,policy)?!=PendingRangeReadiness::ReadyToSeal{return Ok(None)}
    Ok(Some(finalized_slot_inventory_from_mocked_rpc_with_policy(root,source,epoch,pending.start,pending.end,&pending.get_blocks_response,pending.blocks.clone(),source_block_closures,records,events,policy)?))
}
fn records(path:&PathBuf,epoch:&str)->Result<Vec<FilteredIdentity>>{ let text=fs::read_to_string(path.join(format!("{epoch}.filtered-identities.jsonl")))?; text.lines().map(|l|serde_json::from_str(l).map_err(Into::into)).collect() }

fn all_manifest_slot_count(path: &PathBuf, epoch: &str) -> Result<usize> {
    Ok(records(path, epoch)?.into_iter().map(|record| record.slot).collect::<BTreeSet<_>>().len())
}

fn now_ms() -> u64 { Utc::now().timestamp_millis().max(0) as u64 }

fn persist_state(root: &PathBuf, source: &str, dispatcher: &Dispatcher, terminal: &[(u64, String)]) -> Result<()> {
    let slots = dispatcher.slots.iter().map(|(slot, work)| serde_json::json!({
        "slot": slot.to_string(), "state": format!("{:?}", work.state), "attempts": work.attempts,
        "identities": work.identities, "nextAttemptMs": work.next_attempt_ms.to_string(), "failure": work.failure,
    })).collect::<Vec<_>>();
    let temporary = root.join("dispatcher-state.tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(&serde_json::json!({
        "schema": "CANONICAL_FINALIZED_DISPATCHER_STATE_V1", "sourceInstanceId": source,
        "slots": slots, "terminal": terminal,
    }))?)?;
    fs::rename(temporary, root.join("dispatcher-state.json"))?;
    Ok(())
}

async fn integrated_main() -> Result<()> {
    rustls::crypto::ring::default_provider().install_default().map_err(|_|anyhow::anyhow!("RUSTLS"))?;
    let root=PathBuf::from(env::var("FINALIZED_RECONCILIATION_CANARY_ROOT")?);
    let endpoint=env::var("YELLOWSTONE_FULL_META_ENDPOINT")?;
    let token:AsciiMetadataValue=env::var("SHYFT_GRPC_X_TOKEN")?.parse()?;
    let rpc=env::var("SECURE_RPC_URL")?;
    let reserve: u64=env::var("CANONICAL_CANARY_PROTECTED_RESERVE_BYTES").unwrap_or_else(|_|PROTECTED_RESERVE.to_string()).parse()?;
    let start_floor: u64=env::var("CANONICAL_CANARY_START_FLOOR_BYTES").unwrap_or_else(|_|(181_u64*1024*1024*1024).to_string()).parse()?;
    let total_cap: u64=env::var("CANONICAL_CANARY_TOTAL_CAP_BYTES").unwrap_or_else(|_|(6_u64*1024*1024*1024).to_string()).parse()?;
    let max_seconds: u64=env::var("CANONICAL_CANARY_MAX_SECONDS").unwrap_or_else(|_|(45_u64*60).to_string()).parse()?;
    if env::var_os("FINALIZED_RANGE_START_SLOT").is_some()||env::var_os("FINALIZED_RANGE_END_SLOT").is_some(){bail!("FINALIZED_RANGE_STATIC_BOUNDARY_UNSAFE")}
    let range_slots=parse_diagnostic_range_slots(env::var("FINALIZED_RANGE_SLOT_COUNT").ok().as_deref())?;
    let mut budget=RpcBudget::new(16,1,1000)?;
    let reservation=range_slots.map(|slots|diagnostic_rpc_reservation(&budget,0,slots-1)).transpose()?;
    fs::create_dir_all(&root)?;
    let start_free=free(&root)?;
    validate_storage_policy(start_free,reserve,start_floor,total_cap)?;
    // A previous incomplete source instance may not be bridged after a process
    // restart. Its manifest is durable audit evidence, not resumable input.
    ensure_fresh_output_root(&root)?;
    let source=format!("finalized-rpc-canary-{}-{}",Utc::now().format("%Y%m%dT%H%M%SZ"),std::process::id());
    let epoch="canary-epoch-1";
    let relevance_policy=CanonicalRelevancePolicy::new("PUMPSWAP_FINALIZED_RELEVANCE_V1",programs().into_iter().collect(),1)?;
    let run_binding=serde_json::json!({"schema":"CANONICAL_FINALIZED_CANARY_BINDING_V2","sourceInstanceId":source,"continuityEpoch":"1","startupFreeBytes":start_free.to_string(),"startupFloorBytes":start_floor.to_string(),"runtimeReserveBytes":reserve.to_string(),"totalOutputCapBytes":total_cap.to_string(),"relevancePolicyVersion":relevance_policy.policy_version,"relevancePolicySha256":relevance_policy.policy_sha256});
    fs::write(root.join("run-binding.json"),serde_json::to_vec_pretty(&run_binding)?)?;
    let mut report=Report{schema:"CANONICAL_FINALIZED_RPC_RECONCILIATION_CANARY_V1".into(),status:"RUNNING".into(),started_at_utc:Utc::now().to_rfc3339(),finished_at_utc:String::new(),slots_observed:0,slots_reconciled:0,slots_rejected:0,exact_passes:0,failures:0,rpc_calls:0,dispatcher_rpc_calls:0,dispatcher_retry_rpc_calls:0,inventory_get_blocks_rpc_calls:0,inventory_get_block_rpc_calls:0,retries:0,pending_slot_peak:0,max_rss_bytes:0,max_message_bytes:0,disk_growth_bytes:0,decoder_transactions:0,decoder_events:0,decoder_rejections:0,source_gap_or_epoch_failure:false,unresolved:"NONE".into(),reconciled_slot_produced:false,scientific_interval_produced:false,failure:None};
    let result:Result<()> = async {
        let policy=StoragePolicy{manifest_quota_bytes:MANIFEST_QUOTA,protected_free_bytes:reserve,available_free_bytes:start_free,max_in_memory_identities:16384};
        let mut run=CaptureRunJournal::new(&root,"finalized-rpc-diagnostic",&source,"1",&relevance_policy,ResourceAccounting::new(16,total_cap,MAX_RSS,32)?)?;
        let mut manifest=FilteredIdentityManifestWriter::open(&root,epoch,&source,"1",0,0,policy)?;
        let channel=Channel::from_shared(endpoint)?.tls_config(ClientTlsConfig::new().with_native_roots())?.connect_timeout(Duration::from_secs(10)).connect().await?;
        let t=token.clone();
        let mut client=GeyserClient::with_interceptor(channel,move|mut r:Request<()>|{r.metadata_mut().insert("x-token",t.clone());Ok::<_,Status>(r)}).max_decoding_message_size(MAX_MESSAGE);
        let subscription=req(&relevance_policy)?;
        let mut updates=client.subscribe(Request::new(stream::once(async move{subscription}).chain(stream::pending()))).await?.into_inner();
        let http=reqwest::Client::builder().timeout(Duration::from_secs(20)).build()?;
        let mut dispatcher=Dispatcher::new(32,3,1000)?;
        let mut finalized_slots=BTreeSet::new(); let mut source_block_closures=BTreeMap::<u64,SourceBlockClosure>::new(); let mut all_records=Vec::<FilteredIdentity>::new(); let mut raw_events=HashMap::<String,Vec<RawSemanticEvent>>::new(); let mut terminal=Vec::<(u64,String)>::new();let mut pending_range=None::<PendingFinalizedRange>;let mut range_inventory_complete=initial_range_inventory_complete(range_slots);let mut finality_receive_sequence=0_u64;
        let started=std::time::Instant::now();
        while started.elapsed()<Duration::from_secs(max_seconds) && !range_inventory_complete {
            match tokio::time::timeout(Duration::from_millis(250),updates.next()).await {
                Ok(Some(Ok(update))) => {
                    let size=update.encoded_len(); let update_raw=update.encode_to_vec();let update_sha=pump_exact_evidence_ledger::sha256_hex(&update_raw);report.max_message_bytes=report.max_message_bytes.max(size); report.max_rss_bytes=report.max_rss_bytes.max(rss());
                    if size>MAX_MESSAGE { bail!("MESSAGE_CAP") } run.checkpoint.resources.observe_memory(report.max_rss_bytes)?; if report.max_rss_bytes>MAX_RSS { bail!("RSS_CAP") }
                    if free(&root)? < reserve.saturating_add(total_cap.saturating_sub(fs::metadata(root.join(format!("{epoch}.filtered-identities.jsonl"))).map(|m|m.len()).unwrap_or(0))) { bail!("PROTECTED_RUNTIME_RESERVE") }
                    let observation=match update.update_oneof {
                        Some(UpdateOneof::Transaction(item)) => {
                            let info=item.transaction.clone().context("TRANSACTION_INFO")?; let signature=bs58::encode(&info.signature).into_string();
                            let raw_hash=pump_exact_evidence_ledger::sha256_hex(&item.encode_to_vec());
                            let (semantic_events,decoder_rejected)=match info.meta.as_ref(){Some(meta)=>match decode_events(meta){Ok(events)=>(events.into_iter().map(|(log_index,event)|RawSemanticEvent{event_id:format!("{signature}:{log_index}"),log_index,event}).collect(),false),Err(_)=>(Vec::new(),true)},None=>(Vec::new(),false)};
                            Some(SourceObservation::Transaction{slot:item.slot,signature,source_transaction_index:info.index,received_at_unix_ns:Utc::now().timestamp_nanos_opt().unwrap_or_default() as u128,source_frame_sha256:raw_hash,semantic_events,decoder_rejected})
                        }
                        Some(UpdateOneof::Slot(slot)) if slot.status==SlotStatus::SlotFinalized as i32 => Some(SourceObservation::FinalizedSlot{slot:slot.slot,source_frame_sha256:update_sha}),
                        Some(UpdateOneof::Block(block)) => Some(SourceObservation::BlockClosure{closure:source_block_closure_from_update(&source,"1",&relevance_policy,&block,update_sha)?}),
                        _ => None,
                    };
                    if let Some(observation)=observation {apply_source_observation(observation,&source,&mut run,&mut manifest,&mut dispatcher,&mut finalized_slots,&mut source_block_closures,&mut all_records,&mut raw_events,&mut report,&mut finality_receive_sequence,range_slots.is_none())?;}
                }
                Ok(Some(Err(error))) => { report.source_gap_or_epoch_failure=true; bail!("SOURCE_GAP:{error}") }
                Ok(None) => { report.source_gap_or_epoch_failure=true; bail!("SOURCE_ENDED") }
                Err(_) => {}
            }
            report.pending_slot_peak=report.pending_slot_peak.max(dispatcher.active());run.checkpoint.resources.observe_queue(dispatcher.active() as u64)?;
            while range_slots.is_none() && !range_inventory_complete { let Some(slot)=dispatcher.due(now_ms()) else {break};
                let retry_attempt=dispatcher.slots.get(&slot).is_some_and(|work|work.attempts>0);
                consume_dispatcher_rpc(&mut budget,reservation)?;run.checkpoint.resources.consume_rpc()?;run.persist()?; report.dispatcher_rpc_calls+=1;if retry_attempt{report.dispatcher_retry_rpc_calls+=1;}
                let disposition=match block(&http,&rpc,&root,&mut run,slot).await {
                    Ok((proof,transactions,evidence)) => { let receipt=reconcile_finalized_block_with_policy(epoch,&source,"1",proof,&all_records,&relevance_policy)?.bind_raw_provider_response(&evidence.raw_sha256)?; if receipt.disposition=="MATCHED" { let closure=source_block_closures.get(&slot).context("SOURCE_BLOCK_CLOSURE_MISSING_FOR_RECONCILED_SLOT")?; let bundle=producer_slot_bundle_with_policy(&source,"1",receipt,closure,&transactions,&raw_events,&all_records,&relevance_policy)?;bundle.write_atomic(&root)?;"MATCHED".to_owned() } else { report.unresolved=receipt.reason_code.unwrap_or_else(||"RECONCILIATION_REJECTED".into()); report.unresolved.clone() } }
                    Err(error) if error.to_string().contains("RPC_BLOCK_UNAVAILABLE") => "RPC_NOT_FINALIZED".to_owned(),
                    Err(error) => { report.unresolved=error.to_string(); report.unresolved.clone() }
                };
                if disposition=="MISSING_FILTERED_IDENTITY" || disposition=="RPC_NOT_FINALIZED" { report.retries+=1; }
                dispatcher.result(slot,now_ms(),&disposition)?; persist_state(&root,&source,&dispatcher,&terminal)?;if disposition=="MATCHED" {break}
            }
            for (slot,state) in dispatcher.drain_completed() { let outcome=match state { SlotState::Matched=>"MATCHED", SlotState::Failed=>"FAILED", _=>"INVALID" }.to_owned(); if outcome=="MATCHED" {report.exact_passes+=1; report.slots_reconciled+=1; report.reconciled_slot_produced=true;if range_slots.is_none(){range_inventory_complete=true}} else {report.failures+=1; report.slots_rejected+=1;} terminal.push((slot,outcome)); persist_state(&root,&source,&dispatcher,&terminal)?; }
            if pending_range.is_none(){if let Some(slots)=range_slots {if run.checkpoint.lifecycle==CaptureLifecycle::CapturingSource {match run.select_range(slots) {Ok((start,end))=>{let pending=fetch_pending_finalized_range(&http,&rpc,&root,start,end,&mut budget,&mut run,&mut report).await?;let expected=pending.blocks.iter().flat_map(|(proof,_)|proof.transactions.iter().filter(|transaction|relevance_policy.supports_and_matches(transaction).unwrap_or(false)).map(|transaction|ExpectedIdentity{slot:proof.slot.clone(),signature:transaction.signature.clone(),transaction_index:transaction.transaction_index.clone()})).collect::<Vec<_>>();let pending_sha=pump_exact_evidence_ledger::sha256_hex(&pump_exact_evidence_ledger::canonical_state_v1::jcs_bytes(&serde_json::to_value(&pending.raw_rpc_evidence)?)?);run.record_inventory(&pending_sha,expected,pending.raw_rpc_evidence.clone())?;pending_range=Some(pending);},Err(error) if error.to_string().contains("NO_CONTIGUOUS_SOURCE_FINALITY_RANGE")=>{},Err(error)=>return Err(error)}}}}
            if let Some(pending)=pending_range.as_ref(){match pending_range_readiness_with_policy(pending,&finalized_slots,&source_block_closures,&source,epoch,&all_records,&relevance_policy)? {PendingRangeReadiness::AwaitingSourceFinalizedStatus=>report.unresolved="WAITING_FOR_SOURCE_FINALIZED_STATUS".into(),PendingRangeReadiness::AwaitingSourceBlockClosure=>report.unresolved="WAITING_FOR_SOURCE_BLOCK_CLOSURE".into(),PendingRangeReadiness::AwaitingSourceIdentities=>report.unresolved="WAITING_FOR_SOURCE_IDENTITIES".into(),PendingRangeReadiness::ReadyToSeal=>{run.stage_receipts()?;let inventory=seal_pending_finalized_range_with_policy(&root,&source,epoch,pending,&finalized_slots,&source_block_closures,&all_records,&raw_events,&relevance_policy)?.context("FINALIZED_RANGE_SEAL_READY_MISSING")?;run.checkpoint.finalized_inventory_sha256=Some(inventory.inventory_sha256.clone());run.persist()?;run.commit_receipts()?;report.exact_passes+=inventory.entries.iter().filter(|entry|entry.disposition=="PRODUCED_RECONCILED").count();report.slots_reconciled=report.exact_passes;report.reconciled_slot_produced=report.exact_passes>0;range_inventory_complete=true;}}}
        }
        if !report.reconciled_slot_produced { bail!("NO_EXACT_FINALIZED_RECONCILIATION_PASS") }
        if range_slots.is_some()&&!range_inventory_complete {if pending_range.is_some(){bail!("FINALIZED_RANGE_SOURCE_EVIDENCE_INCOMPLETE")}bail!("FINALIZED_RANGE_NOT_REACHED_BEFORE_RUNTIME_CAP")}
        // A reconciled block proves only filtered identity coverage for that
        // block. It is deliberately not an eligible-pool universe or a full
        // scientific market interval, so no semantic state is released here.
        Ok(())
    }.await;
    report.rpc_calls=budget.used_requests();report.slots_observed=all_manifest_slot_count(&root, epoch).unwrap_or(0); report.finished_at_utc=Utc::now().to_rfc3339(); report.max_rss_bytes=report.max_rss_bytes.max(rss()); report.disk_growth_bytes=start_free.saturating_sub(free(&root)?);
    match &result { Ok(_)=>report.status="FALLBACK_CANARY_VERIFIED".into(), Err(error)=>{report.status="FALLBACK_CANARY_FAILED_OR_BLOCKED".into();report.failure=Some(error.to_string())} }
    fs::write(root.join("report.json"),serde_json::to_vec_pretty(&report)?)?;
    result
}

#[tokio::main]
async fn main()->Result<()> { integrated_main().await }

#[cfg(test)]
mod integrated_tests {
    use super::*;

    fn pending_block(slot:u64, signatures:&[&str])->(FinalizedBlockInput,Vec<Value>){
        (FinalizedBlockInput{slot:slot.to_string(),block_identity:format!("block-{slot}"),finalized:true,executed_transaction_count:signatures.len().to_string(),transactions:signatures.iter().enumerate().map(|(index,signature)|FinalizedTransactionInput{transaction_index:index.to_string(),signature:(*signature).into(),account_or_program_ids:vec![PUMP.into()],supported_version:true}).collect()},vec![])
    }
    fn pending_fixture()->PendingFinalizedRange{PendingFinalizedRange{start:100,end:104,get_blocks_response:json!({"jsonrpc":"2.0","id":"fixture","result":[100,101,102,103,104]}),blocks:(100..=104).map(|slot|pending_block(slot,&[&format!("sig-{slot}")])).collect(),raw_rpc_evidence:vec![]}}
    fn closures_for(pending:&PendingFinalizedRange, source:&str, epoch:&str)->BTreeMap<u64,SourceBlockClosure>{
        let policy=default_relevance_policy().unwrap();
        pending.blocks.iter().map(|(block,_)|{let slot=block.slot.parse().unwrap();let identities=expected_identities_for_block(block,&policy).unwrap().into_iter().map(|identity|SourceBlockIdentity{signature:identity.signature,source_transaction_index:"999".into()}).collect();(slot,SourceBlockClosure{schema:String::new(),source_instance_id:source.into(),continuity_epoch:epoch.into(),relevance_policy_sha256:policy.policy_sha256.clone(),slot:block.slot.clone(),blockhash:block.block_identity.clone(),executed_transaction_count:block.executed_transaction_count.clone(),source_block_update_sha256:"c".repeat(64),filtered_identities:identities,closure_sha256:String::new()}.seal().unwrap())}).collect()
    }
    fn fixture_records(root:&PathBuf,slots:&[u64])->Vec<FilteredIdentity>{
        let policy=StoragePolicy{manifest_quota_bytes:1024*1024,protected_free_bytes:0,available_free_bytes:1024*1024*1024,max_in_memory_identities:32};
        let mut writer=FilteredIdentityManifestWriter::open(root,"epoch","source","1",0,0,policy).unwrap();
        for slot in slots {writer.append(*slot,&format!("sig-{slot}"),*slot as u128,Some(0),"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();}
        records(root,"epoch").unwrap()
    }
    fn fixture_root(label:&str)->PathBuf{let root=std::env::temp_dir().join(format!("canonical-range-{label}-{}",std::process::id()));let _=fs::remove_dir_all(&root);fs::create_dir_all(&root).unwrap();root}
    fn mark_capture_verified(root:&PathBuf,start:u64,end:u64,inventory:&FinalizedSlotInventory){
        let policy=default_relevance_policy().unwrap();let mut run=CaptureRunJournal::new(root,"fixture","source","1",&policy,ResourceAccounting::new(16,6*1024*1024,8*1024*1024,32).unwrap()).unwrap();
        for slot in start..=end {run.observe_finality(SourceFinalityRecord{schema:String::new(),source_instance_id:"source".into(),continuity_epoch:"1".into(),slot:slot.to_string(),receive_sequence:slot.to_string(),source_frame_sha256:"a".repeat(64),record_sha256:String::new()}.seal().unwrap()).unwrap();}
        run.select_range(end-start+1).unwrap();run.record_inventory(&inventory.inventory_sha256,vec![],vec![RawRpcEvidence::new("getBlocks",None,"synthetic-finalized-getblocks-response.json",b"fixture").unwrap()]).unwrap();run.stage_receipts().unwrap();run.commit_receipts().unwrap();
    }

    #[test]
    fn subscription_binds_filtered_transactions_and_explicit_finalized_progress() {
        let request = req(&default_relevance_policy().unwrap()).unwrap();
        assert_eq!(request.commitment, Some(CommitmentLevel::Finalized as i32));
        assert_eq!(request.transactions.get("canonicalRelevant").and_then(|filter| filter.failed), Some(false));
        assert_eq!(request.transactions.get("canonicalRelevant").and_then(|filter| filter.vote), Some(false));
        assert_eq!(request.blocks.get("canonicalRelevantClosure").and_then(|filter| filter.include_transactions), Some(true));
        assert_eq!(request.blocks.get("canonicalRelevantClosure").and_then(|filter| filter.include_accounts), Some(false));
        assert_eq!(request.slots.get("finalizedProgress").and_then(|filter| filter.filter_by_commitment), Some(true));
        assert_eq!(request.slots.get("finalizedProgress").and_then(|filter| filter.interslot_updates), Some(false));
    }

    #[test]
    fn fifth_finalized_before_late_source_identities_waits_then_becomes_sealable() {
        let pending=pending_fixture();let finalized=(100..=104).collect::<BTreeSet<_>>();
        let closures=closures_for(&pending,"source","epoch");
        let incomplete_root=fixture_root("late-before");let incomplete=fixture_records(&incomplete_root,&[100,101,102,103]);
        assert_eq!(pending_range_readiness(&pending,&finalized,&closures,"source","epoch",&incomplete).unwrap(),PendingRangeReadiness::AwaitingSourceIdentities);
        let complete_root=fixture_root("late-after");let complete=fixture_records(&complete_root,&[100,101,102,103,104]);
        assert_eq!(pending_range_readiness(&pending,&finalized,&closures,"source","epoch",&complete).unwrap(),PendingRangeReadiness::ReadyToSeal);
        let _=fs::remove_dir_all(incomplete_root);let _=fs::remove_dir_all(complete_root);
    }

    #[test]
    fn range_waits_for_each_produced_slot_source_finalized_status_not_a_higher_slot() {
        let pending=pending_fixture();let records_root=fixture_root("finalized-status");let records=fixture_records(&records_root,&[100,101,102,103,104]);
        let missing_fifth=(100..=103).collect::<BTreeSet<_>>();
        assert_eq!(pending_range_readiness(&pending,&missing_fifth,&closures_for(&pending,"source","epoch"),"source","epoch",&records).unwrap(),PendingRangeReadiness::AwaitingSourceFinalizedStatus);
        let _=fs::remove_dir_all(records_root);
    }

    #[test]
    fn missing_fifth_source_evidence_cannot_seal_complete_inventory_and_zero_relevant_remains_valid() {
        let pending=pending_fixture();let finalized=(100..=104).collect::<BTreeSet<_>>();let root=fixture_root("missing");let records=fixture_records(&root,&[100,101,102,103]);
        assert_ne!(pending_range_readiness(&pending,&finalized,&closures_for(&pending,"source","epoch"),"source","epoch",&records).unwrap(),PendingRangeReadiness::ReadyToSeal);
        let zero=PendingFinalizedRange{start:200,end:204,get_blocks_response:json!({"jsonrpc":"2.0","id":"zero","result":[200,201,202,203,204]}),blocks:(200..=204).map(|slot|pending_block(slot,&[])).collect(),raw_rpc_evidence:vec![]};
        assert_eq!(pending_range_readiness(&zero,&(200..=204).collect(),&closures_for(&zero,"source","epoch"),"source","epoch",&[]).unwrap(),PendingRangeReadiness::ReadyToSeal);
        let _=fs::remove_dir_all(root);
    }

    #[test]
    fn finalized_status_without_a_filtered_block_closure_cannot_seal() {
        let pending=pending_fixture();let finalized=(100..=104).collect::<BTreeSet<_>>();let root=fixture_root("missing-closure");let records=fixture_records(&root,&[100,101,102,103,104]);
        assert_eq!(pending_range_readiness(&pending,&finalized,&BTreeMap::new(),"source","epoch",&records).unwrap(),PendingRangeReadiness::AwaitingSourceBlockClosure);
        let _=fs::remove_dir_all(root);
    }

    #[test]
    fn rpc_failed_transaction_is_never_relevant_and_versions_are_bounded() {
        let failed = serde_json::json!({"transaction":{"signatures":["s"],"message":{"accountKeys":[PUMP]}},"meta":{"err":{"InstructionError":[0,"x"]}},"version":0});
        assert!(relevant(&failed).unwrap().account_or_program_ids.is_empty());
        let unsupported = serde_json::json!({"transaction":{"signatures":["s"],"message":{"accountKeys":[PUMP]}},"meta":{"err":null},"version":2});
        assert!(!relevant(&unsupported).unwrap().supported_version);
    }

    #[test]
    fn identity_receipts_share_exact_reconciliation_relevance_and_version_rules() {
        let successful_relevant = json!({"transaction":{"signatures":["relevant"],"message":{"accountKeys":[PUMP]}},"meta":{"err":null},"version":0});
        let successful_unrelated = json!({"transaction":{"signatures":["unrelated"],"message":{"accountKeys":["other"]}},"meta":{"err":null},"version":0});
        let failed_relevant = json!({"transaction":{"signatures":["failed"],"message":{"accountKeys":[PUMPSWAP_PROGRAM]}},"meta":{"err":{"InstructionError":[0,"x"]}},"version":0});
        let accepted_raw = vec![successful_relevant, successful_unrelated, failed_relevant];
        let mut inputs = accepted_raw.iter().enumerate().map(|(index, tx)| {
            let mut input = relevant(tx).unwrap(); input.transaction_index = index.to_string(); input
        }).collect::<Vec<_>>();
        let root = fixture_root("mixed-receipts");
        let policy=StoragePolicy{manifest_quota_bytes:1024*1024,protected_free_bytes:0,available_free_bytes:1024*1024*1024,max_in_memory_identities:8};
        let mut manifest=FilteredIdentityManifestWriter::open(&root,"epoch","source","1",0,0,policy).unwrap();
        manifest.append(42,"relevant",1,Some(0),"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        let reconciliation=reconcile_finalized_block("epoch","source","1",FinalizedBlockInput{slot:"42".into(),block_identity:"block".into(),finalized:true,executed_transaction_count:inputs.len().to_string(),transactions:inputs.clone()},&records(&root,"epoch").unwrap(),&programs()).unwrap();
        assert_eq!(reconciliation.disposition,"MATCHED");
        let receipts=identity_receipts("source","1",42,"block",&reconciliation.receipt_sha256,&accepted_raw).unwrap();
        let receipt_set=receipts.iter().map(|receipt|(receipt.signature.clone(),receipt.transaction_index.clone())).collect::<BTreeSet<_>>();
        let reconciled_set=inputs.iter().filter(|input|input.is_relevant_to(&programs())).map(|input|(input.signature.clone(),input.transaction_index.clone())).collect::<BTreeSet<_>>();
        assert_eq!(receipt_set,reconciled_set);
        assert_eq!(receipt_set,BTreeSet::from([("relevant".into(),"0".into())]));
        SlotReceiptBundle{reconciliation,source_block_closure_sha256:"a".repeat(64),identities:receipts,paths:vec![],creations:vec![]}.validate().unwrap();

        let unsupported = json!({"transaction":{"signatures":["unsupported"],"message":{"accountKeys":[PUMP]}},"meta":{"err":null},"version":2});
        let mut unsupported_input=relevant(&unsupported).unwrap(); unsupported_input.transaction_index="3".into(); inputs.push(unsupported_input);
        assert!(reconcile_finalized_block("epoch","source","1",FinalizedBlockInput{slot:"42".into(),block_identity:"block".into(),finalized:true,executed_transaction_count:inputs.len().to_string(),transactions:inputs},&records(&root,"epoch").unwrap(),&programs()).is_err());
        let mut unsupported_raw=accepted_raw; unsupported_raw.push(unsupported);
        assert!(identity_receipts("source","1",42,"block","aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",&unsupported_raw).is_err());
        let _=fs::remove_dir_all(root);
    }

    #[test]
    fn storage_policy_requires_full_future_output_above_reserve() {
        let reserve = 160_u64 * 1024 * 1024 * 1024;
        let cap = 6_u64 * 1024 * 1024 * 1024;
        let floor=181_u64 * 1024 * 1024 * 1024;
        assert!(validate_storage_policy(floor,reserve,floor,cap).is_ok());
        assert!(validate_storage_policy(floor-1,reserve,floor,cap).is_err());
        assert!(validate_storage_policy(floor,reserve,floor,cap-1).is_err());
    }

    #[test]
    fn five_slot_reservation_prevents_dispatcher_starvation() {
        let mut budget=RpcBudget::new(16,1,1).unwrap();
        let reservation=diagnostic_rpc_reservation(&budget,100,104).unwrap();
        assert_eq!(reservation,DiagnosticRpcReservation{range_slots:5,inventory_requests:6,dispatcher_limit:10});
        for _ in 0..10 {consume_dispatcher_rpc(&mut budget,Some(reservation)).unwrap();}
        assert_eq!(budget.used_requests(),10);
        assert!(consume_dispatcher_rpc(&mut budget,Some(reservation)).is_err());
        assert!(diagnostic_range_preflight(&budget,100,104).is_ok());
        for _ in 0..reservation.inventory_requests { budget.consume_one().unwrap(); }
        assert_eq!(budget.used_requests(),16);
        assert!(budget.consume_one().is_err());
    }

    #[test]
    fn retries_consume_reserved_dispatcher_allocation_and_zero_prior_requests_are_valid() {
        let mut budget=RpcBudget::new(16,1,1).unwrap();
        let reservation=diagnostic_rpc_reservation(&budget,200,204).unwrap();
        assert!(diagnostic_range_preflight(&budget,200,204).is_ok());
        let mut dispatcher=Dispatcher::new(1,3,1).unwrap();dispatcher.observe(200,0).unwrap();dispatcher.finalized_progress(200,0).unwrap();
        for (now,result) in [(0,"RPC_NOT_FINALIZED"),(1,"RPC_NOT_FINALIZED"),(3,"MATCHED")] {assert_eq!(dispatcher.due(now),Some(200));consume_dispatcher_rpc(&mut budget,Some(reservation)).unwrap();dispatcher.result(200,now,result).unwrap();}
        assert_eq!(budget.used_requests(),3);
        assert!(diagnostic_range_preflight(&budget,200,204).is_ok());
        for _ in 0..reservation.inventory_requests { budget.consume_one().unwrap(); }
        assert_eq!(budget.used_requests(),9);
    }

    #[test]
    fn no_range_mode_enters_the_original_single_slot_collection_loop() {
        assert!(!initial_range_inventory_complete(None));
        assert!(!initial_range_inventory_complete(Some(5)));
    }

    #[test]
    fn automatic_range_requires_finalized_end_and_rejects_invalid_slot_count() {
        assert_eq!(parse_diagnostic_range_slots(Some("5")).unwrap(),Some(5));
        assert!(parse_diagnostic_range_slots(Some("2")).is_err());assert!(parse_diagnostic_range_slots(Some("6")).is_err());
        assert!(!finalized_range_ready(500,5,Some(503)).unwrap());
        assert!(finalized_range_ready(500,5,Some(504)).unwrap());
    }

    #[test]
    fn fresh_root_and_storage_guards_fail_closed() {
        let root=std::env::temp_dir().join(format!("canonical-fresh-root-{}",std::process::id()));let _=fs::remove_dir_all(&root);fs::create_dir_all(&root).unwrap();
        assert!(ensure_fresh_output_root(&root).is_ok());fs::write(root.join("run-binding.json"),b"prior").unwrap();assert!(ensure_fresh_output_root(&root).is_err());let _=fs::remove_dir_all(root);
    }

    fn key(byte: u8) -> String { bs58::encode([byte; 32]).into_string() }
    fn creation_fixture() -> Value {
        let mut create = vec![0_u8; 59]; create[..8].copy_from_slice(&CREATE_POOL); create[10..18].copy_from_slice(&1000_u64.to_le_bytes()); create[18..26].copy_from_slice(&2000_u64.to_le_bytes()); create[26..58].copy_from_slice(&[9;32]);
        let mut event = vec![0_u8;344]; event[..8].copy_from_slice(&[177,49,12,210,160,118,167,116]); event[18..50].copy_from_slice(&[4;32]);event[50..82].copy_from_slice(&[5;32]);event[82..114].copy_from_slice(&[6;32]);event[116..124].copy_from_slice(&1000_u64.to_le_bytes());event[124..132].copy_from_slice(&2000_u64.to_le_bytes());event[173..205].copy_from_slice(&[2;32]);event[301..333].copy_from_slice(&[9;32]);
        let mut keys=vec![PUMPSWAP_PROGRAM.into(),PUMP.into()];keys.extend((2..20).map(key));
        serde_json::json!({"transaction":{"signatures":["sig"],"message":{"accountKeys":keys,"instructions":[{"programIdIndex":1,"accounts":[],"data":""}]}},"meta":{"err":null,"logMessages":[format!("Program {PUMP} invoke [1]"),"Program log: Instruction: Migrate",format!("Program {PUMPSWAP_PROGRAM} invoke [2]"),format!("Program data: {}",base64::Engine::encode(&base64::engine::general_purpose::STANDARD,event)),format!("Program {PUMPSWAP_PROGRAM} success"),format!("Program {PUMP} success")],"innerInstructions":[{"index":0,"instructions":[{"programIdIndex":3,"accounts":[],"data":"","stackHeight":2},{"programIdIndex":0,"accounts":[2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19],"data":bs58::encode(create).into_string(),"stackHeight":2}]}]}})
    }
    #[test]
    fn producer_creation_receipt_is_bound_to_independent_authoritative_path() {
        let tx=creation_fixture(); let txs=vec![tx];
        let identities=identity_receipts("source","1",42,"block","aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",&txs).unwrap();
        let frames=HashMap::from([("sig".to_owned(),"a".repeat(64))]);let policy=default_relevance_policy().unwrap();let paths=creation_path_receipts("source","1",42,"block",&txs,&frames,&policy).unwrap();
        let creations=creation_receipts("source","1",42,"block",&txs,&identities,&paths,&policy).unwrap();
        assert_eq!(identities.len(),1); assert_eq!(paths.len(),1); assert_eq!(creations.len(),1);
        assert_eq!(creations[0].path_receipt_sha256,paths[0].receipt_sha256);
        assert_eq!(creations[0].identity_receipt_sha256,identities[0].receipt_sha256);
    }

    #[test]
    fn mixed_finalized_block_emits_only_relevant_receipts_and_precise_creation_dispositions() {
        let root=fixture_root("mixed-finalized-bundle"); let relevance=default_relevance_policy().unwrap();
        let unrelated=json!({"transaction":{"signatures":["unrelated"],"message":{"accountKeys":["other"],"instructions":[]}},"meta":{"err":null,"logMessages":[]},"version":0});
        let verified=creation_fixture();
        let failed=json!({"transaction":{"signatures":["failed-relevant"],"message":{"accountKeys":[PUMP],"instructions":[]}},"meta":{"err":{"InstructionError":[0,"x"]},"logMessages":[]},"version":0});
        let non_migration=json!({"transaction":{"signatures":["non-migration"],"message":{"accountKeys":[PUMP],"instructions":[]}},"meta":{"err":null,"logMessages":[]},"version":0});
        let mut invalid=creation_fixture(); invalid["transaction"]["signatures"]=json!(["invalid-migration"]); invalid["meta"]["logMessages"][3]=json!("Program data: not-a-create-event");
        let unsupported_unrelated=json!({"transaction":{"signatures":["unsupported-unrelated"],"message":{"accountKeys":["other"],"instructions":[]}},"meta":{"err":null,"logMessages":[]},"version":2});
        let txs=vec![unrelated,verified,failed,non_migration,invalid,unsupported_unrelated];
        let storage=StoragePolicy{manifest_quota_bytes:1024*1024,protected_free_bytes:0,available_free_bytes:1024*1024*1024,max_in_memory_identities:16};let mut writer=FilteredIdentityManifestWriter::open(&root,"epoch","source","1",0,0,storage).unwrap();
        for (signature,index) in [("sig",1_u64),("non-migration",3),("invalid-migration",4)] {writer.append(42,signature,index as u128,Some(index),&"a".repeat(64)).unwrap();}
        let records=records(&root,"epoch").unwrap(); let inputs=txs.iter().enumerate().map(|(index,tx)|{let mut input=relevant(tx).unwrap();input.transaction_index=index.to_string();input}).collect::<Vec<_>>();
        let reconciliation=reconcile_finalized_block_with_policy("epoch","source","1",FinalizedBlockInput{slot:"42".into(),block_identity:"block-42".into(),finalized:true,executed_transaction_count:txs.len().to_string(),transactions:inputs},&records,&relevance).unwrap();
        let closure=SourceBlockClosure{schema:String::new(),source_instance_id:"source".into(),continuity_epoch:"1".into(),relevance_policy_sha256:relevance.policy_sha256.clone(),slot:"42".into(),blockhash:"block-42".into(),executed_transaction_count:"5".into(),source_block_update_sha256:"c".repeat(64),filtered_identities:vec![SourceBlockIdentity{signature:"sig".into(),source_transaction_index:"1".into()},SourceBlockIdentity{signature:"non-migration".into(),source_transaction_index:"3".into()},SourceBlockIdentity{signature:"invalid-migration".into(),source_transaction_index:"4".into()}],closure_sha256:String::new()}.seal().unwrap();let bundle=producer_slot_bundle_with_policy("source","1",reconciliation,&closure,&txs,&HashMap::new(),&records,&relevance).unwrap();
        let actual=bundle.identities.iter().map(|receipt|(receipt.signature.clone(),receipt.transaction_index.clone())).collect::<BTreeSet<_>>();
        assert_eq!(actual,BTreeSet::from([("sig".into(),"1".into()),("non-migration".into(),"3".into()),("invalid-migration".into(),"4".into())]));
        assert_eq!(bundle.paths.iter().filter(|path|path.disposition=="QUARANTINED").count(),3);
        assert_eq!(bundle.creations.len(),2);
        assert!(bundle.creations.iter().any(|receipt|receipt.disposition=="CREATION_VERIFIED_RESTRICTED_COHORT_QUARANTINED"&&receipt.signature=="sig"));
        assert!(bundle.creations.iter().any(|receipt|receipt.disposition=="CREATION_QUARANTINED"&&receipt.signature=="invalid-migration"));
        assert!(bundle.creations.iter().all(|receipt|receipt.signature!="unrelated"&&receipt.signature!="non-migration"&&receipt.signature!="failed-relevant"));
        bundle.write_atomic(&root).unwrap(); SlotReceiptBundle::read(&root,42).unwrap();
        let _=fs::remove_dir_all(root);
    }

    #[test]
    fn persisted_producer_receipts_round_trip_into_receipt_only_cohort() {
        use pump_exact_evidence_ledger::forward_new_pool_cohort_v1::seal_forward_cohort_from_persisted_receipt_chain;
        let root=std::env::temp_dir().join(format!("canonical-receipt-roundtrip-{}",std::process::id()));let _=fs::remove_dir_all(&root);fs::create_dir_all(&root).unwrap();
        let policy=StoragePolicy{manifest_quota_bytes:1024*1024,protected_free_bytes:0,available_free_bytes:1024*1024*1024,max_in_memory_identities:8};
        let mut manifest=FilteredIdentityManifestWriter::open(&root,"canary-epoch-1","source","1",0,0,policy).unwrap();
        let identity=manifest.append(11,"sig",1,Some(0),"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();
        let migration_tx=creation_fixture();
        let mut relevant_tx=relevant(&migration_tx).unwrap();relevant_tx.transaction_index="0".into();
        let inventory=finalized_slot_inventory_from_mocked_rpc(&root,"source","1",11,13,&json!({"jsonrpc":"2.0","id":"fixture","result":[11,13]}),vec![
            (FinalizedBlockInput{slot:"11".into(),block_identity:"block-11".into(),finalized:true,executed_transaction_count:"1".into(),transactions:vec![relevant_tx]},vec![migration_tx]),
            (FinalizedBlockInput{slot:"13".into(),block_identity:"block-13".into(),finalized:true,executed_transaction_count:"0".into(),transactions:vec![]},vec![]),
        ],&[identity],&HashMap::new()).unwrap();
        assert_eq!(inventory.entries.len(),3);assert_eq!(inventory.entries[0].disposition,"PRODUCED_RECONCILED");assert_eq!(inventory.entries[1].disposition,"FINALIZED_SKIPPED");assert_eq!(inventory.entries[2].disposition,"PRODUCED_RECONCILED");mark_capture_verified(&root,11,13,&inventory);
        let accepted=seal_forward_cohort_from_persisted_receipt_chain(&root,"cohort",10).unwrap();assert_eq!(accepted.pools.len(),1);
        fs::remove_dir_all(root.join("slot-bundle-11")).unwrap();fs::remove_dir_all(root.join("slot-bundle-13")).unwrap();fs::remove_dir_all(root.join("canonical-semantic-journal")).unwrap();assert!(seal_forward_cohort_from_persisted_receipt_chain(&root,"cohort",10).is_err());
        let migration_tx=creation_fixture();let mut relevant_tx=relevant(&migration_tx).unwrap();relevant_tx.transaction_index="0".into();
        let _inventory=finalized_slot_inventory_from_mocked_rpc(&root,"source","1",11,13,&json!({"jsonrpc":"2.0","id":"fixture","result":[11,13]}),vec![
            (FinalizedBlockInput{slot:"11".into(),block_identity:"block-11".into(),finalized:true,executed_transaction_count:"1".into(),transactions:vec![relevant_tx]},vec![migration_tx]),
            (FinalizedBlockInput{slot:"13".into(),block_identity:"block-13".into(),finalized:true,executed_transaction_count:"0".into(),transactions:vec![]},vec![]),
        ],&records(&root,"canary-epoch-1").unwrap(),&HashMap::new()).unwrap();
        let path=root.join("slot-bundle-11").join("paths.json");let mut tampered:Value=serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();tampered[0]["pool"]=Value::String("tampered-pool".into());fs::write(&path,pump_exact_evidence_ledger::canonical_state_v1::jcs_bytes(&tampered).unwrap()).unwrap();assert!(seal_forward_cohort_from_persisted_receipt_chain(&root,"cohort",10).is_err());fs::remove_dir_all(root.join("slot-bundle-11")).unwrap();fs::remove_dir_all(root.join("slot-bundle-13")).unwrap();fs::remove_dir_all(root.join("canonical-semantic-journal")).unwrap();
        let migration_tx=creation_fixture();let mut relevant_tx=relevant(&migration_tx).unwrap();relevant_tx.transaction_index="0".into();
        let _inventory=finalized_slot_inventory_from_mocked_rpc(&root,"source","1",11,13,&json!({"jsonrpc":"2.0","id":"fixture","result":[11,13]}),vec![
            (FinalizedBlockInput{slot:"11".into(),block_identity:"block-11".into(),finalized:true,executed_transaction_count:"1".into(),transactions:vec![relevant_tx]},vec![migration_tx]),
            (FinalizedBlockInput{slot:"13".into(),block_identity:"block-13".into(),finalized:true,executed_transaction_count:"0".into(),transactions:vec![]},vec![]),
        ],&records(&root,"canary-epoch-1").unwrap(),&HashMap::new()).unwrap();
        let path=root.join("slot-bundle-11").join("creations.json");let mut duplicate:Value=serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();let clone=duplicate[0].clone();duplicate.as_array_mut().unwrap().push(clone);fs::write(&path,pump_exact_evidence_ledger::canonical_state_v1::jcs_bytes(&duplicate).unwrap()).unwrap();assert!(seal_forward_cohort_from_persisted_receipt_chain(&root,"cohort",10).is_err());
        let migration_tx=creation_fixture();let mut relevant_tx=relevant(&migration_tx).unwrap();relevant_tx.transaction_index="0".into();
        assert!(finalized_slot_inventory_from_mocked_rpc(&root,"source","1",11,13,&json!({"jsonrpc":"2.0","id":"fixture","result":[11,13]}),vec![(FinalizedBlockInput{slot:"11".into(),block_identity:"block-11".into(),finalized:true,executed_transaction_count:"1".into(),transactions:vec![relevant_tx]},vec![migration_tx])],&records(&root,"canary-epoch-1").unwrap(),&HashMap::new()).is_err());
        let mut budget=RpcBudget::new(16,1,1).unwrap();budget.consume_one().unwrap();assert!(diagnostic_range_preflight(&budget,1,15).is_err());assert!(diagnostic_range_preflight(&budget,1,14).is_ok());
        let _=fs::remove_dir_all(root);
    }

    #[test]
    fn canonical_bridge_recovers_existing_journal_without_duplicate_semantic_events() {
        let root=fixture_root("canonical-recovery");let migration_tx=creation_fixture();let mut relevant_tx=relevant(&migration_tx).unwrap();relevant_tx.transaction_index="0".into();
        let policy=StoragePolicy{manifest_quota_bytes:1024*1024,protected_free_bytes:0,available_free_bytes:1024*1024*1024,max_in_memory_identities:8};let mut writer=FilteredIdentityManifestWriter::open(&root,"1","source","1",0,0,policy).unwrap();writer.append(77,"sig",1,Some(0),&"a".repeat(64)).unwrap();let records=records(&root,"1").unwrap();
        let inventory=finalized_slot_inventory_from_mocked_rpc(&root,"source","1",77,77,&json!({"jsonrpc":"2.0","id":"fixture","result":[77]}),vec![(FinalizedBlockInput{slot:"77".into(),block_identity:"block-77".into(),finalized:true,executed_transaction_count:"1".into(),transactions:vec![relevant_tx]},vec![migration_tx])],&records,&HashMap::new()).unwrap();
        let before=pump_exact_evidence_ledger::canonical_state_v1::scan_journal(&root.join("canonical-semantic-journal"),"1").unwrap();
        canonicalize_committed_bundles(&root,"source","1",&inventory).unwrap();
        let after=pump_exact_evidence_ledger::canonical_state_v1::scan_journal(&root.join("canonical-semantic-journal"),"1").unwrap();
        assert_eq!(before.len(),after.len());assert_eq!(before.iter().map(|event|&event.record_sha256).collect::<Vec<_>>(),after.iter().map(|event|&event.record_sha256).collect::<Vec<_>>());let _=fs::remove_dir_all(root);
    }

    #[test]
    fn five_produced_zero_relevant_slots_still_emit_complete_receipt_chain() {
        use pump_exact_evidence_ledger::forward_new_pool_cohort_v1::seal_forward_cohort_from_persisted_receipt_chain;
        let root=std::env::temp_dir().join(format!("canonical-five-produced-{}",std::process::id()));let _=fs::remove_dir_all(&root);fs::create_dir_all(&root).unwrap();
        let slots=vec![20_u64,21,22,23,24];let blocks=slots.iter().map(|slot|(FinalizedBlockInput{slot:slot.to_string(),block_identity:format!("block-{slot}"),finalized:true,executed_transaction_count:"0".into(),transactions:vec![]},vec![])).collect();
        let inventory=finalized_slot_inventory_from_mocked_rpc(&root,"source","1",20,24,&json!({"jsonrpc":"2.0","id":"fixture","result":slots}),blocks,&[],&HashMap::new()).unwrap();
        assert_eq!(inventory.entries.iter().filter(|entry|entry.disposition=="PRODUCED_RECONCILED").count(),5);mark_capture_verified(&root,20,24,&inventory);
        assert_eq!(seal_forward_cohort_from_persisted_receipt_chain(&root,"cohort",19).unwrap().pools.len(),0);
        let _=fs::remove_dir_all(root);
    }

    #[test]
    fn crash_points_cannot_turn_staged_receipts_or_journal_output_into_verified_admission() {
        use pump_exact_evidence_ledger::forward_new_pool_cohort_v1::seal_forward_cohort_from_persisted_receipt_chain;
        let root=fixture_root("crash-points");let migration=creation_fixture();let mut input=relevant(&migration).unwrap();input.transaction_index="0".into();
        let storage=StoragePolicy{manifest_quota_bytes:1024*1024,protected_free_bytes:0,available_free_bytes:1024*1024*1024,max_in_memory_identities:8};let mut writer=FilteredIdentityManifestWriter::open(&root,"1","source","1",0,0,storage).unwrap();writer.append(11,"sig",1,Some(0),&"a".repeat(64)).unwrap();let records=records(&root,"1").unwrap();
        let inventory=finalized_slot_inventory_from_mocked_rpc(&root,"source","1",11,13,&json!({"jsonrpc":"2.0","id":"fixture","result":[11,13]}),vec![(FinalizedBlockInput{slot:"11".into(),block_identity:"block-11".into(),finalized:true,executed_transaction_count:"1".into(),transactions:vec![input]},vec![migration]),(FinalizedBlockInput{slot:"13".into(),block_identity:"block-13".into(),finalized:true,executed_transaction_count:"0".into(),transactions:vec![]},vec![])],&records,&HashMap::new()).unwrap();
        // Simulated crash after the atomic slot-bundle rename but before the
        // inventory rename: no final inventory means no consumer admission.
        let inventory_path=root.join("finalized-slot-inventory.json");let parked=root.join(".crash-after-slot-bundle");fs::rename(&inventory_path,&parked).unwrap();assert!(seal_forward_cohort_from_persisted_receipt_chain(&root,"cohort",10).is_err());fs::rename(&parked,&inventory_path).unwrap();
        // The inventory and canonical bridge may both be durable, but a crash
        // before the terminal checkpoint is still non-admissible.
        let policy=default_relevance_policy().unwrap();let mut run=CaptureRunJournal::new(&root,"fixture","source","1",&policy,ResourceAccounting::new(16,6*1024*1024,8*1024*1024,32).unwrap()).unwrap();for slot in 11..=13{run.observe_finality(SourceFinalityRecord{schema:String::new(),source_instance_id:"source".into(),continuity_epoch:"1".into(),slot:slot.to_string(),receive_sequence:slot.to_string(),source_frame_sha256:"a".repeat(64),record_sha256:String::new()}.seal().unwrap()).unwrap();}run.select_range(3).unwrap();run.record_inventory(&inventory.inventory_sha256,vec![],vec![RawRpcEvidence::new("getBlocks",None,"synthetic-finalized-getblocks-response.json",b"fixture").unwrap()]).unwrap();run.stage_receipts().unwrap();assert!(root.join("canonical-semantic-journal").exists());assert!(seal_forward_cohort_from_persisted_receipt_chain(&root,"cohort",10).is_err());
        // A process crash at this point is permanently voided and can never
        // be silently resumed into a verified run.
        drop(run);CaptureRunJournal::void_existing(&root,"CRASH_AFTER_JOURNAL_APPEND").unwrap();assert_eq!(CaptureRunJournal::load(&root).unwrap().checkpoint.lifecycle,CaptureLifecycle::Voided);assert!(seal_forward_cohort_from_persisted_receipt_chain(&root,"cohort",10).is_err());
        let _=fs::remove_dir_all(root);
    }

    #[test]
    fn range_first_production_helpers_reserve_inventory_before_any_dispatcher_work() {
        use pump_exact_evidence_ledger::{canonical_capture_run_v1::{CanonicalRelevancePolicy, CaptureRunJournal, ResourceAccounting, SourceFinalityRecord}, forward_new_pool_cohort_v1::seal_forward_cohort_from_persisted_receipt_chain};
        let root=fixture_root("range-first-production");let policy=CanonicalRelevancePolicy::new("PUMPSWAP_FINALIZED_RELEVANCE_V1",programs().into_iter().collect(),1).unwrap();let mut run=CaptureRunJournal::new(&root,"fixture","source","1",&policy,ResourceAccounting::new(16,6*1024*1024,8*1024*1024,32).unwrap()).unwrap();
        for slot in 300..305 {run.observe_finality(SourceFinalityRecord{schema:String::new(),source_instance_id:"source".into(),continuity_epoch:"1".into(),slot:slot.to_string(),receive_sequence:slot.to_string(),source_frame_sha256:"a".repeat(64),record_sha256:String::new()}.seal().unwrap()).unwrap();}
        assert_eq!(run.select_range(5).unwrap(),(300,304));
        // Exactly getBlocks + five produced getBlock requests: no dispatcher
        // request is permitted before this inventory has been reserved.
        for _ in 0..6 {run.checkpoint.resources.consume_rpc().unwrap();}run.persist().unwrap();
        let slots=vec![300_u64,301,302,303,304];let blocks=slots.iter().map(|slot|(FinalizedBlockInput{slot:slot.to_string(),block_identity:format!("block-{slot}"),finalized:true,executed_transaction_count:"0".into(),transactions:vec![]},vec![])).collect();
        let inventory=finalized_slot_inventory_from_mocked_rpc(&root,"source","1",300,304,&json!({"jsonrpc":"2.0","id":"fixture","result":slots}),blocks,&[],&HashMap::new()).unwrap();
        run.record_inventory(&inventory.inventory_sha256,vec![],vec![RawRpcEvidence::new("getBlocks",None,"synthetic-finalized-getblocks-response.json",b"fixture").unwrap()]).unwrap();assert!(run.source_match_complete(&BTreeSet::new()).unwrap());run.stage_receipts().unwrap();run.commit_receipts().unwrap();
        assert_eq!(run.checkpoint.resources.rpc_used,"6");assert!(root.join("canonical-production-bridge-receipt.json").exists());assert_eq!(seal_forward_cohort_from_persisted_receipt_chain(&root,"cohort",299).unwrap().pools.len(),0);let _=fs::remove_dir_all(root);
    }

    #[test]
    fn scripted_yellowstone_to_producer_to_persisted_cohort_validator_round_trip() {
        use pump_exact_evidence_ledger::forward_new_pool_cohort_v1::seal_forward_cohort_from_persisted_receipt_chain;
        let root=fixture_root("scripted-production-entry");let policy=default_relevance_policy().unwrap();let mut run=CaptureRunJournal::new(&root,"fixture","source","1",&policy,ResourceAccounting::new(16,6*1024*1024,8*1024*1024,32).unwrap()).unwrap();let storage=StoragePolicy{manifest_quota_bytes:1024*1024,protected_free_bytes:0,available_free_bytes:1024*1024*1024,max_in_memory_identities:16};let mut writer=FilteredIdentityManifestWriter::open(&root,"1","source","1",0,0,storage).unwrap();let mut dispatcher=Dispatcher::new(32,3,1).unwrap();let mut finalized=BTreeSet::new();let mut source_records=Vec::new();let mut events=HashMap::new();let mut report=Report{schema:"test".into(),status:"RUNNING".into(),started_at_utc:String::new(),finished_at_utc:String::new(),slots_observed:0,slots_reconciled:0,slots_rejected:0,exact_passes:0,failures:0,rpc_calls:0,dispatcher_rpc_calls:0,dispatcher_retry_rpc_calls:0,inventory_get_blocks_rpc_calls:0,inventory_get_block_rpc_calls:0,retries:0,pending_slot_peak:0,max_rss_bytes:0,max_message_bytes:0,disk_growth_bytes:0,decoder_transactions:0,decoder_events:0,decoder_rejections:0,source_gap_or_epoch_failure:false,unresolved:"NONE".into(),reconciled_slot_produced:false,scientific_interval_produced:false,failure:None};let mut finality_sequence=0;
        let mut closures=BTreeMap::new();
        for slot in 400..403 {apply_source_observation(SourceObservation::FinalizedSlot{slot,source_frame_sha256:"a".repeat(64)},"source",&mut run,&mut writer,&mut dispatcher,&mut finalized,&mut closures,&mut source_records,&mut events,&mut report,&mut finality_sequence,false).unwrap();}
        apply_source_observation(SourceObservation::Transaction{slot:400,signature:"sig".into(),source_transaction_index:0,received_at_unix_ns:1,source_frame_sha256:"b".repeat(64),semantic_events:vec![],decoder_rejected:false},"source",&mut run,&mut writer,&mut dispatcher,&mut finalized,&mut closures,&mut source_records,&mut events,&mut report,&mut finality_sequence,false).unwrap();
        assert_eq!(run.select_range(3).unwrap(),(400,402));let migration=creation_fixture();let mut relevant_tx=relevant(&migration).unwrap();relevant_tx.transaction_index="0".into();let fixture_blocks=vec![(FinalizedBlockInput{slot:"400".into(),block_identity:"block-400".into(),finalized:true,executed_transaction_count:"1".into(),transactions:vec![relevant_tx]},vec![migration]),(FinalizedBlockInput{slot:"402".into(),block_identity:"block-402".into(),finalized:true,executed_transaction_count:"0".into(),transactions:vec![]},vec![])];let closure_map=fixture_blocks.iter().map(|(block,_)|{let identities=expected_identities_for_block(block,&policy).unwrap().into_iter().map(|identity|SourceBlockIdentity{signature:identity.signature,source_transaction_index:"0".into()}).collect();let slot=block.slot.parse().unwrap();(slot,SourceBlockClosure{schema:String::new(),source_instance_id:"source".into(),continuity_epoch:"1".into(),relevance_policy_sha256:policy.policy_sha256.clone(),slot:block.slot.clone(),blockhash:block.block_identity.clone(),executed_transaction_count:block.executed_transaction_count.clone(),source_block_update_sha256:"c".repeat(64),filtered_identities:identities,closure_sha256:String::new()}.seal().unwrap())}).collect::<BTreeMap<_,_>>();for closure in closure_map.values(){closure.write_atomic(&root).unwrap();}let inventory=finalized_slot_inventory_from_mocked_rpc_with_policy(&root,"source","1",400,402,&json!({"jsonrpc":"2.0","id":"fixture","result":[400,402]}),fixture_blocks,&closure_map,&source_records,&events,&policy).unwrap();run.record_inventory(&inventory.inventory_sha256,vec![ExpectedIdentity{slot:"400".into(),signature:"sig".into(),transaction_index:"0".into()}],vec![RawRpcEvidence::new("getBlocks",None,"synthetic-finalized-getblocks-response.json",b"fixture").unwrap()]).unwrap();let observed=source_records.iter().map(|record|ExpectedIdentity{slot:record.slot.clone(),signature:record.signature.clone(),transaction_index:record.source_transaction_index.clone().unwrap()}).collect();assert!(run.source_match_complete(&observed).unwrap());run.stage_receipts().unwrap();run.commit_receipts().unwrap();assert_eq!(seal_forward_cohort_from_persisted_receipt_chain(&root,"cohort",399).unwrap().pools.len(),1);assert_eq!(report.decoder_transactions,1);let _=fs::remove_dir_all(root);
    }

    #[test]
    fn atomic_bundle_manifest_rejects_expected_emitted_set_digest_tampering() {
        let root=fixture_root("bundle-digest");let blocks=vec![(FinalizedBlockInput{slot:"9".into(),block_identity:"block-9".into(),finalized:true,executed_transaction_count:"0".into(),transactions:vec![]},vec![])];
        let _=finalized_slot_inventory_from_mocked_rpc(&root,"source","1",9,9,&json!({"jsonrpc":"2.0","id":"fixture","result":[9]}),blocks,&[],&HashMap::new()).unwrap();let path=root.join("slot-bundle-9").join("bundle-manifest.json");let mut manifest:Value=serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();manifest["emittedIdentitySetSha256"]=Value::String("f".repeat(64));fs::write(&path,pump_exact_evidence_ledger::canonical_state_v1::jcs_bytes(&manifest).unwrap()).unwrap();assert!(SlotReceiptBundle::read(&root,9).is_err());let _=fs::remove_dir_all(root);
    }
}

/* Legacy eager-loop implementation retained below only as audit history. It is
 * intentionally not compiled or invoked by the integrated producer. The
 * dispatcher path above is the only executable canary path. 
 rustls::crypto::ring::default_provider().install_default().map_err(|_|anyhow::anyhow!("RUSTLS"))?; let root=PathBuf::from(env::var("FINALIZED_RECONCILIATION_CANARY_ROOT")?); let endpoint=env::var("YELLOWSTONE_FULL_META_ENDPOINT")?; let token:AsciiMetadataValue=env::var("SHYFT_GRPC_X_TOKEN")?.parse()?; let rpc=env::var("SECURE_RPC_URL")?; fs::create_dir_all(&root)?; if free(&root)?<=PROTECTED_RESERVE+MANIFEST_QUOTA {bail!("PROTECTED_RESERVE_PREFLIGHT")}; let start_free=free(&root)?; let started=std::time::Instant::now(); let source=format!("fallback-canary-{}-{}",Utc::now().format("%Y%m%dT%H%M%SZ"),std::process::id()); let epoch="canary-epoch-1"; let mut report=Report{schema:"CANONICAL_FINALIZED_RPC_RECONCILIATION_CANARY_V1".into(),status:"RUNNING".into(),started_at_utc:Utc::now().to_rfc3339(),finished_at_utc:String::new(),slots_observed:0,slots_reconciled:0,slots_rejected:0,exact_passes:0,failures:0,rpc_calls:0,retries:0,pending_slot_peak:0,max_rss_bytes:0,max_message_bytes:0,disk_growth_bytes:0,decoder_transactions:0,decoder_events:0,decoder_rejections:0,source_gap_or_epoch_failure:false,unresolved:"NONE".into(),reconciled_slot_produced:false,scientific_interval_produced:false,failure:None};
 let result:Result<()> = async { let policy=StoragePolicy{manifest_quota_bytes:MANIFEST_QUOTA,protected_free_bytes:PROTECTED_RESERVE,available_free_bytes:free(&root)?,max_in_memory_identities:16384}; let mut manifest=FilteredIdentityManifestWriter::open(&root,epoch,&source,"1",0,0,policy)?; let channel=Channel::from_shared(endpoint)?.tls_config(ClientTlsConfig::new().with_native_roots())?.connect_timeout(Duration::from_secs(10)).connect().await?; let t=token.clone(); let mut client=GeyserClient::with_interceptor(channel,move|mut r:Request<()>|{r.metadata_mut().insert("x-token",t.clone());Ok::<_,Status>(r)}).max_decoding_message_size(MAX_MESSAGE); let mut updates=client.subscribe(Request::new(stream::once(async{req()}).chain(stream::pending()))).await?.into_inner(); let http=reqwest::Client::builder().timeout(Duration::from_secs(20)).build()?; let mut budget=RpcBudget::new(MAX_RPC,1,1000)?; let mut pending=BTreeSet::new(); let mut complete=BTreeSet::new(); while started.elapsed()<Duration::from_secs(MAX_SECONDS)&&complete.len()<MAX_SLOTS { let update=tokio::time::timeout(Duration::from_secs(20),updates.next()).await.context("GRPC_TIMEOUT")?.context("GRPC_ENDED")??; let size=update.encoded_len(); report.max_message_bytes=report.max_message_bytes.max(size); report.max_rss_bytes=report.max_rss_bytes.max(rss()); if size>MAX_MESSAGE{bail!("MESSAGE_CAP")} if report.max_rss_bytes>MAX_RSS{bail!("RSS_CAP")} if free(&root)?<=PROTECTED_RESERVE{bail!("PROTECTED_RESERVE")} if let Some(UpdateOneof::Transaction(item))=update.update_oneof { let info=item.transaction.clone().context("TRANSACTION_INFO")?; let signature=bs58::encode(&info.signature).into_string(); let raw_hash=pump_exact_evidence_ledger::sha256_hex(&item.encode_to_vec()); manifest.append(item.slot,&signature,Utc::now().timestamp_nanos_opt().unwrap_or_default() as u128,Some(info.index),&raw_hash)?; report.decoder_transactions+=1; if let Some(meta)=info.meta.as_ref(){match decode_events(meta){Ok(events)=>report.decoder_events+=events.len() as u64,Err(_)=>report.decoder_rejections+=1}} pending.insert(item.slot); if pending.len()>32{bail!("RPC_BACKLOG_CAP")}; }
   for slot in pending.clone(){ if complete.contains(&slot)||budget.used_requests()>=MAX_RPC{continue} budget.consume_one()?; report.rpc_calls+=1; match block(&http,&rpc,slot).await { Ok(proof)=>{let receipt=reconcile_finalized_block(epoch,&source,"1",proof, &records(&root,epoch)?,&programs())?; if receipt.disposition=="MATCHED"{complete.insert(slot);report.exact_passes+=1;report.slots_reconciled+=1;}else{report.failures+=1;report.unresolved=receipt.reason_code.unwrap_or_else(||"RECONCILIATION_REJECTED".into());bail!("RECONCILIATION_REJECTED")} }, Err(_)=>{} } }
 } if complete.is_empty(){bail!("NO_FINALIZED_RECONCILIATION_PASS")} report.slots_observed=pending.len(); report.scientific_slot_produced=true; Ok(()) }.await;
 report.finished_at_utc=Utc::now().to_rfc3339(); report.max_rss_bytes=report.max_rss_bytes.max(rss()); report.disk_growth_bytes=start_free.saturating_sub(free(&root)?); match &result {Ok(_)=>report.status="FALLBACK_CANARY_VERIFIED".into(),Err(e)=>{report.status="FALLBACK_CANARY_FAILED_OR_BLOCKED".into();report.failure=Some(e.to_string())}}; fs::write(root.join("report.json"),serde_json::to_vec_pretty(&report)?)?; result }
*/
