# Canonical State V1 Scientific Coverage Gate

## Result

`BLOCKED_BEFORE_LIVE_INTERVAL_CANARY`

The finalized-RPC canary proved one important lower-layer fact: a finalized
block can reconcile the complete **filtered Pump/PumpSwap transaction identity
set** against Yellowstone (`129/129` in slot `457466348`).  That is not a
complete scientific market interval and it must not be reported as one.

## Verified in this pass

1. The authoritative path decoder is present in the production source and
   passes its focused offline suite: direct outer invocation, router CPI,
   loaded addresses, multiple swaps, multi-mint transactions, malformed
   metadata, failed transactions and ambiguous logs.
2. The decoder now carries a caller-supplied `event_id` from
   `SemanticEventInput` to `DecodedSemanticEvent`.  This closes the otherwise
   unsafe one-to-many association when one transaction contains multiple
   Pump/PumpSwap semantic events.  `event_id` is deliberately not part of the
   causal-order tuple.
3. The real canary binary still compiles after the decoder contract change.
   No service was restarted and no live canary was run.
4. The current producer source explicitly retains `scientific_interval_produced
   = false` after an exact transaction reconciliation.  This is correct.

## Hard admission blocker

The current live source is a filtered Pump/PumpSwap transaction stream.  Its
finalized RPC reconciliation can prove all relevant transactions for a slot,
but it cannot prove the complete *eligible-pool universe* at an arbitrary
decision boundary.  In particular it cannot establish the history needed for
the frozen lifetime feature `ever_observed_buy`, nor can it establish that a
pre-capture pool which is absent from the filtered stream is ineligible.

Consequently, releasing canonical semantic state or describing any short
reconciled interval as suitable for AOM-V0 would violate the approved
`CANDIDATE_UNIVERSE_INCOMPLETE` gate.  The correct disposition for all current
canary semantic candidates remains quarantined diagnostic evidence.

## Required smallest next implementation

1. Add finalized-RPC transaction-to-path adaptation to the actual producer:
   resolve static/loaded account keys and outer/inner instruction metadata,
   then call the decoder using the durable raw-event `event_id` mapping.
2. Persist per-slot path receipts and quarantine any event lacking a unique
   finalized invocation.
3. Feed an independently complete all-supported-pool bootstrap/recovery source
   through the same canonical sequence.  The coverage ledger may issue
   `UNIVERSE_COMPLETE` only after its approved proof, not after elapsed time or
   a filtered-slot match.
4. Only then run a bounded interval canary.  Every slot in its interval must
   reconcile; gaps, restarts or a missing universe proof make the interval
   incomplete rather than bridgeable.

## Requirement checkpoint

| Requirement | Status | Evidence |
| --- | --- | --- |
| Finalized filtered identity/index reconciliation | VERIFIED | prior canary receipt for slot `457466348` |
| Authoritative decoder unit behavior | VERIFIED | 5 focused decoder tests |
| Stable raw-event to decoded-event mapping | VERIFIED | `event_id` contract propagation |
| Decoder wired into live producer | NOT STARTED | current canary counts legacy decoder events only |
| Continuous finalized-slot interval | NOT STARTED | current canary stops after first exact slot |
| Complete eligible-pool universe | BLOCKED | filtered stream cannot establish lifetime pool history |
| Scientific admission/release | BLOCKED | correctly guarded by separate universe proof |
| Bounded live scientific interval canary | NOT STARTED | prohibited until prior gates close |

## Tests

* `cargo test --lib pump_instruction_path_decoder_v1`: 5 passed.
* `cargo test --bin kefi_finalized_rpc_reconciliation_canary_v1 integrated_tests --no-run`: compiled successfully.

Warnings in the canary (legacy constants/imports and dispatcher telemetry field)
are pre-existing cleanup items; none relaxes an integrity gate.
