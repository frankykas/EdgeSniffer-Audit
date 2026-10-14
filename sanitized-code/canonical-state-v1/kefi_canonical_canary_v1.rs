//! Bounded capture-only Canonical State v1 producer canary.
//!
//! This program deliberately emits no scientific decisions.  It writes a
//! compact semantic journal plus a quota-capped raw fixture so decoder parity
//! can later be proven against identical full-meta bytes.  Events lacking a
//! resolved instruction path remain ORDER_AMBIGUOUS and cannot be admitted.

use std::{collections::{BTreeMap, BTreeSet, HashMap}, env, fs, path::PathBuf, process::Command, time::Duration};

use anyhow::{bail, Context, Result};
use chrono::Utc;
use futures::{stream, StreamExt};
use prost::Message;
use pump_exact_evidence_ledger::{canonical_state_v1::{jcs_bytes, JournalWriter, SemanticEvent}, sha256_hex, EvidenceMetadata, FrameRecord, LedgerWriter, SCHEMA_VERSION};
use pumpfun_sidecar::kefi_pumpswap_semantics_v1::{cache_identity, decode_events, pool_is_explicitly_unusable, resolve_pool_identity_batch, PoolIdentity, PUMPSWAP_PROGRAM};
use serde::Serialize;
use tokio::sync::mpsc;
use tonic::{metadata::AsciiMetadataValue, transport::{Channel, ClientTlsConfig}, Request, Status};
use yellowstone_grpc_proto::geyser::{geyser_client::GeyserClient, subscribe_update::UpdateOneof, CommitmentLevel, SubscribeRequest, SubscribeRequestFilterBlocks, SubscribeUpdate};

const PUMP_PROGRAM: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
const PUMP_CPI_ROUTER_PROGRAM: &str = "6Vo3245eszAb5wuqEMw8mGdbfRUdKbHhDHP5LcaGuTAB";

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct CanaryHealth {
    schema: String, status: String, source_instance_id: String, continuity_epoch: String,
    raw_fixture_bytes: u64, raw_fixture_cap_bytes: u64, semantic_events: u64, semantic_bytes: u64,
    source_updates: u64, pending_slots: usize, closed_slots: u64, order_ambiguous_events: u64,
    decoder_rejections: u64, identity_failures: u64, late_after_slot_close: u64,
    hot_available_bytes: u64, protected_reserve_bytes: u64, max_total_bytes: u64,
    stop_reason: Option<String>, last_slot: Option<u64>,
}

fn write_atomic(path: &PathBuf, value: &impl Serialize) -> Result<()> {
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(value)?)?;
    fs::rename(temporary, path)?;
    Ok(())
}

fn free_bytes(path: &PathBuf) -> Result<u64> {
    let output = Command::new("df").args(["-B1", "--output=avail", path.to_str().context("non-utf8 root")?]).output()?;
    if !output.status.success() { bail!("df failed") }
    String::from_utf8(output.stdout)?.lines().nth(1).context("df output missing")?.trim().parse().context("invalid df available bytes")
}

fn request() -> SubscribeRequest {
    // A complete block is the authoritative closure for its transaction set.
    // The previous slot-status canary demonstrated that a SLOT_COMPLETED update
    // can arrive before a matching transaction in this provider stream, so it
    // is not used to close scientific ordering buffers.
    let mut blocks = HashMap::new();
    blocks.insert("supportedPumpBlocks".to_owned(), SubscribeRequestFilterBlocks {
        account_include: vec![PUMP_PROGRAM.to_owned(), PUMP_CPI_ROUTER_PROGRAM.to_owned(), PUMPSWAP_PROGRAM.to_owned()],
        include_transactions: Some(true), include_accounts: Some(false), include_entries: Some(false), cuckoo_account_include: None,
    });
    SubscribeRequest { accounts: HashMap::new(), slots: HashMap::new(), transactions: HashMap::new(), transactions_status: HashMap::new(), blocks, blocks_meta: HashMap::new(), entry: HashMap::new(), accounts_data_slice: vec![], commitment: Some(CommitmentLevel::Confirmed as i32), ping: None, from_slot: None }
}

