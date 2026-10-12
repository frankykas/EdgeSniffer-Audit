#!/usr/bin/env python3
"""Outcome-blind, content-verified incremental canonical event index.

It replaces a repeated full-history JSONL de-duplication pass. Each sealed
window is revalidated from its receipt and rows hash on every invocation, then
its source events are admitted once under `(source_frame_sha256, event_index)`.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import sqlite3
from pathlib import Path

SCHEMA = "KEFI_INCREMENTAL_EVENT_INDEX_V1"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def atomic_json(path: Path, payload: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.tmp")
    temporary.write_text(json.dumps(payload, sort_keys=True, separators=(",", ":")) + "\n")
    os.replace(temporary, path)


def database(path: Path) -> sqlite3.Connection:
    path.parent.mkdir(parents=True, exist_ok=True)
    connection = sqlite3.connect(path)
    connection.execute("PRAGMA journal_mode=WAL")
    connection.execute("PRAGMA foreign_keys=ON")
    connection.execute(
        "CREATE TABLE IF NOT EXISTS ingested_windows ("
        "window_id TEXT PRIMARY KEY, receipt_sha256 TEXT NOT NULL, rows_sha256 TEXT NOT NULL, "
        "source_refs_canonical TEXT NOT NULL, ingested_at_utc TEXT NOT NULL)"
    )
    connection.execute(
        "CREATE TABLE IF NOT EXISTS events ("
        "source_frame_sha256 TEXT NOT NULL, event_index INTEGER NOT NULL, canonical TEXT NOT NULL, "
        "received_at_unix_ns INTEGER NOT NULL, slot INTEGER NOT NULL, "
        "PRIMARY KEY(source_frame_sha256, event_index))"
    )
    return connection


def validated_receipt(out: Path, path: Path) -> tuple[dict, Path, str, str]:
    receipt = json.loads(path.read_text())
    window_id = str(receipt.get("window_id", ""))
    if not window_id:
        raise RuntimeError(f"FAIL_CLOSED: receipt missing window_id {path}")
    if not isinstance(receipt.get("source_partition_refs"), list) or not receipt["source_partition_refs"]:
        raise RuntimeError(f"FAIL_CLOSED: receipt lacks source partition bindings {window_id}")
    if not isinstance(receipt.get("decoder"), dict) or not receipt["decoder"].get("decoder_sha256"):
        raise RuntimeError(f"FAIL_CLOSED: receipt lacks decoder identity {window_id}")
    rows = Path(str(receipt.get("market_rows_path", "")))
    root = (out / "market-rows").resolve()
    if root not in rows.resolve().parents or not rows.is_file():
        raise RuntimeError(f"FAIL_CLOSED: invalid rows path {window_id}")
    rows_hash = sha256(rows)
    if rows_hash != receipt.get("market_rows_sha256"):
        raise RuntimeError(f"FAIL_CLOSED: receipt rows hash mismatch {window_id}")
    return receipt, rows, sha256(path), rows_hash


def ingest(connection: sqlite3.Connection, receipt: dict, rows: Path,
           receipt_hash: str, rows_hash: str) -> tuple[int, int]:
    window_id = receipt["window_id"]
    source_refs = json.dumps(receipt["source_partition_refs"], sort_keys=True, separators=(",", ":"))
    existing = connection.execute(
        "SELECT receipt_sha256, rows_sha256, source_refs_canonical FROM ingested_windows WHERE window_id = ?",
        (window_id,),
    ).fetchone()
    if existing:
        if tuple(existing) != (receipt_hash, rows_hash, source_refs):
            raise RuntimeError(f"FAIL_CLOSED: previously ingested window changed {window_id}")
        return 0, 0
    inserted = duplicates = 0
    with rows.open() as handle:
        for line_number, line in enumerate(handle, start=1):
            row = json.loads(line)
            frame = str(row.get("source_frame_sha256", ""))
            if not frame:
                raise RuntimeError(f"FAIL_CLOSED: missing source frame identity {window_id}:{line_number}")
            event_index = int(row["event_index"])
            canonical = json.dumps(row, sort_keys=True, separators=(",", ":"))
            cursor = connection.execute(
                "INSERT OR IGNORE INTO events VALUES (?, ?, ?, ?, ?)",
                (frame, event_index, canonical, int(row["received_at_unix_ns"]), int(row["slot"])),
            )
            if cursor.rowcount:
                inserted += 1
                continue
            old = connection.execute(
                "SELECT canonical FROM events WHERE source_frame_sha256 = ? AND event_index = ?",
                (frame, event_index),
            ).fetchone()[0]
            if old != canonical:
                raise RuntimeError(f"FAIL_CLOSED: conflicting source-frame event {frame}:{event_index}")
            duplicates += 1
    connection.execute(
        "INSERT INTO ingested_windows VALUES (?, ?, ?, ?, datetime('now'))",
        (window_id, receipt_hash, rows_hash, source_refs),
    )
    return inserted, duplicates


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--materializer-out", type=Path, required=True)
    parser.add_argument("--index-root", type=Path, required=True)
    args = parser.parse_args()
    connection = database(args.index_root / "market-events-v1.sqlite")
    inserted = duplicates = 0
    try:
        for receipt_path in sorted((args.materializer_out / "receipts").glob("*.json")):
            receipt, rows, receipt_hash, rows_hash = validated_receipt(args.materializer_out, receipt_path)
            with connection:
                new, old = ingest(connection, receipt, rows, receipt_hash, rows_hash)
            inserted += new
            duplicates += old
        event_count = connection.execute("SELECT COUNT(*) FROM events").fetchone()[0]
        window_count = connection.execute("SELECT COUNT(*) FROM ingested_windows").fetchone()[0]
    finally:
        connection.close()
    database_path = args.index_root / "market-events-v1.sqlite"
    atomic_json(args.index_root / "checkpoint.json", {
        "schema": SCHEMA + "_CHECKPOINT_V1",
        "index_database_sha256": sha256(database_path),
        "ingested_windows": window_count,
        "canonical_events": event_count,
        "new_events": inserted,
        "verified_identical_overlaps": duplicates,
        "outcome_blind": True,
        "pnl_accessed": False,
    })
    print(json.dumps({"canonical_events": event_count, "new_events": inserted}, sort_keys=True))


if __name__ == "__main__":
    main()
