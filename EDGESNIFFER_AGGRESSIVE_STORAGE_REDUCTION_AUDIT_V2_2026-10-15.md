# EdgeSniffer Aggressive Storage Reduction Audit V2

**Scope:** read-only VPS and NFS audit completed 2026-10-15. No evidence was
deleted, moved, released, compacted, or reclassified. No service was restarted.

## Executive decision

The apparent **2.5 TB** problem is real as a capacity-planning problem, but it
is not 2.5 TB of one active data stream. The three mounted filesystems currently
show **2.16 TiB used** (root 927 GiB, kEFi hot 328 GiB, NFS 908 GiB). Filesystem
usage includes the OS, package/tool caches, and any replicated archive; it must
not be described as 2.16 TiB of unique scientific evidence until manifest-level
deduplication is complete.

The dominant cost is legacy/raw promotion and control material, especially
closed kEFi and 2CQ archives. The compact Canonical State design is not the
current storage driver. The right cure is a one-time evidence-retention
decision, not another emergency cleanup and not more permanent raw capture.

| Mount | Used / size | Free | Status |
|---|---:|---:|---|
| `/` | 927 / 1,007 GiB | 70 GiB | critical (93%) |
| `/mnt/edgesniffer-kefi-hot` | 328 / 492 GiB | 139 GiB | below its 160 GiB protected reserve |
| `/mnt/edgesniffer-expansion` | 908 / 1,000 GiB | 93 GiB | critical (91%); archive only, not an ingest buffer |

## Method and limits

* Size measurements use `du -x` on local mounts and per-root `du -sx` on NFS.
  They count filesystem blocks, not logical object identity.
* No hard-link, reflink, or cross-mount archive equivalence is assumed. A local
  and NFS copy are separately measured physical copies, but are **not** claimed
  to be independently necessary evidence.
* NFS roots that exceeded the bounded 20-second non-invasive scan are marked
  `SIZE NOT YET BOUNDED`, not guessed.
* “Protected” means known active/frozen/pinned or unresolved. “Retention
  review” means it may be removable, but that fact has **not** been proven.

## What is actually consuming capacity

### Top 20 measured consumers

| # | Path | Physical size | Class | Status / principal dependency |
|---:|---|---:|---|---|
| 1 | `/mnt/edgesniffer-expansion/kefi-cold-archives` | 407.0 GiB | RETENTION REVIEW | historical kEFi raw/archive material; replay and forensic value unknown per partition |
| 2 | `/mnt/edgesniffer-kefi-hot/kefi-capture-first-promotions-v8` | 254.0 GiB | RETENTION REVIEW | V8 promotion windows; V8 is currently inactive but this is not automatically releasable |
| 3 | `/mnt/edgesniffer-expansion/2cq-archive` | 164.3 GiB | RETENTION REVIEW | 2CQ archive; reproducibility/forensics policy required |
| 4 | `/mnt/edgesniffer-expansion/2cq-historical-archive` | 130.3 GiB | RETENTION REVIEW | historical 2CQ archive; provenance/replay decision required |
| 5 | `/mnt/edgesniffer-expansion/2cq-full-meta` | 97.7 GiB | RETENTION REVIEW | raw 2CQ full-meta; decoder/source forensics possible |
| 6 | `/root/kefi-capture-first-promotions-v8` | 91.0 GiB | RETENTION REVIEW / DUPLICATION AUDIT | likely earlier/root copy of V8 promotion material; equality not yet proved |
| 7 | `/root/kefi-capture-first-control-v8` | 58.5 GiB | RETENTION REVIEW | V8 controls/cache; historical control reconstruction |
| 8 | `/root/kefi-capture-first-promotions-v7` | 35.2 GiB | RETENTION REVIEW | superseded V7 evidence and incident replay |
| 9 | `/root/kefi-capture-first-control-v7` | 29.6 GiB | RETENTION REVIEW | superseded V7 controls/cache |
| 10 | `/mnt/edgesniffer-expansion/kefi-capture-first-promotions-v5` | 26.3 GiB | RETENTION REVIEW | old promotion archive; no deletion proof yet |
| 11 | `/mnt/edgesniffer-kefi-hot/kefi-capture-first-control-v5` | 20.8 GiB | RETENTION REVIEW | old control material |
| 12 | `/root/bot` | 19.1 GiB | MIXED | source, builds, 8ycb, and active research tooling; needs sub-audit |
| 13 | `/root/kefi-work` | 15.3 GiB | MIXED / BUILD REVIEW | work products and build output; ownership/dependency audit required |
| 14 | `/mnt/edgesniffer-kefi-hot/.canonical-finalized-rpc-build-20261015T0900Z` | 12.9 GiB | DELETE CANDIDATE | isolated reproducible build; no user-space open handle observed |
| 15 | `/mnt/edgesniffer-expansion/evidence-lake-v1` | 12.7 GiB | PROTECTED pending catalog audit | retention/catalog layer; do not touch blindly |
| 16 | `/mnt/edgesniffer-kefi-hot/kefi-capture-first-live-v2` | 7.8 GiB | RETENTION REVIEW | historical live-window data |
| 17 | `/mnt/edgesniffer-kefi-hot/.canonical-inventory-conformance-build-20261015T0738Z` | 6.8 GiB | DELETE CANDIDATE | isolated reproducible build; no user-space open handle observed |
| 18 | `/mnt/edgesniffer-kefi-hot/kefi-capture-first-control-v8` | 5.7 GiB | PROTECTED / ACTIVE-RELATED | V8 control cache; dependency/pin check still required |
| 19 | `/mnt/edgesniffer-kefi-hot/kefi-capture-first-live-v1` | 5.1 GiB | RETENTION REVIEW | historical live-window data |
| 20 | `/mnt/edgesniffer-kefi-hot/kefi-pre-h3-actionability-qualification-v1` | 4.6 GiB | PROTECTED | frozen qualification evidence and audit trail |

