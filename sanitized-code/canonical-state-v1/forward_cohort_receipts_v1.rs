//! Lossless producer/consumer receipt contract for the forward-only cohort.
//!
//! The producer and cohort validator share these exact types.  Their hashes
//! cover the complete JCS semantic object, not a reduced consumer projection
//! and not the pretty-print representation on disk.

use std::{collections::{BTreeMap, BTreeSet}, fs, path::Path};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{canonical_state_v1::{hash_excluding_field, jcs_bytes, sha256_hex}, filtered_identity_manifest_v1::{identity_set_digest, ReconciliationReceipt, FINALIZED_RECONCILIATION_SCHEMA}};

pub const IDENTITY_RECEIPT_SCHEMA: &str = "EDGESNIFFER_FINALIZED_RECONCILIATION_SLOT_RECEIPT_V1";
pub const PATH_RECEIPT_SCHEMA: &str = "CANONICAL_FINALIZED_INSTRUCTION_PATH_RECEIPT_V2";
pub const CREATION_RECEIPT_SCHEMA: &str = "CANONICAL_PUMP_MIGRATE_CREATION_RECEIPT_V2";
pub const SLOT_RECEIPT_BUNDLE_MANIFEST_SCHEMA: &str = "EDGESNIFFER_FINALIZED_SLOT_RECEIPT_BUNDLE_V2";

