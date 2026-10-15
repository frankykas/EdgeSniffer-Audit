# EdgeSniffer Hot Storage Recovery Batch - 2026-10-15

## Scope and outcome

This is a read-only, approval-ready review of `/mnt/edgesniffer-kefi-hot`.
No file was deleted, moved, truncated, archived, released, or modified. No
service was restarted or reconfigured.

At measurement time the hot volume had `148,326,891,520` free bytes
(`138.140 GiB`). The stated operating target is a `160 GiB` protected reserve
plus `15 GiB` of headroom, or `175 GiB` before a bounded canary writes output.

The review found `21.346 GiB` of immediately disposable, inactive build output.
That is safe to retire but **does not meet the 175 GiB target by itself**. The
only additional large hot-volume candidate found is a `17.311 GiB` V5 control
cache. It is derived data, but is not proven byte-duplicated by a retained raw
source; it is therefore deliberately excluded from the approved deletion batch.

## Revalidated immediately disposable batch

All four directories below are on `/dev/vdc1`, owned by `root:root`, have no
open file handles other than the mount itself, no process command-line
references, and no references in `/etc/systemd/system` or `/etc/edgesniffer`.
They contain Cargo build output, cached crates, logs, and copied source trees.
Their reproducible source remains in the audited repository or installed
release locations; none is a capture, receipt, manifest, checkpoint, or
scientific evidence root.

| Exact path | Bytes | GiB | Evidence of dispensability |
|---|---:|---:|---|
| `/mnt/edgesniffer-kefi-hot/.canonical-finalized-rpc-build-20261015T0900Z` | 13,605,298,172 | 12.673 | Isolated temporary Canonical RPC build tree; last build metadata 2026-10-15 08:07 UTC; no active handle/reference. |
| `/mnt/edgesniffer-kefi-hot/.canonical-inventory-conformance-build-20261015T0738Z` | 7,218,000,859 | 6.723 | Isolated temporary inventory-conformance build tree; no active handle/reference. |
| `/mnt/edgesniffer-kefi-hot/kefi-capture-first-build` | 1,066,090,987 | 0.993 | Old Cargo target/release build cache from 2026-10-10; installed service binary is elsewhere; no active handle/reference. |
| `/mnt/edgesniffer-kefi-hot/kefi-build-v8` | 1,030,968,755 | 0.960 | Old V8 Cargo/cache/log build directory from 2026-10-11; no active handle/reference. |
| **Total** | **22,920,358,773** | **21.346** | |

### Expected result if approved

- Current free: `138.140 GiB`
- Recovery from this exact batch: `21.346 GiB`
- Expected free after deletion: **`159.486 GiB`**

This remains `0.514 GiB` below the stated `160 GiB` reserve and `15.514 GiB`
below the requested `175 GiB` reserve-plus-headroom target. It is still useful
and safe recovery, but it is not sufficient to authorize the bounded canary.

## Additional candidates reviewed, not authorized here

### Expired lab artifacts - retention review, not approved

The following inactive lab roots have no active handles or systemd references:

| Exact path | GiB | Why not in the deletion batch |
|---|---:|---|
| `/mnt/edgesniffer-kefi-hot/kefi-fra-capture-first-v4-lab-20261010T1535Z` | 3.813 | Captured a failed/overloaded lab: queue full `4`, shared-burst failures `4`, incomplete frames `7`. It is a useful decoder/overload regression fixture unless explicitly retired. |
| `/mnt/edgesniffer-kefi-hot/kefi-fra-capture-first-v3-lab-20261010T1525Z` | 1.250 | Historical FRA lab fixture; no active dependency found, but no explicit regression-corpus decision. |
| `/mnt/edgesniffer-kefi-hot/kefi-fra-capture-first-v5-lab-20261010T1545Z` | 0.615 | Historical FRA lab fixture; same retention question. |
| `/mnt/edgesniffer-kefi-hot/kefi-fra-group-commit-lab-20261010T1500Z` | 0.283 | Group-commit failure/performance fixture. |
| Six smaller FRA/RabbitStream/dedup labs | 0.379 | Transport/queued-ingest/dedup test fixtures. |
| **Total labs** | **6.340** | Requires an explicit decision to sacrifice historical failure reproduction. |

