# End-to-End Wiring Matrix

| Producer -> consumer | Evidence | Status | Gap / consequence |
|---|---|---|---|
| Yellowstone filtered TX -> identity manifest | real canary source and prior live receipt | LIVE-TESTED bounded | Filtered stream is not full market coverage. |
| identity manifest -> finalized dispatcher -> `getBlock` reconciliation | dispatcher source, 29 offline tests, prior 129/129 slot | LIVE-TESTED one slot | Current canary stops after a match; no continuous interval. |
| finalized JSON -> path decoder | `finalized_input` / `path_receipts` in actual canary source | BUILT, NOT DEPLOYED/LIVE-TESTED | Must compile/deploy only after review; no receipt has yet been produced live. |
| raw event -> durable event ID -> decoded path | `event_id` propagation in decoder | OFFLINE-TESTED | Multi-event event-to-invocation mapping still needs live fixture validation. |
| finalized path -> Canonical State | `canonical_state_v1.rs` | NOT WIRED | Producer writes quarantine receipts, not canonical semantic journal. |
| Pump migration creation witness -> forward cohort ledger | Python init decoder + Rust ledger | OFFLINE-TESTED, NOT WIRED | No live Rust adapter/capture service; standalone PumpSwap creation excluded. |
| canonical state -> F01-F13 | existing kEFi observer computes comparable rolling fields | NOT WIRED | It has independent source/state semantics; F01-F13 parity is unproven. |
| AOM ranker -> execution simulator | AOM audit artifacts | OFFLINE-ONLY / DATA-BLOCKED | No admissible public-universe partition. |
| simulator -> research outcome | AOM framework reports blocked under bad provenance | BLOCKED | No untouched real-data partition. |
| AOM -> execution | none | INTENTIONALLY ABSENT | Correct: research result must not arm execution. |

## Scenario traces

### A. Pump token -> PumpSwap migration

Existing Python code can verify a finalized PumpSwap `create_pool` instruction
and matching event. It is not yet invoked by the finalized-RPC canary. The
pool may therefore be visible in legacy observers without being admitted into
the new cohort ledger. This is the missing handoff for Path B.

### B. Pool buys and sells

Yellowstone event parsing derives reserves and side. Finalized reconciliation
can establish the transaction identity. The newly wired path adapter can prove
ordering, but currently only writes quarantined receipts. No canonical state
mutation or AOM feature update follows.

### C. Multiple inner instructions/events

The decoder handles static/loaded keys, outer indices, inner ordinal and stack
height and rejects non-unique mapping. Raw-event `event_id` preserves
one-to-one mapping. This is implemented/tested offline, not live-proven.

### D. Disconnect/restart/RPC failure

Manifest and dispatcher retain epoch/state and reject holes or source changes.
Canonical state rejects sequence gaps. The forward cohort contract likewise
quarantines. The active canary is not yet a continuous interval collector, so
end-to-end recovery has not been demonstrated.

### E. AOM candidate -> simulated trade

No actual current wiring exists. AOM can consume only an admitted canonical
partition; such a partition does not exist. Any ranking or paper economics
from legacy kEFi-selected evidence would be descriptive, not autonomous AOM
validation.
