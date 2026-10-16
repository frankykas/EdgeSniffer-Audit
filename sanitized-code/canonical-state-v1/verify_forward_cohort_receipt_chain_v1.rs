//! Read-only verification of one bounded diagnostic receipt chain.
//! This never opens a cohort, writes a ledger, or evaluates research outputs.

use std::{env, path::PathBuf};

use anyhow::{Context, Result};
use pump_exact_evidence_ledger::{
    finalized_slot_inventory_v1::FinalizedSlotInventory,
    forward_new_pool_cohort_v1::seal_forward_cohort_from_persisted_receipt_chain,
};
use serde_json::json;

fn main() -> Result<()> {
    let root = PathBuf::from(env::var("CANONICAL_RECEIPT_ROOT").context("CANONICAL_RECEIPT_ROOT")?);
    let inventory = FinalizedSlotInventory::read_verified(&root)?;
    let start_slot: u64 = inventory.start_slot.parse().context("INVENTORY_START_SLOT")?;
    let boundary_slot = start_slot.checked_sub(1).context("INVENTORY_START_SLOT_ZERO")?;
    let cohort_id = env::var("FORWARD_COHORT_VERIFY_ID")
        .unwrap_or_else(|_| "READ_ONLY_DIAGNOSTIC_RECEIPT_VERIFICATION_V1".to_owned());

    // The consumer reads the persisted inventory and receipt files again; the
    // earlier inventory read is only to derive a boundary strictly before it.
    let ledger = seal_forward_cohort_from_persisted_receipt_chain(&root, &cohort_id, boundary_slot)?;
    let zero_qualifying_creations = ledger.pools.is_empty();
    println!(
        "{}",
        serde_json::to_string(&json!({
            "schema": "FORWARD_COHORT_RECEIPT_CHAIN_READ_ONLY_VERIFICATION_V1",
            "status": if zero_qualifying_creations {
                "RECEIPT_CHAIN_VALID_ZERO_QUALIFYING_CREATIONS"
            } else {
                "RECEIPT_CHAIN_VALID"
            },
            "receiptRoot": root,
            "startSlot": inventory.start_slot,
            "endSlot": inventory.end_slot,
            "sourceInstanceId": ledger.source_instance_id,
            "continuityEpoch": ledger.continuity_epoch,
            "inventorySha256": ledger.slot_inventory_sha256,
            "qualifyingCreationCount": ledger.pools.len(),
            "zeroQualifyingCreationsValid": zero_qualifying_creations,
            "ephemeralVerificationLedgerSha256": ledger.ledger_sha256,
        }))?
    );
    Ok(())
}
