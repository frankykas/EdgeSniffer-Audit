# kEfi Audit Remediation Status

Last reconciled: 2026-10-12 11:30 EDT (2026-10-12T15:30Z) against the live VPS and public regression suite.

This is a status ledger, not a claim that the research is ready for live trading.
`VERIFIED` means checked against the live host or an executable test. `IMPLEMENTED`
means code/config exists but still needs the stated validation. `OPEN` means no
claim has been made and no H3 paper boundary may rely on it.

## Pre-H3 Mechanics Closure Update (2026-10-12T15:15Z)

**Infrastructure status: READY FOR INDEPENDENT PRE-H3 SPECIFICATION AUDIT
ONLY.  Scientific H3 status: NOT SPECIFIED / NOT STARTED.**  No H3 boundary,
cohort, timer, paper result, or live-execution permission has been created.

The following new evidence has been produced from one *post-repair* promoted
window, under an isolated engineering-only `FUTURE_UNOPENED` registry epoch:

| Gate | Status | Durable evidence |
| --- | --- | --- |
| Epoch admission mandatory in scientific materialization | VERIFIED | The scientific runner always supplies `--scientific`, registry, and boundary inputs. The paper contract now requires the resulting scientific receipt and binds its row hash; a standalone JSONL cannot pass the contract. |
| Writer sequence propagated to durable order | VERIFIED | Fresh source rows carry `source_instance_id`, `continuity_epoch`, `durable_output_sequence`, `event_index`, and frame SHA. Canonical order is `(source_instance_id, continuity_epoch, durable_output_sequence, event_index, source_frame_sha256)`. |
| Tie ordering / restart determinism | VERIFIED | Public regression coverage proves equal receipt timestamps order by durable sequence then event index, and canonical export remains byte-identical when descriptor arrival/recovery order changes. |
| Exact PumpSwap transition mechanics | VERIFIED | Whole-window mechanics gate checked all 28,725 fresh rows: 28,725 exact transitions; 21,490 supported/tradeable; 7,234 noncanonical quote layouts plus 1 pre-quote-underflow explicitly fail-closed. |
| Entry delay / exits / controls / clustering | VERIFIED AS MECHANICS, NOT AS PERFORMANCE | The library and regressions enforce positive same-instance/same-epoch delay, observed-state-only exits, deterministic same-layout different-mint controls, and epoch/pool/time-bucket clusters. No outcome, PnL, or predictive result was read. |
| Fresh source health | VERIFIED AT CHECK | Source instance `2026-10-12T14:59:43.875661353Z-pid533030` reached 1,211,404 frames with zero reconnects, receive errors, continuity gaps, promotion failures, or queue-full events. |

Authoritative live-proof artifacts on the VPS:

- `/mnt/edgesniffer-kefi-hot/kefi-capture-first-live-v8/operations/pre-h3-live-proof-v1/proof.json`
  `d4b185fe09732b5584066e1b28ef64c0dc39116a5c48329e1b9b75c2e4dcfa92`
- `.../contract.json`
  `304a78ee121c2911ffa8d3119ec02919f23af791f8c1dd6f3171743be3b4498d`
- `.../mechanics-gate.json`
  `82e1fb6e0a15acf25b930ba7ee48adbf1932adaec997388b7bffdf4a21a84804`

The former H3 proposal was correctly reclassified: it was an actionability
qualification, not an answer to H1's economic failure.  The only immediate
proposed study is now
`sanitized-code/KEFI_PRE_H3_ACTIONABILITY_QUALIFICATION_V1_SPEC_FOR_INDEPENDENT_AUDIT.md`.
It needs independent review before a **pre-H3 qualification** boundary is
written.  A real H3 remains blocked pending separate offline discovery of one
new post-H1 economic hypothesis and its own independent review.

The qualification now has an executable frozen control matcher and evaluator.
Controls require identical source instance/epoch, 20-second availability
bucket, canonical layout, amount mode, complete fee tuple, observed pre-entry
pool-age bucket, and reconstructed pre-entry liquidity bucket, with a different
mint.  The matcher never reads post-entry trajectory fields.  The evaluator
reports `A_T_all` over every admitted target and `Delta_A_paired` only over
targets with one or more frozen controls; it has predeclared PASS / REJECT /
INCONCLUSIVE rules with 10,000 cluster-bootstrap replicates, a 70% absolute
all-target actionability lower-CI floor, and a 10-point paired-effect lower-CI
floor.  This is implementation/test verification only, not fresh evaluation.

## Current Safety State

