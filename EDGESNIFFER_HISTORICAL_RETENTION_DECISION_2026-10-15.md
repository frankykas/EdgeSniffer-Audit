# EdgeSniffer Historical Retention Decision

**Base:** `d3f010c83b6ba10dbfde981459ff4582d4f47fc1`  
**Mode:** read-only. This proposal does not delete, compact, move, archive, or
release any file and does not restart a service.

## Decision

The first substantial root-space retirement can be made **without retiring a
single unique raw promotion window**: V7 and V8 control partition caches hold
exact compressed copies of promotion evidence that remains under the matching
root promotion trees.

| Control cache | Exact manifest-bound duplicate payload | Promotion source retained |
|---|---:|---|
| `/root/kefi-capture-first-control-v7/partition-cache/*/evidence.frames.zst` | 25.79 GB / 24.02 GiB | `/root/kefi-capture-first-promotions-v7/shared-windows` |
| `/root/kefi-capture-first-control-v8/partition-cache/*/evidence.frames.zst` | 53.94 GB / 50.24 GiB | `/root/kefi-capture-first-promotions-v8/shared-windows` |
| **Total first proposed release** | **79.74 GB / 74.26 GiB** | raw source remains local |

This is a cache eviction, not a loss of the raw evidence. The control cache can
be reconstructed from the still-retained promotion evidence. It does sacrifice
the immediate materialized-cache read path and makes a future control replay
slower; it does not sacrifice source replay.

## 1. Proof of duplicate control payloads

Each control-cache object has a `cache-receipt.json` naming a
`compressed_sha256`. Each matching promotion partition manifest records the
same `compressedSha256`, raw hash, slot interval, and compressed size.

| Family | Control receipts | Exact one-to-one manifest matches | Missing / ambiguous | Bounded direct SHA-256 checks |
|---|---:|---:|---:|---:|
| V7 | 309 | 309 | 0 / 0 | not necessary after complete manifest binding |
| V8 | 408 | 408 | 0 / 0 | 5 sampled payloads, all equal |

For V8, all 408 cache payload sizes matched a unique promotion payload size,
and each cache receipt’s compressed hash matched exactly one promotion manifest.
The direct SHA-256 samples independently matched byte-for-byte. The archived
control-cache raw files are therefore `EXACT_DUPLICATE` of retained local
promotion files, not merely similar capture windows.

## 2. Required retention and small regression corpus

The following corpus is deliberately small. It supports future decoder,
ordering, restart/interruption, and cache-materialization regressions. It does
**not** claim to reproduce all historical behavior.

| Class | Required raw fixture | Why retain |
|---|---|---|
| Clean successful decode / baseline | `...promotions-v7/shared-windows/kefi-20261011T090941259Z` | ordinary sealed V7 shared window with manifest, candidate, 120-second pre-entry capture |
| Multi-decision causal ordering | `...promotions-v8/shared-windows/kefi-20261011T224446111Z` | sealed V8 window with two wallet candidates, distinct observed transaction indexes |
| Clean V8 cache/materialization pairing | `...promotions-v8/shared-windows/kefi-20261011T174519003Z` plus its V8 control cache receipt | independently demonstrates source-to-cache identity binding |
| Interrupted capture / partial write | `...promotions-v7/shared-windows/.partial-kefi-20261011T103242834Z` | large `evidence.frames.partial` instance |
| Interrupted capture with a sealed side artifact | `...promotions-v7/shared-windows/.partial-kefi-20261011T120137861Z` | contains both partial and `.zst` artifacts |
| Restart/rename edge | `...promotions-v7/shared-windows/.partial-kefi-20261011T123944066Z` | contains `.zst.tmp` and `evidence.frames.sealed` artifacts |

The root V7 service’s terminal `timeout` remains documented incident evidence.
The retained partial fixtures cover capture interruption mechanics. The window
manifests themselves do not encode a source-gap flag; therefore no V7/V8
window can honestly be called a definitive “source-gap fixture.” A known
source-gap test must remain separately sourced from its authoritative incident
receipt/log when available.

