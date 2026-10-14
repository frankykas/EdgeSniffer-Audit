# Bounded Storage Policy

| Class | Placement | Lifetime / quota | Compaction / failure behavior |
|---|---|---|---|
| root filesystem | code, unit files, small logs only | no research bulk data; hard operational reserve | refuse large write paths; alert before reserve breach |
| kEFi hot `/mnt/edgesniffer-kefi-hot` | active journal segments, checkpoints, bounded raw diagnostics, current decision packages | journal rolling quota; raw diagnostics fixed TTL/cap; monitor per-class quota | seal segment before quota; stop new decisions fail-closed before partial write |
| research `/mnt/HC.../edgesniffer-research` | immutable sealed cohorts, compact decision packages, equivalence fixtures | explicit per-project quota, no live spool | archive only verified sealed immutable objects |
| NFS expansion | verified cold archive / redundant receipts | archive quota and manifest catalogue | NFS unavailable never blocks hot canonical ordering; archive retry only |

Retention rules: unreferenced raw diagnostics expire automatically by TTL after checksum/index capture; global semantic segments compact after all referenced decision packages have retained their reconstruction slice and no recovery window requires them; immutable decision evidence, manifests, failure receipts and frozen artifacts remain. Derived matrices and caches are rebuildable and are the first class eligible for release.

Capacity guard reports each class separately. It must stop new scientific decision admission before a complete package cannot be written; it may not delete, overwrite, or truncate evidence to survive.
