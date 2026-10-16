//! Receipt-only admission for the restricted forward new-pool cohort.

use std::{collections::BTreeSet, path::Path};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::{canonical_state_v1::hash_excluding_field, finalized_slot_inventory_v1::FinalizedSlotInventory, forward_cohort_receipts_v1::{CreationReceipt, SlotReceiptBundle}};

pub const FORWARD_COHORT_SCHEMA: &str = "CANONICAL_FORWARD_NEW_POOL_COHORT_V3";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct ForwardPoolAdmission { pub pool:String,pub mint:String,pub slot:String,pub signature:String,pub transaction_index:String,pub event_id:String,pub instruction_path:String,pub creation_receipt_sha256:String }
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all="camelCase")]
pub struct ForwardCohortLedger { pub schema:String,pub cohort_id:String,pub boundary_slot:String,pub last_finalized_slot:String,pub source_instance_id:String,pub continuity_epoch:String,pub slot_inventory_sha256:String,pub pools:Vec<ForwardPoolAdmission>,pub disposition:String,pub reason_code:Option<String>,pub ledger_sha256:String }

fn number(value:&str,label:&str)->Result<u64>{if value.is_empty()||!value.bytes().all(|b|b.is_ascii_digit()){bail!("{label}_INVALID")}Ok(value.parse()?)}
fn digest(value:&str,label:&str)->Result<()> {if value.len()!=64||!value.bytes().all(|b|b.is_ascii_hexdigit()){bail!("{label}_INVALID")}Ok(())}

