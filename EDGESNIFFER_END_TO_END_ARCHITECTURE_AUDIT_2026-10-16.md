# EdgeSniffer End-to-End Architecture and Implementation Audit

**Scope:** audit only.  This report used the production source on the VPS, the
preserved R1/R2/R3 evidence roots, installed unit files, and the public audit
repository.  No service was changed, no RPC request was made, and no evidence
was modified during this audit.

## Executive verdict

**Do not run R4.** The individual components are useful and several have good
fail-closed behavior, but the live canary is not a coherent end-to-end state
machine yet.  R3 is not a capacity problem and should not be solved with more
retries or a larger RPC budget.  It is a control-plane design error: it spends
the pre-range dispatcher allowance reconciling individual slots before a range
exists, while the only operation that can prove range completeness is the
reserved range inventory that is consequently never invoked.

The recommended path is a **targeted partial redesign of the live producer
control plane**, not a rewrite of all decoders, ledgers, or research rules.
Keep the finalized-RPC identity reconciler, instruction-path decoder, creation
witness, receipt schemas, and restricted forward-cohort consumer.  Replace the
dispatcher-first automatic range selection/lifecycle with one explicit,
durably checkpointed range-first capture state machine, then prove it through a
single deterministic production-entry-point integration harness before another
live diagnostic.

The current system is **not ready for a scientific market interval, AOM-V0
admission, a forward cohort, or live trading**.  The restricted forward cohort
also remains distinct from complete historical eligible-pool coverage.

## Evidence reviewed

| Source | What it established |
| --- | --- |
| `CANONICAL_R1_CAPTURE_COMPLETENESS_INVESTIGATION_2026-10-16.md` | A finalized notification alone did not prove that all same-slot filtered identities had arrived. |
| `CANONICAL_R2_LIVE_DIAGNOSTIC_2026-10-16.md` and receipt-emission fix | The final reconciler predicate and receipt-emission predicate had drifted; unrelated finalized transactions entered identity receipts. |
| `CANONICAL_R3_LIVE_DIAGNOSTIC_2026-10-16.md` | Ten dispatcher calls were exhausted in 11.3 seconds, zero slots reconciled, and the six inventory calls remained unused. |
| Production Rust tree `/root/bot/EdgeSniffer` | Actual canary, dispatcher, manifest, inventory, receipt, decoder, cohort, canonical-state, and unit implementations. |
| Installed systemd units | Old capture-only and current finalized-RPC paths coexist; V8 is inactive; the canonical ingest timer has a legacy V8-source dependency. |

## Actual architecture map

```text
Yellowstone finalized transaction filter + finalized-slot updates
  -> canary receive loop
     -> FilteredIdentityManifestWriter (hash chained, source index crosscheck)
     -> source-derived Pump/PumpSwap raw semantic events held in memory
     -> FinalizedRpcDispatcher (per-slot retry state, JSON checkpoint only)
     -> finalized getBlock
        -> relevance reconciliation
        -> finalized transaction adapter
           -> instruction-path decoder
           -> Pump-migrate creation witness
           -> per-slot identity/path/creation receipts
     -> getBlocks/getBlock bounded range inventory
     -> SlotReceiptBundle files
     -> forward_new_pool_cohort_v1 receipt consumer

Separate but not currently on the live canary path:
  canonical_state_v1 journal / causal-order verifier / coverage ledger
  pool_universe_bootstrap_v1
  legacy V8 ingest timer and historical research products
```

The first line is a filtered, low-latency source.  It cannot prove complete
market coverage by itself.  The finalized-RPC inventory is therefore the
authoritative completeness anchor.  That boundary is conceptually correct;
the defect is the lifecycle ordering and incomplete durable binding between the
two paths.

### Primary code references

Line references are to the production source inspected at audit time; the
canary source is deliberately compacted onto long Rust lines in places, so a
single reference may cover a complete control block.

