# EdgeSniffer Shyft and Data-Feed Inventory V1

**Audit mode:** read-only configuration, source, unit, retained-provenance, and
provider-documentation inventory.  No endpoint was contacted, no paid RPC call
was made, and no service, evidence root, cohort, timer, or deployment was
changed.

**Audit timestamp:** 2026-10-14 UTC  
**Scope:** the live EdgeSniffer VPS repository and systemd configuration,
including historical integration code that remains available in the source tree.
Endpoint hosts, paths, API keys, tokens, and private URLs are deliberately
omitted.

## Completion checklist

| Requirement | Status | Evidence / limitation |
|---|---|---|
| Inventory Yellowstone gRPC | VERIFIED | Active V8 unit, its source, environment-variable names, and generated protocol use inspected. |
| Inventory Shyft WebSocket | VERIFIED | `WEBSOCKET_URL` configuration reference and WebSocket adapter references inspected; no active dedicated WebSocket unit was found. |
| Inventory Shyft RabbitStream | VERIFIED | `RABBITSTREAM_URL` / `RABBITSTREAM_TOKEN`, adapter source, and inactive historical unit inspected. |
| Inventory AllenHark ShredStream | VERIFIED | Active proxy and observer units, gRPC wire contract, and observer source inspected. |
| Inventory Jito ShredStream integration | VERIFIED | Configuration, local proxy protocol, Rust sidecar integration, and inactive historical unit inspected. |
| Inventory other configured RPC / archival paths | VERIFIED | Helius, secure JSON-RPC, and kEFi pool-identity RPC configuration/source consumers inspected. |
| Determine current running integrations | VERIFIED | `systemctl` active state, enablement, PIDs, and process start timestamps inspected. |
| Determine complete-block / relevant-transaction / instruction-order usefulness | VERIFIED | Source/protobuf fields and provider documentation checked; limits are explicit below. |
| Determine bounded offline `getBlock` possibility | VERIFIED | Existing Rust source invokes finalized `getBlock`; configured endpoint capability/retention was not exercised. |
| Reveal no credential, private URL, or raw configuration value | VERIFIED | Only variable names, public protocol names, source paths, and unit names appear in this report. |
| Prove every credential currently works | BLOCKED | This would require a connection or billable/provider request, prohibited by this read-only audit. |
| Prove provider-side archival retention or `getBlock` quota | BLOCKED | Configuration and source prove intended use, not provider plan/retention; no RPC call was made. |

## Current service surface

| Integration / unit | State at audit | What this proves |
|---|---|---|
| `edgesniffer-kefi-full-meta-capture-first-v8.service` | **active, enabled** | The current kEfi full-meta source uses the V8 capture-first implementation. |
| `edgesniffer-kefi-full-meta-capture-first-watchdog-v8.service` | **active, enabled** | The V8 source is independently freshness-watched (15-second checks, stale confirmation policy visible in unit arguments). |
| `edgesniffer-allenhark-arrival-observer-v1.service` | **active** | AllenHark entry-arrival observation is live as a separate timing/feed path. |
| `edgesniffer-allenhark-compact-delivery-observer-v1.service` | **active, enabled** | Compact signature/arrival capture from the AllenHark entry stream is live. |
| `shred-proxy.service` | **active, enabled** | A local proxy exposes the entry-stream protocol to observers. |
| `alphabot-yellowstone.service` | inactive, disabled | Historical Yellowstone integration remains installed, not live. |
| `alphabot-rabbitstream.service` | inactive, disabled | Historical RabbitStream integration remains installed, not live. |
| `alphabot-jito-shredstream.service` | inactive, disabled | Historical Jito ShredStream integration remains installed, not live. |
| `edgesniffer-2cq-full-meta-prospective-v1.service` | inactive, disabled | Historical 2CQ full-meta collector exists but is not the current live source. |

The V8 unit started at `2026-10-14 19:40:02 UTC` during this audit window;
the watchdog and AllenHark units predate it. This is an operational observation,
not a continuity certification.

## Provider-by-provider inventory

### 1. Shyft Yellowstone gRPC full-meta

**Configuration and use:** **configured and currently usable in the limited
operational sense that V8 is active.**

