#!/usr/bin/env python3
"""Append-only, hash-chained scientific epoch registry."""
from __future__ import annotations
import argparse, hashlib, json, os
from pathlib import Path

SCHEMA = "KEFI_SCIENTIFIC_EPOCH_REGISTRY_V1"
KINDS = {"HISTORICAL_DEVELOPMENT", "H1_OPENED", "H2_QUARANTINED", "OPERATIONAL", "FUTURE_UNOPENED"}
STATUSES = {"DEVELOPMENT_ONLY", "QUARANTINED", "OPERATIONAL_ONLY", "UNOPENED"}

def canonical(value): return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()
def digest(value): return hashlib.sha256(canonical(value)).hexdigest()
def load(path): return [] if not path.exists() else [json.loads(line) for line in path.read_text().splitlines() if line]

def append(registry: Path, payload: dict) -> dict:
    registry.mkdir(parents=True, exist_ok=True); journal = registry / "epochs.jsonl"; rows = load(journal)
    if any(row["epoch_id"] == payload["epoch_id"] for row in rows): raise RuntimeError(f"FAIL_CLOSED: epoch already exists {payload['epoch_id']}")
    record = {"schema": SCHEMA, **payload, "previous_record_sha256": rows[-1]["record_sha256"] if rows else ""}
    record["record_sha256"] = digest(record)
    with journal.open("ab") as handle: handle.write(canonical(record)); handle.flush(); os.fsync(handle.fileno())
    path = registry / "records" / f"{payload['epoch_id']}.json"; path.parent.mkdir(parents=True, exist_ok=True)
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o444)
    with os.fdopen(fd, "wb") as handle: handle.write(canonical(record)); handle.flush(); os.fsync(handle.fileno())
    return record

def verify(registry: Path) -> None:
    previous = ""
    for record in load(registry / "epochs.jsonl"):
        claimed = record.pop("record_sha256")
        if record.get("previous_record_sha256") != previous or digest(record) != claimed: raise RuntimeError("FAIL_CLOSED: epoch registry hash chain mismatch")
        previous = claimed

def main():
    parser = argparse.ArgumentParser(); parser.add_argument("--registry", type=Path, required=True); parser.add_argument("--verify", action="store_true"); parser.add_argument("--epoch-id"); parser.add_argument("--kind", choices=sorted(KINDS)); parser.add_argument("--status", choices=sorted(STATUSES)); parser.add_argument("--start-utc"); parser.add_argument("--end-utc", default=""); parser.add_argument("--reason", default=""); args = parser.parse_args()
    if args.verify: verify(args.registry); print("VERIFIED"); return
    if not all((args.epoch_id, args.kind, args.status, args.start_utc)): raise SystemExit("epoch-id, kind, status, and start-utc are required")
    print(json.dumps(append(args.registry, {"epoch_id":args.epoch_id,"kind":args.kind,"status":args.status,"start_utc":args.start_utc,"end_utc":args.end_utc,"reason":args.reason}), sort_keys=True))
if __name__ == "__main__": main()
