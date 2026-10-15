# Shyft Complete-Block Inventory Conformance V1

**Base audit:** `fccbe7a288107d6d5c2416ff1ffd7eaa44a85ce8`  
**Mode:** isolated, non-scientific provider conformance probe.  
**Decision:** `PROVIDER_CAPABILITY_REJECTED` for unrestricted
`SubscribeUpdateBlock` as the Canonical State v1 Stage-1 inventory source.

The provider accepted an empty `account_include` block filter. However, its
delivered transaction inventory did not match the same slot's finalized
`getBlock` inventory despite the blockhashes matching. Therefore it cannot be
used as a complete canonical slot inventory.

No live collector, V8 service, observer, cohort, AOM input, decoder, or
trading service was modified or restarted.

## Completion Checklist

| # | Requirement | Status | Evidence |
|---:|---|---|---|
| 1 | Verify upstream block-filter behavior and record revision | VERIFIED | Yellowstone upstream `93db7c58a0b559fa53abf10b99be9d026e3f04e3`, exact behavior below. |
| 1a | Account-filtered block transactions are filtered | VERIFIED | Upstream `filter.rs` filters `message.transactions` with `matches_any_in_set` before constructing a block update. |
| 1b | Empty `account_include` is permitted upstream when allowed | VERIFIED | Upstream calls `FilterLimits::check_any(filter.account_include.is_empty(), limits.account_include_any)`; provider configuration may reject it. |
| 1c | Block messages retain executed count and transaction indexes | VERIFIED | Installed protobuf and source expose `executed_transaction_count`, transaction `signature`, and `index`; the probe checks contiguous indexes. |
| 2 | Try `account_include=[]`, transactions enabled, account/entry disabled | VERIFIED | Shyft accepted the request at finalized commitment and delivered block updates. |
| 2a | Compare accepted blocks against independent finalized `getBlock` | VERIFIED / FAILED_CONFORMANCE | Same-slot same-blockhash comparison produced an inventory count and identity mismatch. Probe stopped fail-closed after the first comparable slot. |
| 2b | Do not confuse delivered blocks with consecutive Solana slots | VERIFIED | The probe collects distinct observed slots only; it does not claim they are consecutive. No interval was admitted. |
| 3 | Inspect RAM, CPU, and disk before testing | VERIFIED | Preflight and post-run measurements below; test used an 8 GiB transient-scope RSS cap and dedicated hot volume. |
| 3a | Respect subscription, RPC, output, and runtime caps | VERIFIED | Two total finalized `getBlock` calls, under 10 seconds combined live subscription time, 64 MiB compact-output cap, 200 GiB protected reserve, and no raw spool. |
| 3b | Avoid live-service interruption | VERIFIED | Post-run collector, watchdog, and evidence-ledger units remained `active`; no probe process remained. |
| 4 | State provider answer, resource measurements, fallback, and canary readiness | VERIFIED | Results and fail-closed decision below. |
| 5 | Add only minimal probe/tests and publish report | VERIFIED | One isolated probe source plus two deterministic comparison regressions; this report is the only audit documentation addition. |
| 6 | Audit original assignment line by line | VERIFIED | No collector implementation, decoder rebuild, AOM change, sustained capture, or scientific admission was performed. |

## 1. Upstream Verification

The source revision used was Yellowstone `master` at:

`93db7c58a0b559fa53abf10b99be9d026e3f04e3`

