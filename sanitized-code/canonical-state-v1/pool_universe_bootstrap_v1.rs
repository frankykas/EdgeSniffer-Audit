//! Immutable eligible-pool-universe bootstrap proof.
//!
//! A present-day program-account scan is identity evidence only: it cannot
//! reconstruct whether a pool was eligible or had an observed buy before the
//! scan.  This module therefore has no optimistic mode.  It can issue a
//! UNIVERSE_COMPLETE proof only from a contiguous, finalized historical event
//! range whose lower boundary is itself proved.

use std::collections::BTreeMap;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use crate::canonical_state_v1::hash_excluding_field;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum LifetimeKnowledge { Known(bool), Unknown }

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PoolUniverseRecord {
    pub pool: String,
    pub eligible_at_boundary: bool,
    pub ever_observed_buy: LifetimeKnowledge,
    pub first_observed_sequence: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PoolUniverseBootstrap {
    pub schema: String,
    pub epoch_id: String,
    pub coverage_start_sequence: String,
    pub boundary_sequence: String,
    pub finalized_range_proof_sha256: String,
    pub source_kind: String,
    pub pools: Vec<PoolUniverseRecord>,
    pub universe_complete: bool,
    pub reason_code: Option<String>,
    pub proof_sha256: String,
}

/// The caller supplies only already-reconciled canonical records.  A live
/// filtered stream, a current getProgramAccounts response, or a self-declared
/// warm-up never satisfies `FULL_FINALIZED_HISTORY_REPLAY`.
pub fn seal_bootstrap(
    epoch_id: &str,
    coverage_start_sequence: u64,
    boundary_sequence: u64,
    finalized_range_proof_sha256: &str,
    source_kind: &str,
    pools: Vec<PoolUniverseRecord>,
) -> Result<PoolUniverseBootstrap> {
    if epoch_id.is_empty() || finalized_range_proof_sha256.is_empty() || boundary_sequence < coverage_start_sequence {
        bail!("UNIVERSE_BOOTSTRAP_INPUT_INVALID")
    }
    let mut seen = BTreeMap::new();
    for pool in &pools {
        let first = pool.first_observed_sequence.parse::<u64>().map_err(|_| anyhow::anyhow!("UNIVERSE_BOOTSTRAP_SEQUENCE_INVALID"))?;
        if pool.pool.is_empty() || first < coverage_start_sequence || first > boundary_sequence || seen.insert(&pool.pool, ()).is_some() {
            bail!("UNIVERSE_BOOTSTRAP_POOL_RECORD_INVALID")
        }
    }
    let complete = source_kind == "FULL_FINALIZED_HISTORY_REPLAY"
        && pools.iter().all(|pool| matches!(pool.ever_observed_buy, LifetimeKnowledge::Known(_)));
    let reason = (!complete).then(|| {
        if source_kind != "FULL_FINALIZED_HISTORY_REPLAY" { "UNIVERSE_INCOMPLETE_SOURCE".to_owned() }
        else { "UNIVERSE_INCOMPLETE_LIFETIME_STATE".to_owned() }
    });
    let mut proof = PoolUniverseBootstrap {
        schema: "CANONICAL_POOL_UNIVERSE_BOOTSTRAP_V1".to_owned(), epoch_id: epoch_id.to_owned(),
        coverage_start_sequence: coverage_start_sequence.to_string(), boundary_sequence: boundary_sequence.to_string(), finalized_range_proof_sha256: finalized_range_proof_sha256.to_owned(),
        source_kind: source_kind.to_owned(), pools, universe_complete: complete, reason_code: reason, proof_sha256: String::new(),
    };
    proof.proof_sha256 = hash_excluding_field(&proof, "proof_sha256")?;
    Ok(proof)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pool(state: LifetimeKnowledge) -> PoolUniverseRecord { PoolUniverseRecord { pool:"pool-a".into(), eligible_at_boundary:true, ever_observed_buy:state, first_observed_sequence:"10".into() } }
    #[test] fn account_snapshot_and_unknown_lifetime_fail_closed() {
        let now = seal_bootstrap("e", 1, 10, "proof", "PROGRAM_ACCOUNT_SNAPSHOT", vec![pool(LifetimeKnowledge::Known(false))]).unwrap();
        assert!(!now.universe_complete); assert_eq!(now.reason_code.as_deref(), Some("UNIVERSE_INCOMPLETE_SOURCE"));
        let unknown = seal_bootstrap("e", 1, 10, "proof", "FULL_FINALIZED_HISTORY_REPLAY", vec![pool(LifetimeKnowledge::Unknown)]).unwrap();
        assert!(!unknown.universe_complete); assert_eq!(unknown.reason_code.as_deref(), Some("UNIVERSE_INCOMPLETE_LIFETIME_STATE"));
    }
    #[test] fn complete_requires_historical_replay_and_is_hash_stable() {
        let a = seal_bootstrap("e", 1, 10, "proof", "FULL_FINALIZED_HISTORY_REPLAY", vec![pool(LifetimeKnowledge::Known(true))]).unwrap();
        let b = seal_bootstrap("e", 1, 10, "proof", "FULL_FINALIZED_HISTORY_REPLAY", vec![pool(LifetimeKnowledge::Known(true))]).unwrap();
        assert!(a.universe_complete); assert_eq!(a.proof_sha256, b.proof_sha256);
    }
}
