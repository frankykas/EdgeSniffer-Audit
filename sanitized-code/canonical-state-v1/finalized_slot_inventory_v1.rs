//! Bounded finalized slot-range inventory.
//!
//! `getBlocks(start, end, finalized)` is the authoritative produced-slot
//! inventory.  Each produced slot must separately have a matching finalized
//! `getBlock` reconciliation receipt.  Slots omitted by `getBlocks` are
//! explicitly retained as finalized-skipped, never inferred from filtered
//! Yellowstone silence.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::canonical_state_v1::hash_excluding_field;

pub const FINALIZED_SLOT_INVENTORY_SCHEMA: &str = "EDGESNIFFER_FINALIZED_SLOT_INVENTORY_V1";
pub const MAX_DIAGNOSTIC_RANGE_SLOTS: u64 = 16;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconciledProducedSlot {
    pub slot: String,
    pub block_identity: String,
    pub slot_reconciliation_receipt_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalizedSlotInventoryEntry {
    pub slot: String,
    pub disposition: String,
    pub block_identity: Option<String>,
    pub slot_reconciliation_receipt_sha256: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FinalizedSlotInventory {
    pub schema: String,
    pub source_instance_id: String,
    pub continuity_epoch: String,
    pub start_slot: String,
    pub end_slot: String,
    /// SHA-256 of the retained bounded finalized `getBlocks` response bytes.
    pub get_blocks_response_sha256: String,
    pub entries: Vec<FinalizedSlotInventoryEntry>,
    pub disposition: String,
    pub reason_code: Option<String>,
    pub inventory_sha256: String,
}

fn number(value: &str, label: &str) -> Result<u64> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) { bail!("{label}_INVALID") }
    value.parse().map_err(Into::into)
}
fn digest(value: &str, label: &str) -> Result<()> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) { bail!("{label}_INVALID") }
    Ok(())
}

