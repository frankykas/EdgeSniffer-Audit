//! Offline-only authoritative Pump/PumpSwap instruction-path decoder.
//!
//! A semantic event is accepted only when complete transaction message and
//! execution metadata identify exactly one matching invocation. Log messages
//! corroborate that the event was emitted while the expected program was
//! active, but never create an instruction path by themselves.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

pub const PUMP_PROGRAM: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
pub const PUMPSWAP_PROGRAM: &str = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CompiledInstructionInput {
    pub program_id_index: usize,
    pub account_indexes: Vec<usize>,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InnerInstructionGroupInput {
    pub parent_outer_index: usize,
    /// Metadata preserves this vector in execution order. It is the only
    /// admissible inner ordinal; arrival order is never consulted.
    pub instructions: Vec<InnerInstructionInput>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct InnerInstructionInput {
    pub program_id_index: usize,
    pub account_indexes: Vec<usize>,
    pub data: Vec<u8>,
    pub stack_height: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FullTransactionInput {
    pub slot: u64,
    /// Must come from a finalized block proof, not Yellowstone arrival.
    pub finalized_transaction_index: Option<u64>,
    pub signature: String,
    pub succeeded: bool,
    pub static_account_keys: Vec<String>,
    pub loaded_writable_account_keys: Vec<String>,
    pub loaded_readonly_account_keys: Vec<String>,
    pub outer_instructions: Vec<CompiledInstructionInput>,
    pub inner_instruction_groups: Vec<InnerInstructionGroupInput>,
    pub log_messages: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SemanticEventInput {
    /// The already-decoded event must name the program which emitted it.
    pub expected_program_id: String,
    /// Canonical Pool identity from the event. It is used only to match the
    /// event to an invocation whose resolved accounts include that Pool.
    pub pool: String,
    /// Event provenance in TransactionStatusMeta.log_messages. This is used
    /// to verify active program nesting, never to derive instruction order.
    pub log_index: usize,
    /// Frozen invocation discriminator(s) supplied by the supported semantic
    /// decoder. A missing discriminator is unsupported rather than guessed.
    pub accepted_instruction_discriminators: Vec<Vec<u8>>,
    pub semantic_kind: String,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct InstructionPath {
    pub outer_index: usize,
    pub inner_index: Option<usize>,
    pub stack_height: Option<usize>,
}

impl InstructionPath {
    pub fn render(&self) -> String {
        match (self.inner_index, self.stack_height) {
            (None, None) => format!("outer:{:05}", self.outer_index),
            (Some(inner), Some(height)) => format!("outer:{:05}/inner:{:05}/stack:{:03}", self.outer_index, inner, height),
            _ => format!("outer:{:05}/inner:INVALID", self.outer_index),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DecodedSemanticEvent {
    pub signature: String,
    pub canonical_order: CanonicalOrderTuple,
    pub semantic_kind: String,
    pub pool: String,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct CanonicalOrderTuple {
    pub slot: u64,
    pub finalized_transaction_index: u64,
    pub instruction_path: InstructionPath,
    pub semantic_ordinal: usize,
}

#[derive(Clone, Debug)]
struct Invocation {
    path: InstructionPath,
    program_id: String,
    accounts: BTreeSet<String>,
    data: Vec<u8>,
}

#[derive(Clone, Debug)]
struct LogStackEntry { program_id: String }

fn all_account_keys(input: &FullTransactionInput) -> Vec<String> {
    input.static_account_keys.iter()
        .chain(input.loaded_writable_account_keys.iter())
        .chain(input.loaded_readonly_account_keys.iter())
        .cloned().collect()
}

fn resolve_invocation(keys: &[String], path: InstructionPath, program_id_index: usize, account_indexes: &[usize], data: &[u8]) -> Result<Invocation> {
    let program_id = keys.get(program_id_index).context("INSTRUCTION_PROGRAM_INDEX_OUT_OF_RANGE")?.clone();
    let mut accounts = BTreeSet::new();
    for account_index in account_indexes {
        accounts.insert(keys.get(*account_index).context("INSTRUCTION_ACCOUNT_INDEX_OUT_OF_RANGE")?.clone());
    }
    Ok(Invocation { path, program_id, accounts, data: data.to_vec() })
}

fn target_program(program_id: &str) -> bool { program_id == PUMP_PROGRAM || program_id == PUMPSWAP_PROGRAM }

fn collect_invocations(input: &FullTransactionInput) -> Result<Vec<Invocation>> {
    let keys = all_account_keys(input);
    if keys.is_empty() { bail!("ACCOUNT_KEY_TABLE_EMPTY") }
    let mut by_parent = BTreeMap::new();
    for group in &input.inner_instruction_groups {
        if group.parent_outer_index >= input.outer_instructions.len() || by_parent.insert(group.parent_outer_index, group).is_some() {
            bail!("INNER_INSTRUCTION_PARENT_AMBIGUOUS")
        }
    }
    let mut out = Vec::new();
    for (outer_index, outer) in input.outer_instructions.iter().enumerate() {
        let outer_path = InstructionPath { outer_index, inner_index: None, stack_height: None };
        let outer_invocation = resolve_invocation(&keys, outer_path, outer.program_id_index, &outer.account_indexes, &outer.data)?;
        if target_program(&outer_invocation.program_id) { out.push(outer_invocation); }
        if let Some(group) = by_parent.get(&outer_index) {
            for (inner_index, inner) in group.instructions.iter().enumerate() {
                // A missing stack height prevents reconstruction of a complete
                // CPI path. It must not silently collapse to the parent path.
                let stack_height = inner.stack_height.context("INNER_INSTRUCTION_STACK_HEIGHT_MISSING")?;
                if stack_height < 2 { bail!("INNER_INSTRUCTION_STACK_HEIGHT_INVALID") }
                let path = InstructionPath { outer_index, inner_index: Some(inner_index), stack_height: Some(stack_height) };
                let invocation = resolve_invocation(&keys, path, inner.program_id_index, &inner.account_indexes, &inner.data)?;
                if target_program(&invocation.program_id) { out.push(invocation); }
            }
        }
    }
    Ok(out)
}

/// Returns the active program for every log index. A malformed invoke/return
/// stack is rejected. This is evidence that the decoded event belonged to the
/// claimed program, not a substitute for message/metadata invocation mapping.
fn active_program_at_logs(logs: &[String]) -> Result<Vec<Option<String>>> {
    let mut stack: Vec<LogStackEntry> = Vec::new();
    let mut active = Vec::with_capacity(logs.len());
    for log in logs {
        if let Some((program, depth)) = parse_invoke(log) {
            if depth == 0 || depth != stack.len() + 1 { bail!("LOG_STACK_DEPTH_AMBIGUOUS") }
            stack.push(LogStackEntry { program_id: program });
            active.push(stack.last().map(|entry| entry.program_id.clone()));
            continue;
        }
        if let Some(program) = parse_return(log) {
            let top = stack.pop().context("LOG_STACK_UNDERFLOW")?;
            if top.program_id != program { bail!("LOG_STACK_PROGRAM_MISMATCH") }
            active.push(Some(program));
            continue;
        }
        active.push(stack.last().map(|entry| entry.program_id.clone()));
    }
    if !stack.is_empty() { bail!("LOG_STACK_UNCLOSED") }
    Ok(active)
}

fn parse_invoke(log: &str) -> Option<(String, usize)> {
    let rest = log.strip_prefix("Program ")?;
    let (program, suffix) = rest.split_once(" invoke [")?;
    let depth = suffix.strip_suffix(']')?.parse().ok()?;
    Some((program.to_owned(), depth))
}

fn parse_return(log: &str) -> Option<String> {
    let rest = log.strip_prefix("Program ")?;
    let (program, suffix) = rest.split_once(' ')?;
    (suffix == "success" || suffix == "failed: custom program error").then(|| program.to_owned())
}

fn starts_with_any(data: &[u8], allowed: &[Vec<u8>]) -> bool {
    !allowed.is_empty() && allowed.iter().all(|value| !value.is_empty()).then_some(())
        .is_some_and(|_| allowed.iter().any(|discriminator| data.starts_with(discriminator)))
}

/// Decodes only deterministic, complete instruction/event mappings. `Ok` is
/// never a permission to mutate scientific state by itself; callers must also
/// have finalized interval and candidate-universe proofs.
pub fn decode_instruction_paths(input: &FullTransactionInput, events: &[SemanticEventInput]) -> Result<Vec<DecodedSemanticEvent>> {
    if !input.succeeded { bail!("FAILED_TRANSACTION_NO_SCIENTIFIC_STATE") }
    let transaction_index = input.finalized_transaction_index.context("FINALIZED_TRANSACTION_INDEX_MISSING")?;
    if input.signature.is_empty() { bail!("TRANSACTION_SIGNATURE_MISSING") }
    let invocations = collect_invocations(input)?;
    let active_programs = active_program_at_logs(&input.log_messages)?;
    let mut emitted = Vec::new();
    let mut used_paths = BTreeSet::new();
    let mut ordinals = BTreeMap::<InstructionPath, usize>::new();
    for event in events {
        if !target_program(&event.expected_program_id) || event.pool.is_empty() || event.semantic_kind.is_empty() {
            bail!("SEMANTIC_EVENT_INPUT_UNSUPPORTED")
        }
        if active_programs.get(event.log_index).and_then(|program| program.as_deref()) != Some(event.expected_program_id.as_str()) {
            bail!("EVENT_LOG_PROGRAM_MISMATCH")
        }
        let matches = invocations.iter().filter(|invocation| {
            invocation.program_id == event.expected_program_id
                && invocation.accounts.contains(&event.pool)
                && starts_with_any(&invocation.data, &event.accepted_instruction_discriminators)
                && !used_paths.contains(&invocation.path)
        }).collect::<Vec<_>>();
        if matches.len() != 1 { bail!("ORDER_AMBIGUOUS_EVENT_TO_INVOCATION_MAPPING") }
        let invocation = matches[0];
        used_paths.insert(invocation.path.clone());
        let semantic_ordinal = *ordinals.entry(invocation.path.clone()).or_default();
        *ordinals.get_mut(&invocation.path).expect("entry exists") += 1;
        emitted.push(DecodedSemanticEvent {
            signature: input.signature.clone(),
            canonical_order: CanonicalOrderTuple { slot: input.slot, finalized_transaction_index: transaction_index, instruction_path: invocation.path.clone(), semantic_ordinal },
            semantic_kind: event.semantic_kind.clone(), pool: event.pool.clone(),
        });
    }
    emitted.sort_by(|left, right| left.canonical_order.cmp(&right.canonical_order));
    Ok(emitted)
}

#[derive(Clone, Debug)]
pub struct ScientificAdmissionProof {
    pub captured_transaction_interval_complete: bool,
    /// This is intentionally a separate proof. Transaction capture coverage
    /// does not prove that all candidate pools/states are known.
    pub historical_candidate_universe_complete: bool,
}

pub fn release_scientific_state(decoded: &[DecodedSemanticEvent], proof: &ScientificAdmissionProof) -> Result<()> {
    if decoded.is_empty() { bail!("NO_SEMANTIC_EVENTS") }
    if !proof.captured_transaction_interval_complete { bail!("CAPTURED_TRANSACTION_INTERVAL_INCOMPLETE") }
    if !proof.historical_candidate_universe_complete { bail!("CANDIDATE_UNIVERSE_INCOMPLETE") }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROUTER: &str = "router";
    const POOL_A: &str = "pool-a";
    const POOL_B: &str = "pool-b";
    const BUY: &[u8] = b"buy-disc";
    const SELL: &[u8] = b"sell-disc";

    fn event(program: &str, pool: &str, log_index: usize, discriminator: &[u8]) -> SemanticEventInput {
        SemanticEventInput { expected_program_id: program.into(), pool: pool.into(), log_index, accepted_instruction_discriminators: vec![discriminator.to_vec()], semantic_kind: "SWAP".into() }
    }
    fn instruction(program_index: usize, accounts: &[usize], data: &[u8]) -> CompiledInstructionInput {
        CompiledInstructionInput { program_id_index: program_index, account_indexes: accounts.to_vec(), data: data.to_vec() }
    }
    fn input(outer: Vec<CompiledInstructionInput>, inner: Vec<InnerInstructionGroupInput>, logs: Vec<&str>) -> FullTransactionInput {
        FullTransactionInput {
            slot: 9, finalized_transaction_index: Some(4), signature: "sig".into(), succeeded: true,
            static_account_keys: vec![PUMP_PROGRAM.into(), PUMPSWAP_PROGRAM.into(), ROUTER.into(), POOL_A.into(), POOL_B.into()],
            loaded_writable_account_keys: vec![], loaded_readonly_account_keys: vec![], outer_instructions: outer, inner_instruction_groups: inner,
            log_messages: logs.into_iter().map(str::to_owned).collect(),
        }
    }
    fn direct_logs(program: &str) -> Vec<&'static str> { vec![Box::leak(format!("Program {program} invoke [1]").into_boxed_str()), "Program data: ignored", Box::leak(format!("Program {program} success").into_boxed_str())] }

    #[test]
    fn direct_pump_and_pumpswap_preserve_outer_path() {
        let pump = input(vec![instruction(0, &[3], BUY)], vec![], direct_logs(PUMP_PROGRAM));
        let decoded = decode_instruction_paths(&pump, &[event(PUMP_PROGRAM, POOL_A, 1, BUY)]).unwrap();
        assert_eq!(decoded[0].canonical_order.instruction_path.render(), "outer:00000");
        let swap = input(vec![instruction(1, &[3], SELL)], vec![], direct_logs(PUMPSWAP_PROGRAM));
        assert_eq!(decode_instruction_paths(&swap, &[event(PUMPSWAP_PROGRAM, POOL_A, 1, SELL)]).unwrap()[0].canonical_order.instruction_path.outer_index, 0);
    }

    #[test]
    fn router_cpi_and_loaded_addresses_resolve_real_inner_path() {
        let mut transaction = input(vec![instruction(2, &[], b"route")], vec![InnerInstructionGroupInput { parent_outer_index: 0, instructions: vec![InnerInstructionInput { program_id_index: 1, account_indexes: [6].to_vec(), data: BUY.to_vec(), stack_height: Some(2) }] }], vec!["Program router invoke [1]", "Program pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA invoke [2]", "Program data: ignored", "Program pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA success", "Program router success"]);
        transaction.loaded_writable_account_keys = vec!["loaded-unused".into()];
        transaction.loaded_readonly_account_keys = vec![POOL_A.into()];
        let decoded = decode_instruction_paths(&transaction, &[event(PUMPSWAP_PROGRAM, POOL_A, 2, BUY)]).unwrap();
        assert_eq!(decoded[0].canonical_order.instruction_path.render(), "outer:00000/inner:00000/stack:002");
    }

    #[test]
    fn multiple_swaps_and_multi_mint_route_have_independent_paths() {
        let transaction = input(vec![instruction(2, &[], b"route")], vec![InnerInstructionGroupInput { parent_outer_index: 0, instructions: vec![
            InnerInstructionInput { program_id_index: 1, account_indexes: vec![3], data: BUY.to_vec(), stack_height: Some(2) },
            InnerInstructionInput { program_id_index: 1, account_indexes: vec![4], data: SELL.to_vec(), stack_height: Some(2) },
        ] }], vec!["Program router invoke [1]", "Program pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA invoke [2]", "Program data: ignored", "Program pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA success", "Program pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA invoke [2]", "Program data: ignored", "Program pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA success", "Program router success"]);
        let decoded = decode_instruction_paths(&transaction, &[event(PUMPSWAP_PROGRAM, POOL_A, 2, BUY), event(PUMPSWAP_PROGRAM, POOL_B, 5, SELL)]).unwrap();
        assert_eq!(decoded.len(), 2);
        assert!(decoded[0].canonical_order < decoded[1].canonical_order);
    }

    #[test]
    fn failed_ambiguous_logs_and_malformed_metadata_fail_closed() {
        let mut failed = input(vec![instruction(1, &[3], BUY)], vec![], direct_logs(PUMPSWAP_PROGRAM)); failed.succeeded = false;
        assert!(decode_instruction_paths(&failed, &[event(PUMPSWAP_PROGRAM, POOL_A, 1, BUY)]).unwrap_err().to_string().contains("FAILED"));
        let ambiguous = input(vec![instruction(1, &[3], BUY), instruction(1, &[3], BUY)], vec![], vec!["Program pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA invoke [1]", "Program data: ignored", "Program pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA success", "Program pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA invoke [1]", "Program pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA success"]);
        assert!(decode_instruction_paths(&ambiguous, &[event(PUMPSWAP_PROGRAM, POOL_A, 1, BUY)]).unwrap_err().to_string().contains("ORDER_AMBIGUOUS"));
        let malformed = input(vec![instruction(2, &[], b"route")], vec![InnerInstructionGroupInput { parent_outer_index: 1, instructions: vec![] }], vec!["Program router invoke [1]", "Program router success"]);
        assert!(decode_instruction_paths(&malformed, &[]).is_err());
    }

    #[test]
    fn incomplete_proofs_never_release_scientific_state() {
        let transaction = input(vec![instruction(1, &[3], BUY)], vec![], direct_logs(PUMPSWAP_PROGRAM));
        let decoded = decode_instruction_paths(&transaction, &[event(PUMPSWAP_PROGRAM, POOL_A, 1, BUY)]).unwrap();
        assert!(release_scientific_state(&decoded, &ScientificAdmissionProof { captured_transaction_interval_complete: false, historical_candidate_universe_complete: true }).unwrap_err().to_string().contains("CAPTURED"));
        assert!(release_scientific_state(&decoded, &ScientificAdmissionProof { captured_transaction_interval_complete: true, historical_candidate_universe_complete: false }).unwrap_err().to_string().contains("UNIVERSE"));
        release_scientific_state(&decoded, &ScientificAdmissionProof { captured_transaction_interval_complete: true, historical_candidate_universe_complete: true }).unwrap();
    }
}
