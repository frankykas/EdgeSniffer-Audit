# EdgeSniffer Autonomous Opportunity Model V1 Redesign

## Scope

This is a research-design specification for an independent, market-wide
Solana opportunity model. It is not a copytrading plan, a kEFi/2CQ extension,
or permission to trade. No live service, frozen cohort, historical evidence,
model, or execution path was changed.

The target question is: at a public causally valid decision boundary, which
eligible Pump/PumpSwap pools have positive expected **net executable** return
relative to a transparent autonomous baseline? Expert-wallet selection is
diagnostic context only; it must never be a live autonomous feature, candidate
filter, denominator, or validation label.

## 1. Frozen F01-F13 baseline audit

The frozen kEFi artifact is hash-bound as
`db08899d92f4d69d8c0234cfff9cbd05bd27dfb1365aed88d08b85f88cfc1b4a`.
Its coefficients, standardization values, and threshold remain confidential
and are intentionally not copied here.

| ID | Public frozen input | Original purpose | Autonomous treatment |
|---|---|---|---|
| F01 | `log_pre_pool_quote` | expert-choice liquidity scale | Reuse as an independently calibrated liquidity input. |
| F02 | `log_pre_pool_base` | expert-choice inventory scale | Reuse only with exact pool orientation/provenance. |
| F03 | `amm_fee_bps` | expert-choice fee context | Use as execution cost, not alpha alone. |
| F04 | `observed_pool_age_seconds` | public age at expert choice | Reuse only after universe recovery; observed age is not chain age. |
| F05 | `seconds_since_previous_observed_buy` | expert timing context | Redesign as public flow recency with validity/missingness. |
| F06 | `prior_pre_pool_quote_log_change` | pre-buy reserve movement | Reuse only under exact causal order. |
| F07-F09 | `prior_buy_count_{5,20,60}s` | short-window crowd/flow | Reuse with independently calibrated sell companions. |
| F10-F12 | `prior_buy_quote_{5,20,60}s` | short-window quote flow | Reuse after size/fee normalization. |
| F13 | `no_prior_observed_buy` | sparse-history selection clue | Treat as explicit data-state category, never an implicit zero. |

F01-F13 were built to score strictly pre-kEFi-action public state. Their
rolling fields require clean 5/20/60-second history, and a gap/missing state
invalidates rather than zero-fills a row. The kEFi action itself was excluded
from its snapshot. They demonstrated selection discrimination in their
historical setting, but H1 rejected one paper-economic translation. They are
therefore a frozen descriptive benchmark, not an autonomous strategy.

## 2. Proposed autonomous architecture

### Market discovery and eligibility

Enumerate every public supported Pump/PumpSwap pool event, never a
wallet-triggered window. Admit a candidate only when identity/layout/fees and
pre-event state resolve; source epoch is continuous; causal order is resolved;
candidate-universe completeness is proven; the pool is tradeable under a
separate frozen liquidity/layout contract; and no risk veto applies. Every
rejection has a reason code; unknown is not a pass.

### Opportunity ranking

At a fixed public decision clock or event boundary, rank the full eligible
universe, preserve top-K plus denominator, feature-validity mask, score
version, and deterministic ties. The scorer cannot use wallet activity,
post-boundary state, later price, outcome, or expert action. Begin with a
transparent regularized baseline using public liquidity, age, normalized flow,
fees, and state-validity fields; promote flexibility only after chronological
economic superiority under identical simulation.

### Candidate signal families

The first autonomous research pass may examine only signals observable before
the decision boundary and accompanied by an explicit validity flag:

| Family | Examples | Initial role |
|---|---|---|
| Executability | supported layout, pool orientation, quote/base reserves, fee tuple, quote age | Eligibility and cost model. |
| Public lifecycle | first public observation, provable observed age, deterministic pool-state availability | Candidate context, never an inferred launch time. |
| Public flow | buy/sell count and normalized quote flow over 5/20/60 seconds; reserve deltas | Ranker inputs after causal-order proof. |
| Market quality | depth, impact at fixed test size, spread/proxy, event cadence | Risk/execution veto and ranker context. |
| Integrity | continuity epoch, ordering status, universe certificate, feature validity | Mandatory gate, not alpha. |