impl FinalizedSlotInventory {
    pub fn seal(
        source_instance_id: &str,
        continuity_epoch: &str,
        start_slot: u64,
        end_slot: u64,
        get_blocks_response_sha256: &str,
        produced_slots: &[u64],
        reconciled: Vec<ReconciledProducedSlot>,
    ) -> Result<Self> {
        if source_instance_id.is_empty() || continuity_epoch.is_empty() || end_slot < start_slot { bail!("SLOT_INVENTORY_BINDING_INVALID") }
        let count = end_slot.checked_sub(start_slot).and_then(|value| value.checked_add(1)).context("SLOT_INVENTORY_RANGE_OVERFLOW")?;
        if count > MAX_DIAGNOSTIC_RANGE_SLOTS { bail!("SLOT_INVENTORY_DIAGNOSTIC_RANGE_CAP") }
        digest(get_blocks_response_sha256, "GET_BLOCKS_RESPONSE_SHA256")?;
        let mut produced = BTreeSet::new();
        for slot in produced_slots {
            if *slot < start_slot || *slot > end_slot || !produced.insert(*slot) { bail!("SLOT_INVENTORY_PRODUCED_SET_INVALID") }
        }
        let mut proofs = BTreeMap::new();
        for proof in reconciled {
            let slot = number(&proof.slot, "SLOT_INVENTORY_PROOF_SLOT")?;
            digest(&proof.slot_reconciliation_receipt_sha256, "SLOT_RECONCILIATION_RECEIPT_SHA256")?;
            if !produced.contains(&slot) || proof.block_identity.is_empty() || proofs.insert(slot, proof).is_some() {
                bail!("SLOT_INVENTORY_PROOF_INVALID")
            }
        }
        if proofs.len() != produced.len() { bail!("SLOT_INVENTORY_PRODUCED_SLOT_UNRECONCILED") }
        let mut entries = Vec::with_capacity(count as usize);
        for slot in start_slot..=end_slot {
            if let Some(proof) = proofs.remove(&slot) {
                entries.push(FinalizedSlotInventoryEntry { slot: slot.to_string(), disposition: "PRODUCED_RECONCILED".into(), block_identity: Some(proof.block_identity), slot_reconciliation_receipt_sha256: Some(proof.slot_reconciliation_receipt_sha256) });
            } else {
                entries.push(FinalizedSlotInventoryEntry { slot: slot.to_string(), disposition: "FINALIZED_SKIPPED".into(), block_identity: None, slot_reconciliation_receipt_sha256: None });
            }
        }
        let mut output = Self { schema: FINALIZED_SLOT_INVENTORY_SCHEMA.into(), source_instance_id: source_instance_id.into(), continuity_epoch: continuity_epoch.into(), start_slot: start_slot.to_string(), end_slot: end_slot.to_string(), get_blocks_response_sha256: get_blocks_response_sha256.into(), entries, disposition: "FINALIZED_RANGE_COMPLETE".into(), reason_code: None, inventory_sha256: String::new() };
        output.inventory_sha256 = hash_excluding_field(&output, "inventorySha256")?;
        output.validate()?;
        Ok(output)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != FINALIZED_SLOT_INVENTORY_SCHEMA || self.source_instance_id.is_empty() || self.continuity_epoch.is_empty() { bail!("SLOT_INVENTORY_SCHEMA_OR_BINDING_INVALID") }
        digest(&self.get_blocks_response_sha256, "GET_BLOCKS_RESPONSE_SHA256")?;
        if self.inventory_sha256 != hash_excluding_field(self, "inventorySha256")? { bail!("SLOT_INVENTORY_HASH_INVALID") }
        let start = number(&self.start_slot, "SLOT_INVENTORY_START")?;
        let end = number(&self.end_slot, "SLOT_INVENTORY_END")?;
        let count = end.checked_sub(start).and_then(|value| value.checked_add(1)).context("SLOT_INVENTORY_RANGE_INVALID")?;
        if count > MAX_DIAGNOSTIC_RANGE_SLOTS || self.entries.len() != count as usize || self.disposition != "FINALIZED_RANGE_COMPLETE" || self.reason_code.is_some() { bail!("SLOT_INVENTORY_INCOMPLETE") }
        for (offset, entry) in self.entries.iter().enumerate() {
            if entry.slot != (start + offset as u64).to_string() { bail!("SLOT_INVENTORY_ENTRY_ORDER_INVALID") }
            match entry.disposition.as_str() {
                "PRODUCED_RECONCILED" => {
                    if entry.block_identity.as_deref().unwrap_or_default().is_empty() { bail!("SLOT_INVENTORY_BLOCK_IDENTITY_MISSING") }
                    digest(entry.slot_reconciliation_receipt_sha256.as_deref().context("SLOT_INVENTORY_RECONCILIATION_RECEIPT_MISSING")?, "SLOT_RECONCILIATION_RECEIPT_SHA256")?;
                }
                "FINALIZED_SKIPPED" if entry.block_identity.is_none() && entry.slot_reconciliation_receipt_sha256.is_none() => {}
                _ => bail!("SLOT_INVENTORY_ENTRY_INVALID"),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const H: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    fn proof(slot: u64) -> ReconciledProducedSlot { ReconciledProducedSlot { slot: slot.to_string(), block_identity: format!("block-{slot}"), slot_reconciliation_receipt_sha256: H.into() } }
    #[test]
    fn produced_and_finalized_skipped_slots_are_explicitly_bound() {
        let inventory = FinalizedSlotInventory::seal("source", "epoch", 10, 13, H, &[10, 12], vec![proof(10), proof(12)]).unwrap();
        assert_eq!(inventory.entries[1].disposition, "FINALIZED_SKIPPED");
        inventory.validate().unwrap();
    }
    #[test]
    fn missing_produced_proof_duplicate_or_tamper_fails_closed() {
        assert!(FinalizedSlotInventory::seal("source", "epoch", 10, 10, H, &[10], vec![]).is_err());
        assert!(FinalizedSlotInventory::seal("source", "epoch", 10, 10, H, &[10,10], vec![proof(10)]).is_err());
        let mut inventory = FinalizedSlotInventory::seal("source", "epoch", 10, 10, H, &[10], vec![proof(10)]).unwrap();
        inventory.entries[0].block_identity = Some("tampered".into());
        assert!(inventory.validate().is_err());
    }
    #[test]
    fn range_budget_is_bounded() {
        assert!(FinalizedSlotInventory::seal("source", "epoch", 1, 17, H, &[], vec![]).is_err());
    }
}
