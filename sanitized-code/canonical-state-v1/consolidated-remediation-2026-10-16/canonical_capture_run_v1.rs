//! Durable control plane for a bounded Canonical State capture.
//!
//! This deliberately separates three facts that earlier diagnostics blurred:
//! source finality progress, finalized-RPC inventory, and arrival of filtered
//! source identities.  A missing identity is never retried through RPC; it is
//! a source-completeness condition which remains pending until the bounded
//! diagnostic deadline or a continuity failure.

use std::{collections::BTreeSet, fs, path::{Path, PathBuf}};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::{canonical_state_v1::{hash_excluding_field, jcs_bytes, sha256_hex}, filtered_identity_manifest_v1::FinalizedTransactionInput};

pub const CAPTURE_RUN_SCHEMA: &str = "EDGESNIFFER_CANONICAL_CAPTURE_RUN_V1";
pub const SOURCE_FINALITY_SCHEMA: &str = "EDGESNIFFER_CANONICAL_SOURCE_FINALITY_V1";
pub const RAW_RPC_EVIDENCE_SCHEMA: &str = "EDGESNIFFER_CANONICAL_RAW_RPC_EVIDENCE_V1";
pub const RELEVANCE_POLICY_SCHEMA: &str = "EDGESNIFFER_CANONICAL_RELEVANCE_POLICY_V1";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CaptureLifecycle {
    Precheck,
    CapturingSource,
    RangeSelected,
    FinalizedInventoryFetched,
    AwaitingSourceMatch,
    ReceiptsStaged,
    ReceiptsCommitted,
    VerifiedDiagnostic,
    Inconclusive,
    Quarantined,
    Voided,
}

