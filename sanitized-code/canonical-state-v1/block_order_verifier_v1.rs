//! Offline-only bounded two-stage Solana block-order verifier.
//!
//! Stage 1 reduces a complete provider block to a compact immutable inventory.
//! Stage 2 reconciles filtered full-meta transactions to that inventory.  A
//! slot cannot release scientific state unless the complete set of relevant
//! transaction identities is present, each record carries a verified ordered
//! instruction path, and all deferred identity resolution has completed.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::canonical_state_v1::{hash_excluding_field, jcs_bytes};

pub const BLOCK_PROOF_SCHEMA: &str = "EDGESNIFFER_BLOCK_ORDER_PROOF_V1";
pub const BLOCK_VERIFIER_CHECKPOINT_SCHEMA: &str = "EDGESNIFFER_BLOCK_ORDER_VERIFIER_CHECKPOINT_V1";
pub const MAX_BLOCK_TRANSACTIONS: usize = 16_384;
pub const MAX_PENDING_SLOTS: usize = 32;
pub const MAX_PENDING_FILTERED_PER_SLOT: usize = 4_096;

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionIdentity {
    pub transaction_index: String,
    pub signature: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteBlockInput {
    pub slot: String,
    pub block_identity: String,
    pub executed_transaction_count: String,
    /// This Boolean is produced by a frozen account-key/program discriminator,
    /// not by decoding pool state or a wallet/teacher identity.
    pub transactions: Vec<BlockTransactionInput>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockTransactionInput {
    pub transaction_index: String,
    pub signature: String,
    pub relevant_to_canonical_state: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockOrderProof {
    pub schema: String,
    pub slot: String,
    pub block_identity: String,
    pub executed_transaction_count: String,
    pub transactions: Vec<TransactionIdentity>,
    pub relevant_transactions: Vec<TransactionIdentity>,
    pub proof_sha256: String,
}

impl BlockOrderProof {
    pub fn from_complete_block(input: CompleteBlockInput) -> Result<Self> {
        let expected: usize = input.executed_transaction_count.parse().context("invalid executed transaction count")?;
        if input.transactions.len() != expected { bail!("BLOCK_INVENTORY_INCOMPLETE") }
        if expected > MAX_BLOCK_TRANSACTIONS { bail!("BLOCK_PROOF_MEMORY_CAP_REACHED") }
        let mut all = Vec::with_capacity(expected);
        let mut relevant = Vec::new();
        let mut seen_indexes = BTreeSet::new(); let mut seen_signatures = BTreeSet::new();
        for tx in input.transactions {
            let index: usize = tx.transaction_index.parse().context("invalid transaction index")?;
            if index >= expected || !seen_indexes.insert(index) || !seen_signatures.insert(tx.signature.clone()) { bail!("BLOCK_ORDER_CONFLICT") }
            let identity = TransactionIdentity { transaction_index: tx.transaction_index, signature: tx.signature };
            if tx.relevant_to_canonical_state { relevant.push(identity.clone()); }
            all.push(identity);
        }
        all.sort(); relevant.sort();
        for (expected_index, actual) in all.iter().enumerate() {
            if actual.transaction_index != expected_index.to_string() { bail!("BLOCK_INDEX_INVENTORY_NOT_CONTIGUOUS") }
        }
        let mut proof = Self { schema: BLOCK_PROOF_SCHEMA.to_owned(), slot: input.slot, block_identity: input.block_identity, executed_transaction_count: input.executed_transaction_count, transactions: all, relevant_transactions: relevant, proof_sha256: String::new() };
        proof.proof_sha256 = hash_excluding_field(&proof, "proofSha256")?;
        Ok(proof)
    }

    pub fn verify(&self) -> Result<()> {
        if self.schema != BLOCK_PROOF_SCHEMA { bail!("unsupported block proof schema") }
        let expected = hash_excluding_field(self, "proofSha256")?;
        if expected != self.proof_sha256 { bail!("BLOCK_PROOF_HASH_INVALID") }
        let count: usize = self.executed_transaction_count.parse().context("invalid proof count")?;
        if self.transactions.len() != count || count > MAX_BLOCK_TRANSACTIONS { bail!("BLOCK_PROOF_INVENTORY_INVALID") }
        for (index, tx) in self.transactions.iter().enumerate() {
            if tx.transaction_index != index.to_string() { bail!("BLOCK_PROOF_ORDER_INVALID") }
        }
        if self.relevant_transactions.iter().any(|tx| !self.transactions.binary_search(tx).is_ok()) { bail!("BLOCK_PROOF_RELEVANT_NOT_IN_INVENTORY") }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstructionPath(pub Vec<String>);

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilteredTransaction {
    pub slot: String,
    pub transaction_index: String,
    pub signature: String,
    /// Paths are decoder provenance, e.g. outer instruction / inner path /
    /// event ordinal. They must be authoritative and strictly sorted.
    pub instruction_paths: Vec<InstructionPath>,
    pub raw_frame_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleasedTransaction {
    pub slot: String,
    pub transaction_index: String,
    pub signature: String,
    pub instruction_paths: Vec<InstructionPath>,
    pub raw_frame_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockVerifierCheckpoint {
    pub schema: String,
    pub proofs: Vec<BlockOrderProof>,
    pub received: Vec<FilteredTransaction>,
    pub resolved: Vec<TransactionIdentity>,
    pub checkpoint_sha256: String,
}

#[derive(Default)]
pub struct BlockOrderVerifier {
    proofs: BTreeMap<String, BlockOrderProof>,
    received: BTreeMap<String, BTreeMap<TransactionIdentity, FilteredTransaction>>,
    resolved: BTreeSet<(String, TransactionIdentity)>,
}

impl BlockOrderVerifier {
    pub fn insert_proof(&mut self, proof: BlockOrderProof) -> Result<()> {
        proof.verify()?;
        if self.proofs.len() >= MAX_PENDING_SLOTS && !self.proofs.contains_key(&proof.slot) { bail!("PENDING_SLOT_MEMORY_CAP_REACHED") }
        if let Some(existing) = self.proofs.get(&proof.slot) {
            if existing.proof_sha256 != proof.proof_sha256 { bail!("CONFLICTING_BLOCK_IDENTITY") }
            return Ok(());
        }
        self.proofs.insert(proof.slot.clone(), proof);
        Ok(())
    }

    pub fn record_filtered_transaction(&mut self, transaction: FilteredTransaction) -> Result<()> {
        validate_paths(&transaction.instruction_paths)?;
        let proof = self.proofs.get(&transaction.slot).context("MISSING_BLOCK_PROOF")?;
        let identity = TransactionIdentity { transaction_index: transaction.transaction_index.clone(), signature: transaction.signature.clone() };
        if proof.relevant_transactions.binary_search(&identity).is_err() { bail!("FILTERED_TRANSACTION_NOT_IN_RELEVANT_PROOF") }
        let slot = self.received.entry(transaction.slot.clone()).or_default();
        if slot.len() >= MAX_PENDING_FILTERED_PER_SLOT && !slot.contains_key(&identity) { bail!("PENDING_TRANSACTION_MEMORY_CAP_REACHED") }
        if let Some(previous) = slot.get(&identity) {
            if previous.raw_frame_sha256 != transaction.raw_frame_sha256 || previous.instruction_paths != transaction.instruction_paths { bail!("DUPLICATE_TRANSACTION_CONFLICT") }
            return Ok(());
        }
        slot.insert(identity, transaction);
        Ok(())
    }

    /// Identity resolution is deliberately separate from receive/ordering. A
    /// resolved identity unlocks nothing until the whole relevant slot set is
    /// complete, so slow RPC can delay but never reorder a scientific slot.
    pub fn mark_identity_resolved(&mut self, slot: &str, transaction_index: &str, signature: &str) -> Result<()> {
        let identity = TransactionIdentity { transaction_index: transaction_index.to_owned(), signature: signature.to_owned() };
        let proof = self.proofs.get(slot).context("MISSING_BLOCK_PROOF")?;
        if proof.relevant_transactions.binary_search(&identity).is_err() { bail!("RESOLUTION_FOR_UNPROVEN_TRANSACTION") }
        self.resolved.insert((slot.to_owned(), identity));
        Ok(())
    }

    pub fn release_slot(&mut self, slot: &str) -> Result<Vec<ReleasedTransaction>> {
        let proof = self.proofs.get(slot).context("MISSING_BLOCK_PROOF")?.clone();
        let received = self.received.get(slot).context("RELEVANT_TRANSACTION_SET_MISSING")?;
        let actual = received.keys().cloned().collect::<Vec<_>>();
        if actual != proof.relevant_transactions { bail!("RELEVANT_TRANSACTION_SET_INCOMPLETE") }
        for identity in &proof.relevant_transactions {
            if !self.resolved.contains(&(slot.to_owned(), identity.clone())) { bail!("POOL_IDENTITY_UNRESOLVED") }
        }
        let mut released = received.values().map(|transaction| ReleasedTransaction { slot: transaction.slot.clone(), transaction_index: transaction.transaction_index.clone(), signature: transaction.signature.clone(), instruction_paths: transaction.instruction_paths.clone(), raw_frame_sha256: transaction.raw_frame_sha256.clone() }).collect::<Vec<_>>();
        released.sort_by_key(|tx| (tx.transaction_index.parse::<u64>().unwrap_or(u64::MAX), tx.instruction_paths.clone()));
        self.proofs.remove(slot); self.received.remove(slot); self.resolved.retain(|(candidate_slot, _)| candidate_slot != slot);
        Ok(released)
    }

    pub fn checkpoint(&self) -> Result<BlockVerifierCheckpoint> {
        let mut received = self.received.values().flat_map(|records| records.values().cloned()).collect::<Vec<_>>();
        received.sort_by_key(|tx| (tx.slot.clone(), tx.transaction_index.clone(), tx.signature.clone()));
        let mut resolved = self.resolved.iter().map(|(_, identity)| identity.clone()).collect::<Vec<_>>(); resolved.sort(); resolved.dedup();
        let mut checkpoint = BlockVerifierCheckpoint { schema: BLOCK_VERIFIER_CHECKPOINT_SCHEMA.to_owned(), proofs: self.proofs.values().cloned().collect(), received, resolved, checkpoint_sha256: String::new() };
        checkpoint.checkpoint_sha256 = hash_excluding_field(&checkpoint, "checkpointSha256")?;
        Ok(checkpoint)
    }

    pub fn restore(checkpoint: BlockVerifierCheckpoint) -> Result<Self> {
        if checkpoint.schema != BLOCK_VERIFIER_CHECKPOINT_SCHEMA || hash_excluding_field(&checkpoint, "checkpointSha256")? != checkpoint.checkpoint_sha256 { bail!("BLOCK_VERIFIER_CHECKPOINT_INVALID") }
        let mut verifier = Self::default();
        for proof in checkpoint.proofs { verifier.insert_proof(proof)?; }
        for transaction in checkpoint.received { verifier.record_filtered_transaction(transaction)?; }
        // Resolution identities are re-associated only where they are still
        // proven present; malformed checkpoint contents fail closed.
        for identity in checkpoint.resolved {
            let slots = verifier.proofs.iter().filter(|(_, proof)| proof.relevant_transactions.binary_search(&identity).is_ok()).map(|(slot, _)| slot.clone()).collect::<Vec<_>>();
            if slots.len() != 1 { bail!("CHECKPOINT_RESOLUTION_AMBIGUOUS") }
            verifier.resolved.insert((slots[0].clone(), identity));
        }
        Ok(verifier)
    }
}

fn validate_paths(paths: &[InstructionPath]) -> Result<()> {
    if paths.is_empty() { bail!("INSTRUCTION_PATH_UNRESOLVED") }
    let mut prior: Option<&InstructionPath> = None;
    for path in paths {
        if path.0.is_empty() || path.0.iter().any(|part| part.parse::<u64>().is_err()) { bail!("INSTRUCTION_PATH_INVALID") }
        if prior.is_some_and(|previous| previous >= path) { bail!("INSTRUCTION_PATH_NOT_STRICTLY_ORDERED") }
        prior = Some(path);
    }
    Ok(())
}

pub fn checkpoint_canonical_bytes(checkpoint: &BlockVerifierCheckpoint) -> Result<Vec<u8>> { jcs_bytes(&serde_json::to_value(checkpoint)?) }

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(index: u64) -> BlockTransactionInput { BlockTransactionInput { transaction_index: index.to_string(), signature: format!("sig-{index}"), relevant_to_canonical_state: index % 2 == 0 } }
    fn proof(slot: &str) -> BlockOrderProof { BlockOrderProof::from_complete_block(CompleteBlockInput { slot: slot.to_owned(), block_identity: format!("block-{slot}"), executed_transaction_count: "4".to_owned(), transactions: vec![identity(0), identity(1), identity(2), identity(3)] }).unwrap() }
    fn filtered(slot: &str, index: u64) -> FilteredTransaction { FilteredTransaction { slot: slot.to_owned(), transaction_index: index.to_string(), signature: format!("sig-{index}"), instruction_paths: vec![InstructionPath(vec!["0".into(), index.to_string(), "0".into()])], raw_frame_sha256: format!("raw-{index}") } }

    #[test]
    fn reversed_arrivals_release_by_block_order_only_when_complete() {
        let mut verifier = BlockOrderVerifier::default(); verifier.insert_proof(proof("10")).unwrap();
        verifier.record_filtered_transaction(filtered("10", 2)).unwrap(); verifier.mark_identity_resolved("10", "2", "sig-2").unwrap();
        assert!(verifier.release_slot("10").unwrap_err().to_string().contains("INCOMPLETE"));
        verifier.record_filtered_transaction(filtered("10", 0)).unwrap(); verifier.mark_identity_resolved("10", "0", "sig-0").unwrap();
        let released = verifier.release_slot("10").unwrap(); assert_eq!(released.iter().map(|tx| tx.transaction_index.as_str()).collect::<Vec<_>>(), vec!["0", "2"]);
    }

    #[test]
    fn rejects_late_missing_duplicate_and_conflicting_inputs() {
        let mut verifier = BlockOrderVerifier::default(); verifier.insert_proof(proof("10")).unwrap();
        assert!(verifier.record_filtered_transaction(filtered("11", 0)).unwrap_err().to_string().contains("MISSING_BLOCK_PROOF"));
        let mut bad = filtered("10", 1); assert!(verifier.record_filtered_transaction(bad.clone()).unwrap_err().to_string().contains("NOT_IN_RELEVANT"));
        verifier.record_filtered_transaction(filtered("10", 0)).unwrap(); bad = filtered("10", 0); bad.raw_frame_sha256 = "different".into();
        assert!(verifier.record_filtered_transaction(bad).unwrap_err().to_string().contains("DUPLICATE"));
        let mut conflict = proof("10"); conflict.block_identity = "fork".into(); conflict.proof_sha256 = hash_excluding_field(&conflict, "proofSha256").unwrap();
        assert!(verifier.insert_proof(conflict).unwrap_err().to_string().contains("CONFLICTING"));
    }

    #[test]
    fn unresolved_instruction_and_slow_identity_resolution_cannot_mutate_state() {
        let mut verifier = BlockOrderVerifier::default(); verifier.insert_proof(proof("10")).unwrap();
        let mut invalid = filtered("10", 0); invalid.instruction_paths.clear(); assert!(verifier.record_filtered_transaction(invalid).unwrap_err().to_string().contains("INSTRUCTION_PATH"));
        verifier.record_filtered_transaction(filtered("10", 0)).unwrap(); verifier.record_filtered_transaction(filtered("10", 2)).unwrap();
        assert!(verifier.release_slot("10").unwrap_err().to_string().contains("UNRESOLVED"));
        verifier.mark_identity_resolved("10", "0", "sig-0").unwrap(); verifier.mark_identity_resolved("10", "2", "sig-2").unwrap(); assert_eq!(verifier.release_slot("10").unwrap().len(), 2);
    }

    #[test]
    fn restart_corrupt_proof_and_memory_pressure_fail_closed() {
        let mut verifier = BlockOrderVerifier::default(); verifier.insert_proof(proof("10")).unwrap(); verifier.record_filtered_transaction(filtered("10", 0)).unwrap();
        let checkpoint = verifier.checkpoint().unwrap(); let bytes = checkpoint_canonical_bytes(&checkpoint).unwrap(); assert!(!bytes.is_empty());
        let mut recovered = BlockOrderVerifier::restore(checkpoint.clone()).unwrap(); assert!(recovered.release_slot("10").unwrap_err().to_string().contains("INCOMPLETE"));
        let mut corrupt = checkpoint; corrupt.checkpoint_sha256 = "00".repeat(32); assert!(BlockOrderVerifier::restore(corrupt).is_err());
        let too_many = CompleteBlockInput { slot: "99".into(), block_identity: "b".into(), executed_transaction_count: (MAX_BLOCK_TRANSACTIONS + 1).to_string(), transactions: (0..=MAX_BLOCK_TRANSACTIONS).map(|index| BlockTransactionInput { transaction_index: index.to_string(), signature: format!("s{index}"), relevant_to_canonical_state: false }).collect() };
        assert!(BlockOrderProof::from_complete_block(too_many).unwrap_err().to_string().contains("MEMORY_CAP"));
    }
}
