#!/usr/bin/env python3
"""Deterministically build PRE_H3_ACTIONABILITY_QUALIFICATION_V1 assignments.

This is deliberately an upstream publisher, not an evaluator or boundary
creator.  It admits all frozen-fingerprint matches before delayed entry,
tradeability, or horizon availability is derived; binds every input and tool
identity; and writes the JSONL consumed by the frozen evaluator.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import math
import os
from collections import defaultdict, deque
from pathlib import Path
from typing import Iterable


SCHEMA = "KEFI_PRE_H3_ASSIGNMENT_BUILDER_V1"
EXPECTED_FINGERPRINT_SHA256 = "db08899d92f4d69d8c0234cfff9cbd05bd27dfb1365aed88d08b85f88cfc1b4a"
WARMUP_NS = 120_000_000_000
ENTRY_DELAY_NS = 1_000_000_000
HORIZON_NS = 120_000_000_000
CHRONOLOGICAL_BLOCKS = 8
MAX_CONTROLS = 4


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def canonical(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()


def atomic(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    with temporary.open("wb") as handle:
        handle.write(canonical(value)); handle.flush(); os.fsync(handle.fileno())
    os.replace(temporary, path)


def load_module(path: Path, name: str):
    spec = importlib.util.spec_from_file_location(name, path)
    if not spec or not spec.loader:
        raise RuntimeError(f"FAIL_CLOSED: cannot load {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def parse_utc_ns(value: str) -> int:
    if not value.endswith("Z"):
        raise RuntimeError("FAIL_CLOSED: UTC boundary must end with Z")
    main, dot, fraction = value[:-1].partition(".")
    from datetime import datetime, timezone
    seconds = int(datetime.fromisoformat(main + "+00:00").replace(tzinfo=timezone.utc).timestamp())
    nanos = int((fraction + "000000000")[:9]) if dot else 0
    return seconds * 1_000_000_000 + nanos


def event_id(mechanics, row: dict) -> str:
    return "|".join(map(str, mechanics.event_key(row)))


class PoolState:
    """Frozen base-fingerprint features, observed strictly before each row."""
    windows = (5, 20, 60)

    def __init__(self) -> None:
        self.events: deque[tuple[int, float]] = deque()
        self.first: int | None = None
        self.last: int | None = None
        self.last_quote: float | None = None

    def snapshot(self, timestamp_ns: int, row: dict) -> list[float]:
        while self.events and self.events[0][0] < timestamp_ns - 60_000_000_000:
            self.events.popleft()
        quote = float(row.get("pool_quote_raw", 0) or 0) - float(row.get("pool_quote_delta", 0) or 0)
        base = float(row.get("pool_base_raw", 0) or 0) + float(row.get("base_amount", 0) or 0)
        age = (timestamp_ns - self.first) / 1e9 if self.first is not None else 0.0
        gap = (timestamp_ns - self.last) / 1e9 if self.last is not None else 86_400.0
        values = [
            math.log1p(max(quote, 0.0)), math.log1p(max(base, 0.0)),
            float(row.get("lp_fee_bps", 0) or 0) + float(row.get("protocol_fee_bps", 0) or 0)
            + float(row.get("coin_creator_bps", 0) or 0),
            math.log1p(min(age, 86_400.0)), math.log1p(min(gap, 86_400.0)),
            0.0 if not self.last_quote or quote <= 0 else math.log1p(quote) - math.log1p(self.last_quote),
        ]
        for window in self.windows:
            values.append(math.log1p(sum(1 for stamp, _ in self.events if stamp >= timestamp_ns - window * 1_000_000_000)))
        for window in self.windows:
            values.append(math.log1p(sum(amount for stamp, amount in self.events if stamp >= timestamp_ns - window * 1_000_000_000)))
        values.append(1.0 if self.last is None else 0.0)
        return values

    def observe(self, timestamp_ns: int, row: dict) -> None:
        if self.first is None:
            self.first = timestamp_ns
        self.events.append((timestamp_ns, float(row.get("user_quote_amount", 0) or 0)))
        self.last = timestamp_ns
        quote = float(row.get("pool_quote_raw", 0) or 0) - float(row.get("pool_quote_delta", 0) or 0)
        if quote > 0:
            self.last_quote = quote


def score(features: list[float], model: dict) -> float:
    required = ("features", "coefficients", "intercept", "standardization_means", "standardization_scales", "threshold")
    if any(field not in model for field in required):
        raise RuntimeError("FAIL_CLOSED: frozen fingerprint missing scoring contract")
    if not (len(features) == len(model["features"]) == len(model["standardization_means"]) == len(model["standardization_scales"])):
        raise RuntimeError("FAIL_CLOSED: frozen fingerprint feature shape mismatch")
    standardized = []
    for value, mean, scale in zip(features, model["standardization_means"], model["standardization_scales"]):
        if float(scale) == 0:
            raise RuntimeError("FAIL_CLOSED: frozen fingerprint zero scale")
        standardized.append((value - float(mean)) / float(scale))
    linear = float(model["intercept"]) + sum(float(model["coefficients"][name]) * value for name, value in zip(model["features"], standardized))
    return 1.0 / (1.0 + math.exp(-max(-60.0, min(60.0, linear))))


def read_rows(paths: Iterable[Path], mechanics) -> list[dict]:
    rows: list[dict] = []
    for path in paths:
        for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
            if not line:
                continue
            try:
                rows.append(json.loads(line))
            except json.JSONDecodeError as error:
                raise RuntimeError(f"FAIL_CLOSED: invalid canonical JSONL {path}:{line_number}") from error
    return mechanics.sorted_unique(rows)


def hard_wall_before(rows: list[dict], signal: dict, until_ns: int, mechanics) -> bool:
    stamp, instance, epoch, *_ = mechanics.availability_key(signal)
    for row in rows:
        received, row_instance, row_epoch, *_ = mechanics.availability_key(row)
        if received < stamp:
            continue
        if received >= until_ns:
            return False
        if row_instance != instance or row_epoch != epoch:
            return True
    return False


def actionability(rows: list[dict], target: dict, mechanics) -> dict:
    entry = mechanics.delayed_entry_state(rows, target, ENTRY_DELAY_NS)
    if entry["status"] != "TRADEABLE":
        deadline = int(target["received_at_unix_ns"]) + ENTRY_DELAY_NS
        reason = str(entry.get("reason", ""))
        outcome = "HARD_WALL_INCOMPLETE_HORIZON" if hard_wall_before(rows, target, deadline, mechanics) else (
            "NO_POST_DELAY_ENTRY_STATE" if reason == "NO_SUPPORTED_POST_DELAY_ENTRY_STATE" else f"POST_DELAY_NON_TRADEABLE_{reason}")
        return {"complete_actionable": False, "outcome_reason": outcome, "entry": entry, "mark": None}
    entry_row = next(row for row in rows if event_id(mechanics, row) == "|".join(map(str, entry["entry_event_key"])))
    mark = mechanics.observed_exit_mark(rows, entry_row, HORIZON_NS)
    if mark["status"] == "MARK_AVAILABLE":
        return {"complete_actionable": True, "outcome_reason": "COMPLETE_ACTIONABLE_TRAJECTORY", "entry": entry, "mark": mark}
    deadline = int(entry_row["received_at_unix_ns"]) + HORIZON_NS
    reason = str(mark.get("reason", ""))
    outcome = "HARD_WALL_INCOMPLETE_HORIZON" if hard_wall_before(rows, entry_row, deadline, mechanics) else (
        "NO_120S_MARK" if reason == "NO_SUPPORTED_EXIT_MARK" else f"POST_ENTRY_NON_TRADEABLE_{reason}")
    return {"complete_actionable": False, "outcome_reason": outcome, "entry": entry, "mark": mark}


def fingerprint_candidates(rows: list[dict], model: dict, boundary_ns: int, mechanics) -> tuple[list[dict], list[dict]]:
    states: dict[tuple[str, int, str], PoolState] = defaultdict(PoolState)
    matches, non_matches = [], []
    for row in rows:
        received, instance, epoch, *_ = mechanics.availability_key(row)
        state = states[(instance, epoch, str(row.get("pool", "")))]
        features = state.snapshot(received, row)
        supported = mechanics.tradeable(row)["tradeable"] and str(row.get("side", "")).upper() == "BUY"
        eligible = supported and not bool(row.get("is_tracked_teacher")) and received >= boundary_ns + WARMUP_NS
        if eligible:
            enriched = {**row, "frozen_fingerprint_score": score(features, model)}
            (matches if enriched["frozen_fingerprint_score"] >= float(model["threshold"]) else non_matches).append(enriched)
        state.observe(received, row)
    return matches, non_matches


def unique_targets(matches: list[dict], mechanics) -> list[dict]:
    retained: dict[tuple[str, int, str, int], dict] = {}
    for row in matches:
        received, instance, epoch, *_ = mechanics.availability_key(row)
        retained.setdefault((instance, epoch, str(row["pool"]), received // 20_000_000_000), row)
    return sorted(retained.values(), key=mechanics.availability_key)


def chronological_block(index: int, total: int) -> str:
    if total <= 0:
        raise RuntimeError("FAIL_CLOSED: no targets")
    return f"block-{(index * CHRONOLOGICAL_BLOCKS) // total + 1:02d}"


def build_assignments(rows: list[dict], model: dict, boundary_ns: int, mechanics, *, control_seed: str) -> list[dict]:
    # Assignment identity must not depend on JSONL enumeration order after a
    # restart; canonical source order is the only permitted input order.
    rows = mechanics.sorted_unique(rows)
    matches, non_matches = fingerprint_candidates(rows, model, boundary_ns, mechanics)
    targets = unique_targets(matches, mechanics)
    controls = mechanics.deterministic_controls(targets, non_matches, bucket_ns=mechanics.CONTROL_BUCKET_NS,
                                                per_target=MAX_CONTROLS, seed=control_seed)
    assignments = []
    for index, target in enumerate(targets):
        target_outcome = actionability(rows, target, mechanics)
        selected_controls = []
        for control in controls.get(mechanics.event_key(target), []):
            control_outcome = actionability(rows, control, mechanics)
            selected_controls.append({
                "control_id": event_id(mechanics, control),
                "complete_actionable": control_outcome["complete_actionable"],
                "outcome_reason": control_outcome["outcome_reason"],
            })
        assignments.append({
            "schema": SCHEMA,
            "target_id": event_id(mechanics, target),
            "target_event_key": list(mechanics.event_key(target)),
            "pool": str(target["pool"]), "mint": str(target["mint"]),
            "cluster_id": mechanics.cluster_key(target, mechanics.CONTROL_BUCKET_NS),
            "chronological_block": chronological_block(index, len(targets)),
            "target_complete_actionable": target_outcome["complete_actionable"],
            "target_outcome_reason": target_outcome["outcome_reason"],
            "controls": selected_controls,
            "control_count": len(selected_controls),
            "fingerprint_score": target["frozen_fingerprint_score"],
            "pnl_accessed": False,
        })
    return assignments


def validate_receipt(receipt_path: Path) -> dict:
    receipt = json.loads(receipt_path.read_text())
    validation = receipt.get("scientific_validation")
    if not isinstance(validation, dict) or int(validation.get("rows", 0)) <= 0:
        raise RuntimeError(f"FAIL_CLOSED: unverified scientific receipt {receipt_path}")
    rows = Path(str(receipt.get("market_rows_path", "")))
    if not rows.is_file() or sha256(rows) != str(receipt.get("market_rows_sha256", "")):
        raise RuntimeError(f"FAIL_CLOSED: receipt row binding mismatch {receipt_path}")
    return {"receipt_path": str(receipt_path), "receipt_sha256": sha256(receipt_path),
            "market_rows_path": str(rows), "market_rows_sha256": sha256(rows)}


def utc_from_ns(value: int) -> str:
    from datetime import datetime, timezone
    seconds, nanos = divmod(value, 1_000_000_000)
    base = datetime.fromtimestamp(seconds, tz=timezone.utc).strftime("%Y-%m-%dT%H:%M:%S")
    return f"{base}.{nanos:09d}Z"


def assert_epoch_eligibility(rows: list[dict], registry_script: Path, registry_root: Path,
                             boundary_utc: str) -> None:
    registry = load_module(registry_script, "kefi_preh3_epoch_registry")
    for row in rows:
        registry.eligible(registry_root, utc_from_ns(int(row["received_at_unix_ns"])), boundary_utc)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--fingerprint", type=Path, required=True)
    parser.add_argument("--boundary-receipt", type=Path, required=True)
    parser.add_argument("--scientific-receipt", type=Path, action="append", required=True)
    parser.add_argument("--registry-script", type=Path, required=True)
    parser.add_argument("--registry-root", type=Path, required=True)
    parser.add_argument("--mechanics", type=Path, required=True)
    parser.add_argument("--evaluator", type=Path, required=True)
    parser.add_argument("--out-assignments", type=Path, required=True)
    parser.add_argument("--out-manifest", type=Path, required=True)
    parser.add_argument("--control-seed", required=True)
    args = parser.parse_args()
    if sha256(args.fingerprint) != EXPECTED_FINGERPRINT_SHA256:
        raise SystemExit("FAIL_CLOSED: frozen fingerprint hash mismatch")
    boundary = json.loads(args.boundary_receipt.read_text())
    if boundary.get("status") not in {"ACTIVE", "FROZEN"} or not boundary.get("boundaryUtc"):
        raise SystemExit("FAIL_CLOSED: invalid scientific boundary receipt")
    mechanics = load_module(args.mechanics, "kefi_preh3_mechanics")
    receipts = [validate_receipt(path) for path in args.scientific_receipt]
    paths = [Path(item["market_rows_path"]) for item in receipts]
    model = json.loads(args.fingerprint.read_text())
    if model.get("status") != "FROZEN_FOR_PROSPECTIVE_OUTCOME_BLIND_SHADOW" or not model.get("strict_preentry"):
        raise SystemExit("FAIL_CLOSED: invalid frozen fingerprint artifact")
    rows = read_rows(paths, mechanics)
    assert_epoch_eligibility(rows, args.registry_script, args.registry_root, str(boundary["boundaryUtc"]))
    assignments = build_assignments(rows, model, parse_utc_ns(str(boundary["boundaryUtc"])), mechanics,
                                    control_seed=args.control_seed)
    if not assignments:
        raise SystemExit("FAIL_CLOSED: no pre-entry eligible fingerprint targets")
    args.out_assignments.parent.mkdir(parents=True, exist_ok=True)
    temporary = args.out_assignments.with_suffix(args.out_assignments.suffix + ".tmp")
    with temporary.open("w", encoding="utf-8") as handle:
        for assignment in assignments:
            handle.write(json.dumps(assignment, sort_keys=True, separators=(",", ":")) + "\n")
        handle.flush(); os.fsync(handle.fileno())
    os.replace(temporary, args.out_assignments)
    manifest = {
        "schema": SCHEMA, "status": "ASSIGNMENTS_BUILT_NOT_EVALUATED", "outcomes_evaluated": False,
        "pnl_accessed": False, "boundary_receipt_path": str(args.boundary_receipt),
        "boundary_receipt_sha256": sha256(args.boundary_receipt), "boundary_utc": boundary["boundaryUtc"],
        "fingerprint_path": str(args.fingerprint), "fingerprint_sha256": sha256(args.fingerprint),
        "scientific_receipts": receipts, "mechanics_path": str(args.mechanics), "mechanics_sha256": sha256(args.mechanics),
        "epoch_registry_path": str(args.registry_root), "epoch_registry_script": str(args.registry_script),
        "epoch_registry_sha256": sha256(args.registry_script),
        "assignment_builder_path": str(Path(__file__)), "assignment_builder_sha256": sha256(Path(__file__)),
        "evaluator_path": str(args.evaluator), "evaluator_sha256": sha256(args.evaluator),
        "control_seed_sha256": hashlib.sha256(args.control_seed.encode()).hexdigest(),
        "assignments_path": str(args.out_assignments), "assignments_sha256": sha256(args.out_assignments),
        "targets_all": len(assignments), "targets_with_controls": sum(bool(a["controls"]) for a in assignments),
        "chronological_blocks": sorted({a["chronological_block"] for a in assignments}),
    }
    atomic(args.out_manifest, manifest)
    print(json.dumps({"status": manifest["status"], "assignments": len(assignments),
                      "assignments_sha256": manifest["assignments_sha256"], "manifest_sha256": sha256(args.out_manifest)}, sort_keys=True))


if __name__ == "__main__":
    main()
