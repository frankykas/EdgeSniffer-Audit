# Finalized-RPC Dispatcher Integration and Storage Gate

## Result

`DISPATCHER_INTEGRATED_OFFLINE`

The canary adapter now composes the existing bounded dispatcher with source
epoch continuity enforcement. It journals before scheduling, schedules from
explicit finalized progress, retries without a new source message, rejects
epoch/gap bridging, and retains terminal receipt state until drain.

Offline harness: **29 passed, 0 failed**.

`STORAGE_BLOCKED`

The live canary must not run: `/mnt/edgesniffer-kefi-hot` has 146 GiB free,
below the frozen 160 GiB reserve, before its 64 MiB output quota.

## Read-Only Inventory

| Location | Size | Classification | Action |
|---|---:|---|---|
| `/mnt/edgesniffer-kefi-hot/kefi-capture-first-promotions-v8` | 265.3 GB | active evidence | pinned; no action |
| `/mnt/edgesniffer-kefi-hot/kefi-capture-first-control-v5` | 22.3 GB | research/control evidence | no action without dependency audit |
| `.canonical-finalized-rpc-build-20261015T0900Z` | 13.8 GB | isolated build artifact | candidate cleanup; requires approval |
| `.canonical-inventory-conformance-build-20261015T0738Z` | 7.3 GB | isolated build artifact | candidate cleanup; requires approval |
| `canonical-state-v1-canary-*` TTL fixtures | about 1.3 GB | migration evidence | inspect TTL/receipt before any action |

The two isolated build roots total about **21.1 GB**, enough to restore the
160 GiB reserve plus an operational buffer if they are confirmed unused.
They are not scientific evidence, but this task did not delete them.

NFS has only about 92.3 GB free and is not a safe ingestion dependency. No
archive/release action was performed or implied.

## Additional Live Gate

The read-only service check found
`edgesniffer-kefi-full-meta-capture-first-v8.service` inactive. It stopped
cleanly at `2026-10-15T15:47:16Z` with systemd status `15`/`Result=success`;
the log records a stop request, not a crash. The V8 watchdog and evidence
ledger remain active. This task does not authorize restarting V8, so this is a
second live-canary blocker independent of storage.

## Exact Approval-Gated Cleanup Candidates

```sh
rm -rf /mnt/edgesniffer-kefi-hot/.canonical-finalized-rpc-build-20261015T0900Z
rm -rf /mnt/edgesniffer-kefi-hot/.canonical-inventory-conformance-build-20261015T0738Z
```

Risk: removes reproducible local compilation artifacts only; it does not
remove the published sanitized source, reports, active services, or sealed
research evidence. Approval is required before executing either command.

## Before The Live Canary

1. Approve or replace the two build-cache cleanup candidates.
2. Confirm hot-volume free space is at least 160 GiB plus the 64 MiB canary
   quota and an additional operational buffer.
3. Compile the adapter-bound canary and run its 16-RPC, ten-minute isolated
   test. A matched slot remains only a reconciled slot, never a complete
   market interval or candidate universe proof.
