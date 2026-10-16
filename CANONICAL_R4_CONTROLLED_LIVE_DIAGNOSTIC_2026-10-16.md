# EdgeSniffer Canonical State V1 - R4 Controlled Live Diagnostic

## Scope and disposition

R4 was the one authorized five-slot, capture-only diagnostic from remediation
commit `b63d4ac6e31b65e89b015724412a21792a883bb0`.  It ran exactly once and
**FAILED CLOSED**.  No scientific cohort, canonical bridge, trading action, or
transaction submission occurred.

This is a `FAIL`, not an `INCONCLUSIVE`: the newly restored independent source
transaction-index consistency gate detected an integrity disagreement.  A
second source-completeness problem was also preserved: the fifth finalized slot
had no Yellowstone identities before the run stopped.

No R5 was prepared or executed.

## Preflight

| Check | Result |
|---|---|
| Approved source commit | `b63d4ac6e31b65e89b015724412a21792a883bb0` |
| Previous installed binary SHA-256 | `96887daf4a76dd7d4cc693076812be930135966c4515d5ef585784ac6500c799` |
| Required R4 candidate SHA-256 | `54d897f54fb4cb8dc2392da16227cfdd3da2cd5e8e9b051d385b9a20eada36cc` |
| Candidate verification | PASS: exact required SHA-256 |
| Canonical mount | PASS: `/dev/vdd1` mounted at `/mnt/edgesniffer-canonical-hot` |
| Available space at preflight | PASS: approximately 1.36 TiB, above the 181 GiB startup floor |
| Runtime storage controls | 160 GiB free-space reserve; 6 GiB evidence ceiling |
| Resource controls | 8 GiB RSS; 45-minute runtime; 16 total JSON-RPC attempts |
| R1/R2/R3 evidence | PASS: the three existing roots and their `run-binding.json`/`report.json` files remained present; not modified |
| V8 | PASS: `edgesniffer-v8.service` inactive; not changed |
| Dust calibration | Untouched.  It was already in a `203/EXEC` auto-restart condition before R4; R4 made no service/configuration change. |
| Credentials | Required Yellowstone and finalized-RPC environment variables were present.  No secret values were printed or committed. |
| Trading/submission isolation | PASS: the invoked diagnostic path was the capture/reconciliation binary and contained no transaction-submission invocation. |

## Atomic installation and rollback

The approved candidate was copied to a temporary sibling, re-hashed, and
atomically renamed into:

```text
/usr/local/lib/edgesniffer/edgesniffer-kefi-finalized-rpc-canary-v1
```

The installed SHA-256 after the rename was exactly:

```text
54d897f54fb4cb8dc2392da16227cfdd3da2cd5e8e9b051d385b9a20eada36cc
```

The former R3 binary remains in the immutable rollback location:

```text
/mnt/edgesniffer-canonical-hot/canonical-state-v1-r4-rollback-20261016T212130Z/edgesniffer-kefi-finalized-rpc-canary-v1.pre-r4
```

It retains SHA-256
`96887daf4a76dd7d4cc693076812be930135966c4515d5ef585784ac6500c799`.
The failed diagnostic did not cause an automatic rollback.

## One authorized run

| Field | Value |
|---|---|
| Fresh evidence root | `/mnt/edgesniffer-canonical-hot/canonical-state-v1-live-diagnostic-r4-20261016T212200Z` |
| Start / finish UTC | `2026-10-16T21:22:00.788164870Z` / `2026-10-16T21:22:09.623607213Z` |
| Elapsed time | 8.835 seconds |
| Target range | `457785574` through `457785578` inclusive (five contiguous slots) |
| Execution attempts | 1 |
| Automatic retry / second diagnostic | 0 |
| Process result | nonzero exit after the fail-closed gate |
| Source instance / continuity epoch | `finalized-rpc-canary-20261016T212200Z-2138956` / `1` |
| Reported source gap / epoch failure | `false` |
| RPC attempts | 6 / 16: `getBlocks=1`, `getBlock=5`, dispatcher=0, retry=0 |
| Peak RSS | 183,738,368 bytes (175.2 MiB) |
| Largest received message | 20,814 bytes |
| Disk growth | 24,080,384 bytes (22.97 MiB), below the 6 GiB limit |
| Decoder work | 761 transactions, 332 events, 0 decoder rejects |
| Reconciled/released slots | 0 / 0 |

