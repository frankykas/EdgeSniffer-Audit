# EdgeSniffer Audit Package

## Start Here

- [Audit brief](AUDIT_BRIEF.md): scope, evidence, and questions given to the
  independent reviewer.
- [Independent response](AUDIT_RESPONSE.md): the external scientific and
  infrastructure recommendations.
- [Implementation status](AUDIT_IMPLEMENTATION_STATUS.md): point-by-point live
  remediation ledger. This is the authority for what is actually fixed versus
  merely proposed.
- [New-chat handoff](NEW_CHAT_HANDOFF.md): the operational and scientific
  context required to continue work without repeating the 2CQ/kEfi failures.

This public repository is a sanitized, review-only package for an independent audit of the kEfi fingerprint-research lane and the supporting evidence-storage architecture.

It deliberately excludes credentials, provider tokens, private keys, raw full-meta evidence, live wallet secrets, and executable trading configuration.

Start with [AUDIT_BRIEF.md](AUDIT_BRIEF.md). It states the claims, failed experiment, evidence inventory, storage failures, current operational state, and the exact audit questions.

The `sanitized-code/` directory contains only the bounded materializer, evidence-catalog, capacity-guard, and regression-test surfaces relevant to this audit. These files are supplied for review, not deployment.

This repository contains no credentials, raw evidence, wallet secrets, or live
execution authorization.

## Audit Rules

- Treat every numerical result as a claim requiring traceable evidence.
- Do not infer profitability from selection imitation.
- Do not recommend live trading from this package.
- Do not recommend copying raw evidence per hypothesis.
- Flag any conclusion that depends on data captured after its stated boundary.
