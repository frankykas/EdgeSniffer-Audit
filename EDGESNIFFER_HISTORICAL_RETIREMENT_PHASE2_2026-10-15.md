# EdgeSniffer Historical Retirement: Phase 2

**Base:** `87d449d011eb2cb7db61b60b9555df8a247dac33`  
**Mode:** read-only. No release, deletion, archive migration, restart, or
Canonical State canary was performed.

## Decision

There is **no verified NFS equivalent for any root V7/V8 promotion or control
partition**. The NFS kEFi cold archive contains older Oct 5–10 capacity-recovery
material; the root V7/V8 evidence begins Oct 11 and the hot V8 source later
begins Oct 14. Consequently, no substantial root release batch can be honestly
authorized from existing archive evidence.

**Verified archived local-release candidates: 0 GiB.**

This is a fail-closed result, not a lack of effort: retaining a source archive
that predates a local partition does not establish identity, completeness, or
recoverability for that partition.

## 1. NFS equivalence comparison

### Archive evidence inspected

`/mnt/edgesniffer-expansion/kefi-cold-archives` has four roots:

| NFS archive root | Relevant provenance | Relation to V7/V8 |
|---|---|---|
| `20261007T-capacity-recovery` | receipt-bound Oct 5 partitions and independent analysis release | predates V7/V8 by several days |
| `20261008-capacity-recovery/legacy` | pre-Oct 8 legacy/v3 material | predates V7/V8 |
| `20261011-replication-capacity/legacy/source-spool-staging` | material last modified Oct 10 | predates V7/V8 |
| `kefi-frozen-rules` | frozen rule artifact | not a promotion/control evidence archive |

The verified-archive roots contain 2CQ material and receipts, not V7/V8 kEFi
promotion/control partitions. The archive roots are readable: their small
receipts/manifests and `*.sha256` inventories can be read from NFS, including
the Oct 7 capacity-recovery receipts that attest `FULL_SHA256_MANIFEST_MATCH`.
Those receipts bind only their named sources (for example,
`/root/kefi-work/partitions`), not the later V7/V8 roots.

### Partition comparison result

| Local family | Local evidence | NFS identity/receipt match | Classification | Reason |
|---|---:|---|---|---|
| `/root/kefi-capture-first-promotions-v7` | 35.2 GiB, 630 manifests, Oct 11 05:19–15:04 UTC | none | PROTECTED_DEPENDENCY | source service ended `timeout`; retained incident/replay evidence has no verified replacement |
| `/root/kefi-capture-first-control-v7` | 29.6 GiB, 309 manifests + 309 receipts, Oct 11 05:27–15:36 UTC | none | UNIQUE_RETENTION_REVIEW | compact replacements are not proven; no archived equivalent |
| `/root/kefi-capture-first-promotions-v8` | 91.0 GiB, 623 manifests, Oct 11 15:09–Oct 12 02:15 UTC | none | UNIQUE_RETENTION_REVIEW | separate historical V8 run; NFS archive catalog predates it |
| `/root/kefi-capture-first-control-v8` | 58.5 GiB, 408 manifests + 408 receipts, Oct 11 15:30–Oct 12 02:13 UTC | none | UNIQUE_RETENTION_REVIEW | no matching NFS partition identities or receipt source binding |

**Exact duplicates:** 0 partitions / 0 GiB.  
**Partial overlap:** none proven.  
**Unproven equivalence:** all 214.3 GiB of root V7/V8 promotion/control data.

No bulk hashing was run. It would be unable to find an archive copy when the
archive catalog/root chronology already excludes these later partition IDs, and
would consume scarce root/NFS I/O without improving the conclusion.

## 2. Frozen and operational dependencies

* No active process command line referenced the reviewed root V7/V8 paths.
* V7 capture is failed/inactive (`Result=timeout`); V8 capture is cleanly
  inactive (`Result=success`); V8 control service and timer are inactive.
* The absence of a live reader proves only that these paths are not in the
  current hot path. It does **not** prove frozen research, historical
  equivalence, or decoder-incident dependencies have been discharged.
* The approved Canonical State implementation still requires historical old-vs-
  new equivalence and failure/restart fixture evidence. Until an equivalent
  compact, hash-bound fixture set is produced and accepted, the relevant raw
  windows remain a future migration dependency.

## 3. Concrete release batch

### Scientific evidence batch

**None.** There is no honest large release batch from root V7/V8 today.

### Already-proven non-evidence batch

The Phase 1 batch remains valid and is the only current approval-ready release:

| Path | Size | Filesystem recovery | Retained evidence | Approval needed |
|---|---:|---|---|---|
| `/mnt/edgesniffer-kefi-hot/.canonical-finalized-rpc-build-20261015T0900Z` | 12.9 GiB | hot | source, commit, tests, reports; rebuildable | explicit deletion approval |
| `/mnt/edgesniffer-kefi-hot/.canonical-inventory-conformance-build-20261015T0738Z` | 6.8 GiB | hot | source, commit, tests, reports; rebuildable | explicit deletion approval |

This is **19.7 GiB hot**, not root capacity. It is deliberately not presented
as a solution to root pressure.

## 4. What must be preserved before unique raw retirement

The future Canonical State architecture need not keep every legacy raw window
forever. But before a unique V7/V8 raw partition may be retired, a documented
compact replacement must retain:

1. the partition manifest and full-file SHA-256 inventory;
2. source/epoch/continuity and hard-wall disposition;
3. canonical semantic events with causal-order proof;
4. pool identity, candidate membership, eligibility/exclusion, and F01–F13
   equivalence output for any partition used as a regression baseline;
5. one minimal raw fixture for each decoder/ordering failure class; and
6. an immutable equivalence receipt proving the compact replacement reproduced
   the frozen historical behavior, plus a release receipt.

Only windows not selected as retained regression fixtures, and not referenced
by frozen studies or unresolved incident investigations, may then become
`VERIFIED_ARCHIVED_LOCAL_RELEASE_CANDIDATE`.

## 5. Capacity decision

| Filesystem | Recoverable now | Large evidence release supported now | Blocker |
|---|---:|---:|---|
| Root | 0 GiB | 0 GiB | no verified NFS equivalents for V7/V8 |
| Hot | 19.7 GiB with approval | 0 GiB from scientific evidence | build-cache approval; all evidence remains review-only |
| NFS | 0 GiB | 0 GiB | archive itself needs a catalog/retention decision |

## 6. Smallest next verification step

Do **not** move or delete the V7/V8 roots. The smallest useful next step is an
offline, bounded **compact-replacement selection audit**:

* select a fixed representative subset of closed V7/V8 partitions needed for
  Canonical State equivalence and decoder regressions;
* prove their compact semantic outputs against existing source data;
* issue a partition-level dependency record for every remaining local window;
* only then identify windows that are neither regression fixtures nor frozen
  evidence and prepare them for archive/receipt validation.

That is the first route to a substantial root-storage release. It must finish
before any large V7/V8 deletion can be called safe.