Wallet identity, wallet holdings, wallet action timing, PnL, future state, and
post-entry pool transitions are prohibited inputs. Holder, authority, and
metadata-related fields are also prohibited until a separate audit proves
their source, timing, completeness, and utility without survivorship bias.

### Risk/manipulation veto

Separate vetoes include unsupported or ambiguous layout, inadequate executable
depth, continuity/order/universe failure, impossible reserve transitions,
abnormal failed-transaction/quote deterioration rate, and concentration or
authority hazards only where complete pre-entry data exists. Unknown evidence
is recorded as unknown, not considered safe.

### Entry, exit, and execution

Ranking and entry timing are distinct. A selected pool needs a frozen delay,
quote freshness limit, impact cap, expiry, and no-fill rule. Exits need a time
stop, hard loss/circuit breaker, liquidity-loss rule, and fixed profit/trailing
contract. Entry and exit cannot be tuned on the same fresh validation data.

The simulator must use exact Pump/PumpSwap transitions, AMM fee, reserve
impact/slippage, network fee, priority fee/tip, retry and failed-transaction
cost, stale quote/latency, partial/no fill, and adverse-selection buffer.
Denied/no-fill attempts remain in the denominator.

## 3. Economic objective

For decision `d`:

`R_net(d) = exit_proceeds - entry_notional - AMM_fees - price_impact - network_fees - priority_fees/tips - failed_tx_cost - adverse_fill_buffer`

The primary endpoint is chronological, cluster-aware mean `R_net` per
attempted eligible decision. The pre-paper promotion contract must require:

1. lower confidence bound of expected net return greater than zero;
2. lower confidence bound of advantage over a transparent autonomous baseline
   greater than zero;
3. no domination by a small set of pools, mints, hours, or one fill assumption;
4. predeclared minimum quote/fill coverage.

Notional, delay, cost schedule, confidence method, and clustering must be
measured from independent operations data then frozen before evaluation.
Expert-selection accuracy is never a substitute for net return.

## 4. Data readiness and bias rules

| Surface | Permitted use | Limitation |
|---|---|---|
| Historical corpus: 1,656 kEFi entries, 8,052 controls, 3,636,007 public candidates, 150 pools/mints | Exploratory representation and pipeline tests | Selection-conditioned; original order is not a certified autonomous universe. |
| Frozen F01-F13 / choice-state cohorts | Schema and validity benchmark | No relabeling into autonomous alpha; confidential parameters stay frozen. |
| H1 fresh paper result | Historical rejection only | Never retune, validate, or confirm a new autonomous rule. |
| PRE_H3, V6, voided cohorts | Integrity/provenance learning only | Frozen/quarantined; no outcome reuse. |
| Exact-transition V8/source rows | Potential AOM-V0 inputs | Usable only partition-by-partition after epoch, interval, decoder, and universe proof. |
| 2CQ history | Generic parser/storage engineering only | Not an autonomous-label source. |
| Canonical State v1 | Future trusted public source | Offline/source-only, not live certified today. |

Never use expert choice, post-entry reserves, eventual survival, realized
price path, future liquidity, exits, or expert action as pre-entry features.
Never bridge gaps or call a subset of newly seen pools a complete universe.

## 5. Offline evaluation

1. Build a compact reference-based autonomous table: canonical event identity,
   provenance, pre-entry inputs/validity masks, execution inputs, and raw-proof
   references. Do not duplicate the raw firehose.
2. Use purged chronological folds; purge overlapping 120-second windows and
   cluster by pool/mint and time bucket. Never randomly split event rows.
3. Compare uniform eligible selection, a transparent liquidity/age/flow
   heuristic, and no-trade. Every comparator uses identical costs.
