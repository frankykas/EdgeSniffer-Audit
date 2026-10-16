# Architectural Risk Review

## Highest-impact findings

1. **No complete vertical slice exists.** The architecture has strong pieces,
   but finalized reconciliation, path receipts, creation discovery, canonical
   state, F01-F13 parity, AOM ranking and simulation are not wired as one
   tested pipeline. This is a proven integration gap.
2. **Finality and actionability are different products.** Finalized `getBlock`
   is suitable for scientific verification, not for low-latency entry. A
   future actionable signal must be prospectively observed first and verified
   later; it cannot wait for finality and then claim live feasibility.
3. **Full-universe completeness is expensive and currently unproven.** It is
   essential for unbiased market-wide AOM claims, but not for a clearly
   restricted forward-only new-pool cohort. Mixing these claims would create
   selection bias.
4. **Legacy independent collectors duplicate state authority.** V8/choice
   observers and the new Canonical path own overlapping pool/event logic. Until
   consumers migrate, equivalence is unproven and results are not exchangeable.
5. **Control plane is broader than the research surface.** Active/failing
   legacy units and a dust service in auto-restart make accidental interference
   plausible. This is an operational risk; it does not prove data corruption.
6. **Storage was historically architecture-shaped.** Raw full-meta and derived
   caches were allowed to multiply. The new compact receipt/journal direction
   is right, but no measured sustained end-to-end rate exists yet.

## What is overbuilt / should be deferred

* Do not build market-wide AOM modeling, additional feature tournaments, or
  execution adapters before one restricted end-to-end data product exists.
* Do not continue historical replay design without a provider that actually
  offers complete historical coverage at a bounded cost.
* Do not extend legacy kEFi holdout machinery to stand in for autonomous AOM.

## Essential guarantees versus study-specific guarantees

Essential for any credible prospective cohort: durable identity, explicit
epoch/gap handling, causal path ordering, pre-decision timestamps, no leakage,
and realistic execution ledger. Full historical eligible-pool completeness is
additionally required for the frozen market-wide AOM-V0 claim. A restricted
new-pool cohort can omit it only through a reviewed amendment and explicit
scope limitation.