fn number(value: &str, label: &str) -> Result<u64> { if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) { bail!("{label}_INVALID") } Ok(value.parse()?) }
fn digest(value: &str, label: &str) -> Result<()> { if value.len()!=64 || !value.bytes().all(|byte|byte.is_ascii_hexdigit()) { bail!("{label}_INVALID") } Ok(()) }

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct IdentityReceipt {
    pub schema:String, pub source_instance_id:String, pub continuity_epoch:String,
    pub slot:String, pub signature:String, pub transaction_index:String, pub block_identity:String,
    pub block_reconciliation_receipt_sha256:String, pub disposition:String,
    pub reason_code:Option<String>, pub receipt_sha256:String,
}
impl IdentityReceipt {
    pub fn seal(mut self)->Result<Self>{self.receipt_sha256=hash_excluding_field(&self,"receiptSha256")?;Ok(self)}
    pub fn validate(&self)->Result<()> { digest(&self.receipt_sha256,"IDENTITY_RECEIPT_SHA256")?;digest(&self.block_reconciliation_receipt_sha256,"IDENTITY_BLOCK_RECONCILIATION_REFERENCE")?;number(&self.slot,"IDENTITY_RECEIPT_SLOT")?;number(&self.transaction_index,"IDENTITY_RECEIPT_TRANSACTION_INDEX")?;if self.schema!=IDENTITY_RECEIPT_SCHEMA||self.source_instance_id.is_empty()||self.continuity_epoch.is_empty()||self.signature.is_empty()||self.block_identity.is_empty()||self.disposition!="MATCHED"||self.reason_code.is_some()||self.receipt_sha256!=hash_excluding_field(self,"receiptSha256")?{bail!("IDENTITY_RECEIPT_INVALID")}Ok(()) }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct PathReceipt {
    pub schema:String, pub source_instance_id:String, pub continuity_epoch:String,
    pub slot:String, pub finalized_blockhash:String, pub signature:String, pub transaction_index:String,
    pub event_id:String, pub instruction_path:String, pub semantic_kind:String, pub pool:String,
    /// Exact source-frame hash for the filtered transaction whose semantic
    /// observation was decoded. Quarantined missing-semantic receipts keep
    /// the same binding so absence is also durable evidence.
    pub source_frame_sha256:String,
    pub canonical_order:Value, pub disposition:String, pub reason_code:Option<String>, pub receipt_sha256:String,
}
impl PathReceipt {
    pub fn seal(mut self)->Result<Self>{self.receipt_sha256=hash_excluding_field(&self,"receiptSha256")?;Ok(self)}
    pub fn validate(&self)->Result<()> { digest(&self.receipt_sha256,"PATH_RECEIPT_SHA256")?;digest(&self.source_frame_sha256,"PATH_SOURCE_FRAME_SHA256")?;number(&self.slot,"PATH_RECEIPT_SLOT")?;number(&self.transaction_index,"PATH_RECEIPT_TRANSACTION_INDEX")?;if self.schema!=PATH_RECEIPT_SCHEMA||self.source_instance_id.is_empty()||self.continuity_epoch.is_empty()||self.finalized_blockhash.is_empty()||self.signature.is_empty()||self.event_id.is_empty()||self.semantic_kind.is_empty()||self.pool.is_empty()||self.receipt_sha256!=hash_excluding_field(self,"receiptSha256")?{bail!("PATH_RECEIPT_INVALID")}if self.disposition=="PATH_RESOLVED_QUARANTINED_UNIVERSE_INCOMPLETE" {if self.reason_code.as_deref()!=Some("CANDIDATE_UNIVERSE_INCOMPLETE")||self.instruction_path.is_empty()||self.canonical_order.is_null(){bail!("PATH_RECEIPT_RESOLVED_INVALID")}let order=&self.canonical_order;let outer=order.get("outerInstructionIndex").and_then(Value::as_str).and_then(|v|v.parse::<u64>().ok());let inner=order.get("innerInstructionIndex").and_then(Value::as_str).and_then(|v|v.parse::<u64>().ok());if order.get("slot").and_then(Value::as_str)!=Some(self.slot.as_str())||order.get("finalizedTransactionIndex").and_then(Value::as_str)!=Some(self.transaction_index.as_str())||outer.is_none()||!self.instruction_path.starts_with(&format!("outer:{:05}",outer.unwrap()))||inner.is_some_and(|value|!self.instruction_path.contains(&format!("/inner:{value:05}")))||order.get("semanticOrdinal").and_then(Value::as_str).map(|v|v.parse::<u64>().is_ok())!=Some(true){bail!("PATH_RECEIPT_TUPLE_BINDING_INVALID")}} else if self.disposition!="QUARANTINED" {bail!("PATH_RECEIPT_DISPOSITION_INVALID")}Ok(()) }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct CreationReceipt {
    pub schema:String, pub source_instance_id:String, pub continuity_epoch:String,
    pub slot:String, pub finalized_blockhash:String, pub signature:String, pub transaction_index:String,
    pub pool:String, pub mint:String, pub event_id:String, pub instruction_path:String,
    pub identity_receipt_sha256:String, pub path_receipt_sha256:String, pub witness:Value,
    pub disposition:String, pub reason_code:Option<String>, pub receipt_sha256:String,
}
impl CreationReceipt {
    pub fn seal(mut self)->Result<Self>{self.receipt_sha256=hash_excluding_field(&self,"receiptSha256")?;Ok(self)}
    pub fn validate(&self)->Result<()> { digest(&self.receipt_sha256,"CREATION_RECEIPT_SHA256")?;number(&self.slot,"CREATION_RECEIPT_SLOT")?;number(&self.transaction_index,"CREATION_RECEIPT_TRANSACTION_INDEX")?;if self.schema!=CREATION_RECEIPT_SCHEMA||self.source_instance_id.is_empty()||self.continuity_epoch.is_empty()||self.finalized_blockhash.is_empty()||self.signature.is_empty()||self.receipt_sha256!=hash_excluding_field(self,"receiptSha256")?{bail!("CREATION_RECEIPT_INVALID")}match self.disposition.as_str(){"CREATION_VERIFIED_RESTRICTED_COHORT_QUARANTINED"=>{digest(&self.identity_receipt_sha256,"CREATION_IDENTITY_REFERENCE")?;digest(&self.path_receipt_sha256,"CREATION_PATH_REFERENCE")?;if self.reason_code.as_deref()!=Some("FORWARD_COHORT_NOT_OPENED")||self.pool.is_empty()||self.mint.is_empty()||self.event_id.is_empty()||self.instruction_path.is_empty(){bail!("CREATION_RECEIPT_VERIFIED_INVALID")}for(key,value)in[("pool",&self.pool),("mint",&self.mint),("slot",&self.slot),("signature",&self.signature),("transactionIndex",&self.transaction_index),("eventId",&self.event_id),("instructionPath",&self.instruction_path)] {if self.witness.get(key).and_then(Value::as_str)!=Some(value.as_str()){bail!("CREATION_WITNESS_CROSS_REFERENCE_MISMATCH")}}},"CREATION_QUARANTINED"=>{digest(&self.identity_receipt_sha256,"CREATION_QUARANTINE_IDENTITY_REFERENCE")?;if !self.path_receipt_sha256.is_empty()||!self.pool.is_empty()||!self.mint.is_empty()||!self.event_id.is_empty()||!self.instruction_path.is_empty()||self.witness.get("candidateKind").and_then(Value::as_str)!=Some("PUMP_MIGRATE_CREATE_POOL")||self.witness.get("signature").and_then(Value::as_str)!=Some(self.signature.as_str())||self.witness.get("transactionIndex").and_then(Value::as_str)!=Some(self.transaction_index.as_str())||self.reason_code.as_deref().unwrap_or_default().is_empty(){bail!("CREATION_QUARANTINE_INVALID")}},_=>bail!("CREATION_RECEIPT_DISPOSITION_INVALID")}Ok(()) }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct SlotReceiptBundle { pub reconciliation:ReconciliationReceipt, pub source_block_closure_sha256:String, pub identities:Vec<IdentityReceipt>, pub paths:Vec<PathReceipt>, pub creations:Vec<CreationReceipt> }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct SlotReceiptBundleManifest {
    pub schema:String, pub slot:String, pub block_identity:String, pub relevance_policy_sha256:String,
    pub expected_identity_set_sha256:String, pub emitted_identity_set_sha256:String,
    pub reconciliation_receipt_sha256:String, pub source_block_closure_sha256:String, pub identity_receipt_sha256s:Vec<String>,
    pub path_receipt_sha256s:Vec<String>, pub creation_receipt_sha256s:Vec<String>,
    pub manifest_sha256:String,
}
impl SlotReceiptBundleManifest {
    fn seal(mut self)->Result<Self>{self.manifest_sha256=hash_excluding_field(&self,"manifestSha256")?;Ok(self)}
    fn validate(&self)->Result<()> { digest(&self.manifest_sha256,"SLOT_BUNDLE_MANIFEST_SHA256")?;digest(&self.relevance_policy_sha256,"SLOT_BUNDLE_POLICY_SHA256")?;digest(&self.expected_identity_set_sha256,"SLOT_BUNDLE_EXPECTED_SET_SHA256")?;digest(&self.emitted_identity_set_sha256,"SLOT_BUNDLE_EMITTED_SET_SHA256")?;digest(&self.reconciliation_receipt_sha256,"SLOT_BUNDLE_RECONCILIATION_SHA256")?;digest(&self.source_block_closure_sha256,"SLOT_BUNDLE_SOURCE_BLOCK_CLOSURE_SHA256")?;number(&self.slot,"SLOT_BUNDLE_SLOT")?;if self.schema!=SLOT_RECEIPT_BUNDLE_MANIFEST_SCHEMA||self.block_identity.is_empty()||self.expected_identity_set_sha256!=self.emitted_identity_set_sha256||self.manifest_sha256!=hash_excluding_field(self,"manifestSha256")?{bail!("SLOT_BUNDLE_MANIFEST_INVALID")}let mut all=BTreeSet::new();for hash in self.identity_receipt_sha256s.iter().chain(self.path_receipt_sha256s.iter()).chain(self.creation_receipt_sha256s.iter()){digest(hash,"SLOT_BUNDLE_CHILD_SHA256")?;if !all.insert(hash){bail!("SLOT_BUNDLE_DUPLICATE_CHILD_HASH")}}Ok(()) }
}
impl SlotReceiptBundle {
    fn emitted_identity_set_digest(&self)->Result<String>{let mut identities=BTreeMap::new();for identity in &self.identities{let index:usize=identity.transaction_index.parse()?;if identities.insert((identity.slot.clone(),identity.signature.clone()),index).is_some(){bail!("SLOT_IDENTITY_RECEIPT_CONFLICT")}}identity_set_digest(&identities)}
    fn manifest(&self)->Result<SlotReceiptBundleManifest>{let emitted=self.emitted_identity_set_digest()?;Ok(SlotReceiptBundleManifest{schema:SLOT_RECEIPT_BUNDLE_MANIFEST_SCHEMA.into(),slot:self.reconciliation.slot.clone(),block_identity:self.reconciliation.block_identity.clone(),relevance_policy_sha256:self.reconciliation.relevance_policy_sha256.clone(),expected_identity_set_sha256:self.reconciliation.expected_identity_set_sha256.clone(),emitted_identity_set_sha256:emitted,reconciliation_receipt_sha256:self.reconciliation.receipt_sha256.clone(),source_block_closure_sha256:self.source_block_closure_sha256.clone(),identity_receipt_sha256s:self.identities.iter().map(|v|v.receipt_sha256.clone()).collect(),path_receipt_sha256s:self.paths.iter().map(|v|v.receipt_sha256.clone()).collect(),creation_receipt_sha256s:self.creations.iter().map(|v|v.receipt_sha256.clone()).collect(),manifest_sha256:String::new()}.seal()?)}
    pub fn validate(&self)->Result<()> {
        let r=&self.reconciliation;
        if r.schema!=FINALIZED_RECONCILIATION_SCHEMA||r.disposition!="MATCHED"||r.reason_code.is_some()||r.relevance_policy_sha256.len()!=64||r.expected_identity_set_sha256.len()!=64||r.receipt_sha256!=hash_excluding_field(r,"receiptSha256")?{bail!("SLOT_RECONCILIATION_RECEIPT_INVALID")}digest(&self.source_block_closure_sha256,"SLOT_BUNDLE_SOURCE_BLOCK_CLOSURE_SHA256")?;
        let mut identity_hashes=BTreeSet::new();let mut path_hashes=BTreeSet::new();
        for identity in &self.identities {identity.validate()?;if identity.source_instance_id!=r.source_instance_id||identity.continuity_epoch!=r.continuity_epoch||identity.slot!=r.slot||identity.block_identity!=r.block_identity||identity.block_reconciliation_receipt_sha256!=r.receipt_sha256||!identity_hashes.insert(identity.receipt_sha256.clone()){bail!("SLOT_IDENTITY_RECEIPT_CONFLICT")}}
        for path in &self.paths {path.validate()?;if path.source_instance_id!=r.source_instance_id||path.continuity_epoch!=r.continuity_epoch||path.slot!=r.slot||path.finalized_blockhash!=r.block_identity||!path_hashes.insert(path.receipt_sha256.clone()){bail!("SLOT_PATH_RECEIPT_CONFLICT")}}
        let mut creations=BTreeSet::new();for creation in &self.creations {creation.validate()?;if creation.source_instance_id!=r.source_instance_id||creation.continuity_epoch!=r.continuity_epoch||creation.slot!=r.slot||creation.finalized_blockhash!=r.block_identity{bail!("SLOT_CREATION_RECEIPT_BINDING_INVALID")}let identity=self.identities.iter().find(|v|v.receipt_sha256==creation.identity_receipt_sha256).context("SLOT_CREATION_IDENTITY_MISSING")?;if identity.signature!=creation.signature||identity.transaction_index!=creation.transaction_index{bail!("SLOT_CREATION_IDENTITY_CROSS_REFERENCE_INVALID")}if creation.disposition=="CREATION_VERIFIED_RESTRICTED_COHORT_QUARANTINED" {let path=self.paths.iter().find(|v|v.receipt_sha256==creation.path_receipt_sha256).context("SLOT_CREATION_PATH_MISSING")?;if path.signature!=creation.signature||path.transaction_index!=creation.transaction_index||path.event_id!=creation.event_id||path.instruction_path!=creation.instruction_path||!creations.insert((creation.pool.clone(),creation.event_id.clone())){bail!("SLOT_CREATION_CROSS_REFERENCE_INVALID")}}}
        let manifest=self.manifest()?;manifest.validate()?;Ok(())
    }
    pub fn write_atomic(&self,root:&Path)->Result<()> {self.validate()?;let slot=&self.reconciliation.slot;let final_dir=root.join(format!("slot-bundle-{slot}"));if final_dir.exists(){bail!("SLOT_BUNDLE_ALREADY_COMMITTED")}let staging=root.join(format!(".slot-bundle-{slot}-staging-{}",std::process::id()));fs::create_dir_all(&staging)?;let manifest=self.manifest()?;for(name,value)in [("reconciliation.json",serde_json::to_value(&self.reconciliation)?),("source-block-closure-binding.json",serde_json::json!({"sourceBlockClosureSha256":self.source_block_closure_sha256})),("identities.json",serde_json::to_value(&self.identities)?),("paths.json",serde_json::to_value(&self.paths)?),("creations.json",serde_json::to_value(&self.creations)?),("bundle-manifest.json",serde_json::to_value(&manifest)?)]{let bytes=jcs_bytes(&value)?;fs::write(staging.join(name),&bytes)?;}fs::rename(&staging,&final_dir)?;Ok(())}
    pub fn read(root:&Path,slot:u64)->Result<Self>{let directory=root.join(format!("slot-bundle-{slot}"));let manifest:SlotReceiptBundleManifest=serde_json::from_slice(&fs::read(directory.join("bundle-manifest.json")).context("SLOT_BUNDLE_UNCOMMITTED")?)?;manifest.validate()?;if manifest.slot!=slot.to_string(){bail!("SLOT_BUNDLE_MANIFEST_SLOT_MISMATCH")}let reconciliation:ReconciliationReceipt=serde_json::from_slice(&fs::read(directory.join("reconciliation.json"))?)?;let closure:Value=serde_json::from_slice(&fs::read(directory.join("source-block-closure-binding.json"))?)?;let source_block_closure_sha256=closure.get("sourceBlockClosureSha256").and_then(Value::as_str).context("SLOT_BUNDLE_SOURCE_BLOCK_CLOSURE_BINDING_MISSING")?.to_owned();let identities=serde_json::from_slice(&fs::read(directory.join("identities.json"))?)?;let paths=serde_json::from_slice(&fs::read(directory.join("paths.json"))?)?;let creations=serde_json::from_slice(&fs::read(directory.join("creations.json"))?)?;let output=Self{reconciliation,source_block_closure_sha256,identities,paths,creations};output.validate()?;let expected=output.manifest()?;if expected.manifest_sha256!=manifest.manifest_sha256{bail!("SLOT_BUNDLE_MANIFEST_CONTENT_MISMATCH")}Ok(output)}
    pub fn receipt_file_sha256(&self)->Result<String>{Ok(sha256_hex(&jcs_bytes(&serde_json::to_value(&self.reconciliation)?)?))}
}
