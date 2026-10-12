# Pre-H3 Void-Cohort Remediation

Published: 2026-10-12

## Cohort disposition

The initial `PRE_H3_ACTIONABILITY_QUALIFICATION_V1` cohort is permanently
**VOID**. No new boundary is created by this remediation.

The assignment builder correctly failed closed when two individually sealed
windows were concatenated and contained the same intrinsic source event. The
failure was an input-assembly defect, not evidence that the source events
conflicted:

- 67,038 window occurrences;
- 50,882 unique canonical intrinsic identities;
- 16,156 identical cross-window exposures;
- 0 within-window duplicate identities;
- 0 conflicting intrinsic identities.

No frozen evaluator was run, no PASS/REJECT outcome was calculated, and no
PnL or qualification outcome was accessed for the voided cohort.

## Remediation

The only accepted multi-window input is now the deterministic canonical export
from `kefi_incremental_event_index_v1.py`:

- `canonical-events.jsonl` contains every intrinsic `(source_frame_sha256,
  event_index)` event exactly once;
- `window-exposures.jsonl` separately preserves receipt/window multiplicity;
- the qualification manifest hashes both files and declares the input policy;
- derived outcome annotations are excluded from intrinsic identity;
- the assignment builder accepts only the canonical input plus its manifest;
- the epoch registry records a closure as a whole-cohort void, so it cannot be
  reused across a later boundary.

This removes receipt concatenation as a possible qualification-input path.

## Verified replay evidence

The exact two-window replay was rebuilt twice, without invoking the evaluator:

| Artifact | SHA-256 |
| --- | --- |
| Canonical events, run A/B | `187bb55ba608e0c99b58ccec0a6559e22bf388a4818cdde5dcf4f3e4d39295a4` |
| Window exposure provenance, run A/B | `aaac2a26469efee901c5e59d631fa0616128515031f28ccd54169a20f6da8685` |
| Assignment output, run A/B | `d63d9d8e85f12c8942a7789d8d2544ad450df56f9426f410c86a400204dfb448` |
| Original duplicate-summary forensic record | `63db75668ac7c172438518f3b237492943a8331ffad7586a6c27eedbebbccd16` |
| Original duplicate-details forensic record | `c9f9ed20b2cba79832282cfe6d752b7e703126d0a48f8ca7a041fb001e61fc9b` |

Private evidence and raw forensic JSONL remain off-repository. The values above
are references for an auditor with authorized access to the retained evidence.

## Updated deployment bundle

The original remediated deployment manifest was
`18ddb32a818becb8c0827cf633d640a85ef22164d09068d0079064ba69bff89d`.
The explicit-void-diagnostic remediation bundle was superseded by the final
non-bridging-void deployment bundle after the complete regression run. Its
manifest SHA-256 is recorded below with the final component hashes.

Final non-bridging-void deployment manifest SHA-256:
`f036e37581c481f148a58e825601ff6493683aaaee49bd3b966241a661cb24a4`.

| Component | SHA-256 |
| --- | --- |
| Incremental canonical index | `e8139b71c585e0058b7083ce63e712faa85e2961bbfa422f9b6c07c9603577ed` |
| Qualification assignment builder | `1b78bb03f54a323174ca4c2740197dab2b7e0eba8efa63344c2f389f7e1cc39b` |
| Scientific epoch registry | `bff3ed4d5f7e085d0ae4ec72a743a64cdcfa416c11fc02de0e026e69f86499f1` |
| Qualification runner | `97b32a855d4d493c378828d63b9b779bb492d5317e51ec4b9749c937cd0bd766` |
| Duplicate forensic helper | `6ff8cacc2e6640c13ed9be66d0f7cbaa96e29064b29fb16953095dcb7cbc6aa9` |
| Frozen fingerprint artifact | `db08899d92f4d69d8c0234cfff9cbd05bd27dfb1365aed88d08b85f88cfc1b4a` |
| Frozen machine-readable qualification contract | `d36f78896102456155efcb934321e98394255b155ff38f5c6f8d68636b91e615` |
| Frozen prose qualification specification | `920ab7e4ccba0d66f5dd8389296acd2e6a07407bba020657eec83bfc702988bf` |

## Regression state

The complete private regression suite was run after remediation: **75 passed**.
The sanitized public suite included in this repository was also run: **59
passed**. Coverage includes canonical/exposure separation, conflicting
intrinsic identity failure, outcome-field identity invariance, restart-order
determinism, indexed actionability equivalence, and voided-epoch ineligibility.

## Boundary state

The old timer remains disabled. The capture source remains operationally
independent of this work. This publication authorizes neither a replacement
qualification boundary nor a scientific outcome evaluation.
