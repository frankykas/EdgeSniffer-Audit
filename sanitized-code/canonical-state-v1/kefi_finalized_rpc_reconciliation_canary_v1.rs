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
use pump_exact_evidence_ledger::{filtered_identity_manifest_v1::{reconcile_finalized_block, FilteredIdentity, FilteredIdentityManifestWriter, FinalizedBlockInput, FinalizedTransactionInput, RpcBudget, StoragePolicy}, pump_instruction_path_decoder_v1::{decode_instruction_paths, CompiledInstructionInput, FullTransactionInput, InnerInstructionGroupInput, InnerInstructionInput, SemanticEventInput}};
use pumpfun_sidecar::kefi_pumpswap_semantics_v1::{decode_events, Event, BUY_DISC, BUY_EXACT_DISC, SELL_DISC, PUMPSWAP_PROGRAM};

const PUMP: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const ROUTER: &str = "6Vo3245eszAb5wuqEMw8mGdbfRUdKbHhDHP5LcaGuTAB";
const MAX_SECONDS: u64 = 600;
const MAX_SLOTS: usize = 4;
const MAX_RPC: u64 = 8;
const MAX_MESSAGE: usize = 128 * 1024 * 1024;
const MAX_RSS: u64 = 8 * 1024 * 1024 * 1024;
const MANIFEST_QUOTA: u64 = 64 * 1024 * 1024;
const OPERATIONAL_BUFFER: u64 = 15 * 1024 * 1024 * 1024;
// The kEFi hot volume currently has about 189 GiB free. This leaves a 160 GiB
// protected reserve while still permitting this 64 MiB manifest-only canary.
const PROTECTED_RESERVE: u64 = 160 * 1024 * 1024 * 1024;

#[derive(Serialize)] #[serde(rename_all="camelCase")]
struct Report { schema:String, status:String, started_at_utc:String, finished_at_utc:String, slots_observed:usize, slots_reconciled:usize, slots_rejected:usize, exact_passes:usize, failures:usize, rpc_calls:u64, retries:u64, pending_slot_peak:usize, max_rss_bytes:u64, max_message_bytes:usize, disk_growth_bytes:u64, decoder_transactions:u64, decoder_events:u64, decoder_rejections:u64, source_gap_or_epoch_failure:bool, unresolved:String, reconciled_slot_produced:bool, scientific_interval_produced:bool, failure:Option<String> }

fn free(path:&PathBuf)->Result<u64>{ let out=Command::new("df").args(["-B1","--output=avail",path.to_str().context("PATH")?]).output()?; Ok(String::from_utf8(out.stdout)?.lines().nth(1).context("DF")?.trim().parse()?) }
fn rss()->u64 { fs::read_to_string("/proc/self/status").ok().and_then(|s|s.lines().find_map(|l|l.strip_prefix("VmHWM:")?.split_whitespace().next()?.parse::<u64>().ok())).unwrap_or(0)*1024 }
fn req()->SubscribeRequest { let mut transactions=HashMap::new(); let mut slots=HashMap::new(); transactions.insert("canonicalRelevant".into(), SubscribeRequestFilterTransactions { vote:Some(false), failed:Some(false), signature:None, account_include:vec![PUMP.into(),ROUTER.into(),PUMPSWAP_PROGRAM.into()], account_exclude:vec![], account_required:vec![], token_accounts:None }); slots.insert("finalizedProgress".into(), SubscribeRequestFilterSlots { filter_by_commitment:Some(true), interslot_updates:Some(false) }); SubscribeRequest { accounts:HashMap::new(), slots, transactions, transactions_status:HashMap::new(), blocks:HashMap::new(), blocks_meta:HashMap::new(), entry:HashMap::new(), accounts_data_slice:vec![], commitment:Some(CommitmentLevel::Finalized as i32), ping:None, from_slot:None } }
fn programs()->BTreeSet<String>{ [PUMP.to_owned(),ROUTER.to_owned(),PUMPSWAP_PROGRAM.to_owned()].into_iter().collect() }
fn relevant(tx:&Value)->Result<FinalizedTransactionInput>{ let signature=tx.pointer("/transaction/signatures/0").and_then(Value::as_str).context("RPC_SIGNATURE")?.to_owned(); let mut ids=BTreeSet::new(); for key in tx.pointer("/transaction/message/accountKeys").and_then(Value::as_array).context("RPC_KEYS")? { if let Some(s)=key.as_str(){ids.insert(s.to_owned());} else if let Some(s)=key.get("pubkey").and_then(Value::as_str){ids.insert(s.to_owned());} else {bail!("RPC_KEY_LAYOUT")} } for p in ["/meta/loadedAddresses/writable","/meta/loadedAddresses/readonly"] { if let Some(keys)=tx.pointer(p).and_then(Value::as_array){ for key in keys { ids.insert(key.as_str().context("RPC_LOADED_KEY")?.to_owned()); } } } let failed = tx.pointer("/meta/err").map(|v| !v.is_null()).unwrap_or(true); if failed { ids.clear(); } let supported_version = match tx.get("version") { None => true, Some(value) if value.as_str()==Some("legacy") => true, Some(value) => value.as_u64().is_some_and(|version| version <= 1) }; Ok(FinalizedTransactionInput { transaction_index:String::new(), signature, account_or_program_ids:ids.into_iter().collect(), supported_version }) }

