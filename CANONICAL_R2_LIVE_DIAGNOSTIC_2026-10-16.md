# Canonical State V1 R2 Live Diagnostic

Source implementation commit: `ba41646a6c7c9db1569533301437e79a62871438`.

## Verdict

`FAIL` - the single authorized R2 diagnostic failed closed on an immutable
receipt-integrity conflict. It did not publish a finalized-slot inventory or a
scientific interval. This is an implementation defect in receipt emission, not
a claim that the five-slot source interval is incomplete.

No second diagnostic was launched.

## Authorized deployment and safety record

| Check | Result |
| --- | --- |
| R2 source binary SHA-256 | `b6a0e501b67b11ebaba00d67b395673ff8f98218efc2ed127cc5f92fc2a27f54` verified |
| Prior R1 binary preserved | `/usr/local/lib/edgesniffer/edgesniffer-kefi-finalized-rpc-canary-v1.pre-r2-capture-completeness-20261016T162028Z` (`4b9d34...75c2b`) |
| Output root | `/mnt/edgesniffer-canonical-hot/canonical-state-v1-live-diagnostic-r2-20261016T162053Z` (fresh) |
| Canonical volume | `/dev/vdd1` at `/mnt/edgesniffer-canonical-hot` |
| Startup free space | `1,496,648,646,656` bytes |
| Storage policy | 181 GiB startup floor, 160 GiB runtime reserve, 6 GiB cap - passed |
| V8 | inactive throughout |
| Dust calibration service | untouched; remained outside this diagnostic |
| Trading/submission | not invoked |
| Diagnostic count | exactly one R2 run |

## Execution

| Item | Value |
| --- | --- |
| Transient unit | `edgesniffer-canonical-live-diagnostic-r2-20261016T162053Z.service` |
| Invocation ID | `2e3898fe749849f1b9679acab2bb2b83` |
| Start | `2026-10-16T16:20:53.189972827Z` |
| Stop | `2026-10-16T16:21:01.566292415Z` |
| Runtime | 8.392 seconds |
| Systemd exit | `1` (`SLOT_IDENTITY_RECEIPT_CONFLICT`) |
| Produced finalized slots queried | `457702185` through `457702189` |
| Source identities observed by slot | `90`, `70`, `110`, `71`, `72` respectively |
| JSON-RPC total | `9 / 16` |
| Dispatcher / retry dispatcher RPC | `3 / 1` |
| Inventory RPC (`getBlocks` / `getBlock`) | `1 / 5` |
| Retry schedules | `2` |
| Pending-slot peak | `4` |
| Peak process RSS | `195,911,680` bytes |
| Peak gRPC message | `21,058` bytes |
| Reported disk growth | `540,672` bytes |
| Root disk footprint after stop | `506,266` bytes |
| Decoder transactions / events / rejects | `413 / 285 / 0` |
| Source gaps / epoch failures | `0` |

## What R2 proved and what failed

The R2 source-completeness correction behaved as intended up to receipt
publication: the process recorded an individual finalized source observation
and compact filtered identities for each of the five produced slots before it
attempted the range workflow. This is materially different from R1, which
sealed its range when only a high-water finalized notification was present.

The first finalized slot, `457702185`, passed exact source-versus-finalized
reconciliation: both the compact manifest and the finalized relevant set had
exactly `90` identities, and the immutable reconciliation receipt is
`34290f857c2c3a6ac6e49b2ecdda5876f19fb9a2d240d2f84e2110ddbb526f1a`.

Receipt publication then failed. The per-slot identity-receipt builder emitted
`281` receipts, one for every successful transaction in the full finalized
block, while the reconciled filtered relevant set contains `90` transactions.
The bundle validator correctly rejected that contradiction as
`SLOT_IDENTITY_RECEIPT_CONFLICT`. Because receipt emission stops at the first
slot, R2 does **not** establish exact reconciliation for slots `457702186`
through `457702189`; it establishes only that their source-side filtered
identities were durably observed.

The smallest corrective change is to make `identity_receipts` emit receipts
only for finalized transactions matching the same Pump/PumpSwap relevance
predicate used by `reconcile_finalized_block`, then add an integration
regression using a block with relevant and unrelated successful transactions.
That repair requires separate authorization; it was not made during this
one-run task.

## Evidence integrity

The R2 root is preserved unchanged. Key file hashes:

| Artifact | SHA-256 |
| --- | --- |
| `report.json` | `9a5aa23b57595990bc15ee5a5a57a96b25587f8c963676618684fcc52493ce97` |
| identity manifest | `32f6be5dab4a8fe2df472814cdede17f6f2f6ff8db96105d0a0834b397ba37b6` |
| finalized `getBlocks` response | `a5544f574b09bc8ab7766425c98645a65778b4d3fe282db568931fdd2ae61ddb` |
| reconciliation receipt for slot `457702185` | `cf0d50d746599252519a94a510b487432db623c969aaebc0ef88520ae0a7d553` |
| per-slot identity receipts | `1bcea9ff73c10f7d7ccd80914f5d752df11a4e122edc8edae703ddfd397dfaa4` |
| path receipts | `0025527ee9c6b9802a31173594bde27b71c480d79db4325dc2761cbc4424e8f5` |

No `finalized-slot-inventory.json` exists. Therefore the persisted
receipt-chain cohort verifier was correctly **not run**. There were no creation
witnesses in the partial evidence (`creation-receipt-slot-457702185.json` is
the empty-array hash).

## Requirement checklist

| Requirement | Status | Evidence |
| --- | --- | --- |
| Preserve R1 executable for rollback | VERIFIED | Recorded backup path and SHA above |
| Install approved R2 executable | VERIFIED | Installed SHA matches approved SHA |
| Exactly one fresh five-slot capture-only run | VERIFIED | One transient unit/root; no rerun |
| At most 16 JSON-RPC requests | VERIFIED | `9 / 16` in durable report |
| At most 45 minutes | VERIFIED | 8.392 seconds |
| Preserve evidence; V8 inactive; dust untouched | VERIFIED | Fresh root; V8 inactive; no dust action |
| Run verifier only on complete inventory | VERIFIED | No inventory; verifier not run |
| Publish scientific interval | NOT PRODUCED | Fail-closed integrity rejection |

## Next blocker

Repair and test the identity-receipt relevance filter, then obtain fresh
authorization for a new bounded diagnostic. The current R2 evidence must not
be treated as a complete interval or reused to bridge a future run.
