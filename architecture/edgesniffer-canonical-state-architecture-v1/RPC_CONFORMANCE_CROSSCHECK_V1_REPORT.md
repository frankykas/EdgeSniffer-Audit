# Canonical State V1 - Bounded RPC Conformance Cross-Check

**Mode:** non-scientific bounded offline provider conformance test.  
**Date:** 2026-10-15 UTC.  
**Live impact:** none. V8, its watchdog, all observers, cohorts, and evidence
roots remained unchanged.

## Completion checklist

| Requirement | Status | Evidence |
|---|---|---|
| Identify secure/general RPC provider | VERIFIED | `SECURE_RPC_URL` is a Shyft regional RPC host; its value and key are not retained here. |
| Verify finalized `getBlock` support | VERIFIED | Shyft public documentation supports finalized block, signature-only, and full transaction responses. |
| Verify billing/rate treatment before requests | PARTIALLY VERIFIED | Public Shyft pricing is plan-dependent. Exact account plan/credit consumption requires dashboard access and was not inspected. Operator-authorized request ceiling applied. |
| Select retained Yellowstone sample | VERIFIED | Three distinct recent V8 candidate-ledger rows with retained slot, signature, and observed index. |
| Hard request cap | VERIFIED | **12 / 20** requests used. |
| Short runtime / output quota | VERIFIED | Network probes took roughly 11 seconds. Three sanitized summaries totaled 4,700 bytes; raw responses were never persisted. |
| Finalized block identity + ordered signatures | VERIFIED | Returned for all three slots. |
| Retained signature is in stated finalized block | VERIFIED | True for all three samples. |
| Verify actual Pump/PumpSwap relevance | VERIFIED | Full finalized transaction detail confirmed program/account evidence for all three samples. |
| Classify related vs unrelated block transactions | VERIFIED | Full-block scans completed for all samples. |
| Validate complete filtered Yellowstone relevant set | BLOCKED | V8 retains selected wallet candidates, not every Yellowstone-filtered transaction. |
| State remaining live-completeness requirements | VERIFIED | Compact per-slot filtered manifest, finalized block proof, exact instruction path, and fail-closed set comparison are required. |

## Provider, billing, and request budget

The configured secure RPC is Shyft regional JSON-RPC. Shyft documents
`getBlock` with finalized commitment, signature inventories, full transaction
detail, and transaction-version configuration. [Shyft getBlock documentation](https://docs.shyft.to/solana/rpc-calls/http/getblock)

Shyft's public pricing shows plan-specific request limits and both
unlimited-credit and legacy credit-plan models. The exact account plan and
whether these calls consumed credits were not verifiable without private
dashboard access. No claim of zero cost is made. [Shyft pricing](https://shyft.to/solana-rpc-grpc-pricing)

| RPC request | Count | Result |
|---|---:|---|
| `getBlock(finalized, signatures)` | 3 | Successful. |
| `getTransaction(finalized, transaction version 0)` | 3 | Provider rejected all three because sampled transactions were version 1. |
| `getTransaction(finalized, transaction version 1)` | 3 | Successful. |
| `getBlock(finalized, full, transaction version 1)` | 3 | Successful. |
| **Total** | **12 / 20** | Cap respected. |

The sample was a three-slot bounded selection from the V8 candidate ledger.
Signatures and block hashes were replaced by SHA-256 identifiers. Temporary
sanitized summaries, not raw RPC responses, were written under `/tmp` and had
these hashes:

| Summary | SHA-256 |
|---|---|
| Signature-inventory comparison | `6a7e33d80dd00386b00a9a6c4c83f81c175cda049372a936ccd6f72c4a19735a` |
| Version-1 transaction retry | `1fbaf9d2f5d3ad85fcdac4b4fcf1d92857e2e0ee2c6a9efb6cec80f2b662e4d7` |
| Full-block relevance scan | `94b9fe82c785e135d65eefc570c0d4ef70252e8bf0fc06982855584fc7969e05` |

## Findings

| Sampled slot | Full transactions | Program/account relevant | Unrelated | Selected retained signature present | Transaction relevance |
|---:|---:|---:|---:|---|---|
| 457093576 | 787 | 220 | 567 | YES | YES |
| 457093732 | 490 | 156 | 334 | YES | YES |
| 457093762 | 308 | 125 | 183 | YES | YES |

All three retained candidate signatures appeared in their recorded finalized
block and their version-1 transaction responses confirmed the slot and
Pump/PumpSwap program/account evidence. Two sampled transactions had a failed
on-chain result; this is descriptive metadata only and was not used for any
scientific admission.

The relevance counts were obtained from actual full block message/account and
instruction data, not from signatures. They are only a program/account
relevance **superset**; exact semantic relevance requires the separate
instruction-path decoder.

## Ordering finding

Every sample's V8 `observedTransactionIndex` differed from its position in the
finalized block signature inventory. This is not evidence corruption: the V8
source writes canonical `transaction_index: None`, marks
`missing_canonical_ordering: true`, and preserves the Yellowstone value only as
`observed_transaction_index` provenance. It must not be promoted to canonical
Solana order. The block proof's transaction position/index is required.

## Filtered-set validation verdict

**BLOCKED / UNPROVEN.** This test proves bounded finalized retrieval works and
that three retained selected-wallet observations map to actual relevant
transactions. It cannot prove Yellowstone did not omit another relevant
transaction because V8's retained candidate ledger does not contain the full
Yellowstone Pump/PumpSwap filtered transaction set for these slots.

To validate the live filtered set, the canonical producer must persist a compact
per-slot manifest of every filtered transaction identity before wallet/candidate
selection, bind it to a complete finalized block proof, derive relevant
transactions from actual transaction data, and fail closed on any set mismatch
or ambiguous event path.

## Final verdict

**RPC CONFORMANCE: PASS** for bounded offline finalized block and transaction
retrieval.

**LIVE FILTERED-SET COMPLETENESS: BLOCKED / UNPROVEN.** This result does not
authorize a canary, observer cutover, cohort, or scientific decision.
