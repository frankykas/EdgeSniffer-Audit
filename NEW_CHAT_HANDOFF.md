# EdgeSniffer New-Chat Handoff

Use this document to start a new Codex session without losing the history that
matters. The new agent must verify live state before acting; this is a truthful
handoff, not a substitute for inspection.

## Paste This as the First New-Chat Message

```text
You are taking over EdgeSniffer research and infrastructure. Read AGENTS.md,
AUDIT_BRIEF.md, AUDIT_RESPONSE.md, AUDIT_IMPLEMENTATION_STATUS.md, and this
NEW_CHAT_HANDOFF.md before changing anything. Then independently verify the
live VPS state over SSH. Do not claim a result, restart a service, create an
experiment boundary, or begin H3 until you state which facts you verified live.

Priority order:
1. Preserve source evidence and avoid another root-volume incident.
2. Build the canonical compact evidence layer once, so research does not
   repeatedly scan/copy raw full-meta data.
3. Complete the independent-audit blockers in the documented order.
4. Only then run one bounded H3 historical discovery tournament and one fresh
   pre-registered paper/control gate.

Hard rules: no live trading; no Jupiter fallback; Rust-only execution posture;
no H3/H4/H5 sequence; never call selection imitation profitability; no outcome
peeking; use EDT in user-facing updates; explain every update in plain English
with: what happened, why it matters, what is bullish/not bullish, and the next
concrete action. Continue proactively through safe next steps instead of
waiting for repeated approval, but report blockers honestly.
```

## Live Access

- VPS: `root@159.195.207.109`
- Windows SSH key: `C:\Users\Cheet\.ssh\codex_edgesniffer_netcup`
- Repo on VPS: `/root/bot/EdgeSniffer`
- Audit repo: `https://github.com/frankykas/EdgeSniffer-Audit`
- Audit repo local checkout: `C:\Users\Cheet\OneDrive\Documents\EdgeSniffer-Audit`

Typical read-only command from Windows PowerShell:

```powershell
$key='C:\Users\Cheet\.ssh\codex_edgesniffer_netcup'
ssh -n -i $key -o BatchMode=yes root@159.195.207.109 'date -u; df -h / /mnt/edgesniffer-kefi-hot'
```

## The Actual Research Goal

The project is trying to reverse engineer teacher-wallet behavior into a
**strictly pre-entry, reproducible, economic paper fingerprint**. It is not
trying to prove that recognizing a wallet's purchases alone is tradable.

The correct chain is:

1. offline fingerprint discovery;
2. fresh prospective paper/control test;
3. execution-parity paper test;
4. separate tiny dust execution calibration;
5. only then consider larger live sizing.

At present we are before step 1 for the final kEfi H3 attempt.

## 2CQ History That Must Not Be Forgotten

- 2CQ selection work succeeded at a different question: V2 development N350
  and a one-shot N150 holdout were frozen. Frozen V2 fingerprint hash:
  `7606a3a138591516f503e7f7b56501f1828b92947a2b38707bff166fbb233b64`.
- That was WHICH-token imitation at known teacher moments. It did not prove
  autonomous WHEN detection, execution parity, or profitability.
- Later autonomous WHEN V1/V2/V3 work suffered repeated Shyft silent stalls,
  reconnect/gap accounting, hourly rollover races, lane reader/recovery bugs,
  and root-volume exhaustion. Valid segments were repeatedly fail-closed.
- Do not restart 2CQ collection or any old research/livedust services merely
  because they exist. Treat their historical claims separately from kEfi.
- The current repository AGENTS.md contains historical 2CQ paper checkpoints
  and strict rules around its active/paused service surface. Verify before
  calling any of them current.

## kEfi History and Current Truth

- Wallet: `kEFiAX3jo5NmemysQov342TZ9mGh6yp92GDRjhA8XDf`.
- Historical corpus: 1,656 selected entries, 8,052 controls, about 3.64M
  candidate events, 150 selected pools/mints. Historical selection separation
  was strong, but that is not economics.
- Fresh H1 paper result is rejected: 5,174 entries, -13.3939 SOL, PF 0.1548;
  controls -6.2433 SOL, PF 0.4066. It rejects one translation from selection
  imitation to entry/exit economics. It does not prove no possible kEfi edge.
- H2 has no fresh result. Do not score it on H1-opened evidence.
- H3 has never begun. Current live source is collecting operational evidence,
  not an H3 scientific paper cohort.

