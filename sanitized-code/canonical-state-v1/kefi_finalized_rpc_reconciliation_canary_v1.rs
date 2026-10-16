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
use yellowstone_grpc_proto::geyser::{geyser_client::GeyserClient, subscribe_update::UpdateOneof, CommitmentLevel, SlotStatus, SubscribeRequest, SubscribeRequestFilterSlots, SubscribeRequestFilterTransactions};
use pump_exact_evidence_ledger::{filtered_identity_manifest_v1::{reconcile_finalized_block, FilteredIdentity, FilteredIdentityManifestWriter, FinalizedBlockInput, FinalizedTransactionInput, ReconciliationReceipt, RpcBudget, StoragePolicy}, finalized_slot_inventory_v1::{FinalizedSlotInventory, ReconciledProducedSlot}, forward_cohort_receipts_v1::{CreationReceipt, IdentityReceipt as FinalizedIdentityReceipt, PathReceipt, SlotReceiptBundle}, pump_instruction_path_decoder_v1::{decode_instruction_paths, CompiledInstructionInput, FullTransactionInput, InnerInstructionGroupInput, InnerInstructionInput, SemanticEventInput}};
use pumpfun_sidecar::kefi_pumpswap_semantics_v1::{decode_events, Event, BUY_DISC, BUY_EXACT_DISC, SELL_DISC, PUMPSWAP_PROGRAM};
use pumpfun_sidecar::pumpswap_creation_witness_v1::{validate_finalized_pump_migration, CREATE_POOL};

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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PendingRangeReadiness {
    AwaitingSourceFinalizedStatus,
    AwaitingSourceIdentities,
    ReadyToSeal,
}

#[derive(Serialize)] #[serde(rename_all="camelCase")]
struct Report { schema:String, status:String, started_at_utc:String, finished_at_utc:String, slots_observed:usize, slots_reconciled:usize, slots_rejected:usize, exact_passes:usize, failures:usize, rpc_calls:u64, dispatcher_rpc_calls:u64, dispatcher_retry_rpc_calls:u64, inventory_get_blocks_rpc_calls:u64, inventory_get_block_rpc_calls:u64, retries:u64, pending_slot_peak:usize, max_rss_bytes:u64, max_message_bytes:usize, disk_growth_bytes:u64, decoder_transactions:u64, decoder_events:u64, decoder_rejections:u64, source_gap_or_epoch_failure:bool, unresolved:String, reconciled_slot_produced:bool, scientific_interval_produced:bool, failure:Option<String> }

fn free(path:&PathBuf)->Result<u64>{ let out=Command::new("df").args(["-B1","--output=avail",path.to_str().context("PATH")?]).output()?; Ok(String::from_utf8(out.stdout)?.lines().nth(1).context("DF")?.trim().parse()?) }
fn rss()->u64 { fs::read_to_string("/proc/self/status").ok().and_then(|s|s.lines().find_map(|l|l.strip_prefix("VmHWM:")?.split_whitespace().next()?.parse::<u64>().ok())).unwrap_or(0)*1024 }
fn validate_storage_policy(start_free:u64,reserve:u64,start_floor:u64,total_cap:u64)->Result<()> { if total_cap != 6_u64*1024*1024*1024 || start_floor < reserve.saturating_add(total_cap).saturating_add(OPERATIONAL_BUFFER) { bail!("STORAGE_POLICY_INVALID") } if start_free<start_floor {bail!("PROTECTED_STARTUP_FLOOR")}Ok(()) }
fn ensure_fresh_output_root(root:&PathBuf)->Result<()> { if root.join("run-binding.json").exists(){bail!("DURABLE_PRIOR_RUN_REQUIRES_NEW_ROOT")}Ok(()) }
fn diagnostic_rpc_reservation(budget:&RpcBudget,start:u64,end:u64)->Result<DiagnosticRpcReservation>{let range_slots=end.checked_sub(start).and_then(|value|value.checked_add(1)).context("FINALIZED_RANGE_INVALID")?;if range_slots==0||range_slots>15{bail!("FINALIZED_RANGE_DIAGNOSTIC_CAP")}let inventory_requests=range_slots.checked_add(1).context("FINALIZED_RANGE_BUDGET_OVERFLOW")?;if inventory_requests>budget.maximum_requests{bail!("FINALIZED_RANGE_RPC_BUDGET_INSUFFICIENT")}let dispatcher_limit=budget.maximum_requests-inventory_requests;if budget.used_requests()>dispatcher_limit{bail!("FINALIZED_RANGE_RESERVATION_ALREADY_STARVED")}Ok(DiagnosticRpcReservation{range_slots,inventory_requests,dispatcher_limit})}
fn consume_dispatcher_rpc(budget:&mut RpcBudget,reservation:Option<DiagnosticRpcReservation>)->Result<()> {if let Some(reservation)=reservation {if budget.used_requests()>=reservation.dispatcher_limit{bail!("DISPATCHER_RPC_RESERVATION_EXHAUSTED")}}budget.consume_one()}
fn parse_diagnostic_range_slots(value:Option<&str>)->Result<Option<u64>> { let Some(value)=value else{return Ok(None)};let slots:u64=value.parse().context("FINALIZED_RANGE_SLOT_COUNT")?;if !(3..=5).contains(&slots){bail!("FINALIZED_RANGE_SLOT_COUNT_OUT_OF_BOUNDS")}Ok(Some(slots)) }
fn finalized_range_ready(start:u64,slots:u64,highest_finalized_slot:Option<u64>)->Result<bool>{let end=start.checked_add(slots.checked_sub(1).context("FINALIZED_RANGE_INVALID")?).context("FINALIZED_RANGE_OVERFLOW")?;Ok(highest_finalized_slot.is_some_and(|highest|highest>=end))}
// Both diagnostic modes start collecting. Range mode completes only after its
// inventory seals; no-range mode completes after one exact dispatcher match.
fn initial_range_inventory_complete(_:Option<u64>)->bool { false }
fn req()->SubscribeRequest { let mut transactions=HashMap::new(); let mut slots=HashMap::new(); transactions.insert("canonicalRelevant".into(), SubscribeRequestFilterTransactions { vote:Some(false), failed:Some(false), signature:None, account_include:vec![PUMP.into(),ROUTER.into(),PUMPSWAP_PROGRAM.into()], account_exclude:vec![], account_required:vec![], token_accounts:None }); slots.insert("finalizedProgress".into(), SubscribeRequestFilterSlots { filter_by_commitment:Some(true), interslot_updates:Some(false) }); SubscribeRequest { accounts:HashMap::new(), slots, transactions, transactions_status:HashMap::new(), blocks:HashMap::new(), blocks_meta:HashMap::new(), entry:HashMap::new(), accounts_data_slice:vec![], commitment:Some(CommitmentLevel::Finalized as i32), ping:None, from_slot:None } }
fn programs()->BTreeSet<String>{ [PUMP.to_owned(),ROUTER.to_owned(),PUMPSWAP_PROGRAM.to_owned()].into_iter().collect() }
fn relevant(tx:&Value)->Result<FinalizedTransactionInput>{ let signature=tx.pointer("/transaction/signatures/0").and_then(Value::as_str).context("RPC_SIGNATURE")?.to_owned(); let mut ids=BTreeSet::new(); for key in tx.pointer("/transaction/message/accountKeys").and_then(Value::as_array).context("RPC_KEYS")? { if let Some(s)=key.as_str(){ids.insert(s.to_owned());} else if let Some(s)=key.get("pubkey").and_then(Value::as_str){ids.insert(s.to_owned());} else {bail!("RPC_KEY_LAYOUT")} } for p in ["/meta/loadedAddresses/writable","/meta/loadedAddresses/readonly"] { if let Some(keys)=tx.pointer(p).and_then(Value::as_array){ for key in keys { ids.insert(key.as_str().context("RPC_LOADED_KEY")?.to_owned()); } } } let failed = tx.pointer("/meta/err").map(|v| !v.is_null()).unwrap_or(true); if failed { ids.clear(); } let supported_version = match tx.get("version") { None => true, Some(value) if value.as_str()==Some("legacy") => true, Some(value) => value.as_u64().is_some_and(|version| version <= 1) }; Ok(FinalizedTransactionInput { transaction_index:String::new(), signature, account_or_program_ids:ids.into_iter().collect(), supported_version }) }

