# Canonical State V1 - Pump/PumpSwap Instruction-Path Decoder V1

**Scope:** offline implementation, source-only test integration, and review of
commit `02632448ea83601d20d2afd4784e1d35d1a42b71`. No live collector, V8
binary/service, RPC request, cohort, holdout, scientific admission, execution
path, or historical evidence was changed.

## Requirement checklist

| Requirement | Status | Evidence |
|---|---|---|
| Independently review filtered identity manifest | VERIFIED, defect fixed | `IntervalCompleteness` previously accepted an arbitrary supplied receipt list without proving coverage through its declared end slot. It now requires contiguous `RECONCILED_BLOCK` or authoritative `FINALIZED_SKIPPED` coverage and rejects filtered identities in skipped slots. |
| Restart durability / hash chain | VERIFIED offline | Reopen scans every JCS self-hashed record, checks both sequences, source/epoch, duplicate identity, tail, and checkpoint. A valid stale checkpoint after a synced append is recovered from the verified journal tail; a corrupt or non-prefix checkpoint rejects. |
| Bounded memory / storage / fork behavior | VERIFIED offline | Writer has an explicit identity cap; reconciler caps per-slot identities; registry caps pending slots and rejects conflicting finalized block identities. The interval proof defect above is covered by regressions. |
| Completeness claims / fail closed | VERIFIED offline | Captured-transaction interval requires contiguous slot coverage, exact finalized reconciliation, one epoch, and no gap/restart. It does not assert candidate-universe completeness. |
| Static and loaded account keys | VERIFIED | Decoder resolves `static + loaded writable + loaded readonly`; invalid indexes reject. |
| Outer and ordered inner positions | VERIFIED | Preserves outer index, inner ordinal, and required stack height in `InstructionPath`. |
| Pump/PumpSwap and router CPI association | VERIFIED offline | Requires one matching invocation using program, resolved account set including canonical Pool, and frozen discriminator; router CPI is represented as an inner path. |
| Multiple invocations / multi-mint routes | VERIFIED synthetic | Different Pool/discriminator combinations get independent ordered paths. A non-unique mapping rejects. |
| Canonical ordering tuple | VERIFIED | Every result carries `slot, finalized_transaction_index, instruction_path, semantic_ordinal`. Yellowstone arrival is absent from this tuple. |
| Reject unsupported/ambiguous mappings | VERIFIED | Failed transactions, absent finalized transaction index, malformed parents/stack heights, invalid account index, bad log stack, wrong active log program, missing discriminators, and multi-match bindings all return errors. |
| Decoder test matrix | VERIFIED synthetic | Direct Pump, direct PumpSwap, router CPI, multiple swaps, multi-mint route, loaded address, failed transaction, ambiguous logs, malformed metadata, and proof-gated state release pass in the harness. |
| Retained real fixture use | BLOCKED WITH EVIDENCE | The retained source-tree fixture inventory contains `rust-exact-ledger/tests/fixtures/2cq-known-buy.protobuf`; no retained full Pump/PumpSwap message-plus-meta fixture was found in the bounded source-tree check. Synthetic fixtures are explicitly not represented as live/provider proof. |
| No release on incomplete/ambiguous evidence | VERIFIED | `release_scientific_state` separately requires complete captured-transaction interval and complete historical candidate-universe proof. The test proves either missing proof blocks release. |
| Distinguish transaction capture from historical universe | VERIFIED | These are separate booleans/proofs. Passing finalized transaction reconciliation cannot satisfy `historical_candidate_universe_complete`. |
| Existing compatible Cargo toolchain | VERIFIED | VPS has existing Cargo/Rust 1.99 under `/root/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/`; tests used explicit `RUSTC` and Cargo paths only. System `/usr/bin/cargo` and default Rust remained unchanged. |
| No live deployment or paid RPC | VERIFIED | No service restart/deployment or RPC request was made. |

## Implementation

Added `pump_instruction_path_decoder_v1.rs`. It deliberately accepts an
already decoded semantic event only with explicit program, Pool, event-log
provenance, and frozen instruction discriminator evidence. It builds an
account-key table from the full transaction message and execution metadata,
enumerates direct and CPI invocations, and maps each event only when exactly
one invocation matches.

Log nesting is checked only as corroborating execution metadata. A log line
does not provide a transaction or instruction order. Any mismatch or
ambiguity is `ORDER_AMBIGUOUS_EVENT_TO_INVOCATION_MAPPING` and blocks output.

## Test results

- Sanitized isolated harness: **24 passed, 0 failed**.
- Existing VPS source package, using only its already installed Rust 1.99:
  **40 lib tests passed, 0 failed**.
- The prior toolchain blocker was resolved by setting `RUSTC` to the already
  installed Rust 1.99 binary when invoking the already installed Cargo 1.99.
  No install, upgrade, or system-default change occurred.

The source-only VPS files tested above have SHA-256 values:

- `filtered_identity_manifest_v1.rs`:
  `6d6f31763f5046c7f9b7931e1adb9f5b592a53c971f161ef3f3e553f59cd5804`
- `pump_instruction_path_decoder_v1.rs`:
  `748320a60df057b25ecacb8c49ff8e849238a104abdf60abf9a93ef12e212033`

## Readiness verdict

**OFFLINE DECODER PRIMITIVE: READY FOR OFFLINE ADAPTER INTEGRATION.**

**LIVE CANARY AUTHORIZED: NO.** Required gates remain: actual Yellowstone
protobuf adapter, retained full Pump/PumpSwap fixture parity, bounded live
identity-manifest/reconciliation adapter, and complete candidate-universe
proof. The decoder does not weaken any of those gates.
