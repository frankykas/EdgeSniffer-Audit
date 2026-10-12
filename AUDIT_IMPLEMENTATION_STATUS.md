# kEfi Audit Remediation Status

Last reconciled: 2026-10-12 00:02 EDT (2026-10-12T04:02Z) against the live VPS.

This is a status ledger, not a claim that the research is ready for live trading.
`VERIFIED` means checked against the live host or an executable test. `IMPLEMENTED`
means code/config exists but still needs the stated validation. `OPEN` means no
claim has been made and no H3 paper boundary may rely on it.

## Current Safety State

| Surface | Status | Evidence / consequence |
| --- | --- | --- |
| Capture source | VERIFIED HEALTHY | `edgesniffer-kefi-full-meta-capture-first-v8.service` active; Frankfurt source health at 04:02Z reported fresh frames, zero reconnects, errors, continuity gaps, queue-full events, and promotion failures since the 02:17Z operational restart. |
| Source watchdog | VERIFIED HEALTHY | Watchdog active. This establishes current process health only; it does not make the pre-restart epoch scientific evidence. |
| Hot-volume routing | VERIFIED | `/mnt/edgesniffer-kefi-hot` is `/dev/vdc1`; source and materializer units have explicit routing overrides. |
| Mount identity fail-closed check | IMPLEMENTED | Service pre-start checks require the mountpoint and `/dev/vdc1`. A reboot/mount-loss simulation has not been run because it would interrupt capture. |
| Materializer timer | INTENTIONALLY INACTIVE | It remains disabled while the full-history JSONL design is replaced. Capture continues; this prevents another escalating I/O scan from competing with ingest. |
| Capacity guard | IMPLEMENTED, HEALTHY CHECKED | Guard checks root plus hot path, stops materialization first, and only reports `EMERGENCY_PAUSED_VERIFIED` after a unit-state check. Mocked stop-failure regression coverage is still OPEN. |
| Root free capacity | WATCH | 112 GiB free at reconciliation. New source/promotion writes are routed off root; legacy root data still needs verified archive/release. |
| Hot free capacity | VERIFIED ADEQUATE | 406 GiB free at reconciliation. |

## Independent-Audit Checklist

### 1. Quarantine scientific epochs

**Status: OPEN.**

The following must become durable registry records before H3 discovery/fresh
collection:

- historical development corpus;
- H1-opened fresh evidence (development-only from now on);
- H2 artifacts (quarantined; no fresh economic result);
- operational routing restart at `2026-10-12T02:17:33Z`;
- future unopened H3 evidence.

Required invariant: no fresh H3 result can consume evidence earlier than its
new boundary, and no H1-opened outcome can be relabeled as confirmation.

### 2. Actual materializer regression suite

**Status: PARTIALLY VERIFIED.**

Resolved:

- public packet source path drift was corrected;
- catalog implementation was added to the packet;
- production event identity is explicitly `(source_frame_sha256, event_index)`;
- the public sanitized tests passed (`4 passed`) against the corrected packet.

Still open:

- a production-oriented test for a corrupt/replaced previously processed
  window; and
- a test that a failed `systemctl stop` cannot be reported as a verified pause.

### 3. Capacity safety and mount identity

**Status: IMPLEMENTED; NEGATIVE-PATH TEST OPEN.**

Implemented on the VPS:

- `RequiresMountsFor` and device identity checks for the source/materializer;
- source promotions and control output route to `/mnt/edgesniffer-kefi-hot`;
- warning pressure stops materialization before capture;
- emergency pause verifies the collector unit is inactive before claiming
  success.

Do not call this fully regression-proven until the negative paths are tested in
an isolated/mocked harness.

### 4. Evidence catalog

**Status: IMPLEMENTED FOR LEGACY JSONL; REDESIGN OPEN.**

The actual reference-only catalog implementation is packaged and a legacy
catalog exists. It still references `market-deduplicated.jsonl`, so it cannot
be the final catalog for the incremental evidence layer.

### 5. Retire repeated full-history JSONL dedup

**Status: EMERGENCY MITIGATION IMPLEMENTED; ARCHITECTURAL REPLACEMENT OPEN.**

The service runner no longer calls the global deduplicated-market rebuild after
each bounded cycle. A live four-window bounded cycle completed without that
rebuild. This removes the immediate repeated-scan hazard.

It does **not** yet provide the required canonical indexed event store. The
next implementation target is append-only, content-verified partition
ingestion followed by compact Parquet/DuckDB research tables.

### 6. Restart-safe content verification

**Status: OPEN.**

`state.json` currently remembers processed identifiers. It must instead verify
the existing receipt and output content hash before reusing a processed window.
Bare IDs are not sufficient scientific provenance.

### 7. Source-manifest / decoder binding

**Status: OPEN.**

Promotion admission must bind immutable source manifest/object hashes, and each
derived-window receipt must record decoder binary or source hash plus schema
version. A `COMPLETE` status/signature alone is not enough.

### 8. Temporal/actionability and paper-mechanics audit

**Status: OPEN.**

Before H3 discovery, audit same-slot ordering, `available_at_ns`, entry delay,
AMM fee/slippage arithmetic, non-tradeable outcomes, exits, and clustering.
No claim about a predictive/economic fingerprint may be made before this is
complete.

### 9. Bounded H3 discovery tournament

**Status: NOT STARTED.**

Preconditions: items 1, 5, 6, 7, and 8 must be verified. The tournament is
limited to 24 complete configurations, uses purged chronological folds and
pool/time clustering, and selects exactly one H3 by out-of-fold economic
advantage over controls. Selection AUC is diagnostic, not the objective.

### 10. Fresh H3 paper/control gate

**Status: BLOCKED BY PRECONDITIONS.**

It must start only after immutable H3 preregistration and a clean post-deploy
boundary. Its minimum floor is 300 accepted events, 40 distinct pools/mints,
8 chronological blocks, no pool above 10% of entries, plus the larger
variance-based sample requirement. It is not an arbitrary 24-hour test.

## Explicit Non-Claims

- H1 is rejected. It does not establish kEfi economic alpha.
- H2 has no one-shot fresh economic result and is quarantined.
- H3 has not started; no `H3`, `H4`, or live/dust experiment is authorized.
- Current healthy capture is operational collection, not a scientific paper
  cohort.
- Historical selection discrimination is not profitability or execution parity.

## Immediate Build Order

1. Write the epoch/quarantine registry and strict pre-H3 boundary guard.
2. Build verified incremental ingestion for sealed windows; do not re-enable
   the old full-history rebuild loop.
3. Bind source manifests, output hashes, and decoder identity to receipts;
   make restart reuse verify them.
4. Generate canonical compact event/research tables from already sealed data.
5. Run the temporal/actionability and paper-mechanics audit.
6. Only then register the fixed, maximum-24 H3 discovery tournament.

