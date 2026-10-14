# Canonical Live State Engine

One incremental engine consumes only canonical semantic records in exact sequence. It owns the following maps:

| Existing observer structure | Canonical state representation | Recovery rule |
|---|---|---|
| `PoolIdentity` cache | immutable `pool -> identity/provenance` registry | checkpointable; conflict is epoch failure |
| `PoolState.first_ns/first_id` | first-seen sequence/time/identity | checkpointable only with canonical history proof |
| reserves and fee tuple | latest transition state keyed by pool | checkpointable |
| `prior_pre_pool_quote`, `last_buy_ns` | prior-buy state keyed by pool | checkpointable |
| `VecDeque<Flow>` | bounded 60-second per-pool semantic event deque | checkpointable including all unexpired events |
| candidate filter | explicit eligibility state + exclusion code | checkpointable |
| observed-wallet ledger | append-only wallet identity ledger with tail hash | durable separate file, transactionally bound to journal seq |
| observer `sequence` | canonical sequence only | never per-consumer counter |

The engine applies each event once. It exposes read-only `state_at(sequence)` only at committed boundaries. A decision at sequence `N` reads candidate state after every **causally earlier** resolved event and before the wallet-action instruction; the teacher action is then applied separately. If same-slot state cannot be causally ordered, `state_at` returns invalid for scientific admission rather than using arrival order. It also returns `UNIVERSE_INCOMPLETE` after a hard gap until the recovery proof described in `CANDIDATE_UNIVERSE_RECOVERY.md` is satisfied.

No model is inside this engine. It produces frozen semantic state and feature vectors only after the candidate universe has been certified complete for that boundary.