impl CaptureLifecycle {
    fn terminal(&self) -> bool {
        matches!(self, Self::VerifiedDiagnostic | Self::Inconclusive | Self::Quarantined | Self::Voided)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceFinalityRecord {
    pub schema: String,
    pub source_instance_id: String,
    pub continuity_epoch: String,
    pub slot: String,
    pub receive_sequence: String,
    pub source_frame_sha256: String,
    pub record_sha256: String,
}

impl SourceFinalityRecord {
    pub fn seal(mut self) -> Result<Self> {
        self.schema = SOURCE_FINALITY_SCHEMA.to_owned();
        self.record_sha256 = hash_excluding_field(&self, "recordSha256")?;
        Ok(self)
    }
    pub fn validate(&self, source: &str, epoch: &str) -> Result<()> {
        if self.schema != SOURCE_FINALITY_SCHEMA || self.source_instance_id != source || self.continuity_epoch != epoch || self.slot.parse::<u64>().is_err() || self.receive_sequence.parse::<u64>().is_err() || self.source_frame_sha256.len() != 64 || self.record_sha256 != hash_excluding_field(self, "recordSha256")? {
            bail!("SOURCE_FINALITY_RECORD_INVALID")
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpectedIdentity {
    pub slot: String,
    pub signature: String,
    pub transaction_index: String,
}

impl ExpectedIdentity {
    pub fn validate(&self) -> Result<()> {
        if self.slot.parse::<u64>().is_err() || self.transaction_index.parse::<u64>().is_err() || self.signature.is_empty() { bail!("EXPECTED_IDENTITY_INVALID") }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawRpcEvidence {
    pub schema: String,
    pub method: String,
    pub parser_schema: String,
    pub slot: Option<String>,
    pub raw_path: String,
    pub raw_sha256: String,
    pub byte_length: String,
    pub evidence_sha256: String,
}

impl RawRpcEvidence {
    pub fn new(method: &str, slot: Option<u64>, raw_path: &str, raw: &[u8]) -> Result<Self> {
        let parser_schema=match method {"getBlocks"=>"SOLANA_JSON_RPC_GETBLOCKS_FINALIZED_V1","getBlock"=>"SOLANA_JSON_RPC_GETBLOCK_FULL_V1",_=>"SOLANA_JSON_RPC_UNSUPPORTED"};if parser_schema=="SOLANA_JSON_RPC_UNSUPPORTED"{bail!("RAW_RPC_METHOD_UNSUPPORTED")}let mut output = Self { schema: RAW_RPC_EVIDENCE_SCHEMA.into(), method: method.into(), parser_schema:parser_schema.into(), slot: slot.map(|value| value.to_string()), raw_path: raw_path.into(), raw_sha256: sha256_hex(raw), byte_length: raw.len().to_string(), evidence_sha256: String::new() };
        output.evidence_sha256 = hash_excluding_field(&output, "evidenceSha256")?;
        Ok(output)
    }
    pub fn validate(&self) -> Result<()> {
        if self.schema != RAW_RPC_EVIDENCE_SCHEMA || !matches!(self.parser_schema.as_str(),"SOLANA_JSON_RPC_GETBLOCKS_FINALIZED_V1"|"SOLANA_JSON_RPC_GETBLOCK_FULL_V1") || self.method.is_empty() || self.raw_path.is_empty() || self.raw_sha256.len() != 64 || self.byte_length.parse::<u64>().is_err() || self.evidence_sha256 != hash_excluding_field(self, "evidenceSha256")? { bail!("RAW_RPC_EVIDENCE_INVALID") }
        if let Some(slot) = &self.slot { slot.parse::<u64>().context("RAW_RPC_EVIDENCE_SLOT")?; }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalRelevancePolicy {
    pub schema: String,
    pub policy_version: String,
    pub program_ids: Vec<String>,
    pub include_votes: bool,
    pub include_failed: bool,
    pub max_supported_transaction_version: String,
    pub policy_sha256: String,
}

impl CanonicalRelevancePolicy {
    pub fn new(policy_version: &str, program_ids: Vec<String>, max_supported_transaction_version: u64) -> Result<Self> {
        let mut program_ids = program_ids;
        program_ids.sort(); program_ids.dedup();
        if policy_version.is_empty() || program_ids.is_empty() { bail!("RELEVANCE_POLICY_INVALID") }
        let mut value = Self { schema: RELEVANCE_POLICY_SCHEMA.into(), policy_version: policy_version.into(), program_ids, include_votes: false, include_failed: false, max_supported_transaction_version: max_supported_transaction_version.to_string(), policy_sha256: String::new() };
        value.policy_sha256 = hash_excluding_field(&value, "policySha256")?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        if self.schema != RELEVANCE_POLICY_SCHEMA || self.policy_version.is_empty() || self.program_ids.is_empty() || self.include_votes || self.include_failed || self.max_supported_transaction_version.parse::<u64>().is_err() || self.policy_sha256 != hash_excluding_field(self, "policySha256")? { bail!("RELEVANCE_POLICY_INVALID") }
        let mut sorted = self.program_ids.clone(); sorted.sort(); sorted.dedup(); if sorted != self.program_ids { bail!("RELEVANCE_POLICY_PROGRAM_ORDER_INVALID") }
        Ok(())
    }
    pub fn supports_and_matches(&self, transaction: &FinalizedTransactionInput) -> Result<bool> {
        self.validate()?;
        Ok(transaction.supported_version && self.matches_program_ids(transaction)?)
    }
    pub fn matches_program_ids(&self, transaction: &FinalizedTransactionInput) -> Result<bool> {
        self.validate()?;
        Ok(transaction.account_or_program_ids.iter().any(|id| self.program_ids.binary_search(id).is_ok()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceAccounting {
    pub rpc_maximum: String,
    pub rpc_used: String,
    pub evidence_cap_bytes: String,
    pub evidence_used_bytes: String,
    pub memory_cap_bytes: String,
    pub memory_observed_bytes: String,
    pub queue_cap: String,
    pub queue_observed: String,
}

impl ResourceAccounting {
    pub fn new(rpc_maximum: u64, evidence_cap_bytes: u64, memory_cap_bytes: u64, queue_cap: u64) -> Result<Self> {
        if rpc_maximum == 0 || evidence_cap_bytes == 0 || memory_cap_bytes == 0 || queue_cap == 0 { bail!("RESOURCE_ACCOUNTING_INVALID") }
        Ok(Self { rpc_maximum: rpc_maximum.to_string(), rpc_used: "0".into(), evidence_cap_bytes: evidence_cap_bytes.to_string(), evidence_used_bytes: "0".into(), memory_cap_bytes: memory_cap_bytes.to_string(), memory_observed_bytes: "0".into(), queue_cap: queue_cap.to_string(), queue_observed: "0".into() })
    }
    fn number(value: &str, label: &str) -> Result<u64> { value.parse::<u64>().with_context(|| label.to_owned()) }
    pub fn consume_rpc(&mut self) -> Result<()> { let next=Self::number(&self.rpc_used,"RPC_USED")?.checked_add(1).context("RPC_OVERFLOW")?; if next>Self::number(&self.rpc_maximum,"RPC_MAX")? { bail!("RPC_REQUEST_BUDGET_EXHAUSTED") }; self.rpc_used=next.to_string(); Ok(()) }
    pub fn reserve_evidence(&mut self, bytes:u64) -> Result<()> { let next=Self::number(&self.evidence_used_bytes,"EVIDENCE_USED")?.checked_add(bytes).context("EVIDENCE_OVERFLOW")?; if next>Self::number(&self.evidence_cap_bytes,"EVIDENCE_CAP")? { bail!("EVIDENCE_OUTPUT_CAP_REACHED") }; self.evidence_used_bytes=next.to_string(); Ok(()) }
    pub fn observe_memory(&mut self, bytes:u64) -> Result<()> { if bytes>Self::number(&self.memory_cap_bytes,"MEMORY_CAP")? { bail!("RSS_CAP") }; self.memory_observed_bytes=self.memory_observed_bytes.parse::<u64>()?.max(bytes).to_string(); Ok(()) }
    pub fn observe_queue(&mut self, entries:u64) -> Result<()> { if entries>Self::number(&self.queue_cap,"QUEUE_CAP")? { bail!("QUEUE_CAP_REACHED") }; self.queue_observed=self.queue_observed.parse::<u64>()?.max(entries).to_string(); Ok(()) }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureRunCheckpoint {
    pub schema: String,
    pub run_id: String,
    pub lifecycle: CaptureLifecycle,
    pub source_instance_id: String,
    pub continuity_epoch: String,
    pub policy_version: String,
    pub policy_sha256: String,
    pub identity_manifest_tail_sha256: String,
    pub identity_manifest_next_sequence: String,
    pub source_finality: Vec<SourceFinalityRecord>,
    pub range_start_slot: Option<String>,
    pub range_end_slot: Option<String>,
    pub finalized_inventory_sha256: Option<String>,
    pub expected_identities: Vec<ExpectedIdentity>,
    pub raw_rpc_evidence: Vec<RawRpcEvidence>,
    pub resources: ResourceAccounting,
    pub receipt_state: String,
    pub terminal_reason: Option<String>,
    pub checkpoint_sha256: String,
}

impl CaptureRunCheckpoint {
    fn seal(mut self) -> Result<Self> { self.checkpoint_sha256=hash_excluding_field(&self,"checkpointSha256")?; Ok(self) }
    pub fn validate(&self) -> Result<()> {
        if self.schema != CAPTURE_RUN_SCHEMA || self.run_id.is_empty() || self.source_instance_id.is_empty() || self.continuity_epoch.is_empty() || self.policy_version.is_empty() || self.policy_sha256.len()!=64 || self.identity_manifest_next_sequence.parse::<u64>().is_err() || self.checkpoint_sha256 != hash_excluding_field(self,"checkpointSha256")? { bail!("CAPTURE_RUN_CHECKPOINT_INVALID") }
        let mut finalities=BTreeSet::new(); for record in &self.source_finality { record.validate(&self.source_instance_id,&self.continuity_epoch)?; if !finalities.insert(record.slot.clone()) { bail!("SOURCE_FINALITY_DUPLICATE") } }
        let mut expected=BTreeSet::new(); for identity in &self.expected_identities { identity.validate()?; if !expected.insert(identity.clone()) { bail!("EXPECTED_IDENTITY_DUPLICATE") } }
        for evidence in &self.raw_rpc_evidence { evidence.validate()?; }
        if self.lifecycle.terminal() != self.terminal_reason.is_some() { bail!("CAPTURE_RUN_TERMINAL_STATE_INVALID") }
        Ok(())
    }
}

pub struct CaptureRunJournal { root: PathBuf, checkpoint_path: PathBuf, pub checkpoint: CaptureRunCheckpoint }

impl CaptureRunJournal {
    pub fn new(root:&Path, run_id:&str, source:&str, epoch:&str, policy:&CanonicalRelevancePolicy, resources:ResourceAccounting) -> Result<Self> {
        policy.validate()?; if root.join("capture-run.checkpoint.json").exists() { bail!("DURABLE_PRIOR_RUN_REQUIRES_VOID_OR_RECOVERY") }
        fs::create_dir_all(root)?;
        let checkpoint=CaptureRunCheckpoint { schema:CAPTURE_RUN_SCHEMA.into(),run_id:run_id.into(),lifecycle:CaptureLifecycle::Precheck,source_instance_id:source.into(),continuity_epoch:epoch.into(),policy_version:policy.policy_version.clone(),policy_sha256:policy.policy_sha256.clone(),identity_manifest_tail_sha256:String::new(),identity_manifest_next_sequence:"0".into(),source_finality:vec![],range_start_slot:None,range_end_slot:None,finalized_inventory_sha256:None,expected_identities:vec![],raw_rpc_evidence:vec![],resources,receipt_state:"NONE".into(),terminal_reason:None,checkpoint_sha256:String::new() }.seal()?;
        let mut journal=Self { root:root.into(),checkpoint_path:root.join("capture-run.checkpoint.json"),checkpoint };
        journal.persist()?; journal.transition(CaptureLifecycle::CapturingSource)?; Ok(journal)
    }
    pub fn load(root:&Path) -> Result<Self> {
        let checkpoint:CaptureRunCheckpoint=serde_json::from_slice(&fs::read(root.join("capture-run.checkpoint.json"))?)?; checkpoint.validate()?;
        Ok(Self {root:root.into(),checkpoint_path:root.join("capture-run.checkpoint.json"),checkpoint})
    }
    pub fn persist(&mut self) -> Result<()> { self.checkpoint=self.checkpoint.clone().seal()?; let bytes=jcs_bytes(&serde_json::to_value(&self.checkpoint)?)?; let temporary=self.checkpoint_path.with_extension("tmp"); fs::write(&temporary,&bytes)?; fs::rename(&temporary,&self.checkpoint_path)?; Ok(()) }
    pub fn transition(&mut self, next:CaptureLifecycle) -> Result<()> {
        use CaptureLifecycle::*;
        let allowed=matches!((&self.checkpoint.lifecycle,&next),(Precheck,CapturingSource)|(CapturingSource,RangeSelected)|(RangeSelected,FinalizedInventoryFetched)|(FinalizedInventoryFetched,AwaitingSourceMatch)|(AwaitingSourceMatch,ReceiptsStaged)|(ReceiptsStaged,ReceiptsCommitted)|(ReceiptsCommitted,VerifiedDiagnostic)|(_,Inconclusive)|(_,Quarantined)|(_,Voided));
        if !allowed || self.checkpoint.lifecycle.terminal() { bail!("CAPTURE_RUN_INVALID_TRANSITION") }
        self.checkpoint.lifecycle=next; self.persist()
    }
    pub fn observe_finality(&mut self, record:SourceFinalityRecord) -> Result<()> {
        if self.checkpoint.lifecycle != CaptureLifecycle::CapturingSource && self.checkpoint.lifecycle != CaptureLifecycle::AwaitingSourceMatch { bail!("SOURCE_FINALITY_OUTSIDE_CAPTURE") }
        record.validate(&self.checkpoint.source_instance_id,&self.checkpoint.continuity_epoch)?;
        if self.checkpoint.source_finality.iter().any(|existing|existing.slot==record.slot) { bail!("SOURCE_FINALITY_DUPLICATE") }
        self.checkpoint.source_finality.push(record); self.persist()
    }
    pub fn bind_manifest(&mut self, tail:&str,next_sequence:u64) -> Result<()> { if tail.len()!=64 && !tail.is_empty(){bail!("MANIFEST_TAIL_INVALID")} self.checkpoint.identity_manifest_tail_sha256=tail.into();self.checkpoint.identity_manifest_next_sequence=next_sequence.to_string();self.persist() }
    pub fn select_range(&mut self, slots:u64) -> Result<(u64,u64)> {
        if self.checkpoint.lifecycle != CaptureLifecycle::CapturingSource || !(3..=5).contains(&slots) { bail!("RANGE_SELECTION_INVALID") }
        let seen=self.checkpoint.source_finality.iter().map(|record|record.slot.parse::<u64>()).collect::<Result<BTreeSet<_>,_>>()?;
        let start=seen.iter().copied().find(|candidate| (0..slots).all(|offset| seen.contains(&(*candidate + offset)))).context("NO_CONTIGUOUS_SOURCE_FINALITY_RANGE")?;
        let end=start.checked_add(slots-1).context("RANGE_OVERFLOW")?; self.checkpoint.range_start_slot=Some(start.to_string());self.checkpoint.range_end_slot=Some(end.to_string());self.transition(CaptureLifecycle::RangeSelected)?;Ok((start,end))
    }
    pub fn record_inventory(&mut self, inventory_sha:&str, expected:Vec<ExpectedIdentity>, evidence:Vec<RawRpcEvidence>) -> Result<()> {
        if self.checkpoint.lifecycle!=CaptureLifecycle::RangeSelected || inventory_sha.len()!=64 { bail!("FINALIZED_INVENTORY_BINDING_INVALID") }
        let mut set=BTreeSet::new();for item in &expected{item.validate()?;if !set.insert(item.clone()){bail!("EXPECTED_IDENTITY_DUPLICATE")}}for item in &evidence{item.validate()?;}
        self.checkpoint.finalized_inventory_sha256=Some(inventory_sha.into());self.checkpoint.expected_identities=expected;self.checkpoint.raw_rpc_evidence=evidence;self.transition(CaptureLifecycle::FinalizedInventoryFetched)?;self.transition(CaptureLifecycle::AwaitingSourceMatch)
    }
    pub fn source_match_complete(&self, observed:&BTreeSet<ExpectedIdentity>) -> Result<bool> { if self.checkpoint.lifecycle!=CaptureLifecycle::AwaitingSourceMatch {bail!("SOURCE_MATCH_OUTSIDE_AWAITING")}let expected=self.checkpoint.expected_identities.iter().cloned().collect::<BTreeSet<_>>();Ok(expected==*observed) }
    pub fn stage_receipts(&mut self)->Result<()> { if self.checkpoint.lifecycle!=CaptureLifecycle::AwaitingSourceMatch {bail!("RECEIPT_STAGE_INVALID")}self.checkpoint.receipt_state="STAGED".into();self.transition(CaptureLifecycle::ReceiptsStaged) }
    pub fn commit_receipts(&mut self)->Result<()> { if self.checkpoint.lifecycle!=CaptureLifecycle::ReceiptsStaged {bail!("RECEIPT_COMMIT_INVALID")}self.checkpoint.receipt_state="COMMITTED".into();self.transition(CaptureLifecycle::ReceiptsCommitted)?;self.checkpoint.terminal_reason=Some("VERIFIED_DIAGNOSTIC_RECEIPTS_COMMITTED".into());self.transition(CaptureLifecycle::VerifiedDiagnostic) }
    pub fn terminal(&mut self, lifecycle:CaptureLifecycle, reason:&str)->Result<()> { if !matches!(lifecycle,CaptureLifecycle::Inconclusive|CaptureLifecycle::Quarantined|CaptureLifecycle::Voided)||reason.is_empty(){bail!("CAPTURE_RUN_TERMINAL_INVALID")}self.checkpoint.terminal_reason=Some(reason.into());self.transition(lifecycle) }
    pub fn void_existing(root:&Path, reason:&str)->Result<()> { let mut journal=Self::load(root)?; if !journal.checkpoint.lifecycle.terminal(){journal.terminal(CaptureLifecycle::Voided,reason)?;}Ok(()) }
    pub fn root(&self)->&Path {&self.root}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn policy()->CanonicalRelevancePolicy{CanonicalRelevancePolicy::new("v1",vec!["pump".into(),"swap".into()],1).unwrap()}
    fn resources()->ResourceAccounting{ResourceAccounting::new(16,6*1024*1024*1024,8*1024*1024*1024,32).unwrap()}
    fn root(label:&str)->std::path::PathBuf{let root=std::env::temp_dir().join(format!("capture-run-{label}-{}",std::process::id()));let _=fs::remove_dir_all(&root);fs::create_dir_all(&root).unwrap();root}
    fn finality(slot:u64)->SourceFinalityRecord{SourceFinalityRecord{schema:String::new(),source_instance_id:"source".into(),continuity_epoch:"1".into(),slot:slot.to_string(),receive_sequence:slot.to_string(),source_frame_sha256:"a".repeat(64),record_sha256:String::new()}.seal().unwrap()}
    #[test] fn range_is_selected_from_source_finality_without_rpc_or_identity_match(){let root=root("range");let mut run=CaptureRunJournal::new(&root,"run","source","1",&policy(),resources()).unwrap();for slot in 10..15{run.observe_finality(finality(slot)).unwrap();}assert_eq!(run.select_range(5).unwrap(),(10,14));assert_eq!(run.checkpoint.resources.rpc_used,"0");}
    #[test] fn delayed_identity_is_awaited_without_consuming_another_rpc(){let root=root("late");let mut run=CaptureRunJournal::new(&root,"run","source","1",&policy(),resources()).unwrap();for slot in 10..15{run.observe_finality(finality(slot)).unwrap();}run.select_range(5).unwrap();let expected=(10..15).map(|slot|ExpectedIdentity{slot:slot.to_string(),signature:format!("sig-{slot}"),transaction_index:"0".into()}).collect();let evidence=vec![RawRpcEvidence::new("getBlocks",None,"raw.json",b"{}").unwrap()];run.record_inventory("b".repeat(64).as_str(),expected,evidence).unwrap();let mut observed=BTreeSet::new();for slot in 10..14{observed.insert(ExpectedIdentity{slot:slot.to_string(),signature:format!("sig-{slot}"),transaction_index:"0".into()});}assert!(!run.source_match_complete(&observed).unwrap());assert_eq!(run.checkpoint.resources.rpc_used,"0");observed.insert(ExpectedIdentity{slot:"14".into(),signature:"sig-14".into(),transaction_index:"0".into()});assert!(run.source_match_complete(&observed).unwrap());}
    #[test] fn crash_is_explicitly_voidable_and_never_bridged(){let root=root("void");let _=CaptureRunJournal::new(&root,"run","source","1",&policy(),resources()).unwrap();CaptureRunJournal::void_existing(&root,"PROCESS_CRASH").unwrap();assert_eq!(CaptureRunJournal::load(&root).unwrap().checkpoint.lifecycle,CaptureLifecycle::Voided);}
    #[test] fn policy_is_single_supported_relevance_rule(){let p=policy();let tx=FinalizedTransactionInput{transaction_index:"0".into(),signature:"s".into(),account_or_program_ids:vec!["pump".into()],supported_version:true};assert!(p.supports_and_matches(&tx).unwrap());let failed=FinalizedTransactionInput{supported_version:false,..tx};assert!(!p.supports_and_matches(&failed).unwrap());}
    #[test] fn every_terminal_checkpoint_reloads_after_atomic_transition(){
        for (label, terminal) in [("inconclusive",CaptureLifecycle::Inconclusive),("quarantined",CaptureLifecycle::Quarantined),("voided",CaptureLifecycle::Voided)] {
            let root=root(label);let mut run=CaptureRunJournal::new(&root,"run","source","1",&policy(),resources()).unwrap();run.terminal(terminal.clone(),"TEST_TERMINAL").unwrap();assert_eq!(CaptureRunJournal::load(&root).unwrap().checkpoint.lifecycle,terminal);
        }
        let root=root("verified");let mut run=CaptureRunJournal::new(&root,"run","source","1",&policy(),resources()).unwrap();for slot in 10..13{run.observe_finality(finality(slot)).unwrap();}run.select_range(3).unwrap();run.record_inventory(&"b".repeat(64),vec![],vec![RawRpcEvidence::new("getBlocks",None,"raw.json",b"{}").unwrap()]).unwrap();run.stage_receipts().unwrap();run.commit_receipts().unwrap();let reloaded=CaptureRunJournal::load(&root).unwrap();assert_eq!(reloaded.checkpoint.lifecycle,CaptureLifecycle::VerifiedDiagnostic);assert_eq!(reloaded.checkpoint.terminal_reason.as_deref(),Some("VERIFIED_DIAGNOSTIC_RECEIPTS_COMMITTED"));
    }
}