Even the build batch plus every listed lab reaches only `165.826 GiB` free,
which remains below the requested `175 GiB` target.

### V5 control materializer cache - blocked pending retention decision

`/mnt/edgesniffer-kefi-hot/kefi-capture-first-control-v5/partition-cache`
contains 147 `evidence.frames.zst` payloads totaling `18,587,860,649` bytes
(`17.311 GiB`). The V5 service and watchdog are both inactive, with clean
stops, and no process or service references to this cache were found.

The cache receipts bind each payload to an original promotion-manifest SHA and
payload SHA. However, the NFS path
`/mnt/edgesniffer-expansion/kefi-capture-first-promotions-v5/shared-windows`
currently retains compact manifests/state, **not** byte-identical
`evidence.frames.zst` payloads. Consequently, this is a *derived historical
cache whose source provenance remains available*, but it is not a proven raw
duplicate. Deleting it would sacrifice the locally retained V5 raw full-meta
replay material. It needs explicit scientific-retention approval, not a cache
cleanup authorization.

If that separate retirement decision is approved after a per-payload manifest
preflight, the build batch plus V5 cache would yield `176.798 GiB` free. That
would clear the user-stated 175 GiB target, but does not change the fact that
the raw V5 replay capability is being knowingly retired.

## Exact approval-ready deletion batch

The only batch that is verified nonessential now is the four build directories
listed in **Revalidated immediately disposable batch**. The approval should be
limited to those exact four directory roots and their contents. It must not be
expanded to the V5 cache, labs, canaries, V8 promotion data, or any evidence
root.

Suggested execution, after a fresh just-before-delete handle/path preflight:

```bash
sudo rm -rf --one-file-system -- \
  /mnt/edgesniffer-kefi-hot/.canonical-finalized-rpc-build-20261015T0900Z \
  /mnt/edgesniffer-kefi-hot/.canonical-inventory-conformance-build-20261015T0738Z \
  /mnt/edgesniffer-kefi-hot/kefi-capture-first-build \
  /mnt/edgesniffer-kefi-hot/kefi-build-v8
```

The command is shown for approval only and was **not run**. Before execution,
record an immutable eviction receipt with paths, `du -sb` values, mount checks,
active-handle checks, and measured free-space delta.

## V8 inactive status

V8 is inactive because the capacity guard cleanly stopped it at
`2026-10-15T20:23:32.696595Z` when root had only `68.836 GiB` free. The pause
record is:

- path: `/mnt/edgesniffer-kefi-hot/kefi-capture-first-live-v8/operations/capacity-pause.json`
- reason: `LOCAL_STORAGE_BELOW_EMERGENCY_WATERMARK`
- `collectorStoppedVerified: true`
- `manualResumeRequired: true`

The service exited successfully at `2026-10-15 15:47:16 UTC` with status `15`.
Its systemd unit has `ConditionPathExists=!…/capacity-pause.json`, so it is
correctly prevented from restarting while the pause marker remains. The current
V8 guard is healthy and sees `138.14 GiB` free on hot plus `140.323 GiB` on
root, but it intentionally does not clear a manual-resume pause. No restart or
pause-file change was performed in this review.

## Canary gate discrepancy

The installed canonical-canary unit currently declares a `6 GiB` total capture
cap and a `240 GiB` protected-reserve environment value
(`CANONICAL_CANARY_PROTECTED_RESERVE_BYTES=257698037760`). This is stricter
than the stated 160 GiB reserve. Therefore even a `175 GiB` free result is not
sufficient for that unit as currently configured. Resolving that configuration
policy is outside this read-only task; it must be addressed explicitly before a
live canary is authorized.

## Recommended next action

Approve the four-directory build cleanup to recover `21.346 GiB` safely. Then
make one explicit retention decision: either retain the V5 raw cache for replay
or authorize a separately receipted retirement of it to cross the 175 GiB
hot-volume target. Do not treat it as a duplicate and do not alter the V8 pause
until storage policy and the canary reserve discrepancy are resolved.
