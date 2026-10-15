# Canonical State V1 - Filtered Identity Manifest and RPC Reconciliation

**Scope:** offline implementation and deterministic tests only.
**Deployment:** none. No V8 binary, service, configuration, RPC endpoint,
cohort, holdout, execution path, or historical evidence was changed.

## Requirement-by-requirement completion

| Requested item | Status | Implementation / evidence |
|---|---|---|
| Review prior RPC conformance report | VERIFIED | `RPC_CONFORMANCE_CROSSCHECK_V1_REPORT.md` reviewed. Its conclusion - RPC works but V8 does not retain every filtered identity - is the design input for this gate. |
| Record every filtered identity before selection | VERIFIED offline | `FilteredIdentityManifestWriter` appends a hash-bound identity before any wallet/candidate interface exists. |
| Bind source/epoch/slot/signature/receive provenance | VERIFIED | Every record has source instance, continuity epoch, slot, signature, receive sequence, receipt time, source-frame hash, continuity status, and append/hash-chain fields. |
| Do not call arrival/index canonical order | VERIFIED | `observedArrivalIndex` is explicitly arrival provenance only and reconciliation ignores it. |
| Compact append-only, hash-bound writer | VERIFIED offline | JCS self-hash exclusion, previous-record chain, synced append, atomic checkpoint publication, reopen scan, and corrupt-checkpoint rejection. |
| Derive block relevance from actual data | VERIFIED offline | `FinalizedTransactionInput.accountOrProgramIds` is compared to frozen program IDs. Signatures alone never determine relevance. |
| Compare complete relevant set with manifest | VERIFIED offline | Exact set equality produces `MATCHED`; missing, extra, duplicate, or conflicting evidence fails closed. |
| Preserve provider/block/version failure disposition | VERIFIED offline | Explicit errors include `BLOCK_NOT_FINALIZED`, `BLOCK_INVENTORY_INCOMPLETE`, `UNSUPPORTED_TRANSACTION_VERSION`, and identity/fork conflicts. Receipts retain a reason code. |
| Complete interval contract | VERIFIED offline | `IntervalCompleteness` requires a reconciled first capture slot, one source instance/epoch, no restart/gap, and matched receipts. No arbitrary time warm-up exists. |
| Capture-start boundary and restart continuity | VERIFIED offline | A capture interval begins only with its first explicitly reconciled finalized slot. Any source restart/epoch change or gap closes that interval; it cannot bridge into another interval. |
| Storage quota / reserve | VERIFIED offline | Writer checks manifest quota and protected free-space reserve before append. It writes identity metadata only, never a raw transaction spool. |
| Bounded memory | VERIFIED offline | The writer has an explicit 1,024-identity in-memory de-duplication cap in the harness; per-block reconciliation cap is 4,096 identities and the pending finalized-block registry cap is 32 slots. Oversize input fails closed. |
| RPC budget / concurrency policy | VERIFIED offline primitive | `RpcBudget` enforces request count and validates concurrency/rate parameters. A live RPC dispatcher is deliberately out of scope. |
| Deterministic complete interval test | VERIFIED | Passed. |
| Incomplete / extra / duplicate tests | VERIFIED | Passed. |
| Gap / reverse arrival / fork tests | VERIFIED | Passed. |
| Restart / checkpoint recovery / corrupt manifest tests | VERIFIED | Passed. |
| Explicit instruction-path remaining work | VERIFIED | Documented separately in `INSTRUCTION_PATH_DECODER_REMAINING_V1.md`; reconciliation does not claim semantic ordering. |
| Live deployment / paid RPC probe / V8 change | NOT PERFORMED BY DESIGN | Prohibited by this gate. |

## Implementation

Added:

- `sanitized-code/canonical-state-v1/filtered_identity_manifest_v1.rs`
- `sanitized-code/canonical-state-v1/offline-harness/`

The offline source-copy integration also adds the module export to the VPS
Rust package source. It is not compiled into or deployed to any running
binary. The source file SHA-256 in that offline source copy is:

`619f7914a7e866ef847e41007e7b6a3548ee730ecbe533bbdfc6a3ce2beeb653`

### Manifest semantics

Each received Yellowstone-filtered identity is sealed before any wallet,
candidate, model, feature, or selection logic can inspect it. The record has:

- a monotonically chained manifest sequence;
- source instance and continuity epoch;
- slot and signature;
- receive sequence and receipt timestamp;
- optional observed provider/arrival index, labeled non-canonical;
- source frame SHA-256;
- explicit `CONTINUOUS` status; and
- previous-record and record SHA-256 values over JCS bytes.

The writer rejects missing binding/provenance, duplicate `(slot, signature)`,
sequence gaps on restart, source/epoch mismatch, corrupt records/checkpoints,
quota exhaustion, and protected-reserve violation. It does not retain raw
transaction bytes.

### Reconciliation semantics

For a finalized complete block, the reconciler checks contiguous transaction
indices, unique signatures, exact transaction count, supported transaction
version, and finality. It computes the expected relevant set from actual
transaction account/program IDs using the frozen canonical program list.
It then requires exact equality with the captured per-slot identity set:

`block-derived relevant identities == manifest identities`

Missing, extra, duplicate, conflicting, unavailable, unsupported-version, or
forked evidence is a rejected/failed disposition. A later slot cannot cover a
missing earlier block. A `FinalizedBlockRegistry` rejects two block identities
for the same slot.

### Interval completeness

There is intentionally **no clock warm-up rule**. An interval starts at its
first captured slot only after that slot has a matching finalized proof. It is
complete only while all collected proof slots match and a single source
instance/continuity epoch stays continuous. A source restart, continuity gap,
or epoch change closes the interval. A later clean interval must be a new
separately certified interval.

## Tests and results

The isolated local Rust harness ran:

```text
cargo test
16 passed; 0 failed
```

Covered cases:

- exact complete set reconciliation;
- reversed Yellowstone arrival order without interpreting it as block order;
- missing and extra identities;
- duplicate identity append;
- malformed receive-sequence gap;
- duplicate transaction index / incomplete block inventory;
- unsupported transaction version;
- finalized-block fork conflict;
- output quota exhaustion;
- protected-reserve and explicit in-memory identity-cap rejection;
- restart/reopen plus checkpoint validation;
- corrupt manifest rejection;
- source/epoch mismatch rejection;
- reconciled capture-start requirement; and
- post-start source continuity break rejection.

The existing canonical primitive tests also passed in the harness, including
JCS/hash vectors, causal-order validation, immutable historical coverage,
journal recovery, and corrupt-segment rejection.

The real VPS Rust package test was attempted but is **BLOCKED by the existing
environment**: the system default is Cargo 1.75, while the project lockfile is
lockfile v4 and requires a newer Cargo. No toolchain was installed or changed.
The local harness used a current Cargo and compiled the exact sanitized module.

## Remaining decoder work

Identity reconciliation proves only which finalized transactions were present.
It does **not** prove the semantic event's instruction path or permit semantic
state mutation. See `INSTRUCTION_PATH_DECODER_REMAINING_V1.md`.

## Readiness verdict

**OFFLINE MANIFEST/RECONCILIATION PRIMITIVE: READY FOR INDEPENDENT REVIEW.**

**READY FOR LIVE CANARY: NO.** A live adapter, a durable per-slot manifest
integration, a bounded RPC dispatcher, provider-size validation, and exact
instruction-path decoder work remain mandatory before any isolated canary.