| Component | Production location | Relevant lines |
| --- | --- | --- |
| Live canary reservation/range helpers | `rust-sidecar/src/bin/kefi_finalized_rpc_reconciliation_canary_v1.rs` | 56–59, 88–112 |
| Live receive loop, in-memory state, dispatcher-first lifecycle | same | 172–216 |
| Finalized range serialization/sealing | same | 94–112 |
| Dispatcher terminal retry behavior | `rust-sidecar/src/bin/finalized_rpc_dispatcher_v1.rs` | 9–28 |
| Identity hash chain and source index crosscheck | `rust-exact-ledger/src/filtered_identity_manifest_v1.rs` | 58–88 |
| Receipt validation and bundle object | `rust-exact-ledger/src/forward_cohort_receipts_v1.rs` | 32–64 |
| Receipt-driven cohort admission / quarantined-creation rejection | `rust-exact-ledger/src/forward_new_pool_cohort_v1.rs` | 23–45 |
| Canonical journal and causal-order implementation | `rust-exact-ledger/src/canonical_state_v1.rs` | 21, 75, 92–150 |
| Restricted/full pool-universe proof gate | `rust-exact-ledger/src/pool_universe_bootstrap_v1.rs` | 34–88 |

## R1, R2, and R3 as one failure story

| Run | Failure boundary | Confirmed lesson | Status after fixes |
| --- | --- | --- | --- |
| R1 | Fifth finalized slot sealed before filtered transaction delivery was proven complete | On-chain finality, source observation, and source completeness are different facts | Addressed locally, but not made into a single end-to-end state machine |
| R2 | Receipt writer emitted every successful finalized transaction rather than the reconciler's supported relevant set | Receipt generation must use the exact same relevance policy as reconciliation | Predicate sharing fixed; bundle-level set commitment still absent |
| R3 | Dispatcher consumed its ten pre-range calls before any exact reconciliation; range inventory never began | Dispatcher-first range selection has no progress guarantee | Unresolved; this is the present blocking design defect |

R3 facts from the preserved execution report: six source slots were seen,
`rpcCalls=10`, five retry requests were recorded, no inventory `getBlocks` or
`getBlock` call occurred, no exact match occurred, and the result was
`DISPATCHER_RPC_RESERVATION_EXHAUSTED`.  Source gap and epoch flags were false.
This is strong evidence of scheduler starvation, not proof of source failure.

## Prioritized findings

### Critical

| ID | Location | Root cause and impact | Required correction |
| --- | --- | --- | --- |
| F1 | `rust-sidecar/src/bin/kefi_finalized_rpc_reconciliation_canary_v1.rs`, automatic-range/dispatcher loop | A range is chosen only after a dispatcher exact match. Before that, all non-reserved calls are spent reconciling transient source state. In R3, delayed/incomplete source views retried until the ten-call allocation was exhausted; the six calls that could have independently defined the five-slot universe never ran. There is no liveness/progress guarantee. | Replace with a range-first state machine. Select a contiguous source-finality candidate window without pre-range RPC reconciliation; reserve and execute `getBlocks` + produced-slot `getBlock` first; then compare durable source identities against the authoritative expected sets until deadline. |
| F2 | `finalized_rpc_dispatcher_v1.rs` plus canary lifecycle | `MISSING_FILTERED_IDENTITY` is retried three times and then terminally failed even though source delivery can still be pending. The dispatcher is designed as a reconciliation retry machine, not as a source-completeness waiter. | Do not RPC-retry source arrival. Keep slots in `AWAITING_SOURCE_MATCH` until an overall bounded run deadline or explicit continuity failure. One finalized RPC response is sufficient unless the response itself is unavailable/invalid. |
| F3 | `kefi_finalized_rpc_reconciliation_canary_v1.rs`; `canonical_state_v1.rs`; `block_order_verifier_v1.rs` | The production canary bypasses the canonical state journal/order/coverage engine that its offline tests validate. It writes a separate receipt path and in-memory raw events. Passing canonical-library tests therefore does not prove the executable's scientific pipeline. | Make the live producer use one authoritative state engine and one durable source/coverage journal, or explicitly defer all scientific release until a production adapter exercises that engine. Do not claim the present receipt-only canary is canonical-state release. |
| F4 | `kefi_finalized_rpc_reconciliation_canary_v1.rs`, `fetch_pending_finalized_range` | RPC evidence is parsed into `serde_json::Value`, serialized again, and then hashed/persisted. The receipt is not a hash of the raw provider bytes. This loses exact transport evidence and makes provider-response reproduction weaker. | Persist bounded raw HTTP response bytes first, hash those bytes, then parse using a pinned schema. Bind parsed inventory/blocks to raw byte hashes. |

### High

