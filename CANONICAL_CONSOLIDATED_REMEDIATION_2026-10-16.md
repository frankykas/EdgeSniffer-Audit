# Canonical Consolidated Remediation

**Base audit:** `d9e1714d07e3bc3a9e4b26198acbc1fde004a1b2`  
**Scope:** offline-only remediation. No R4, live RPC, deployment, service action, V8 action, dust-service action, trading, or historical-evidence mutation occurred.

## Result

The production canary executable now follows a persisted range-first control
plane. It does not spend dispatcher RPC budget before selecting a source-finalized
range and reserving the range inventory. Finalized RPC establishes expected
relevant identities; Yellowstone only supplies the independently cross-checked
source identity set. A late source identity waits without another finalized RPC
attempt. A source miss reaches an explicit inconclusive disposition instead of a
false sealed inventory.

Release candidate:

```text
cde3693cc6ecd3bd290bc721e3f1ac4da1e7bc3e97a04ce6c4e46aa6a88ccf04
```

Artifact path, not installed:

```text
/mnt/edgesniffer-canonical-hot/canonical-remediation-target/release/kefi_finalized_rpc_reconciliation_canary_v1
```

## Architecture Changes

| Area | Implemented behavior |
| --- | --- |
| Control plane | Durable `CaptureRunJournal` lifecycle: `PRECHECK -> CAPTURING_SOURCE -> RANGE_SELECTED -> FINALIZED_INVENTORY_FETCHED -> AWAITING_SOURCE_MATCH -> RECEIPTS_STAGED -> RECEIPTS_COMMITTED -> VERIFIED_DIAGNOSTIC`; `INCONCLUSIVE`, `QUARANTINED`, and `VOIDED` are terminal. |
| Crash policy | A prior nonterminal root is atomically marked `VOIDED`; it cannot bridge into a later run. Canonical journal reopen validates its complete existing chain before append. |
| Raw RPC | Exact `getBlocks` / `getBlock` HTTP response bytes are quota-reserved before write, atomically persisted, SHA-256 hashed, parsed only after persistence, and bound through the run checkpoint. Live range reconciliation receipts bind their raw `getBlock` SHA-256. |
| Relevance | `CanonicalRelevancePolicy` is versioned, hashed, bound in the run, drives the Yellowstone subscription, finalized derivation, and identity receipt emission. The reconciler rejects a receipt whose policy hash differs. |
| Bundles | Each slot publishes a staged then renamed receipt bundle. Its manifest binds child hashes, block identity, policy hash, expected identity-set digest, and emitted identity-set digest; validation requires equality. |
| Semantic evidence | Every relevant finalized transaction receives either authoritative path evidence or explicit quarantine (`SOURCE_SEMANTIC_ABSENT` / decoder reason). Path receipts bind source-frame hashes and full causal tuple fields. |
| Canonical bridge | Committed receipt bundles are fed through `CausalOrderResolver`, `JournalWriter`, and `CanonicalState` in the real canary path. The bridge always records `UNIVERSE_INCOMPLETE` and prohibits scientific admission; restricted forward coverage remains distinct from full historical universe coverage. |
| Resources | The capture-run checkpoint owns RPC count, raw/receipt evidence bytes, observed RSS, and queue maximum. The existing 181 GiB startup floor, 160 GiB runtime reserve, 6 GiB evidence cap, 8 GiB RSS cap, and five-slot/16-request ceiling remain enforced. |

## Source Inventory

Sanitized implementation is published under
`sanitized-code/canonical-state-v1/consolidated-remediation-2026-10-16/`.

- `canonical_capture_run_v1.rs`: persisted lifecycle, policy, raw evidence,
  resource accounting, source finality and explicit voiding.
- `filtered_identity_manifest_v1.rs`: durable filtered identity chain,
  policy-bound finalized reconciliation, source-index equality gate, raw-block
  receipt binding.
- `forward_cohort_receipts_v1.rs`: atomic receipt-bundle validation and
  quarantined-creation rejection.
- `finalized_slot_inventory_v1.rs`: finalized produced/skipped-slot inventory.
- `canonical_state_v1.rs`: causal ordering, canonical journal, coverage model
  and deterministic serialization.
