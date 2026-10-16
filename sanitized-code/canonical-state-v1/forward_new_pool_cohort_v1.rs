//! Receipt-driven forward-only new-pool cohort proof.
//!
//! This restricted cohort is not a substitute for the full historical pool
//! universe.  It may admit only a post-boundary canonical Pump migration whose
//! identity, finalized path, creation witness, and contiguous slot inventory
//! all validate as one immutable receipt chain.  There are deliberately no
//! caller-controlled completeness booleans in this API.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::{canonical_state_v1::hash_excluding_field, finalized_slot_inventory_v1::FinalizedSlotInventory};

pub const FORWARD_COHORT_SCHEMA: &str = "CANONICAL_FORWARD_NEW_POOL_COHORT_V2";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityReceipt {
    pub schema: String,
    pub source_instance_id: String,
    pub continuity_epoch: String,
    pub slot: String,
    pub signature: String,
    pub transaction_index: String,
    pub block_identity: String,
    pub block_reconciliation_receipt_sha256: String,
    pub disposition: String,
    pub reason_code: Option<String>,
    pub receipt_sha256: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathReceipt {
    pub schema: String,
    pub source_instance_id: String,
    pub continuity_epoch: String,
    pub slot: String,
    pub signature: String,
    pub transaction_index: String,
    pub event_id: String,
    pub instruction_path: String,
    pub disposition: String,
    pub reason_code: Option<String>,
    pub receipt_sha256: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreationReceipt {
    pub schema: String,
    pub source_instance_id: String,
    pub continuity_epoch: String,
    pub slot: String,
    pub signature: String,
    pub transaction_index: String,
    pub pool: String,
    pub mint: String,
    pub event_id: String,
    pub instruction_path: String,
    pub identity_receipt_sha256: String,
    pub path_receipt_sha256: String,
    pub disposition: String,
    pub reason_code: Option<String>,
    pub receipt_sha256: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForwardPoolAdmission {
    pub pool: String,
    pub mint: String,
    pub slot: String,
    pub signature: String,
    pub transaction_index: String,
    pub event_id: String,
    pub instruction_path: String,
    pub creation_receipt_sha256: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForwardCohortLedger {
    pub schema: String,
    pub cohort_id: String,
    pub boundary_slot: String,
    pub last_finalized_slot: String,
    pub source_instance_id: String,
    pub continuity_epoch: String,
    pub slot_inventory_sha256: String,
    pub pools: Vec<ForwardPoolAdmission>,
    pub disposition: String,
    pub reason_code: Option<String>,
    pub ledger_sha256: String,
}

fn number(value: &str, label: &str) -> Result<u64> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) { bail!("{label}_INVALID") }
    value.parse().map_err(Into::into)
}
fn digest(value: &str, label: &str) -> Result<()> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) { bail!("{label}_INVALID") }
    Ok(())
}
fn hash_receipt<T: Serialize>(receipt: &T, field: &str, expected: &str, label: &str) -> Result<()> {
    digest(expected, label)?;
    if hash_excluding_field(receipt, field)? != expected { bail!("{label}_HASH_MISMATCH") }
    Ok(())
}
fn verify_identity(receipt: &IdentityReceipt, source: &str, epoch: &str) -> Result<()> {
    hash_receipt(receipt, "receiptSha256", &receipt.receipt_sha256, "IDENTITY_RECEIPT")?;
    digest(&receipt.block_reconciliation_receipt_sha256, "IDENTITY_BLOCK_RECONCILIATION_REFERENCE")?;
    if receipt.schema != "EDGESNIFFER_FINALIZED_RECONCILIATION_SLOT_RECEIPT_V1" || receipt.source_instance_id != source || receipt.continuity_epoch != epoch || receipt.disposition != "MATCHED" || receipt.reason_code.is_some() || receipt.signature.is_empty() || receipt.block_identity.is_empty() { bail!("IDENTITY_RECEIPT_INVALID") }
    number(&receipt.slot, "IDENTITY_RECEIPT_SLOT")?; number(&receipt.transaction_index, "IDENTITY_RECEIPT_TRANSACTION_INDEX")?; Ok(())
}
fn verify_path(receipt: &PathReceipt, source: &str, epoch: &str) -> Result<()> {
    hash_receipt(receipt, "receiptSha256", &receipt.receipt_sha256, "PATH_RECEIPT")?;
    if receipt.schema != "CANONICAL_FINALIZED_INSTRUCTION_PATH_RECEIPT_V2" || receipt.source_instance_id != source || receipt.continuity_epoch != epoch || receipt.disposition != "PATH_RESOLVED_QUARANTINED_UNIVERSE_INCOMPLETE" || receipt.reason_code.as_deref()!=Some("CANDIDATE_UNIVERSE_INCOMPLETE") || receipt.signature.is_empty() || receipt.event_id.is_empty() || receipt.instruction_path.is_empty() { bail!("PATH_RECEIPT_INVALID") }
    number(&receipt.slot, "PATH_RECEIPT_SLOT")?; number(&receipt.transaction_index, "PATH_RECEIPT_TRANSACTION_INDEX")?; Ok(())
}
fn verify_creation(receipt: &CreationReceipt, source: &str, epoch: &str) -> Result<()> {
    hash_receipt(receipt, "receiptSha256", &receipt.receipt_sha256, "CREATION_RECEIPT")?;
    if receipt.schema != "CANONICAL_PUMP_MIGRATE_CREATION_RECEIPT_V2" || receipt.source_instance_id != source || receipt.continuity_epoch != epoch || receipt.disposition != "CREATION_VERIFIED_RESTRICTED_COHORT_QUARANTINED" || receipt.reason_code.as_deref()!=Some("FORWARD_COHORT_NOT_OPENED") || receipt.pool.is_empty() || receipt.mint.is_empty() || receipt.signature.is_empty() || receipt.event_id.is_empty() || receipt.instruction_path.is_empty() { bail!("CREATION_RECEIPT_INVALID") }
    digest(&receipt.identity_receipt_sha256, "CREATION_IDENTITY_REFERENCE")?; digest(&receipt.path_receipt_sha256, "CREATION_PATH_REFERENCE")?; number(&receipt.slot, "CREATION_RECEIPT_SLOT")?; number(&receipt.transaction_index, "CREATION_RECEIPT_TRANSACTION_INDEX")?; Ok(())
}