#[derive(Clone)]
struct RawSemanticEvent { event_id: String, log_index: usize, event: Event }


fn json_keys(tx:&Value, path:&str)->Result<Vec<String>> { tx.pointer(path).and_then(Value::as_array).context("RPC_ACCOUNT_KEYS")?.iter().map(|key| key.as_str().or_else(||key.get("pubkey").and_then(Value::as_str)).map(str::to_owned).context("RPC_ACCOUNT_KEY_LAYOUT")).collect() }
fn json_instruction(value:&Value)->Result<CompiledInstructionInput> { Ok(CompiledInstructionInput { program_id_index:value.get("programIdIndex").and_then(Value::as_u64).context("RPC_PROGRAM_INDEX")? as usize, account_indexes:value.get("accounts").and_then(Value::as_array).context("RPC_ACCOUNTS")?.iter().map(|value|value.as_u64().map(|n|n as usize).context("RPC_ACCOUNT_INDEX")).collect::<Result<Vec<_>>>()?, data:bs58::decode(value.get("data").and_then(Value::as_str).context("RPC_INSTRUCTION_DATA")?).into_vec().context("RPC_INSTRUCTION_DATA_BASE58")? }) }
fn finalized_input(slot:u64, index:usize, tx:&Value)->Result<FullTransactionInput> { let signature=tx.pointer("/transaction/signatures/0").and_then(Value::as_str).context("RPC_SIGNATURE")?.to_owned(); let outer=tx.pointer("/transaction/message/instructions").and_then(Value::as_array).context("RPC_OUTER_INSTRUCTIONS")?.iter().map(json_instruction).collect::<Result<Vec<_>>>()?; let mut inner=Vec::new(); if let Some(groups)=tx.pointer("/meta/innerInstructions").and_then(Value::as_array) { for group in groups { let parent=group.get("index").and_then(Value::as_u64).context("RPC_INNER_PARENT_INDEX")? as usize; let instructions=group.get("instructions").and_then(Value::as_array).context("RPC_INNER_INSTRUCTIONS")?.iter().map(|value| Ok(InnerInstructionInput { program_id_index:value.get("programIdIndex").and_then(Value::as_u64).context("RPC_INNER_PROGRAM_INDEX")? as usize, account_indexes:value.get("accounts").and_then(Value::as_array).context("RPC_INNER_ACCOUNTS")?.iter().map(|value|value.as_u64().map(|n|n as usize).context("RPC_INNER_ACCOUNT_INDEX")).collect::<Result<Vec<_>>>()?, data:bs58::decode(value.get("data").and_then(Value::as_str).context("RPC_INNER_DATA")?).into_vec().context("RPC_INNER_DATA_BASE58")?, stack_height:value.get("stackHeight").and_then(Value::as_u64).map(|n|n as usize) })).collect::<Result<Vec<_>>>()?; inner.push(InnerInstructionGroupInput { parent_outer_index:parent, instructions }); } }
    Ok(FullTransactionInput { slot, finalized_transaction_index:Some(index as u64), signature, succeeded:tx.pointer("/meta/err").map(Value::is_null).unwrap_or(false), static_account_keys:json_keys(tx,"/transaction/message/accountKeys")?, loaded_writable_account_keys:tx.pointer("/meta/loadedAddresses/writable").map(|_|json_keys(tx,"/meta/loadedAddresses/writable")).transpose()?.unwrap_or_default(), loaded_readonly_account_keys:tx.pointer("/meta/loadedAddresses/readonly").map(|_|json_keys(tx,"/meta/loadedAddresses/readonly")).transpose()?.unwrap_or_default(), outer_instructions:outer, inner_instruction_groups:inner, log_messages:tx.pointer("/meta/logMessages").and_then(Value::as_array).context("RPC_LOG_MESSAGES")?.iter().map(|value|value.as_str().map(str::to_owned).context("RPC_LOG_ENTRY")).collect::<Result<Vec<_>>>()? }) }
