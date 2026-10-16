# Canonical State V1 Finalized-RPC Canary - 2026-10-16

## Verdict

`FALLBACK_CANARY_VERIFIED` for one real finalized slot. This is evidence that
the integrated filtered-Yellowstone to finalized-RPC path works end to end. It
is **not** a complete scientific market interval, complete candidate universe,
or authorization for AOM/V8 admission.

## Deployed integration

The deployed executable is a separate bounded systemd unit; it does not replace
the old capture-only canary or modify V8.

| Item | Value |
| --- | --- |
| Binary | `/usr/local/lib/edgesniffer/edgesniffer-kefi-finalized-rpc-canary-v1` |
| Binary SHA-256 after report-counter correction | `9f3bc274cc1cf468960c6d9c79f1a79809e04e7f0fcc591940917ccb2067acb6` |
| Runtime root | `/mnt/edgesniffer-canonical-hot/canonical-state-v1-canary-next` |
| Required mount | `/dev/vdd1` at `/mnt/edgesniffer-canonical-hot` |
| Start floor / runtime reserve / cap | `181 GiB` / `160 GiB` / `6 GiB` |
| RPC dispatcher | bounded: 16 calls, one concurrent call, 32 active slots, 3 attempts, 1 s exponential backoff |
| Restart rule | durable manifests/checkpoints/dispatcher state are preserved; a prior source instance cannot be bridged by a restarted process |

The service starts only after mount identity and startup-free-space checks pass.
It consumes the existing secure RPC environment file without recording its URL
or credentials in this repository.

## Live canary result

The one allowed run started at `2026-10-16T02:12:47.489065688Z` and completed
normally at `2026-10-16T02:12:50.753135202Z`.

| Measure | Result |
| --- | ---: |
| Yellowstone filtered transactions journaled | 130 |
| Distinct observed slots (from immutable manifest) | 2 |
| Finalized `getBlock` calls | 3 / 16 budget |
| Exact reconciled slots | 1 |
| Rejected slots | 1 |
| Retry attempts | 2 |
| Peak pending slots | 2 |
| Decoder transactions / events / rejects | 130 / 78 / 0 |
| Source gaps or epoch changes | 0 |
| Peak RSS | 54,931,456 bytes |
| Largest gRPC message | 15,665 bytes |
| Disk growth | 114,688 bytes |
| Semantic events released | 0 |
| Scientific interval released | 0 |

### Immutable receipts

* Reconciled slot `457466348`: `129` observed identities, `129` finalized
  relevant identities, receipt SHA-256
  `838b92f955300d21df75984de87ee0f5258822fb9860c372dc3e4034e13d630d`.
* Rejected slot `457466349`: `1` observed identity versus `71` finalized
  relevant identities. It is retained only as a fail-closed missing-identity
  receipt, never released.
* Manifest SHA-256:
  `0bf075f07d332f73cae9a117ccf248643e4b9887e568d63be6a5b10e281a3c47`.

The original live report counted rejection *attempts* and omitted its observed
slot count. That reporting defect was corrected in the deployed follow-up
binary; the completed run, its report, and receipts were deliberately left
unchanged.

## Verification

| Check | Result |
| --- | --- |
| Integrated canary unit tests | 5 passed |
| Focused identity-manifest/reconciliation suite | 11 passed |
| Actual production dependency-graph build | passed |
| Installed-unit verification | passed |
| V8 service | remains inactive |
| V8 capacity pause marker | remains present |

The broad legacy `rust-exact-ledger` all-target test command remains blocked by
unrelated stale Yellowstone V3 binaries with pre-existing compilation errors.
The focused library test suite that the new canary links against passed; those
unrelated binaries were not deployed or changed.

## Scientific boundary

The canary proves a real exact filtered-identity/index reconciliation can occur.
It does **not** establish complete slot coverage over an interval, a complete
eligible-pool universe, or exact semantic instruction paths. The current
PumpSwap decoder therefore records decoded events but releases no scientific
semantic state from this run. The next gate is to collect and reconcile a
bounded continuous interval, with authoritative coverage proofs and the
instruction-path decoder, before market-candidate admission.
