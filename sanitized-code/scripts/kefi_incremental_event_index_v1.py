#!/usr/bin/env python3
"""Crash-recoverable, queue-driven canonical kEfi event ingestion.

Normal operation consumes only descriptors published by the materializer after
its receipt is durable.  Historical receipts are never globbed or re-hashed on
the hot path.  A descriptor is claimed atomically, its receipt and rows are
verified once, and SQLite commits the event plus exposure mapping together.
After a crash, an item left in ``processing`` is safely replayed: the existing
window binding must match byte-for-byte or ingestion fails closed.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import sqlite3
from pathlib import Path


SCHEMA = "KEFI_INCREMENTAL_EVENT_INDEX_V2"
WINDOW_SCOPED_FIELDS = {
    "candidate_signature", "candidate_signatures", "candidate_index",
    "window_id", "windowId", "window_path", "windowPath",
}


def canonical(value: object) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"))


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def fsync_directory(path: Path) -> None:
    # Windows does not permit opening a directory this way.  Production is
    # Linux and fsyncs directory metadata; local regression tests retain the
    # same atomic rename semantics without pretending Windows supports it.
    if os.name == "nt":
        return
    descriptor = os.open(path, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def atomic_json(path: Path, payload: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.tmp")
    with temporary.open("w") as handle:
        handle.write(canonical(payload) + "\n")
        handle.flush()
        os.fsync(handle.fileno())
    os.replace(temporary, path)
    fsync_directory(path.parent)


def connection_for(path: Path) -> sqlite3.Connection:
    path.parent.mkdir(parents=True, exist_ok=True)
    connection = sqlite3.connect(path)
    connection.execute("PRAGMA journal_mode=WAL")
    connection.execute("PRAGMA synchronous=FULL")
    connection.execute("PRAGMA foreign_keys=ON")
    connection.execute(
        "CREATE TABLE IF NOT EXISTS ingested_windows ("
        "window_id TEXT PRIMARY KEY, receipt_sha256 TEXT NOT NULL, rows_sha256 TEXT NOT NULL, "
        "source_refs_canonical TEXT NOT NULL, decoder_sha256 TEXT NOT NULL, ingested_at_utc TEXT NOT NULL)"
    )
    connection.execute(
        "CREATE TABLE IF NOT EXISTS events ("
        "source_frame_sha256 TEXT NOT NULL, event_index INTEGER NOT NULL, intrinsic_canonical TEXT NOT NULL, "
        "received_at_unix_ns INTEGER NOT NULL, slot INTEGER NOT NULL, "
        "PRIMARY KEY(source_frame_sha256, event_index))"
    )
    connection.execute(
        "CREATE TABLE IF NOT EXISTS window_events ("
        "window_id TEXT NOT NULL, source_frame_sha256 TEXT NOT NULL, event_index INTEGER NOT NULL, "
        "exposure_canonical TEXT NOT NULL, PRIMARY KEY(window_id, source_frame_sha256, event_index), "
        "FOREIGN KEY(window_id) REFERENCES ingested_windows(window_id))"
    )
    return connection


def queue_paths(queue_root: Path) -> tuple[Path, Path, Path]:
    return queue_root / "pending", queue_root / "processing", queue_root / "processed"


def claim_descriptors(queue_root: Path) -> list[Path]:
    pending, processing, _ = queue_paths(queue_root)
    pending.mkdir(parents=True, exist_ok=True)
    processing.mkdir(parents=True, exist_ok=True)
    claimed = sorted(processing.glob("*.json"))  # recover a prior interrupted claim first
    for path in sorted(pending.glob("*.json")):
        claimed_path = processing / path.name
        try:
            os.replace(path, claimed_path)
            claimed.append(claimed_path)
        except FileNotFoundError:
            continue
    fsync_directory(pending)
    fsync_directory(processing)
    return claimed


def validate_descriptor(out: Path, descriptor_path: Path) -> tuple[dict, dict, Path, str, str]:
    descriptor = json.loads(descriptor_path.read_text())
    if descriptor.get("schema") != "KEFI_CAPTURE_FIRST_CONTROL_MATERIALIZER_V1_INGEST_DESCRIPTOR_V1":
        raise RuntimeError(f"FAIL_CLOSED: invalid ingest descriptor {descriptor_path}")
    window_id = str(descriptor.get("window_id", ""))
    receipt_path = Path(str(descriptor.get("receipt_path", "")))
    receipt_root = (out / "receipts").resolve()
    if not window_id or receipt_path.resolve().parent != receipt_root:
        raise RuntimeError(f"FAIL_CLOSED: descriptor receipt path escapes output root {descriptor_path}")
    if receipt_path.name != f"{window_id}.json" or not receipt_path.is_file():
        raise RuntimeError(f"FAIL_CLOSED: descriptor receipt absent {window_id}")
    receipt_sha256 = sha256(receipt_path)
    if receipt_sha256 != descriptor.get("receipt_sha256"):
        raise RuntimeError(f"FAIL_CLOSED: descriptor receipt hash mismatch {window_id}")
    receipt = json.loads(receipt_path.read_text())
    source_refs = receipt.get("source_partition_refs")
    decoder = receipt.get("decoder")
    if receipt.get("window_id") != window_id or not isinstance(source_refs, list) or not source_refs:
        raise RuntimeError(f"FAIL_CLOSED: invalid source receipt {window_id}")
    if not isinstance(decoder, dict) or not decoder.get("decoder_sha256"):
        raise RuntimeError(f"FAIL_CLOSED: source receipt lacks decoder identity {window_id}")
    rows_path = Path(str(receipt.get("market_rows_path", "")))
    rows_root = (out / "market-rows").resolve()
    if rows_path.resolve().parent != rows_root or not rows_path.is_file():
        raise RuntimeError(f"FAIL_CLOSED: source rows missing or outside output root {window_id}")
    rows_sha256 = sha256(rows_path)
    if rows_sha256 != receipt.get("market_rows_sha256") or rows_sha256 != descriptor.get("market_rows_sha256"):
        raise RuntimeError(f"FAIL_CLOSED: source rows hash mismatch {window_id}")
    return descriptor, receipt, rows_path, receipt_sha256, rows_sha256


def intrinsic_event(row: dict) -> dict:
    return {key: value for key, value in row.items() if key not in WINDOW_SCOPED_FIELDS}


def ingest(connection: sqlite3.Connection, receipt: dict, rows_path: Path,
           receipt_sha256: str, rows_sha256: str) -> tuple[int, int]:
    window_id = receipt["window_id"]
    source_refs = canonical(receipt["source_partition_refs"])
    decoder_sha256 = str(receipt["decoder"]["decoder_sha256"])
    existing = connection.execute(
        "SELECT receipt_sha256, rows_sha256, source_refs_canonical, decoder_sha256 "
        "FROM ingested_windows WHERE window_id = ?", (window_id,)
    ).fetchone()
    expected = (receipt_sha256, rows_sha256, source_refs, decoder_sha256)
    if existing:
        if tuple(existing) != expected:
            raise RuntimeError(f"FAIL_CLOSED: previously ingested window changed {window_id}")
        return 0, 0
    inserted = duplicates = 0
    connection.execute(
        "INSERT INTO ingested_windows VALUES (?, ?, ?, ?, ?, datetime('now'))",
        (window_id, *expected),
    )
    with rows_path.open() as handle:
        for line_number, line in enumerate(handle, start=1):
            row = json.loads(line)
            frame = str(row.get("source_frame_sha256", ""))
            if not frame:
                raise RuntimeError(f"FAIL_CLOSED: missing source frame identity {window_id}:{line_number}")
            event_index = int(row["event_index"])
            intrinsic = canonical(intrinsic_event(row))
            cursor = connection.execute(
                "INSERT OR IGNORE INTO events VALUES (?, ?, ?, ?, ?)",
                (frame, event_index, intrinsic, int(row["received_at_unix_ns"]), int(row["slot"])),
            )
            if cursor.rowcount:
                inserted += 1
            else:
                previous = connection.execute(
                    "SELECT intrinsic_canonical FROM events WHERE source_frame_sha256 = ? AND event_index = ?",
                    (frame, event_index),
                ).fetchone()[0]
                if previous != intrinsic:
                    raise RuntimeError(f"FAIL_CLOSED: conflicting source event {frame}:{event_index}")
                duplicates += 1
            exposure = canonical({key: row[key] for key in sorted(WINDOW_SCOPED_FIELDS & row.keys())})
            connection.execute(
                "INSERT OR IGNORE INTO window_events VALUES (?, ?, ?, ?)",
                (window_id, frame, event_index, exposure),
            )
    return inserted, duplicates


def checkpoint(connection: sqlite3.Connection, database: Path, index_root: Path, new: int, duplicates: int) -> tuple[int, int]:
    events = connection.execute("SELECT COUNT(*) FROM events").fetchone()[0]
    windows = connection.execute("SELECT COUNT(*) FROM ingested_windows").fetchone()[0]
    atomic_json(index_root / "checkpoint.json", {
        "schema": SCHEMA + "_CHECKPOINT_V1",
        "index_database_sha256": sha256(database),
        "ingested_windows": windows,
        "canonical_events": events,
        "new_events": new,
        "verified_identical_overlaps": duplicates,
        "outcome_blind": True,
        "pnl_accessed": False,
    })
    return events, windows


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--materializer-out", type=Path, required=True)
    parser.add_argument("--index-root", type=Path, required=True)
    parser.add_argument("--queue-root", type=Path)
    args = parser.parse_args()
    queue_root = args.queue_root or args.materializer_out / "canonical-ingest"
    database = args.index_root / "market-events-v2.sqlite"
    connection = connection_for(database)
    new = duplicates = 0
    try:
        for descriptor_path in claim_descriptors(queue_root):
            _, receipt, rows, receipt_hash, rows_hash = validate_descriptor(args.materializer_out, descriptor_path)
            with connection:
                added, overlap = ingest(connection, receipt, rows, receipt_hash, rows_hash)
            new += added; duplicates += overlap
            _, processing, processed = queue_paths(queue_root)
            processed.mkdir(parents=True, exist_ok=True)
            os.replace(descriptor_path, processed / descriptor_path.name)
            fsync_directory(processing); fsync_directory(processed)
        events, _ = checkpoint(connection, database, args.index_root, new, duplicates)
    finally:
        connection.close()
    print(json.dumps({"canonical_events": events, "new_events": new, "verified_identical_overlaps": duplicates}, sort_keys=True))


if __name__ == "__main__":
    main()
