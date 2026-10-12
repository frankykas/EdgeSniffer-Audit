import hashlib
import json
import subprocess
import sys
from pathlib import Path


INDEXER = Path(__file__).parents[1] / "scripts" / "kefi_incremental_event_index_v1.py"


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_window(out: Path, name: str, rows: list[dict]) -> None:
    rows_path = out / "market-rows" / f"{name}.jsonl"
    rows_path.parent.mkdir(parents=True, exist_ok=True)
    rows_path.write_text("".join(json.dumps(row, sort_keys=True) + "\n" for row in rows))
    receipt = {
        "window_id": name,
        "market_rows_path": str(rows_path),
        "market_rows_sha256": digest(rows_path),
        "source_partition_refs": [{"relative_manifest_path": "partitions/x/manifest.json", "manifest_sha256": "m", "compressed_sha256": "c"}],
        "decoder": {"decoder_path": "/decoder", "decoder_sha256": "d"},
    }
    receipt_path = out / "receipts" / f"{name}.json"
    receipt_path.parent.mkdir(parents=True, exist_ok=True)
    receipt_path.write_text(json.dumps(receipt, sort_keys=True))


def run(out: Path, index: Path):
    return subprocess.run(
        [sys.executable, str(INDEXER), "--materializer-out", str(out), "--index-root", str(index)],
        capture_output=True,
        text=True,
    )


def test_verified_overlap_is_deduplicated_once(tmp_path):
    out = tmp_path / "out"
    event_a = {"source_frame_sha256": "a", "event_index": 0, "received_at_unix_ns": 1, "slot": 1}
    event_b = {"source_frame_sha256": "b", "event_index": 0, "received_at_unix_ns": 2, "slot": 2}
    write_window(out, "a", [event_a])
    write_window(out, "b", [event_a, event_b])
    result = run(out, tmp_path / "index")
    assert result.returncode == 0, result.stderr
    checkpoint = json.loads((tmp_path / "index" / "checkpoint.json").read_text())
    assert checkpoint["canonical_events"] == 2
    assert checkpoint["verified_identical_overlaps"] == 1


def test_changed_rows_fail_closed(tmp_path):
    out = tmp_path / "out"
    write_window(out, "a", [{"source_frame_sha256": "a", "event_index": 0, "received_at_unix_ns": 1, "slot": 1}])
    assert run(out, tmp_path / "index").returncode == 0
    (out / "market-rows" / "a.jsonl").write_text('{"source_frame_sha256":"a","event_index":0,"received_at_unix_ns":2,"slot":1}\n')
    result = run(out, tmp_path / "index")
    assert result.returncode != 0
    assert "receipt rows hash mismatch" in result.stderr