| ID | Location | Root cause and impact | Required correction |
| --- | --- | --- | --- |
| F5 | Can-ary receive loop / dispatcher state | Finalized slot notifications, source frame identity, and source progress are held in memory. `dispatcher-state.json` does not restore finality, raw events, budget, manifest tail, or pending range. Fresh-root enforcement makes the executable effectively non-resumable despite lower-level restart claims. | Either declare any crash as `VOIDED_RUN` and prohibit restart claims, or implement one atomic checkpoint containing source journal cursor/hash, finality records, range state, expected sets, budget, and receipt commit state. |
| F6 | `SlotReceiptBundle::write_atomic`, `forward_cohort_receipts_v1.rs` | Individual receipt files are atomically renamed, but a bundle is not committed atomically. A crash can leave a plausible partial set. The consumer fails closed, but there is no explicit bundle manifest/commit marker or recovery protocol. | Write all receipt payloads to a staged directory, create a canonical bundle manifest containing every child hash and expected identity-set digest, then atomically publish the directory/manifest. Treat uncommitted directories as quarantined. |
| F7 | Reconciliation receipt/bundle schemas | A slot disposition records counts but not an authoritative sorted digest of the reconciled relevant `(signature, transaction_index)` set. The consumer validates individual receipts but cannot prove an omitted valid receipt or a policy drift did not change the full set. | Embed relevance-policy hash and sorted expected-set digest in finalized reconciliation receipts. Recompute the emitted identity-receipt digest in bundle validation and require exact equality. |
| F8 | `kefi_finalized_rpc_reconciliation_canary_v1.rs` resource handling | `all_records`, `raw_events`, pending finalized blocks, and decoded events grow in memory. The output cap is approximated from manifest length rather than all produced evidence. RSS is checked reactively. A large block JSON is materialized fully. | Introduce one quota/accounting owner for RAM, disk, queue slots, retained raw diagnostics, and RPC responses; bounded per-slot spooling with hard admission before allocation; release source payload after durable receipt linkage. |
| F9 | `req()`, reconciler, receipt writer, inventory adapters | Relevance/filter/version semantics exist in several locations. R2 fixed one drift instance, but the architecture still permits future divergence between Yellowstone request, finalized relevance derivation, receipts, and cohort acceptance. | One versioned `CanonicalRelevancePolicy` shared by subscription construction, RPC decoding, receipt writer, tests, and persisted policy hash. |

### Medium

| ID | Location | Root cause and impact | Required correction |
| --- | --- | --- | --- |
| F10 | Path receipt flow in canary | Path receipts are emitted only when a matching source-derived raw semantic event exists. Missing source semantic decoding can result in no explicit event-level quarantine, even when the transaction identity reconciles. Path receipts also do not bind to the source frame/manifest record that supplied the raw event. | For every relevant finalized transaction, emit either authoritative path evidence or a signed quarantine disposition. Include source identity/frame/manifest reference for source-derived event claims. |
| F11 | `forward_cohort_receipts_v1.rs` | Path receipts validate that a canonical-order object exists, but validation is not sufficiently schema-tight to prove each order tuple belongs to the matching slot/signature/index/event identity. | Validate full tuple equality and schema/version; bind path receipt, finalized transaction record, and creation witness through immutable identifiers. |
| F12 | Unit files and deployment surface | Old capture-only service, current finalized-RPC canary, and a legacy canonical-ingest timer coexist. The ingest timer points at a V8-derived root while V8 is inactive. This is an unreviewed hidden dependency and a source of deployment drift. | Inventory and explicitly classify units. The new canary must have its own isolated root and no legacy timer consumer. Add a deployment assertion that fails when a legacy producer/consumer shares the run root. |
| F13 | `pool_universe_bootstrap_v1.rs` / forward cohort | Full historical universe completeness is correctly blocked without `FULL_FINALIZED_HISTORY_REPLAY`; however, limited forward-new-pool receipts can be mistaken operationally for wider market coverage. | Keep the restricted-cohort contract separate in paths, schemas, and reporting. No AOM-V0 full-universe admission from forward-only evidence. |

## What remains sound

- Finalized RPC is correctly treated as the authority for canonical transaction
  ordering; Yellowstone `info.index` is an independent consistency check, not
  the ordering authority.
- Signature + slot + exact source/finalized set equality, with matching source
  and finalized transaction indexes where supplied, is an appropriate identity
  invariant for the bounded relevance reconciliation.  It does not prove the
  full market universe alone.
- The R2 change to use the reconciler relevance predicate for identity receipts
  is directionally correct.
- Creation evidence is strict: unsupported standalone PumpSwap creations and
  quarantined creation dispositions are not admitted by the forward consumer.
- The full-universe bootstrap refuses to issue `UNIVERSE_COMPLETE` without a
  full finalized historical replay.  This is the right fail-closed posture.

