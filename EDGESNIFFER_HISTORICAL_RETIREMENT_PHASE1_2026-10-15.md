# EdgeSniffer Historical Data Retirement: Phase 1

**Base audit:** `e37f92d4b8e4f8481f96d06ad4390808d4f093e2`  
**Mode:** read-only. No evidence was deleted, moved, released, compacted, or
rehash-copied. No service was restarted and no Canonical State canary ran.

## Decision

Phase 1 proves that the two apparently similar V8 promotion roots are **not
duplicates**. The root path is an Oct 11–12 run; the hot path is an Oct 14–15
run. Their sealed-window identifiers have an empty intersection. Therefore the
amount of large scientific evidence proven redundant in this phase is **0 GiB**.

This is still a useful retirement result: it prevents an unsafe deletion of
91.0 GiB under the false assumption that it duplicates 254.0 GiB on the hot
volume. The only fully approval-ready retirement batch remains the two isolated
reproducible build directories, totalling **19.7 GiB**.

## 1. Root/hot V8 duplication proof

### Evidence inspected

* directory and partition identifiers;
* per-window `manifest.json` files;
* manifest counts and date ranges;
* physical `du -x` sizes;
* file type inventories; and
* bounded direct set comparison of all partition IDs.

No bulk SHA-256 pass was appropriate: the identity sets already prove that the
two roots are disjoint. Hashing every multi-gigabyte payload could not change
that conclusion and would add I/O pressure to constrained volumes.

| Root | Physical size | Sealed-window IDs | Time span from manifest IDs | Result |
|---|---:|---:|---|---|
| `/root/kefi-capture-first-promotions-v8/shared-windows` | 91.0 GiB | 623 | 2026-10-11 15:09–2026-10-12 02:15 UTC | UNIQUE relative to hot V8 |
| `/mnt/edgesniffer-kefi-hot/kefi-capture-first-promotions-v8/shared-windows` | 253.9 GiB | 2,165 sealed plus one partial directory | 2026-10-14–2026-10-15 UTC | UNIQUE relative to root V8 |

* Partition-ID intersection: **0**.
* Same-ID manifests: **0** (there are no common IDs to compare).
* Proven `EXACT_DUPLICATE` bytes: **0 GiB**.
* Proven `PARTIAL_OVERLAP` bytes: **0 GiB** from available partition identity
  evidence. “No overlap” here means no same sealed partition identifier; it
  does not make a claim about abstractly similar market events.

The hot root contains one `.partial-*` directory and remains a retention-review
root. It is not proposed for retirement.

## 2. Historical dependency review

| Dataset | Size | Service/process consumer found | Evidence/replay status | Classification | Retirement conclusion |
|---|---:|---|---|---|---|
| Root V8 promotions | 91.0 GiB | no active PID; source service inactive cleanly | 623 manifests, historical raw window material | RETENTION REVIEW | not duplicate; requires a science owner decision |
| Root V8 controls | 58.5 GiB | control V8 service/timer inactive | 408 manifests + 408 receipts | RETENTION REVIEW | archive receipts do not prove a compact replacement; no retirement yet |
| Root V7 promotions | 35.2 GiB | V7 service failed/inactive | 630 manifests, 327 `.bin`, 319 `.zst`, 15 partials | RETENTION REVIEW | superseded operationally, but unique incident/replay evidence |
| Root V7 controls | 29.6 GiB | no active PID | 309 manifests + 309 receipts | RETENTION REVIEW | needs partition dependency/canonical replacement proof |
| Hot V5 controls | 20.8 GiB | V5 service inactive | 147 manifests + 148 receipts | RETENTION REVIEW | closed, but receipts alone are not deletion permission |
| NFS V5 promotions | 26.3 GiB | no active PID | 320 manifests | RETENTION REVIEW | archived historical raw; no local-release consequence, but NFS capacity matters |
| NFS V6 promotions | 1.0 GiB | no active PID | no manifests in root | RETENTION REVIEW | needs origin/receipt binding before retirement |
| NFS 2CQ archive | 164.3 GiB | no active PID found | 542 manifest/receipt-named files | RETENTION REVIEW | major policy decision; no deletion proof |
| NFS 2CQ full-meta | 97.7 GiB | no active PID found | 419 manifest/receipt-named files | RETENTION REVIEW | source/decoder forensic value unresolved |
| NFS 2CQ historical archive | 130.3 GiB | no active PID found | no manifest/receipt files at root scan | RETENTION REVIEW / HIGHEST AUDIT PRIORITY | not releasable until provenance is reconstructed |
| NFS kEFi cold archives | 407.0 GiB | no active PID found | 100 manifest/receipt-named files | RETENTION REVIEW | largest target; needs per-partition catalog and compact replacement proof |

