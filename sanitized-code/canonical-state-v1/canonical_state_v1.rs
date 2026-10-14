//! Offline-only canonical semantic-state primitives.  Nothing in this module
//! opens a provider connection or is wired into a systemd service.

use std::{collections::{BTreeMap, BTreeSet, VecDeque}, fs::{self, File, OpenOptions}, io::{BufRead, BufReader, Write}, path::{Path, PathBuf}};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const CANONICAL_SCHEMA: &str = "EDGESNIFFER_CANONICAL_SEMANTIC_EVENT_V1";
pub const COVERAGE_SCHEMA: &str = "EDGESNIFFER_SCIENTIFIC_COVERAGE_LEDGER_V1";

pub fn sha256_hex(bytes: &[u8]) -> String { Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect() }

/// RFC 8785/JCS-compatible subset used by this schema: values are objects,
/// arrays, strings, booleans and null. Exact quantities/timestamps are schema
/// strings, therefore JSON numbers (whose cross-runtime rendering is unsafe)
/// are rejected.
pub fn jcs_bytes(value: &Value) -> Result<Vec<u8>> {
    fn emit(value: &Value, out: &mut Vec<u8>) -> Result<()> {
        match value {
            Value::Null => out.extend_from_slice(b"null"),
            Value::Bool(true) => out.extend_from_slice(b"true"),
            Value::Bool(false) => out.extend_from_slice(b"false"),
            Value::String(s) => out.extend_from_slice(&serde_json::to_vec(s)?),
            Value::Number(_) => bail!("canonical state schema forbids JSON numeric values; encode exact values as strings"),
            Value::Array(values) => { out.push(b'['); for (i, v) in values.iter().enumerate() { if i > 0 { out.push(b','); } emit(v, out)?; } out.push(b']'); }
            Value::Object(map) => {
                // Schema keys are ASCII. UTF-8 lexical sort equals JCS code-point order for ASCII.
                let mut keys = map.keys().collect::<Vec<_>>();
                if keys.iter().any(|key| !key.is_ascii()) { bail!("canonical state schema key must be ASCII"); }
                keys.sort_unstable(); out.push(b'{');
                for (i, key) in keys.iter().enumerate() { if i > 0 { out.push(b','); } out.extend_from_slice(&serde_json::to_vec(key)?); out.push(b':'); emit(&map[*key], out)?; }
                out.push(b'}');
            }
        } Ok(())
    }
    let mut bytes = Vec::new(); emit(value, &mut bytes)?; Ok(bytes)
}

