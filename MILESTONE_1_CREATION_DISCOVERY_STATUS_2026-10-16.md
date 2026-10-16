# Milestone 1: Rust Creation Discovery Status

## Implemented in the real finalized-RPC canary source

`pumpswap_creation_witness_v1.rs` ports the strict finalized semantics of
`scripts/pumpSwapInitDecoderV1.py::validate_finalized_init`:

* successful transaction required;
* published PumpSwap `create_pool` discriminator and exact 18-account layout;
* matching 344-byte `CreatePoolEvent` with matching pool, creator, mints,
  index, amounts, coin-creator and mayhem fields;
* inner `create_pool`, index zero, Pump-program participation and migration
  log required for `PUMP_MIGRATE_CREATE_POOL`;
* standalone PumpSwap creations are intentionally not admitted.

The actual canary's exact-reconciliation branch now invokes this decoder after
the finalized block proof, writes atomic `creation-receipt-slot-<slot>.json`,
and binds each receipt to the SHA-256 of the finalized identity receipt.  A
verified creation is marked quarantined because no forward cohort is open.

## Validation

* Rust creation witness unit tests: 2 passed (failed transaction and standalone
  exclusion).
* Existing semantic-reference test suite:
  `python3 scripts/testPumpSwapInitDecoderV1.py` passed.
* Real canary executable compiled and release binary SHA-256 is
  `a91b4d194deb035b5597c9746e83ef8b40d18d68130fbd00bf11a1459dfc58b3`.

## Remaining requirements not yet closed

This milestone is **not diagnostic-canary ready**:

1. The Rust module still needs fixture parity tests for a valid canonical
   migration, multiple creations/inner instructions, event mismatch, missing
   metadata, duplicate/tampered receipt and zero-event finalized slots.
2. Creation receipts are written by the producer but are not yet consumed by
   `forward_new_pool_cohort_v1`; the ledger therefore cannot be opened through
   receipt-only evidence yet.
3. A filtered Yellowstone stream plus per-slot `getBlock` can prove
   reconciliation for slots that it observes, but cannot currently prove it
   received every slot in a contiguous boundary range.  It needs an explicit
   finalized slot-range inventory/progress contract.  Without it, “all
   observed creations” is not complete discovery.

## Diagnostic plan once those are closed

Capture a declared finalized slot range, retain compact identities/path/creation
receipts only (64 MiB cap), reconcile every slot through finalized RPC, and
fail on any missing slot, source epoch transition, reconnect, queue overflow,
RPC failure, creation witness error or receipt hash mismatch.  Initial budget:
no more than 30 minutes, 64 MiB, 16 `getBlock` calls, and no AOM decision or
cohort opening.  The current 16-call budget likely covers only seconds of
contiguous Solana slots, so it is a wiring diagnostic rather than a useful
cohort duration.

## Operational preflight

* V8 is inactive, as required.
* Canonical finalized-RPC canary is inactive.
* `edgesniffer-teacher-impulse-mc5000-dust.service` is outside EdgeSniffer's
  Canonical source root (`/root/bot/FinalBot-cya`), configured `Restart=always`
  and observed with `NRestarts=199152`, currently `activating/auto-restart`.
  It must be isolated by its owner before any shared-host diagnostic.  No
  service action was taken here.
