# Milestone 1 Closure Report

## Criteria

| Criterion | Status | Evidence |
|---|---|---|
| Valid Rust/Python parity fixture for migration | BLOCKED | Python reference has passing fixtures; Rust has not yet imported a genuine finalized migration fixture. |
| Actual producer writes creation/path receipts | VERIFIED in source/build | `creation_receipts` executes in the canary reconciliation branch; not deployed/live-exercised. |
| Cohort ledger independently validates receipts | BLOCKED | Current forward ledger still accepts caller-supplied continuity/coverage fields and is not a receipt consumer. |
| Contiguous finalized interval complete or limitation stated | BLOCKED WITH PRECISE LIMIT | Filtered stream + per-observed-slot RPC does not enumerate all slots. A `getBlocks(start,end)` finalized inventory plus `getBlock` per produced slot is required. With a 16-call cap, at most roughly 15 produced slots can be verified after inventory; 30 minutes is thousands of slots and is infeasible under the current cap. |
| Production build / integrated adversarial tests | PARTIAL | Release SHA `38f05b3846c4e205bf2d2a1556a50517383bec90226683413019cd152c6194f9`; creation unit tests 3/3; canary integration tests compile. Full producer fixture matrix remains missing. |
| Diagnostic canary has exact limits | NOT READY | Proposed: 15 produced slots maximum, 16 RPC calls maximum (one inventory + at most 15 `getBlock`), 10 min, 64 MiB receipts. Must first add receipt-consuming cohort ledger and slot inventory. |

## Correctness change

An observed `CreatePoolEvent` without an instruction, and an instruction
without an event, now fail with `CREATE_POOL_EVENT_CARDINALITY`.  Neither
condition can silently return an empty creation set.

## Operational readiness

`edgesniffer-teacher-impulse-mc5000-dust.service` is a separate
`/root/bot/FinalBot-cya` service, root-owned, `Restart=always` every two
seconds.  It currently exits `203/EXEC`; it had 199,817 restarts at read-only
inspection.  It consumes little CPU per failed invocation but creates a noisy
and unnecessary shared-host loop.  Safest isolation is an owner-approved
`systemctl disable --now` followed by a drop-in/review, not a Canonical change.
No action was taken.

## Stop point

Milestone 1 cannot be declared complete.  The next exact work is limited to:
1. port/reference a genuine migration fixture into Rust and add the requested
   adversarial fixture matrix;
2. replace caller flags in `forward_new_pool_cohort_v1` with receipt-chain
   validation; and
3. implement finalized `getBlocks` range inventory before any diagnostic.