4. Freeze one entry/exit/size/cost contract per experiment; include failures.
5. Limit discovery to a bounded configuration family and select one winner
   out-of-fold before seeing untouched validation.
6. Validate only on a later independently certified public interval, never H1,
   PRE_H3, V6, or any frozen holdout.

## 6. Exact first prototype

**`AOM_V0_PUBLIC_UNIVERSE_REPLAY`** is the recommended first implementation.
It is offline and non-trading. It enumerates public eligible pools from one or
more sealed exact-transition partitions; computes F01-F13 public analogues and
validity masks without wallet input; applies one transparent fixed baseline
ranker/top-K schedule; simulates fixed 30/60/120-second exploratory outcomes
with full costs; and emits coverage/exclusion accounting.

It must stop rather than score a partition if it cannot prove the candidate
denominator or required execution inputs. Its only goal is to prove the
independent-universe, timing, cost, and reporting plumbing, not profitability.

## 7. Roadmap, resources, and stop criteria

| Priority | Milestone | Estimated work | Acceptance / stop |
|---|---|---:|---|
| P0 | Immutable AOM-V0 input/feature contract | 0.5-1 day | Stop on expert or post-boundary dependency. |
| P1 | Reference-based table over one sealed interval | 1-2 days | Accept only with reconciled identity/order/state validity. |
| P2 | Exact cost adapter and transparent simulator | 1-2 days | Stop if execution costs/fills cannot be evidenced. |
| P3 | Chronological exploratory replay + baselines | 1-3 days | Stop on incomplete denominator, poor coverage, or no conservative advantage. |
| P4 | Bounded autonomous discovery tournament | 2-4 days | Choose at most one candidate or reject/no-trade. |
| P5 | Fresh independent paper cohort | runtime-dependent | Requires frozen economic/execution contract. |
| P6 | Dust execution calibration | separate approval | Requires positive fresh paper evidence and Rust parity. |

Planning storage is 0.5-2 KiB per candidate row: roughly 2-8 GiB uncompressed
for 3.636M rows and 0.5-3 GiB compressed. Checkpoints/indexes stay below 10%
of table size. A bounded 4-8-core batch should require 2-6 GiB RAM and hours,
not days; P1 must replace estimates with measurements. Raw full-meta duplication
is prohibited.

## 8. Completion checklist

| Request | Status | Result |
|---|---|---|
| Audit frozen inputs/timing/purpose | VERIFIED | F01-F13 matrix and original choice-study constraints documented. |
| Market-wide independent design | VERIFIED | Full public-universe admission/ranking contract specified. |
| Generalize/redesign inputs and signals | VERIFIED | Reuse matrix plus risk/execution additions specified. |
| Separate six components | VERIFIED | Discovery, scoring, risk, entry, exit, execution are distinct. |
| Cost-inclusive objective | VERIFIED | Net executable return and promotion requirements defined. |
| Historical data/bias assessment | VERIFIED | Exploratory, frozen, usable, and missing surfaces separated. |
| Offline evaluation / untouched validation | VERIFIED | Purged chronological framework and explicit no-reuse rule specified. |
| Smallest existing-data prototype | VERIFIED | AOM-V0 defined. |
| Roadmap/resources/milestones | VERIFIED | Prioritized plan and stop criteria supplied. |
| Preserve live/frozen surfaces | VERIFIED | Documentation-only work. |

## Current blockers

1. No live-certified complete autonomous candidate universe exists; Canonical
   State remains offline/source-only.
2. Existing history supports exploratory AOM-V0 plumbing, not an unbiased
   autonomous profitability claim.
3. Independent execution-cost telemetry and untouched autonomous validation do
   not exist yet.
4. Root free space was read as approximately 74 GiB; AOM-V0 must use the
   monitored hot volume/reference tables and cannot create raw duplicates.

**Verdict:** implement AOM-V0 offline next. Do not create an autonomous paper
or live cohort until P1-P4 are reviewed and a contract is frozen.
