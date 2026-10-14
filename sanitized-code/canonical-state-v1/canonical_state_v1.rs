//! Offline-only Canonical State v1 primitives.
//!
//! This module has no provider connection and is not wired into a service.
//! Its invariants deliberately fail closed: arrival order is never scientific
//! order, an unproved candidate universe cannot admit a decision, and a torn
//! journal segment is recovered only when its full sequence/hash prefix proves
//! valid.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const CANONICAL_SCHEMA: &str = "EDGESNIFFER_CANONICAL_SEMANTIC_EVENT_V1";
pub const COVERAGE_SCHEMA: &str = "EDGESNIFFER_SCIENTIFIC_COVERAGE_LEDGER_V1";
pub const SEGMENT_SCHEMA: &str = "EDGESNIFFER_CANONICAL_SEGMENT_V1";
pub const CHECKPOINT_SCHEMA: &str = "EDGESNIFFER_CANONICAL_CHECKPOINT_V1";

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The Canonical State schemas intentionally use strings for quantities and
/// identifiers.  This restricted JCS implementation therefore has no JSON
/// number rendering ambiguity while retaining RFC 8785 ordering/UTF-8 rules.
pub fn jcs_bytes(value: &Value) -> Result<Vec<u8>> {
    fn emit(value: &Value, output: &mut Vec<u8>) -> Result<()> {
        match value {
            Value::Null => output.extend_from_slice(b"null"),
            Value::Bool(true) => output.extend_from_slice(b"true"),
            Value::Bool(false) => output.extend_from_slice(b"false"),
            Value::String(text) => output.extend_from_slice(&serde_json::to_vec(text)?),
            Value::Number(_) => bail!("canonical schemas forbid JSON numbers; use strings for exact quantities"),
            Value::Array(values) => {
                output.push(b'[');
                for (index, value) in values.iter().enumerate() {
                    if index != 0 { output.push(b','); }
                    emit(value, output)?;
                }
                output.push(b']');
            }
            Value::Object(map) => {
                let mut keys = map.keys().collect::<Vec<_>>();
                // All frozen v1 schema keys are ASCII, where UTF-8 lexical and
                // JCS Unicode code-point member order are identical.
                if keys.iter().any(|key| !key.is_ascii()) {
                    bail!("canonical schema object key must be ASCII")
                }
                keys.sort_unstable();
                output.push(b'{');
                for (index, key) in keys.iter().enumerate() {
                    if index != 0 { output.push(b','); }
                    output.extend_from_slice(&serde_json::to_vec(key)?);
                    output.push(b':');
                    emit(&map[*key], output)?;
                }
                output.push(b'}');
            }
        }
        Ok(())
    }

    let mut output = Vec::new();
    emit(value, &mut output)?;
    Ok(output)
}

