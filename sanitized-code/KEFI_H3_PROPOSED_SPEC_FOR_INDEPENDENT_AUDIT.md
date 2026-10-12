# Proposed kEfi H3 Specification (Not Yet Frozen)

Status: `PROPOSED_FOR_INDEPENDENT_AUDIT_ONLY`  
This document creates no cohort, scientific boundary, timer, or result.

## Question

Does the already frozen **base pre-entry kEfi fingerprint** identify fresh,
prospectively observable PumpSwap states that remain mechanically actionable
after a conservative fixed delay, relative to deterministic contemporaneous
non-match controls?

This is an actionability/selection paper gate. It is not a profitability,
live-trading, PnL, or execution claim.

## Fixed Inputs

- Fingerprint: existing frozen strict pre-entry artifact with SHA-256
  `db08899d92f4d69d8c0234cfff9cbd05bd27dfb1365aed88d08b85f88cfc1b4`.
- No crowded-impulse overlay, refitting, feature selection, threshold tuning,
  outcome-driven filtering, or model-family comparison.
- New evidence only, beginning at a boundary created **after** this proposal is
  independently approved. Historical, H1/H2/quarantined/operational, and
  pre-boundary evidence are rejected by the epoch registry.

## Required Evidence Contract

Every considered row must have all of the following:

1. A single `FUTURE_UNOPENED` and `UNOPENED` registry epoch containing its
   receipt timestamp, after the frozen boundary.
2. `source_instance_id`, `continuity_epoch`,
   `durable_output_sequence`, `event_index`, and source-frame SHA-256.
3. A unique deterministic source order:
   `(source_instance_id, continuity_epoch, durable_output_sequence,
   event_index, source_frame_sha256)`.
4. An exact instruction/event-validated PumpSwap transition.
5. Canonical SOL quote layout, a non-SOL base mint, and reconstructable raw
   pre-state. Unsupported layouts, missing state, duplicate order, source
   changes, and incomplete horizons are non-tradeable and excluded with a
   stable reason. They are never imputed.

## Paper Mechanics

- Availability clock: `received_at_unix_ns`.
- Fixed entry delay: `1,000,000,000 ns` (1 second), selected as a conservative
  operational delay rather than from outcome performance.
- Paper entry state: first supported same-source-instance/same-epoch state for
  the selected `(pool, mint)` at or after that delay.
- Warm-up: 120 seconds from the cohort boundary.
- Exit/mark: first supported same-source-instance/same-epoch state for the
  same `(pool, mint)` at or after 120 seconds following the paper entry.
- The exit is an observed-state mark only. H3 does **not** infer a
  hypothetical fill or claim PnL. A separate, independently reviewed exact
  executable-quote contract would be required before an economic paper test.

## Target and Controls

- Target: one deterministic first eligible base-fingerprint match per
  `(pool, 20-second availability bucket)`, evaluated strictly before that
  event updates state.
- Controls: up to four deterministic, same-source-instance/same-epoch,
  canonical-SOL-quote, different-mint non-match candidates. Selection is by
  SHA-256 of frozen seed plus target/control identities; no outcome field is
  read.
- Dependence clusters: `(source_instance_id, continuity_epoch, pool,
  20-second availability bucket)`. Any later estimate must report cluster-aware
  uncertainty; raw rows must not be treated as independent trades.

## Fresh Gate

The one-shot actionability summary may be produced only after all are true:

- three post-boundary sealed source partitions;
- at least 1,000 target fingerprint matches with complete delayed entry and
  120-second mark eligibility, or an independently approved smaller target;
- zero source reconnects, continuity gaps, late-earlier violations, decoder
  rejections, watermark regressions, and missing scheduled ticks since the
  boundary;
- exact source/decoder/registry/materializer/indexer hashes stored in the
  pre-evaluation manifest.

Any interruption invalidates the active candidate cohort rather than bridging
it. Valid pre-wall evidence may be retained only under a separately approved
segmented accounting amendment; no such amendment is proposed here.

## Explicit Non-Claims

H3 cannot establish profitability, executable fill quality, autonomous
trading readiness, or a permission to send a live order. Those require a
separate exact executable-quote/economic gate and then a separately approved
dust-size calibration.