The absence of active processes is evidence that these paths are not in a live
hot path. It is **not** evidence that frozen studies, historical equivalence,
or decoder incident analysis no longer need them.

### Current operational status

* V5 source: inactive, prior result `success`.
* V7 source: failed/inactive; prior result `timeout`.
* V8 source: inactive, prior result `success`; stopped at
  `2026-10-15T15:47:16Z` after normal promotion logs.
* V8 control service and timer: inactive.
* No active process command line referenced the reviewed V5/V7/V8 promotion or
  control roots at audit time.

The V8 stop is a clean systemd stop (`ExecMainStatus=15`, `Result=success`),
not an observed crash. Available journal data establishes the stop, but does
not identify the human or orchestration actor that sent SIGTERM. This is not a
storage-retirement blocker.

## 3. First approval-ready retirement batch

No large evidence partition is yet proven redundant or dispensable. The first
batch therefore contains only reproducible build output.

| Source path | Filesystem | Size | Proof of dispensability | Remains after retirement | Irreversible loss |
|---|---|---:|---|---|---|
| `/mnt/edgesniffer-kefi-hot/.canonical-finalized-rpc-build-20261015T0900Z` | hot | 12.9 GiB | root-owned isolated build root; no user-space open handle observed | source tree, tests, commits, reports; rebuild possible | local compile cache/time only |
| `/mnt/edgesniffer-kefi-hot/.canonical-inventory-conformance-build-20261015T0738Z` | hot | 6.8 GiB | same | source tree, tests, commits, reports; rebuild possible | local compile cache/time only |

**Expected recovery:** 19.7 GiB on the hot volume only.  
**Rollback:** none after deletion; regenerate by rebuilding from the recorded
source/commit.  
**Pre-execution rechecks:** exact path, owner, mount identity, `fuser`,
`lsof` if available, and systemd/config path-reference search.

Proposed commands, deliberately **not executed**:

```sh
rm -rf /mnt/edgesniffer-kefi-hot/.canonical-finalized-rpc-build-20261015T0900Z
rm -rf /mnt/edgesniffer-kefi-hot/.canonical-inventory-conformance-build-20261015T0738Z
```

This would raise hot free space from roughly 139 GiB to roughly 159 GiB. It
still does not clear the 160 GiB protected reserve with an operational margin;
it is safe cleanup, not a complete capacity solution.

## 4. Recoverable capacity by filesystem

| Filesystem | Proven now | Review surface, not approved recovery | What is required next |
|---|---:|---:|---|
| Root | 0 GiB | 214.3 GiB in V7/V8 promotion/control roots | dependency manifests plus compact-replacement comparison |
| Hot | 19.7 GiB | 280+ GiB V5/V8 promotions/controls/live material | active/pin check then closed-partition proof |
| NFS | 0 GiB | at least 826 GiB measured 2CQ/kEFi archival families | catalog reconstruction, receipt binding, explicit research-retention approval |

## 5. Explicit approvals required

1. Approve deletion of the two named build directories only.
2. Approve a **read-only, partition-level manifest/receipt comparison** of
   root V7/V8 versus NFS kEFi archives. This is the next likely path to a
   meaningful local retirement candidate; it will not delete anything.
3. Approve a separate 2CQ retention policy decision before treating any of its
   421+ GiB NFS archive family as compactable or disposable.
4. Approve any future release only after a dependency record proves:
   closed/sealed status, no active reader/writer, verified replacement evidence,
   no frozen-study dependency, and an immutable release receipt.

## Recommended next action

Approve the 19.7 GiB build-cache deletion if immediate hot-space relief is
needed. In parallel, perform the next **read-only V7/V8-to-NFS receipt matching
audit**. It is the first place where tens or hundreds of GiB may be released
without sacrificing reproducibility, but Phase 1 proves that broad path-name
similarity is not enough to authorize such a release.