Additional material of note: `/root/kefi-yellowstone-prospective-v1` 4.6 GiB;
completed FRA labs total about 5.7 GiB; canonical canary fixtures total 1.2 GiB;
root recovery labs 1.0 GiB; Cargo and Rust tool caches about 1.2 GiB. The NFS
roots `edgesniffer-verified-archive-v1` and `verified-cold-archives` exceeded
the bounded scan; their exact contribution is not asserted here.

## Dataset challenge table

| Major dataset family | Original purpose | Still actively used? | Can useful information be compacted? | What deletion would permanently remove | Decision |
|---|---|---|---|---|---|
| V8 promotion windows (hot/root) | capture-first public context around wallet activity | V8 source is inactive; downstream dependency status unresolved | yes: preserve manifests, selected semantic events, decoder fixtures, and sealed wallet-decision packages | byte-identical source replay, new decoder bug analysis, and unmaterialized window reconstruction | compare manifests first; then retention decision per closed partition |
| V7 promotions/controls | predecessor cohort/incident investigation | no known active consumer, but postmortem evidence | usually | exact V7 reproduction and future root-cause tests | retention review, not cleanup by age |
| V5 controls/promotions/live roots | prior exploratory capture | no known active consumer | likely | legacy comparison/replay | compact candidate after dependency proof |
| 2CQ raw/full-meta/archive | original teacher-wallet and execution research | no current claim of an active collector dependency | partly; raw remains needed only for selected forensic fixtures | raw replay, independent decoder validation, exact historical source reconstruction | explicit 2CQ research-retention decision required |
| kEFi cold archives | reverse-engineering history/replay | historical research dependency unresolved | yes for portions after a reproducibility decision | historical replays and ability to rederive excluded fields | largest retention-review item |
| PRE_H3 qualification | sealed frozen contract/cohort | frozen/auditable evidence | only through an approved immutable compaction procedure | auditability and qualification provenance | protected |
| evidence lake / verified archives | manifests, receipts, archive coordination | catalog likely active | already intended as compact reference layer | ability to prove archive identity/release state | protected until catalog audit |
| canonical build roots | compilation outputs for bounded offline tests | no active process handle; reproducible from source | not applicable | only local rebuild time | immediate approval-gated delete candidates |

## Classification and recoverable capacity

### DELETE CANDIDATE — safe only after explicit approval

| Exact path | Size | Evidence checked | Proposed action |
|---|---:|---|---|
| `/mnt/edgesniffer-kefi-hot/.canonical-finalized-rpc-build-20261015T0900Z` | 12.9 GiB | root-owned; `fuser` shows only the mount kernel, no user-space process | delete after final path/handle recheck |
| `/mnt/edgesniffer-kefi-hot/.canonical-inventory-conformance-build-20261015T0738Z` | 6.8 GiB | root-owned; same handle result | delete after final path/handle recheck |

**Immediate recoverable capacity: 19.7 GiB (20.6 GB decimal).** This does not
solve the hot-volume reserve problem: 139 GiB free would become roughly 159 GiB,
still below the 160 GiB reserve before a practical canary/build buffer.

### COMPACT CANDIDATE

Completed FRA labs, canonical TTL fixtures, old lab captures, debug traces, and
bounded canary inputs account for roughly **8–12 GiB** local. They should retain
only: immutable manifest, checksum, report, one minimum failing/success fixture,
and compact semantic output. This needs a partition dependency audit and an
approval; it is not an automatic delete list.

### RETENTION REVIEW

The V7/V8 promotion/control families on root/hot are about **489 GiB** before
any cross-root duplicate comparison. NFS historical 2CQ and kEFi families are
at least **838 GiB** in the measured roots alone. These are the only places a
dramatic reduction can come from. Their potential recovery is presently a
**decision range**, not a guaranteed number:

