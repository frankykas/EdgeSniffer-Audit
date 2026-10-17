# Canonical R4 Source-Closure Remediation

## Status

`OFFLINE_IMPLEMENTED_NOT_LIVE_TESTED`

This is the consolidated repair for the R1-R4 capture-completeness failures. It is not an R5 authorization and no VPS service, evidence root, credential, or live endpoint was changed while preparing it.

## Root Cause

The previous canary combined three independent facts into one brittle control path:

1. Yellowstone transaction delivery and provider metadata.
2. Finalized chain identity and canonical ordering.
3. Readiness to seal a fixed finalized range.

R4 demonstrated that Yellowstone `SubscribeUpdateTransactionInfo.index` did not equal the finalized `getBlock` transaction position for hundreds of otherwise exact `(slot, signature)` matches. Treating that source index as canonical rejected correct source observations. R1 also demonstrated that a finalized slot notification alone does not prove that all filtered transaction messages for that slot have arrived.

## Frozen Repair Contract

### Finalized RPC is canonical truth

`getBlock(finalized)` supplies blockhash, transaction membership, supported-version validation, and transaction index. It is the only source used for canonical transaction ordering.

### Yellowstone is observability evidence

The transaction stream is journaled immediately. Its `info.index` is preserved as `sourceTransactionIndex` telemetry and may support provider diagnostics, but it is not canonical order and is never compared to the finalized transaction index.

### A filtered block update closes source observation

The canary now requests a filtered Yellowstone block subscription using the same program filter and transaction/vote/failed semantics as the transaction subscription. The block update is persisted as an atomic, self-hashed `SourceBlockClosure` containing:

- source instance and continuity epoch;
- relevance-policy hash;
- slot and blockhash;
- provider executed-transaction count;
- raw block-update hash;
- sorted filtered `(signature, sourceTransactionIndex)` identities.

For every produced finalized slot in range mode, sealing requires all of:

1. a source finalized-slot notification;
2. a source block closure bound to the finalized blockhash, source instance, epoch, and relevance policy;
3. exact source-block-closure and finalized-RPC signature-set equality;
4. exact transaction-stream and finalized-RPC signature-set equality.

No inactivity timeout or higher-slot observation proves closure. Missing closure remains `INCONCLUSIVE` and cannot seal.

### Durable receipt-chain binding

Every produced-slot bundle now contains the closure SHA-256. The V2 finalized-slot inventory also records that SHA. The persisted forward-cohort reader reopens the closure file, verifies its hash and bindings, and rechecks its finalized signature set before admitting the slot bundle. A closure therefore cannot be silently ignored, swapped, or replaced with a caller flag.

### Range-first lifecycle

When `FINALIZED_RANGE_SLOT_COUNT` is present, dispatcher work is disabled for that run. The run reserves and performs only the bounded finalized-range inventory work; there is no pre-range dispatcher queue able to exhaust the 16-request diagnostic budget. Legacy no-range mode retains its one-slot collection path, but it also requires a valid block closure before it writes a successful receipt bundle.

## What This Proves, And What It Does Not

This repair can prove completeness of the **same filtered Pump/PumpSwap relevance set** over a bounded finalized range, subject to an actual provider conformance result for the block feed. It does not prove a complete all-Solana transaction inventory, a complete historical eligible-pool universe, AOM market completeness, profitability, or scientific cohort admission. Those gates remain separate and fail closed.

## Evidence Preservation

R1, R2, R3, and R4 roots remain unchanged audit evidence. Their failures are regression cases; none are reused as complete intervals.

## Remaining Pre-Live Gate

Before another diagnostic, run one explicit provider conformance check against the real Shyft block feed to verify that the delivered block transaction list obeys the requested account filter and includes the matching filtered identities. If that feed is unsupported, rejected, or semantically different, the repaired run must remain `INCONCLUSIVE`; do not substitute a timer.