/// SHA-256(JCS(semantic object with the named self-hash member removed)).
/// A missing self-hash key is an error: the convention is schema-visible and
/// never represented by an empty string.
pub fn hash_excluding_field<T: Serialize>(value: &T, field: &str) -> Result<String> {
    let mut object = serde_json::to_value(value)?;
    let map = object.as_object_mut().context("canonical hash requires object")?;
    if map.remove(field).is_none() {
        bail!("canonical hash field {field} was absent")
    }
    Ok(sha256_hex(&jcs_bytes(&object)?))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct CausalOrder {
    pub slot: String,
    pub transaction_index: Option<String>,
    pub instruction_path: Option<String>,
    pub semantic_ordinal: Option<String>,
}

impl CausalOrder {
    fn parse_nonnegative(label: &str, value: &Option<String>) -> Result<u64> {
        let text = value.as_deref().with_context(|| label.to_owned())?;
        if text.is_empty() || text.starts_with('+') || text.starts_with('-') || !text.bytes().all(|byte| byte.is_ascii_digit()) {
            bail!("invalid {label}")
        }
        Ok(text.parse()?)
    }

    fn parse_path(&self) -> Result<Vec<u64>> {
        let text = self.instruction_path.as_deref().context("instruction_path")?;
        let pieces = text.split('/').collect::<Vec<_>>();
        if pieces.is_empty() || pieces.iter().any(|piece| piece.is_empty() || !piece.bytes().all(|byte| byte.is_ascii_digit())) {
            bail!("invalid instruction_path")
        }
        pieces.into_iter().map(str::parse::<u64>).collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn resolved_key(&self) -> Result<(u64, u64, Vec<u64>, u64)> {
        let slot = Self::parse_nonnegative("slot", &Some(self.slot.clone()))?;
        Ok((slot, Self::parse_nonnegative("transaction_index", &self.transaction_index)?, self.parse_path()?, Self::parse_nonnegative("semantic_ordinal", &self.semantic_ordinal)?))
    }

    pub fn resolved(&self) -> bool { self.resolved_key().is_ok() }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SemanticEvent {
    pub schema: String,
    pub canonical_sequence: String,
    pub source_instance_id: String,
    pub epoch_id: String,
    pub event_identity: String,
    pub causal_order: CausalOrder,
    pub causal_order_status: String,
    pub kind: String,
    pub signature: String,
    pub pool: Option<String>,
    pub mint: Option<String>,
    pub actor: Option<String>,
    pub side: Option<String>,
    pub pool_base_raw: Option<String>,
    pub pool_quote_raw: Option<String>,
    pub quote_delta_raw: Option<String>,
    pub fee_tuple: Option<[String; 5]>,
    pub observed_at_unix_ns: String,
    pub parser_version: String,
    pub raw_frame_sha256: Option<String>,
    pub previous_record_sha256: String,
    pub record_sha256: String,
}

impl SemanticEvent {
    pub fn seal(mut self, previous: String) -> Result<Self> {
        self.schema = CANONICAL_SCHEMA.to_owned();
        self.previous_record_sha256 = previous;
        self.record_sha256 = hash_excluding_field(&self, "record_sha256")?;
        Ok(self)
    }

    pub fn validate(&self, expected_sequence: u64, previous: &str) -> Result<()> {
        if self.schema != CANONICAL_SCHEMA { bail!("canonical schema mismatch") }
        if self.canonical_sequence.parse::<u64>()? != expected_sequence { bail!("canonical sequence gap") }
        if self.previous_record_sha256 != previous { bail!("canonical hash-chain break") }
        if self.record_sha256 != hash_excluding_field(self, "record_sha256")? { bail!("canonical record hash mismatch") }
        if self.causal_order_status == "RESOLVED" && !self.causal_order.resolved() {
            bail!("RESOLVED event lacks a valid causal order")
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FailureReceipt {
    pub schema: String,
    pub epoch_id: String,
    pub last_sequence: String,
    pub last_event_hash: String,
    pub reason: String,
    pub receipt_sha256: String,
}

impl FailureReceipt {
    pub fn seal(mut self) -> Result<Self> {
        self.receipt_sha256 = hash_excluding_field(&self, "receipt_sha256")?;
        Ok(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoverageRecord {
    pub schema: String,
    pub sequence: String,
    pub epoch_id: String,
    pub kind: String,
    pub start_sequence: String,
    pub end_sequence: String,
    pub reason_code: Option<String>,
    pub details_hash: String,
    pub previous_record_sha256: String,
    pub record_sha256: String,
}

impl CoverageRecord {
    fn range(&self) -> Result<(u64, u64)> { Ok((self.start_sequence.parse()?, self.end_sequence.parse()?)) }
    fn is_interval(&self) -> bool { !matches!(self.kind.as_str(), "ACCEPTED_DECISION" | "EXCLUDED_DECISION" | "UNIVERSE_COMPLETE") }
    fn seal(mut self, previous: String) -> Result<Self> {
        self.schema = COVERAGE_SCHEMA.to_owned();
        self.previous_record_sha256 = previous;
        self.record_sha256 = hash_excluding_field(&self, "record_sha256")?;
        Ok(self)
    }
}

#[derive(Debug, Default)]
pub struct CoverageLedger {
    pub records: Vec<CoverageRecord>,
    tail: String,
    next: u64,
    universe_proof_by_epoch: BTreeMap<String, String>,
}

impl CoverageLedger {
    pub fn append(&mut self, epoch: &str, kind: &str, start: u64, end: u64, reason: Option<&str>, details_hash: &str) -> Result<()> {
        if end < start { bail!("coverage interval reversed") }
        if details_hash.is_empty() { bail!("coverage details hash required") }
        let allowed = matches!(kind, "VALID_INTERVAL" | "BLACKOUT" | "RECOVERY" | "WARMUP" | "UNIVERSE_INCOMPLETE" | "UNIVERSE_COMPLETE" | "ACCEPTED_DECISION" | "EXCLUDED_DECISION");
        if !allowed { bail!("unknown coverage kind") }
        if kind == "UNIVERSE_COMPLETE" && details_hash == "proof" { bail!("universe completion requires an immutable proof hash") }
        if kind == "UNIVERSE_COMPLETE" { self.universe_proof_by_epoch.insert(epoch.to_owned(), details_hash.to_owned()); }
        if kind == "UNIVERSE_INCOMPLETE" { self.universe_proof_by_epoch.remove(epoch); }

        let candidate = CoverageRecord {
            schema: String::new(), sequence: self.next.to_string(), epoch_id: epoch.to_owned(), kind: kind.to_owned(),
            start_sequence: start.to_string(), end_sequence: end.to_string(), reason_code: reason.map(str::to_owned),
            details_hash: details_hash.to_owned(), previous_record_sha256: String::new(), record_sha256: String::new(),
        };
        if candidate.is_interval() {
            for prior in self.records.iter().filter(|record| record.epoch_id == epoch && record.is_interval()) {
                let (left, right) = prior.range()?;
                if start <= right && left <= end { bail!("ambiguous overlapping coverage intervals") }
            }
        }
        let record = candidate.seal(self.tail.clone())?;
        self.tail = record.record_sha256.clone();
        self.next += 1;
        self.records.push(record);
        Ok(())
    }

    pub fn universe_complete_at(&self, epoch: &str, sequence: u64) -> bool {
        let mut complete = false;
        for record in self.records.iter().filter(|record| record.epoch_id == epoch) {
            let Ok((start, end)) = record.range() else { return false; };
            if start > sequence { continue; }
            if matches!(record.kind.as_str(), "BLACKOUT" | "RECOVERY" | "WARMUP" | "UNIVERSE_INCOMPLETE") && sequence <= end { return false; }
            if record.kind == "UNIVERSE_COMPLETE" && start <= sequence && self.universe_proof_by_epoch.get(epoch) == Some(&record.details_hash) { complete = true; }
        }
        complete
    }
}

#[derive(Debug, Clone, Default)]
pub struct PoolState {
    pub first_seen_sequence: u64,
    pub base: String,
    pub quote: String,
    pub prior_quote: Option<String>,
    pub last_buy_sequence: Option<u64>,
    pub flows: VecDeque<(u64, String, String)>,
}

#[derive(Debug, Default)]
pub struct CanonicalState {
    pub pools: BTreeMap<String, PoolState>,
    pub seen: BTreeSet<String>,
    pub last_sequence: Option<u64>,
    pub epoch: String,
    pub coverage: CoverageLedger,
}

impl CanonicalState {
    pub fn apply(&mut self, event: &SemanticEvent) -> Result<()> {
        let sequence = event.canonical_sequence.parse::<u64>()?;
        if let Some(last) = self.last_sequence {
            if sequence != last + 1 { bail!("state sequence gap") }
        }
        if !self.seen.insert(event.event_identity.clone()) { bail!("duplicate semantic identity") }
        if event.kind == "PUMPSWAP_SWAP" && event.causal_order_status != "RESOLVED" { bail!("ORDER_AMBIGUOUS") }
        if let Some(pool) = &event.pool {
            let state = self.pools.entry(pool.clone()).or_default();
            if state.first_seen_sequence == 0 { state.first_seen_sequence = sequence; }
            if let (Some(base), Some(quote)) = (&event.pool_base_raw, &event.pool_quote_raw) {
                state.prior_quote = Some(state.quote.clone()).filter(|existing| !existing.is_empty());
                state.base = base.clone(); state.quote = quote.clone();
            }
            if event.side.as_deref() == Some("BUY") { state.last_buy_sequence = Some(sequence); }
            if let (Some(side), Some(flow)) = (&event.side, &event.quote_delta_raw) {
                state.flows.push_back((sequence, side.clone(), flow.clone()));
            }
        }
        self.last_sequence = Some(sequence);
        Ok(())
    }

    pub fn admit_decision(&self, event: &SemanticEvent) -> Result<()> {
        let sequence = event.canonical_sequence.parse()?;
        if !self.coverage.universe_complete_at(&event.epoch_id, sequence) { bail!("UNIVERSE_INCOMPLETE") }
        if event.causal_order_status != "RESOLVED" { bail!("ORDER_AMBIGUOUS") }
        Ok(())
    }
}

/// Buffers a closed Solana slot and emits its records in approved causal order.
/// An arrival is never applied to state until the caller has positively closed
/// the slot boundary (or has an equivalent finalized transaction inventory).
#[derive(Debug, Default)]
pub struct CausalOrderResolver { pending: BTreeMap<u64, Vec<SemanticEvent>> }

impl CausalOrderResolver {
    pub fn push(&mut self, event: SemanticEvent) -> Result<()> {
        let (slot, _, _, _) = event.causal_order.resolved_key().map_err(|_| anyhow::anyhow!("ORDER_AMBIGUOUS"))?;
        if event.causal_order_status != "RESOLVED" { bail!("ORDER_AMBIGUOUS") }
        self.pending.entry(slot).or_default().push(event);
        Ok(())
    }

    pub fn flush_closed_slot(&mut self, slot: u64) -> Result<Vec<SemanticEvent>> {
        let mut records = self.pending.remove(&slot).unwrap_or_default();
        records.sort_by_key(|event| event.causal_order.resolved_key().expect("validated causal order"));
        let mut seen = BTreeSet::new();
        for record in &records {
            let key = record.causal_order.resolved_key()?;
            if !seen.insert(key) { bail!("ORDER_AMBIGUOUS duplicate causal position") }
        }
        Ok(records)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SegmentManifest {
    schema: String,
    epoch_id: String,
    first_sequence: String,
    last_sequence: String,
    record_count: String,
    canonical_uncompressed_sha256: String,
    tail_record_sha256: String,
    manifest_sha256: String,
}

impl SegmentManifest {
    fn seal(mut self) -> Result<Self> { self.manifest_sha256 = hash_excluding_field(&self, "manifest_sha256")?; Ok(self) }
    fn validate(&self) -> Result<()> {
        if self.schema != SEGMENT_SCHEMA || self.manifest_sha256 != hash_excluding_field(self, "manifest_sha256")? { bail!("invalid segment manifest") }
        Ok(())
    }
}

#[derive(Debug)]
struct JournalRecovery { next: u64, tail: String, open_path: Option<PathBuf>, open_count: u64 }

fn journal_paths(root: &Path, epoch: &str, extension: &str) -> Result<Vec<PathBuf>> {
    let marker = format!("epoch-{epoch}-");
    let mut paths = fs::read_dir(root)?.filter_map(|entry| entry.ok()).map(|entry| entry.path())
        .filter(|path| path.file_name().and_then(|name| name.to_str()).map(|name| name.starts_with(&marker) && name.ends_with(extension)).unwrap_or(false)).collect::<Vec<_>>();
    paths.sort(); Ok(paths)
}

fn scan_records(path: &Path, expected: &mut Option<u64>, tail: &mut String) -> Result<Vec<SemanticEvent>> {
    let mut records = Vec::new();
    for line in BufReader::new(File::open(path)?).lines() {
        let line = line?;
        if line.is_empty() { continue; }
        let event: SemanticEvent = serde_json::from_str(&line).with_context(|| format!("decoding {}", path.display()))?;
        let sequence = event.canonical_sequence.parse::<u64>()?;
        let expected_sequence = expected.unwrap_or(sequence);
        event.validate(expected_sequence, tail)?;
        *expected = Some(expected_sequence + 1);
        *tail = event.record_sha256.clone();
        records.push(event);
    }
    Ok(records)
}

fn recover_journal(root: &Path, epoch: &str, requested_first: u64) -> Result<JournalRecovery> {
    let mut expected = None;
    let mut tail = String::new();
    for path in journal_paths(root, epoch, ".jsonl")? { let _ = scan_records(&path, &mut expected, &mut tail)?; }
    let opens = journal_paths(root, epoch, ".jsonl.open")?;
    if opens.len() > 1 { bail!("multiple open segments for one epoch") }
    let (open_path, open_count) = if let Some(path) = opens.into_iter().next() {
        let before = expected;
        let records = scan_records(&path, &mut expected, &mut tail)?;
        if before.is_none() && records.is_empty() { bail!("empty open segment cannot establish first sequence") }
        (Some(path), records.len() as u64)
    } else { (None, 0) };
    let first = if let Some(first_record) = journal_paths(root, epoch, ".jsonl")?.into_iter().next().or_else(|| open_path.clone()) {
        let line = BufReader::new(File::open(first_record)?).lines().next().transpose()?.context("journal segment unexpectedly empty")?;
        Some(serde_json::from_str::<SemanticEvent>(&line)?.canonical_sequence.parse::<u64>()?)
    } else { None };
    if let Some(actual_first) = first { if actual_first != requested_first { bail!("journal first sequence mismatch") } }
    Ok(JournalRecovery { next: expected.unwrap_or(requested_first), tail, open_path, open_count })
}

pub struct JournalWriter {
    root: PathBuf, epoch: String, next: u64, tail: String, current: Option<File>, current_path: Option<PathBuf>, max_records: u64, count: u64,
}

impl JournalWriter {
    pub fn open(root: &Path, epoch: &str, first_sequence: u64, max_records: u64) -> Result<Self> {
        if max_records == 0 { bail!("max_records must be positive") }
        fs::create_dir_all(root)?;
        let recovery = recover_journal(root, epoch, first_sequence)?;
        let mut writer = Self { root: root.to_owned(), epoch: epoch.to_owned(), next: recovery.next, tail: recovery.tail, current: None, current_path: recovery.open_path, max_records, count: recovery.open_count };
        if let Some(path) = writer.current_path.clone() { writer.current = Some(OpenOptions::new().append(true).open(path)?); }
        Ok(writer)
    }

    fn ensure_segment(&mut self) -> Result<()> {
        if self.current.is_none() {
            let path = self.root.join(format!("epoch-{}-{:020}.jsonl.open", self.epoch, self.next));
            self.current = Some(OpenOptions::new().create_new(true).write(true).open(&path)?);
            self.current_path = Some(path); self.count = 0;
        }
        Ok(())
    }

    pub fn append(&mut self, event: SemanticEvent) -> Result<SemanticEvent> {
        self.ensure_segment()?;
        let sealed = event.seal(self.tail.clone())?;
        sealed.validate(self.next, &self.tail)?;
        let bytes = jcs_bytes(&serde_json::to_value(&sealed)?)?;
        let file = self.current.as_mut().context("open segment")?;
        file.write_all(&bytes)?; file.write_all(b"\n")?; file.sync_data()?;
        self.tail = sealed.record_sha256.clone(); self.next += 1; self.count += 1;
        if self.count >= self.max_records { self.seal_segment()?; }
        Ok(sealed)
    }

    pub fn seal_segment(&mut self) -> Result<()> {
        if let (Some(file), Some(path)) = (self.current.take(), self.current_path.take()) {
            file.sync_all()?; drop(file);
            let bytes = fs::read(&path)?;
            let first = self.next.checked_sub(self.count).context("segment first sequence underflow")?;
            let manifest = SegmentManifest { schema: SEGMENT_SCHEMA.to_owned(), epoch_id: self.epoch.clone(), first_sequence: first.to_string(), last_sequence: (self.next - 1).to_string(), record_count: self.count.to_string(), canonical_uncompressed_sha256: sha256_hex(&bytes), tail_record_sha256: self.tail.clone(), manifest_sha256: String::new() }.seal()?;
            let manifest_path = path.with_extension("manifest.json.tmp");
            fs::write(&manifest_path, jcs_bytes(&serde_json::to_value(&manifest)?)?)?;
            fs::rename(&manifest_path, path.with_extension("manifest.json"))?;
            fs::rename(&path, path.with_extension("jsonl"))?;
        }
        Ok(())
    }
}

pub fn scan_journal(root: &Path, epoch: &str) -> Result<Vec<SemanticEvent>> {
    let mut expected = None;
    let mut tail = String::new();
    let mut records = Vec::new();
    for path in journal_paths(root, epoch, ".jsonl")? {
        let manifest: SegmentManifest = serde_json::from_slice(&fs::read(path.with_extension("manifest.json"))?)?;
        manifest.validate()?;
        let bytes = fs::read(&path)?;
        if manifest.canonical_uncompressed_sha256 != sha256_hex(&bytes) { bail!("sealed segment byte hash mismatch") }
        let segment = scan_records(&path, &mut expected, &mut tail)?;
        if manifest.record_count.parse::<usize>()? != segment.len() || manifest.tail_record_sha256 != tail { bail!("sealed segment manifest mismatch") }
        records.extend(segment);
    }
    for path in journal_paths(root, epoch, ".jsonl.open")? { records.extend(scan_records(&path, &mut expected, &mut tail)?); }
    Ok(records)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub schema: String, pub epoch_id: String, pub last_sequence: String, pub last_record_sha256: String, pub coverage_tail_sha256: String, pub checkpoint_sha256: String,
}

impl Checkpoint {
    pub fn seal(mut self) -> Result<Self> { self.schema = CHECKPOINT_SCHEMA.to_owned(); self.checkpoint_sha256 = hash_excluding_field(&self, "checkpoint_sha256")?; Ok(self) }
    pub fn validate(&self) -> Result<()> { if self.schema != CHECKPOINT_SCHEMA || self.checkpoint_sha256 != hash_excluding_field(self, "checkpoint_sha256")? { bail!("checkpoint integrity failure") } Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn event(sequence: u64, tx: u64, path: &str) -> SemanticEvent {
        SemanticEvent { schema: String::new(), canonical_sequence: sequence.to_string(), source_instance_id: "source".into(), epoch_id: "epoch".into(), event_identity: format!("id-{sequence}"), causal_order: CausalOrder { slot: "9".into(), transaction_index: Some(tx.to_string()), instruction_path: Some(path.into()), semantic_ordinal: Some("0".into()) }, causal_order_status: "RESOLVED".into(), kind: "PUMPSWAP_SWAP".into(), signature: format!("sig-{sequence}"), pool: Some("pool".into()), mint: Some("mint".into()), actor: None, side: Some("BUY".into()), pool_base_raw: Some("1".into()), pool_quote_raw: Some("2".into()), quote_delta_raw: Some("1".into()), fee_tuple: None, observed_at_unix_ns: "1".into(), parser_version: "v1".into(), raw_frame_sha256: None, previous_record_sha256: String::new(), record_sha256: String::new() }
    }

    #[test]
    fn jcs_hash_vectors_exclude_self_hash() -> Result<()> {
        let a = serde_json::json!({"b":"2","a":"1"});
        assert_eq!(jcs_bytes(&a)?, br#"{"a":"1","b":"2"}"#);
        assert!(jcs_bytes(&serde_json::json!({"n": 1})).is_err());
        let sealed = event(7, 2, "1/0").seal("tail".into())?;
        assert_eq!(sealed.record_sha256, hash_excluding_field(&sealed, "record_sha256")?);
        assert!(!sealed.record_sha256.is_empty());
        Ok(())
    }

    #[test]
    fn causal_order_ignores_arrival_order() -> Result<()> {
        let mut resolver = CausalOrderResolver::default();
        resolver.push(event(2, 2, "0/0"))?;
        resolver.push(event(1, 1, "0/0"))?;
        let ordered = resolver.flush_closed_slot(9)?;
        assert_eq!(ordered.iter().map(|item| item.causal_order.transaction_index.as_deref()).collect::<Vec<_>>(), vec![Some("1"), Some("2")]);
        Ok(())
    }

    #[test]
    fn resolved_requires_all_valid_fields() {
        let mut invalid = event(1, 1, "0/x"); invalid.causal_order_status = "RESOLVED".into();
        assert!(invalid.seal(String::new()).unwrap().validate(1, "").is_err());
    }

    #[test]
    fn journal_restart_and_open_segment_recovery_is_contiguous() -> Result<()> {
        let directory = tempdir()?;
        let mut first = JournalWriter::open(directory.path(), "epoch", 40, 10)?;
        let a = first.append(event(40, 1, "0/0"))?;
        let _b = first.append(event(41, 2, "0/0"))?;
        drop(first); // Simulated process interruption with a valid .open segment.
        let mut resumed = JournalWriter::open(directory.path(), "epoch", 40, 10)?;
        let c = resumed.append(event(42, 3, "0/0"))?;
        resumed.seal_segment()?;
        let all = scan_journal(directory.path(), "epoch")?;
        assert_eq!(all.iter().map(|item| item.canonical_sequence.as_str()).collect::<Vec<_>>(), vec!["40", "41", "42"]);
        assert_eq!(all[0].record_sha256, a.record_sha256);
        assert_eq!(all[2].record_sha256, c.record_sha256);
        assert!(JournalWriter::open(directory.path(), "epoch", 1, 10).is_err());
        Ok(())
    }

    #[test]
    fn coverage_requires_proof_and_nonoverlap() -> Result<()> {
        let mut ledger = CoverageLedger::default();
        ledger.append("epoch", "WARMUP", 1, 3, Some("RESTART"), "warmup-hash")?;
        assert!(ledger.append("epoch", "VALID_INTERVAL", 3, 6, None, "valid-hash").is_err());
        assert!(ledger.append("epoch", "UNIVERSE_COMPLETE", 4, 4, None, "proof").is_err());
        ledger.append("epoch", "UNIVERSE_COMPLETE", 4, 4, None, "abc123")?;
        assert!(ledger.universe_complete_at("epoch", 4));
        ledger.append("epoch", "BLACKOUT", 5, 8, Some("SOURCE_GAP"), "gap-hash")?;
        assert!(!ledger.universe_complete_at("epoch", 6));
        Ok(())
    }

    #[test]
    fn state_refuses_ambiguous_and_incomplete_universe() -> Result<()> {
        let mut state = CanonicalState::default();
        let first = event(1, 1, "0/0").seal(String::new())?;
        state.apply(&first)?;
        assert!(state.admit_decision(&first).is_err());
        state.coverage.append("epoch", "UNIVERSE_COMPLETE", 1, 1, None, "proof-hash")?;
        assert!(state.admit_decision(&first).is_ok());
        let mut ambiguous = event(2, 2, "0/0"); ambiguous.causal_order_status = "ORDER_AMBIGUOUS".into();
        assert!(state.apply(&ambiguous.seal(first.record_sha256)?).is_err());
        Ok(())
    }

    #[test]
    fn scanner_rejects_corrupt_segment_and_checkpoint() -> Result<()> {
        let directory = tempdir()?;
        let mut writer = JournalWriter::open(directory.path(), "epoch", 1, 1)?;
        writer.append(event(1, 1, "0/0"))?;
        let segment = journal_paths(directory.path(), "epoch", ".jsonl")?.pop().context("sealed segment")?;
        fs::write(&segment, b"{bad}\n")?;
        assert!(scan_journal(directory.path(), "epoch").is_err());
        let checkpoint = Checkpoint { schema: String::new(), epoch_id: "epoch".into(), last_sequence: "1".into(), last_record_sha256: "hash".into(), coverage_tail_sha256: "coverage".into(), checkpoint_sha256: String::new() }.seal()?;
        checkpoint.validate()?;
        let mut corrupt = checkpoint.clone(); corrupt.last_sequence = "2".into();
        assert!(corrupt.validate().is_err());
        Ok(())
    }
}
