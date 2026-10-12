import json
import subprocess
import sys
from pathlib import Path


MATERIALIZER = Path(__file__).parents[1] / "scripts" / "kefi_capture_first_control_materializer_v1.py"


def row(candidate_signature: str, source_frame_sha256: str, pool_quote_raw: int) -> dict:
    return {
        "candidate_signature": candidate_signature,
        "event_index": 0,
        "source_frame_sha256": source_frame_sha256,
        "received_at_unix_ns": 10,
        "slot": 1,
        "pool_quote_raw": pool_quote_raw,
    }


def rebuild(tmp_path: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            str(MATERIALIZER),
            "--root",
            str(tmp_path / "source"),
            "--out",
            str(tmp_path / "out"),
            "--decoder",
            str(tmp_path / "unused-decoder"),
            "--rebuild-deduplicated-market",
        ],
        text=True,
        capture_output=True,
    )


def write_rows(tmp_path: Path, *rows: dict) -> None:
    path = tmp_path / "out" / "market-rows" / "rows.jsonl"
    path.parent.mkdir(parents=True)
    path.write_text("".join(json.dumps(row, sort_keys=True) + "\n" for row in rows))


def test_teacher_window_identity_does_not_collapse_distinct_source_frames(tmp_path):
    write_rows(tmp_path, row("teacher", "frame-a", 100), row("teacher", "frame-b", 200))
    result = rebuild(tmp_path)
    assert result.returncode == 0, result.stderr
    rows = (tmp_path / "out" / "market-deduplicated.jsonl").read_text().splitlines()
    assert len(rows) == 2


def test_true_source_frame_overlap_must_be_byte_identical(tmp_path):
    first = row("teacher-a", "frame-a", 100)
    write_rows(tmp_path, first, first)
    result = rebuild(tmp_path)
    assert result.returncode == 0, result.stderr
    rows = (tmp_path / "out" / "market-deduplicated.jsonl").read_text().splitlines()
    assert len(rows) == 1


def test_conflicting_source_frame_overlap_fails_closed(tmp_path):
    write_rows(tmp_path, row("teacher-a", "frame-a", 100), row("teacher-b", "frame-a", 200))
    result = rebuild(tmp_path)
    assert result.returncode != 0
    assert "FAIL_CLOSED: conflicting source-frame overlap" in result.stderr
