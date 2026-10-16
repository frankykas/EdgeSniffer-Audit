# Canonical State V1 Production Decoder and Coverage Strategy

## Production decoder integration

The actual finalized-RPC canary executable now retains PumpSwap raw semantic
events by durable `signature:log_index` event ID.  After exact finalized block
identity/index reconciliation, it adapts the authoritative `getBlock` JSON
transaction metadata into `FullTransactionInput` and invokes
`decode_instruction_paths`.

The adapter resolves static keys, loaded writable/read-only keys, outer
instructions, indexed inner-instruction groups, base58 instruction bytes,
stack heights, finalized transaction index, signature, success status and log
messages.  It uses the finalized transaction's array position as the proven
transaction index; Yellowstone arrival order is never used for causal order.

Each raw event has one result in an atomically published per-slot
`path-receipt-slot-<slot>.json`:

* unique invocation: `PATH_RESOLVED_QUARANTINED_UNIVERSE_INCOMPLETE`;
* missing/ambiguous/unsupported metadata: `QUARANTINED` with the exact
  decoder reason.

Receipts are hash-bound and serialize exact quantities as strings.  The code
does not release semantic state: universe coverage remains an independent
admission gate.

## Build/test evidence

* Production release binary built successfully:
  `b1e58e3811eaaf2afed41ae9dd7bf620560505866ecb7f5d7b45c84bada5a2d9`.
* Can-ary integration tests: 3 passed.
* Path decoder adversarial suite: 5 passed (direct, router CPI/loaded
  addresses, multi-event/multi-mint, failed/ambiguous/malformed and admission
  refusal).
* Universe bootstrap suite: 2 passed.

No binary was deployed and no new canary was run.

## Historical coverage feasibility

### Path A: complete historical replay

Not achievable from the currently configured retained evidence and ordinary
RPC alone.  The available Shyft finalized `getBlock` endpoint can reconcile a
known slot, but neither the filtered Yellowstone history nor current
`getProgramAccounts` establishes a complete historical list of eligible pools
and every pool's lifetime buy state from an authoritative lower boundary.
Doing so would require a provider-supported complete historical block/event
archive from an explicit start slot through the boundary, with every relevant
Pump/PumpSwap event decoded and finalized-reconciled.  Existing evidence does
not prove that interval; no cost or completeness guarantee has been approved.

### Path B: strict forward-only restricted cohort

Scientifically defensible only as a **new, explicitly scoped** cohort, not as
equivalence to historical AOM-V0 completeness.  It would admit only pools
whose creation is observed after a declared finalized boundary in a source
that proves every supported-pool creation and every subsequent supported
event.  All pre-boundary pools remain `UNKNOWN` and excluded.  For admitted
post-boundary pools, `ever_observed_buy` becomes known from the contiguous
canonical sequence.  Any gap, source replacement, missing creation, or failed
finalized reconciliation invalidates the cohort.  This requires an explicit
AOM eligibility amendment because frozen AOM-V0 currently targets the complete
eligible-pool universe rather than “new pools since boundary.”

## Verdict

The production executable now invokes the authoritative decoder, but no
`UNIVERSE_COMPLETE` receipt can legitimately be issued from existing sources.
The smallest next step is to approve either a provider-backed full historical
replay or a separately preregistered forward-only new-pool cohort; neither may
be silently substituted for the frozen complete-universe requirement.
