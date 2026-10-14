# Component Reuse Matrix

| Current component | Source path | Keep / modify / replace | Required change |
|---|---|---|---|
| V8 Yellowstone receive/decode path | `rust-exact-ledger/src/bin/yellowstone_full_meta_prospective_lane.rs` | modify | become the one canonical receiver; emit durable semantic records, not only a RAM ring/promotion flow |
| ledger durability primitives | `rust-exact-ledger/src/lib.rs`, `src/reliability.rs` | keep and extend | segment footer, hash-chain manifest, atomic recovery scanner and epoch receipts |
| PumpSwap observer | `rust-sidecar/src/bin/kefi_teacher_independent_pool_state_feed_v1.rs` | split/modify | remove direct Yellowstone ownership; consume canonical semantic reader only |
| candidate source reader prototype | `rust-sidecar/src/kefi_canonical_source_v1.rs` | keep, complete | production segment validation, checkpoint/restart and consumer lag enforcement |
| Pool resolver | same observer plus Pool fixtures | keep unchanged semantically | move behind canonical identity registry/cache API |
| wallet ledger | same observer | keep, transactionally bind | associate ledger writes with canonical sequence and checkpoint tail |
| ShredStream fast path | `rust-sidecar/src/main.rs`, `protos/shredstream.proto` | keep isolated | emit provisional observation envelope only; no research mutation |
| execution engine | Rust sidecar / existing Rust executor | keep isolated | consume state read model, never wait on research persistence |
| receipts/manifests | ledger/observer helpers | keep | standardize journal/checkpoint/decision/failure manifest fields |
| archive/capacity tooling | existing service/timer scripts | modify later | operate on compact segments/packages by explicit retention class |

No frozen model, historical receipt, V6 evidence package, PRE_H3 contract, or 2CQ scientific definition should be rewritten as part of this migration.