| Surface | Status | Evidence / consequence |
| --- | --- | --- |
| Capture source | VERIFIED HEALTHY | `edgesniffer-kefi-full-meta-capture-first-v8.service` active; Frankfurt source health at 04:02Z reported fresh frames, zero reconnects, errors, continuity gaps, queue-full events, and promotion failures since the 02:17Z operational restart. |
| Source watchdog | VERIFIED HEALTHY | Watchdog active. This establishes current process health only; it does not make the pre-restart epoch scientific evidence. |
| Hot-volume routing | VERIFIED | `/mnt/edgesniffer-kefi-hot` is `/dev/vdc1`; source and materializer units have explicit routing overrides. |
| Mount identity fail-closed check | IMPLEMENTED | Service pre-start checks require the mountpoint and `/dev/vdc1`. A reboot/mount-loss simulation has not been run because it would interrupt capture. |
| Materializer timer | INTENTIONALLY INACTIVE | It remains disabled while the full-history JSONL design is replaced. Capture continues; this prevents another escalating I/O scan from competing with ingest. |
| Capacity guard | VERIFIED HEALTHY; HARDENED | Guard checks root plus hot path plus the live `/dev/vdc1` mount identity. Missing-path measurement is itself an emergency input. At warning it verifies both materializer timer and service; failure is explicit. Emergency pause requires successful stop plus loaded/inactive/dead/zero-PID state. |
| Root free capacity | WATCH | 112 GiB free at reconciliation. New source/promotion writes are routed off root; legacy root data still needs verified archive/release. |
| Hot free capacity | VERIFIED ADEQUATE | 406 GiB free at reconciliation. |

## Independent-Audit Checklist

### 1. Quarantine scientific epochs

**Status: VERIFIED FOR THE SCIENTIFIC PIPELINE; OPERATIONAL GENERIC PATH
REMAINS NON-SCIENTIFIC BY DESIGN.**

The following must become durable registry records before H3 discovery/fresh
collection:

- historical development corpus;
- H1-opened fresh evidence (development-only from now on);
- H2 artifacts (quarantined; no fresh economic result);
- operational routing restart at `2026-10-12T02:17:33Z`;
- future unopened H3 evidence.

Required invariant: no fresh H3 result can consume evidence earlier than its
new boundary, and no H1-opened outcome can be relabeled as confirmation.

The registry serializes appends, verifies the existing chain before append,
uses durable record-then-journal publication, recovers only a fully valid
orphan record after interruption, and rejects duplicate/tampered records.
Eligibility requires membership in exactly one post-boundary
`FUTURE_UNOPENED`/`UNOPENED` epoch. H1-opened, H2-quarantined, operational,
pre-boundary, gap, overlapping-epoch ambiguity, and invalid kind/status cases
are rejected. A post-boundary future-unopened record is accepted.
The separately tested cohort-admission guard calls this policy using real UTC
parsing, including exact fractional-second boundaries. The only supported
scientific materializer runner requires the registry, boundary, strict row
validation, and deterministic source order. The paper contract requires the
resulting scientific receipt and its row hash. Generic operational
materialization cannot pass that scientific contract. No epoch record has been
used to create a qualification or H3 cohort.

### 2. Actual materializer regression suite

**Status: PARTIALLY VERIFIED.**

Resolved:

- public packet source path drift was corrected;
- catalog implementation was added to the packet;
- production event identity is explicitly `(source_frame_sha256, event_index)`;
- the public sanitized tests passed (`4 passed`) against the corrected packet.

Still open:

- a production-oriented test for a corrupt/replaced previously processed
  window.

Capacity-guard regressions now cover successful stop, failed stop, unknown
unit, still-active/nonzero-PID state, failed `systemctl show`, missing hot-path
measurement, and failed materializer service stop at warning pressure.

### 3. Capacity safety and mount identity

**Status: IMPLEMENTED; GUARD NEGATIVE-PATH TESTED.**

Implemented on the VPS:

- `RequiresMountsFor` and device identity checks for the source/materializer;
- source promotions and control output route to `/mnt/edgesniffer-kefi-hot`;
- warning pressure stops materialization before capture;
- emergency pause verifies successful stop, loaded unit, inactive/dead state,
  and zero PID before claiming success;
- each guard cycle verifies that the live hot mount is still `/dev/vdc1`.

Mount-loss is deliberately not simulated against the live collector. The guard
and stop-verification negative paths are isolated/mocked; runtime mount
identity is verified every guard cycle.

### 4. Evidence catalog

**Status: IMPLEMENTED FOR LEGACY JSONL; REDESIGN OPEN.**

The actual reference-only catalog implementation is packaged and a legacy
catalog exists. It still references `market-deduplicated.jsonl`, so it cannot
be the final catalog for the incremental evidence layer.

### 5. Retire repeated full-history JSONL dedup

**Status: QUEUE-DRIVEN IMPLEMENTATION AND ISOLATED LIVE PROOF VERIFIED;
CONTINUOUS SCIENTIFIC TIMER NOT ENABLED PENDING SPEC APPROVAL.**

