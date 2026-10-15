# AOM-V0 Correctness Review and Real-Data Replay Gate

## Scope

Focused corrective review of AOM-V0 base commit
`fbf676124031d446e469af8853e459c785cc3413`. This work is offline code,
tests, and read-only VPS inspection only. It did not read frozen holdout
outcomes, call paid RPC, start a canary, modify V8, restart a service, or
modify historical evidence.

## Numbered completion checklist

| # | Requirement | Status | Evidence |
|---:|---|---|---|
| 1 | Verify immutable partition coverage/order/state/universe proofs | VERIFIED | AOM now requires a hash-bound `AOM_V0_IMMUTABLE_PROOF_BUNDLE_V1`; boolean claims and nonempty strings alone fail. |
| 2 | Correct cash ledger state machine | VERIFIED | Explicit `ENTRY_NO_FILL`, `ENTRY_FAILED`, `OPEN_POSITION`, and `CLOSED_FILLED` states; failed costs require evidence. |
| 3 | Correct F13 lifetime semantics | VERIFIED | `ever_observed_buy` is lifetime pool state, separate from 5/20/60-second deques; unknown lifetime history is null/masked. |
| 4 | Review causality, pool state, boundary binding, fees | VERIFIED | Single source/epoch feature input, causal ordering, state-proof checkpoint equality, and input-token fee denomination checks. |
| 5 | Reconcile test count | VERIFIED | Earlier report corrected: current AOM suite is 13 passing tests. |
| 6 | Explain `exactReplayComplete=false` | VERIFIED | Read-only manifest/coverage inventory below. |
| 7 | Find an existing complete public interval | BLOCKED | No retained partition meets the independent coverage/universe gate. |
| 8 | Specify smallest future capture | VERIFIED AS CONDITIONAL PLAN | Defined below; cannot start until provider/bootstrap prerequisites exist. |
| 9 | Canonical State / V6-V8 compatibility | VERIFIED | Plan extends Canonical State v1 only; V6/V7/V8 are neither reused nor replaced during this work. |
| 10 | Clear evidence-based decision | VERIFIED | `BLOCKED BY PROVIDER/CAPABILITY` for a scientifically admissible capture today. |

## Correctness defects fixed

1. **Admission accepted assertions instead of evidence.** The original
   partition validator trusted three booleans plus reference strings. It now
   resolves a hash-chain-protected bundle and verifies exact partition and
   manifest binding for finalized coverage, causal order, complete universe,
   and pre-decision state proofs. Tampering or unresolved references blocks.
2. **F13 was incorrectly a 60-second statistic.** The rolling buy deque was
   used for both flow fields and `no_prior_observed_buy`. F13 now uses a
   dedicated lifetime set. If complete lifetime history is not proven, its
   value is `null` and its validity is false; it is never silently `true`.
3. **Feature state could mix epochs.** Public feature reconstruction now
   rejects source-instance/continuity-epoch changes.
4. **Boundary state was weakly bound.** The state proof now has to bind the
   exact decision timestamp and causal checkpoint, while coverage/universe
   references must match the admitted partition.
5. **Fee meaning was implicit.** Public and simulation inputs now require
   `INPUT_AMOUNT_BPS` / `INPUT_TOKEN`; unsupported denominations block.
6. **Execution accounting was unsafe.** No-fill costs are now zero unless a
   documented failed attempt exists. Missing exit evidence produces an open
   position with unclosed inventory, not a synthetic realized loss or a
   discarded position. AMM fee remains embedded once in exact swap output.

## Test results

`python -m pytest sanitized-code/tests/test_aom_v0_public_universe_replay.py -q`

```text
13 passed
```

The suite includes immutable-proof tampering, missing/unresolved proof bundle,
capture versus universe separation, boundary checkpoint mismatch, leakage,
cross-epoch input, F13 after rolling expiry, fee denomination, failed-attempt
evidence, no-fill, open-position, causal ordering, reproducibility, quota,
and RPC-budget regressions.

The broader sanitized suite remains **71 passed, 1 failed** at a protected,
unrelated pre-H3 frozen-scorer hash assertion. It is not modified or bypassed
by this AOM work.

## Read-only historical admission inventory

At inspection time, the shared V8-window root contained **1,601** sealed
`PUMP_EXACT_EVIDENCE_LEDGER_V1` partition manifests. All were temporally
closed, but all had:

