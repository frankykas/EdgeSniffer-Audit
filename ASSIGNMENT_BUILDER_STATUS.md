# Pre-H3 Assignment Builder Status

Status: `CANONICAL_INPUT_REMEDIATED_AND_REGRESSION_VERIFIED_NO_BOUNDARY`

No PRE_H3_ACTIONABILITY_QUALIFICATION_V1 boundary, cohort, or fresh outcome
evaluation was created while implementing this stage.

The assignment publisher is:

`sanitized-code/scripts/kefi_pre_h3_assignment_builder_v1.py`

SHA-256:

`1b78bb03f54a323174ca4c2740197dab2b7e0eba8efa63344c2f389f7e1cc39b`

It accepts only a hash-matching frozen fingerprint, the hash-matching original
strict-preentry scorer, an already-written
boundary receipt, hash-bound canonical intrinsic-event JSONL and canonical-input
manifest, the epoch registry, frozen mechanics/matcher, and the frozen evaluator
identity. Receipt-list concatenation is forbidden. It does not create a boundary
and does not execute the evaluator.

Before delayed entry, non-tradeability, or horizon availability is derived, it:

- scores only frozen pre-entry state;
- admits every match after warm-up;
- retains the deterministic first source event per `(source instance, epoch,
  pool, 20-second availability bucket)`;
- builds the fingerprint non-match universe from pre-entry state only;
- invokes the frozen context matcher;
- keeps zero-control targets in the all-target population.

Observed pool first-seen comes from the entire ordered scientific row stream
before warm-up or fingerprint filtering, not from the target/control subset.
It then derives delayed-entry, 120-second, non-tradeable, and hard-wall
outcomes through frozen mechanics, derives cluster IDs and contiguous
chronological blocks deterministically, emits evaluator JSONL, and writes a
manifest hash-binding fingerprint, boundary, canonical input/exposure
provenance, registry, matcher, builder, evaluator, seed, and JSONL.

The builder permits exactly one source instance/continuity epoch for an active
qualification. Any break is a cohort-level hard wall and fails closed; it does
not combine pre-break and post-break assignments.

The end-to-end regression invokes the full builder twice using synthetic
canonical JSONL, its qualification-input manifest, a boundary receipt, and a
real future-unopened epoch registry; the assignment output is byte-identical
after rerun/restart. A separate regression proves that post-entry/PnL-like
fields do not alter target/control identities. Matcher tests cover each
individual frozen stratum dimension, including all fee fields, observed age,
liquidity, layout, and mint identity.

Complete private kEfi suite: `75 passed`. Sanitized public suite: `59 passed`.

The approved contract is now hash-bound to the canonical-input implementation.
No pre-H3 qualification boundary is authorized or created by this status file.
