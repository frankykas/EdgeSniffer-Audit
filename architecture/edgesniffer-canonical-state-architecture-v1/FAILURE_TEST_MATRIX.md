# Failure-Test Matrix

| Fault | Injection | Required result |
|---|---|---|
| gRPC `lagged` | fixture/controlled receiver signal | seal epoch, receipt, no later decision bridge |
| disconnect | terminate source connection | same; replay only after exact identity/sequence proof |
| ShredStream interruption | stop provisional feed | execution telemetry degrades; canonical research remains independent |
| RabbitStream interruption | stop cross-check | cross-check alert only; no canonical mutation |
| duplicate across feeds | inject same signature/instruction | one canonical identity; provenance records all observations |
| arrival-order disagreement | deliver providers in reverse order | canonical order follows canonical receiver, discrepancy is telemetry |
| producer restart | new instance generation | epoch boundary and receipt |
| consumer restart | restore valid/invalid checkpoint variants | exact restore continues; otherwise fresh epoch/warm-up |
| corrupt checkpoint | mutate checksum | refuse restore, hard wall |
| journal sequence gap | remove record | refuse consumer advance, hard wall |
| corrupt segment | mutate compressed payload/footer | recovery scanner rejects segment |
| disk pressure | quota/fill test | stop before partial segment/package; receipt and no data deletion |
| diagnostic-buffer full | exceed TTL/cap | discard only unreferenced diagnostics; canonical journal continues or fail closes on required evidence limit |
| NFS unavailable | unmount/mocked failure | hot ingest continues; archive publish defers with receipt |
| provider disagreement | contradictory same event payload | contradiction receipt; no silent selection |

All tests must run on fixtures/shadow roots before a controlled deployment. Each must assert both preservation of prior sealed packages and absence of a bridged post-fault decision.