The run used a transient one-shot unit with `Restart=no`; it did not modify a
systemd unit or restart any active service.

## Reconciliation evidence

The finalized inventory expected 867 relevant identities across the five target
slots.  Yellowstone captured the exact 761 signatures expected for the first
four slots and no extras.  The fifth slot was finalized and present in the RPC
inventory, but its 106 expected filtered identities were absent from the source
manifest when the fail-closed check ran.

| Slot | Yellowstone identities | Finalized expected identities | Signature-set result |
|---:|---:|---:|---|
| 457785574 | 152 | 152 | same signatures |
| 457785575 | 185 | 185 | same signatures |
| 457785576 | 186 | 186 | same signatures |
| 457785577 | 238 | 238 | same signatures |
| 457785578 | 0 | 106 | missing source evidence |

For the 761 same-signature records, 327 source indexes matched finalized
transaction indexes and **434 differed**.  The first preserved disagreement is
slot `457785574`, source `observedArrivalIndex=1172`, finalized
`transactionIndex=485`.

The durable report stopped at:

```text
FINALIZED_RANGE_SOURCE_RECONCILIATION_REJECTED:
FILTERED_SOURCE_TRANSACTION_INDEX_MISMATCH
```

This is evidence that the source field currently persisted as
`observedArrivalIndex` cannot be assumed to equal the finalized block
transaction index for this live provider path.  R4 did not weaken that gate or
reinterpret the value.  Separately, the fifth-slot absence confirms that the
range still cannot be sealed on finality alone.

## Post-run verification

All checks below were read-only.  No additional RPC request was made.

| Verification | Result |
|---|---|
| Durable checkpoint reload | PASS as a nonterminal checkpoint: `AWAITING_SOURCE_MATCH`, RPC accounting `6/16`, source instance and epoch persisted |
| Raw RPC evidence | PASS: one `getBlocks` and five `getBlock` payload hashes recomputed from disk matched their recorded SHA-256 values |
| Evidence root | Preserved: 13 files, 24,056,727 bytes; deterministic file-tree SHA-256 `b9d9601fa4e752ca8cd573007184f9c9ede20358fbc0149bc939afd744baf178` |
| Finalized inventory validation | Not applicable: no inventory was committed after the reconciliation rejection |
| Slot-bundle / identity-set validation | Not applicable: no receipt bundle was committed |
| Canonical bridge | Correctly absent; no bridge can exist before a complete receipt chain.  No scientific state was released. |
| Persisted cohort receipt-chain verifier | FAIL-CLOSED as expected: it could not open the required committed receipt-chain artifact (`No such file or directory`) |
| Cohort/trading admission | PASS: none occurred |

The absence of inventory, bundle, bridge, and cohort artifacts is the correct
state for a rejected pre-seal run.  It is not retroactively treated as a
complete interval.

## Remaining defect and required next decision

R4 exposes two distinct, preserved facts:

1. **Source-index semantics are unresolved.** The independently supplied
   Yellowstone field persisted as `observedArrivalIndex` differs from finalized
   canonical transaction order for 434 otherwise identical transactions.  The
   architecture must decide, with provider evidence and offline fixture proof,
   whether that field is a valid `source_transaction_index`, a provider-local
   arrival sequence, or is being mapped incorrectly before it can be used as a
   fail-closed equality gate.
2. **Five-slot source completeness is still unproven.** The selected fifth
   finalized slot had no matching filtered identities at sealing time.  This
   remains distinct from transport health (`sourceGapOrEpochFailure=false`) and
   must not be solved with an arbitrary sleep.

No remediation is implemented in this report.  Any future action requires a
reviewed design decision that addresses both evidence facts before another live
diagnostic is authorized.

## Isolation confirmation

R1/R2/R3 evidence was not modified.  R4 evidence and the immutable rollback
binary remain intact.  V8 stayed inactive; the dust service was untouched; no
trading, transaction submission, infrastructure purchase, evidence deletion,
or follow-up diagnostic occurred.
