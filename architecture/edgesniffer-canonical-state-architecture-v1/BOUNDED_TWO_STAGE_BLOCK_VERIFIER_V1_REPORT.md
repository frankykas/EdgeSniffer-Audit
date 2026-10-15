# Bounded Two-Stage Block Verifier V1

## Scope and Safety

This is an offline-only implementation gate.  No service was restarted or
replaced.  V8, the existing observer, 2CQ, execution, frozen models and all
historical cohorts were not modified.  No holdout, shadow admission or
scientific decision was created.

## Requirement Checklist

| Requirement | Status | Evidence |
|---|---|---|
| Review prior canary report | VERIFIED | `CANONICAL_PRODUCER_CANARY_V1_REPORT.md` reviewed before this implementation. |
| Historical universe completeness stability | VERIFIED | Existing regression `later_gap_cannot_rewrite_an_earlier_complete_boundary`. |
| Stage 1 compact immutable proof | VERIFIED | `block_order_verifier_v1.rs::BlockOrderProof`; retains slot, block identity, contiguous transaction index/signature inventory, and frozen relevant-transaction subset. |
| Stage 1 avoids pool RPC and decoder work | VERIFIED | `from_complete_block` accepts only identities plus a frozen relevance bit; no RPC/decoder dependency. |
| Stage 1 bounded state | VERIFIED | hard caps: 16,384 transactions/proof and 32 pending slots. |
| Reject incomplete/conflicting/unverifiable proof | VERIFIED | count, contiguous-index, duplicate index/signature, JCS self-hash, and conflicting same-slot proof checks. |
| Stage 2 filtered transaction reconciliation | VERIFIED | every filtered transaction must match both slot and `(transaction_index, signature)` in the proven relevant set. |
| Prove no relevant transaction omitted | VERIFIED in offline verifier | release requires the *exact equality* of received identities and the Stage 1 relevant identity manifest. |
| Preserve transaction/instruction order | VERIFIED in offline verifier | release orders by transaction index then strictly ordered, numeric instruction paths; unresolved/invalid paths reject. |
| Delayed identity resolution cannot reorder state | VERIFIED | release additionally requires resolution of every relevant identity. |
| Bounded deferred work/backpressure | VERIFIED in verifier state | 4,096 pending filtered transactions/slot; cap breach fails closed. Live bounded RPC worker adapter remains separate. |
| Reversed arrival / late transaction | VERIFIED | `reversed_arrivals_release_by_block_order_only_when_complete`, missing proof rejection. |
| Missing, duplicate and conflicting transactions/blocks/forks | VERIFIED | `rejects_late_missing_duplicate_and_conflicting_inputs`. |
| Unresolved paths / slow RPC | VERIFIED | `unresolved_instruction_and_slow_identity_resolution_cannot_mutate_state`. |
| Restart and corrupt proof/checkpoint | VERIFIED | `restart_corrupt_proof_and_memory_pressure_fail_closed`. |
| Memory-pressure failure | VERIFIED | oversized complete block rejected before retention. |
| No scientific state mutation before proofs | VERIFIED in offline verifier | `release_slot` is the only release API and requires exact membership, resolution, and paths. |
| Live-provider Stage 1 adapter | BLOCKED | Shyft complete block updates are decoded as an entire gRPC message before application code can compact them. The existing attempt approached the 1 GiB canary limit before bounded progress. |
| Provider proof that the filtered transaction feed is complete | BLOCKED | `SLOT_COMPLETED` proved unsafe. Block messages have full inventory but no demonstrated compact filtered-inventory delivery. |
| Authoritative instruction-path source | BLOCKED | current PumpSwap decoder has log-event order but not a verified Solana instruction path for each semantic event. Events must remain `ORDER_AMBIGUOUS`. |
| Offline legacy/new replay parity using retained canary input | BLOCKED | no complete bounded validation fixture exists yet; the captured attempts are not complete scientific inputs. |
| Resource measurements from complete capture | BLOCKED | no valid complete capture exists. Live source rate cannot be inferred from the prior 271 KB snapshot measurement or the aborted block canary. |
| Ready for another isolated canary | NO | blockers below must be closed first. |

## Offline Test Results

Run on the isolated VPS source tree with the pinned modern Rust toolchain:

```
cargo test --lib                                      24 passed
cargo test kefi_canonical_source_v1 --lib              2 passed
cargo test --bin kefi_canonical_canary_v1              1 passed
```

The 24-test ledger suite includes the four new verifier tests plus the prior
canonical journal, JCS, coverage, recovery and corruption tests.

## Actual Bounded State Parameters

These are hard implementation limits, not storage-rate projections:

- maximum transaction identities in one compact proof: 16,384;
- maximum unsealed proof slots: 32;
- maximum reconciled filtered transactions for one slot: 4,096;
- no pool lookup, full PumpSwap decoding, feature calculation or state
  mutation occurs in Stage 1;
- a release is withheld, not partially emitted, whenever any prerequisite is
  missing.

## Remaining Provider/Decoder Blockers

1. **Complete inventory transport.** Yellowstone sends complete block content
   as one decoded message.  Application code cannot currently make that
   message itself streaming.  A future adapter must either demonstrate a
   provider-supported compact block inventory endpoint or enforce a much lower
   upstream message bound with a retained fail-closed spill design.  Increasing
   memory alone is not a proof or a solution.
2. **Relevant-set attestation.** The Stage 1 `relevant_transactions` manifest
   must be derived from every complete block using the frozen Pump/PumpSwap
   account/program rule.  A filtered transaction subscription alone cannot
   attest absence of a matching transaction.
3. **Instruction paths.** Full transaction/instruction decoding must expose
   the approved outer/inner path before a semantic event can be released.
   Existing log event index is not substituted.
4. **Live evidence.** Until the first two items are demonstrated on a bounded
   fixture, there is no complete validation interval and no valid old-vs-new
   replay/equivalence measurement.

## Required Next Gate

Do not relaunch the canary yet.  First implement a provider adapter that can
derive and durably retain a compact complete transaction inventory without
pool resolution, then show that the retained inventory produces an exact
relevant-set manifest for a small bounded block fixture.  If Shyft cannot
provide this under the configured message limit, the system must remain
fail-closed and the provider limitation must be recorded rather than relaxed.