#[derive(Clone)]
struct RawSemanticEvent { event_id: String, log_index: usize, event: Event }

#[derive(Serialize)]
#[serde(rename_all="camelCase")]
struct PathReceipt { schema: String, slot: String, finalized_blockhash: String, signature: String, event_id: String, semantic_kind: String, pool: String, canonical_order: Value, disposition: String, reason_code: Option<String>, receipt_sha256: String }

fn json_keys(tx:&Value, path:&str)->Result<Vec<String>> { tx.pointer(path).and_then(Value::as_array).context("RPC_ACCOUNT_KEYS")?.iter().map(|key| key.as_str().or_else(||key.get("pubkey").and_then(Value::as_str)).map(str::to_owned).context("RPC_ACCOUNT_KEY_LAYOUT")).collect() }
fn json_instruction(value:&Value)->Result<CompiledInstructionInput> { Ok(CompiledInstructionInput { program_id_index:value.get("programIdIndex").and_then(Value::as_u64).context("RPC_PROGRAM_INDEX")? as usize, account_indexes:value.get("accounts").and_then(Value::as_array).context("RPC_ACCOUNTS")?.iter().map(|value|value.as_u64().map(|n|n as usize).context("RPC_ACCOUNT_INDEX")).collect::<Result<Vec<_>>>()?, data:bs58::decode(value.get("data").and_then(Value::as_str).context("RPC_INSTRUCTION_DATA")?).into_vec().context("RPC_INSTRUCTION_DATA_BASE58")? }) }
fn finalized_input(slot:u64, index:usize, tx:&Value)->Result<FullTransactionInput> { let signature=tx.pointer("/transaction/signatures/0").and_then(Value::as_str).context("RPC_SIGNATURE")?.to_owned(); let outer=tx.pointer("/transaction/message/instructions").and_then(Value::as_array).context("RPC_OUTER_INSTRUCTIONS")?.iter().map(json_instruction).collect::<Result<Vec<_>>>()?; let mut inner=Vec::new(); if let Some(groups)=tx.pointer("/meta/innerInstructions").and_then(Value::as_array) { for group in groups { let parent=group.get("index").and_then(Value::as_u64).context("RPC_INNER_PARENT_INDEX")? as usize; let instructions=group.get("instructions").and_then(Value::as_array).context("RPC_INNER_INSTRUCTIONS")?.iter().map(|value| Ok(InnerInstructionInput { program_id_index:value.get("programIdIndex").and_then(Value::as_u64).context("RPC_INNER_PROGRAM_INDEX")? as usize, account_indexes:value.get("accounts").and_then(Value::as_array).context("RPC_INNER_ACCOUNTS")?.iter().map(|value|value.as_u64().map(|n|n as usize).context("RPC_INNER_ACCOUNT_INDEX")).collect::<Result<Vec<_>>>()?, data:bs58::decode(value.get("data").and_then(Value::as_str).context("RPC_INNER_DATA")?).into_vec().context("RPC_INNER_DATA_BASE58")?, stack_height:value.get("stackHeight").and_then(Value::as_u64).map(|n|n as usize) })).collect::<Result<Vec<_>>>()?; inner.push(InnerInstructionGroupInput { parent_outer_index:parent, instructions }); } }
    Ok(FullTransactionInput { slot, finalized_transaction_index:Some(index as u64), signature, succeeded:tx.pointer("/meta/err").map(Value::is_null).unwrap_or(false), static_account_keys:json_keys(tx,"/transaction/message/accountKeys")?, loaded_writable_account_keys:tx.pointer("/meta/loadedAddresses/writable").map(|_|json_keys(tx,"/meta/loadedAddresses/writable")).transpose()?.unwrap_or_default(), loaded_readonly_account_keys:tx.pointer("/meta/loadedAddresses/readonly").map(|_|json_keys(tx,"/meta/loadedAddresses/readonly")).transpose()?.unwrap_or_default(), outer_instructions:outer, inner_instruction_groups:inner, log_messages:tx.pointer("/meta/logMessages").and_then(Value::as_array).context("RPC_LOG_MESSAGES")?.iter().map(|value|value.as_str().map(str::to_owned).context("RPC_LOG_ENTRY")).collect::<Result<Vec<_>>>()? }) }