## 3. Classification of the 214.3 GiB root V7/V8 review surface

| Material | Classification | Bytes / rationale |
|---|---|---|
| V7 control `partition-cache/*/evidence.frames.zst` | RETIREMENT_CANDIDATE | 25.79 GB proven exact duplicates of retained V7 promotions |
| V8 control `partition-cache/*/evidence.frames.zst` | RETIREMENT_CANDIDATE | 53.94 GB proven exact duplicates of retained V8 promotions |
| Six fixtures above, manifests, receipts, and promotion data needed for their raw files | REGRESSION_CORPUS | preserve, exact corpus bytes not presented as an all-history replacement |
| V7 promotion windows not selected as corpus | RETIREMENT_CANDIDATE after explicit replay-loss acceptance | no active process dependency identified; no NFS copy; deleting loses raw replay |
| V8 promotion windows not selected as corpus | RETIREMENT_CANDIDATE after explicit replay-loss acceptance | same; broader retirement should wait for Canonical State equivalence selection |
| V7 partial windows not selected as corpus | UNKNOWN | may contain unique interruption/failure variants; classify after incident-log cross-reference |
| V7/V8 control manifests, receipts, market rows, checkpoints, and result JSONL | MUST_RETAIN for this batch | receipt/provenance and derived-result audit trail is small relative to raw cache payload |

## 4. Approval-ready first retirement batch

### Exact scope

Delete **only** the duplicate compressed cache payload files:

```text
/root/kefi-capture-first-control-v7/partition-cache/*/evidence.frames.zst
/root/kefi-capture-first-control-v8/partition-cache/*/evidence.frames.zst
```

Do not delete their `manifest.json`, `cache-receipt.json`, checkpoints,
market rows, `market-deduplicated.jsonl`, or any promotion file.

### Preconditions at execution time

1. Revalidate all 717 cache receipts against promotion manifests and confirm
   the same 717 unique compressed hashes.
2. Re-run bounded open-handle and systemd/path-reference checks. Both control
   services/timers are currently inactive, but this must be rechecked.
3. Issue an immutable **derived-cache eviction receipt** listing each cache
   path, compressed SHA-256, retained source-promotion path, manifest hash,
   bytes, time, and tool version.
4. Require explicit user approval of this exact batch.

### Expected recovery and rollback

* **Root recovery:** 79.74 GB decimal / **74.26 GiB**.
* **Scientific capability retained:** raw promotion replay, source manifests,
  cache identity receipts, frozen/result metadata.
* **Capability deliberately lost:** immediate control-cache reads; a future
  replay must re-materialize from the retained promotion source.
* **Rollback:** the exact payload files can be regenerated from retained source
  promotion data, but only by rerunning the materializer; no file-system
  rollback exists after deletion.

### Proposed commands, not executed

```sh
find /root/kefi-capture-first-control-v7/partition-cache \
  -type f -name evidence.frames.zst -delete
find /root/kefi-capture-first-control-v8/partition-cache \
  -type f -name evidence.frames.zst -delete
```

The commands are intentionally limited to the proved duplicate payload names.
The preflight receipt generator must run and approval must be given before they
can ever be executed.

## 5. Long-term policy

1. Treat materializer outputs as derived caches, not a second evidence source.
2. Keep one manifest-bound raw source, receipts/manifests, compact results, and
   a deliberately small regression corpus.
3. Enforce a derived-cache quota and require a cache-eviction receipt once a
   closed run has an identified retained source.
4. Retire legacy promotion raw only after selecting the fixed regression corpus
   and explicitly accepting loss of broad ad-hoc historical replay.
5. A frozen study may pin its manifest/results, not an accidental second copy
   of every source payload.

No frozen study contract needs amendment for this first batch: it retains the
source raw evidence and all existing manifests/receipts. The only operational
addition required is the approval-gated derived-cache eviction receipt, so a
future audit can prove why the cache copy was removed.
