# Canonical Live Diagnostic Preflight Remediation

Base: `fc0480aa2973ef8ccbd1276eba01151443b991f6`.

## Result

`READY_FOR_REVIEW — NOT DEPLOYED — NO LIVE RPC OR DIAGNOSTIC RUN`

## Corrected RPC Reservation

The canary now parses `FINALIZED_RANGE_SLOT_COUNT` before it opens the
Yellowstone subscription or dispatches RPC. For a range of `N` slots it reserves
exactly `1 + N` requests:

- one `getBlocks(start, end, finalized)` request;
- up to `N` finalized `getBlock` requests, including zero-relevant blocks.

The pre-range dispatcher is capped at `16 - 1 - N`. Each dispatcher attempt,
including failed requests and retries, calls the same budget consumer and
therefore counts against that cap. Once its allocation is exhausted it fails
closed with `DISPATCHER_RPC_RESERVATION_EXHAUSTED`; it cannot consume the
inventory reservation.

For the intended five-slot diagnostic:

```text
total budget:          16
inventory reservation:  6  (1 getBlocks + 5 getBlock)
dispatcher maximum:    10
```

The existing inventory preflight remains in place and checks actual remaining
budget again immediately before `getBlocks`. Static start/end environment
variables are rejected: they could name a range that has not reached finalized
status yet.

For a bounded run the canary anchors the interval at its first exact reconciled
source slot, continues receiving source data, and runs the inventory only once
the fifth inclusive slot has reached finalized status. This ensures the range is
both source-observed and finalized without a pre-run `getSlot` request.

## Offline Verification

Passed with the installed compatible Rust toolchain:

```text
cargo test --bin kefi_finalized_rpc_reconciliation_canary_v1  # 11 passed
cargo test --lib finalized_slot_inventory_v1                 # 3 passed
cargo test --lib forward_new_pool_cohort_v1                  # compiled; no focused tests
cargo build --release --bin kefi_finalized_rpc_reconciliation_canary_v1
```

The canary tests cover:

- five-slot reservation and dispatcher saturation;
- zero earlier dispatcher calls;
- dispatcher retries consuming reserved dispatcher allocation;
- automatic range selection waiting for its finalized inclusive end;
- five produced zero-relevant slots;
- explicit skipped slots;
- missing and byte-tampered receipts;
- incomplete inventory;
- storage-floor rejection; and
- fresh-root rejection when `run-binding.json` exists.

## Proposed Artifact

The reservation change creates a new proposed binary. It does **not** equal
the prior approved artifact and has not been installed.

```text
source:      /root/bot/EdgeSniffer/rust-sidecar/target/release/kefi_finalized_rpc_reconciliation_canary_v1
destination: /usr/local/lib/edgesniffer/edgesniffer-kefi-finalized-rpc-canary-v1
new SHA-256: a072f8e83363b85c75ed967a3c6152ae66bbe5d6938a372775ac34de4b9b9641
installed SHA-256 (old): 9f3bc274cc1cf468960c6d9c79f1a79809e04e7f0fcc591940917ccb2067acb6
```

### Approval-gated atomic installation

```bash
set -euo pipefail
src=/root/bot/EdgeSniffer/rust-sidecar/target/release/kefi_finalized_rpc_reconciliation_canary_v1
dst=/usr/local/lib/edgesniffer/edgesniffer-kefi-finalized-rpc-canary-v1
new_sha=a072f8e83363b85c75ed967a3c6152ae66bbe5d6938a372775ac34de4b9b9641
old_sha=9f3bc274cc1cf468960c6d9c79f1a79809e04e7f0fcc591940917ccb2067acb6
backup="${dst}.pre-rpc-reservation-20261016T142447Z"

test "$(sha256sum "$src" | awk '{print $1}')" = "$new_sha"
test "$(sha256sum "$dst" | awk '{print $1}')" = "$old_sha"
test ! -e "$backup"
cp --preserve=mode,ownership,timestamps "$dst" "$backup"
install -m 0755 "$src" "${dst}.new"
test "$(sha256sum "${dst}.new" | awk '{print $1}')" = "$new_sha"
mv -f "${dst}.new" "$dst"
test "$(sha256sum "$dst" | awk '{print $1}')" = "$new_sha"
```

`mv` is a same-filesystem atomic replacement. No service restart is required;
the canary unit is currently inactive.

Rollback, also approval-gated and without starting any service:

```bash
set -euo pipefail
test "$(sha256sum "$backup" | awk '{print $1}')" = "$old_sha"
install -m 0755 "$backup" "${dst}.rollback"
mv -f "${dst}.rollback" "$dst"
test "$(sha256sum "$dst" | awk '{print $1}')" = "$old_sha"
```

## Fresh Diagnostic Root and Guard State

Proposed fresh root, verified absent during preflight:

```text
/mnt/edgesniffer-canonical-hot/canonical-state-v1-diagnostic-rpc-reserved-20261016T134500Z
```

It is on `/dev/vdd1`. The mount has approximately 1.36 TiB free. The binary
continues to enforce the unchanged 181 GiB startup floor, 160 GiB runtime
reserve, six-GiB total output cap, and refusal to reuse a root containing
`run-binding.json`.

The existing systemd unit is intentionally left unchanged: it points to the
historical `canonical-state-v1-canary-next` root, which already contains a
run-binding and is therefore unsuitable for this diagnostic.

## Proposed Run Shape, After Separate Approval

No command below has been run. A transient bounded invocation avoids changing
the installed persistent systemd unit. The process selects its own five-slot
range after it obtains its first exact finalized reconciliation, and waits until
the inclusive end slot is finalized before the inventory request.

```bash
root=/mnt/edgesniffer-canonical-hot/canonical-state-v1-diagnostic-rpc-reserved-20261016T134500Z

test ! -e "$root"
systemd-run --unit=edgesniffer-canonical-diagnostic-rpc-reserved-r1 \
  --collect --wait --service-type=exec \
  --property=RequiresMountsFor=/mnt/edgesniffer-canonical-hot \
  --property=MemoryMax=8G --property=CPUQuota=200% \
  --property=IOSchedulingClass=idle --property=NoNewPrivileges=yes \
  --property=EnvironmentFile=/etc/edgesniffer/kefi-full-meta.env \
  --property=EnvironmentFile=/etc/edgesniffer/kefi-choice-state-observer-v1-live.env \
  --property=EnvironmentFile=/root/bot/EdgeSniffer/.env \
  --setenv=YELLOWSTONE_FULL_META_ENDPOINT=https://grpc.fra.shyft.to \
  --setenv=FINALIZED_RECONCILIATION_CANARY_ROOT="$root" \
  --setenv=FINALIZED_RANGE_SLOT_COUNT=5 \
  --setenv=CANONICAL_CANARY_TOTAL_CAP_BYTES=6442450944 \
  --setenv=CANONICAL_CANARY_PROTECTED_RESERVE_BYTES=171798691840 \
  --setenv=CANONICAL_CANARY_START_FLOOR_BYTES=194347270144 \
  --setenv=CANONICAL_CANARY_MAX_SECONDS=2700 \
  /usr/local/lib/edgesniffer/edgesniffer-kefi-finalized-rpc-canary-v1
```

This command has no transaction-submission flags or execution wallet inputs.
The canary source contains no transaction submission API usage.
Its maximum JSON-RPC usage is exactly 16: at most 10 dispatcher `getBlock`
attempts, one `getBlocks`, and at most five inventory `getBlock` calls.

## Operational Safety

V8 remains inactive. Its watchdog is active but only restarts an already-active
stale collector; it does not start an inactive collector. No V8 unit was
changed.

`edgesniffer-teacher-impulse-mc5000-dust.service` is unrelated, configured for
real submission, and currently cycles in `203/EXEC` before retaining a process.
It is not a dependency of the diagnostic. No action was taken. If its owner
chooses isolation before a future run, the separately approved commands are:

```bash
systemctl stop edgesniffer-teacher-impulse-mc5000-dust.service
systemctl disable edgesniffer-teacher-impulse-mc5000-dust.service
```

Rollback, only with separate approval because it attempts to restore a
real-submission service:

```bash
systemctl enable edgesniffer-teacher-impulse-mc5000-dust.service
systemctl start edgesniffer-teacher-impulse-mc5000-dust.service
```

## Remaining Authorization

1. Approve installation of the new SHA-256 artifact.
2. Approve one transient five-slot capture-only diagnostic. The new binary
   chooses the range only after it has source-observed and reconciled its first
   anchor slot.
3. Optionally, separately approve dust-service isolation. It is not included
   in diagnostic approval.

No live RPC call, binary installation, service restart, V8 change, dust-service
change, evidence deletion, or trading action occurred in this remediation.