| Scenario | Estimated recovery | Preconditions |
|---|---:|---|
| Conservative | 19.7 GiB | approved removal of the two build trees only |
| Moderate | 30–45 GiB | build trees plus individually audited completed lab/debug roots |
| Aggressive, local | 200–400 GiB | manifest equality proves redundant/superseded V5–V8 raw/control partitions and science owner approves compaction/retirement |
| Aggressive, all mounts | 0.8–1.2 TiB potential review surface | explicit 2CQ/kEFi retention policy; verified replacement semantic evidence; NFS catalog and archive dependencies reconciled |

The last row is intentionally not called “recoverable” yet. It is the amount
that merits review, not permission to destroy it.

## Why the storage grew this large

1. Several generations retained their own raw promotion and control trees.
2. Some V8 promotion data exists both under root and the hot volume; whether it
is identical, overlapping, or complementary has not been proved.
3. NFS became an archive destination without a separate research-retention
decision for old raw material.
4. Bounded one-off canary/build directories were allowed to accumulate on the
protected hot volume.
5. Legacy capture-first workflows preserve much larger windows than the target
compact semantic journal. V8 itself was configured with a 6 GiB memory ring;
the large disk cost is promotion/window persistence, not the ring alone.

## Sustainable retention policy

This policy is proposed, not activated.

| Data type | Hot retention | Cold retention | Hard rule |
|---|---|---|---|
| Full-meta/raw validation fixture | one bounded fixture, 2 GiB max per canary, 7-day TTL after parity receipt | none unless it is a named decoder regression fixture | never an unbounded raw spool |
| Raw tracked-wallet decision bytes | compact sealed package per decision | retain with decision receipt | permanent because this is the audit-critical raw exception |
| Canonical semantic journal/checkpoints | active epoch + 30 days hot | immutable compact archive with manifest | semantic hash is independent of compression |
| Closed raw research partitions | local only while `PIN_ACTIVE` or unresolved replay/decoder dependency exists | one verified archive plus manifest/receipt | archive success never grants local deletion automatically |
| Debug traces / canary diagnostics | 7 days, 2 GiB per named run | none after report/fixture preservation | quota failure stops the run cleanly |
| Build artifacts | 10 GiB per build root; 7-day TTL | none | build outside a protected evidence volume where possible |
| Test fixtures | compact, versioned, bounded | repository or verified archive | retain only fixtures needed by regressions |
| Backups | one verified archival copy, not uncontrolled replicas | archive volume | receipt/catalog must identify the authoritative copy |

Daily-growth projection cannot honestly be inferred from the current snapshot:
V8 is inactive and the existing roots mix different generations and durations.
The next active producer must emit a daily storage ledger recording raw bytes,
semantic bytes, promotion bytes, and retained-fixture bytes separately. Without
that ledger, any “GB/day” claim would be invented.

## Exact approval-gated cleanup sequence

1. **Approve or reject the two build-tree deletions only.** Before execution,
   re-run `stat`, `fuser`, and a systemd/path reference search. Proposed
   commands, not executed:

   ```sh
   rm -rf /mnt/edgesniffer-kefi-hot/.canonical-finalized-rpc-build-20261015T0900Z
   rm -rf /mnt/edgesniffer-kefi-hot/.canonical-inventory-conformance-build-20261015T0738Z
   ```

2. Run a read-only manifest/content comparison between root and hot V8 promotion
   roots. Result must classify each partition as identical, overlapping, unique,
   or unknown. No path gets deleted in this step.
3. For each closed V5–V8 partition, create a dependency record with active
   references, frozen-contract references, decoder/replay dependencies, archive
   receipt, and compact replacement availability.
4. Ask for an explicit research decision per family: retain raw, compact after
   validation, or retire after receipt. Update the archive catalog and issue a
   release receipt only after that decision.
5. Separately decide the 2CQ retention objective. Its 421+ GiB NFS archive
   footprint cannot be justified merely by historical existence.

## Live-operation protection

`edgesniffer-kefi-full-meta-capture-first-v8.service` is **inactive/dead** with
`Result=success`; it cleanly stopped at `2026-10-15T15:47:16Z`, after normal
promotion messages. The unit is configured `Restart=on-failure`, so this was
not a crash/restart loop. The watchdog is still active. This audit did not
restart, change, or remove V8, the watchdog, 2CQ, or Canonical State services.

## Bottom line

EdgeSniffer does not need multi-terabytes of permanently retained raw material
to operate trustworthy future research. It needs a bounded canonical semantic
journal, retained raw bytes only for sealed tracked-wallet decisions and small
decoder fixtures, and an explicit review gate before historical raw evidence is
released. The immediate 19.7 GiB cleanup is straightforward but small; the
real win is resolving the approximately 1.3 TiB V7/V8/2CQ/kEFi historical raw
review surface deliberately and once.
