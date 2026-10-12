# Pre-H3 Assignment Builder Status

Status: `IMPLEMENTED_AND_REGRESSION_VERIFIED_PENDING_FINAL_INDEPENDENT_AUDIT`

No PRE_H3_ACTIONABILITY_QUALIFICATION_V1 boundary, cohort, or fresh outcome
evaluation was created while implementing this stage.

The assignment publisher is:

`sanitized-code/scripts/kefi_pre_h3_assignment_builder_v1.py`

SHA-256:

`1078d5ea5ef6679e33092ab69b9b305ba9f31e26add3753bb25b5ef31c3068b2`

It accepts only a hash-matching frozen fingerprint, an already-written
boundary receipt, verified scientific materialization receipts, the canonical
row files they bind, the epoch registry, frozen mechanics/matcher, and the
frozen evaluator identity. It does not create a boundary and does not execute
the evaluator.

Before delayed entry, non-tradeability, or horizon availability is derived, it:

- scores only frozen pre-entry state;
- admits every match after warm-up;
- retains the deterministic first source event per `(source instance, epoch,
  pool, 20-second availability bucket)`;
- builds the fingerprint non-match universe from pre-entry state only;
- invokes the frozen context matcher;
- keeps zero-control targets in the all-target population.

It then derives delayed-entry, 120-second, non-tradeable, and hard-wall
outcomes through frozen mechanics, derives cluster IDs and contiguous
chronological blocks deterministically, emits evaluator JSONL, and writes a
manifest hash-binding fingerprint, boundary, scientific receipts/canonical
rows, registry, matcher, builder, evaluator, seed, and JSONL.

The new end-to-end regression invokes the full builder twice using synthetic
canonical JSONL, a materialization receipt, a boundary receipt, and a real
future-unopened epoch registry; the assignment output is byte-identical after
rerun/restart. A separate regression proves that post-entry/PnL-like fields do
not alter target/control identities. Matcher tests cover each individual frozen
stratum dimension, including all fee fields, observed age, liquidity, layout,
and mint identity.

Local kEfi suite: `64 passed`.

Final audit question: verify that this builder exactly implements the approved
qualification contract before authorizing the pre-H3 qualification boundary.