fn path_receipt(source:&str, epoch:&str, slot:u64, blockhash:&str, transaction_index:usize, event_id:String, semantic_kind:String, pool:String, signature:String, decoded:Option<pump_exact_evidence_ledger::pump_instruction_path_decoder_v1::DecodedSemanticEvent>, reason:Option<String>)->Result<PathReceipt> {
    let (instruction_path, canonical_order, disposition, reason_code) = match decoded { Some(event)=>{let order=&event.canonical_order;(order.instruction_path.render(),json!({"slot":order.slot.to_string(),"finalizedTransactionIndex":order.finalized_transaction_index.to_string(),"outerInstructionIndex":order.instruction_path.outer_index.to_string(),"innerInstructionIndex":order.instruction_path.inner_index.map(|v|v.to_string()),"stackHeight":order.instruction_path.stack_height.map(|v|v.to_string()),"semanticOrdinal":order.semantic_ordinal.to_string()}),"PATH_RESOLVED_QUARANTINED_UNIVERSE_INCOMPLETE".into(),Some("CANDIDATE_UNIVERSE_INCOMPLETE".into()))},None=>(String::new(),Value::Null,"QUARANTINED".into(),reason.or(Some("PATH_UNRESOLVED".into()))) };
    PathReceipt {schema:"CANONICAL_FINALIZED_INSTRUCTION_PATH_RECEIPT_V2".into(),source_instance_id:source.into(),continuity_epoch:epoch.into(),slot:slot.to_string(),finalized_blockhash:blockhash.into(),signature,transaction_index:transaction_index.to_string(),event_id,instruction_path,semantic_kind,pool,canonical_order,disposition,reason_code,receipt_sha256:String::new()}.seal()
}
fn path_receipts(source:&str, epoch:&str, slot:u64, blockhash:&str, txs:&[Value], events:&HashMap<String,Vec<RawSemanticEvent>>)->Result<Vec<PathReceipt>> { let mut out=Vec::new(); for (index,tx) in txs.iter().enumerate() { let signature=tx.pointer("/transaction/signatures/0").and_then(Value::as_str).context("RPC_SIGNATURE")?; let Some(raw)=events.get(signature) else {continue}; let input=finalized_input(slot,index,tx); let semantic=raw.iter().map(|raw| SemanticEventInput { event_id:raw.event_id.clone(), expected_program_id:PUMPSWAP_PROGRAM.into(), pool:raw.event.pool.clone(), log_index:raw.log_index, accepted_instruction_discriminators:if raw.event.side=="BUY" {vec![BUY_DISC.to_vec(),BUY_EXACT_DISC.to_vec()]} else {vec![SELL_DISC.to_vec()]}, semantic_kind:format!("PUMPSWAP_{}",raw.event.side) }).collect::<Vec<_>>(); match input.and_then(|input|decode_instruction_paths(&input,&semantic)) { Ok(decoded) => for event in decoded { let signature=event.signature.clone(); let kind=event.semantic_kind.clone(); let pool=event.pool.clone(); let event_id=event.event_id.clone();out.push(path_receipt(source,epoch,slot,blockhash,index,event_id,kind,pool,signature,Some(event),None)?); }, Err(error)=> for raw in raw { out.push(path_receipt(source,epoch,slot,blockhash,index,raw.event_id.clone(),format!("PUMPSWAP_{}",raw.event.side),raw.event.pool.clone(),signature.into(),None,Some(error.to_string()))?); }
    }} Ok(out) }