- Sanitized configuration references:
  `YELLOWSTONE_FULL_META_ENDPOINT`, `SHYFT_GRPC_ENABLED`,
  `SHYFT_GRPC_ENDPOINT`, `SHYFT_GRPC_URL`, `SHYFT_GRPC_TOKEN`,
  `SHYFT_GRPC_X_TOKEN`, `SHYFT_GRPC_COMMITMENT`, and
  `SHYFT_GRPC_RECONNECT_MS`.
- Live unit: `edgesniffer-kefi-full-meta-capture-first-v8.service`.
- Primary source inspected:
  `rust-exact-ledger/src/bin/yellowstone_full_meta_prospective_lane.rs`.
- Subscription actually requested by that source: filtered **transaction**
  updates, confirmed commitment, account inclusion for the Pump program, Pump
  router, and PumpSwap program. Its request leaves `slots`, `blocks`, and
  `blocks_meta` empty.
- Existing recovery logic uses a bounded slot overlap (`YELLOWSTONE_RECOVERY_OVERLAP_SLOTS`);
  it is a recovery/deduplication mechanism, not a proof of complete historical
  replay beyond the provider horizon.

**Data actually available from the configured full-meta transaction path:**
transaction signature, slot, message/account keys, outer compiled
instructions, metadata/logs, loaded addresses, token balances, and inner
instruction groups when carried by the full transaction update. Those fields
can support exact instruction-path reconstruction only after decoder work
maps event logs/invocations to outer and inner paths.

**Complete block / relevant transaction / ordering assessment:**

| Question | Verdict | Why |
|---|---|---|
| Compact authoritative complete-block inventory | **POTENTIALLY AVAILABLE, NOT YET PROVEN FOR THIS subscription** | Yellowstone block update types contain `slot`, `blockhash`, executed count, transaction entries, transaction signature, and transaction index. The live V8 request does not subscribe to blocks. Shyft documentation says block subscriptions are filter-matched; it does not prove the returned transaction vector is a whole-slot inventory under the intended filter. |
| Relevant-transaction completeness | **NOT PROVEN** | A transaction filter can identify Pump/PumpSwap-relevant transactions, but a filtered feed alone does not prove no relevant transaction was omitted. The two-stage verifier’s provider-conformance gate remains necessary. |
| Authoritative transaction order | **PARTIALLY AVAILABLE** | The block message’s transaction index is the right top-level order source. V8 filtered transaction arrival order is not canonical order. Same-transaction outer/inner ordering still needs decoder path attribution. |

