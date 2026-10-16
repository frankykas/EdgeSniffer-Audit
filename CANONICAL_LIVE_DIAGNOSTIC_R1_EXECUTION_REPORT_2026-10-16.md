# Canonical Live Diagnostic R1 Execution Report

Authorization: owner-approved one-shot diagnostic R1.

Outcome: `INCONCLUSIVE`.

The one authorized live run was executed exactly once. It did not establish a
complete five-slot interval, and it was not retried.

## Deployment

| Item | Result |
| --- | --- |
| Reviewed audit commit | `cd13c42090feec60db4b4081b83c4902b62d7eb6` |
| Installed canary SHA-256 | `4b9d34aba7b76dfaeb82760490e2d18ff984d533e58e45514140dde438a75c2b` |
| Read-only verifier SHA-256 | `889c83b5c1c174613f9be9cb4dd1e4d0e34dfa7ccc86658e5f35ecf9a283b906` |
| Rollback backup | `/usr/local/lib/edgesniffer/edgesniffer-kefi-finalized-rpc-canary-v1.pre-live-diagnostic-r1-20261016T145807Z` |
| Diagnostic root | `/mnt/edgesniffer-canonical-hot/canonical-state-v1-live-diagnostic-r1-20261016T145835Z` |
| Transient unit | `edgesniffer-canonical-live-diagnostic-r1-20261016T145835Z.service` |
| Persistent unit restart | None |
| V8 state before and after | `inactive` |

Preflight passed: `/mnt/edgesniffer-canonical-hot` resolved to `/dev/vdd1`;
free space was 1,496,649,646,080 bytes before and 1,496,648,646,656 bytes
after; the 181 GiB startup floor, 160 GiB runtime reserve, and 6 GiB cap were
configured. Credential references were present without exposing values. No
static finalized range variables were inherited. The dust service was not
changed.

## Live Execution

| Item | Measured result |
| --- | --- |
| Start | `2026-10-16T14:58:35Z` |
| Finish | `2026-10-16T14:58:41Z` |
| Runtime | 5.566 seconds |
| Anchor/start slot | `457679394` |
| Requested inclusive end slot | `457679398` |
| Finalized produced slots from `getBlocks` | `457679394` through `457679398` (5) |
| Skipped slots | 0 |
| Exact reconciled slots | 4: `457679394` through `457679397` |
| Unsealed slot | `457679398` |
| Resulting error | `FINALIZED_RANGE_RECONCILIATION_INCOMPLETE` |
| Source gap / epoch failure | false |
| Decoder rejects | 0 |
| Yellowstones filtered observations | 272 identities across the first 4 slots |

The source manifest contains identities only for slots `457679394` through
`457679397`: 63, 61, 104, and 44 respectively. The authoritative finalized
range included the fifth produced slot `457679398`; no corresponding captured
filtered identity set was present when the inventory attempted reconciliation.
The canary reported `MISSING_FILTERED_IDENTITY` and failed closed. This is
missing fifth-slot capture evidence, not a source-continuity claim.

## RPC and Resources

| Metric | Value |
| --- | --- |
| Total RPC allocations | 9 / 16 |
| Dispatcher requests | 3 |
| Dispatcher retry requests | 1 (subset of dispatcher requests) |
| Inventory `getBlocks` | 1 |
| Inventory `getBlock` | 5 |
| Scheduled retries | 2 |
| Pending-slot peak | 3 |
| Peak RSS | 70,119,424 bytes |
| Maximum Yellowstone message | 19,295 bytes |
| Output root size | 924,017 bytes |
| Canary-reported disk growth | 991,232 bytes |
| Storage guard violation | None |

No unused RPC allocation was spent, no request budget was exceeded, and no
transaction-submission capability was invoked.

## Evidence Preserved

| Evidence | Result |
| --- | --- |
| `getBlocks` proof SHA-256 | `79f2bacaf2f5cb61dabfc4ac10e0c567dd50ed775ad060a3422e9742e98e8161` |
| Finalized slot inventory | Not written: sealing stopped at incomplete slot `457679398` |
| Reconciliation receipts | 4 |
| Identity receipt records | 1,044 serialized records across those four complete block bundles |
| Instruction-path receipts | 128 |
| Creation receipts | 0 verified, 0 quarantined |
| Read-only verifier | Not run: `finalized-slot-inventory.json` does not exist |

The 1,044 identity-receipt records are retained exactly as produced. They are
not the same measure as the 272 filtered Yellowstone identities used by the
reconciler: current per-slot bundle generation emits identity receipts from the
full successful finalized block representation. This did not cause the R1
failure, but it must remain explicit in any subsequent review.

## Conclusion

`INCONCLUSIVE` is the only valid classification. The run demonstrates that the
installed binary honors the shared RPC budget, writes bounded evidence, and
fails closed when the fifth slot cannot be reconciled. It does not demonstrate
a complete bounded interval, a valid cohort, full market coverage, or any
trading property.

No second run, persistent capture, V8 activation, dust-service action, trading,
evidence deletion, or speculative code change was performed. The preserved
root and rollback binary are the complete R1 audit trail.