fn path_receipts(slot:u64, blockhash:&str, txs:&[Value], events:&HashMap<String,Vec<RawSemanticEvent>>)->Result<Vec<PathReceipt>> { let mut out=Vec::new(); for (index,tx) in txs.iter().enumerate() { let signature=tx.pointer("/transaction/signatures/0").and_then(Value::as_str).context("RPC_SIGNATURE")?; let Some(raw)=events.get(signature) else {continue}; let input=finalized_input(slot,index,tx); let semantic=raw.iter().map(|raw| SemanticEventInput { event_id:raw.event_id.clone(), expected_program_id:PUMPSWAP_PROGRAM.into(), pool:raw.event.pool.clone(), log_index:raw.log_index, accepted_instruction_discriminators:if raw.event.side=="BUY" {vec![BUY_DISC.to_vec(),BUY_EXACT_DISC.to_vec()]} else {vec![SELL_DISC.to_vec()]}, semantic_kind:format!("PUMPSWAP_{}",raw.event.side) }).collect::<Vec<_>>(); match input.and_then(|input|decode_instruction_paths(&input,&semantic)) { Ok(decoded) => for event in decoded { let order=&event.canonical_order; let canonical_order=json!({"slot":order.slot.to_string(),"finalizedTransactionIndex":order.finalized_transaction_index.to_string(),"outerInstructionIndex":order.instruction_path.outer_index.to_string(),"innerInstructionIndex":order.instruction_path.inner_index.map(|v|v.to_string()),"stackHeight":order.instruction_path.stack_height.map(|v|v.to_string()),"semanticOrdinal":order.semantic_ordinal.to_string()}); let mut receipt=PathReceipt {schema:"CANONICAL_FINALIZED_INSTRUCTION_PATH_RECEIPT_V1".into(),slot:slot.to_string(),finalized_blockhash:blockhash.into(),signature:event.signature, event_id:event.event_id,semantic_kind:event.semantic_kind,pool:event.pool,canonical_order,disposition:"PATH_RESOLVED_QUARANTINED_UNIVERSE_INCOMPLETE".into(),reason_code:Some("CANDIDATE_UNIVERSE_INCOMPLETE".into()),receipt_sha256:String::new()}; receipt.receipt_sha256=pump_exact_evidence_ledger::canonical_state_v1::hash_excluding_field(&receipt,"receipt_sha256")?; out.push(receipt); }, Err(error)=> for raw in raw { let mut receipt=PathReceipt {schema:"CANONICAL_FINALIZED_INSTRUCTION_PATH_RECEIPT_V1".into(),slot:slot.to_string(),finalized_blockhash:blockhash.into(),signature:signature.into(),event_id:raw.event_id.clone(),semantic_kind:format!("PUMPSWAP_{}",raw.event.side),pool:raw.event.pool.clone(),canonical_order:Value::Null,disposition:"QUARANTINED".into(),reason_code:Some(error.to_string()),receipt_sha256:String::new()}; receipt.receipt_sha256=pump_exact_evidence_ledger::canonical_state_v1::hash_excluding_field(&receipt,"receipt_sha256")?; out.push(receipt); }
    }} Ok(out) }
