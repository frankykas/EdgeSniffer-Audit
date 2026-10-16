//! Restricted forward-only new-pool cohort proof.
//!
//! This is intentionally not a replacement for the full-universe protocol.
//! It admits only canonical Pump -> PumpSwap migrations created after a
//! finalized boundary, and only while every slot and creation witness is
//! continuously finalized/reconciled.

use std::collections::BTreeMap;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use crate::canonical_state_v1::hash_excluding_field;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CreationWitness {
    pub pool: String, pub mint: String, pub slot: String, pub signature: String,
    pub instruction_path: String, pub create_pool_event_sha256: String,
    pub canonical_pump_migration: bool, pub finalized_reconciled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ForwardCohortLedger {
    pub schema: String, pub cohort_id: String, pub boundary_slot: String,
    pub last_finalized_slot: String, pub source_epoch: String,
    pub continuity_intact: bool, pub slots_complete: bool,
    pub pools: Vec<CreationWitness>, pub disposition: String,
    pub reason_code: Option<String>, pub ledger_sha256: String,
}

pub fn seal_forward_cohort(
    cohort_id:&str, boundary_slot:u64, last_finalized_slot:u64, source_epoch:&str,
    continuity_intact:bool, slots_complete:bool, pools:Vec<CreationWitness>,
) -> Result<ForwardCohortLedger> {
    if cohort_id.is_empty() || source_epoch.is_empty() || last_finalized_slot < boundary_slot { bail!("FORWARD_COHORT_INPUT_INVALID") }
    let mut seen=BTreeMap::new();
    let mut reason=None;
    for pool in &pools {
        let slot=pool.slot.parse::<u64>().map_err(|_|anyhow::anyhow!("FORWARD_POOL_SLOT_INVALID"))?;
        if pool.pool.is_empty() || pool.mint.is_empty() || pool.signature.is_empty() || pool.instruction_path.is_empty() || pool.create_pool_event_sha256.is_empty() || slot <= boundary_slot || !pool.canonical_pump_migration || !pool.finalized_reconciled || seen.insert(&pool.pool,()).is_some() { reason=Some("POOL_CREATION_WITNESS_INVALID_OR_INCOMPLETE".to_owned()); break; }
    }
    if !continuity_intact { reason=Some("SOURCE_CONTINUITY_BROKEN".into()); }
    if !slots_complete { reason=Some("FINALIZED_SLOT_COVERAGE_INCOMPLETE".into()); }
    let disposition=if reason.is_some(){"FORWARD_COHORT_QUARANTINED"}else{"FORWARD_COHORT_COMPLETE_RESTRICTED"}.to_owned();
    let mut ledger=ForwardCohortLedger { schema:"CANONICAL_FORWARD_NEW_POOL_COHORT_V1".into(), cohort_id:cohort_id.into(), boundary_slot:boundary_slot.to_string(),last_finalized_slot:last_finalized_slot.to_string(),source_epoch:source_epoch.into(),continuity_intact,slots_complete,pools,disposition,reason_code:reason,ledger_sha256:String::new()};
    ledger.ledger_sha256=hash_excluding_field(&ledger,"ledger_sha256")?; Ok(ledger)
}

#[cfg(test)]
mod tests { use super::*;
 fn witness(slot:&str)->CreationWitness{CreationWitness{pool:"pool".into(),mint:"mint".into(),slot:slot.into(),signature:"sig".into(),instruction_path:"outer:00001/inner:00002/stack:002".into(),create_pool_event_sha256:"event".into(),canonical_pump_migration:true,finalized_reconciled:true}}
 #[test] fn only_post_boundary_complete_migrations_admit(){let x=seal_forward_cohort("c",10,12,"e",true,true,vec![witness("11")]).unwrap();assert_eq!(x.disposition,"FORWARD_COHORT_COMPLETE_RESTRICTED");}
 #[test] fn gaps_and_preboundary_pools_quarantine(){let gap=seal_forward_cohort("c",10,12,"e",false,true,vec![witness("11")]).unwrap();assert_eq!(gap.reason_code.as_deref(),Some("SOURCE_CONTINUITY_BROKEN"));let pre=seal_forward_cohort("c",10,12,"e",true,true,vec![witness("10")]).unwrap();assert!(pre.reason_code.is_some());}
}
