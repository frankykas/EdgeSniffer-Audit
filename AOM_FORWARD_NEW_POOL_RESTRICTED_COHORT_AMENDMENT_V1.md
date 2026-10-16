# AOM Forward New-Pool Restricted Cohort Amendment V1

## Status

**DRAFT FOR REVIEW.** This amendment does not modify frozen AOM-V0 full-universe
eligibility and does not authorize collection or scientific admission.

## Scope

The restricted cohort contains only PumpSwap pools whose creation is proven by
one finalized transaction after the cohort boundary with all of:

1. a published PumpSwap `create_pool` instruction with the supported complete
   account layout;
2. a matching published `CreatePoolEvent` in the same successful transaction;
3. an authoritative finalized block transaction index and instruction path;
4. exact Yellowstone-filtered identity versus finalized-RPC reconciliation;
5. canonical Pump-migration evidence (`PUMP_MIGRATE_CREATE_POOL`).

Standalone PumpSwap pool creations are excluded, not inferred.  Every pool
created at or before the boundary is `PRE_BOUNDARY_UNKNOWN` and never enters
this cohort.

## Coverage and lifetime state

The source interval begins at a finalized boundary only after the first
reconciled source epoch.  Each relevant finalized slot must be reconciled.
For a newly admitted pool, `ever_observed_buy` is known only from its verified
creation transition and the contiguous canonical event sequence thereafter.
A source gap, restart/epoch change, missing slot, failed RPC reconciliation,
ambiguous creation witness, missing path receipt, or duplicate pool witness
quarantines the cohort.  It cannot bridge evidence from the other side.

## What this can support

It can support research about public, post-creation Pump-migration pools that
were completely observed under this cohort's explicit scope.  It can establish
the pool's observed lifetime only **since creation**.

## What this cannot support

It cannot estimate a market-wide complete pool universe, make claims about
pre-existing pools, substitute for frozen AOM-V0, or validate autonomous
profitability.  Its selection effect is material: it excludes standalone
PumpSwap creation and all pools that existed before its boundary.

## Implementation artifacts

* `scripts/pumpSwapInitDecoderV1.py`: finalized instruction + event witness.
* `forward_new_pool_cohort_v1.rs`: durable restricted cohort ledger.
* `pump_instruction_path_decoder_v1.rs`: authoritative path receipts.

## Diagnostic canary plan (requires separate approval)

Run a bounded capture-only service for no more than 30 minutes and no more
than 64 MiB compact receipts.  It must collect filtered Pump/PumpSwap
identities, finalized block reconciliations, creation witnesses, and path
receipts, but must write no AOM decisions.  Success means only that the
creation-discovery path produces complete restricted-cohort receipts; it does
not authorize science or trading.