pub fn jcs_sha256<T: Serialize>(value: &T) -> Result<String> { Ok(sha256_hex(&jcs_bytes(&serde_json::to_value(value)?)?)) }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct CausalOrder {
    pub slot: String, pub transaction_index: Option<String>, pub instruction_path: Option<String>, pub semantic_ordinal: Option<String>,
}
impl CausalOrder {
    pub fn resolved(&self) -> bool { self.transaction_index.is_some() && self.instruction_path.is_some() && self.semantic_ordinal.is_some() }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SemanticEvent {
    pub schema: String, pub canonical_sequence: String, pub source_instance_id: String, pub epoch_id: String,
    pub event_identity: String, pub causal_order: CausalOrder, pub causal_order_status: String,
    pub kind: String, pub signature: String, pub pool: Option<String>, pub mint: Option<String>, pub actor: Option<String>,
    pub side: Option<String>, pub pool_base_raw: Option<String>, pub pool_quote_raw: Option<String>, pub quote_delta_raw: Option<String>,
    pub fee_tuple: Option<[String; 5]>, pub observed_at_unix_ns: String, pub parser_version: String,
    pub raw_frame_sha256: Option<String>, pub previous_record_sha256: String, pub record_sha256: String,
}
impl SemanticEvent {
    pub fn unsigned_value(&self) -> Result<Value> { let mut v=serde_json::to_value(self)?; v.as_object_mut().context("event object")?.insert("record_sha256".into(), Value::String(String::new())); Ok(v) }
    pub fn seal(mut self, previous: String) -> Result<Self> { self.schema=CANONICAL_SCHEMA.into(); self.previous_record_sha256=previous; self.record_sha256=jcs_sha256(&self.unsigned_value()?)?; Ok(self) }
    pub fn validate(&self, expected_sequence: u64, previous: &str) -> Result<()> {
        if self.schema != CANONICAL_SCHEMA { bail!("canonical schema mismatch"); }
        if self.canonical_sequence.parse::<u64>()? != expected_sequence { bail!("canonical sequence gap"); }
        if self.previous_record_sha256 != previous { bail!("canonical hash-chain break"); }
        if self.record_sha256 != jcs_sha256(&self.unsigned_value()?)? { bail!("canonical record hash mismatch"); }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FailureReceipt { pub schema: String, pub epoch_id: String, pub last_sequence: String, pub last_event_hash: String, pub reason: String, pub receipt_sha256: String }
impl FailureReceipt { pub fn seal(mut self) -> Result<Self> { self.receipt_sha256=String::new(); self.receipt_sha256=jcs_sha256(&self)?; Ok(self) } }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoverageRecord { pub schema: String, pub sequence: String, pub epoch_id: String, pub kind: String, pub start_sequence: String, pub end_sequence: String, pub reason_code: Option<String>, pub details_hash: String, pub previous_record_sha256: String, pub record_sha256: String }
impl CoverageRecord {
    pub fn seal(mut self, previous: String) -> Result<Self> { self.schema=COVERAGE_SCHEMA.into(); self.previous_record_sha256=previous; self.record_sha256=String::new(); self.record_sha256=jcs_sha256(&self)?; Ok(self) }
}

#[derive(Debug, Default)]
pub struct CoverageLedger { pub records: Vec<CoverageRecord>, tail: String, next: u64 }
impl CoverageLedger {
    pub fn append(&mut self, epoch: &str, kind: &str, start: u64, end: u64, reason: Option<&str>, details_hash: &str) -> Result<()> {
        if end < start { bail!("coverage interval reversed"); }
        let record=CoverageRecord { schema:String::new(), sequence:self.next.to_string(), epoch_id:epoch.into(), kind:kind.into(), start_sequence:start.to_string(), end_sequence:end.to_string(), reason_code:reason.map(str::to_owned), details_hash:details_hash.into(), previous_record_sha256:String::new(), record_sha256:String::new() }.seal(self.tail.clone())?;
        self.tail=record.record_sha256.clone(); self.next+=1; self.records.push(record); Ok(())
    }
    pub fn universe_complete(&self) -> bool { self.records.iter().rev().find(|r| matches!(r.kind.as_str(), "UNIVERSE_COMPLETE"|"UNIVERSE_INCOMPLETE")).map(|r| r.kind=="UNIVERSE_COMPLETE").unwrap_or(false) }
}

#[derive(Debug, Clone, Default)]
pub struct PoolState { pub first_seen_sequence:u64, pub base:String, pub quote:String, pub prior_quote:Option<String>, pub last_buy_sequence:Option<u64>, pub flows:VecDeque<(u64,String,String)> }
#[derive(Debug, Default)]
pub struct CanonicalState { pub pools:BTreeMap<String,PoolState>, pub seen:BTreeSet<String>, pub last_sequence:u64, pub epoch:String, pub coverage:CoverageLedger }
impl CanonicalState {
    pub fn apply(&mut self, event:&SemanticEvent) -> Result<()> {
        let seq=event.canonical_sequence.parse::<u64>()?; if seq != self.last_sequence+1 { bail!("state sequence gap"); }
        if !self.seen.insert(event.event_identity.clone()) { bail!("duplicate semantic identity"); }
        if event.causal_order_status != "RESOLVED" && event.kind == "PUMPSWAP_SWAP" { bail!("ORDER_AMBIGUOUS"); }
        if let Some(pool)=&event.pool { let state=self.pools.entry(pool.clone()).or_default(); if state.first_seen_sequence==0 { state.first_seen_sequence=seq; }
            if let (Some(base),Some(quote))=(&event.pool_base_raw,&event.pool_quote_raw) { state.prior_quote=Some(state.quote.clone()).filter(|s| !s.is_empty()); state.base=base.clone(); state.quote=quote.clone(); }
            if event.side.as_deref()==Some("BUY") { state.last_buy_sequence=Some(seq); }
            if let (Some(side),Some(flow))=(&event.side,&event.quote_delta_raw) { state.flows.push_back((seq,side.clone(),flow.clone())); while state.flows.len()>10_000 { state.flows.pop_front(); } }
        }
        self.last_sequence=seq; Ok(())
    }
    pub fn admit_decision(&self, event:&SemanticEvent) -> Result<()> { if !self.coverage.universe_complete() { bail!("UNIVERSE_INCOMPLETE"); } if event.causal_order_status!="RESOLVED" { bail!("ORDER_AMBIGUOUS"); } Ok(()) }
}

pub struct JournalWriter { root:PathBuf, epoch:String, next:u64, tail:String, current:Option<File>, current_path:Option<PathBuf>, max_records:u64, count:u64 }
impl JournalWriter {
    pub fn open(root:&Path, epoch:&str, first_sequence:u64, max_records:u64) -> Result<Self> { fs::create_dir_all(root)?; Ok(Self { root:root.into(), epoch:epoch.into(), next:first_sequence, tail:String::new(), current:None, current_path:None, max_records, count:0 }) }
    fn ensure_segment(&mut self) -> Result<()> { if self.current.is_none() { let path=self.root.join(format!("epoch-{}-{:020}.jsonl.open",self.epoch,self.next)); self.current=Some(OpenOptions::new().create_new(true).write(true).open(&path)?); self.current_path=Some(path); self.count=0; } Ok(()) }
    pub fn append(&mut self, event:SemanticEvent) -> Result<SemanticEvent> { self.ensure_segment()?; let sealed=event.seal(self.tail.clone())?; sealed.validate(self.next,&self.tail)?; let bytes=jcs_bytes(&serde_json::to_value(&sealed)?)?; let file=self.current.as_mut().context("segment file")?; file.write_all(&bytes)?; file.write_all(b"\n")?; file.sync_data()?; self.tail=sealed.record_sha256.clone(); self.next+=1; self.count+=1; if self.count>=self.max_records { self.seal_segment()?; } Ok(sealed) }
    pub fn seal_segment(&mut self) -> Result<()> { if let (Some(file),Some(path))=(self.current.take(),self.current_path.take()) { file.sync_all()?; drop(file); let bytes=fs::read(&path)?; let manifest=serde_json::json!({"schema":"EDGESNIFFER_CANONICAL_SEGMENT_V1","epoch_id":self.epoch,"last_sequence":(self.next-1).to_string(),"record_count":self.count.to_string(),"canonical_uncompressed_sha256":sha256_hex(&bytes),"tail_record_sha256":self.tail}); let manifest_bytes=jcs_bytes(&manifest)?; fs::write(path.with_extension("manifest.json"),manifest_bytes)?; fs::rename(&path,path.with_extension("jsonl"))?; } Ok(()) }
}

pub fn scan_journal(root:&Path, epoch:&str) -> Result<Vec<SemanticEvent>> { let mut paths=fs::read_dir(root)?.filter_map(|e|e.ok()).map(|e|e.path()).filter(|p|p.extension().and_then(|e|e.to_str())==Some("jsonl") && p.file_name().and_then(|x|x.to_str()).map(|n|n.contains(epoch)).unwrap_or(false)).collect::<Vec<_>>(); paths.sort(); let mut expected=1_u64; let mut tail=String::new(); let mut result=Vec::new(); for path in paths { for line in BufReader::new(File::open(path)?).lines() { let event:SemanticEvent=serde_json::from_str(&line?)?; event.validate(expected,&tail)?; tail=event.record_sha256.clone(); expected+=1; result.push(event); } } Ok(result) }

#[cfg(test)]
mod tests { use super::*; use tempfile::tempdir;
    fn event(seq:u64) -> SemanticEvent { SemanticEvent { schema:String::new(), canonical_sequence:seq.to_string(), source_instance_id:"s".into(), epoch_id:"e".into(), event_identity:format!("id-{seq}"), causal_order:CausalOrder{slot:"1".into(),transaction_index:Some(seq.to_string()),instruction_path:Some("0/0".into()),semantic_ordinal:Some("0".into())}, causal_order_status:"RESOLVED".into(), kind:"PUMPSWAP_SWAP".into(), signature:"sig".into(), pool:Some("p".into()), mint:Some("m".into()), actor:None, side:Some("BUY".into()), pool_base_raw:Some("1".into()), pool_quote_raw:Some("2".into()), quote_delta_raw:Some("1".into()), fee_tuple:None, observed_at_unix_ns:"1".into(), parser_version:"v".into(), raw_frame_sha256:None, previous_record_sha256:String::new(), record_sha256:String::new() } }
    #[test] fn jcs_is_stable_and_rejects_numbers() -> Result<()> { let a=serde_json::json!({"b":"2","a":"1"}); assert_eq!(jcs_bytes(&a)?,br#"{"a":"1","b":"2"}"#); assert!(jcs_bytes(&serde_json::json!({"n":1})).is_err()); Ok(()) }
    #[test] fn journal_replays_identically_after_restart() -> Result<()> { let d=tempdir()?; let mut w=JournalWriter::open(d.path(),"e",1,2)?; let a=w.append(event(1))?; let b=w.append(event(2))?; w.seal_segment()?; let got=scan_journal(d.path(),"e")?; assert_eq!(got,vec![a,b]); Ok(()) }
    #[test] fn state_refuses_ambiguous_and_incomplete_universe() -> Result<()> { let mut s=CanonicalState::default(); let e=event(1).seal(String::new())?; s.apply(&e)?; assert!(s.admit_decision(&e).is_err()); s.coverage.append("e","UNIVERSE_COMPLETE",1,1,None,"proof")?; assert!(s.admit_decision(&e).is_ok()); let mut bad=event(2); bad.causal_order_status="ORDER_AMBIGUOUS".into(); assert!(s.apply(&bad.seal(e.record_sha256)?).is_err()); Ok(()) }
    #[test] fn scanner_rejects_corruption_and_coverage_blocks_after_gap() -> Result<()> { let d=tempdir()?; let mut w=JournalWriter::open(d.path(),"e",1,100)?; w.append(event(1))?; w.seal_segment()?; let path=fs::read_dir(d.path())?.filter_map(|e|e.ok()).map(|e|e.path()).find(|p|p.extension().and_then(|e|e.to_str())==Some("jsonl")).context("segment")?; fs::write(&path,b"{bad}\n")?; assert!(scan_journal(d.path(),"e").is_err()); let mut coverage=CoverageLedger::default(); coverage.append("e","UNIVERSE_INCOMPLETE",2,9,Some("SOURCE_LAG"),"gap")?; assert!(!coverage.universe_complete()); coverage.append("e","UNIVERSE_COMPLETE",10,10,None,"proof")?; assert!(coverage.universe_complete()); Ok(()) }
}