- `exactReplayComplete=false`;
- `completeUtcHour=false`;
- positive `provisionalTransactions` and `provisionalUnresolved` counters;
- zero `rpcFinalizedBlocks`, `rpcFinalizedTransactions`, and `rpcSnapshots`;
- zero retained account/block-meta/slot update coverage counters; and
- a `reconciliationManifest` filename without the referenced file present
  beside the partition manifest.

Across the inventory, 208,061,182 observed/relevant transaction records were
still provisional/unresolved. These facts explain the false flag: the legacy
partitions were capture products, not finalized block-reconciled evidence.
They cannot supply canonical transaction coverage, exact causal order, or a
complete eligible-pool universe. No manifest was opened, edited, repaired, or
promoted.

## Existing-data decision

**EXISTING DATA SUFFICIENT: NO.** A transaction interval and an eligible-pool
universe are separate scientific claims. The retained V8 manifests establish
neither one at the required standard, and wallet-selected records cannot fill
the missing denominator.

## Smallest scientifically admissible future capture, conditional on provider proof

The next capture must be a bounded **Canonical State v1 producer extension**,
not another V6/V7/V8-style collector.

1. **Provider conformance prerequisite.** Obtain a small, non-scientific
   Shyft conformance fixture or written contract proving that the selected
   Yellowstone complete-block subscription exposes a full per-slot inventory,
   canonical transaction indexes, and the required filtered Pump/PumpSwap
   membership. The existing `SLOT_COMPLETED` signal is specifically invalid;
   RabbitStream, ShredStream, and WebSockets cannot supply execution metadata
   and final ordering.
2. **Stage 1, Canonical State v1 only.** Capture compact block identity/order
   proofs, derive the complete relevant set from program/account membership,
   and retain a quota-capped filtered-identity manifest. No pool RPC or full
   semantic decode occurs in the block receive path. Oversized/incomplete
   blocks become a coverage blackout.
3. **Stage 2.** Reconcile each filtered full-meta transaction against that
   proof, decode outer/inner instruction paths from message plus meta, and
   resolve Pool identities with bounded workers. Exact set equality and
   resolved order are required before state release.
4. **Bootstrap is not time-based.** Current candidate rules include lifetime
   `F13` and pool-history state. Therefore a finite post-start warm-up cannot
   prove the existing eligible-pool universe. Before the first decision, the
   producer needs either exact finalized replay across the boundary or an
   authoritative retained inventory that classifies *every* potentially
   eligible pre-boundary pool as restored-complete or provably-ineligible.
   Until then record `UNIVERSE_INCOMPLETE`; do not admit decisions.
5. **Bounds.** Retain compact semantic/block proofs plus a short-TTL raw
   validation fixture only. Keep raw data capped, use the Canonical State
   checkpoint/coverage ledger, prohibit general raw spooling, zero live RPC
   in AOM-V0, and reserve the configured protected free-space floor before
   start. Current free space was about 76 GiB, below the prior canary's 240
   GiB reserve; capacity/mount policy must be satisfied before any canary is
   even proposed.
6. **Acceptance.** One interval is admissible only when every slot has a
   hash-bound finalized block proof and exact relevant-identity reconciliation;
   every relevant semantic event has a resolved instruction path; one source
   epoch is continuous; state and all feature histories are valid; a complete
   universe certificate exists; and the interval passes old/new deterministic
   replay twice plus restart recovery. Only then may AOM-V0 inspect it offline.

## Decision and single next operational milestone

**Decision: BLOCKED BY PROVIDER/CAPABILITY.** The current provider path has
not demonstrated bounded complete-block inventory delivery, and existing
retained evidence has no complete-universe proof. Starting another capture
without closing that would repeat the same invalid evidence pattern.

**Single recommended next milestone:** obtain and freeze a bounded Shyft
complete-block inventory conformance result for the exact Canonical State v1
subscription shape. It must answer whether each emitted block contains the
whole slot inventory with indexes under the configured message limit. A `YES`
unblocks an isolated Stage-1 producer canary; a `NO` or missing guarantee
keeps the system fail-closed and requires an authoritative compact inventory
source before any autonomous replay work.

## Readiness

- **Offline engineering readiness:** READY; corrected framework and tests are
  available.
- **Real-data replay readiness:** BLOCKED by missing finalized coverage/order/
  universe proof.
- **Autonomous scientific validation readiness:** BLOCKED.
- **Paper-trading readiness:** BLOCKED.
