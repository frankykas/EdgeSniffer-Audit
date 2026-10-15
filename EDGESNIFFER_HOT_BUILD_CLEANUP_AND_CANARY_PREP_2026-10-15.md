# Safe Hot-Volume Cleanup and Canary Recovery Preparation - 2026-10-15

## Execution scope

Only the four explicitly authorized build directories were deleted. The first
preflight failed closed because a descendant Cargo registry symlink was found
inside `kefi-build-v8`; no deletion happened in that attempt. The second
preflight recorded the link and proved that recursive deletion would unlink,
not traverse, it. It then rechecked all candidate roots immediately before
deletion.

No V5 data, lab fixture, manifest, capture, receipt, service, pause marker,
or installed systemd unit was modified. V8 remains inactive and paused.

## Verified cleanup result

| Path | Expected allocated bytes | Result |
|---|---:|---|
| `/mnt/edgesniffer-kefi-hot/.canonical-finalized-rpc-build-20261015T0900Z` | 13,808,775,168 | Deleted |
| `/mnt/edgesniffer-kefi-hot/.canonical-inventory-conformance-build-20261015T0738Z` | 7,297,204,224 | Deleted |
| `/mnt/edgesniffer-kefi-hot/kefi-capture-first-build` | 1,083,961,344 | Deleted |
| `/mnt/edgesniffer-kefi-hot/kefi-build-v8` | 1,048,133,632 | Deleted; its `cargo-home/registry` symlink was unlinked, never followed |

- Expected allocated recovery: `23,238,074,368` bytes
- Actual hot-volume recovery: `23,238,074,368` bytes = **21.642 GiB**
- All four exact roots are absent after deletion.

Immutable VPS receipts:

- preflight: `/root/edgesniffer-retirement-receipts/hot-build-cache-eviction-preflight-20261015T225147Z.json`
  - file SHA-256: `96340a7e063273ca0cea8caf1e92cee4ad85c4252af6b814e823c1487d43caa5`
- execution: `/root/edgesniffer-retirement-receipts/hot-build-cache-eviction-execution-20261015T225147Z.json`
  - file SHA-256: `005b2b6b75c87d5d563f548f5e9155e1aac133ba9817ce4fe9f0474bd3457fd9`

Exact copies of both receipts are committed under `retirement-receipts/`.

## Current free capacity

| Filesystem | Free bytes | Free GiB |
|---|---:|---:|
| Root `/` | 149,914,447,872 | 139.619 |
| Hot `/mnt/edgesniffer-kefi-hot` | 171,564,290,048 | 159.782 |
| NFS expansion | 99,146,072,064 | 92.337 |

The hot volume is now only `0.218 GiB` below the 160 GiB final reserve, but
still **21.218 GiB below** the reviewed 181 GiB canary startup floor.

## Remaining capacity decision

The next strictly disposable item is not yet eligible: the temporary raw
fixture:

`/mnt/edgesniffer-kefi-hot/canonical-state-v1-canary-20261014T233200Z/validation-fixture-ttl`

is `0.662 GiB`, but its own immutable TTL expires at `2026-10-15T23:31:50Z`.
At the post-cleanup check (`2026-10-15T22:54:47Z`) it had not yet expired, so
it was retained. Its deletion after expiry would yield only `160.444 GiB` free
and still leave a `20.556 GiB` startup-floor shortfall.

No additional candidate is both (a) presently proven disposable and (b) large
enough to reach 181 GiB. The remaining 6.34 GiB FRA/RabbitStream labs are
historical overload/decoder/transport fixtures. The 17.311 GiB V5 control
cache contains the only locally retained V5 raw replay payloads. Neither is
approved for deletion in this task. The correct next step is a separate
explicit retention decision, rather than silently consuming scientific
regression/replay evidence.

## Reviewable canary-gate patch

The sanitized Canonical State V1 patch changes only the proposed code and
service template. It is **not installed** on the VPS.

- Adds required `CANONICAL_CANARY_START_FLOOR_BYTES`.
- Changes final runtime reserve from the unreviewed 240 GiB deployment value
  to 160 GiB.
- Sets a 181 GiB startup floor (`160 + 6 GiB output cap + 15 GiB buffer`).
- Fails closed when the storage query fails, the policy is internally invalid,
  startup free space is below the floor, or concurrent consumption means the
  remaining output cap can no longer fit above the 160 GiB reserve.
- Writes startup floor, startup free space, current free space, reserve, and
  output cap to the canary health/receipt state.

Patch files:

- `sanitized-code/canonical-state-v1/kefi_canonical_canary_v1.rs`
- `sanitized-code/canonical-state-v1/edgesniffer-kefi-canonical-canary-v1.service`

Verification: an isolated Rust test compiled and ran the exact storage-gate
logic with two tests: startup boundaries and concurrent-consumption boundaries.
Result: **2 passed, 0 failed**. This verifies the new pure storage logic; it
does not constitute a deployed canary or a full production-binary build.

## V8

V8 remains `inactive/dead`, successful exit at `2026-10-15 15:47:16 UTC`.
The later capacity pause record was written at `2026-10-15T20:23:32.696595Z`
because root had crossed its 70 GiB emergency watermark. The timeline does not
prove that the later guard event initiated the earlier exit. No pause marker
was removed and no service was started.

## Explicit blockers

1. **Capacity:** 21.218 GiB still required before the corrected 181 GiB
   canary start floor is met.
2. **Deployment review:** the corrected source/template patch is not yet
   independently reviewed, built, or installed.
3. **V8:** manual-resume authorization and its separate fresh preflight are
   required; neither is implied by build-cache cleanup.

Therefore: no live canary and no V8 restart is authorized by this report.