## Consolidated remediation plan

### M1 — One live-run state machine and durable run journal

Implement a single persisted state model:

```text
PRECHECK
  -> CAPTURING_SOURCE
  -> RANGE_SELECTED
  -> FINALIZED_INVENTORY_FETCHED
  -> AWAITING_SOURCE_MATCH
  -> RECEIPTS_STAGED
  -> RECEIPTS_COMMITTED
  -> VERIFIED_DIAGNOSTIC | INCONCLUSIVE | QUARANTINED | VOIDED
```

- `CAPTURING_SOURCE` journals every filtered identity and every source-finality
  update immediately; no dispatcher RPC retries occur here.
- `RANGE_SELECTED` chooses exactly N contiguous candidate slots using durable
  source finality evidence. It is an operational capture boundary, not a proof
  that identities have arrived.
- `FINALIZED_INVENTORY_FETCHED` consumes the pre-reserved `getBlocks` plus all
  produced-slot `getBlock` requests. It persists raw response bytes and
  expected relevant identity sets, including zero-relevant slots and skipped
  slots.
- `AWAITING_SOURCE_MATCH` accepts late source identities without extra RPC.
  It completes only when every produced slot exact-matches expected identities
  and source-index crosschecks, or becomes `INCONCLUSIVE_SOURCE_INCOMPLETE` at
  the fixed run deadline. Any source gap/epoch change immediately quarantines.
- `RECEIPTS_STAGED/COMMITTED` publishes an atomic bundle manifest; only a
  committed bundle may be passed to the cohort verifier.

This eliminates R3 starvation and consumes a predictable maximum of
`1 + number_of_produced_slots` inventory requests. The old dispatcher can be
reused only as a bounded finalized-RPC client for post-inventory repair of RPC
transport failures; it must not choose the range or repeatedly query for
missing source arrivals.

### M2 — Evidence binding and recovery

1. Define the shared policy version/hash and use it everywhere.
2. Add source-finality records to the identity journal, bound to source
   instance/epoch and receive sequence.
3. Persist raw finalized-RPC bytes, parsed schema version, blockhash, expected
   identity-set digest, and inventory boundaries.
4. Atomically checkpoint all run state. Until then, make a process crash void
   the run rather than pretending restart recovery exists.
5. Publish receipt bundles atomically and retain explicit partial/failed
   dispositions.

### M3 — Production-path scientific integration

Make production use the canonical ordering/coverage engine rather than a
parallel receipt-only path. Integrate instruction-path output through the same
journal and produce event-level quarantines. Keep full-universe and
forward-only restricted coverage modes cryptographically and semantically
separate.

### M4 — Release control

No new live diagnostic until the production-entry-point integration matrix
below passes. The first future live run is still a capture-only operational
diagnostic; it must not release AOM data merely because it passes identity
reconciliation.

## Deterministic offline integration strategy

The tests must execute the production entry-point state machine through trait
backed scripted Yellowstone and finalized-RPC transports. They must read
producer-written files back through the real cohort validator. Tests may not
manually construct receipt bundles to bypass producer code.

| Scenario | Scripted condition | Required assertion |
| --- | --- | --- |
| Nominal five-slot range | Mixed relevant/unrelated blocks, one skipped, one zero-relevant | Exactly `1 + produced` RPC calls; committed receipt chain validates; zero creation remains valid |
| R1 | Fifth finality arrives before its identities | Inventory may fetch, but sealing waits; late identities permit exact match without second RPC |
| R1 hard miss | Expected identity never arrives | `INCONCLUSIVE_SOURCE_INCOMPLETE`; no committed cohort bundle |
| R2 | Mixed successful unrelated, failed, unsupported-version transactions | Receipt set digest equals reconciler expected set; unrelated/failed/unsupported receipts absent |
| R3 | Ten noisy pre-range slots and delayed identities | Range inventory is still invoked with all reserved calls; no dispatcher-first starvation |
| Reverse delivery | Transactions arrive reverse index order | Exact identity reconciliation passes; finalized order is used for semantic ordering |
| Source index mismatch | Same signature/slot, differing source vs finalized index | Clearly named fail-closed mismatch; no bundle |
| Zero relevant interval | All produced blocks have no relevant tx | Complete interval receipts valid; no invented semantic event/creation |
| RPC transport failure | `getBlocks`/`getBlock` timeout, error, or malformed bytes | Budget charged; terminal incomplete/quarantine; no false retry of source data |
| Gap/epoch change | Source reconnect or continuity break during every phase | Run is quarantined/voided, never bridged |
| Receipt conflict | Tamper one byte, omit receipt, mismatch bundle digest | Consumer rejects fail-closed |
| Creation quarantine | Absent/mismatched event, duplicate witness, standalone creation | No admitted pool; receipts retain quarantine reason |
| Crash points | Crash before/after journal append, inventory, staging, commit | Restart restores identical state or run explicitly voids; never duplicates/bridges |
| Resource pressure | Queue, per-block response, disk reserve, RAM threshold | Admission stops before breach; durable incomplete receipt; no partial accepted bundle |
| Full semantic route | Router CPI, loaded addresses, multi-event transaction | Finalized order tuple and event identity are stable over two replays |