pub fn seal_forward_cohort_from_persisted_receipts(root:&Path,cohort_id:&str,boundary_slot:u64,inventory:&FinalizedSlotInventory)->Result<ForwardCohortLedger>{
    inventory.validate()?;
    let mut bundles=Vec::new();
    for entry in &inventory.entries { if entry.disposition=="PRODUCED_RECONCILED" { let slot=number(&entry.slot,"INVENTORY_SLOT")?;let bundle=SlotReceiptBundle::read(root,slot)?; if entry.block_identity.as_deref()!=Some(bundle.reconciliation.block_identity.as_str())||entry.slot_reconciliation_receipt_sha256.as_deref()!=Some(bundle.reconciliation.receipt_sha256.as_str()){bail!("PERSISTED_SLOT_INVENTORY_BINDING_INVALID")}bundles.push(bundle) } }
    seal_forward_cohort_from_bundles(cohort_id,boundary_slot,inventory,&bundles)
}
/// The production receipt-only entry point. The consumer never accepts an
/// inventory supplied by a caller without first loading and binding the
/// producer's persisted inventory and its retained `getBlocks` proof bytes.
pub fn seal_forward_cohort_from_persisted_receipt_chain(root:&Path,cohort_id:&str,boundary_slot:u64)->Result<ForwardCohortLedger>{
    let inventory=FinalizedSlotInventory::read_verified(root)?;
    seal_forward_cohort_from_persisted_receipts(root,cohort_id,boundary_slot,&inventory)
}
pub fn seal_forward_cohort_from_bundles(cohort_id:&str,boundary_slot:u64,inventory:&FinalizedSlotInventory,bundles:&[SlotReceiptBundle])->Result<ForwardCohortLedger>{
    if cohort_id.is_empty(){bail!("FORWARD_COHORT_ID_MISSING")}inventory.validate()?;let start=number(&inventory.start_slot,"SLOT_INVENTORY_START")?;let end=number(&inventory.end_slot,"SLOT_INVENTORY_END")?;if boundary_slot>=start{bail!("FORWARD_COHORT_BOUNDARY_INVALID")}
    let mut seen_slots=BTreeSet::new();let mut pool_ids=BTreeSet::new();let mut decision_ids=BTreeSet::new();let mut pools=Vec::new();
    for bundle in bundles {bundle.validate()?;let r=&bundle.reconciliation;if r.source_instance_id!=inventory.source_instance_id||r.continuity_epoch!=inventory.continuity_epoch||!seen_slots.insert(r.slot.clone()){bail!("FORWARD_SLOT_RECEIPT_DUPLICATE_OR_EPOCH_CONFLICT")}let entry=inventory.entries.iter().find(|entry|entry.slot==r.slot).context("FORWARD_SLOT_NOT_IN_INVENTORY")?;if entry.disposition!="PRODUCED_RECONCILED"||entry.block_identity.as_deref()!=Some(r.block_identity.as_str())||entry.slot_reconciliation_receipt_sha256.as_deref()!=Some(r.receipt_sha256.as_str()){bail!("FORWARD_SLOT_INVENTORY_MISMATCH")}
        for creation in &bundle.creations {admit_creation(creation,boundary_slot,start,end,&mut pool_ids,&mut decision_ids,&mut pools)?;}
    }
    let produced=inventory.entries.iter().filter(|entry|entry.disposition=="PRODUCED_RECONCILED").count();if seen_slots.len()!=produced{bail!("FORWARD_PRODUCED_SLOT_RECEIPT_MISSING")}
    let mut ledger=ForwardCohortLedger{schema:FORWARD_COHORT_SCHEMA.into(),cohort_id:cohort_id.into(),boundary_slot:boundary_slot.to_string(),last_finalized_slot:end.to_string(),source_instance_id:inventory.source_instance_id.clone(),continuity_epoch:inventory.continuity_epoch.clone(),slot_inventory_sha256:inventory.inventory_sha256.clone(),pools,disposition:"FORWARD_COHORT_COMPLETE_RESTRICTED".into(),reason_code:None,ledger_sha256:String::new()};ledger.ledger_sha256=hash_excluding_field(&ledger,"ledgerSha256")?;ledger.validate()?;Ok(ledger)
}
fn admit_creation(creation:&CreationReceipt,boundary:u64,start:u64,end:u64,pools:&mut BTreeSet<String>,decisions:&mut BTreeSet<(String,String,String)>,out:&mut Vec<ForwardPoolAdmission>)->Result<()> {creation.validate()?;if creation.disposition!="CREATION_VERIFIED_RESTRICTED_COHORT_QUARANTINED"{bail!("FORWARD_CREATION_QUARANTINED")}let slot=number(&creation.slot,"CREATION_SLOT")?;if slot<=boundary||slot<start||slot>end||!pools.insert(creation.pool.clone())||!decisions.insert((creation.slot.clone(),creation.signature.clone(),creation.event_id.clone())){bail!("FORWARD_CREATION_NOT_UNIQUE_OR_OUTSIDE_BOUNDARY")}out.push(ForwardPoolAdmission{pool:creation.pool.clone(),mint:creation.mint.clone(),slot:creation.slot.clone(),signature:creation.signature.clone(),transaction_index:creation.transaction_index.clone(),event_id:creation.event_id.clone(),instruction_path:creation.instruction_path.clone(),creation_receipt_sha256:creation.receipt_sha256.clone()});Ok(())}
impl ForwardCohortLedger {pub fn validate(&self)->Result<()> {if self.schema!=FORWARD_COHORT_SCHEMA||self.disposition!="FORWARD_COHORT_COMPLETE_RESTRICTED"||self.reason_code.is_some()||self.source_instance_id.is_empty()||self.continuity_epoch.is_empty(){bail!("FORWARD_COHORT_INVALID")}number(&self.boundary_slot,"FORWARD_COHORT_BOUNDARY")?;number(&self.last_finalized_slot,"FORWARD_COHORT_LAST_SLOT")?;digest(&self.slot_inventory_sha256,"FORWARD_COHORT_INVENTORY")?;if self.ledger_sha256!=hash_excluding_field(self,"ledgerSha256")?{bail!("FORWARD_COHORT_HASH_INVALID")}Ok(())}}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn quarantined_creation_receipt_cannot_be_admitted() {
        let creation = CreationReceipt {
            schema: "CANONICAL_PUMP_MIGRATE_CREATION_RECEIPT_V2".into(),
            source_instance_id: "source".into(), continuity_epoch: "1".into(),
            slot: "11".into(), finalized_blockhash: "block".into(),
            signature: "signature".into(), transaction_index: "0".into(),
            pool: String::new(), mint: String::new(), event_id: String::new(),
            instruction_path: String::new(), identity_receipt_sha256: String::new(),
            path_receipt_sha256: String::new(), witness: json!({}),
            disposition: "CREATION_QUARANTINED".into(), reason_code: Some("MALFORMED_WITNESS".into()),
            receipt_sha256: String::new(),
        }.seal().unwrap();
        let mut pools = BTreeSet::new(); let mut decisions = BTreeSet::new(); let mut out = Vec::new();
        assert!(admit_creation(&creation, 10, 11, 11, &mut pools, &mut decisions, &mut out)
            .unwrap_err().to_string().contains("FORWARD_CREATION_QUARANTINED"));
        assert!(out.is_empty());
    }
}
