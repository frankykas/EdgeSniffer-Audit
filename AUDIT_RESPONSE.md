# Response to Independent Audit

## Verified Package Corrections

The first public packet had packaging drift: the supplied tests expected scripts
under `sanitized-code/scripts/`, while one materializer copy had been placed at
the directory root, and the catalog implementation was omitted. The actual
VPS production sources are now included at the expected paths.

Production source hashes at the time of this update:

- Materializer: `ddb7a576f4b5b09a8ca63282e6767561a953eff6a82b76e1edf24f2185d5e27a`
- Evidence catalog: `15267094610fdc5b5d285244a5cb684372db4f932bc54bbc748ffec6b2a99917`
- Capacity guard: `8f6570f9429e878322952aeab6a3dd311d4219ee4d3df3a7b779b9c48bcc7e3a`

The overlap identity used by production is explicitly
`(source_frame_sha256, event_index)`. The materializer supports
`--rebuild-deduplicated-market`; this is now auditable from the supplied source.

## Accepted Production Findings

The independent audit correctly identified these remaining production blockers:

1. The materializer still rebuilds the full historical deduplicated JSONL after
   each bounded batch. This is being replaced by incremental indexed ingestion.
2. A processed window should be revalidated from its receipt and content hash,
   rather than trusting a bare ID in `state.json`.
3. The capacity guard must verify systemd reached the intended stopped state
   before it records an emergency pause.
4. Services must verify the expected hot-volume mount identity, not merely the
   existence of a `/mnt/...` path.

No H3 paper boundary may be created until those mechanics are repaired and
tested against the production source surface.
