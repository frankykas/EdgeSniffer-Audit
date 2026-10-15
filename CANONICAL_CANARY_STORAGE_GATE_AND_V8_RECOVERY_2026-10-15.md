# Canonical Canary Storage Gate and V8 Recovery - 2026-10-15

## Scope

Read-only reconciliation from storage-audit commit  d68a7bdfa9d6bec98111de40247a4eb637697dd2 .
No build directory was removed, no systemd unit was changed, no pause marker
was removed, and no service or canary was started.

## Origin of the 240 GiB value

240 GiB is an installed deployment value, not a binary default and not an
environment-file override:

```ini
# /etc/systemd/system/edgesniffer-kefi-canonical-canary-v1.service
Environment=CANONICAL_CANARY_PROTECTED_RESERVE_BYTES=257698037760
```

The effective systemd environment is identical and there are no unit drop-ins.
The installed binary has SHA-256
2e2838b409660c5cd3e61e1db1f1edd211f2aa2cd5bbef41814ca6c3e90603a0.
Its source requires the value from the environment and does not define a
default. It checks only:

```rust
if free_bytes(&root)? <= reserve { bail!(...) }
```

and later stops when the same free <= reserve condition is observed. The unit
also declares a 4 GiB raw-fixture cap and 6 GiB total-output cap.

The prior finalized-RPC dispatcher report documented a separate, smaller
canary with a 160 GiB reserve and a 64 MiB output quota. There is no Git
evidence in the deployed source tree that promotes 240 GiB to a scientific
or storage-policy requirement. These are two different canary deployments.
The current 240 GiB setting is therefore a **deployment mismatch / unreviewed
stricter operational setting**, not a confirmed intentional policy.

## Safe-start arithmetic

The bounded canonical producer canary can write up to 6 GiB after startup.
Its reserve-only check is insufficient: a run beginning just above the reserve
can consume capacity below it before the periodic check runs.

Recommended authoritative policy:

| Component | GiB | Basis |
|---|---:|---|
| Protected hot-volume reserve | 160 | Existing prior operational policy; above V8's separate 70 GiB emergency watermark. |
| Maximum canary output | 6 | Installed total cap; use the cap, not an optimistic historical average. |
| Operational buffer | 15 | Journal/receipt/filesystem overhead and ordinary concurrent growth. |
| **Minimum free space before canary start** | **181** | 160 + 6 + 15; require a small measurement margin in enforcement. |

No hot-volume compilation should occur for the canary. The binary must be
prebuilt elsewhere, so expected temporary build use on hot is 0 GiB. The
earlier finalized-RPC canary retained only 9.6 MiB, but this is not a
substitute for the installed producer's 6 GiB hard cap.

### Required configuration change, after approval

Do not merely change 257698037760 to 160 GiB. The deployment must bind both
concepts explicitly:

1. Set the final protected reserve to 160 GiB.
2. Add an immutable start-floor check of at least 181 GiB before files are created.
3. Keep the runtime stop rule at 160 GiB and the total-output cap at 6 GiB.
4. Emit reserve, start floor, cap, measured free bytes, and output bytes in the receipt.

This is a future reviewed deployment change. It was not made here.

With the installed 240 GiB value, a correct safe-start floor would be
261 GiB (240 + 6 + 15), not merely 240 GiB. The current unit neither
documents nor enforces that combined floor.

## Current capacity and shortfall

| Filesystem | Free GiB |
|---|---:|
| Root / | 140.09 |
| Hot /mnt/edgesniffer-kefi-hot | 138.14 |
| NFS expansion | 92.34 |

The four verified disposable hot build directories total 21.346 GiB. If
they are separately approved and removed, hot free space is expected to become
159.486 GiB.

| Target | Shortfall after approved build cleanup |
|---|---:|
| 160 GiB final reserve | 0.514 GiB |
| 181 GiB recommended safe-start floor | 21.514 GiB |
| 261 GiB floor implied by the present 240 GiB setting | 101.514 GiB |

The V5 cache must not be deleted merely to cross a threshold: it is not a
proven byte-identical duplicate of retained raw data, and it would not make
the present 240 GiB deployment policy feasible.

## V8 capacity-stop timeline and recovery conditions

### Verified timeline

- 2026-10-15 15:47:16 UTC: V8 stopped cleanly, status 15, after normal
  promotion activity. No crash/restart loop was observed.
- 2026-10-15 20:23:32.696595 UTC: the V8 capacity guard wrote
  capacity-pause.json because root free space was 68.836 GiB, below its
  70 GiB emergency watermark. It verified V8 stopped and marked
  manualResumeRequired: true.
- Current guard execution: healthy, with hot 138.14 GiB and root 140.22
  GiB. It intentionally does not clear a manual pause marker.

The pause is enforced by:

```ini
ConditionPathExists=!/mnt/edgesniffer-kefi-hot/kefi-capture-first-live-v8/operations/capacity-pause.json
```

This was a storage-protection stop caused by **root** pressure, not a V8
failure and not hot-volume pressure.

### Conditions for a separately authorized manual V8 resume

1. Recheck root and hot mount identities immediately before action.
2. Confirm both capacity guards are healthy and both volumes meet active-capture policy.
3. Retain the prior pause receipt; remove the marker only under explicit
   authorization and record a new receipt.
4. Verify frozen V8 unit/watchdog/promotion-root paths and hashes.
5. Confirm no canary/cleanup job competes for hot storage and budget capture growth.
6. Obtain a separate service-start authorization.

No resume action was taken.

## Build cleanup preflight, prepared but not executed

Before deleting the four build directories, run a fresh preflight that proves
for every root: exact path, /dev/vdc1 mount, owner, type, no symlink
traversal, current du -sb, no lsof/fuser handles, no process reference, no
systemd/configuration reference, and no change to the approved path list.
Write a durable pre-deletion receipt outside the four roots.

The exact roots are:

```text
/mnt/edgesniffer-kefi-hot/.canonical-finalized-rpc-build-20261015T0900Z
/mnt/edgesniffer-kefi-hot/.canonical-inventory-conformance-build-20261015T0738Z
/mnt/edgesniffer-kefi-hot/kefi-capture-first-build
/mnt/edgesniffer-kefi-hot/kefi-build-v8
```

After a separately authorized deletion, remeasure free space and preserve an
eviction receipt. This is prepared guidance, not deletion authorization.

## Decision

Adopt the 181 GiB safe-start policy for the bounded producer canary only
through an explicitly reviewed deployment change that preserves a 160 GiB
post-run reserve. Approving the build cleanup improves safety but leaves a
21.514 GiB shortfall to that start floor. Keep V8 paused until its own
manual-resume preflight and authorization are complete.
