# Canonical R1 Capture-Completeness Investigation

Base live-report commit: `92f6a3f7c8fd3845ebc91ade865ad88c43d345a2`.

Result: R1 remains `INCONCLUSIVE`. No live RPC request, deployment, service
change, or second diagnostic was performed for this investigation.

## R1 Evidence Findings

### Confirmed facts

| Evidence | Confirmed finding |
| --- | --- |
| Manifest | 272 filtered identities were durably appended for slots `457679394` through `457679397`; none has slot `457679398`. |
| Manifest timing | First retained identity was written at `2026-10-16T14:58:35.874673872Z`; last was at `2026-10-16T14:58:39.462874653Z`. |
| Dispatcher state | Its last retained checkpoint at `14:58:38.411Z` contains only slot `457679394` terminally matched and slot `457679395` pending. |
| Finalized inventory start | `getBlocks` proof was written at `14:58:41.118Z` and covers produced slots `457679394` through `457679398`; no slot was skipped. |
| Failure | The inventory generated receipts for the first four slots, then stopped at `457679398` with `MISSING_FILTERED_IDENTITY` and `FINALIZED_RANGE_RECONCILIATION_INCOMPLETE`. |
| Meaning of error | The finalized block for `457679398` had a nonempty relevant identity set while the preserved Yellowstone manifest set for that slot was empty. |
| Source health | R1 reported no gRPC error, source gap, continuity-epoch break, decoder reject, storage breach, or budget exhaustion. |

The original receipt artifacts do **not** persist individual finalized-slot
notification timestamps, the expected signature set for the failed fifth
block, or raw `getBlock` response data for that block. Therefore they cannot
determine the expected fifth-slot identity count without a new RPC call, and
cannot distinguish delayed provider delivery from a genuinely omitted filtered
transaction set. The process exited 5.566 seconds after start, so it also could
not observe whether late slot-`457679398` identities would have arrived.

### Root cause

**Confirmed implementation defect:** the old range gate considered the range
ready when only this condition held:

```text
highest source finalized slot >= range end
```

That proves neither that the source sent an individual finalized notification
for every produced slot nor that its filtered transaction identities had all
arrived. R1 fetched and immediately sealed its RPC inventory under that weaker
condition.

**Unresolved provider-level cause:** R1 cannot prove whether slot `457679398`
was late in the Yellowstone stream or never delivered. The R2 change does not
assume either theory; both result in an incomplete interval rather than a seal.

### Retry wording reconciled

The R1 summary's "no retry" means no second live diagnostic was launched.
Within the single process, `dispatcherRetryRpcCalls=1` records one repeated
dispatcher RPC attempt, while `retries=2` records two retry schedules. Those
are bounded in-run reconciliation operations, not a second diagnostic run.

## Smallest Fail-Closed Correction

The canary now has a bounded pending-range state after the fifth inclusive slot
appears finalized. The six reserved inventory requests are used once to fetch
the finalized range, then no further RPC is needed for that range.

A range can seal only when every *produced* slot passes all of these gates:

1. **On-chain finality:** `getBlocks(..., finalized)` includes the slot and
   `getBlock(..., finalized)` supplies its authoritative block.
2. **Individual source observation:** the Yellowstone finalized-slot stream
   contains that exact slot. A higher slot is not accepted as a substitute.
3. **Sufficient source transactions:** `reconcile_finalized_block` finds an
   exact signature/index set match between the compact Yellowstone identity
   manifest and the finalized block-derived relevant set.
4. **Safe seal:** only after every produced slot meets gates 1-3 do receipt
   emission and `FinalizedSlotInventory` publication occur.

If a produced slot has zero relevant transactions, an empty source identity set
matches the empty finalized relevant set and remains valid. If an expected
identity is late, the pending range keeps receiving and rechecking source data.
If it never arrives, the run reaches its normal limit with
`FINALIZED_RANGE_SOURCE_EVIDENCE_INCOMPLETE`; it never publishes an inventory.
Any extra/conflicting/index-mismatched identity remains an immediate
fail-closed error.

