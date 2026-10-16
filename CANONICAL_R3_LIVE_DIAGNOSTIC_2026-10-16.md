# Canonical State V1 R3 Live Diagnostic

Source/audit commit: `a127c1cf7999eba8e0a75ff9fe5fd8235715a64b`.

## Verdict

`FAIL` - the one authorized R3 five-slot capture-only diagnostic stopped
fail-closed before finalized-range inventory. It produced no scientific
interval and no receipt-chain verification was run.

The stop was `DISPATCHER_RPC_RESERVATION_EXHAUSTED`, not a
source-transaction-index cross-check failure. No source/finalized index
mismatch was evaluated live because no dispatcher slot reached exact
source/finalized identity reconciliation.

## Deployment And Safety Record

| Check | Result |
| --- | --- |
| R3 candidate SHA-256 | `96887daf4a76dd7d4cc693076812be930135966c4515d5ef585784ac6500c799` verified before install |
| R2 rollback binary | Preserved at `/usr/local/lib/edgesniffer/edgesniffer-kefi-finalized-rpc-canary-v1.pre-r3-source-index-20261016T173708Z`; SHA-256 `b6a0e501b67b11ebaba00d67b395673ff8f98218efc2ed127cc5f92fc2a27f54` |
| Installed binary | R3 candidate SHA-256 verified after atomic installation |
| Output root | `/mnt/edgesniffer-canonical-hot/canonical-state-v1-live-diagnostic-r3-20261016T173708Z` (new, isolated) |
| Canonical storage | `/dev/vdd1` at `/mnt/edgesniffer-canonical-hot` |
| Startup storage | `1,496,648,097,792` bytes free; 181 GiB floor, 160 GiB reserve, 6 GiB cap passed |
| V8 / dust | V8 inactive throughout; dust untouched and remained in its pre-existing `203/EXEC` auto-restart state |
| Trading/submission | Not invoked |
| Run count | Exactly one R3 transient unit and run |

## Execution Results

| Item | Value |
| --- | --- |
| Unit | `edgesniffer-canonical-live-diagnostic-r3-20261016T173708Z.service` |
| Runtime | 11.332 seconds: `2026-10-16T17:37:08.929273394Z` to `2026-10-16T17:37:20.261012981Z` |
| Exit | `1`: `DISPATCHER_RPC_RESERVATION_EXHAUSTED` |
| Source slots observed | 6 (`457723244` through `457723249`) |
| Exact reconciled slots | 0 |
| Dispatcher requests / retries | 10 / 5 |
| Inventory requests | 0 / 6 reserved (`getBlocks`: 0; `getBlock`: 0) |
| Total JSON-RPC requests | 10 / 16 |
| Pending-slot peak | 4 |
| Source gaps / epoch failures | 0 |
| Decoder transactions / events / rejects | 667 / 308 / 0 |
| Peak RSS / gRPC message | 103,636,992 bytes / 26,418 bytes |
| CPU / reported disk growth | 2.706426s / 471,040 bytes |
| Post-run canonical free space | 1,496,647,618,560 bytes |

The persisted dispatcher has three terminal failed slots (`457723245`,
`457723246`, `457723247`) and three unclosed slots. Its last unresolved
condition is `MISSING_FILTERED_IDENTITY`.

## Cause And Meaning

The five-slot plan reserves six of sixteen requests for a future `getBlocks`
plus five `getBlock` inventory. The remaining ten calls belong to the
dispatcher. R3 spent all ten, including five retries, on early slots whose
compact Yellowstone identity set had not reached exact equality with the
finalized relevant identity set.

No exact reconciliation meant no automatic range start. Therefore the range
inventory was never fetched and all six inventory requests remained reserved.
The next dispatcher attempt correctly failed with
`DISPATCHER_RPC_RESERVATION_EXHAUSTED`; it did not borrow an inventory request,
relax completeness, or start another diagnostic.

This is a scheduler/budget-lifecycle blocker, not proof of a source continuity
break and not a live result for the new source-index check. The narrow next
engineering task is to stop early incomplete dispatcher slots from exhausting
the entire pre-inventory allocation before a viable range can be formed. No
fix or second run was started.

## Evidence Integrity

R1 and R2 are unchanged. R3 is retained at the root above.

| Artifact | SHA-256 |
| --- | --- |
| `run-binding.json` | `c7eac75979c4e0dbf51229681142d9686db00d9822870b503ae0b56838f8b58a` |
| filtered-identity manifest | `dc8cf869d2791deedcdd5076a42732eecad3c944916cc4569f8aab279dd6813e` |
| filtered-identity checkpoint | `2818a897268473b52c05330f5b2e91a38bdf05fa6956058a48d1b17259ecbf9e` |
| dispatcher state | `8de222c8bd0523b22b4b69e2c03bdb513267b6e0a8ebdd40979b1a9ed0efd0c2` |
| final report | `4f3189e74737d252fed4ef07e2ee3731a3a61849da21a4759d701da34898b798` |

No finalized-range inventory, identity/path/creation receipts, or cohort
receipt chain was emitted. The read-only persisted receipt-chain verifier was
therefore not run.

## Checklist

| Requirement | Status | Evidence |
| --- | --- | --- |
| Exactly one capture-only five-slot R3 run | VERIFIED | One transient unit/root; no rerun |
| Candidate hash checked and R2 preserved | VERIFIED | Hashes and backup path above |
| Max 16 RPC, 45m, 8 GiB, 6 GiB growth | VERIFIED | 10 RPC; 11.332s; 103.6 MiB RSS; 471 KiB growth |
| Storage guards and fresh root | VERIFIED | `/dev/vdd1`, floor/reserve/cap passed, new root |
| No V8/dust/trading/unrelated service action | VERIFIED | V8 stayed inactive; dust only read; no submission path invoked |
| Receipt-chain verifier on complete inventory | NOT APPLICABLE | Complete inventory was not produced |
| Scientific market interval | NOT PRODUCED | Zero exact reconciliations |

R3 is complete. No follow-on run, service action, evidence modification, or
new research phase was started.
