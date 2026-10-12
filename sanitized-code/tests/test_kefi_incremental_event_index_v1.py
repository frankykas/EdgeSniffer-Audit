import hashlib
import json
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).parents[1]
INDEXER = ROOT / "scripts" / "kefi_incremental_event_index_v1.py"


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def row(frame: str, event_index: int, quote: int, candidate: str) -> dict:
    return {
        "source_frame_sha256": frame, "event_index": event_index,
        "received_at_unix_ns": 10, "slot": 1, "pool_quote_raw": quote,
        "candidate_signature": candidate,
    }


def write_window(out: Path, window_id: str, rows: list[dict], *, decoder=True) -> Path:
    rows_path = out / "market-rows" / f"{window_id}.jsonl"
    rows_path.parent.mkdir(parents=True, exist_ok=True)
    rows_path.write_text("".join(json.dumps(item, sort_keys=True) + "\n" for item in rows))
    receipt = {
        "window_id": window_id, "market_rows_path": str(rows_path),
        "market_rows_sha256": sha256(rows_path),
        "source_partition_refs": [{"relative_manifest_path": "partitions/manifest.json", "manifest_sha256": "manifest", "compressed_sha256": "compressed"}],
    }
    if decoder:
        receipt["decoder"] = {"decoder_path": "/decoder", "decoder_sha256": "decoder"}
    receipt_path = out / "receipts" / f"{window_id}.json"
    receipt_path.parent.mkdir(parents=True, exist_ok=True)
    receipt_path.write_text(json.dumps(receipt, sort_keys=True) + "\n")
    descriptor = {"schema": "KEFI_CAPTURE_FIRST_CONTROL_MATERIALIZER_V1_INGEST_DESCRIPTOR_V1", "window_id": window_id,
                  "receipt_path": str(receipt_path), "receipt_sha256": sha256(receipt_path),
                  "market_rows_sha256": receipt["market_rows_sha256"]}
    pending = out / "canonical-ingest" / "pending" / f"{window_id}.json"
    pending.parent.mkdir(parents=True, exist_ok=True)
    pending.write_text(json.dumps(descriptor, sort_keys=True) + "\n")
    return receipt_path


def run(out: Path, index: Path, *extra: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run([sys.executable, str(INDEXER), "--materializer-out", str(out), "--index-root", str(index), *extra], text=True, capture_output=True)


def checkpoint(index: Path) -> dict:
    return json.loads((index / "checkpoint.json").read_text())


def test_queue_ingest_preserves_intrinsic_identity_and_exposures(tmp_path):
    out, index = tmp_path / "out", tmp_path / "index"
    write_window(out, "a", [row("frame", 0, 100, "teacher-a")])
    write_window(out, "b", [row("frame", 0, 100, "teacher-b"), row("frame-b", 0, 200, "teacher-b")])
    result = run(out, index)
    assert result.returncode == 0, result.stderr
    assert checkpoint(index)["canonical_events"] == 2
    assert checkpoint(index)["verified_identical_overlaps"] == 1
    assert len(list((out / "canonical-ingest" / "processed").glob("*.json"))) == 2


def test_normal_run_does_not_require_old_receipts_or_scan_history(tmp_path):
    out, index = tmp_path / "out", tmp_path / "index"
    first = write_window(out, "a", [row("frame-a", 0, 100, "teacher-a")])
    assert run(out, index).returncode == 0
    first.unlink()  # already verified and committed; hot path must not revisit it
    second = write_window(out, "b", [row("frame-b", 0, 200, "teacher-b")])
    result = run(out, index)
    assert result.returncode == 0, result.stderr
    assert checkpoint(index)["canonical_events"] == 2
    assert second.is_file()


def test_recovery_replays_processing_descriptor_safely(tmp_path):
    out, index = tmp_path / "out", tmp_path / "index"
    write_window(out, "a", [row("frame-a", 0, 100, "teacher-a")])
    pending = out / "canonical-ingest" / "pending" / "a.json"
    processing = out / "canonical-ingest" / "processing" / "a.json"
    processing.parent.mkdir(parents=True)
    pending.replace(processing)
    result = run(out, index)
    assert result.returncode == 0, result.stderr
    assert (out / "canonical-ingest" / "processed" / "a.json").is_file()


def test_queue_ingest_fails_closed_on_changed_rows_or_missing_decoder(tmp_path):
    out, index = tmp_path / "out", tmp_path / "index"
    write_window(out, "a", [row("frame-a", 0, 100, "teacher-a")], decoder=False)
    result = run(out, index)
    assert result.returncode != 0 and "decoder" in result.stderr


def test_explicit_canonical_export_is_identical_across_replay_and_restart(tmp_path):
    first_out, second_out = tmp_path / "first", tmp_path / "second"
    for out in (first_out, second_out):
        write_window(out, "a", [row("frame", 0, 100, "teacher-a")])
        write_window(out, "b", [row("frame", 0, 100, "teacher-b"), row("frame-b", 1, 200, "teacher-b")])
    assert run(first_out, tmp_path / "index-a", "--export-canonical").returncode == 0
    # Simulate a process restart with one descriptor already claimed.
    pending = second_out / "canonical-ingest" / "pending" / "a.json"
    processing = second_out / "canonical-ingest" / "processing" / "a.json"; processing.parent.mkdir(parents=True)
    pending.replace(processing)
    assert run(second_out, tmp_path / "index-b", "--export-canonical").returncode == 0
    a = json.loads((tmp_path / "index-a" / "canonical-export-v1" / "manifest.json").read_text())
    b = json.loads((tmp_path / "index-b" / "canonical-export-v1" / "manifest.json").read_text())
    assert a == b