**Documentation checked:** Shyft’s [Yellowstone Blocks guide](https://docs.shyft.to/solana-yellowstone-grpc/docs/blocks) and [transaction-subscription guide](https://docs.shyft.to/solana-yellowstone-grpc/docs/transaction-subscription) describe blocks, transaction filters, and account-include filtering. The public docs do not establish the required filtered-block completeness guarantee.

### 2. Shyft WebSocket / standard Solana PubSub

**Configuration and use:** **configured reference; live use not proven.**

- Sanitized configuration reference: `WEBSOCKET_URL`.
- Source references occur in the TypeScript execution/confirmation path; no
  dedicated active EdgeSniffer WebSocket collector unit was found in this
  inventory.
- Shyft documents standard PubSub-style subscriptions including account,
  program, logs, signature, slot, and block subscriptions.

**Data actually provided:** depending on subscription, updates and block
objects similar to standard Solana JSON-RPC / `getBlock` representations.
It is useful for notifications, signatures, logs, slots, and potentially full
block payloads.

**Completeness / ordering value:**

- **Complete-block inventory:** **unverified.** A `blockSubscribe` payload can
  contain a block-shaped object, but this audit found no deployed bounded
  verifier or provider guarantee that a filtered WebSocket subscription gives
  all relevant transactions in a slot.
- **Relevant-transaction completeness:** **not proven** for a filter alone.
- **Instruction ordering:** outer/inner transaction content may be available in
  full block/transaction details, but WebSocket arrival time is not authority
  for same-slot causal ordering.

**Documentation checked:** Shyft’s [WebSocket overview](https://docs.shyft.to/solana-rpc/websocket) and [block subscription guide](https://docs.shyft.to/solana-rpc/websocket/blocksubscribe). Exact configured endpoint behavior, enabled methods, and retention remain unverified because no connection was made.

### 3. Shyft RabbitStream

**Configuration and use:** **configured in code/environment; not currently
running; credentials/path usability not exercised.**

- Sanitized variables: `RABBITSTREAM_URL`, `RABBITSTREAM_TOKEN`,
  `SHYFT_RABBITSTREAM_ENABLED`, `SHYFT_RABBITSTREAM_URL`.
- TypeScript adapter: `src/lib/streamTriggers.ts` recognizes `rabbitstream`
  as a Yellowstone-compatible trigger provider.
- Historical service: `alphabot-rabbitstream.service`, currently inactive and
  disabled.

**Data actually provided:** Shyft describes RabbitStream as a pre-execution
raw-shred / transaction trigger feed with Yellowstone-compatible transaction
filters. It is valuable for low-latency arrival visibility but, by design,
does not supply execution metadata, logs, inner instruction results, final
success/failure, or final block order.

**Completeness / ordering value:**

- **Complete block:** **NO.** It is not a final complete-block inventory.
- **Relevant-transaction completeness:** **NO scientific proof.** It can be a
  latency witness, but lost/recovered shreds and pre-execution status prevent
  it from proving final relevant-set completeness.
- **Authoritative instruction order:** **NO.** It has no final execution meta;
  source code already records that RabbitStream observations have no
  success/failure authority and require finalized reconciliation.

**Documentation checked:** Shyft’s [RabbitStream overview](https://docs.shyft.to/rabbitstream) and its [Yellowstone/RabbitStream comparison](https://docs.shyft.to/rabbitstream/compare-rabbitstream-yellowstone). No live connection was attempted.

### 4. AllenHark entry/ShredStream integration

**Configuration and use:** **configured and currently operational as an
arrival-observation feed, but not a canonical scientific source.**

- Sanitized variables observed in source/unit environment:
  `ALLENHARK_SHREDSTREAM_GRPC_URL`, `ALLENHARK_RECONNECT_MS`, plus
  output/health/continuity path variables for the observers.
- Active consumers:
  `edgesniffer-allenhark-arrival-observer-v1.service` and
  `edgesniffer-allenhark-compact-delivery-observer-v1.service`.
- Observers call
  `/shredstream.ShredstreamProxy/SubscribeEntries` and receive only
  `{ slot, entries: bytes }`.
- They deserialize Solana entries locally and emit compact signature/receipt
  records with a source label explicitly stating that observed slot ordinals
  are **not canonical**.

**Data actually provided:** raw serialized entry contents, slots, observed
receipt time, signatures and local decoded transactions where reconstruction
succeeds. It can establish fast arrival evidence and a separate availability
witness.

**Completeness / ordering value:**

- **Complete block / relevant-set completeness:** **NO.** Entries are a live
  transport feed; the audited contract contains no finalized whole-block
  inventory or successful-execution proof.
- **Authoritative instruction ordering:** **NO.** It has raw entries and
  observed order, not final block transaction index or final inner-instruction
  execution metadata. The source itself labels its slot ordinal
  non-canonical.

**Evidence checked:** observer source
`rust-exact-ledger/src/bin/allenhark_{arrival_observer,compact_delivery_observer}.rs`,
the local `shredstream.proto`, active systemd units, and local health/continuity
output contracts. The upstream provider’s delivery/completeness SLA remains
unverified.

### 5. Jito ShredStream / local shred proxy

**Configuration and use:** **configured; local proxy active; historical
direct service inactive; provider-level live subscription was not exercised.**

- Sanitized variables: `JITO_SHREDSTREAM_ENABLED`,
  `JITO_SHREDSTREAM_ENDPOINT`, `JITO_SHREDSTREAM_GRPC_URL`,
  `JITO_SHREDSTREAM_RAW_UDP_PORT`,
  `JITO_SHREDSTREAM_DECODED_GRPC_ENABLED`, and
  `JITO_SHREDSTREAM_RUST_SIDECAR_ENABLED`.
- `shred-proxy.service` is active. The TypeScript stream adapter can invoke the
  Rust sidecar for Jito entry handling; `alphabot-jito-shredstream.service` is
  inactive and disabled.
- The local proxy protocol returns `Entry { slot, entries }`, matching the
  AllenHark observer’s raw-entry consumption model.

**Data actually provided:** pre-execution raw ledger entries/shreds. The Rust
sidecar deserializes `Vec<SolanaEntry>` and derives local swap triggers.

**Completeness / ordering value:**

- **Complete block:** **NO.** An entry stream is not final block inventory.
- **Relevant transaction completeness:** **NO proof.** Duplicate/recovered
  shreds and feed loss are possible; the source has deduplication paths but
  that does not establish omission-free coverage.
- **Authoritative instruction order:** **NO final authority.** Useful for
  arrival/latency, not for finalized causal state.

**Documentation checked:** Jito’s [ShredStream overview](https://docs.jito.wtf/lowlatencytxnfeed/) and public entries protocol documentation. Jito’s documentation includes a sunset notice for its public ShredStream offering; this audit does not infer current availability from that notice. The active local proxy only proves a local process is running.

### 6. Helius JSON-RPC / hydration path

**Configuration and use:** **configured; source uses it as a hydration and
fallback polling path; active service binding not established by this audit.**

- Sanitized variables: `HELIUS_API_KEY`, `HELIUS_API_KEYS`, and
  `HELIUS_KEY_COOLDOWN_MS`.
- `src/lib/infraStatus.ts` labels it `helius_polling` for parsed transaction
  hydration, fallback polling, and research source. `streamTriggers.ts`
  describes hybrid operation as stream signature trigger followed by Helius
  hydration.
- The source calls standard transaction retrieval (`getTransaction`) in
  several paths. It does not establish a currently-enabled archival plan.

**Completeness / ordering value:**

- **Complete block / historical cross-check:** **CONDITIONALLY USEFUL.** A
  standard JSON-RPC provider that permits finalized `getBlock` can provide an
  offline block inventory. Helius credentials are configured, but this audit
  did not make a request and therefore cannot verify method availability,
  historical retention, block-size limits, rate limits, or plan entitlement.
- **Relevant-set completeness:** only after obtaining a complete finalized
  block response and locally filtering it. Polling/parsed transaction
  hydration alone is not an omission proof.
- **Instruction order:** finalized transaction/block data can supply outer
  and inner instruction structures; exact semantic event attribution still
  requires the decoder enhancement already documented in the canonical work.

### 7. Secure/general Solana JSON-RPC

**Configuration and use:** **configured; existing code invokes it for
finalized transaction/block reconciliation; endpoint/provider capability not
live-tested by this audit.**

- Sanitized variables: `SECURE_RPC_URL` and, in compatibility fallbacks,
  `RPC_URL`, `HELIUS_RPC_URL`, and `SOLANA_RPC_URL` references.
- `rust-exact-ledger/src/main.rs` constructs finality requests for
  `getTransaction` and **finalized `getBlock`**. Its `getBlock` request uses
  `transactionDetails: "signatures"`, `rewards: false`, and transaction
  version support; it caches slot responses for bounded reconciliation.

**Bounded offline `getBlock` verdict:** **YES, implementation support exists;
provider capability remains unverified.** This is the strongest already-built
candidate for a bounded offline completeness cross-check: fetch a finalized
slot inventory, compare signatures with the canonical proof/filter result,
and fail closed on absent or conflicting responses. It is not suitable as a
real-time primary ingestion path without measured latency, availability, and
quota evidence.

**Complete-block / ordering value:** A successful finalized full-block result
is authoritative for transaction list/order at the block level, subject to
provider retention and response completeness. The current code’s
`transactionDetails: "signatures"` setting proves only signature inventory,
not inner instruction paths; an offline verifier requiring instruction paths
must request full transaction detail or reconcile selected transactions with
`getTransaction`.

### 8. kEFi Pool-identity JSON-RPC

**Configuration and use:** **configured and consumed by the kEfi Pool identity
resolver; historical/block capability unverified.**

- Sanitized variable: `KEFI_POOL_IDENTITY_RPC_URL`.
- Consumers: `rust-sidecar/src/bin/kefi_teacher_independent_pool_state_feed_v1.rs`
  and the offline canonical canary source.
- Current purpose: authoritative PumpSwap Pool account lookup / identity
  validation, not a broad historical feed.

**Data actually provided:** account reads sufficient for Pool identity
metadata when the provider returns the canonical account. No evidence shows
that this endpoint is intended to provide archival blocks or broad historical
transaction enumeration.

**Completeness / ordering value:** it helps establish **pool identity**, not
complete-block inventory, relevant-set completeness, or transaction order.
A standard RPC may expose `getBlock`, but this is **UNVERIFIED** without a
separate bounded capability probe.

## What can solve which canonical-state problem

| Need | Existing path | Current conclusion |
|---|---|---|
| Low-latency arrival witness | RabbitStream, Jito/AllenHark entry stream | Available in principle; non-authoritative. |
| Final full transaction metadata | Shyft Yellowstone full-meta; finalized JSON-RPC transaction lookup | Available/implemented for targeted transactions. |
| Exact top-level transaction order in a slot | Yellowstone block update transaction index; finalized `getBlock` | Structurally available, but live Shyft filtered-block completeness must be proven; RPC is the practical bounded offline cross-check. |
| Exact outer/inner instruction path | Full-meta / full RPC transaction data | Data fields exist; EdgeSniffer decoder still needs unambiguous event-to-path attribution and must fail closed on ambiguity. |
| Complete relevant transaction set | Complete finalized block inventory followed by local program/account filtering | Not proven by any active filtered stream alone. |
| Scientific primary source | V8 Yellowstone full-meta | Current source, but its transaction filter alone does not establish full-block completeness. |

## Recommended bounded offline completeness cross-check

No live configuration change is recommended by this inventory. The smallest
existing mechanism to test is already present in the Rust reconciliation code:

1. for a deliberately bounded retained slot interval, request finalized
   `getBlock` through the configured secure/general RPC path;
2. retain only the slot identity, ordered signatures, and a hash of the
   response/proof under a quota;
3. locally identify all Pump/PumpSwap-relevant transactions using transaction
   program/account data;
4. compare that set against the bounded canonical/Yellowstone candidate set;
5. fail closed on an unavailable block, conflicting block identity,
   missing/extra relevant transaction, or unsupported instruction-path
   attribution.

This is an **offline verification candidate**, not approval to use RPC as the
real-time producer and not proof that any provider will serve the requested
historical range.

## Remaining blockers and unverified evidence

1. **Shyft filtered block semantics:** public documentation does not prove
   whether the block transaction list under a filter is a complete slot
   inventory or only matching transactions. Provider confirmation or a bounded
   paid/conformance probe is required.
2. **RPC archival capability:** the code has `getBlock`, but endpoint plan,
   retention, rate/size limits, and full-detail availability are unverified.
3. **Exact semantic instruction paths:** full-meta has the transaction data,
   but the existing event decoder’s log-index association is not yet a proof of
   outer/inner instruction path for every event.
4. **Entry feeds:** AllenHark/Jito/RabbitStream can corroborate arrival but
   cannot replace finalized complete-block proof.

## Evidence inspected

- VPS systemd unit state/configuration for the units listed above (environment
  values redacted at collection time).
- VPS source:
  `rust-exact-ledger/src/bin/yellowstone_full_meta_prospective_lane.rs`,
  `rust-exact-ledger/src/main.rs`,
  `rust-exact-ledger/src/bin/allenhark_*`,
  `rust-sidecar/protos/shredstream.proto`, `rust-sidecar/src/main.rs`,
  `rust-sidecar/src/kefi_pumpswap_semantics_v1.rs`,
  `src/lib/streamTriggers.ts`, `src/lib/infraStatus.ts`, and `src/config.ts`.
- Sanitized environment-variable names only; no environment values were read
  into this report.
- Public provider documentation linked in the applicable sections.

## Final verdict

**INVENTORY COMPLETE WITH EXPLICIT PROVIDER-CAPABILITY BLOCKERS.**

EdgeSniffer has several feed integrations, but only V8 Yellowstone is the
current full-meta scientific source. The existing JSON-RPC reconciliation code
is the only already-implemented path that can plausibly provide a bounded
offline finalized `getBlock` signature-inventory cross-check. No active or
historical feed observed here independently proves complete relevant
transaction coverage plus authoritative instruction ordering by itself.
