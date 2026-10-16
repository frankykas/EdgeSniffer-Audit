# Canonical Final Predeployment Corrections

Base: `21f49c7849c2be020ab72ed0df70364d65cfb8f1`.

Result: `READY_FOR_DEPLOYMENT_REVIEW - NOT INSTALLED - NO LIVE RPC RUN`.

## Corrected behavior

1. **No-range diagnostic restored.** Both modes now enter the receive and
   reconciliation loop. With `FINALIZED_RANGE_SLOT_COUNT` absent, the canary
   stops only after its first exact reconciled slot or its existing runtime and
   integrity limits. The range mode alone waits for its inventory to seal.

2. **One shared RPC count.** `report.json.rpcCalls` is assigned from the
   shared `RpcBudget.used_requests()` after the run, so it includes every
   successful allocation: dispatcher attempts, retry attempts, `getBlocks`,
   and every inventory `getBlock` attempt, including attempts that later fail.
   The report also has these audited breakdown fields:

   - `dispatcherRpcCalls`
   - `dispatcherRetryRpcCalls` (a subset of dispatcher calls)
   - `inventoryGetBlocksRpcCalls`
   - `inventoryGetBlockRpcCalls`
   - `retries` (scheduled retry count, not an additional RPC total)

   A five-slot run can consume at most 16 requests: 10 dispatcher allocations,
   one `getBlocks`, and five inventory `getBlock` allocations. A shorter
   dispatcher path leaves the unused quota unused; it is not fabricated in the
   report.

3. **Read-only persisted-receipt verifier.** The added
   `verify_forward_cohort_receipt_chain_v1` binary re-reads the sealed
   `finalized-slot-inventory.json` and calls
   `seal_forward_cohort_from_persisted_receipt_chain`. It neither opens a
   cohort nor writes a ledger. Zero qualifying migrations is a successful
   result when the receipt chain itself validates.

   After a future approved capture, run exactly:

   ```bash
   set -euo pipefail
   root=/mnt/edgesniffer-canonical-hot/canonical-state-v1-diagnostic-rpc-reserved-20261016T134500Z
   test -f "$root/finalized-slot-inventory.json"
   CANONICAL_RECEIPT_ROOT="$root" \
     /root/bot/EdgeSniffer/rust-exact-ledger/target/release/verify_forward_cohort_receipt_chain_v1
   ```

   Its `RECEIPT_CHAIN_VALID_ZERO_QUALIFYING_CREATIONS` status is valid for a
   complete zero-creation interval. It is receipt verification only, not a
   research-cohort authorization.

## Tests

Executed with the already-installed compatible Rust toolchain:

```text
cargo test --release --bin kefi_finalized_rpc_reconciliation_canary_v1
13 passed, 0 failed

cargo test --release --bin verify_forward_cohort_receipt_chain_v1
0 passed, 0 failed (compile-only binary target)

cargo build --release --bin kefi_finalized_rpc_reconciliation_canary_v1
cargo build --release --bin verify_forward_cohort_receipt_chain_v1
```

The prior submitted source had 12 canary tests: nine integration tests plus
three dispatcher tests. The earlier report's claim of 11 was incorrect. This
revision adds the no-range-loop regression, so the executed canary count is 13.

The focused tests cover the no-range regression; full five-slot allocation of
all 16 shared requests; a shorter three-dispatcher-request path using nine
total allocations after its six-request inventory; retry reservation; automatic
range readiness; storage and fresh-root failures; failed transaction/version
filtering; and persisted receipt-chain success/tamper/incomplete/zero-creation
cases.

## Proposed artifacts

Nothing has been installed.

```text
canary source:
  /root/bot/EdgeSniffer/rust-sidecar/target/release/kefi_finalized_rpc_reconciliation_canary_v1
canary SHA-256:
  4b9d34aba7b76dfaeb82760490e2d18ff984d533e58e45514140dde438a75c2b

read-only verifier source:
  /root/bot/EdgeSniffer/rust-exact-ledger/target/release/verify_forward_cohort_receipt_chain_v1
verifier SHA-256:
  889c83b5c1c174613f9be9cb4dd1e4d0e34dfa7ccc86658e5f35ecf9a283b906

installed canary SHA-256 (unchanged old artifact):
  9f3bc274cc1cf468960c6d9c79f1a79809e04e7f0fcc591940917ccb2067acb6
```

## Approval-gated deployment

The following has not been executed. It changes only the dedicated diagnostic
binary, keeps a rollback copy, and does not restart a service.

```bash
set -euo pipefail
src=/root/bot/EdgeSniffer/rust-sidecar/target/release/kefi_finalized_rpc_reconciliation_canary_v1
dst=/usr/local/lib/edgesniffer/edgesniffer-kefi-finalized-rpc-canary-v1
new_sha=4b9d34aba7b76dfaeb82760490e2d18ff984d533e58e45514140dde438a75c2b
old_sha=9f3bc274cc1cf468960c6d9c79f1a79809e04e7f0fcc591940917ccb2067acb6
backup="${dst}.pre-final-predeploy-$(date -u +%Y%m%dT%H%M%SZ)"

test "$(sha256sum "$src" | awk '{print $1}')" = "$new_sha"
test "$(sha256sum "$dst" | awk '{print $1}')" = "$old_sha"
test ! -e "$backup"
cp --preserve=mode,ownership,timestamps "$dst" "$backup"
install -m 0755 "$src" "${dst}.new"
test "$(sha256sum "${dst}.new" | awk '{print $1}')" = "$new_sha"
mv -f "${dst}.new" "$dst"
test "$(sha256sum "$dst" | awk '{print $1}')" = "$new_sha"
printf 'rollback_backup=%s\n' "$backup"
```

Rollback, separately approval-gated:

```bash
set -euo pipefail
backup=/usr/local/lib/edgesniffer/edgesniffer-kefi-finalized-rpc-canary-v1.pre-final-predeploy-REPLACE_WITH_RECORDED_TIMESTAMP
dst=/usr/local/lib/edgesniffer/edgesniffer-kefi-finalized-rpc-canary-v1
old_sha=9f3bc274cc1cf468960c6d9c79f1a79809e04e7f0fcc591940917ccb2067acb6
test "$(sha256sum "$backup" | awk '{print $1}')" = "$old_sha"
install -m 0755 "$backup" "${dst}.rollback"
mv -f "${dst}.rollback" "$dst"
test "$(sha256sum "$dst" | awk '{print $1}')" = "$old_sha"
```

The proposed diagnostic remains isolated at a fresh root on `/dev/vdd1`, with
the unchanged 181 GiB startup floor, 160 GiB runtime reserve, and 6 GiB output
cap. V8, the dust-calibration service, historical evidence, and all live
services remain untouched.

## Requirement checklist

| Requirement | Status | Evidence |
| --- | --- | --- |
| Restore no-range loop | VERIFIED | `no_range_mode_enters_the_original_single_slot_collection_loop` |
| Count every RPC allocation | VERIFIED | Shared budget-derived report total and 16/9 allocation regressions |
| Read-only persisted receipt verification | VERIFIED | New verifier binary plus existing persisted-chain/tamper/zero-creation integration tests |
| Correct test count | VERIFIED | Actual canary test command: 13 passed |
| Release artifact rebuilt | VERIFIED | SHA-256 above |
| Install/deploy/live RPC/service change | NOT STARTED | Explicit authorization boundary preserved |