Source: [`filter.rs` at the verified revision](https://github.com/rpcpool/yellowstone-grpc/blob/93db7c58a0b559fa53abf10b99be9d026e3f04e3/yellowstone-grpc-geyser/src/plugin/filter/filter.rs).

| Claim | Verified behavior |
|---|---|
| Account-filtered block contents | `FilterBlocks::get_updates` retains only transactions for which `inner.matches_any_in_set(&tx.transaction.account_keys)` is true. Thus a nonempty `account_include` changes the `transactions[]` vector itself; it is not merely a block-selection predicate. |
| Empty account filter | The parser calls `FilterLimits::check_any(filter.account_include.is_empty(), limits.account_include_any)`. Generic Yellowstone permits an empty vector only if the operator's limits allow it. |
| Provider restriction | `account_include_any=false` makes the empty request invalid, even though upstream supports the request shape. |
| Transaction identity/order material | The block payload carries `executed_transaction_count`; retained transactions carry signature and original `index`. Exact canonical interpretation still requires finalized reconciliation. |

This validates the prior architecture concern: an account-filtered block cannot
be called a complete slot inventory. It also made the empty-filter probe the
correct narrow provider experiment.

## 2. Isolated Probe Contract

The added source is:

`sanitized-code/canonical-state-v1/shyft_block_inventory_conformance_v1.rs`

It requests exactly:

```text
blocks["unfilteredInventory"]:
  account_include = []
  include_transactions = true
  include_accounts = false
  include_entries = false
commitment = finalized
```

It then retains compact hashes only: blockhash hash, transaction-index/signature
hash pairs, count, and the comparison result. It performs no Pump/PumpSwap
decoding, Pool RPC, state mutation, wallet lookup, raw-block persistence, or
scientific decision admission.

### Bounds used

| Control | Value |
|---|---:|
| Requested block updates | At most 8 per run; no claim of consecutive slots |
| Total finalized `getBlock` calls across all live attempts | 2 / 8 maximum |
| Total subscription wall time | Under 6 seconds / 10-minute maximum |
| gRPC decoded-message ceiling | 256 MiB |
| Transient process RSS cap | 8 GiB |
| CPU scope | 200% CPU quota |
| Compact output cap | 64 MiB (stricter than the authorized 4 GiB maximum) |
| Hot-volume protected reserve | 200 GiB |
| Raw block / transaction spool | None |

Before the probe, the server had 32 GiB RAM, 12 CPU cores, about 20.4 GiB
available memory, and about 210.8 GiB free on
`/mnt/edgesniffer-kefi-hot`. The root filesystem was deliberately not used.

The first local preflight correctly refused a hypothetical 4 GiB output budget
because it would have breached the 200 GiB reserve. The probe was tightened to
a 64 MiB compact-report cap before it opened a connection. A separate initial
binary run exposed a missing Rustls provider initialization before TLS; it made
no successful provider request. The probe then installed the same `ring`
provider used by the existing canary and was rebuilt with an already-installed
Rust 1.99 toolchain. No system toolchain was installed or changed.

## 3. Actual Shyft Result

### Acceptance result

**Yes, Shyft accepted unrestricted block contents at the request level.**

The first successful connection with empty `account_include` received eight
finalized block updates. The first finalized RPC response was spent on a local
parser issue (`base64` was incompatible with a signature-path parser); that
attempt made no inventory conclusion. The parser was corrected to use JSON
full transaction details and its deterministic tests were rerun.

The subsequent bounded attempt received seven distinct finalized block updates
and performed one finalized `getBlock` comparison before stopping on the first
mismatch:

| Evidence | Observed value |
|---|---:|
| Finalized slot | `457159355` |
| Blockhash | Same SHA-256 on Yellowstone and finalized RPC: `865d6c14752c6b22f89208c83dbe6867cc76cd6f3710f9528b444ff62846b991` |
| Yellowstone `executed_transaction_count` / vector length | 1,002 |
| Finalized `getBlock(..., transactionDetails=full)` transaction count | 336 |
| Exact identity/index digest | Different |
| Disposition | `TRANSACTION_COUNT_MISMATCH`; fail closed |

The equal blockhash rules out comparing unrelated slot forks in this instance.
The factor-of-three count difference has **not** been attributed to a cause:
the test establishes a provider/output mismatch, not why Shyft produced it.
No subsequent slot was used to dilute or work around the failed proof.

### Measured resource use

| Measurement | First accepted connection | Mismatch comparison run |
|---|---:|---:|
| Largest decoded `SubscribeUpdate` protobuf | 2,441,889 bytes | 2,066,868 bytes |
| Peak process RSS | 43,487,232 bytes | 35,364,864 bytes |
| Wall time | 2,968 ms | 2,653 ms |
| Process CPU | 53 ticks (~0.53 CPU seconds at 100 Hz) | 43 ticks (~0.43 CPU seconds at 100 Hz) |
| Compact durable artifact | none on parser-only run | 1,347 bytes |

The protobuf encoded length is an application-level decoded-message size, not
a claim of exact compressed wire-network bytes. No raw network payload was
retained. The mismatch-run report SHA-256 is:

`9a462d159a48cabe96a14a07757a56e8f68d2b2409faf54aae11f03d37c286a4`

Its invocation receipt SHA-256 is:

`cded988de8480cef0785058206b9402b417c06fdefa05c1a77dd09541516d0de`

Both remain on the protected hot volume as non-scientific provider-conformance
evidence. They contain only public slot numbers and hashes, no credentials.

### Service safety verification

After the probe:

```text
edgesniffer-kefi-full-meta-capture-first-v8.service            active
edgesniffer-kefi-full-meta-capture-first-watchdog-v8.service   active
pump-exact-evidence-ledger-v1.service                          active
```

No conformance process remained. Hot-volume free space remained approximately
203.3 GiB, above the 200 GiB protected reserve.

## 4. Answers to the Requested Questions

### 1. Does Shyft accept unfiltered block subscriptions?

**YES at request acceptance.** An empty `account_include` request with full
transactions was accepted and produced finalized block updates.

**NO as a proven complete-inventory capability.** The delivered inventory
failed exact finalized-RPC reconciliation for a same-hash block.

### 2. Can we independently verify complete transaction inventories?

**YES for the finalized RPC reference; NO for the unrestricted Yellowstone
block feed in this conformance run.** `getBlock(finalized, full)` produced an
independent ordered 336-transaction inventory. Yellowstone produced 1,002
indexed entries for that same blockhash. The required equality condition is
false.

### 3. What were actual RAM, CPU, message-size, and network requirements?

The small observed sample was lightweight: at most 43.5 MiB RSS and 2.44 MiB
decoded protobuf per block, far below the configured 8 GiB/256 MiB ceilings.
CPU was below 0.53 seconds for each sub-three-second attempt. Exact compressed
wire bytes were intentionally not measured or persisted; decoded protobuf
sizes are reported instead. These measurements show only this failed sample,
not a sustained capacity guarantee.

### 4. If unrestricted blocks are unavailable, is finalized-RPC reconciliation viable?

**Conditionally yes, as a fail-closed verification architecture.** The earlier
bounded RPC conformance test already demonstrated finalized `getBlock` support.
The present failure also proves that exact reconciliation catches a bad
Yellowstone inventory rather than silently promoting it.

The viable fallback remains:

```text
filtered Yellowstone identity manifest
  + finalized full getBlock per captured slot
  + derive relevant set from message/account/instruction evidence
  + exact equality required before a slot is scientifically usable
```

It is not a low-latency inventory feed. It remains contingent on confirmed
request-credit/rate limits, a bounded finalized-RPC dispatcher, a live compact
filtered identity manifest, and exact instruction-path binding. A gap,
unavailable block, unsupported transaction version, duplicate, mismatch, or
backlog must invalidate the slot/interval.

### 5. Is existing Canonical State v1 Stage-1 capture ready for a separately authorized bounded canary?

**NO.** The intended unrestricted-block Stage-1 source has been rejected by
the first exact same-hash comparison. Do not start a Stage-1 canary on it.

Remaining prerequisites for a separately authorized fallback canary are:

1. obtain Shyft's explanation for the 1,002-versus-336 same-block mismatch;
2. freeze provider billing/rate limits for full finalized `getBlock` at the
required per-slot rate;
3. integrate the already designed compact filtered-identity manifest with the
real source, before candidate/wallet filtering;
4. implement and test the bounded finalization/reconciliation dispatcher;
5. wire the approved instruction-path decoder into the live adapter;
6. establish the separate complete eligible-pool-universe bootstrap and
coverage-ledger conditions.

None of these is solved by acceptance of an empty block filter.

## Test Results

```text
cargo test --locked --bin shyft_block_inventory_conformance_v1
2 passed; 0 failed
```

The regression suite proves that equal transaction counts cannot hide a
signature mismatch and that equal signatures in different canonical index
positions also fail. It prevents the exact class of false-positive conformance
claim this gate was designed to avoid.

## Exact Next Action

Send Shyft support the same-hash evidence above and ask why an empty-filter
finalized block update reports 1,002 indexed transactions where finalized
`getBlock` reports 336. Keep the current Canonical State v1 Stage-1 capture
blocked. In parallel, only the existing filtered-Yellowstone plus finalized
RPC reconciliation fallback may be developed further, under a separate
authorization; no new collector is authorized by this report.
