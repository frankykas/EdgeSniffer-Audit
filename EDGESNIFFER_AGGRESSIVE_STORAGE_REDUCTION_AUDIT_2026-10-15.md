# EdgeSniffer Aggressive Storage Reduction Audit

## Decision

The approximately 2.5 TB concern is directionally correct, but it is not one
undifferentiated EdgeSniffer dataset. Current allocated bytes visible on the
three mounts are approximately **2.35 TB**: root **927 GB used**, kEFi hot
volume **328 GB used**, and NFS **908 GB used**. Filesystem-used values include
operating-system and unrelated allocations; they are not summed as unique
research evidence. No hard-link or archive duplicate has been counted as a
second local copy in the table below.

The storage problem is primarily historical full candidate/window material,
not the compact Canonical State design.

## Largest Consumers

| Mount/path | Size | Classification | Why it exists | Consequence of deletion |
|---|---:|---|---|---|
| `/mnt/.../kefi-capture-first-promotions-v8` | 272.7 GB | RETENTION REVIEW | V8 promotion/shared windows | loses V8 replay/recovery and unresolved dependency evidence |
| `/root/kefi-capture-first-promotions-v8` | 97.7 GB | RETENTION REVIEW / likely duplicate review | earlier/local V8 promotion copy | cannot delete until content/manifest comparison proves redundancy |
| `/root/kefi-capture-first-control-v8` | 62.8 GB | RETENTION REVIEW | V8 controls/partition cache | loses historical control reconstruction |
| `/root/kefi-capture-first-promotions-v7` | 37.8 GB | RETENTION REVIEW | superseded V7 evidence | loses V7 forensic/replay ability |
| `/root/kefi-capture-first-control-v7` | 31.7 GB | RETENTION REVIEW | superseded V7 controls | same |
| `/mnt/.../kefi-capture-first-control-v5` | 22.3 GB | RETENTION REVIEW | old control cache | likely compact/archive candidate only after dependency audit |
| Canonical finalized-RPC build root | 13.8 GB | DELETE CANDIDATE | isolated Cargo build | none beyond local rebuild time |
| `/mnt/.../kefi-capture-first-live-v2` | 8.4 GB | RETENTION REVIEW | historical live window copy | loses legacy replay |
| Canonical inventory build root | 7.3 GB | DELETE CANDIDATE | isolated Cargo build | none beyond local rebuild time |
| `/mnt/.../kefi-capture-first-control-v8` | 6.1 GB | PROTECTED / active-related | current control cache | must inspect current dependency pins |
| `/root/kefi-yellowstone-prospective-v1` | 4.9 GB | RETENTION REVIEW | prior source/proofs | loses prior gap/source investigation ability |
| `/mnt/.../kefi-pre-h3-actionability-qualification-v1` | 5.0 GB | PROTECTED | frozen qualification evidence | invalidates audit trail |
| `/mnt/.../kefi-fra-capture-first-v4-lab-*` | 4.1 GB | COMPACT CANDIDATE | completed lab capture | retain manifest/receipts plus compact semantic fixture |

`/mnt/edgesniffer-expansion` contains archived 2CQ/kEFi evidence, contracts,
and evidence lake data and has only 93 GB free. It is not a viable ingestion
buffer and archive presence alone does not authorize releasing any local copy.

## Reduction Classes

### DELETE CANDIDATE

* The two isolated build trees: **about 20.6 GB** immediate recovery after
  explicit approval and a final `fuser`/ownership recheck.
* Cargo/debug build outputs under `/root/kefi-work`, recovery labs, and old
  V3 scratch traces: approximately **16-20 GB**, but each needs one grouped
  path review because some may be referenced by reports.

### COMPACT CANDIDATE

* Completed `kefi-fra-*` laboratories, old canonical TTL fixtures, old
  cache benchmarks: roughly **8-12 GB** local. Keep a manifest, hashes,
  reports, and minimum decoder fixtures; discard only raw/debug payloads once
  an approval confirms no replay dependency.

### RETENTION REVIEW

* V7/V8 promotions and controls: roughly **500 GB** across root/hot before
  checking cross-volume duplication. These are the only route to a dramatic
  reduction. They require a manifest-level comparison and an explicit
  scientific decision for each closed cohort; no broad `rm -rf` is safe.
* 2CQ historical archives on NFS: retain until the user decides whether those
  studies remain reproducibility requirements.

### PROTECTED

Active V8-related roots, the PRE_H3 frozen cohort, current evidence ledger,
active receipts/manifests, and any `PIN_ACTIVE` dependency remain untouched.

## Capacity Scenarios

| Scenario | Recoverable local capacity | Preconditions |
|---|---:|---|
| Conservative | 20.6 GB | remove only two approved isolated build trees |
| Moderate | 35-50 GB | plus approved old build/debug/lab artifacts after path review |
| Aggressive | 300-500+ GB | manifest/provenance audit proves superseded V7/V8 promotion/control copies redundant or reducible |

## Sustainable Policy

* Raw filtered source: retain only bounded validation fixtures, 24-72h TTL,
  hard quota, receipt and hash retained permanently.
* Semantic journals: retain indefinitely; target compact event records,
  checkpoints, and sealed decision packages.
* Closed research partitions: archive immutable compressed partitions with
  manifest/receipt; keep local copies only while an active dependency pin or
  unresolved replay/decoder question exists.
* Debug traces and builds: quotas of 10 GB per lab/build root, 7-day TTL.
* Backups: one verified archive plus manifests, never several unmanaged local
  copies.
* Capacity guard: stop before the protected reserve, not after it.

The current legacy V8 shared-window approach grows far faster than a compact
semantic journal. Canonical State must not retain an indefinite raw firehose.

## Approval-Gated First Cleanup

Before any deletion, recheck ownership, path identity, and open handles, then
ask approval to remove exactly:

```sh
rm -rf /mnt/edgesniffer-kefi-hot/.canonical-finalized-rpc-build-20261015T0900Z
rm -rf /mnt/edgesniffer-kefi-hot/.canonical-inventory-conformance-build-20261015T0738Z
```

This recovers about 20.6 GB but does not by itself solve the full retention
problem. The next highest-value approved task is a content/manifest
deduplication audit of V7/V8 promotions and controls, not another archive
framework.

## V8 Operation

V8 is inactive with a clean systemd stop at `2026-10-15T15:47:16Z`; the last
log showed normal promotion activity before the explicit stop. The watchdog
and evidence ledger are active. No restart or change was made.
