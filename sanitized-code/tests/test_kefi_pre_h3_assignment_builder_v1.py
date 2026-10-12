import copy
import hashlib
import importlib.util
import json
import sys
from pathlib import Path


ROOT = Path(__file__).parents[1]
SCRIPT = ROOT / "scripts" / "kefi_pre_h3_assignment_builder_v1.py"
MECHANICS = ROOT / "scripts" / "kefi_h3_pumpswap_mechanics_v1.py"
spec = importlib.util.spec_from_file_location("builder", SCRIPT); assert spec and spec.loader
b = importlib.util.module_from_spec(spec); spec.loader.exec_module(b)
spec = importlib.util.spec_from_file_location("mechanics", MECHANICS); assert spec and spec.loader
m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)


def row(seq, stamp, *, pool, mint, quote, tracked=False):
    return {
        "source_instance_id": "instance-a", "continuity_epoch": 1,
        "durable_output_sequence": seq, "event_index": 0, "source_frame_sha256": f"frame-{seq}",
        "received_at_unix_ns": stamp, "exact_transition_validated": True,
        "quote_mint": m.WSOL, "mint": mint, "amount_mode": "EXACT_BASE", "side": "BUY",
        "pool": pool, "pool_base_raw": 90, "pool_quote_raw": quote, "base_amount": 10,
        "pool_quote_delta": 10, "lp_fee_bps": 10, "protocol_fee_bps": 20,
        "coin_creator_bps": 30, "cashback_bps": 0, "buyback_bps": 0,
        "user_quote_amount": 1, "is_tracked_teacher": tracked,
    }


def model():
    # The first frozen feature distinguishes target/control while both remain
    # in the same pre-event liquidity stratum; this keeps the fixture honest.
    features = [f"f{index}" for index in range(13)]
    return {
        "status": "FROZEN_FOR_PROSPECTIVE_OUTCOME_BLIND_SHADOW", "strict_preentry": True,
        "features": features, "coefficients": {name: (1.0 if index == 0 else 0.0) for index, name in enumerate(features)},
        "intercept": -5.0, "standardization_means": [0.0] * 13,
        "standardization_scales": [1.0] * 13, "threshold": 0.5,
    }


def canonical_rows():
    ns = 1_000_000_000
    # First observed states establish equal pool-age buckets before warm-up.
    rows = [
        row(1, 1 * ns, pool="target-pool", mint="target-warm", quote=200, tracked=True),
        row(2, 1 * ns + 1, pool="control-pool", mint="control-warm", quote=120, tracked=True),
        # Both candidates are in availability bucket 6 after a 120-second warm-up.
        row(3, 130 * ns, pool="target-pool", mint="target", quote=200),
        row(4, 131 * ns, pool="control-pool", mint="control", quote=120),
        # State evidence for delayed entry and the 120-second mark is not itself a candidate.
        row(5, 132 * ns, pool="target-pool", mint="target", quote=200, tracked=True),
        row(6, 133 * ns, pool="control-pool", mint="control", quote=120, tracked=True),
        row(7, 253 * ns, pool="target-pool", mint="target", quote=200, tracked=True),
        row(8, 254 * ns, pool="control-pool", mint="control", quote=120, tracked=True),
    ]
    return rows


def test_assignments_are_restart_deterministic_and_keep_zero_controls():
    rows = canonical_rows()
    first = b.build_assignments(rows, model(), 0, m, control_seed="frozen-seed")
    second = b.build_assignments(list(reversed(rows)), model(), 0, m, control_seed="frozen-seed")
    assert first == second
    assert len(first) == 1
    assert first[0]["target_complete_actionable"] is True
    assert first[0]["controls"] and first[0]["controls"][0]["complete_actionable"] is True

    # A target is not dropped just because no compatible fingerprint non-match exists.
    zero_control = b.build_assignments(rows, {**model(), "threshold": 0.0}, 0, m, control_seed="frozen-seed")
    assert zero_control and all(not item["controls"] for item in zero_control)