fn raw_record(update: &SubscribeUpdate, raw: Vec<u8>, source: &str, sequence: u64) -> FrameRecord {
    let received = Utc::now();
    let (kind, slot, transaction_index, signature) = match update.update_oneof.as_ref() {
        Some(UpdateOneof::Transaction(transaction)) => {
            let info = transaction.transaction.as_ref();
            ("transaction", Some(transaction.slot), info.map(|item| item.index), info.map(|item| bs58::encode(&item.signature).into_string()))
        }
        Some(UpdateOneof::Block(block)) => ("block", Some(block.slot), None, None),
        _ => ("other", None, None, None),
    };
    let raw_sha = sha256_hex(&raw);
    FrameRecord { metadata: EvidenceMetadata {
        schema_version: SCHEMA_VERSION.to_owned(), evidence_id: sha256_hex(format!("canary|{source}|{sequence}|{raw_sha}").as_bytes()), raw_sha256: raw_sha,
        kind: kind.to_owned(), raw_format: "yellowstone_subscribe_update_protobuf_full_meta".to_owned(), received_at_utc: received.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true), received_at_unix_ns: received.timestamp_nanos_opt().unwrap_or_default(), decode_at_unix_ns: received.timestamp_nanos_opt().unwrap_or_default(), writer_sequence: sequence, continuity_epoch: Some(1), source_instance_id: Some(source.to_owned()), slot, transaction_index, signature, account_pubkey: None, account_owner: None, write_version: None, finalized: false, successful: None, failed: None, unresolved: true, pump_instruction_kinds: vec![], pump_instruction_coordinates: vec![], discovered_mints: vec![], has_trade_event: false, has_fee_evidence: false, missing_raw_tx: false, missing_trade_event: false, missing_reserve_transition: false, missing_canonical_ordering: true, missing_fee_evidence: false, decode_failed: false, filters: vec!["capture-only-canonical-canary".to_owned()],
    }, raw }
}