## Current kEfi Infrastructure

- Source root: `/mnt/edgesniffer-kefi-hot/kefi-capture-first-live-v8`
- Promotion root: `/mnt/edgesniffer-kefi-hot/kefi-capture-first-promotions-v8`
- Control output: `/mnt/edgesniffer-kefi-hot/kefi-capture-first-control-v8`
- Old root copies remain at `/root/kefi-capture-first-promotions-v8` and
  `/root/kefi-capture-first-control-v8`; do not delete them without full
  archive verification, dependency audit, and separate release receipt.
- Source unit: `edgesniffer-kefi-full-meta-capture-first-v8.service`
- Watchdog: `edgesniffer-kefi-full-meta-capture-first-watchdog-v8.service`
- Materializer: `edgesniffer-kefi-capture-first-control-v8.service/.timer`
- Capacity guard: `edgesniffer-kefi-capacity-guard-v8.service/.timer`
- At 2026-10-12T04:32Z: source + watchdog active, materializer timer/service
  inactive by design, guard active, root ~111 GiB free, hot volume ~403 GiB
  free. Fresh source health since 02:17Z restart: zero reconnects/errors/gaps,
  zero queue-full and promotion failures.

## Repairs Already Made

- Fixed actual event identity to `(source_frame_sha256,event_index)`; historical
  audit found duplicates were byte-identical and no conflicts.
- Routed fresh promotions/materialization to hot `/dev/vdc1` volume.
- Added mountpoint and `/dev/vdc1` identity pre-start checks to source and
  materializer units.
- Patched capacity guard so it stops materialization first and only claims an
  emergency collector pause after successful `systemctl stop`, a loaded unit,
  inactive/dead state, and `MainPID=0`. It verifies the hot mount resolves to
  `/dev/vdc1` on every guard cycle. The current deployed guard SHA-256 is
  `9d4c9c0f4297a4099bf724cb41d00515a5c78a6ecaa80f235383d3f37a908e3d`.
- Removed the service runner's automatic global `market-deduplicated.jsonl`
  rebuild after every bounded materializer cycle. A bounded live four-window
  cycle passed without triggering the rebuild.
- Public audit packet corrected to include actual production materializer and
  evidence-catalog sources; sanitized tests passed (4 passed).

## The Important Remaining Work

This is the exact implementation order. Do not skip it because capture happens
to look healthy:

1. Create scientific epoch/quarantine registry and enforce it.
2. Replace legacy JSONL/replay mechanics with truly incremental,
   content-verified canonical raw-event ingestion. Its current SQLite index is
   a prototype only: unintegrated and still O(history) due to re-hashing old
   rows on every invocation.
3. Prove restart equivalence and bind frozen receipt hashes, source manifests,
   decoder binary **and schema generation**. Keep intrinsic events separate
   from window/teacher exposure mappings.
4. Perform the pre-entry/actionability/paper-mechanics audit: availability
   semantics, same-slot ordering, decision latency, AMM arithmetic, fees,
   incomplete windows, controls, clustering, and clock assumptions.
5. Build audited compact Parquet/DuckDB research tables and deterministic
   control/split assignments.
6. Register at most 24 complete historical H3 candidate specifications,
   evaluate with purged chronological folds and pool/time clustering, then
   freeze exactly one.
7. Create one fresh H3 paper boundary and keep outcomes sealed until the
   pre-registered information requirement is reached.

## Communication Expectations

The user has spent days dealing with storage failures and misleading ETAs. Do
not give vague “all good” status. For every report say:

- **Plain English:** what is happening.
- **Bullish / not bullish:** exactly what the evidence supports.
- **Numbers:** sample/window count, current free storage, source health, ETA
  only when based on observed throughput.
- **Next:** what is running automatically and what will happen next without
  another prompt.
- **Honesty:** distinguish live verified, implementation-only, blocked, and
  untested. Never hide a disabled timer, failed gate, or scientific limitation.

## Forbidden Shortcuts

- no live trading or live dust based on H1/H2/current collection;
- no new H3 boundary before all audit blockers are verified;
- no arbitrary “24h is enough” rule;
- no fresh experiment after opening outcomes unless explicitly preregistered;
- no pool/mint ID as predictive feature;
- no random row-level train/test split;
- no raw copy per wallet/hypothesis;
- no broad delete/archive without hashes, dependency proofs, and release receipt;
- no restart of healthy capture for a test that can be mocked/offline.
