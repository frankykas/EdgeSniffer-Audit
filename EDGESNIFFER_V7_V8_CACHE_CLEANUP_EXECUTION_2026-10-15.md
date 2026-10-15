# EdgeSniffer V7/V8 Verified Cache Cleanup Execution

**Authorization basis:** `a1b8e622f5ab371d57a8e01973ebf3b57001cff6`  
**Status:** `SUCCESS`

## Result

The approved duplicate-cache cleanup completed without expanding scope.

| Check | Result |
|---|---:|
| V7 payloads verified and deleted | 309 |
| V8 payloads verified and deleted | 408 |
| Total payloads | **717** |
| Expected duplicate payload bytes | 79,736,535,863 bytes / 74.260 GiB |
| Actual root free-space recovery | 79,469,236,224 bytes / **74.011 GiB** |
| Difference | 0.249 GiB; filesystem block/metadata and concurrent system activity scale |
| Remaining V7 cache payloads | 0 |
| Remaining V8 cache payloads | 0 |
| Retained promotion sources re-hashed post-delete | 717 exact source pairs |

## Preflight and deletion proof

Before the first unlink, the server verified every cache payload against one
unique retained promotion source:

* 309 V7 and 408 V8 cache receipts;
* cache receipt `compressed_sha256` equals the only matching promotion manifest
  `compressedSha256`;
* cache/source file sizes equal manifest `compressedBytes`;
* actual SHA-256 of both cache and retained source equals that identity;
* no symlinks, unexpected path depth, hard links, source-mount mismatch,
  active related service, or process file-descriptor/path reference;
* immutable pre-deletion manifest was written before deletion.

Only individually pre-verified `evidence.frames.zst` paths were unlinked. No
promotion source, manifest, receipt, checkpoint, market row, result, V5, 2CQ,
or NFS path was touched.

## Receipts

Server-durable originals:

* `/root/edgesniffer-retirement-receipts/v7-v8-control-cache-eviction-manifest-20261015T202106Z.json`
  * content SHA-256: `84125ffa4313dcc9b2ca82d48bca5b0ea94e6ae09d4f3d733c39d667701a6f73`
  * embedded semantic manifest hash: `8c951e84154db5058899c8814c6790f1c1736c755c829129583d44ba4bf46e4e`
* `/root/edgesniffer-retirement-receipts/v7-v8-control-cache-eviction-execution-20261015T202106Z.json`
  * content SHA-256: `15ef5899a5b505c229ec3f7e5b5affe6236ce539184bea385dba1b4b2a9ff22b`
  * embedded execution hash: `7cfbef3b0e1acd97bd602732ca7ea4ea0a65e2f39b49e24ff7d2ffb793a9ee82`

Exact copies are committed under `retirement-receipts/` for independent review.

## Post-deletion health

| Mount | Free after cleanup |
|---|---:|
| Root | 143 GiB (86% used) |
| kEFi hot | 139 GiB (71% used) |
| NFS | 93 GiB (91% used) |

The V8 source remains cleanly inactive; the V8 freshness watchdog remains
active/running. No services were restarted or changed.

## Remaining bottlenecks

Root is no longer in immediate critical condition, but hot remains below its
160 GiB protected reserve and NFS remains 91% full. The next highest-value safe
opportunity is not another broad deletion: it is the approval-gated removal of
the two isolated hot build trees (19.7 GiB), followed by the compact-regression
corpus decision for unique V7/V8 promotion evidence.