Use the R1/R2/R3 roots as read-only fixtures where their retained messages and
receipts are sufficient. Use deterministic synthetic raw/protobuf and JSON-RPC
fixtures for missing timing, full block, and crash cases. Do not claim a full
historical replay from the incomplete retained data.

## Measurable acceptance criteria before another live diagnostic

1. One production entry-point test covers the complete lifecycle from a
   scripted stream to persisted receipt-chain consumer validation.
2. The five-slot fixture proves a maximum of 16 total RPC attempts, including
   every failed attempt/retry, and inventory calls cannot be starved.
3. All three historical failure scenarios have regressions that initially fail
   against the legacy control path and pass against the replacement path.
4. Every finalized response hash corresponds to retained raw bytes; every
   receipt bundle has an atomic bundle manifest and exact expected/emitted set
   digest equality.
5. Crash behavior is either fully restored from one checkpoint or explicitly
   voided and tested at every commit boundary.
6. Static tests prove the production binary invokes the canonical
   ordering/coverage implementation, not only an offline library.
7. Resource tests show bounded memory/disk/queue behavior under a declared
   maximum block response and rate, with the 181 GiB startup floor, 160 GiB
   runtime reserve, and 6 GiB output cap preserved.
8. A release checklist verifies V8 remains inactive, the dust service remains
   untouched, a fresh run root is used, and no transaction-submission code is
   reachable.

## Repair versus redesign assessment

**Repair these components:** filtered identity manifest, finalized relevance
derivation, source-index crosscheck, instruction-path decoder, creation
witness, receipt validation, and restricted forward-cohort rules. Their core
semantics are usable once they are bound together correctly.

**Partially redesign the producer control plane:** scheduling, range selection,
durable run state, resource accounting, evidence publication, and the adapter
between the real producer and canonical state engine. These are the repeated
integration boundaries exposed by R1/R2/R3. Continuing with isolated patches
will create another boundary failure because the present architecture does not
make its lifecycle invariants explicit or test them end to end.

## Audit checklist

| Area requested | Status | Audit conclusion |
| --- | --- | --- |
| Yellowstone ingestion, filtering, finality, ordering | VERIFIED | Filtered source is appropriate only when finalized-RPC reconciled; finality does not imply identity completeness. |
| Finalized RPC, indexes, retries, reservation | VERIFIED | Identity reconciliation is sound in isolation; scheduler use is flawed and starvation-prone. |
| Range/startup/progress | VERIFIED | Dispatcher-first automatic range selection lacks a progress guarantee and caused R3. |
| Identity/path/creation receipts | VERIFIED | Individual mechanisms exist; bundle atomicity and set-level binding are insufficient. |
| Receipt chain/cohort quarantine | VERIFIED | Consumer is appropriately fail-closed for quarantined creations, but needs stronger bundle set validation. |
| Pump/PumpSwap semantics | VERIFIED | Decoder/witness path is strict; production event-level source binding/quarantine remains incomplete. |
| Memory/disk/concurrency/recovery | VERIFIED | Limits exist but are fragmented/reactive; real run restart recovery is not implemented end to end. |
| Cross-component consistency | VERIFIED | Production canary and canonical engine diverge; unit/deployment surface has legacy hidden dependencies. |
| Tests/release readiness | VERIFIED | Strong component tests, no sufficient production-path deterministic lifecycle test. Not release-ready. |

## Final decision

**Current architecture: repairable with a constrained control-plane redesign.**
Do not run a fourth live diagnostic until M1 and M2 are implemented and the
production-entry-point matrix passes.  No evidence supports increasing retry
counts, increasing the RPC budget, or interpreting the R3 dispatcher failure
as an upstream provider failure.
