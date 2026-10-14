# Execution Fast Path

ShredStream remains the lowest-latency observation path already integrated in the Rust sidecar. It may produce a provisional detection, fill a latency telemetry record, and drive execution/risk only under the existing Rust-only posture. It must not mutate the canonical research journal or candidate universe.

```text
ShredStream entry -> provisional parsed observation -> existing risk/fingerprint/execution path
                                                | signature/slot/event identity
                                                v
canonical gRPC semantic record -> confirm, annotate, or reject provisional observation
```

If canonical confirmation disagrees or never arrives, record execution telemetry as provisional anomaly. Do not backfill a research observation from it. RabbitStream/WebSockets may provide independent arrival telemetry or account hints, but likewise never create a second state history.

No HTTP/RPC lookup is added to the buy/sell hot path. Pool-account resolution for science stays on the canonical state path, cached and rate-bounded.