- `kefi_finalized_rpc_reconciliation_canary_v1.rs`: actual producer entry
  point, source observation transition, range inventory, bundle commitment and
  canonical bridge.

## Acceptance Ledger

| Audit criterion | Status | Evidence |
| --- | --- | --- |
| Range-first persisted lifecycle | PASS | `CaptureRunJournal`; canary range mode does not enter dispatcher loop. |
| R1 delayed fifth identity | PASS | `fifth_finalized_before_late_source_identities_waits_then_becomes_sealable`. |
| Permanent source identity miss | PASS | `missing_fifth_source_evidence_cannot_seal_complete_inventory...`. |
| R2 relevance/receipt parity | PASS | `identity_receipts_share_exact_reconciliation_relevance_and_version_rules`. |
| R3 pre-range dispatcher starvation | PASS | `range_first_production_helpers_reserve_inventory_before_any_dispatcher_work`. |
| Yellowstone/finalized index mismatch | PASS | ledger reconciliation source-index mismatch regressions. |
| Raw provider bytes before parsing | PASS | `rpc_raw` reserves/writes/hashes first; range raw files are hash-bound. |
| Atomic expected/emitted receipt set | PASS | `atomic_bundle_manifest_rejects_expected_emitted_set_digest_tampering`. |
| Quarantined creation cannot admit | PASS | receipt-chain consumer regression suite. |
| Canonical production code path exercised | PASS | `scripted_yellowstone_to_producer_to_persisted_cohort_validator_round_trip`; same source-observation transition is called by live protobuf loop. |
| Canonical restart/idempotency | PASS | `canonical_bridge_recovers_existing_journal_without_duplicate_semantic_events`. |
| Resource limits before breach | PASS | run resource unit tests plus canary storage/reservation tests. |
| Five-slot RPC cap, including retries | PASS | shared `RpcBudget` / `ResourceAccounting`; five-slot inventory reserves six requests, leaving at most ten dispatcher requests. |
| V8 / dust / submission path isolation | PASS | no imports, calls, installation, unit changes, or submission code in this binary path. |
| Full unrelated-workspace compile | BLOCKED, non-scope | `cargo test --all-targets` exposes pre-existing unrelated `yellowstone_v3_*` compile defects. The only source diff in ledger `lib.rs` is module export `canonical_capture_run_v1`; the defects are not in this remediation path. Focused production target and ledger suites pass. |

## Offline Verification

Executed on the VPS with Rust `1.99`, using only the dedicated canonical build
target on `/mnt/edgesniffer-canonical-hot`:

```text
rust-exact-ledger: cargo test --lib --quiet
53 passed; 0 failed

rust-sidecar: cargo test --bin kefi_finalized_rpc_reconciliation_canary_v1 --quiet
21 passed; 0 failed

rust-sidecar: cargo build --release --bin kefi_finalized_rpc_reconciliation_canary_v1 --quiet
success; SHA-256 above
```

The canary harness covers the twenty requested classes across its production
state-machine tests and ledger adversarial tests: nominal range, R1 delay/miss,
R2 filtering/version cases, R3 starvation, reverse delivery, index mismatch,
zero-relevant and skipped slots, timeout/error disposition, continuity/epoch
failure, bundle tampering/omission/digest mismatch, quarantined creations,
crash/void behavior, resource pressure, router-CPI/loaded-address/multi-event
decoder behavior, and deterministic replay/recovery.

## Known Limits

This is readiness for a separately authorized, bounded diagnostic only.

- No live run was performed, so provider delivery timing remains a live fact to
  verify under the new lifecycle.
- A reconciled filtered range is not a complete historical eligible-pool
  universe. The canonical bridge therefore writes only quarantined
  `UNIVERSE_INCOMPLETE` state and cannot admit a scientific cohort.
- The retained raw source input remains compact identity/frame provenance, not
  an unlimited Yellowstone firehose. Raw finalized-RPC evidence is retained
  only inside the capped diagnostic root.
- The existing dispatcher module has one harmless dead-code warning
  (`first_seen_ms`); it is not used as a correctness clock.

## Release Decision

**OFFLINE REMEDIATION PASS.** The next action, if separately approved, is one
fresh bounded diagnostic using this exact uninstalled release candidate. It is
not authorization for R4, scientific admission, AOM evaluation, paper trading,
or execution.