pub fn seal_forward_cohort_from_receipts(
    cohort_id: &str,
    boundary_slot: u64,
    inventory: &FinalizedSlotInventory,
    identities: &[IdentityReceipt],
    paths: &[PathReceipt],
    creations: &[CreationReceipt],
) -> Result<ForwardCohortLedger> {
    if cohort_id.is_empty() { bail!("FORWARD_COHORT_ID_MISSING") }
    inventory.validate()?;
    let start = number(&inventory.start_slot, "SLOT_INVENTORY_START")?;
    let end = number(&inventory.end_slot, "SLOT_INVENTORY_END")?;
    if boundary_slot >= start || end < start { bail!("FORWARD_COHORT_BOUNDARY_INVALID") }
    let mut identity_by_hash = BTreeMap::new();
    for receipt in identities {
        verify_identity(receipt, &inventory.source_instance_id, &inventory.continuity_epoch)?;
        if identity_by_hash.insert(receipt.receipt_sha256.clone(), receipt).is_some() { bail!("FORWARD_DUPLICATE_IDENTITY_RECEIPT") }
    }
    let mut path_by_hash = BTreeMap::new();
    for receipt in paths {
        verify_path(receipt, &inventory.source_instance_id, &inventory.continuity_epoch)?;
        if path_by_hash.insert(receipt.receipt_sha256.clone(), receipt).is_some() { bail!("FORWARD_DUPLICATE_PATH_RECEIPT") }
    }
    let mut inventory_by_slot = BTreeMap::new();
    for entry in &inventory.entries {
        inventory_by_slot.insert(entry.slot.as_str(), entry);
    }
    let mut pools = Vec::new(); let mut seen_pool = BTreeSet::new(); let mut seen_decision = BTreeSet::new();
    for creation in creations {
        verify_creation(creation, &inventory.source_instance_id, &inventory.continuity_epoch)?;
        let slot = number(&creation.slot, "CREATION_RECEIPT_SLOT")?;
        if slot <= boundary_slot || slot < start || slot > end { bail!("FORWARD_CREATION_OUTSIDE_BOUNDARY") }
        let identity = identity_by_hash.get(&creation.identity_receipt_sha256).context("FORWARD_CREATION_IDENTITY_RECEIPT_MISSING")?;
        let path = path_by_hash.get(&creation.path_receipt_sha256).context("FORWARD_CREATION_PATH_RECEIPT_MISSING")?;
        let entry = inventory_by_slot.get(creation.slot.as_str()).context("FORWARD_CREATION_SLOT_NOT_IN_INVENTORY")?;
        if entry.disposition != "PRODUCED_RECONCILED" || entry.block_identity.as_deref()!=Some(identity.block_identity.as_str()) || entry.slot_reconciliation_receipt_sha256.as_deref()!=Some(identity.block_reconciliation_receipt_sha256.as_str()) { bail!("FORWARD_CREATION_SLOT_UNRECONCILED") }
        if identity.slot != creation.slot || identity.signature != creation.signature || identity.transaction_index != creation.transaction_index || path.slot != creation.slot || path.signature != creation.signature || path.transaction_index != creation.transaction_index || path.event_id != creation.event_id || path.instruction_path != creation.instruction_path { bail!("FORWARD_RECEIPT_CROSS_REFERENCE_MISMATCH") }
        if !seen_pool.insert(creation.pool.clone()) || !seen_decision.insert((creation.slot.clone(), creation.signature.clone(), creation.event_id.clone())) { bail!("FORWARD_DUPLICATE_CREATION_WITNESS") }
        pools.push(ForwardPoolAdmission { pool: creation.pool.clone(), mint: creation.mint.clone(), slot: creation.slot.clone(), signature: creation.signature.clone(), transaction_index: creation.transaction_index.clone(), event_id: creation.event_id.clone(), instruction_path: creation.instruction_path.clone(), creation_receipt_sha256: creation.receipt_sha256.clone() });
    }
    let mut ledger = ForwardCohortLedger { schema: FORWARD_COHORT_SCHEMA.into(), cohort_id: cohort_id.into(), boundary_slot: boundary_slot.to_string(), last_finalized_slot: end.to_string(), source_instance_id: inventory.source_instance_id.clone(), continuity_epoch: inventory.continuity_epoch.clone(), slot_inventory_sha256: inventory.inventory_sha256.clone(), pools, disposition: "FORWARD_COHORT_COMPLETE_RESTRICTED".into(), reason_code: None, ledger_sha256: String::new() };
    ledger.ledger_sha256 = hash_excluding_field(&ledger, "ledgerSha256")?;
    ledger.validate()?;
    Ok(ledger)
}

