//! Offline-only durable identity manifest and finalized-block reconciliation.
//!
//! The manifest is deliberately written *before* wallet/candidate selection.
//! It preserves Yellowstone delivery provenance and its independent block
//! transaction-index cross-check without treating either as canonical Solana
//! order. The reconciler compares that complete received identity set with a
//! finalized block-derived, program/account-filtered identity set. Any
//! identity or source-index mismatch is a hard failure.

use std::{collections::{BTreeMap, BTreeSet}, fs::{self, File, OpenOptions}, io::{BufRead, BufReader, Write}, path::{Path, PathBuf}};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::{canonical_capture_run_v1::CanonicalRelevancePolicy, canonical_state_v1::{hash_excluding_field, jcs_bytes, sha256_hex}};

pub const FILTERED_IDENTITY_SCHEMA: &str = "EDGESNIFFER_FILTERED_IDENTITY_MANIFEST_V1";
pub const FILTERED_IDENTITY_CHECKPOINT_SCHEMA: &str = "EDGESNIFFER_FILTERED_IDENTITY_CHECKPOINT_V1";
pub const FINALIZED_RECONCILIATION_SCHEMA: &str = "EDGESNIFFER_FINALIZED_RECONCILIATION_V1";
pub const MAX_IDENTITIES_PER_SLOT: usize = 4_096;
pub const MAX_PENDING_SLOTS: usize = 32;

#[derive(Clone, Debug)]
pub struct RpcBudget {
    pub maximum_requests: u64,
    pub maximum_concurrency: usize,
    pub minimum_interval_ms: u64,
    used_requests: u64,
}

impl RpcBudget {
    pub fn new(maximum_requests: u64, maximum_concurrency: usize, minimum_interval_ms: u64) -> Result<Self> {
        if maximum_requests == 0 || maximum_concurrency == 0 { bail!("RPC_BUDGET_INVALID") }
        Ok(Self { maximum_requests, maximum_concurrency, minimum_interval_ms, used_requests: 0 })
    }
    pub fn consume_one(&mut self) -> Result<()> {
        if self.used_requests >= self.maximum_requests { bail!("RPC_REQUEST_BUDGET_EXHAUSTED") }
        self.used_requests += 1;
        Ok(())
    }
    pub fn used_requests(&self) -> u64 { self.used_requests }
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilteredIdentity {
    pub schema: String,
    pub manifest_sequence: String,
    pub source_instance_id: String,
    pub continuity_epoch: String,
    pub slot: String,
    pub signature: String,
    pub receive_sequence: String,
    pub received_at_unix_ns: String,
    /// Yellowstone `SubscribeUpdateTransactionInfo.index`: the source
    /// transaction index in its block. This intentionally retains the legacy
    /// serialized `observedArrivalIndex` key so existing immutable manifests
    /// and their semantic hashes remain readable. Receive sequence and
    /// timestamp are the delivery provenance. Finalized RPC remains the
    /// authoritative ordering source.
    #[serde(rename = "observedArrivalIndex", alias = "sourceTransactionIndex")]
    pub source_transaction_index: Option<String>,
    pub source_frame_sha256: String,
    pub continuity_status: String,
    pub previous_record_sha256: String,
    pub record_sha256: String,
}

impl FilteredIdentity {
    pub fn seal(mut self, previous: String) -> Result<Self> {
        self.schema = FILTERED_IDENTITY_SCHEMA.to_owned();
        self.previous_record_sha256 = previous;
        self.record_sha256 = hash_excluding_field(&self, "recordSha256")?;
        Ok(self)
    }