fn semantic_from_event(sequence: u64, source: &str, raw_hash: &str, slot: u64, transaction_index: u64, event_index: usize, signature: String, event: pumpfun_sidecar::kefi_pumpswap_semantics_v1::Event, identity: &PoolIdentity) -> SemanticEvent {
    SemanticEvent {
        schema: String::new(), canonical_sequence: sequence.to_string(), source_instance_id: source.to_owned(), epoch_id: "1".to_owned(), event_identity: sha256_hex(format!("pumpswap-v1|{slot}|{transaction_index}|{event_index}|{}|{}|{}", event.pool, identity.selection_mint, event.side).as_bytes()),
        // The existing decoder indexes log events, but does not yet provide
        // authoritative instruction paths.  Preserve that limitation rather
        // than inventing a path from arrival order.
        causal_order: pump_exact_evidence_ledger::canonical_state_v1::CausalOrder { slot: slot.to_string(), transaction_index: Some(transaction_index.to_string()), instruction_path: None, semantic_ordinal: Some(event_index.to_string()) }, causal_order_status: "ORDER_AMBIGUOUS".to_owned(),
        kind: "PUMPSWAP_SWAP".to_owned(), signature, pool: Some(event.pool), mint: Some(identity.selection_mint.clone()), actor: Some(event.user), side: Some(event.side.to_owned()), pool_base_raw: Some(event.pool_base.to_string()), pool_quote_raw: Some(event.pool_quote.to_string()), quote_delta_raw: Some(event.pool_quote_delta.to_string()), fee_tuple: Some([event.lp_fee_bps.to_string(), event.protocol_fee_bps.to_string(), event.coin_creator_bps.to_string(), event.cashback_bps.to_string(), event.buyback_bps.to_string()]), observed_at_unix_ns: Utc::now().timestamp_nanos_opt().unwrap_or_default().to_string(), parser_version: "KEFI_PUMPSWAP_DECODER_V1".to_owned(), raw_frame_sha256: Some(raw_hash.to_owned()), previous_record_sha256: String::new(), record_sha256: String::new(),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    rustls::crypto::ring::default_provider().install_default().map_err(|_| anyhow::anyhow!("rustls provider unavailable"))?;
    let endpoint = env::var("YELLOWSTONE_FULL_META_ENDPOINT").context("YELLOWSTONE_FULL_META_ENDPOINT missing")?;
    let token: AsciiMetadataValue = env::var("SHYFT_GRPC_X_TOKEN")?.parse()?;
    let root = PathBuf::from(env::var("CANONICAL_CANARY_ROOT").context("CANONICAL_CANARY_ROOT missing")?);
    let rpc_url = env::var("KEFI_POOL_IDENTITY_RPC_URL").context("KEFI_POOL_IDENTITY_RPC_URL missing")?;
    let raw_cap: u64 = env::var("CANONICAL_CANARY_RAW_CAP_BYTES").context("CANONICAL_CANARY_RAW_CAP_BYTES missing")?.parse()?;
    let total_cap: u64 = env::var("CANONICAL_CANARY_TOTAL_CAP_BYTES").context("CANONICAL_CANARY_TOTAL_CAP_BYTES missing")?.parse()?;
    let reserve: u64 = env::var("CANONICAL_CANARY_PROTECTED_RESERVE_BYTES").context("CANONICAL_CANARY_PROTECTED_RESERVE_BYTES missing")?.parse()?;
    let max_seconds: u64 = env::var("CANONICAL_CANARY_MAX_SECONDS").context("CANONICAL_CANARY_MAX_SECONDS missing")?.parse()?;
    let fixture_ttl_seconds: u64 = env::var("CANONICAL_CANARY_FIXTURE_TTL_SECONDS").unwrap_or_else(|_| "86400".to_owned()).parse()?;
    if raw_cap == 0 || total_cap < raw_cap || max_seconds == 0 { bail!("invalid bounded canary limits") }
    fs::create_dir_all(&root)?;
    if free_bytes(&root)? <= reserve { bail!("protected hot-storage reserve is already breached") }
    let source = format!("canonical-canary-{}-pid{}", Utc::now().format("%Y%m%dT%H%M%S%3fZ"), std::process::id());
    let raw_root = root.join("validation-fixture-ttl"); fs::create_dir_all(&raw_root)?;
    write_atomic(&root.join("fixture-ttl.json"), &serde_json::json!({
        "schema": "CANONICAL_CANARY_FIXTURE_TTL_V1", "createdAtUtc": Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        "expiresAtUtc": (Utc::now() + chrono::Duration::seconds(fixture_ttl_seconds as i64)).to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        "ttlSeconds": fixture_ttl_seconds.to_string(), "rawFixtureCapBytes": raw_cap.to_string(), "purpose": "decoder-and-canonical-replay-validation-only"
    }))?;
    let mut raw_writer = LedgerWriter::open(&raw_root)?;
    let mut semantic_writer = JournalWriter::open(&root.join("semantic-journal"), "1", 1, 4096)?;
    let health_path = root.join("health.json");
    let mut health = CanaryHealth { schema: "EDGESNIFFER_CANONICAL_CANARY_V1".to_owned(), status: "RUNNING".to_owned(), source_instance_id: source.clone(), continuity_epoch: "1".to_owned(), raw_fixture_cap_bytes: raw_cap, protected_reserve_bytes: reserve, max_total_bytes: total_cap, hot_available_bytes: free_bytes(&root)?, ..CanaryHealth::default() };
    let channel = Channel::from_shared(endpoint)?.tls_config(ClientTlsConfig::new().with_native_roots())?.connect_timeout(Duration::from_secs(10)).connect().await?;
    let interceptor = token.clone();
    let mut client = GeyserClient::with_interceptor(channel, move |mut request: Request<()>| { request.metadata_mut().insert("x-token", interceptor.clone()); Ok::<_, Status>(request) }).max_decoding_message_size(128 * 1024 * 1024);
    let requests = stream::once(async { request() }).chain(stream::pending());
    let mut updates = client.subscribe(Request::new(requests)).await?.into_inner();
    // Receive task intentionally does no decoding, Pool RPC, hashing, or disk
    // work.  The bounded queue exposes overload rather than silently losing
    // transport order.
    let (source_tx, mut source_rx) = mpsc::channel::<std::result::Result<SubscribeUpdate, String>>(4_096);
    tokio::spawn(async move {
        loop {
            let next = match updates.next().await {
                Some(Ok(update)) => Ok(update),
                Some(Err(error)) => Err(format!("{error:#}")),
                None => Err("yellowstone stream ended".to_owned()),
            };
            let terminal = next.is_err();
            if source_tx.send(next).await.is_err() || terminal { break; }
        }
    });
    let mut identities = BTreeMap::<String, PoolIdentity>::new();
    let http = reqwest::Client::builder().timeout(Duration::from_secs(8)).build()?;
    let started = tokio::time::Instant::now(); let mut raw_sequence = 0_u64; let mut semantic_sequence = 0_u64;
    while started.elapsed() < Duration::from_secs(max_seconds) {
        let update = tokio::time::timeout(Duration::from_secs(20), source_rx.recv()).await.context("source silence")?.context("source receiver ended")?.map_err(anyhow::Error::msg)?;
        raw_sequence += 1; let raw = update.encode_to_vec();
        if health.raw_fixture_bytes.saturating_add(raw.len() as u64) > raw_cap { health.stop_reason = Some("RAW_FIXTURE_CAP_REACHED".into()); break; }
        if health.raw_fixture_bytes.saturating_add(health.semantic_bytes).saturating_add(raw.len() as u64) > total_cap { health.stop_reason = Some("TOTAL_CANARY_CAP_REACHED".into()); break; }
        if health.source_updates % 512 == 0 { health.hot_available_bytes = free_bytes(&root)?; if health.hot_available_bytes <= reserve { health.stop_reason = Some("PROTECTED_RESERVE_REACHED".into()); break; } }
        let record = raw_record(&update, raw, &source, raw_sequence); health.raw_fixture_bytes += record.raw.len() as u64; health.source_updates += 1;
        raw_writer.append_buffered(Utc::now(), record.clone())?;
        if let Some(UpdateOneof::Block(block)) = update.update_oneof {
            // A block update carries the provider's complete transaction inventory
            // plus its authoritative transaction indexes. It replaces the unsafe
            // use of transport-delivered SLOT_COMPLETED as an ordering barrier.
            health.last_slot = Some(block.slot); health.closed_slots += 1;
            let raw_hash = record.metadata.raw_sha256.clone();
            let mut decoded_transactions = Vec::new();
            for info in block.transactions {
                let Some(meta) = info.meta.as_ref() else { continue };
                match decode_events(meta) {
                    Ok(events) => decoded_transactions.push((info.index, bs58::encode(&info.signature).into_string(), events)),
                    Err(_) => health.decoder_rejections += 1,
                }
            }
            let unknown = decoded_transactions.iter().flat_map(|(_, _, events)| events.iter().map(|(_, event)| event.pool.clone())).filter(|pool| !identities.contains_key(pool)).collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>();
            if !unknown.is_empty() {
                for (_pool, resolution) in resolve_pool_identity_batch(&http, &rpc_url, &unknown).await? {
                    match resolution {
                        Ok(identity) => { cache_identity(&mut identities, identity)?; }
                        Err(error) if pool_is_explicitly_unusable(&error) => {}
                        Err(_) => health.identity_failures += 1,
                    }
                }
            }
            decoded_transactions.sort_by_key(|(index, _, _)| *index);
            for (transaction_index, signature, mut events) in decoded_transactions {
                events.sort_by_key(|(event_index, _)| *event_index);
                for (event_index, event) in events {
                    if let Some(identity) = identities.get(&event.pool) {
                        semantic_sequence += 1;
                        let event = semantic_from_event(semantic_sequence, &source, &raw_hash, block.slot, transaction_index, event_index, signature.clone(), event, identity);
                        let sealed = semantic_writer.append(event)?;
                        health.semantic_bytes += jcs_bytes(&serde_json::to_value(&sealed)?)?.len() as u64;
                        health.semantic_events += 1; health.order_ambiguous_events += 1;
                    }
                }
            }
        }
        if health.source_updates % 512 == 0 { raw_writer.sync_buffered()?; write_atomic(&health_path, &health)?; }
    }
    raw_writer.sync_buffered()?; raw_writer.finalize_current(true)?; semantic_writer.seal_segment()?;
    health.status = if health.stop_reason.is_some() { "SEALED".into() } else { "TIME_LIMIT_REACHED".into() };
    write_atomic(&health_path, &health)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_contract_uses_complete_blocks_not_arrival_order_slot_closure() {
        let subscription = request();
        assert!(subscription.transactions.is_empty());
        assert!(subscription.slots.is_empty());
        let filter = subscription.blocks.get("supportedPumpBlocks").expect("block filter");
        assert_eq!(filter.include_transactions, Some(true));
        assert_eq!(filter.include_accounts, Some(false));
        assert_eq!(filter.include_entries, Some(false));
        assert!(filter.account_include.contains(&PUMP_PROGRAM.to_owned()));
        assert!(filter.account_include.contains(&PUMPSWAP_PROGRAM.to_owned()));
    }
}
