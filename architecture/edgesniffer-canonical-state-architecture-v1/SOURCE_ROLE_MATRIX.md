# Source Role Matrix

| Source | Existing evidence in EdgeSniffer | Data/ordering/recovery reality | Recommended role | Not permitted to do |
|---|---|---|---|---|
| Shyft gRPC Yellowstone full-meta | active V8 Rust collector; direct observer used it; `fromSlot` configuration and full PumpSwap transaction decode exist | complete transaction/meta input currently proven for kEFi; stream can silently stall or issue `lagged`; replay must be proven contiguous, not assumed | **A primary canonical source**, **C recovery/replay** | direct scientific consumers may not each subscribe independently |
| AllenHark ShredStream | Rust sidecar proto/client and active 2CQ observer integration; slot/entry stream, reconnect/dedupe logic | earliest observed entries; arrival order is provider/provisional, not confirmed canonical order; no demonstrated full kEFi research replay ledger | **B low-latency execution source**, **D cross-check** | create research sequence, candidate universe, or permanent evidence |
| Shyft RabbitStream | provider enum/config path exists; ledger code explicitly treats observations as non-finality authority | currently no demonstrated live kEFi canonical deployment, no validated kEFi replay/ordering contract in retained evidence | **D cross-check/validation** and **E auxiliary** until benchmarked | canonical science or finality authority |
| Shyft WebSockets | auxiliary WebSocket handling exists in TypeScript execution code; no proven kEFi semantic journal integration | account/subscription hints can be useful; no demonstrated ordered replayable transaction completeness for this purpose | **E auxiliary**; optional bootstrap/account cross-check | sole source of pool universe, wallet action, or recovery |

## Required feed qualification before role expansion

For every feed, record connection instance, subscription request, event identity coverage, duplicate rate, ordering divergence versus canonical gRPC, reconnect semantics, replay range, account/transaction completeness, and observed lag. Absence of a result keeps the source in its conservative role.

## Source identity

`canonical_source_instance_id = provider + endpoint-class + process-generation UUID + subscription-contract hash`. It changes on any producer process replacement or subscription-contract change. `epoch_id` is monotonic within one append-only registry and changes after any unproven continuity condition.