async fn block(http:&reqwest::Client,url:&str,slot:u64)->Result<(FinalizedBlockInput,Vec<Value>)>{ let response:Value=http.post(url).json(&json!({"jsonrpc":"2.0","id":slot,"method":"getBlock","params":[slot,{"commitment":"finalized","encoding":"json","transactionDetails":"full","rewards":false,"maxSupportedTransactionVersion":1}]})).send().await?.error_for_status()?.json().await?; if response.get("error").is_some(){bail!("RPC_ERROR")} let result=response.get("result").context("RPC_RESULT")?; if result.is_null(){bail!("RPC_BLOCK_UNAVAILABLE")} let raw_transactions=result.get("transactions").and_then(Value::as_array).context("RPC_TRANSACTIONS")?.clone(); let mut transactions=Vec::new(); for (index,tx) in raw_transactions.iter().enumerate(){let mut parsed=relevant(tx)?; parsed.transaction_index=index.to_string(); transactions.push(parsed);} let proof=FinalizedBlockInput {slot:slot.to_string(),block_identity:result.get("blockhash").and_then(Value::as_str).context("RPC_BLOCKHASH")?.to_owned(),finalized:true,executed_transaction_count:transactions.len().to_string(),transactions}; Ok((proof,raw_transactions)) }
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
    if total_cap != 6_u64*1024*1024*1024 || start_floor < reserve.saturating_add(total_cap).saturating_add(OPERATIONAL_BUFFER) { bail!("STORAGE_POLICY_INVALID") }
    fs::create_dir_all(&root)?;
    let start_free=free(&root)?;
    if start_free < start_floor { bail!("PROTECTED_STARTUP_FLOOR") }
    // A previous incomplete source instance may not be bridged after a process
    // restart. Its manifest is durable audit evidence, not resumable input.
    if root.join("run-binding.json").exists() { bail!("DURABLE_PRIOR_RUN_REQUIRES_NEW_ROOT") }
    let source=format!("finalized-rpc-canary-{}-{}",Utc::now().format("%Y%m%dT%H%M%SZ"),std::process::id());
    let epoch="canary-epoch-1";
    let run_binding=serde_json::json!({"schema":"CANONICAL_FINALIZED_CANARY_BINDING_V1","sourceInstanceId":source,"continuityEpoch":"1","startupFreeBytes":start_free.to_string(),"startupFloorBytes":start_floor.to_string(),"runtimeReserveBytes":reserve.to_string(),"totalOutputCapBytes":total_cap.to_string()});
    fs::write(root.join("run-binding.json"),serde_json::to_vec_pretty(&run_binding)?)?;
    let mut report=Report{schema:"CANONICAL_FINALIZED_RPC_RECONCILIATION_CANARY_V1".into(),status:"RUNNING".into(),started_at_utc:Utc::now().to_rfc3339(),finished_at_utc:String::new(),slots_observed:0,slots_reconciled:0,slots_rejected:0,exact_passes:0,failures:0,rpc_calls:0,retries:0,pending_slot_peak:0,max_rss_bytes:0,max_message_bytes:0,disk_growth_bytes:0,decoder_transactions:0,decoder_events:0,decoder_rejections:0,source_gap_or_epoch_failure:false,unresolved:"NONE".into(),reconciled_slot_produced:false,scientific_interval_produced:false,failure:None};
    let result:Result<()> = async {
        let policy=StoragePolicy{manifest_quota_bytes:MANIFEST_QUOTA,protected_free_bytes:reserve,available_free_bytes:start_free,max_in_memory_identities:16384};
        let mut manifest=FilteredIdentityManifestWriter::open(&root,epoch,&source,"1",0,0,policy)?;
        let channel=Channel::from_shared(endpoint)?.tls_config(ClientTlsConfig::new().with_native_roots())?.connect_timeout(Duration::from_secs(10)).connect().await?;
        let t=token.clone();
        let mut client=GeyserClient::with_interceptor(channel,move|mut r:Request<()>|{r.metadata_mut().insert("x-token",t.clone());Ok::<_,Status>(r)}).max_decoding_message_size(MAX_MESSAGE);
        let mut updates=client.subscribe(Request::new(stream::once(async{req()}).chain(stream::pending()))).await?.into_inner();
        let http=reqwest::Client::builder().timeout(Duration::from_secs(20)).build()?;
        let mut budget=RpcBudget::new(16,1,1000)?;
        let mut dispatcher=Dispatcher::new(32,3,1000)?;
        let mut finalized_slots=BTreeSet::new(); let mut all_records=Vec::<FilteredIdentity>::new(); let mut raw_events=HashMap::<String,Vec<RawSemanticEvent>>::new(); let mut terminal=Vec::<(u64,String)>::new();
        let started=std::time::Instant::now();
        while started.elapsed()<Duration::from_secs(max_seconds) && report.exact_passes==0 {
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
                            finalized_slots.insert(slot.slot); if dispatcher.slots.contains_key(&slot.slot) { dispatcher.finalized_progress(slot.slot,now_ms())?; }
                        }
                        _ => {}
                    }
                }
                Ok(Some(Err(error))) => { report.source_gap_or_epoch_failure=true; bail!("SOURCE_GAP:{error}") }
                Ok(None) => { report.source_gap_or_epoch_failure=true; bail!("SOURCE_ENDED") }
                Err(_) => {}
            }
            report.pending_slot_peak=report.pending_slot_peak.max(dispatcher.active());
            while let Some(slot)=dispatcher.due(now_ms()) {
                if budget.used_requests()>=16 { bail!("RPC_BUDGET_EXHAUSTED") }
                budget.consume_one()?; report.rpc_calls+=1;
                let disposition=match block(&http,&rpc,slot).await {
                    Ok((proof,transactions)) => { let blockhash=proof.block_identity.clone(); let receipt=reconcile_finalized_block(epoch,&source,"1",proof,&all_records,&programs())?; let encoded=serde_json::to_vec_pretty(&receipt)?; fs::write(root.join(format!("receipt-slot-{slot}.json")),encoded)?; if receipt.disposition=="MATCHED" { let paths=path_receipts(slot,&blockhash,&transactions,&raw_events)?; let temporary=root.join(format!("path-receipt-slot-{slot}.tmp")); fs::write(&temporary,serde_json::to_vec_pretty(&paths)?)?; fs::rename(temporary,root.join(format!("path-receipt-slot-{slot}.json")))?; "MATCHED".to_owned() } else { report.unresolved=receipt.reason_code.unwrap_or_else(||"RECONCILIATION_REJECTED".into()); report.unresolved.clone() } }
                    Err(error) if error.to_string().contains("RPC_BLOCK_UNAVAILABLE") => "RPC_NOT_FINALIZED".to_owned(),
                    Err(error) => { report.unresolved=error.to_string(); report.unresolved.clone() }
                };
                if disposition=="MISSING_FILTERED_IDENTITY" || disposition=="RPC_NOT_FINALIZED" { report.retries+=1; }
                dispatcher.result(slot,now_ms(),&disposition)?; persist_state(&root,&source,&dispatcher,&terminal)?;
            }
            for (slot,state) in dispatcher.drain_completed() { let outcome=match state { SlotState::Matched=>"MATCHED", SlotState::Failed=>"FAILED", _=>"INVALID" }.to_owned(); if outcome=="MATCHED" {report.exact_passes+=1; report.slots_reconciled+=1; report.reconciled_slot_produced=true;} else {report.failures+=1; report.slots_rejected+=1;} terminal.push((slot,outcome)); persist_state(&root,&source,&dispatcher,&terminal)?; }
        }
        if !report.reconciled_slot_produced { bail!("NO_EXACT_FINALIZED_RECONCILIATION_PASS") }
        // A reconciled block proves only filtered identity coverage for that
        // block. It is deliberately not an eligible-pool universe or a full
        // scientific market interval, so no semantic state is released here.
        Ok(())
    }.await;
    report.slots_observed=all_manifest_slot_count(&root, epoch).unwrap_or(0); report.finished_at_utc=Utc::now().to_rfc3339(); report.max_rss_bytes=report.max_rss_bytes.max(rss()); report.disk_growth_bytes=start_free.saturating_sub(free(&root)?);
    match &result { Ok(_)=>report.status="FALLBACK_CANARY_VERIFIED".into(), Err(error)=>{report.status="FALLBACK_CANARY_FAILED_OR_BLOCKED".into();report.failure=Some(error.to_string())} }
    fs::write(root.join("report.json"),serde_json::to_vec_pretty(&report)?)?;
    result
}

#[tokio::main]
async fn main()->Result<()> { integrated_main().await }

#[cfg(test)]
mod integrated_tests {
    use super::*;

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
        assert!(181_u64 * 1024 * 1024 * 1024 >= reserve + cap + OPERATIONAL_BUFFER);
        assert!(180_u64 * 1024 * 1024 * 1024 < reserve + cap + OPERATIONAL_BUFFER);
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
