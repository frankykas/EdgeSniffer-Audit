# EdgeSniffer Audit Package

This private repository is a sanitized, review-only package for an independent audit of the kEfi fingerprint-research lane and the supporting evidence-storage architecture.

It deliberately excludes credentials, provider tokens, private keys, raw full-meta evidence, live wallet secrets, and executable trading configuration.

Start with [AUDIT_BRIEF.md](AUDIT_BRIEF.md). It states the claims, failed experiment, evidence inventory, storage failures, current operational state, and the exact audit questions.

The `sanitized-code/` directory contains only the bounded materializer, evidence-catalog, capacity-guard, and regression-test surfaces relevant to this audit. These files are supplied for review, not deployment.

## Audit Rules

- Treat every numerical result as a claim requiring traceable evidence.
- Do not infer profitability from selection imitation.
- Do not recommend live trading from this package.
- Do not recommend copying raw evidence per hypothesis.
- Flag any conclusion that depends on data captured after its stated boundary.

