//! Bounded, non-scientific Shyft block-inventory conformance probe.
//!
//! This binary asks for an empty Yellowstone block account filter exactly once,
//! collects at most eight finalised block updates, then uses at most one
//! finalised `getBlock` call per collected slot. It never writes raw blocks,
//! runs no decoder, creates no scientific state, and treats every mismatch as
//! an unverified provider capability.

use std::{collections::{BTreeMap, HashMap}, env, fs, path::PathBuf, process::Command, time::Duration};

use anyhow::{bail, Context, Result};
use chrono::Utc;
use futures::{stream, StreamExt};
use prost::Message;
use serde::Serialize;
use serde_json::{json, Value};
use tonic::{metadata::AsciiMetadataValue, transport::{Channel, ClientTlsConfig}, Request, Status};
use yellowstone_grpc_proto::geyser::{geyser_client::GeyserClient, subscribe_update::UpdateOneof, CommitmentLevel, SubscribeRequest, SubscribeRequestFilterBlocks};

const MAX_SLOT_LIMIT: usize = 8;
const MAX_SECONDS: u64 = 600;
const MAX_MESSAGE_BYTES: usize = 256 * 1024 * 1024;
// The authorization permits up to 4 GiB, but this probe writes only one compact
// JSON report. A 64 MiB cap leaves the live hot-volume reserve intact.
const MAX_OUTPUT_BYTES: u64 = 64 * 1024 * 1024;
const PROTECTED_RESERVE_BYTES: u64 = 200 * 1024 * 1024 * 1024;
const MAX_RSS_BYTES: u64 = 8 * 1024 * 1024 * 1024;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
struct Identity {
    transaction_index: u64,
    signature_sha256: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Inventory {
    slot: u64,
    blockhash_sha256: String,
    executed_transaction_count: u64,
    identities: Vec<Identity>,
    message_bytes: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SlotResult {
    slot: u64,
    result: String,
    grpc_count: u64,
    rpc_count: Option<usize>,
    grpc_blockhash_sha256: String,
    rpc_blockhash_sha256: Option<String>,
    grpc_identity_digest: String,
    rpc_identity_digest: Option<String>,
    grpc_message_bytes: usize,
    detail: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProbeReport {
    schema: String,
    started_at_utc: String,
    finished_at_utc: String,
    status: String,
    requested_account_include_empty: bool,
    requested_commitment: String,
    slots_requested: usize,
    slots_received: usize,
    rpc_requests: usize,
    max_message_bytes: usize,
    max_output_bytes: u64,
    protected_reserve_bytes: u64,
    max_rss_bytes: u64,
    max_observed_rss_bytes: u64,
    max_observed_message_bytes: usize,
    elapsed_millis: u128,
    process_cpu_ticks: Option<u64>,
    output_bytes: u64,
    slot_results: Vec<SlotResult>,
    failure: Option<String>,
}

fn sha256_hex(bytes: &[u8]) -> String {
    pump_exact_evidence_ledger::sha256_hex(bytes)
}

fn free_bytes(root: &PathBuf) -> Result<u64> {
    let output = Command::new("df")
        .args(["-B1", "--output=avail", root.to_str().context("non-utf8 root")?])
        .output()?;
    if !output.status.success() { bail!("DF_FAILED") }
    let text = String::from_utf8(output.stdout)?;
    text.lines().nth(1).context("DF_OUTPUT_MISSING")?.trim().parse().context("DF_OUTPUT_INVALID")
}

fn status_rss_bytes() -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    status.lines().find_map(|line| {
        let value = line.strip_prefix("VmHWM:")?.split_whitespace().next()?.parse::<u64>().ok()?;
        Some(value.saturating_mul(1024))
    })
}

fn process_cpu_ticks() -> Option<u64> {
    let stat = fs::read_to_string("/proc/self/stat").ok()?;
    let fields = stat.rsplit_once(") ")?.1.split_whitespace().collect::<Vec<_>>();
    let utime = fields.get(11)?.parse::<u64>().ok()?;
    let stime = fields.get(12)?.parse::<u64>().ok()?;
    Some(utime.saturating_add(stime))
}

fn request() -> SubscribeRequest {
    let mut blocks = HashMap::new();
    blocks.insert("unfilteredInventory".to_owned(), SubscribeRequestFilterBlocks {
        account_include: vec![],
        include_transactions: Some(true),
        include_accounts: Some(false),
        include_entries: Some(false),
        cuckoo_account_include: None,
    });
    SubscribeRequest {
        accounts: HashMap::new(), slots: HashMap::new(), transactions: HashMap::new(),
        transactions_status: HashMap::new(), blocks, blocks_meta: HashMap::new(),
        entry: HashMap::new(), accounts_data_slice: vec![],
        commitment: Some(CommitmentLevel::Finalized as i32), ping: None, from_slot: None,
    }
}

fn digest_identities(identities: &[Identity]) -> String {
    let bytes = serde_json::to_vec(identities).expect("identity serialization");
    sha256_hex(&bytes)
}

fn block_inventory(block: &yellowstone_grpc_proto::geyser::SubscribeUpdateBlock, message_bytes: usize) -> Result<Inventory> {
    let mut identities = Vec::with_capacity(block.transactions.len());
    for transaction in &block.transactions {
        if transaction.signature.is_empty() { bail!("GRPC_EMPTY_SIGNATURE") }
        identities.push(Identity {
            transaction_index: transaction.index,
            signature_sha256: sha256_hex(&transaction.signature),
        });
    }
    identities.sort();
    if identities.windows(2).any(|pair| pair[0].transaction_index == pair[1].transaction_index || pair[0].signature_sha256 == pair[1].signature_sha256) {
        bail!("GRPC_DUPLICATE_IDENTITY")
    }
    for (expected, identity) in identities.iter().enumerate() {
        if identity.transaction_index != expected as u64 { bail!("GRPC_NONCONTIGUOUS_INDEX") }
    }
    if block.executed_transaction_count != identities.len() as u64 { bail!("GRPC_EXECUTED_COUNT_MISMATCH") }
    Ok(Inventory {
        slot: block.slot,
        blockhash_sha256: sha256_hex(block.blockhash.as_bytes()),
        executed_transaction_count: block.executed_transaction_count,
        identities,
        message_bytes,
    })
}

fn rpc_inventory(slot: u64, value: &Value) -> Result<Inventory> {
    if let Some(error) = value.get("error") { bail!("RPC_ERROR:{error}") }
    let result = value.get("result").context("RPC_RESULT_MISSING")?;
    if result.is_null() { bail!("RPC_BLOCK_UNAVAILABLE") }
    let blockhash = result.get("blockhash").and_then(Value::as_str).context("RPC_BLOCKHASH_MISSING")?;
    let transactions = result.get("transactions").and_then(Value::as_array).context("RPC_TRANSACTIONS_MISSING")?;
    let mut identities = Vec::with_capacity(transactions.len());
    for (index, transaction) in transactions.iter().enumerate() {
        let signature = transaction.pointer("/transaction/signatures/0").and_then(Value::as_str).context("RPC_SIGNATURE_MISSING")?;
        identities.push(Identity { transaction_index: index as u64, signature_sha256: sha256_hex(signature.as_bytes()) });
    }
    Ok(Inventory {
        slot,
        blockhash_sha256: sha256_hex(blockhash.as_bytes()),
        executed_transaction_count: identities.len() as u64,
        identities,
        message_bytes: 0,
    })
}

fn compare(grpc: &Inventory, rpc: &Inventory) -> Result<()> {
    if grpc.slot != rpc.slot { bail!("SLOT_MISMATCH") }
    if grpc.blockhash_sha256 != rpc.blockhash_sha256 { bail!("BLOCKHASH_MISMATCH") }
    if grpc.executed_transaction_count != rpc.executed_transaction_count { bail!("TRANSACTION_COUNT_MISMATCH") }
    if grpc.identities != rpc.identities { bail!("IDENTITY_OR_CANONICAL_INDEX_MISMATCH") }
    Ok(())
}

async fn finalized_block(client: &reqwest::Client, rpc_url: &str, slot: u64) -> Result<Value> {
    let request = json!({
        "jsonrpc": "2.0", "id": slot, "method": "getBlock",
        // JSON is required here because the comparison deliberately reads the
        // ordered message signatures. Base64 carries opaque transaction bytes.
        "params": [slot, {"commitment": "finalized", "encoding": "json", "transactionDetails": "full", "rewards": false, "maxSupportedTransactionVersion": 1}]
    });
    Ok(client.post(rpc_url).json(&request).send().await?.error_for_status()?.json().await?)
}

fn write_report(root: &PathBuf, report: &mut ProbeReport) -> Result<()> {
    let target = root.join("report.json");
    let temporary = root.join("report.json.tmp");
    // Stabilize the self-reported compact output size without putting raw
    // transaction data in the report.
    let mut bytes = serde_json::to_vec_pretty(report)?;
    report.output_bytes = bytes.len() as u64;
    bytes = serde_json::to_vec_pretty(report)?;
    if bytes.len() as u64 > MAX_OUTPUT_BYTES { bail!("OUTPUT_QUOTA_REACHED") }
    fs::write(&temporary, &bytes)?;
    fs::rename(temporary, target)?;
    report.output_bytes = bytes.len() as u64;
    Ok(())
}

fn redacted_error(error: &anyhow::Error) -> String {
    // Provider URL/token values must never be written to a durable report.
    // Network stacks commonly embed a complete request URL in their errors,
    // so retain a stable category instead of trying to partially redact it.
    let text = error.to_string();
    if text.contains("://") || text.contains("authorization") || text.contains("token") {
        "NETWORK_OR_PROVIDER_ERROR_REDACTED".to_owned()
    } else {
        text
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| anyhow::anyhow!("RUSTLS_PROVIDER_UNAVAILABLE"))?;
    let root = PathBuf::from(env::var("SHYFT_BLOCK_PROBE_ROOT").context("SHYFT_BLOCK_PROBE_ROOT_MISSING")?);
    let endpoint = env::var("YELLOWSTONE_FULL_META_ENDPOINT").context("YELLOWSTONE_FULL_META_ENDPOINT_MISSING")?;
    let token: AsciiMetadataValue = env::var("SHYFT_GRPC_X_TOKEN").context("SHYFT_GRPC_X_TOKEN_MISSING")?.parse()?;
    let rpc_url = env::var("SECURE_RPC_URL").context("SECURE_RPC_URL_MISSING")?;
    let slot_limit = env::var("SHYFT_BLOCK_PROBE_SLOT_LIMIT")
        .unwrap_or_else(|_| MAX_SLOT_LIMIT.to_string())
        .parse::<usize>()
        .context("SHYFT_BLOCK_PROBE_SLOT_LIMIT_INVALID")?;
    if slot_limit == 0 || slot_limit > MAX_SLOT_LIMIT { bail!("SHYFT_BLOCK_PROBE_SLOT_LIMIT_OUT_OF_RANGE") }
    fs::create_dir_all(&root)?;
    if free_bytes(&root)? <= PROTECTED_RESERVE_BYTES.saturating_add(MAX_OUTPUT_BYTES) { bail!("PROTECTED_RESERVE_PREFLIGHT_FAILED") }
    let started = std::time::Instant::now();
    let mut report = ProbeReport {
        schema: "SHYFT_BLOCK_INVENTORY_CONFORMANCE_V1".to_owned(),
        started_at_utc: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
        finished_at_utc: String::new(), status: "RUNNING".to_owned(),
        requested_account_include_empty: true, requested_commitment: "finalized".to_owned(),
        slots_requested: slot_limit, slots_received: 0, rpc_requests: 0,
        max_message_bytes: MAX_MESSAGE_BYTES, max_output_bytes: MAX_OUTPUT_BYTES,
        protected_reserve_bytes: PROTECTED_RESERVE_BYTES, max_rss_bytes: MAX_RSS_BYTES,
        max_observed_rss_bytes: 0, max_observed_message_bytes: 0, elapsed_millis: 0,
        process_cpu_ticks: None, output_bytes: 0, slot_results: vec![], failure: None,
    };

    let outcome: Result<()> = async {
        let channel = Channel::from_shared(endpoint)?.tls_config(ClientTlsConfig::new().with_native_roots())?.connect_timeout(Duration::from_secs(10)).connect().await?;
        let interceptor = token.clone();
        let mut client = GeyserClient::with_interceptor(channel, move |mut request: Request<()>| {
            request.metadata_mut().insert("x-token", interceptor.clone());
            Ok::<_, Status>(request)
        }).max_decoding_message_size(MAX_MESSAGE_BYTES);
        let requests = stream::once(async { request() }).chain(stream::pending());
        let mut updates = client.subscribe(Request::new(requests)).await?.into_inner();
        let mut blocks = BTreeMap::<u64, Inventory>::new();
        while blocks.len() < slot_limit && started.elapsed() < Duration::from_secs(MAX_SECONDS) {
            let update = tokio::time::timeout(Duration::from_secs(30), updates.next()).await.context("GRPC_RECEIVE_TIMEOUT")?.context("GRPC_STREAM_ENDED")??;
            let encoded = update.encoded_len();
            report.max_observed_message_bytes = report.max_observed_message_bytes.max(encoded);
            report.max_observed_rss_bytes = report.max_observed_rss_bytes.max(status_rss_bytes().unwrap_or_default());
            if encoded > MAX_MESSAGE_BYTES { bail!("GRPC_MESSAGE_LIMIT_BREACH") }
            if report.max_observed_rss_bytes > MAX_RSS_BYTES { bail!("RSS_LIMIT_BREACH") }
            if free_bytes(&root)? <= PROTECTED_RESERVE_BYTES { bail!("PROTECTED_RESERVE_REACHED") }
            if let Some(UpdateOneof::Block(block)) = update.update_oneof {
                let inventory = block_inventory(&block, encoded)?;
                if blocks.insert(inventory.slot, inventory).is_some() { bail!("DUPLICATE_SLOT_UPDATE") }
            }
        }
        if blocks.len() != slot_limit { bail!("INSUFFICIENT_BLOCKS_WITHIN_RUNTIME_LIMIT") }
        report.slots_received = blocks.len();
        let http = reqwest::Client::builder().timeout(Duration::from_secs(30)).build()?;
        for inventory in blocks.values() {
            if report.rpc_requests >= slot_limit { bail!("RPC_REQUEST_BUDGET_EXCEEDED") }
            let response = finalized_block(&http, &rpc_url, inventory.slot).await?;
            report.rpc_requests += 1;
            let rpc = rpc_inventory(inventory.slot, &response)?;
            let comparison = compare(inventory, &rpc);
            report.slot_results.push(SlotResult {
                slot: inventory.slot,
                result: if comparison.is_ok() { "MATCHED_COMPLETE_INVENTORY".to_owned() } else { "MISMATCH".to_owned() },
                grpc_count: inventory.executed_transaction_count,
                rpc_count: Some(rpc.executed_transaction_count as usize),
                grpc_blockhash_sha256: inventory.blockhash_sha256.clone(),
                rpc_blockhash_sha256: Some(rpc.blockhash_sha256.clone()),
                grpc_identity_digest: digest_identities(&inventory.identities),
                rpc_identity_digest: Some(digest_identities(&rpc.identities)),
                grpc_message_bytes: inventory.message_bytes,
                detail: comparison.as_ref().err().map(redacted_error),
            });
            comparison?;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        Ok(())
    }.await;

    report.finished_at_utc = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Nanos, true);
    report.elapsed_millis = started.elapsed().as_millis();
    report.process_cpu_ticks = process_cpu_ticks();
    report.max_observed_rss_bytes = report.max_observed_rss_bytes.max(status_rss_bytes().unwrap_or_default());
    match &outcome {
        Ok(()) => report.status = "PASS_UNFILTERED_BLOCK_INVENTORY_MATCHED_ON_EIGHT_SLOTS".to_owned(),
        Err(error) => { report.status = "UNVERIFIED_OR_REJECTED".to_owned(); report.failure = Some(redacted_error(error)); }
    }
    write_report(&root, &mut report)?;
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inventory(slot: u64, ids: &[(u64, &str)]) -> Inventory {
        Inventory {
            slot, blockhash_sha256: "block".to_owned(), executed_transaction_count: ids.len() as u64,
            identities: ids.iter().map(|(index, signature)| Identity { transaction_index: *index, signature_sha256: signature.to_string() }).collect(),
            message_bytes: 1,
        }
    }

    #[test]
    fn equal_count_does_not_hide_identity_mismatch() {
        let grpc = inventory(1, &[(0, "a"), (1, "b")]);
        let rpc = inventory(1, &[(0, "a"), (1, "c")]);
        assert!(compare(&grpc, &rpc).unwrap_err().to_string().contains("IDENTITY_OR_CANONICAL_INDEX_MISMATCH"));
    }

    #[test]
    fn canonical_index_order_is_part_of_the_comparison() {
        let grpc = inventory(1, &[(0, "a"), (1, "b")]);
        let rpc = inventory(1, &[(0, "b"), (1, "a")]);
        assert!(compare(&grpc, &rpc).is_err());
    }
}