fn creation_path_receipts(source:&str, epoch:&str, slot:u64, blockhash:&str, txs:&[Value])->Result<Vec<PathReceipt>> { let mut out=Vec::new(); for (index,tx) in txs.iter().enumerate() { for witness in validate_finalized_pump_migration(slot,index as u64,tx)? { let log_index=witness.event_id.rsplit(':').next().context("CREATION_EVENT_ID")?.parse()?; let semantic=vec![SemanticEventInput{event_id:witness.event_id.clone(),expected_program_id:PUMPSWAP_PROGRAM.into(),pool:witness.pool.clone(),log_index,accepted_instruction_discriminators:vec![CREATE_POOL.to_vec()],semantic_kind:"PUMP_MIGRATE_CREATE_POOL".into()}]; match finalized_input(slot,index,tx).and_then(|input|decode_instruction_paths(&input,&semantic)) { Ok(mut decoded)=>{ if decoded.len()!=1 || decoded[0].canonical_order.instruction_path.render()!=witness.instruction_path { bail!("CREATION_PATH_WITNESS_MISMATCH") }; let event=decoded.remove(0);out.push(path_receipt(source,epoch,slot,blockhash,index,event.event_id.clone(),event.semantic_kind.clone(),event.pool.clone(),event.signature.clone(),Some(event),None)?);},Err(error)=>out.push(path_receipt(source,epoch,slot,blockhash,index,witness.event_id,"PUMP_MIGRATE_CREATE_POOL".into(),witness.pool,witness.signature,None,Some(error.to_string()))?) } } } Ok(out) }
fn identity_receipts(source:&str,epoch:&str,slot:u64,blockhash:&str,block_reconciliation_receipt_sha256:&str,txs:&[Value])->Result<Vec<FinalizedIdentityReceipt>> { let mut out=Vec::new();for(index,tx) in txs.iter().enumerate(){let relevant=relevant(tx)?;if !relevant.supported_version{bail!("UNSUPPORTED_TRANSACTION_VERSION")};if relevant.account_or_program_ids.is_empty(){continue}out.push(FinalizedIdentityReceipt{schema:"EDGESNIFFER_FINALIZED_RECONCILIATION_SLOT_RECEIPT_V1".into(),source_instance_id:source.into(),continuity_epoch:epoch.into(),slot:slot.to_string(),signature:relevant.signature,transaction_index:index.to_string(),block_identity:blockhash.into(),block_reconciliation_receipt_sha256:block_reconciliation_receipt_sha256.into(),disposition:"MATCHED".into(),reason_code:None,receipt_sha256:String::new()}.seal()?)}Ok(out) }
fn creation_receipts(source:&str,epoch:&str,slot:u64, blockhash:&str, txs:&[Value], identities:&[FinalizedIdentityReceipt], paths:&[PathReceipt])->Result<Vec<CreationReceipt>> { let mut out=Vec::new(); for (index,tx) in txs.iter().enumerate() { match validate_finalized_pump_migration(slot,index as u64,tx) { Ok(witnesses)=>for witness in witnesses { let identity=identities.iter().find(|item|item.signature==witness.signature&&item.transaction_index==witness.transaction_index).context("CREATION_IDENTITY_RECEIPT_MISSING")?;let path=paths.iter().find(|item|item.event_id==witness.event_id&&item.disposition=="PATH_RESOLVED_QUARANTINED_UNIVERSE_INCOMPLETE").context("CREATION_PATH_RECEIPT_MISSING")?;if path.instruction_path!=witness.instruction_path {bail!("CREATION_PATH_RECEIPT_MISMATCH")}; out.push(CreationReceipt{schema:"CANONICAL_PUMP_MIGRATE_CREATION_RECEIPT_V2".into(),source_instance_id:source.into(),continuity_epoch:epoch.into(),slot:slot.to_string(),finalized_blockhash:blockhash.into(),signature:witness.signature.clone(),transaction_index:witness.transaction_index.clone(),pool:witness.pool.clone(),mint:witness.mint.clone(),event_id:witness.event_id.clone(),instruction_path:witness.instruction_path.clone(),identity_receipt_sha256:identity.receipt_sha256.clone(),path_receipt_sha256:path.receipt_sha256.clone(),witness:serde_json::to_value(witness)?,disposition:"CREATION_VERIFIED_RESTRICTED_COHORT_QUARANTINED".into(),reason_code:Some("FORWARD_COHORT_NOT_OPENED".into()),receipt_sha256:String::new()}.seal()?);}, Err(error)=>{let signature=tx.pointer("/transaction/signatures/0").and_then(Value::as_str).unwrap_or("UNKNOWN");out.push(CreationReceipt{schema:"CANONICAL_PUMP_MIGRATE_CREATION_RECEIPT_V2".into(),source_instance_id:source.into(),continuity_epoch:epoch.into(),slot:slot.to_string(),finalized_blockhash:blockhash.into(),signature:signature.into(),transaction_index:index.to_string(),pool:String::new(),mint:String::new(),event_id:String::new(),instruction_path:String::new(),identity_receipt_sha256:String::new(),path_receipt_sha256:String::new(),witness:json!({"signature":signature}),disposition:"CREATION_QUARANTINED".into(),reason_code:Some(error.to_string()),receipt_sha256:String::new()}.seal()?);} } } Ok(out) }
async fn block(http:&reqwest::Client,url:&str,slot:u64)->Result<(FinalizedBlockInput,Vec<Value>)>{ let response:Value=http.post(url).json(&json!({"jsonrpc":"2.0","id":slot,"method":"getBlock","params":[slot,{"commitment":"finalized","encoding":"json","transactionDetails":"full","rewards":false,"maxSupportedTransactionVersion":1}]})).send().await?.error_for_status()?.json().await?; if response.get("error").is_some(){bail!("RPC_ERROR")} let result=response.get("result").context("RPC_RESULT")?; if result.is_null(){bail!("RPC_BLOCK_UNAVAILABLE")} let raw_transactions=result.get("transactions").and_then(Value::as_array).context("RPC_TRANSACTIONS")?.clone(); let mut transactions=Vec::new(); for (index,tx) in raw_transactions.iter().enumerate(){let mut parsed=relevant(tx)?; parsed.transaction_index=index.to_string(); transactions.push(parsed);} let proof=FinalizedBlockInput {slot:slot.to_string(),block_identity:result.get("blockhash").and_then(Value::as_str).context("RPC_BLOCKHASH")?.to_owned(),finalized:true,executed_transaction_count:transactions.len().to_string(),transactions}; Ok((proof,raw_transactions)) }
fn producer_slot_bundle(source:&str,epoch:&str,reconciliation:ReconciliationReceipt,transactions:&[Value],events:&HashMap<String,Vec<RawSemanticEvent>>)->Result<SlotReceiptBundle>{
    let slot:u64=reconciliation.slot.parse()?;let blockhash=reconciliation.block_identity.clone();let identities=identity_receipts(source,epoch,slot,&blockhash,&reconciliation.receipt_sha256,transactions)?;let mut paths=path_receipts(source,epoch,slot,&blockhash,transactions,events)?;paths.extend(creation_path_receipts(source,epoch,slot,&blockhash,transactions)?);let creations=creation_receipts(source,epoch,slot,&blockhash,transactions,&identities,&paths)?;let bundle=SlotReceiptBundle{reconciliation,identities,paths,creations};bundle.validate()?;Ok(bundle)
}
fn diagnostic_range_preflight(budget:&RpcBudget,start:u64,end:u64)->Result<()> { let span=end.checked_sub(start).and_then(|n|n.checked_add(1)).context("FINALIZED_RANGE_INVALID")?;if span>15||budget.used_requests().checked_add(1+span).context("FINALIZED_RANGE_BUDGET_OVERFLOW")?>budget.maximum_requests{bail!("FINALIZED_RANGE_RPC_BUDGET_INSUFFICIENT")}Ok(()) }
fn finalized_slot_inventory_from_mocked_rpc(root:&PathBuf,source:&str,epoch:&str,start:u64,end:u64,get_blocks_response:&Value,blocks:Vec<(FinalizedBlockInput,Vec<Value>)>,records:&[FilteredIdentity],events:&HashMap<String,Vec<RawSemanticEvent>>)->Result<FinalizedSlotInventory>{
    if end<start||end-start+1>15{bail!("FINALIZED_RANGE_DIAGNOSTIC_CAP")}if get_blocks_response.get("error").is_some(){bail!("GET_BLOCKS_RPC_ERROR")}let result=get_blocks_response.get("result").and_then(Value::as_array).context("GET_BLOCKS_RESULT")?;let mut produced=Vec::new();for value in result{let slot=value.as_u64().context("GET_BLOCKS_SLOT")?;if slot<start||slot>end||produced.contains(&slot){bail!("GET_BLOCKS_INVENTORY_INVALID")}produced.push(slot)}
    // Provider JSON may contain JSON numbers, which are intentionally outside
    // the JCS semantic-record contract. Store/hash the provider bytes exactly;
    // all EdgeSniffer receipts below are separately JCS-bound.
    let response_bytes=serde_json::to_vec(get_blocks_response)?;let response_sha=pump_exact_evidence_ledger::sha256_hex(&response_bytes);let tmp=root.join(".finalized-getblocks-response.tmp");fs::write(&tmp,response_bytes)?;fs::rename(tmp,root.join("finalized-getblocks-response.json"))?;let mut blocks_by_slot=BTreeMap::new();for(block,transactions)in blocks{let slot:u64=block.slot.parse()?;if blocks_by_slot.insert(slot,(block,transactions)).is_some(){bail!("FINALIZED_RANGE_DUPLICATE_BLOCK_RESPONSE")}}let mut proofs=Vec::new();for slot in &produced{let(proof,transactions)=blocks_by_slot.remove(slot).context("FINALIZED_RANGE_BLOCK_RESPONSE_MISSING")?;let receipt=reconcile_finalized_block(epoch,source,"1",proof.clone(),records,&programs())?;if receipt.disposition!="MATCHED"||receipt.reason_code.is_some(){bail!("FINALIZED_RANGE_RECONCILIATION_INCOMPLETE")}let bundle=producer_slot_bundle(source,epoch,receipt,&transactions,events)?;bundle.write_atomic(root)?;proofs.push(ReconciledProducedSlot{slot:slot.to_string(),block_identity:proof.block_identity,slot_reconciliation_receipt_sha256:bundle.reconciliation.receipt_sha256.clone()});}if !blocks_by_slot.is_empty(){bail!("FINALIZED_RANGE_UNEXPECTED_BLOCK_RESPONSE")}let inventory=FinalizedSlotInventory::seal(source,"1",start,end,&response_sha,&produced,proofs)?;inventory.write_atomic(root)?;Ok(inventory)
}
async fn fetch_pending_finalized_range(http:&reqwest::Client,url:&str,root:&PathBuf,start:u64,end:u64,budget:&mut RpcBudget,report:&mut Report)->Result<PendingFinalizedRange>{
    diagnostic_range_preflight(budget,start,end)?; budget.consume_one()?; report.inventory_get_blocks_rpc_calls+=1;
    let response:Value=http.post(url).json(&json!({"jsonrpc":"2.0","id":"bounded-finalized-slot-inventory","method":"getBlocks","params":[start,end,{"commitment":"finalized"}]})).send().await?.error_for_status()?.json().await?;
    if response.get("error").is_some(){bail!("GET_BLOCKS_RPC_ERROR")}
    let response_bytes=serde_json::to_vec(&response)?;let response_tmp=root.join(".finalized-getblocks-response.tmp");fs::write(&response_tmp,response_bytes)?;fs::rename(response_tmp,root.join("finalized-getblocks-response.json"))?;
    let result=response.get("result").and_then(Value::as_array).context("GET_BLOCKS_RESULT")?;
    let produced=result.len();
    if budget.used_requests().saturating_add(produced as u64)>budget.maximum_requests {bail!("FINALIZED_RANGE_RPC_BUDGET_INSUFFICIENT")}
    let mut blocks=Vec::new();for value in result {let slot=value.as_u64().context("GET_BLOCKS_SLOT")?;budget.consume_one()?;report.inventory_get_block_rpc_calls+=1;blocks.push(block(http,url,slot).await?)}
    Ok(PendingFinalizedRange {start,end,get_blocks_response:response,blocks})
}
fn pending_range_readiness(pending:&PendingFinalizedRange,source_finalized_slots:&BTreeSet<u64>,source:&str,epoch:&str,records:&[FilteredIdentity])->Result<PendingRangeReadiness>{
    for (block,_) in &pending.blocks {let slot:u64=block.slot.parse()?;if !source_finalized_slots.contains(&slot){return Ok(PendingRangeReadiness::AwaitingSourceFinalizedStatus)}}
    for (block,_) in &pending.blocks {let receipt=reconcile_finalized_block(epoch,source,"1",block.clone(),records,&programs())?;match (receipt.disposition.as_str(),receipt.reason_code.as_deref()) {("MATCHED",None)=>{},("REJECTED",Some("MISSING_FILTERED_IDENTITY"))=>return Ok(PendingRangeReadiness::AwaitingSourceIdentities),_=>bail!("FINALIZED_RANGE_SOURCE_RECONCILIATION_REJECTED:{:?}",receipt.reason_code)}}
    Ok(PendingRangeReadiness::ReadyToSeal)
}
fn seal_pending_finalized_range(root:&PathBuf,source:&str,epoch:&str,pending:&PendingFinalizedRange,source_finalized_slots:&BTreeSet<u64>,records:&[FilteredIdentity],events:&HashMap<String,Vec<RawSemanticEvent>>)->Result<Option<FinalizedSlotInventory>>{
    if pending_range_readiness(pending,source_finalized_slots,source,epoch,records)?!=PendingRangeReadiness::ReadyToSeal{return Ok(None)}
    Ok(Some(finalized_slot_inventory_from_mocked_rpc(root,source,epoch,pending.start,pending.end,&pending.get_blocks_response,pending.blocks.clone(),records,events)?))
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
    let run_binding=serde_json::json!({"schema":"CANONICAL_FINALIZED_CANARY_BINDING_V1","sourceInstanceId":source,"continuityEpoch":"1","startupFreeBytes":start_free.to_string(),"startupFloorBytes":start_floor.to_string(),"runtimeReserveBytes":reserve.to_string(),"totalOutputCapBytes":total_cap.to_string()});
    fs::write(root.join("run-binding.json"),serde_json::to_vec_pretty(&run_binding)?)?;
    let mut report=Report{schema:"CANONICAL_FINALIZED_RPC_RECONCILIATION_CANARY_V1".into(),status:"RUNNING".into(),started_at_utc:Utc::now().to_rfc3339(),finished_at_utc:String::new(),slots_observed:0,slots_reconciled:0,slots_rejected:0,exact_passes:0,failures:0,rpc_calls:0,dispatcher_rpc_calls:0,dispatcher_retry_rpc_calls:0,inventory_get_blocks_rpc_calls:0,inventory_get_block_rpc_calls:0,retries:0,pending_slot_peak:0,max_rss_bytes:0,max_message_bytes:0,disk_growth_bytes:0,decoder_transactions:0,decoder_events:0,decoder_rejections:0,source_gap_or_epoch_failure:false,unresolved:"NONE".into(),reconciled_slot_produced:false,scientific_interval_produced:false,failure:None};
    let result:Result<()> = async {
        let policy=StoragePolicy{manifest_quota_bytes:MANIFEST_QUOTA,protected_free_bytes:reserve,available_free_bytes:start_free,max_in_memory_identities:16384};
        let mut manifest=FilteredIdentityManifestWriter::open(&root,epoch,&source,"1",0,0,policy)?;
        let channel=Channel::from_shared(endpoint)?.tls_config(ClientTlsConfig::new().with_native_roots())?.connect_timeout(Duration::from_secs(10)).connect().await?;
        let t=token.clone();
        let mut client=GeyserClient::with_interceptor(channel,move|mut r:Request<()>|{r.metadata_mut().insert("x-token",t.clone());Ok::<_,Status>(r)}).max_decoding_message_size(MAX_MESSAGE);
        let mut updates=client.subscribe(Request::new(stream::once(async{req()}).chain(stream::pending()))).await?.into_inner();
        let http=reqwest::Client::builder().timeout(Duration::from_secs(20)).build()?;
        let mut dispatcher=Dispatcher::new(32,3,1000)?;
        let mut finalized_slots=BTreeSet::new(); let mut highest_finalized_slot=None::<u64>; let mut all_records=Vec::<FilteredIdentity>::new(); let mut raw_events=HashMap::<String,Vec<RawSemanticEvent>>::new(); let mut terminal=Vec::<(u64,String)>::new();let mut auto_range_start=None::<u64>;let mut pending_range=None::<PendingFinalizedRange>;let mut range_inventory_complete=initial_range_inventory_complete(range_slots);
        let started=std::time::Instant::now();
        while started.elapsed()<Duration::from_secs(max_seconds) && !range_inventory_complete {
            match tokio::time::timeout(Duration::from_millis(250),updates.next()).await {
                Ok(Some(Ok(update))) => {
                    let size=update.encoded_len(); report.max_message_bytes=report.max_message_bytes.max(size); report.max_rss_bytes=report.max_rss_bytes.max(rss());
                    if size>MAX_MESSAGE { bail!("MESSAGE_CAP") } if report.max_rss_bytes>MAX_RSS { bail!("RSS_CAP") }
                    if free(&root)? < reserve.saturating_add(total_cap.saturating_sub(fs::metadata(root.join(format!("{epoch}.filtered-identities.jsonl"))).map(|m|m.len()).unwrap_or(0))) { bail!("PROTECTED_RUNTIME_RESERVE") }
                    match update.update_oneof {
                        Some(UpdateOneof::Transaction(item)) => {
                            let info=item.transaction.clone().context("TRANSACTION_INFO")?; let signature=bs58::encode(&info.signature).into_string();
                            let raw_hash=pump_exact_evidence_ledger::sha256_hex(&item.encode_to_vec());
                            let identity=manifest.append(item.slot,&signature,Utc::now().timestamp_nanos_opt().unwrap_or_default() as u128,Some(info.index),&raw_hash)?;
                            all_records.push(identity); dispatcher.observe(item.slot,now_ms())?;
                            if finalized_slots.contains(&item.slot) { dispatcher.finalized_progress(item.slot,now_ms())?; }
                            report.decoder_transactions+=1; if let Some(meta)=info.meta.as_ref(){match decode_events(meta){Ok(events)=>{report.decoder_events+=events.len() as u64; raw_events.insert(signature.clone(),events.into_iter().map(|(log_index,event)|RawSemanticEvent{event_id:format!("{signature}:{log_index}"),log_index,event}).collect());},Err(_)=>report.decoder_rejections+=1}}
                        }
                        Some(UpdateOneof::Slot(slot)) if slot.status==SlotStatus::SlotFinalized as i32 => {
                            finalized_slots.insert(slot.slot);highest_finalized_slot=Some(highest_finalized_slot.map_or(slot.slot,|current|current.max(slot.slot))); if dispatcher.slots.contains_key(&slot.slot) { dispatcher.finalized_progress(slot.slot,now_ms())?; }
                        }
                        _ => {}
                    }
                }
                Ok(Some(Err(error))) => { report.source_gap_or_epoch_failure=true; bail!("SOURCE_GAP:{error}") }
                Ok(None) => { report.source_gap_or_epoch_failure=true; bail!("SOURCE_ENDED") }
                Err(_) => {}
            }
            report.pending_slot_peak=report.pending_slot_peak.max(dispatcher.active());
            while auto_range_start.is_none() && !range_inventory_complete { let Some(slot)=dispatcher.due(now_ms()) else {break};
                let retry_attempt=dispatcher.slots.get(&slot).is_some_and(|work|work.attempts>0);
                consume_dispatcher_rpc(&mut budget,reservation)?; report.dispatcher_rpc_calls+=1;if retry_attempt{report.dispatcher_retry_rpc_calls+=1;}
                let disposition=match block(&http,&rpc,slot).await {
                    Ok((proof,transactions)) => { let receipt=reconcile_finalized_block(epoch,&source,"1",proof,&all_records,&programs())?; if receipt.disposition=="MATCHED" { let bundle=producer_slot_bundle(&source,"1",receipt,&transactions,&raw_events)?;bundle.write_atomic(&root)?;"MATCHED".to_owned() } else { report.unresolved=receipt.reason_code.unwrap_or_else(||"RECONCILIATION_REJECTED".into()); report.unresolved.clone() } }
                    Err(error) if error.to_string().contains("RPC_BLOCK_UNAVAILABLE") => "RPC_NOT_FINALIZED".to_owned(),
                    Err(error) => { report.unresolved=error.to_string(); report.unresolved.clone() }
                };
                if disposition=="MISSING_FILTERED_IDENTITY" || disposition=="RPC_NOT_FINALIZED" { report.retries+=1; }
                dispatcher.result(slot,now_ms(),&disposition)?; persist_state(&root,&source,&dispatcher,&terminal)?;if disposition=="MATCHED" {break}
            }
            for (slot,state) in dispatcher.drain_completed() { let outcome=match state { SlotState::Matched=>"MATCHED", SlotState::Failed=>"FAILED", _=>"INVALID" }.to_owned(); if outcome=="MATCHED" {report.exact_passes+=1; report.slots_reconciled+=1; report.reconciled_slot_produced=true;if range_slots.is_some()&&auto_range_start.is_none(){auto_range_start=Some(slot)}if range_slots.is_none(){range_inventory_complete=true}} else {report.failures+=1; report.slots_rejected+=1;} terminal.push((slot,outcome)); persist_state(&root,&source,&dispatcher,&terminal)?; }
            if pending_range.is_none(){if let (Some(start),Some(slots))=(auto_range_start,range_slots){if finalized_range_ready(start,slots,highest_finalized_slot)?{let end=start.checked_add(slots-1).context("FINALIZED_RANGE_OVERFLOW")?;pending_range=Some(fetch_pending_finalized_range(&http,&rpc,&root,start,end,&mut budget,&mut report).await?);}}}
            if let Some(pending)=pending_range.as_ref(){match pending_range_readiness(pending,&finalized_slots,&source,epoch,&all_records)? {PendingRangeReadiness::AwaitingSourceFinalizedStatus=>report.unresolved="WAITING_FOR_SOURCE_FINALIZED_STATUS".into(),PendingRangeReadiness::AwaitingSourceIdentities=>report.unresolved="WAITING_FOR_SOURCE_IDENTITIES".into(),PendingRangeReadiness::ReadyToSeal=>{seal_pending_finalized_range(&root,&source,epoch,pending,&finalized_slots,&all_records,&raw_events)?.context("FINALIZED_RANGE_SEAL_READY_MISSING")?;range_inventory_complete=true;}}}
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
    fn pending_fixture()->PendingFinalizedRange{PendingFinalizedRange{start:100,end:104,get_blocks_response:json!({"jsonrpc":"2.0","id":"fixture","result":[100,101,102,103,104]}),blocks:(100..=104).map(|slot|pending_block(slot,&[&format!("sig-{slot}")])).collect()}}
    fn fixture_records(root:&PathBuf,slots:&[u64])->Vec<FilteredIdentity>{
        let policy=StoragePolicy{manifest_quota_bytes:1024*1024,protected_free_bytes:0,available_free_bytes:1024*1024*1024,max_in_memory_identities:32};
        let mut writer=FilteredIdentityManifestWriter::open(root,"epoch","source","1",0,0,policy).unwrap();
        for slot in slots {writer.append(*slot,&format!("sig-{slot}"),*slot as u128,Some(0),"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();}
        records(root,"epoch").unwrap()
    }
    fn fixture_root(label:&str)->PathBuf{let root=std::env::temp_dir().join(format!("canonical-range-{label}-{}",std::process::id()));let _=fs::remove_dir_all(&root);fs::create_dir_all(&root).unwrap();root}

    #[test]
    fn subscription_binds_filtered_transactions_and_explicit_finalized_progress() {
        let request = req();
        assert_eq!(request.commitment, Some(CommitmentLevel::Finalized as i32));
        assert_eq!(request.transactions.get("canonicalRelevant").and_then(|filter| filter.failed), Some(false));
        assert_eq!(request.transactions.get("canonicalRelevant").and_then(|filter| filter.vote), Some(false));
        assert_eq!(request.slots.get("finalizedProgress").and_then(|filter| filter.filter_by_commitment), Some(true));
        assert_eq!(request.slots.get("finalizedProgress").and_then(|filter| filter.interslot_updates), Some(false));
    }

    #[test]
    fn fifth_finalized_before_late_source_identities_waits_then_becomes_sealable() {
        let pending=pending_fixture();let finalized=(100..=104).collect::<BTreeSet<_>>();
        let incomplete_root=fixture_root("late-before");let incomplete=fixture_records(&incomplete_root,&[100,101,102,103]);
        assert_eq!(pending_range_readiness(&pending,&finalized,"source","epoch",&incomplete).unwrap(),PendingRangeReadiness::AwaitingSourceIdentities);
        let complete_root=fixture_root("late-after");let complete=fixture_records(&complete_root,&[100,101,102,103,104]);
        assert_eq!(pending_range_readiness(&pending,&finalized,"source","epoch",&complete).unwrap(),PendingRangeReadiness::ReadyToSeal);
        let _=fs::remove_dir_all(incomplete_root);let _=fs::remove_dir_all(complete_root);
    }

    #[test]
    fn range_waits_for_each_produced_slot_source_finalized_status_not_a_higher_slot() {
        let pending=pending_fixture();let records_root=fixture_root("finalized-status");let records=fixture_records(&records_root,&[100,101,102,103,104]);
        let missing_fifth=(100..=103).collect::<BTreeSet<_>>();
        assert_eq!(pending_range_readiness(&pending,&missing_fifth,"source","epoch",&records).unwrap(),PendingRangeReadiness::AwaitingSourceFinalizedStatus);
        let _=fs::remove_dir_all(records_root);
    }

    #[test]
    fn missing_fifth_source_evidence_cannot_seal_complete_inventory_and_zero_relevant_remains_valid() {
        let pending=pending_fixture();let finalized=(100..=104).collect::<BTreeSet<_>>();let root=fixture_root("missing");let records=fixture_records(&root,&[100,101,102,103]);
        assert_ne!(pending_range_readiness(&pending,&finalized,"source","epoch",&records).unwrap(),PendingRangeReadiness::ReadyToSeal);
        let zero=PendingFinalizedRange{start:200,end:204,get_blocks_response:json!({"jsonrpc":"2.0","id":"zero","result":[200,201,202,203,204]}),blocks:(200..=204).map(|slot|pending_block(slot,&[])).collect()};
        assert_eq!(pending_range_readiness(&zero,&(200..=204).collect(),"source","epoch",&[]).unwrap(),PendingRangeReadiness::ReadyToSeal);
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
        let paths=creation_path_receipts("source","1",42,"block",&txs).unwrap();
        let creations=creation_receipts("source","1",42,"block",&txs,&identities,&paths).unwrap();
        assert_eq!(identities.len(),1); assert_eq!(paths.len(),1); assert_eq!(creations.len(),1);
        assert_eq!(creations[0].path_receipt_sha256,paths[0].receipt_sha256);
        assert_eq!(creations[0].identity_receipt_sha256,identities[0].receipt_sha256);
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
        assert_eq!(inventory.entries.len(),3);assert_eq!(inventory.entries[0].disposition,"PRODUCED_RECONCILED");assert_eq!(inventory.entries[1].disposition,"FINALIZED_SKIPPED");assert_eq!(inventory.entries[2].disposition,"PRODUCED_RECONCILED");
        let accepted=seal_forward_cohort_from_persisted_receipt_chain(&root,"cohort",10).unwrap();assert_eq!(accepted.pools.len(),1);
        fs::remove_file(root.join("path-receipt-slot-11.json")).unwrap();assert!(seal_forward_cohort_from_persisted_receipt_chain(&root,"cohort",10).is_err());
        let migration_tx=creation_fixture();let mut relevant_tx=relevant(&migration_tx).unwrap();relevant_tx.transaction_index="0".into();
        let _inventory=finalized_slot_inventory_from_mocked_rpc(&root,"source","1",11,13,&json!({"jsonrpc":"2.0","id":"fixture","result":[11,13]}),vec![
            (FinalizedBlockInput{slot:"11".into(),block_identity:"block-11".into(),finalized:true,executed_transaction_count:"1".into(),transactions:vec![relevant_tx]},vec![migration_tx]),
            (FinalizedBlockInput{slot:"13".into(),block_identity:"block-13".into(),finalized:true,executed_transaction_count:"0".into(),transactions:vec![]},vec![]),
        ],&records(&root,"canary-epoch-1").unwrap(),&HashMap::new()).unwrap();
        let path=root.join("path-receipt-slot-11.json");let mut tampered:Value=serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();tampered[0]["pool"]=Value::String("tampered-pool".into());fs::write(&path,pump_exact_evidence_ledger::canonical_state_v1::jcs_bytes(&tampered).unwrap()).unwrap();assert!(seal_forward_cohort_from_persisted_receipt_chain(&root,"cohort",10).is_err());
        let migration_tx=creation_fixture();let mut relevant_tx=relevant(&migration_tx).unwrap();relevant_tx.transaction_index="0".into();
        let _inventory=finalized_slot_inventory_from_mocked_rpc(&root,"source","1",11,13,&json!({"jsonrpc":"2.0","id":"fixture","result":[11,13]}),vec![
            (FinalizedBlockInput{slot:"11".into(),block_identity:"block-11".into(),finalized:true,executed_transaction_count:"1".into(),transactions:vec![relevant_tx]},vec![migration_tx]),
            (FinalizedBlockInput{slot:"13".into(),block_identity:"block-13".into(),finalized:true,executed_transaction_count:"0".into(),transactions:vec![]},vec![]),
        ],&records(&root,"canary-epoch-1").unwrap(),&HashMap::new()).unwrap();
        let path=root.join("creation-receipt-slot-11.json");let mut duplicate:Value=serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();let clone=duplicate[0].clone();duplicate.as_array_mut().unwrap().push(clone);fs::write(&path,pump_exact_evidence_ledger::canonical_state_v1::jcs_bytes(&duplicate).unwrap()).unwrap();assert!(seal_forward_cohort_from_persisted_receipt_chain(&root,"cohort",10).is_err());
        let migration_tx=creation_fixture();let mut relevant_tx=relevant(&migration_tx).unwrap();relevant_tx.transaction_index="0".into();
        assert!(finalized_slot_inventory_from_mocked_rpc(&root,"source","1",11,13,&json!({"jsonrpc":"2.0","id":"fixture","result":[11,13]}),vec![(FinalizedBlockInput{slot:"11".into(),block_identity:"block-11".into(),finalized:true,executed_transaction_count:"1".into(),transactions:vec![relevant_tx]},vec![migration_tx])],&records(&root,"canary-epoch-1").unwrap(),&HashMap::new()).is_err());
        let mut budget=RpcBudget::new(16,1,1).unwrap();budget.consume_one().unwrap();assert!(diagnostic_range_preflight(&budget,1,15).is_err());assert!(diagnostic_range_preflight(&budget,1,14).is_ok());
        let _=fs::remove_dir_all(root);
    }

    #[test]
    fn five_produced_zero_relevant_slots_still_emit_complete_receipt_chain() {
        use pump_exact_evidence_ledger::forward_new_pool_cohort_v1::seal_forward_cohort_from_persisted_receipt_chain;
        let root=std::env::temp_dir().join(format!("canonical-five-produced-{}",std::process::id()));let _=fs::remove_dir_all(&root);fs::create_dir_all(&root).unwrap();
        let slots=vec![20_u64,21,22,23,24];let blocks=slots.iter().map(|slot|(FinalizedBlockInput{slot:slot.to_string(),block_identity:format!("block-{slot}"),finalized:true,executed_transaction_count:"0".into(),transactions:vec![]},vec![])).collect();
        let inventory=finalized_slot_inventory_from_mocked_rpc(&root,"source","1",20,24,&json!({"jsonrpc":"2.0","id":"fixture","result":slots}),blocks,&[],&HashMap::new()).unwrap();
        assert_eq!(inventory.entries.iter().filter(|entry|entry.disposition=="PRODUCED_RECONCILED").count(),5);
        assert_eq!(seal_forward_cohort_from_persisted_receipt_chain(&root,"cohort",19).unwrap().pools.len(),0);
        let _=fs::remove_dir_all(root);
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