The bounded `getBlocks` response is now retained immediately when the pending
range is fetched, preserving R2 partial evidence without creating an unbounded
raw block spool.

## Code and Tests

The exact implementation diff is published in:

```text
sanitized-code/canonical-state-v1/kefi_finalized_rpc_reconciliation_canary_v1.rs
```

New release candidate SHA-256:

```text
b6a0e501b67b11ebaba00d67b395673ff8f98218efc2ed127cc5f92fc2a27f54
```

Focused production-binary test command:

```text
cargo test --release --bin kefi_finalized_rpc_reconciliation_canary_v1
16 passed, 0 failed
```

The suite now explicitly proves:

- finalized fifth-slot notice before its identities waits, then seals only
  after the identities are present;
- a higher finality notification cannot stand in for the fifth slot's own
  source-finalized observation;
- permanently missing fifth-slot source evidence cannot reach `ReadyToSeal`;
- five zero-relevant produced slots can still complete;
- the 10-dispatcher plus 6-inventory reservation remains capped at 16;
- the no-range single-slot loop remains enabled.

## R2 Review-Only Deployment Plan

R2 is **not authorized to run**. The current installed R1 binary remains
unchanged at `4b9d34aba7b76dfaeb82760490e2d18ff984d533e58e45514140dde438a75c2b`.
The newly built artifact has not been installed.

If separately approved, installation must revalidate all hashes, preserve the
current R1 binary as rollback, and install only the following source artifact:

```bash
set -euo pipefail
src=/root/bot/EdgeSniffer/rust-sidecar/target/release/kefi_finalized_rpc_reconciliation_canary_v1
dst=/usr/local/lib/edgesniffer/edgesniffer-kefi-finalized-rpc-canary-v1
new_sha=b6a0e501b67b11ebaba00d67b395673ff8f98218efc2ed127cc5f92fc2a27f54
old_sha=4b9d34aba7b76dfaeb82760490e2d18ff984d533e58e45514140dde438a75c2b
backup="${dst}.pre-r2-capture-completeness-$(date -u +%Y%m%dT%H%M%SZ)"

test "$(sha256sum "$src" | awk '{print $1}')" = "$new_sha"
test "$(sha256sum "$dst" | awk '{print $1}')" = "$old_sha"
test ! -e "$backup"
cp --preserve=mode,ownership,timestamps "$dst" "$backup"
install -m 0755 "$src" "${dst}.new"
test "$(sha256sum "${dst}.new" | awk '{print $1}')" = "$new_sha"
mv -f "${dst}.new" "$dst"
test "$(sha256sum "$dst" | awk '{print $1}')" = "$new_sha"
printf 'ROLLBACK_BACKUP=%s\n' "$backup"
```

Rollback, also separately approval-gated:

```bash
set -euo pipefail
backup=/usr/local/lib/edgesniffer/edgesniffer-kefi-finalized-rpc-canary-v1.pre-r2-capture-completeness-REPLACE_WITH_RECORDED_TIMESTAMP
dst=/usr/local/lib/edgesniffer/edgesniffer-kefi-finalized-rpc-canary-v1
old_sha=4b9d34aba7b76dfaeb82760490e2d18ff984d533e58e45514140dde438a75c2b
test "$(sha256sum "$backup" | awk '{print $1}')" = "$old_sha"
install -m 0755 "$backup" "${dst}.rollback"
mv -f "${dst}.rollback" "$dst"
test "$(sha256sum "$dst" | awk '{print $1}')" = "$old_sha"
```

Any R2 run requires fresh authorization for installation and exactly one new
bounded diagnostic. V8 remains inactive; the dust service remains untouched.

## Preservation and Remaining Risk

The R1 root was only read during this investigation and remains unchanged:

```text
/mnt/edgesniffer-canonical-hot/canonical-state-v1-live-diagnostic-r1-20261016T145835Z
```

The correction prevents a false complete seal but does not establish a provider
ordering guarantee. A future R2 could still be inconclusive if the provider
never delivers a required identity. That would be a valid fail-closed result,
not evidence of complete coverage.
