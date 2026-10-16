//! Lossless producer/consumer receipt contract for the forward-only cohort.
//!
//! The producer and cohort validator share these exact types.  Their hashes
//! cover the complete JCS semantic object, not a reduced consumer projection
//! and not the pretty-print representation on disk.

use std::{collections::BTreeSet, fs, path::Path};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{canonical_state_v1::{hash_excluding_field, jcs_bytes, sha256_hex}, filtered_identity_manifest_v1::{ReconciliationReceipt, FINALIZED_RECONCILIATION_SCHEMA}};

pub const IDENTITY_RECEIPT_SCHEMA: &str = "EDGESNIFFER_FINALIZED_RECONCILIATION_SLOT_RECEIPT_V1";
pub const PATH_RECEIPT_SCHEMA: &str = "CANONICAL_FINALIZED_INSTRUCTION_PATH_RECEIPT_V2";
pub const CREATION_RECEIPT_SCHEMA: &str = "CANONICAL_PUMP_MIGRATE_CREATION_RECEIPT_V2";

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
    pub canonical_order:Value, pub disposition:String, pub reason_code:Option<String>, pub receipt_sha256:String,
}
impl PathReceipt {
    pub fn seal(mut self)->Result<Self>{self.receipt_sha256=hash_excluding_field(&self,"receiptSha256")?;Ok(self)}
    pub fn validate(&self)->Result<()> { digest(&self.receipt_sha256,"PATH_RECEIPT_SHA256")?;number(&self.slot,"PATH_RECEIPT_SLOT")?;number(&self.transaction_index,"PATH_RECEIPT_TRANSACTION_INDEX")?;if self.schema!=PATH_RECEIPT_SCHEMA||self.source_instance_id.is_empty()||self.continuity_epoch.is_empty()||self.finalized_blockhash.is_empty()||self.signature.is_empty()||self.event_id.is_empty()||self.semantic_kind.is_empty()||self.pool.is_empty()||self.receipt_sha256!=hash_excluding_field(self,"receiptSha256")?{bail!("PATH_RECEIPT_INVALID")}if self.disposition=="PATH_RESOLVED_QUARANTINED_UNIVERSE_INCOMPLETE" {if self.reason_code.as_deref()!=Some("CANDIDATE_UNIVERSE_INCOMPLETE")||self.instruction_path.is_empty()||self.canonical_order.is_null(){bail!("PATH_RECEIPT_RESOLVED_INVALID")}} else if self.disposition!="QUARANTINED" {bail!("PATH_RECEIPT_DISPOSITION_INVALID")}Ok(()) }
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
    pub fn validate(&self)->Result<()> { digest(&self.receipt_sha256,"CREATION_RECEIPT_SHA256")?;number(&self.slot,"CREATION_RECEIPT_SLOT")?;number(&self.transaction_index,"CREATION_RECEIPT_TRANSACTION_INDEX")?;if self.schema!=CREATION_RECEIPT_SCHEMA||self.source_instance_id.is_empty()||self.continuity_epoch.is_empty()||self.finalized_blockhash.is_empty()||self.signature.is_empty()||self.receipt_sha256!=hash_excluding_field(self,"receiptSha256")?{bail!("CREATION_RECEIPT_INVALID")}match self.disposition.as_str(){"CREATION_VERIFIED_RESTRICTED_COHORT_QUARANTINED"=>{digest(&self.identity_receipt_sha256,"CREATION_IDENTITY_REFERENCE")?;digest(&self.path_receipt_sha256,"CREATION_PATH_REFERENCE")?;if self.reason_code.as_deref()!=Some("FORWARD_COHORT_NOT_OPENED")||self.pool.is_empty()||self.mint.is_empty()||self.event_id.is_empty()||self.instruction_path.is_empty(){bail!("CREATION_RECEIPT_VERIFIED_INVALID")}for(key,value)in[("pool",&self.pool),("mint",&self.mint),("slot",&self.slot),("signature",&self.signature),("transactionIndex",&self.transaction_index),("eventId",&self.event_id),("instructionPath",&self.instruction_path)] {if self.witness.get(key).and_then(Value::as_str)!=Some(value.as_str()){bail!("CREATION_WITNESS_CROSS_REFERENCE_MISMATCH")}}},"CREATION_QUARANTINED"=>{if self.reason_code.as_deref().unwrap_or_default().is_empty(){bail!("CREATION_QUARANTINE_REASON_MISSING")}},_=>bail!("CREATION_RECEIPT_DISPOSITION_INVALID")}Ok(()) }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct SlotReceiptBundle { pub reconciliation:ReconciliationReceipt, pub identities:Vec<IdentityReceipt>, pub paths:Vec<PathReceipt>, pub creations:Vec<CreationReceipt> }
impl SlotReceiptBundle {
    pub fn validate(&self)->Result<()> {
        let r=&self.reconciliation;
        if r.schema!=FINALIZED_RECONCILIATION_SCHEMA||r.disposition!="MATCHED"||r.reason_code.is_some()||r.receipt_sha256!=hash_excluding_field(r,"receiptSha256")?{bail!("SLOT_RECONCILIATION_RECEIPT_INVALID")}
        let mut identity_hashes=BTreeSet::new();let mut path_hashes=BTreeSet::new();
        for identity in &self.identities {identity.validate()?;if identity.source_instance_id!=r.source_instance_id||identity.continuity_epoch!=r.continuity_epoch||identity.slot!=r.slot||identity.block_identity!=r.block_identity||identity.block_reconciliation_receipt_sha256!=r.receipt_sha256||!identity_hashes.insert(identity.receipt_sha256.clone()){bail!("SLOT_IDENTITY_RECEIPT_CONFLICT")}}
        for path in &self.paths {path.validate()?;if path.source_instance_id!=r.source_instance_id||path.continuity_epoch!=r.continuity_epoch||path.slot!=r.slot||path.finalized_blockhash!=r.block_identity||!path_hashes.insert(path.receipt_sha256.clone()){bail!("SLOT_PATH_RECEIPT_CONFLICT")}}
        let mut creations=BTreeSet::new();for creation in &self.creations {creation.validate()?;if creation.source_instance_id!=r.source_instance_id||creation.continuity_epoch!=r.continuity_epoch||creation.slot!=r.slot||creation.finalized_blockhash!=r.block_identity{bail!("SLOT_CREATION_RECEIPT_BINDING_INVALID")}let identity=self.identities.iter().find(|v|v.receipt_sha256==creation.identity_receipt_sha256).context("SLOT_CREATION_IDENTITY_MISSING")?;let path=self.paths.iter().find(|v|v.receipt_sha256==creation.path_receipt_sha256).context("SLOT_CREATION_PATH_MISSING")?;if identity.signature!=creation.signature||identity.transaction_index!=creation.transaction_index||path.signature!=creation.signature||path.transaction_index!=creation.transaction_index||path.event_id!=creation.event_id||path.instruction_path!=creation.instruction_path||!creations.insert((creation.pool.clone(),creation.event_id.clone())){bail!("SLOT_CREATION_CROSS_REFERENCE_INVALID")}}
        Ok(())
    }
    pub fn write_atomic(&self,root:&Path)->Result<()> {self.validate()?;let slot=&self.reconciliation.slot;for(name,value)in [(format!("receipt-slot-{slot}.json"),serde_json::to_value(&self.reconciliation)?),(format!("identity-receipt-slot-{slot}.json"),serde_json::to_value(&self.identities)?),(format!("path-receipt-slot-{slot}.json"),serde_json::to_value(&self.paths)?),(format!("creation-receipt-slot-{slot}.json"),serde_json::to_value(&self.creations)?)]{let bytes=jcs_bytes(&value)?;let temporary=root.join(format!(".{name}.tmp"));fs::write(&temporary,&bytes)?;fs::rename(temporary,root.join(name))?;}Ok(())}
    pub fn read(root:&Path,slot:u64)->Result<Self>{let reconciliation:ReconciliationReceipt=serde_json::from_slice(&fs::read(root.join(format!("receipt-slot-{slot}.json")))?)?;let identities=serde_json::from_slice(&fs::read(root.join(format!("identity-receipt-slot-{slot}.json")))?)?;let paths=serde_json::from_slice(&fs::read(root.join(format!("path-receipt-slot-{slot}.json")))?)?;let creations=serde_json::from_slice(&fs::read(root.join(format!("creation-receipt-slot-{slot}.json")))?)?;let output=Self{reconciliation,identities,paths,creations};output.validate()?;Ok(output)}
    pub fn receipt_file_sha256(&self)->Result<String>{Ok(sha256_hex(&jcs_bytes(&serde_json::to_value(&self.reconciliation)?)?))}
}