impl ForwardCohortLedger {
    pub fn validate(&self) -> Result<()> {
        if self.schema != FORWARD_COHORT_SCHEMA || self.disposition != "FORWARD_COHORT_COMPLETE_RESTRICTED" || self.reason_code.is_some() || self.source_instance_id.is_empty() || self.continuity_epoch.is_empty() { bail!("FORWARD_COHORT_INVALID") }
        number(&self.boundary_slot, "FORWARD_COHORT_BOUNDARY")?; number(&self.last_finalized_slot, "FORWARD_COHORT_LAST_SLOT")?; digest(&self.slot_inventory_sha256, "FORWARD_COHORT_INVENTORY")?;
        if self.ledger_sha256 != hash_excluding_field(self, "ledgerSha256")? { bail!("FORWARD_COHORT_HASH_INVALID") }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::finalized_slot_inventory_v1::{FinalizedSlotInventory, ReconciledProducedSlot};
    const H: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    fn identity() -> IdentityReceipt { let mut v=IdentityReceipt { schema:"EDGESNIFFER_FINALIZED_RECONCILIATION_SLOT_RECEIPT_V1".into(),source_instance_id:"source".into(),continuity_epoch:"epoch".into(),slot:"11".into(),signature:"sig".into(),transaction_index:"2".into(),block_identity:"block".into(),block_reconciliation_receipt_sha256:H.into(),disposition:"MATCHED".into(),reason_code:None,receipt_sha256:String::new() }; v.receipt_sha256=hash_excluding_field(&v,"receiptSha256").unwrap();v }
    fn path() -> PathReceipt { let mut v=PathReceipt { schema:"CANONICAL_FINALIZED_INSTRUCTION_PATH_RECEIPT_V2".into(),source_instance_id:"source".into(),continuity_epoch:"epoch".into(),slot:"11".into(),signature:"sig".into(),transaction_index:"2".into(),event_id:"sig:3".into(),instruction_path:"outer:00000/inner:00001/stack:002".into(),disposition:"PATH_RESOLVED_QUARANTINED_UNIVERSE_INCOMPLETE".into(),reason_code:Some("CANDIDATE_UNIVERSE_INCOMPLETE".into()),receipt_sha256:String::new() };v.receipt_sha256=hash_excluding_field(&v,"receiptSha256").unwrap();v }
    fn creation(identity:&IdentityReceipt,path:&PathReceipt) -> CreationReceipt { let mut v=CreationReceipt { schema:"CANONICAL_PUMP_MIGRATE_CREATION_RECEIPT_V2".into(),source_instance_id:"source".into(),continuity_epoch:"epoch".into(),slot:"11".into(),signature:"sig".into(),transaction_index:"2".into(),pool:"pool".into(),mint:"mint".into(),event_id:"sig:3".into(),instruction_path:"outer:00000/inner:00001/stack:002".into(),identity_receipt_sha256:identity.receipt_sha256.clone(),path_receipt_sha256:path.receipt_sha256.clone(),disposition:"CREATION_VERIFIED_RESTRICTED_COHORT_QUARANTINED".into(),reason_code:Some("FORWARD_COHORT_NOT_OPENED".into()),receipt_sha256:String::new()};v.receipt_sha256=hash_excluding_field(&v,"receiptSha256").unwrap();v }
    fn inventory() -> FinalizedSlotInventory { FinalizedSlotInventory::seal("source", "epoch", 11, 11, H, &[11], vec![ReconciledProducedSlot { slot:"11".into(), block_identity:"block".into(), slot_reconciliation_receipt_sha256:H.into() }]).unwrap() }
    #[test]
    fn receipt_chain_admits_without_caller_controlled_flags() { let i=identity();let p=path();let inv=inventory();let c=creation(&i,&p); let out=seal_forward_cohort_from_receipts("cohort",10,&inv,&[i],&[p],&[c]).unwrap(); assert_eq!(out.pools.len(),1); }
    #[test]
    fn missing_tampered_or_conflicting_receipts_fail_closed() { let i=identity();let p=path();let inv=inventory();let mut c=creation(&i,&p); assert!(seal_forward_cohort_from_receipts("cohort",10,&inv,&[],&[p.clone()],&[c.clone()]).is_err()); c.instruction_path="tampered".into(); assert!(seal_forward_cohort_from_receipts("cohort",10,&inv,&[i],&[p],&[c]).is_err()); }
}