The service runner no longer calls the global deduplicated-market rebuild after
each bounded cycle. A live four-window bounded cycle completed without that
rebuild. This removes the immediate repeated-scan hazard.

A queue-driven SQLite canonical index now consumes only newly published
materializer descriptors, verifies the receipt/rows/source bindings once,
separates intrinsic source-event identity from window exposure, and recovers a
claimed-but-unfinished descriptor safely. Its local regressions prove that a
normal subsequent run does not require old receipts. It is deliberately not
yet enabled on the live materializer timer. A separate low-priority systemd
runner was installed but remains disabled; its isolated VPS queue/recovery
smoke passed without touching capture. A two-run isolated VPS proof over
copied sealed windows produced identical canonical exports after an interrupted
descriptor claim: 25,406 events and 35,317 exposures. A fresh post-repair live
scientific proof additionally indexed 28,725 ordered rows with source-instance,
epoch, and durable-sequence provenance. Continuous scientific timer activation
is intentionally deferred until specification approval; periodic full
integrity scrubs remain outside the hot append path.

### 6. Restart-safe content verification

**Status: VERIFIED FOR THE PRE-H3 QUALIFICATION PATH.**

The materializer verifies remembered output against its current receipt and
rows hash. The canonical index persists receipt hash, source-manifest
references, decoder identity, intrinsic event identity, and window exposure.
The fresh live proof verified restart-safe canonical export under this binding.
Bare IDs remain insufficient provenance and are rejected.

### 7. Source-manifest / decoder binding

**Status: VERIFIED FOR PRE-H3 QUALIFICATION; DATASET-GENERATION POLICY OPEN
FOR FUTURE H3 DISCOVERY.**

Materializer receipts bind partition-manifest/object hashes and decoder binary
hash. The canonical index separates intrinsic event identity from window
exposure, and the scientific contract binds the exact receipt/row hash. A
future H3 discovery dataset still needs a separately reviewed decoder-schema
generation policy and authenticated compact-table publication.

### 8. Temporal/actionability and paper-mechanics audit

**Status: PRE-H3 MECHANICS VERIFIED; TRUE H3 ECONOMIC CONTRACT NOT YET
SPECIFIED.**

A bounded 182,339-row / 70,365-source-event audit found no malformed identity,
fee, or conflicting-event defects and established that `event_timestamp` is
later than source receipt in every sampled row. Actionability uses
`received_at_unix_ns`, not chain event time. The repaired live source now emits
source instance, continuity epoch, and durable writer sequence; equal
receipt-time ordering is deterministic. A fresh 28,725-row live proof
validated exact raw instruction/event transitions, pre-state reconstruction,
non-tradeable exclusions, same-epoch delay/mark mechanics, frozen strata, and
cluster-aware evaluator logic. It forbids summed fee-BPS approximation.
No economic claim may be made from this qualification.

### 9. Bounded H3 discovery tournament

**Status: NOT STARTED; STILL REQUIRED FOR A TRUE H3.**

Preconditions: items 1, 5, 6, 7, and 8 must be verified. The tournament is
limited to 24 complete configurations, uses purged chronological folds and
pool/time clustering, and selects exactly one H3 by out-of-fold economic
advantage over controls. Selection AUC is diagnostic, not the objective.

### 10. Pre-H3 qualification, then fresh H3 paper/control gate

**Status: PRE-H3 QUALIFICATION SPECIFICATION PROPOSED; TRUE H3 BLOCKED BY
DISCOVERY.**

The proposed pre-H3 qualification has a fixed all-target denominator and a
cluster-aware actionability endpoint. It cannot be renamed H3 or used for
economic claims. A true H3 starts only after the tournament selects one novel
post-H1 economic hypothesis, that hypothesis is preregistered, and a clean
post-deploy boundary is independently approved. The pre-H3 representation
floors are 300 targets, 40 pools/mints, 8 chronological blocks, no pool above
10% of targets, and >=70% target control availability; they are not an
arbitrary 24-hour test.

## Explicit Non-Claims

- H1 is rejected. It does not establish kEfi economic alpha.
- H2 has no one-shot fresh economic result and is quarantined.
- H3 has not started; no `H3`, `H4`, or live/dust experiment is authorized.
- Current healthy capture is operational collection, not a scientific paper
  cohort.
- Historical selection discrimination is not profitability or execution parity.

## Immediate Build Order

1. Independently audit `PRE_H3_ACTIONABILITY_QUALIFICATION_V1`; do not create
   any boundary until approved.
2. Run that qualification once, if approved, and report PASS / INCONCLUSIVE /
   REJECT without PnL or strategy tuning.
3. Generate compact research tables and execute the bounded offline discovery
   protocol to select one genuinely post-H1 H3 economic hypothesis.
4. Independently audit that separate H3 preregistration before its boundary.
