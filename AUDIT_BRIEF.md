# Independent kEfi Research and Storage Audit

## Requested Verdict

Determine whether the current kEfi research program has a defensible path from captured evidence to a pre-entry paper-trading fingerprint, and prescribe a durable storage/evidence architecture that prevents another root-volume incident.

This is a methodology and systems audit, not a request to trade live.

## Claims That Must Stay Separate

1. **Selection imitation:** distinguishing events kEfi buys from matched controls.
2. **Pre-entry prediction:** doing so using only evidence observable before kEfi's own buy.
3. **Paper economics:** a frozen entry, fee/slippage, and exit contract beating comparable controls on fresh evidence.
4. **Execution parity:** reproducing paper assumptions in a real execution path.
5. **Live profitability:** not established by any current result.

## Historical Corpus

- 1,656 kEfi-selected entries.
- 8,052 deterministic matched controls.
- 3,636,007 reconstructed candidate events.
- 150 distinct selected mints/pools.
- Closed source partitions: 2026-10-05 03:00 through 12:00 UTC.
- Canonical order: `received_at_unix_ns, slot, signature, event_index`.
- Strict pre-entry state: target wallet transactions are excluded from their own candidate snapshots.
- Available features include pool reserves/age, fee fields, prior buy counts and quote flow at 5/20/60 seconds, and timing provenance.

## Historical Selection Result

The richer chronological pre-entry model reported holdout AUC about 0.889. An earlier simpler temporal model reported unseen-time AUC about 0.960. These are selection-imitation results only. They do not establish tradeability, profitability, or execution readiness.

## Fresh H1 Paper Result: Rejected

H1 was frozen and scored once on fresh data using a fixed paper entry, fee/slippage, and exit contract.

| Metric | H1 matches | Comparable controls |
| --- | ---: | ---: |
| Entries | 5,174 | 5,174 |
| PnL | -13.3939 SOL | -6.2433 SOL |
| Profit factor | 0.1548 | 0.4066 |
| Win rate | 9.74% | 10.96% |

H1 did not fail because of one outlier: its top-one gain share was about 6.8% and top-five about 21%. The exact frozen H1 rule is rejected. No post-hoc modification may reuse the H1 fresh sample as validation.

## H2 Status

H2 was proposed as a controlled ablation that removed H1's sparse-crowd overlay while retaining the historical pre-entry score. It had selection support but no independent economic support. Its evaluator was disabled before a one-shot economic result was created. Treat H2 as quarantined, not validated.

## Current Capture Plane

- Wallet: `kEFiAX3jo5NmemysQov342TZ9mGh6yp92GDRjhA8XDf`
- Feed: Shyft Yellowstone full-meta, Frankfurt endpoint.
- Capture-first mode with 120-second promoted shared windows.
- Recent healthy state after a controlled storage-routing restart: zero reconnects, zero receive errors, zero reported continuity gaps, zero promotion failures, and zero queue-full events.
- The collector restart occurred while no scientific paper cohort was active. It must be treated as an operational epoch and excluded from any later prospective boundary.

## Storage Incident and Current Routing

Root cause: raw source was moved to a dedicated kEfi hot volume, but promotion windows and materializer output continued writing under root. This caused root storage to fill despite available hot storage.

Current approximate capacity:

| Volume | Free capacity |
| --- | ---: |
| Root | 110 GiB |
| Dedicated kEfi hot volume | 420 GiB |
| Additional NFS/local-block capacity | about 2 TiB aggregate |

Correction underway:

- New promotions route to the dedicated hot volume; a post-change promoted window verified this path.
- New materializer output routes to the hot volume; one bounded materialization cycle succeeded there.
- The capacity guard now checks both root and the actual hot promotion path.
- Legacy root-resident promotions/materialization remain for separate hash-verified archive/release processing. They must not be deleted without dependency proof and receipts.

## Required Audit Questions

1. Can the sealed historical corpus be used for exploratory economic discovery if one new H3 rule is frozen before using any genuinely fresh V8 paper evidence?
2. What split/nested-validation design prevents repeated hypothesis shopping?
3. Why can high selection AUC coexist with poor paper economics? Separate proven facts from plausible mechanisms.
4. Is H1 fatal to kEfi research, merely a rejection of one translation from selection to entry, or insufficiently interpretable?
5. Should discovery choose one H3 candidate or a small preregistered family? State the anti-overfit policy.
6. What compact, hash-linked research table should be created once from existing evidence so many rules can be tested without rereading or duplicating raw full-meta partitions?
7. What is the minimum fresh paper gate that is meaningful without an arbitrary 24-hour wait?
8. What exact pass/fail criteria should be frozen for a future H3 paper/control result?
9. What canonical evidence-lake, retention-pin, archive, release-receipt, and capacity-guard design is required for sustainable multi-wallet research?
10. What should be stopped now because it creates storage waste or invalid inference?

## Required Recommendations

The reviewer should return:

- an executive verdict;
- what is and is not scientifically proven;
- methodology risks;
- storage/operations risks;
- a reusable evidence-lake schema;
- a chronological offline discovery protocol;
- an H3 preregistration template;
- a minimal fresh paper/control gate;
- stop conditions;
- an ordered implementation plan.

