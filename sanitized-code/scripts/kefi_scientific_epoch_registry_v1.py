#!/usr/bin/env python3
"""Append-only hash-chained registry for kEfi scientific evidence epochs."""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import time
from pathlib import Path


SCHEMA = "KEFI_SCIENTIFIC_EPOCH_REGISTRY_V1"
KINDS = {"HISTORICAL_DEVELOPMENT", "H1_OPENED", "H2_QUARANTINED", "OPERATIONAL", "FUTURE_UNOPENED"}
STATUSES = {"DEVELOPMENT_ONLY", "QUARANTINED", "OPERATIONAL_ONLY", "UNOPENED"}
VALID_KIND_STATUS = {
    "HISTORICAL_DEVELOPMENT": "DEVELOPMENT_ONLY",
    "H1_OPENED": "DEVELOPMENT_ONLY",
    "H2_QUARANTINED": "QUARANTINED",
    "OPERATIONAL": "OPERATIONAL_ONLY",
    "FUTURE_UNOPENED": "UNOPENED",
}


def canonical(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def digest(value: object) -> str:
    return hashlib.sha256(canonical(value)).hexdigest()


def load_lines(path: Path) -> list[dict]:
    if not path.exists():
        return []
    return [json.loads(line) for line in path.read_text().splitlines() if line]


def record_path(registry: Path, epoch_id: str) -> Path:
    return registry / "records" / f"{epoch_id}.json"


def acquire_lock(registry: Path, timeout_seconds: float = 10.0) -> int:
    lock = registry / ".append.lock"
    deadline = time.monotonic() + timeout_seconds
    while True:
        try:
            return os.open(lock, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        except FileExistsError:
            if time.monotonic() >= deadline:
                raise RuntimeError("FAIL_CLOSED: epoch registry append lock unavailable")
            time.sleep(0.05)


def release_lock(registry: Path, descriptor: int) -> None:
    os.close(descriptor)
    (registry / ".append.lock").unlink(missing_ok=True)


def append(registry: Path, payload: dict) -> dict:
    registry.mkdir(parents=True, exist_ok=True)
    descriptor = acquire_lock(registry)
    try:
        verify(registry)
        recover(registry)
        verify(registry)
        journal = registry / "epochs.jsonl"
        rows = load_lines(journal)
        validate_payload(payload)
        if any(row["epoch_id"] == payload["epoch_id"] for row in rows):
            raise RuntimeError(f"FAIL_CLOSED: epoch already exists {payload['epoch_id']}")
        previous = rows[-1]["record_sha256"] if rows else ""
        record = {"schema": SCHEMA, **payload, "previous_record_sha256": previous}
        record["record_sha256"] = digest(record)
        path = record_path(registry, payload["epoch_id"])
        path.parent.mkdir(parents=True, exist_ok=True)
        pending = registry / "pending" / f"{payload['epoch_id']}.json"
        pending.parent.mkdir(parents=True, exist_ok=True)
        with pending.open("wb") as handle:
            handle.write(canonical(record)); handle.flush(); os.fsync(handle.fileno())
        fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o444)
        with os.fdopen(fd, "wb") as handle:
            handle.write(canonical(record)); handle.flush(); os.fsync(handle.fileno())
        with journal.open("ab") as handle:
            handle.write(canonical(record)); handle.flush(); os.fsync(handle.fileno())
        pending.unlink(missing_ok=True)
        return record
    finally:
        release_lock(registry, descriptor)


def verify(registry: Path) -> None:
    previous = ""
    for record in load_lines(registry / "epochs.jsonl"):
        validate_payload(record)
        claimed = record.pop("record_sha256")
        if record.get("previous_record_sha256") != previous or digest(record) != claimed:
            raise RuntimeError("FAIL_CLOSED: epoch registry hash chain mismatch")
        disk_record = record_path(registry, record["epoch_id"])
        disk_payload = json.loads(disk_record.read_text()) if disk_record.is_file() else {}
        disk_claimed = disk_payload.pop("record_sha256", "")
        if disk_claimed != claimed or digest(disk_payload) != claimed:
            raise RuntimeError("FAIL_CLOSED: epoch registry record publication mismatch")
        previous = claimed


def recover(registry: Path) -> None:
    """Publish only a fully written orphan record that extends the verified tail."""
    journal = registry / "epochs.jsonl"
    rows = load_lines(journal)
    known = {row["epoch_id"] for row in rows}
    previous = rows[-1]["record_sha256"] if rows else ""
    for path in sorted((registry / "records").glob("*.json")) if (registry / "records").exists() else []:
        record = json.loads(path.read_text())
        if record["epoch_id"] in known:
            continue
        claimed = record.pop("record_sha256")
        if record.get("previous_record_sha256") != previous or digest(record) != claimed:
            raise RuntimeError("FAIL_CLOSED: unreconciled epoch record")
        with journal.open("ab") as handle:
            handle.write(canonical({**record, "record_sha256": claimed})); handle.flush(); os.fsync(handle.fileno())
        previous = claimed


def eligible(registry: Path, at_utc: str, boundary_utc: str) -> None:
    verify(registry)
    at = parse_utc(at_utc)
    boundary = parse_utc(boundary_utc)
    if at < boundary:
        raise RuntimeError("INELIGIBLE_PRE_BOUNDARY")
    matching = []
    for record in load_lines(registry / "epochs.jsonl"):
        validate_payload(record)
        start = parse_utc(record["start_utc"])
        end = parse_utc(record.get("end_utc") or "9999-12-31T23:59:59Z")
        if start <= at <= end:
            matching.append(record)
    if not matching:
        raise RuntimeError("INELIGIBLE_EPOCH_GAP")
    if len(matching) != 1:
        raise RuntimeError("INELIGIBLE_EPOCH_AMBIGUOUS")
    record = matching[0]
    if record["kind"] != "FUTURE_UNOPENED" or record["status"] != "UNOPENED":
        raise RuntimeError(f"INELIGIBLE_EPOCH_{record['kind']}")


def validate_payload(payload: dict) -> None:
    kind, status = payload.get("kind"), payload.get("status")
    if kind not in KINDS or status not in STATUSES or VALID_KIND_STATUS.get(kind) != status:
        raise RuntimeError("FAIL_CLOSED: invalid epoch kind/status combination")
    start = parse_utc(str(payload.get("start_utc", "")))
    end_text = str(payload.get("end_utc") or "")
    if end_text and parse_utc(end_text) < start:
        raise RuntimeError("FAIL_CLOSED: epoch end precedes start")


def parse_utc(value: str) -> dt.datetime:
    parsed = dt.datetime.fromisoformat(value.replace("Z", "+00:00"))
    if parsed.tzinfo is None:
        raise RuntimeError("FAIL_CLOSED: timestamp lacks timezone")
    return parsed.astimezone(dt.timezone.utc)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--registry", type=Path, required=True)
    parser.add_argument("--verify", action="store_true")
    parser.add_argument("--eligible-at-utc")
    parser.add_argument("--boundary-utc")
    parser.add_argument("--epoch-id")
    parser.add_argument("--kind", choices=sorted(KINDS))
    parser.add_argument("--status", choices=sorted(STATUSES))
    parser.add_argument("--start-utc")
    parser.add_argument("--end-utc")
    parser.add_argument("--reason", default="")
    args = parser.parse_args()
    if args.verify:
        verify(args.registry); print("VERIFIED"); return
    if args.eligible_at_utc or args.boundary_utc:
        if not (args.eligible_at_utc and args.boundary_utc):
            raise SystemExit("eligible-at-utc and boundary-utc are required together")
        eligible(args.registry, args.eligible_at_utc, args.boundary_utc); print("ELIGIBLE"); return
    if not all((args.epoch_id, args.kind, args.status, args.start_utc)):
        raise SystemExit("epoch-id, kind, status, and start-utc are required")
    print(json.dumps(append(args.registry, {
        "epoch_id": args.epoch_id, "kind": args.kind, "status": args.status,
        "start_utc": args.start_utc, "end_utc": args.end_utc, "reason": args.reason,
    }), sort_keys=True))


if __name__ == "__main__":
    main()