    pub fn validate(&self, expected_manifest_sequence: u64, previous: &str) -> Result<()> {
        if self.schema != FILTERED_IDENTITY_SCHEMA { bail!("FILTERED_IDENTITY_SCHEMA_INVALID") }
        if self.manifest_sequence.parse::<u64>()? != expected_manifest_sequence { bail!("FILTERED_IDENTITY_MANIFEST_SEQUENCE_GAP") }
        if self.receive_sequence.parse::<u64>().is_err() || self.slot.parse::<u64>().is_err() || self.received_at_unix_ns.parse::<u128>().is_err() {
            bail!("FILTERED_IDENTITY_PROVENANCE_INVALID")
        }
        if self.source_instance_id.is_empty() || self.continuity_epoch.is_empty() || self.signature.is_empty() || self.source_frame_sha256.is_empty() {
            bail!("FILTERED_IDENTITY_REQUIRED_FIELD_MISSING")
        }
        if self.continuity_status != "CONTINUOUS" { bail!("FILTERED_IDENTITY_CONTINUITY_NOT_CLEAN") }
        if self.previous_record_sha256 != previous { bail!("FILTERED_IDENTITY_HASH_CHAIN_BREAK") }
        if self.record_sha256 != hash_excluding_field(self, "recordSha256")? { bail!("FILTERED_IDENTITY_HASH_INVALID") }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct StoragePolicy {
    pub manifest_quota_bytes: u64,
    pub protected_free_bytes: u64,
    pub available_free_bytes: u64,
    /// The manifest keeps only an identity de-duplication index in memory.
    /// This hard cap makes the memory bound independent of the byte quota.
    pub max_in_memory_identities: usize,
}

impl StoragePolicy {
    fn permit(&self, existing: u64, next: u64) -> Result<()> {
        if self.available_free_bytes <= self.protected_free_bytes { bail!("MANIFEST_PROTECTED_FREE_SPACE_RESERVE") }
        if existing.checked_add(next).context("manifest size overflow")? > self.manifest_quota_bytes { bail!("MANIFEST_OUTPUT_QUOTA_REACHED") }
        Ok(())
    }

    fn permit_identity_count(&self, existing: usize) -> Result<()> {
        if existing >= self.max_in_memory_identities {
            bail!("MANIFEST_MEMORY_CAP_REACHED")
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestCheckpoint {
    pub schema: String,
    pub epoch_id: String,
    pub source_instance_id: String,
    pub continuity_epoch: String,
    pub next_manifest_sequence: String,
    pub next_receive_sequence: String,
    pub tail_record_sha256: String,
    pub checkpoint_sha256: String,
}

impl ManifestCheckpoint {
    fn seal(mut self) -> Result<Self> {
        self.checkpoint_sha256 = hash_excluding_field(&self, "checkpointSha256")?;
        Ok(self)
    }
    fn validate(&self) -> Result<()> {
        if self.schema != FILTERED_IDENTITY_CHECKPOINT_SCHEMA || self.checkpoint_sha256 != hash_excluding_field(self, "checkpointSha256")? {
            bail!("FILTERED_IDENTITY_CHECKPOINT_INVALID")
        }
        Ok(())
    }
}

pub struct FilteredIdentityManifestWriter {
    path: PathBuf,
    checkpoint_path: PathBuf,
    epoch_id: String,
    source_instance_id: String,
    continuity_epoch: String,
    next_manifest_sequence: u64,
    next_receive_sequence: u64,
    tail: String,
    bytes_written: u64,
    identities: BTreeSet<(String, String)>,
    policy: StoragePolicy,
}

impl FilteredIdentityManifestWriter {
    pub fn open(root: &Path, epoch_id: &str, source_instance_id: &str, continuity_epoch: &str, first_manifest_sequence: u64, first_receive_sequence: u64, policy: StoragePolicy) -> Result<Self> {
        if epoch_id.is_empty() || source_instance_id.is_empty() || continuity_epoch.is_empty() { bail!("MANIFEST_BINDING_MISSING") }
        fs::create_dir_all(root)?;
        let path = root.join(format!("{epoch_id}.filtered-identities.jsonl"));
        let checkpoint_path = root.join(format!("{epoch_id}.filtered-identities.checkpoint.json"));
        let mut next_manifest_sequence = first_manifest_sequence;
        let mut next_receive_sequence = first_receive_sequence;
        let mut tail = String::new();
        let mut identities = BTreeSet::new();
        let mut valid_tails = BTreeMap::new();
        valid_tails.insert((next_manifest_sequence, next_receive_sequence), tail.clone());
        if path.exists() {
            for line in BufReader::new(File::open(&path)?).lines() {
                let record: FilteredIdentity = serde_json::from_str(&line?)?;
                record.validate(next_manifest_sequence, &tail)?;
                if record.source_instance_id != source_instance_id || record.continuity_epoch != continuity_epoch { bail!("MANIFEST_EPOCH_OR_SOURCE_CONFLICT") }
                if record.receive_sequence.parse::<u64>()? != next_receive_sequence { bail!("MANIFEST_RECEIVE_SEQUENCE_GAP") }
                policy.permit_identity_count(identities.len())?;
                if !identities.insert((record.slot.clone(), record.signature.clone())) { bail!("MANIFEST_DUPLICATE_IDENTITY") }
                tail = record.record_sha256;
                next_manifest_sequence += 1;
                next_receive_sequence += 1;
                valid_tails.insert((next_manifest_sequence, next_receive_sequence), tail.clone());
            }
        }
        let mut checkpoint_needs_repair = !checkpoint_path.exists();
        if checkpoint_path.exists() {
            let checkpoint: ManifestCheckpoint = serde_json::from_slice(&fs::read(&checkpoint_path)?)?;
            checkpoint.validate()?;
            let checkpoint_manifest_sequence: u64 = checkpoint.next_manifest_sequence.parse()?;
            let checkpoint_receive_sequence: u64 = checkpoint.next_receive_sequence.parse()?;
            let checkpoint_tail = valid_tails.get(&(checkpoint_manifest_sequence, checkpoint_receive_sequence));
            if checkpoint.epoch_id != epoch_id || checkpoint.source_instance_id != source_instance_id || checkpoint.continuity_epoch != continuity_epoch || checkpoint_tail != Some(&checkpoint.tail_record_sha256) {
                bail!("MANIFEST_CHECKPOINT_RECOVERY_CONFLICT")
            }
            checkpoint_needs_repair = checkpoint_manifest_sequence != next_manifest_sequence || checkpoint_receive_sequence != next_receive_sequence || checkpoint.tail_record_sha256 != tail;
        }
        let bytes_written = fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
        let writer = Self { path, checkpoint_path, epoch_id: epoch_id.to_owned(), source_instance_id: source_instance_id.to_owned(), continuity_epoch: continuity_epoch.to_owned(), next_manifest_sequence, next_receive_sequence, tail, bytes_written, identities, policy };
        if checkpoint_needs_repair { writer.persist_checkpoint()?; }
        Ok(writer)
    }

    pub fn append(&mut self, slot: u64, signature: &str, received_at_unix_ns: u128, source_transaction_index: Option<u64>, source_frame_sha256: &str) -> Result<FilteredIdentity> {
        if signature.is_empty() || source_frame_sha256.is_empty() { bail!("MANIFEST_INPUT_INVALID") }
        let identity = (slot.to_string(), signature.to_owned());
        if self.identities.contains(&identity) { bail!("MANIFEST_DUPLICATE_IDENTITY") }
        self.policy.permit_identity_count(self.identities.len())?;
        self.identities.insert(identity.clone());
        let candidate = FilteredIdentity { schema: String::new(), manifest_sequence: self.next_manifest_sequence.to_string(), source_instance_id: self.source_instance_id.clone(), continuity_epoch: self.continuity_epoch.clone(), slot: slot.to_string(), signature: signature.to_owned(), receive_sequence: self.next_receive_sequence.to_string(), received_at_unix_ns: received_at_unix_ns.to_string(), source_transaction_index: source_transaction_index.map(|value| value.to_string()), source_frame_sha256: source_frame_sha256.to_owned(), continuity_status: "CONTINUOUS".to_owned(), previous_record_sha256: String::new(), record_sha256: String::new() }.seal(self.tail.clone())?;
        let mut bytes = jcs_bytes(&serde_json::to_value(&candidate)?)?;
        bytes.push(b'\n');
        if let Err(error) = self.policy.permit(self.bytes_written, bytes.len() as u64) {
            self.identities.remove(&identity);
            return Err(error);
        }
        let mut file = OpenOptions::new().create(true).append(true).open(&self.path)?;
        file.write_all(&bytes)?;
        file.sync_data()?;
        self.bytes_written += bytes.len() as u64;
        self.tail = candidate.record_sha256.clone();
        self.next_manifest_sequence += 1;
        self.next_receive_sequence += 1;
        self.persist_checkpoint()?;
        Ok(candidate)
    }

    /// Durable cursor used by the run checkpoint. The journal itself remains
    /// authoritative; a checkpoint can only resume after this tail is
    /// independently revalidated by `open`.
    pub fn tail_record_sha256(&self) -> &str { &self.tail }
    pub fn next_manifest_sequence(&self) -> u64 { self.next_manifest_sequence }

    fn persist_checkpoint(&self) -> Result<()> {
        let checkpoint = ManifestCheckpoint { schema: FILTERED_IDENTITY_CHECKPOINT_SCHEMA.to_owned(), epoch_id: self.epoch_id.clone(), source_instance_id: self.source_instance_id.clone(), continuity_epoch: self.continuity_epoch.clone(), next_manifest_sequence: self.next_manifest_sequence.to_string(), next_receive_sequence: self.next_receive_sequence.to_string(), tail_record_sha256: self.tail.clone(), checkpoint_sha256: String::new() }.seal()?;
        let temporary = self.checkpoint_path.with_extension("tmp");
        let mut file = File::create(&temporary)?;
        file.write_all(&jcs_bytes(&serde_json::to_value(&checkpoint)?)?)?;
        file.sync_all()?;
        fs::rename(temporary, &self.checkpoint_path)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalizedTransactionInput {
    pub transaction_index: String,
    pub signature: String,
    /// The actual account keys and invoked program IDs of this finalized
    /// transaction. No relevance decision may use the signature itself.
    pub account_or_program_ids: Vec<String>,
    pub supported_version: bool,
}

impl FinalizedTransactionInput {
    /// The single relevance rule used by both block reconciliation and receipt
    /// emission. Signatures and receive order never participate.
    pub fn is_relevant_to(&self, canonical_program_ids: &BTreeSet<String>) -> bool {
        self.account_or_program_ids
            .iter()
            .any(|id| canonical_program_ids.contains(id))
    }

    /// Version support is a block-proof property. Consumers must reject the
    /// exact same unsupported inputs as finalized reconciliation.
    pub fn require_supported_version(&self) -> Result<()> {
        if !self.supported_version {
            bail!("UNSUPPORTED_TRANSACTION_VERSION")
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalizedBlockInput {
    pub slot: String,
    pub block_identity: String,
    pub finalized: bool,
    pub executed_transaction_count: String,
    pub transactions: Vec<FinalizedTransactionInput>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconciliationReceipt {
    pub schema: String,
    pub epoch_id: String,
    pub source_instance_id: String,
    pub continuity_epoch: String,
    pub slot: String,
    pub block_identity: String,
    pub manifest_identity_count: String,
    pub block_relevant_identity_count: String,
    /// Hash of the one frozen relevance policy used for subscription intent,
    /// finalized derivation, receipt emission, and cohort validation.
    pub relevance_policy_sha256: String,
    /// SHA-256 of canonical sorted `(slot, signature, transaction_index)`.
    /// This lets receipt consumers prove they received every expected identity,
    /// rather than merely trusting matching counts.
    pub expected_identity_set_sha256: String,
    /// Exact SHA-256 of the immutable raw finalized `getBlock` response that
    /// was parsed into this proof.  `None` is allowed only for synthetic
    /// fixtures; retained live evidence must bind this field.
    pub raw_provider_response_sha256: Option<String>,
    pub disposition: String,
    pub reason_code: Option<String>,
    pub receipt_sha256: String,
}

pub fn identity_set_digest(identities: &BTreeMap<(String, String), usize>) -> Result<String> {
    let rows = identities.iter().map(|((slot, signature), index)| {
        serde_json::json!({"slot":slot,"signature":signature,"transactionIndex":index.to_string()})
    }).collect::<Vec<_>>();
    Ok(sha256_hex(&jcs_bytes(&serde_json::Value::Array(rows))?))
}

pub fn relevance_policy_digest(canonical_program_ids: &BTreeSet<String>) -> Result<String> {
    let programs = canonical_program_ids.iter().cloned().map(serde_json::Value::String).collect::<Vec<_>>();
    Ok(sha256_hex(&jcs_bytes(&serde_json::json!({"schema":"EDGESNIFFER_CANONICAL_RELEVANCE_POLICY_V1","includeFailed":false,"includeVotes":false,"maxSupportedTransactionVersion":"1","programIds":programs,"policyVersion":"PUMPSWAP_FINALIZED_RELEVANCE_V1"}))?))
}

impl ReconciliationReceipt {
    fn seal(mut self) -> Result<Self> { self.receipt_sha256 = hash_excluding_field(&self, "receiptSha256")?; Ok(self) }
    pub fn bind_raw_provider_response(mut self, raw_sha256: &str) -> Result<Self> {
        if raw_sha256.len() != 64 { bail!("RAW_PROVIDER_RESPONSE_SHA_INVALID") }
        self.raw_provider_response_sha256 = Some(raw_sha256.into());
        self.receipt_sha256.clear();
        self.seal()
    }
}

/// A slot may receive at most one finalized identity.  A different identity
/// for the same slot is a fork/provider conflict and closes the interval.
#[derive(Default)]
pub struct FinalizedBlockRegistry { identities: BTreeMap<String, String> }

impl FinalizedBlockRegistry {
    pub fn insert(&mut self, block: &FinalizedBlockInput) -> Result<()> {
        if !block.finalized || block.slot.is_empty() || block.block_identity.is_empty() { bail!("FINALIZED_BLOCK_PROOF_INVALID") }
        if self.identities.len() >= MAX_PENDING_SLOTS && !self.identities.contains_key(&block.slot) { bail!("FINALIZED_BLOCK_REGISTRY_CAPACITY") }
        if let Some(existing) = self.identities.get(&block.slot) {
            if existing != &block.block_identity { bail!("FINALIZED_BLOCK_FORK_CONFLICT") }
            return Ok(())
        }
        self.identities.insert(block.slot.clone(), block.block_identity.clone());
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct SlotCoverageProof {
    pub slot: String,
    /// `RECONCILED_BLOCK` requires an exact reconciliation receipt. A skipped
    /// slot must be explicitly evidenced as `FINALIZED_SKIPPED`; absence is
    /// never treated as a skipped slot.
    pub status: String,
    pub block_identity: Option<String>,
    pub proof_reference: String,
}

#[derive(Clone, Debug)]
pub struct IntervalCompleteness {
    pub epoch_id: String,
    pub source_instance_id: String,
    pub continuity_epoch: String,
    pub first_capture_slot: String,
    pub last_capture_slot: String,
    pub first_capture_slot_reconciled: bool,
    pub source_restart_or_gap_observed: bool,
    pub block_receipts: Vec<ReconciliationReceipt>,
    /// Complete slot coverage is independent of filtered transaction arrival.
    /// Every slot in the claimed interval must be a reconciled block or an
    /// authoritative finalized-skipped proof.
    pub slot_coverage: Vec<SlotCoverageProof>,
    /// Filtered identities cannot exist in an explicitly skipped slot.
    pub observed_filtered_slots: BTreeSet<String>,
}

impl IntervalCompleteness {
    /// There is no time-based warm-up. The interval starts at its first
    /// explicitly reconciled finalized slot and stays complete only while one
    /// source instance/epoch remains continuous and every collected proof slot
    /// matched exactly. Slots absent from a provider result are failures, not
    /// gaps that a later slot can silently cover.
    pub fn verify(&self) -> Result<()> {
        if self.first_capture_slot.is_empty() || self.last_capture_slot.is_empty() || !self.first_capture_slot_reconciled { bail!("CAPTURE_START_NOT_RECONCILED") }
        if self.source_restart_or_gap_observed { bail!("INTERVAL_CONTINUITY_BREAK") }
        if self.block_receipts.is_empty() { bail!("INTERVAL_NO_BLOCK_PROOFS") }
        let first: u64 = self.first_capture_slot.parse().context("CAPTURE_START_SLOT_INVALID")?;
        let last: u64 = self.last_capture_slot.parse().context("CAPTURE_END_SLOT_INVALID")?;
        if last < first { bail!("CAPTURE_SLOT_RANGE_INVALID") }
        let expected_coverage = last.checked_sub(first).and_then(|value| value.checked_add(1)).context("CAPTURE_SLOT_RANGE_OVERFLOW")? as usize;
        if self.slot_coverage.len() != expected_coverage { bail!("INTERVAL_SLOT_COVERAGE_INCOMPLETE") }
        let mut receipts_by_slot = BTreeMap::new();
        let mut saw_start = false;
        for receipt in &self.block_receipts {
            if receipt.schema != FINALIZED_RECONCILIATION_SCHEMA || receipt.receipt_sha256 != hash_excluding_field(receipt, "receiptSha256")? { bail!("RECONCILIATION_RECEIPT_INVALID") }
            if receipt.epoch_id != self.epoch_id || receipt.source_instance_id != self.source_instance_id || receipt.continuity_epoch != self.continuity_epoch { bail!("INTERVAL_EPOCH_BINDING_MISMATCH") }
            if receipt.disposition != "MATCHED" || receipt.reason_code.is_some() { bail!("INTERVAL_RECONCILIATION_NOT_COMPLETE") }
            if receipts_by_slot.insert(receipt.slot.clone(), receipt).is_some() { bail!("INTERVAL_DUPLICATE_BLOCK_RECEIPT") }
            if receipt.slot == self.first_capture_slot { saw_start = true; }
        }
        if !saw_start { bail!("CAPTURE_START_PROOF_MISSING") }
        for (offset, coverage) in self.slot_coverage.iter().enumerate() {
            let expected_slot = first.checked_add(offset as u64).context("CAPTURE_SLOT_RANGE_OVERFLOW")?.to_string();
            if coverage.slot != expected_slot || coverage.proof_reference.is_empty() { bail!("INTERVAL_SLOT_COVERAGE_INVALID") }
            match coverage.status.as_str() {
                "RECONCILED_BLOCK" => {
                    let receipt = receipts_by_slot.remove(&coverage.slot).context("INTERVAL_BLOCK_RECEIPT_MISSING")?;
                    if coverage.block_identity.as_deref() != Some(receipt.block_identity.as_str()) { bail!("INTERVAL_BLOCK_IDENTITY_MISMATCH") }
                }
                "FINALIZED_SKIPPED" => {
                    if coverage.block_identity.is_some() || self.observed_filtered_slots.contains(&coverage.slot) { bail!("INTERVAL_SKIPPED_SLOT_CONFLICT") }
                }
                _ => bail!("INTERVAL_SLOT_STATUS_UNSUPPORTED"),
            }
        }
        if !receipts_by_slot.is_empty() { bail!("INTERVAL_UNCOVERED_BLOCK_RECEIPT") }
        Ok(())
    }
}

fn validate_manifest_chain(manifest: &[FilteredIdentity], source_instance_id: &str, continuity_epoch: &str) -> Result<()> {
    let mut prior = String::new();
    let mut expected_manifest_sequence = None;
    let mut expected_receive_sequence = None;
    for identity in manifest {
        let manifest_sequence: u64 = identity.manifest_sequence.parse()?;
        let receive_sequence: u64 = identity.receive_sequence.parse()?;
        if let Some(expected) = expected_manifest_sequence {
            if manifest_sequence != expected { bail!("MANIFEST_SEQUENCE_GAP") }
        }
        if let Some(expected) = expected_receive_sequence {
            if receive_sequence != expected { bail!("MANIFEST_RECEIVE_SEQUENCE_GAP") }
        }
        identity.validate(manifest_sequence, &prior)?;
        if identity.source_instance_id != source_instance_id || identity.continuity_epoch != continuity_epoch || identity.continuity_status != "CONTINUOUS" {
            bail!("MANIFEST_CONTINUITY_OR_EPOCH_INVALID")
        }
        prior = identity.record_sha256.clone();
        expected_manifest_sequence = Some(manifest_sequence.checked_add(1).context("manifest sequence overflow")?);
        expected_receive_sequence = Some(receive_sequence.checked_add(1).context("receive sequence overflow")?);
    }
    Ok(())
}

/// Reconcile against the one persisted policy object.  This is intentionally
/// the production entry point: receipt emission and finality reconciliation
/// must share one policy hash rather than independently reproducing a list of
/// program ids and version semantics.
pub fn reconcile_finalized_block_with_policy(epoch_id: &str, source_instance_id: &str, continuity_epoch: &str, block: FinalizedBlockInput, manifest: &[FilteredIdentity], policy: &CanonicalRelevancePolicy) -> Result<ReconciliationReceipt> {
    policy.validate()?;
    if !block.finalized { bail!("BLOCK_NOT_FINALIZED") }
    let expected: usize = block.executed_transaction_count.parse().context("BLOCK_COUNT_INVALID")?;
    if block.transactions.len() != expected { bail!("BLOCK_INVENTORY_INCOMPLETE") }
    if block.transactions.len() > MAX_IDENTITIES_PER_SLOT { bail!("BLOCK_RECONCILIATION_MEMORY_CAP") }
    let mut seen_indexes = BTreeSet::new(); let mut seen_signatures = BTreeSet::new(); let mut block_relevant = BTreeMap::new();
    for transaction in block.transactions {
        let index: usize = transaction.transaction_index.parse().context("BLOCK_TRANSACTION_INDEX_INVALID")?;
        if index >= expected || !seen_indexes.insert(index) || !seen_signatures.insert(transaction.signature.clone()) { bail!("BLOCK_ORDER_OR_IDENTITY_CONFLICT") }
        // Transaction-version support is only a scientific obligation for a
        // transaction matching the frozen Pump/PumpSwap program predicate.
        // An unrelated future-version transaction must not poison a bounded
        // filtered relevance proof.
        if policy.matches_program_ids(&transaction)? {
            transaction.require_supported_version()?;
            if block_relevant.insert((block.slot.clone(), transaction.signature), index).is_some() {
                bail!("BLOCK_RELEVANT_DUPLICATE_IDENTITY")
            }
        }
    }
    if seen_indexes.len() != expected { bail!("BLOCK_INDEX_INVENTORY_INCOMPLETE") }
    validate_manifest_chain(manifest, source_instance_id, continuity_epoch)?;
    // `SubscribeUpdateTransactionInfo.index` remains durable source telemetry,
    // but it is not canonical Solana ordering. Finalized `getBlock` is the
    // sole authority for transaction indexes and causal ordering. Comparing
    // the provider field to a finalized block position caused R4 to reject
    // four exact signature sets without evidence of a source gap.
    let mut observed = BTreeSet::new();
    for identity in manifest.iter().filter(|identity| identity.slot == block.slot) {
        if !observed.insert((identity.slot.clone(), identity.signature.clone())) { bail!("MANIFEST_DUPLICATE_IDENTITY") }
    }
    if observed.len() > MAX_IDENTITIES_PER_SLOT { bail!("MANIFEST_RECONCILIATION_MEMORY_CAP") }
    let observed_identities = observed;
    let block_identities: BTreeSet<_> = block_relevant.keys().cloned().collect();
    let (disposition, reason_code) = if observed_identities != block_identities {
        if observed_identities.is_subset(&block_identities) { ("REJECTED", Some("MISSING_FILTERED_IDENTITY".to_owned())) }
        else if block_identities.is_subset(&observed_identities) { ("REJECTED", Some("EXTRA_FILTERED_IDENTITY".to_owned())) }
        else { ("REJECTED", Some("FILTERED_IDENTITY_SET_CONFLICT".to_owned())) }
    } else {
        ("MATCHED", None)
    };
    let expected_identity_set_sha256 = identity_set_digest(&block_relevant)?;
    let relevance_policy_sha256 = policy.policy_sha256.clone();
    Ok(ReconciliationReceipt { schema: FINALIZED_RECONCILIATION_SCHEMA.to_owned(), epoch_id: epoch_id.to_owned(), source_instance_id: source_instance_id.to_owned(), continuity_epoch: continuity_epoch.to_owned(), slot: block.slot, block_identity: block.block_identity, manifest_identity_count: observed_identities.len().to_string(), block_relevant_identity_count: block_relevant.len().to_string(), relevance_policy_sha256, expected_identity_set_sha256, raw_provider_response_sha256: None, disposition: disposition.to_owned(), reason_code, receipt_sha256: String::new() }.seal()?)
}

/// Compatibility wrapper retained for existing offline callers.  Production
/// capture uses `reconcile_finalized_block_with_policy` above.
pub fn reconcile_finalized_block(epoch_id: &str, source_instance_id: &str, continuity_epoch: &str, block: FinalizedBlockInput, manifest: &[FilteredIdentity], canonical_program_ids: &BTreeSet<String>) -> Result<ReconciliationReceipt> {
    let policy = CanonicalRelevancePolicy::new(
        "PUMPSWAP_FINALIZED_RELEVANCE_V1",
        canonical_program_ids.iter().cloned().collect(),
        1,
    )?;
    reconcile_finalized_block_with_policy(epoch_id, source_instance_id, continuity_epoch, block, manifest, &policy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn policy() -> StoragePolicy { StoragePolicy { manifest_quota_bytes: 64 * 1024, protected_free_bytes: 100, available_free_bytes: 100_000, max_in_memory_identities: 1_024 } }
    fn writer(root: &Path) -> FilteredIdentityManifestWriter { FilteredIdentityManifestWriter::open(root, "epoch-a", "source-a", "7", 0, 0, policy()).unwrap() }
    fn block(slot: u64, relevant: &[&str]) -> FinalizedBlockInput {
        let ids = ["pump", "router", "swap"];
        let mut transactions = Vec::new();
        for index in 0..4 {
            let signature = format!("sig-{slot}-{index}");
            transactions.push(FinalizedTransactionInput { transaction_index: index.to_string(), signature: signature.clone(), account_or_program_ids: if relevant.contains(&signature.as_str()) { vec![ids[0].to_owned()] } else { vec!["other".to_owned()] }, supported_version: true });
        }
        FinalizedBlockInput { slot: slot.to_string(), block_identity: format!("block-{slot}"), finalized: true, executed_transaction_count: "4".into(), transactions }
    }
    fn programs() -> BTreeSet<String> { ["pump".to_owned(), "router".to_owned(), "swap".to_owned()].into_iter().collect() }
    fn all_records(root: &Path) -> Vec<FilteredIdentity> { BufReader::new(File::open(root.join("epoch-a.filtered-identities.jsonl")).unwrap()).lines().map(|line| serde_json::from_str(&line.unwrap()).unwrap()).collect() }

    #[test]
    fn complete_interval_reconciles_when_arrival_is_reverse_of_block_order() {
        let root = tempdir().unwrap(); let mut out = writer(root.path());
        // Receive order is deliberately reversed. Source transaction indexes
        // remain the valid block indexes; neither drives finalized ordering.
        out.append(10, "sig-10-2", 30, Some(2), "raw-2").unwrap();
        out.append(10, "sig-10-0", 31, Some(0), "raw-0").unwrap();
        let receipt = reconcile_finalized_block("epoch-a", "source-a", "7", block(10, &["sig-10-0", "sig-10-2"]), &all_records(root.path()), &programs()).unwrap();
        assert_eq!(receipt.disposition, "MATCHED");
    }

    #[test]
    fn source_transaction_index_mismatch_is_not_canonical_order() {
        let root = tempdir().unwrap(); let mut out = writer(root.path());
        out.append(10, "sig-10-0", 1, Some(3), "raw").unwrap();
        let receipt = reconcile_finalized_block("epoch-a", "source-a", "7", block(10, &["sig-10-0"]), &all_records(root.path()), &programs()).unwrap();
        assert_eq!(receipt.disposition, "MATCHED");
        assert_eq!(receipt.reason_code, None);
    }

    #[test]
    fn missing_source_transaction_index_is_permitted_for_identity_reconciliation() {
        let root = tempdir().unwrap(); let mut out = writer(root.path());
        out.append(10, "sig-10-0", 1, None, "raw").unwrap();
        let receipt = reconcile_finalized_block("epoch-a", "source-a", "7", block(10, &["sig-10-0"]), &all_records(root.path()), &programs()).unwrap();
        assert_eq!(receipt.disposition, "MATCHED");
        assert_eq!(receipt.reason_code, None);
    }

    #[test]
    fn source_index_rename_preserves_legacy_manifest_json_and_hash_validation() {
        let root = tempdir().unwrap(); let mut out = writer(root.path());
        out.append(10, "sig-10-0", 1, Some(0), "raw").unwrap();
        let raw = fs::read_to_string(root.path().join("epoch-a.filtered-identities.jsonl")).unwrap();
        assert!(raw.contains("\"observedArrivalIndex\":\"0\""));
        assert!(!raw.contains("sourceTransactionIndex"));
        let record: FilteredIdentity = serde_json::from_str(raw.trim()).unwrap();
        record.validate(0, "").unwrap();
    }

    #[test]
    fn missing_extra_and_duplicate_identities_fail_closed() {
        let root = tempdir().unwrap(); let mut out = writer(root.path());
        out.append(10, "sig-10-0", 1, Some(0), "raw").unwrap();
        let missing = reconcile_finalized_block("epoch-a", "source-a", "7", block(10, &["sig-10-0", "sig-10-2"]), &all_records(root.path()), &programs()).unwrap();
        assert_eq!(missing.reason_code.as_deref(), Some("MISSING_FILTERED_IDENTITY"));
        assert!(out.append(10, "sig-10-0", 2, None, "raw").unwrap_err().to_string().contains("DUPLICATE"));
        let extra = reconcile_finalized_block("epoch-a", "source-a", "7", block(10, &[]), &all_records(root.path()), &programs()).unwrap();
        assert_eq!(extra.reason_code.as_deref(), Some("EXTRA_FILTERED_IDENTITY"));
    }

    #[test]
    fn gaps_forks_versions_and_capture_storage_fail_closed() {
        let root = tempdir().unwrap(); let mut out = writer(root.path());
        out.append(10, "sig-10-0", 1, Some(0), "raw").unwrap();
        let mut bad = block(10, &["sig-10-0"]); bad.transactions[1].transaction_index = "0".into();
        assert!(reconcile_finalized_block("epoch-a", "source-a", "7", bad, &all_records(root.path()), &programs()).is_err());
        let mut unsupported = block(10, &["sig-10-0"]); unsupported.transactions[0].supported_version = false;
        assert!(reconcile_finalized_block("epoch-a", "source-a", "7", unsupported, &all_records(root.path()), &programs()).unwrap_err().to_string().contains("UNSUPPORTED"));
        let tiny = StoragePolicy { manifest_quota_bytes: 1, protected_free_bytes: 100, available_free_bytes: 100_000, max_in_memory_identities: 1_024 };
        let mut tiny_writer = FilteredIdentityManifestWriter::open(root.path(), "tiny", "source-a", "7", 0, 0, tiny).unwrap();
        assert!(tiny_writer.append(1, "x", 1, None, "raw").unwrap_err().to_string().contains("QUOTA"));

        let memory_limited = StoragePolicy { manifest_quota_bytes: 64 * 1024, protected_free_bytes: 100, available_free_bytes: 100_000, max_in_memory_identities: 1 };
        let mut memory_writer = FilteredIdentityManifestWriter::open(root.path(), "memory", "source-a", "7", 0, 0, memory_limited).unwrap();
        memory_writer.append(1, "first", 1, None, "raw").unwrap();
        assert!(memory_writer.append(2, "second", 2, None, "raw").unwrap_err().to_string().contains("MEMORY_CAP"));
    }

    #[test]
    fn restart_recovers_and_corruption_or_epoch_change_rejects() {
        let root = tempdir().unwrap(); let mut out = writer(root.path());
        out.append(10, "sig-10-0", 1, None, "raw-a").unwrap(); drop(out);
        let mut resumed = writer(root.path()); resumed.append(10, "sig-10-2", 2, None, "raw-b").unwrap();
        assert_eq!(all_records(root.path()).len(), 2);
        assert!(FilteredIdentityManifestWriter::open(root.path(), "epoch-a", "source-b", "7", 0, 0, policy()).is_err());
        let path = root.path().join("epoch-a.filtered-identities.jsonl"); fs::write(&path, b"corrupt\n").unwrap();
        assert!(FilteredIdentityManifestWriter::open(root.path(), "epoch-a", "source-a", "7", 0, 0, policy()).is_err());
    }

    #[test]
    fn stale_checkpoint_after_a_synced_append_recovers_from_verified_journal_tail() {
        let root = tempdir().unwrap(); let mut out = writer(root.path());
        out.append(10, "sig-10-0", 1, None, "raw-a").unwrap();
        let checkpoint_path = root.path().join("epoch-a.filtered-identities.checkpoint.json");
        let stale_checkpoint = fs::read(&checkpoint_path).unwrap();
        out.append(10, "sig-10-2", 2, None, "raw-b").unwrap();
        fs::write(&checkpoint_path, stale_checkpoint).unwrap();
        drop(out);
        let mut resumed = writer(root.path());
        resumed.append(11, "sig-11-0", 3, None, "raw-c").unwrap();
        assert_eq!(all_records(root.path()).len(), 3);
    }

    #[test]
    fn interval_proof_requires_reconciled_start_and_unbroken_epoch() {
        let root = tempdir().unwrap(); let mut out = writer(root.path());
        out.append(10, "sig-10-0", 1, Some(0), "raw").unwrap();
        let receipt = reconcile_finalized_block("epoch-a", "source-a", "7", block(10, &["sig-10-0"]), &all_records(root.path()), &programs()).unwrap();
        let complete = IntervalCompleteness {
            epoch_id: "epoch-a".into(), source_instance_id: "source-a".into(), continuity_epoch: "7".into(),
            first_capture_slot: "10".into(), last_capture_slot: "10".into(), first_capture_slot_reconciled: true,
            source_restart_or_gap_observed: false, block_receipts: vec![receipt.clone()],
            slot_coverage: vec![SlotCoverageProof { slot: "10".into(), status: "RECONCILED_BLOCK".into(), block_identity: Some(receipt.block_identity.clone()), proof_reference: "finalized-getBlock:10".into() }],
            observed_filtered_slots: ["10".to_owned()].into_iter().collect(),
        };
        complete.verify().unwrap();
        let broken = IntervalCompleteness { source_restart_or_gap_observed: true, ..complete.clone() };
        assert!(broken.verify().unwrap_err().to_string().contains("CONTINUITY"));
        let no_start = IntervalCompleteness { first_capture_slot_reconciled: false, ..complete };
        assert!(no_start.verify().unwrap_err().to_string().contains("CAPTURE_START"));
    }

    #[test]
    fn interval_cannot_claim_coverage_from_a_partial_receipt_list() {
        let root = tempdir().unwrap(); let mut out = writer(root.path());
        out.append(10, "sig-10-0", 1, Some(0), "raw").unwrap();
        let receipt = reconcile_finalized_block("epoch-a", "source-a", "7", block(10, &["sig-10-0"]), &all_records(root.path()), &programs()).unwrap();
        let incomplete = IntervalCompleteness {
            epoch_id: "epoch-a".into(), source_instance_id: "source-a".into(), continuity_epoch: "7".into(),
            first_capture_slot: "10".into(), last_capture_slot: "11".into(), first_capture_slot_reconciled: true,
            source_restart_or_gap_observed: false, block_receipts: vec![receipt],
            slot_coverage: vec![SlotCoverageProof { slot: "10".into(), status: "RECONCILED_BLOCK".into(), block_identity: Some("block-10".into()), proof_reference: "finalized-getBlock:10".into() }],
            observed_filtered_slots: ["10".to_owned()].into_iter().collect(),
        };
        assert!(incomplete.verify().unwrap_err().to_string().contains("COVERAGE_INCOMPLETE"));
    }

    #[test]
    fn finalized_skipped_slot_requires_authoritative_status_and_no_identity() {
        let root = tempdir().unwrap(); let mut out = writer(root.path());
        out.append(10, "sig-10-0", 1, Some(0), "raw").unwrap();
        let receipt = reconcile_finalized_block("epoch-a", "source-a", "7", block(10, &["sig-10-0"]), &all_records(root.path()), &programs()).unwrap();
        let valid = IntervalCompleteness {
            epoch_id: "epoch-a".into(), source_instance_id: "source-a".into(), continuity_epoch: "7".into(),
            first_capture_slot: "10".into(), last_capture_slot: "11".into(), first_capture_slot_reconciled: true,
            source_restart_or_gap_observed: false, block_receipts: vec![receipt.clone()],
            slot_coverage: vec![
                SlotCoverageProof { slot: "10".into(), status: "RECONCILED_BLOCK".into(), block_identity: Some(receipt.block_identity), proof_reference: "finalized-getBlock:10".into() },
                SlotCoverageProof { slot: "11".into(), status: "FINALIZED_SKIPPED".into(), block_identity: None, proof_reference: "finalized-slot-status:11".into() },
            ],
            observed_filtered_slots: ["10".to_owned()].into_iter().collect(),
        };
        valid.verify().unwrap();
        let invalid = IntervalCompleteness { observed_filtered_slots: ["10".to_owned(), "11".to_owned()].into_iter().collect(), ..valid };
        assert!(invalid.verify().unwrap_err().to_string().contains("SKIPPED_SLOT_CONFLICT"));
    }

    #[test]
    fn block_registry_and_rpc_budget_fail_closed() {
        let mut registry = FinalizedBlockRegistry::default();
        let known = block(10, &[]); registry.insert(&known).unwrap(); registry.insert(&known).unwrap();
        let mut fork = known.clone(); fork.block_identity = "different".into();
        assert!(registry.insert(&fork).unwrap_err().to_string().contains("FORK"));
        let mut budget = RpcBudget::new(2, 1, 100).unwrap(); budget.consume_one().unwrap(); budget.consume_one().unwrap();
        assert_eq!(budget.used_requests(), 2); assert!(budget.consume_one().unwrap_err().to_string().contains("BUDGET"));
        assert!(RpcBudget::new(1, 0, 1).is_err());
    }

    #[test]
    fn malformed_manifest_gap_blocks_reconciliation() {
        let root = tempdir().unwrap(); let mut out = writer(root.path());
        out.append(10, "sig-10-0", 1, None, "raw-a").unwrap();
        out.append(10, "sig-10-2", 2, None, "raw-b").unwrap();
        let mut records = all_records(root.path()); records[1].receive_sequence = "9".into();
        assert!(reconcile_finalized_block("epoch-a", "source-a", "7", block(10, &["sig-10-0", "sig-10-2"]), &records, &programs()).unwrap_err().to_string().contains("RECEIVE_SEQUENCE"));
    }

    #[test]
    fn unavailable_or_nonfinalized_block_cannot_close_an_interval() {
        let root = tempdir().unwrap(); let mut out = writer(root.path());
        out.append(10, "sig-10-0", 1, None, "raw").unwrap();
        let mut unfinalized = block(10, &["sig-10-0"]); unfinalized.finalized = false;
        assert!(reconcile_finalized_block("epoch-a", "source-a", "7", unfinalized, &all_records(root.path()), &programs()).unwrap_err().to_string().contains("NOT_FINALIZED"));
    }
}
