//! Hash-bound closure evidence for a filtered Yellowstone block update.
//!
//! A transaction subscription tells us what happened to arrive.  It does not
//! tell us that the source is finished delivering a slot.  A filtered block
//! update is the only source-side closure witness used by the bounded
//! diagnostic.  Finalized RPC remains authoritative for canonical ordering.

use std::{collections::BTreeSet, fs, path::Path};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::{
    canonical_capture_run_v1::ExpectedIdentity,
    canonical_state_v1::{hash_excluding_field, jcs_bytes},
};

pub const SOURCE_BLOCK_CLOSURE_SCHEMA: &str = "EDGESNIFFER_FILTERED_SOURCE_BLOCK_CLOSURE_V1";

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceBlockIdentity {
    pub signature: String,
    /// Provider telemetry only. Finalized RPC is the authoritative order.
    pub source_transaction_index: String,
}

impl SourceBlockIdentity {
    fn validate(&self) -> Result<()> {
        if self.signature.is_empty() || self.source_transaction_index.parse::<u64>().is_err() {
            bail!("SOURCE_BLOCK_IDENTITY_INVALID")
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceBlockClosure {
    pub schema: String,
    pub source_instance_id: String,
    pub continuity_epoch: String,
    pub relevance_policy_sha256: String,
    pub slot: String,
    pub blockhash: String,
    pub executed_transaction_count: String,
    pub source_block_update_sha256: String,
    /// Filtered identities after the source filter's vote/failed semantics.
    pub filtered_identities: Vec<SourceBlockIdentity>,
    pub closure_sha256: String,
}

impl SourceBlockClosure {
    pub fn seal(mut self) -> Result<Self> {
        self.schema = SOURCE_BLOCK_CLOSURE_SCHEMA.to_owned();
        self.filtered_identities.sort();
        self.closure_sha256 = hash_excluding_field(&self, "closureSha256")?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != SOURCE_BLOCK_CLOSURE_SCHEMA
            || self.source_instance_id.is_empty()
            || self.continuity_epoch.is_empty()
            || self.relevance_policy_sha256.len() != 64
            || self.slot.parse::<u64>().is_err()
            || self.blockhash.is_empty()
            || self.executed_transaction_count.parse::<u64>().is_err()
            || self.source_block_update_sha256.len() != 64
            || self.closure_sha256 != hash_excluding_field(self, "closureSha256")?
        {
            bail!("SOURCE_BLOCK_CLOSURE_INVALID")
        }
        let mut prior = None;
        for identity in &self.filtered_identities {
            identity.validate()?;
            if prior.as_ref() >= Some(&identity.signature) {
                bail!("SOURCE_BLOCK_CLOSURE_DUPLICATE_OR_UNSORTED_IDENTITY")
            }
            prior = Some(identity.signature.clone());
        }
        Ok(())
    }

    pub fn validate_against_finalized(
        &self,
        source_instance_id: &str,
        continuity_epoch: &str,
        relevance_policy_sha256: &str,
        slot: u64,
        finalized_blockhash: &str,
        expected: &[ExpectedIdentity],
    ) -> Result<()> {
        self.validate()?;
        if self.source_instance_id != source_instance_id
            || self.continuity_epoch != continuity_epoch
            || self.relevance_policy_sha256 != relevance_policy_sha256
            || self.slot != slot.to_string()
            || self.blockhash != finalized_blockhash
        {
            bail!("SOURCE_BLOCK_CLOSURE_BINDING_MISMATCH")
        }
        let expected = expected
            .iter()
            .filter(|identity| identity.slot == self.slot)
            .map(|identity| identity.signature.clone())
            .collect::<BTreeSet<_>>();
        let observed = self
            .filtered_identities
            .iter()
            .map(|identity| identity.signature.clone())
            .collect::<BTreeSet<_>>();
        if observed != expected {
            bail!("SOURCE_BLOCK_CLOSURE_IDENTITY_SET_MISMATCH")
        }
        Ok(())
    }

    pub fn write_atomic(&self, root: &Path) -> Result<()> {
        self.validate()?;
        fs::create_dir_all(root)?;
        let final_path = root.join(format!("source-block-closure-{}.json", self.slot));
        let temporary = root.join(format!(".source-block-closure-{}.tmp", self.slot));
        fs::write(&temporary, jcs_bytes(&serde_json::to_value(self)?)?)?;
        fs::rename(&temporary, &final_path)?;
        Ok(())
    }

    pub fn read(root: &Path, slot: u64) -> Result<Self> {
        let path = root.join(format!("source-block-closure-{slot}.json"));
        let output: Self = serde_json::from_slice(&fs::read(path)?)?;
        output.validate()?;
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn expected(slot: u64, signatures: &[&str]) -> Vec<ExpectedIdentity> {
        signatures
            .iter()
            .enumerate()
            .map(|(index, signature)| ExpectedIdentity {
                slot: slot.to_string(),
                signature: (*signature).to_owned(),
                transaction_index: index.to_string(),
            })
            .collect()
    }

    fn closure(slot: u64, blockhash: &str, signatures: &[(&str, u64)]) -> SourceBlockClosure {
        SourceBlockClosure {
            schema: String::new(),
            source_instance_id: "source".into(),
            continuity_epoch: "1".into(),
            relevance_policy_sha256: "a".repeat(64),
            slot: slot.to_string(),
            blockhash: blockhash.into(),
            executed_transaction_count: "500".into(),
            source_block_update_sha256: "b".repeat(64),
            filtered_identities: signatures
                .iter()
                .map(|(signature, index)| SourceBlockIdentity {
                    signature: (*signature).into(),
                    source_transaction_index: index.to_string(),
                })
                .collect(),
            closure_sha256: String::new(),
        }
        .seal()
        .unwrap()
    }

    #[test]
    fn source_index_difference_is_telemetry_not_canonical_order() {
        let closure = closure(10, "block", &[("a", 91), ("b", 4)]);
        closure
            .validate_against_finalized("source", "1", &"a".repeat(64), 10, "block", &expected(10, &["a", "b"]))
            .unwrap();
    }

    #[test]
    fn missing_extra_duplicate_and_blockhash_conflict_fail_closed() {
        let expected = expected(10, &["a", "b"]);
        let missing = closure(10, "block", &[("a", 0)]);
        assert!(missing.validate_against_finalized("source", "1", &"a".repeat(64), 10, "block", &expected).is_err());
        let extra = closure(10, "block", &[("a", 0), ("b", 1), ("c", 2)]);
        assert!(extra.validate_against_finalized("source", "1", &"a".repeat(64), 10, "block", &expected).is_err());
        let wrong_block = closure(10, "other", &[("a", 0), ("b", 1)]);
        assert!(wrong_block.validate_against_finalized("source", "1", &"a".repeat(64), 10, "block", &expected).is_err());
        let duplicate = SourceBlockClosure { filtered_identities: vec![
            SourceBlockIdentity { signature: "a".into(), source_transaction_index: "0".into() },
            SourceBlockIdentity { signature: "a".into(), source_transaction_index: "1".into() },
        ], ..closure(10, "block", &[]) }.seal().unwrap();
        assert!(duplicate.validate().is_err());
    }

    #[test]
    fn zero_relevant_slot_is_valid_only_with_a_bound_closure() {
        let closure = closure(10, "block", &[]);
        closure
            .validate_against_finalized("source", "1", &"a".repeat(64), 10, "block", &[])
            .unwrap();
    }

    #[test]
    fn atomic_round_trip_and_tamper_detection() {
        let root = tempdir().unwrap();
        let closure = closure(10, "block", &[("a", 1)]);
        closure.write_atomic(root.path()).unwrap();
        assert_eq!(SourceBlockClosure::read(root.path(), 10).unwrap(), closure);
        let path = root.path().join("source-block-closure-10.json");
        let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["blockhash"] = serde_json::Value::String("tampered".into());
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(SourceBlockClosure::read(root.path(), 10).is_err());
    }
}
