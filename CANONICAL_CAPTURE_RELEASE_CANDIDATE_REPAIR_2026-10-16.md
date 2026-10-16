# Canonical Capture Release-Candidate Repair

**Base audit commit:** `4040a5b76af93bc9aa8fd7cc723dc68ee1669eaf`  
**Scope:** offline-only. No install, live RPC, R4, service action, V8 action,
dust-service action, submission, trade, or R1/R2/R3 evidence change occurred.

## Result

All four requested findings are resolved in the offline release candidate.
The repaired producer handles mixed finalized blocks using the same versioned
relevance policy as reconciliation, emits no creation quarantine for ordinary
or unrelated activity, and makes receipt-chain admission contingent on a
durably committed `VERIFIED_DIAGNOSTIC` capture checkpoint.

Release candidate, **not installed**:

```text
54d897f54fb4cb8dc2392da16227cfdd3da2cd5e8e9b051d385b9a20eada36cc
```

Build artifact:

```text
/mnt/edgesniffer-canonical-hot/canonical-remediation-target/release/kefi_finalized_rpc_reconciliation_canary_v1
```

## Corrections

| Finding | Repair | Offline proof |
| --- | --- | --- |
| P0 mixed-block receipts | `CanonicalRelevancePolicy` now determines final receipt inclusion before a source frame or semantic receipt is required. The original finalized block position is retained. Unsupported **relevant** transactions fail closed; unrelated unsupported transactions are excluded without poisoning a filtered proof. | `mixed_finalized_block_emits_only_relevant_receipts_and_precise_creation_dispositions` |
| P0 creation quarantine | `FinalizedCreationCandidate` separates `NotCandidate`, `StandalonePumpSwapExcluded`, and `PumpMigrationCandidate`. Only a migration candidate with failed witness validation gets `CREATION_QUARANTINED`; it must reference a matched identity and cannot contain pool/mint/event/path fields. | Creation classifier unit test, mixed block test, and forward-cohort quarantine rejection test |
| P1 terminal checkpoint | `commit_receipts` writes the terminal reason before atomically entering `VERIFIED_DIAGNOSTIC`. Fresh reload validates each terminal state. | `every_terminal_checkpoint_reloads_after_atomic_transition` |
| P1 crash consistency | The consumer now requires a matching committed `VERIFIED_DIAGNOSTIC` checkpoint, inventory hash, source instance, and continuity epoch. Atomic bundle/inventory artifacts or canonical journal output alone are not admissible. | `crash_points_cannot_turn_staged_receipts_or_journal_output_into_verified_admission` |

### Receipt semantics

1. Unrelated successful transaction: no source frame, semantic path, identity, or creation receipt.
2. Failed relevant transaction: excluded by the frozen failed-transaction policy, with no receipt.
3. Relevant non-migration: matched identity plus explicit semantic quarantine when source semantic evidence is absent; no creation receipt.
4. Invalid Pump migration candidate: matched identity, semantic quarantine if applicable, and one bound `CREATION_QUARANTINED` receipt.
5. Verified Pump migration: matched identity, authoritative path receipt, and one restricted verified-creation receipt. It is still not scientific admission.

## Test Results

Executed on the VPS using Rust 1.99 and only the dedicated canonical build
target. The system compiler remained unchanged.

```text
rust-exact-ledger: cargo test --lib --quiet
54 passed; 0 failed

rust-sidecar: cargo test --lib --quiet
108 passed; 0 failed

rust-sidecar: cargo test --bin kefi_finalized_rpc_reconciliation_canary_v1 --quiet
23 passed; 0 failed

rust-sidecar: cargo build --release --bin kefi_finalized_rpc_reconciliation_canary_v1 --quiet
success; SHA-256 above
```

The only warning is an existing unused `first_seen_ms` dispatcher field; it is
not a correctness failure and does not change runtime behavior.

## Acceptance Matrix

| Evidence class | Status | Meaning |
| --- | --- | --- |
| Unit tests | PASS | Policy, creation classification, terminal state, receipt validation, and cohort rejection use deterministic inputs. |
| Scripted producer integration | PASS | Real producer functions create atomic bundles/inventory, canonical bridge artifacts, then the real receipt-chain validator reloads them. |
| Crash-injection tests | PASS | Crash after slot bundle, inventory/journal before terminal commit, and terminal checkpoint reload fail closed or validate as terminal. |
| Existing R1/R2/R3 regressions | PASS | Delayed identity, relevance/version, range-first, skipped/zero slots, index mismatch, source gaps, and resource-budget scenarios remain covered. |
| Actual live evidence for this candidate | NOT RUN | R1/R2/R3 remain immutable historical evidence. No R4 was run. |

## Remaining Limits

- This candidate has no new live-provider proof. R4 remains authorization-gated.
- A matched bounded capture would still prove only its restricted receipts and
  inventory, not complete historical eligible-pool universe coverage.
- The fixed operational limits remain: 16 total JSON-RPC attempts, 3–5 slots,
  45 minutes, 181 GiB startup free-space floor, 160 GiB runtime reserve, 6
  GiB evidence cap, and 8 GiB RSS ceiling.

## Future R4 Installation / Rollback Plan (Not Executed)

1. Obtain explicit R4 authorization and create a fresh, empty run root below
   `/mnt/edgesniffer-canonical-hot/`.
2. Verify the candidate with `sha256sum`; it must equal the SHA above.
3. Copy the currently installed diagnostic executable to a timestamped
   rollback path on the canonical volume, hash it, then atomically install the
   verified candidate using a temporary sibling file and rename.
4. Invoke only the bounded capture-only command with the existing environment;
   do not change V8, systemd units, or the dust service.
5. On any preflight, integrity, resource, source-gap, or continuity failure,
   stop once and preserve the new evidence root. Do not retry automatically.
6. Roll back by atomically restoring the recorded previous binary and checking
   its recorded SHA. No evidence is removed during rollback.

This is a proposed procedure only. It was not installed or executed.