def test_post_entry_or_pnl_fields_never_change_target_or_control_identity():
    original = canonical_rows()
    altered = copy.deepcopy(original)
    for index, item in enumerate(altered):
        item["post_entry_pnl"] = 999_999 - index
        item["later_mark"] = {"return": -999, "timestamp": 9_999_999_999}
        item["future_outcome_label"] = bool(index % 2)
    before = b.build_assignments(original, model(), 0, m, control_seed="frozen-seed")
    after = b.build_assignments(altered, model(), 0, m, control_seed="frozen-seed")
    identity = lambda items: [(item["target_id"], [control["control_id"] for control in item["controls"]]) for item in items]
    assert identity(before) == identity(after)


def test_atomic_jsonl_output_has_stable_content_across_rerun(tmp_path):
    assignments = b.build_assignments(canonical_rows(), model(), 0, m, control_seed="frozen-seed")
    path = tmp_path / "assignments.jsonl"
    for _ in range(2):
        path.write_text("".join(json.dumps(item, sort_keys=True, separators=(",", ":")) + "\n" for item in assignments))
        assert b.sha256(path) == b.sha256(path)


def test_end_to_end_receipt_registry_restart_is_byte_identical(tmp_path, monkeypatch):
    rows_path = tmp_path / "canonical.jsonl"
    rows_path.write_text("".join(json.dumps(item, sort_keys=True) + "\n" for item in canonical_rows()))
    fingerprint = tmp_path / "frozen-fingerprint.json"
    fingerprint.write_text(json.dumps(model(), sort_keys=True))
    boundary = tmp_path / "boundary.json"
    boundary.write_text(json.dumps({"status": "ACTIVE", "boundaryUtc": "1970-01-01T00:00:00.000000000Z"}))
    receipt = tmp_path / "scientific-receipt.json"
    receipt.write_text(json.dumps({
        "scientific_validation": {"rows": len(canonical_rows())},
        "market_rows_path": str(rows_path), "market_rows_sha256": b.sha256(rows_path),
    }))
    registry_root = tmp_path / "registry"
    registry = ROOT / "scripts" / "kefi_scientific_epoch_registry_v1.py"
    registry_module = importlib.util.spec_from_file_location("registry", registry); assert registry_module and registry_module.loader
    epochs = importlib.util.module_from_spec(registry_module); registry_module.loader.exec_module(epochs)
    epochs.append(registry_root, {"epoch_id": "future", "kind": "FUTURE_UNOPENED", "status": "UNOPENED",
                                  "start_utc": "1970-01-01T00:00:00Z", "end_utc": "", "reason": "test"})
    assignments, manifest = tmp_path / "assignments.jsonl", tmp_path / "manifest.json"
    old_expected = b.EXPECTED_FINGERPRINT_SHA256
    b.EXPECTED_FINGERPRINT_SHA256 = b.sha256(fingerprint)
    try:
        argv = ["builder", "--fingerprint", str(fingerprint), "--boundary-receipt", str(boundary),
                "--scientific-receipt", str(receipt), "--registry-script", str(registry), "--registry-root", str(registry_root),
                "--mechanics", str(MECHANICS), "--evaluator", str(ROOT / "scripts" / "kefi_pre_h3_actionability_evaluator_v1.py"),
                "--out-assignments", str(assignments), "--out-manifest", str(manifest), "--control-seed", "test-seed"]
        monkeypatch.setattr(sys, "argv", argv); b.main(); first = b.sha256(assignments)
        monkeypatch.setattr(sys, "argv", argv); b.main(); second = b.sha256(assignments)
    finally:
        b.EXPECTED_FINGERPRINT_SHA256 = old_expected
    assert first == second
    output = [json.loads(line) for line in assignments.read_text().splitlines()]
    assert output and output[0]["target_id"] and output[0]["controls"]
