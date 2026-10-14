# Migration and Rollback Plan

| Phase | Deployable unit | Success gate | Rollback |
|---|---|---|---|
| offline | no live unit | deterministic tests/equivalence pass | discard build outputs; existing services untouched |
| shadow receiver | new isolated unit/root | bounded storage, contiguous journal, no impact to V8/2CQ | stop shadow, retain its diagnostics/receipt |
| shadow state | canonical consumer with no snapshot admission | identical state/candidate hashes on defined interval | stop consumer; journal remains immutable |
| failure injection | isolated fixtures/shadow | each fault creates correct hard wall/recovery result | discard shadow epoch; no production source replacement |
| controlled cutover | maintenance-window service swap | health, sequence, checkpoint restore and state comparison pass | restore old unit only as operational feed; new epoch is sealed, not bridged |
| soak | canonical live with admission disabled | defined duration/resource/failure criteria | stop admission; preserve journal and receipts |

Rollback never treats old and new epochs as one scientific stream. No phase modifies frozen models, V6 evidence, PRE_H3 or 2CQ evidence. NFS outage is nonfatal to hot ingestion; it blocks archival publication only.

## Required fault behaviors

Lag/disconnect/queue overflow/producer restart/consumer restart/checkpoint corruption/journal gap/corrupt segment/disk pressure/full diagnostics/source disagreement each must: stop decision admission; seal the epoch; persist a failure receipt; preserve previously sealed packages; and require a new epoch or exact proved replay. Duplicate provider arrivals may be deduplicated only by canonical identity; conflicting payloads fail closed.
