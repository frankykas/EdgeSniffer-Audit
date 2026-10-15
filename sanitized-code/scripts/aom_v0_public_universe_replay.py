#!/usr/bin/env python3
"""AOM V0: bounded offline public-universe replay primitives.

This module is deliberately *not* a strategy, a paper lane, or a live
collector.  It accepts only provenance-qualified public market-state records.
It has no wallet fields, label fields, PnL fields, or RPC client.  A partition
is rejected unless capture coverage and the historical eligible-pool universe
are proven separately.
"""
from __future__ import annotations

import hashlib
import json
import math
import os
import pathlib
import shutil
import tempfile
import time
from collections import defaultdict, deque
from dataclasses import dataclass
from typing import Any, Iterable


INPUT_SCHEMA = "AOM_V0_PUBLIC_INPUT_CONTRACT_V1"
PARTITION_SCHEMA = "AOM_V0_PARTITION_ADMISSION_V1"
DECISION_BOUNDARY_SCHEMA = "AOM_V0_PUBLIC_DECISION_BOUNDARY_V1"
EXECUTION_SCHEMA = "AOM_V0_EXECUTION_ASSUMPTIONS_V1"
OUTPUT_SCHEMA = "AOM_V0_PUBLIC_REPLAY_OUTPUT_V1"
FEATURE_NAMES = (
    "F01_log_pre_pool_quote", "F02_log_pre_pool_base", "F03_amm_fee_bps",
    "F04_observed_pool_age_seconds", "F05_seconds_since_previous_observed_buy",
    "F06_prior_pre_pool_quote_log_change", "F07_prior_buy_count_5s",
    "F08_prior_buy_count_20s", "F09_prior_buy_count_60s",
    "F10_prior_buy_quote_5s", "F11_prior_buy_quote_20s",
    "F12_prior_buy_quote_60s", "F13_no_prior_observed_buy",
)
WINDOWS_NS = (5_000_000_000, 20_000_000_000, 60_000_000_000)
EXPLORATORY_HORIZONS_SECONDS = (30, 60, 120)


class AdmissionBlocked(ValueError):
    """The evidence cannot support an autonomous replay claim."""


class BudgetExceeded(RuntimeError):
    """The bounded job must stop before consuming its protected reserve."""


def canonical_bytes(value: Any) -> bytes:
    """Stable compact JSON for local contract/receipt identity.

    Inputs are deliberately restricted to JSON primitives; NaN and infinities
    are rejected so hashes cannot depend on platform float encodings.
    """
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True,
                      allow_nan=False).encode("utf-8")


def sha256(value: Any) -> str:
    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def require_keys(row: dict[str, Any], keys: Iterable[str], reason: str) -> None:
    missing = [key for key in keys if key not in row]
    if missing:
        raise AdmissionBlocked(f"{reason}:{','.join(missing)}")


def event_identity(row: dict[str, Any]) -> tuple:
    require_keys(row, ("source_instance_id", "continuity_epoch", "slot", "transaction_index",
                       "instruction_path", "semantic_ordinal", "signature", "event_id"),
                 "MISSING_CAUSAL_IDENTITY")
    path = row["instruction_path"]
    if not isinstance(path, list) or not all(isinstance(item, int) and item >= 0 for item in path):
        raise AdmissionBlocked("INVALID_INSTRUCTION_PATH")
    fields = (str(row["source_instance_id"]), int(row["continuity_epoch"]), int(row["slot"]),
              int(row["transaction_index"]), tuple(path), int(row["semantic_ordinal"]),
              str(row["signature"]), str(row["event_id"]))
    if not fields[0] or min(fields[1], fields[2], fields[3], fields[5]) < 0 or not fields[6] or not fields[7]:
        raise AdmissionBlocked("INVALID_CAUSAL_IDENTITY")
    return fields


def causal_key(row: dict[str, Any]) -> tuple:
    identity = event_identity(row)
    return identity[2], identity[3], identity[4], identity[5], identity[6], identity[7]


def public_event_time(row: dict[str, Any]) -> int:
    require_keys(row, ("observed_at_unix_ns",), "MISSING_PUBLIC_TIME")
    value = int(row["observed_at_unix_ns"])
    if value < 0:
        raise AdmissionBlocked("INVALID_PUBLIC_TIME")
    return value


def validate_input_contract(row: dict[str, Any]) -> None:
    """Reject expert-linked or post-decision fields at the public boundary."""
    if row.get("schema") != INPUT_SCHEMA:
        raise AdmissionBlocked("INPUT_SCHEMA_MISMATCH")
    prohibited = {"wallet", "wallet_action", "teacher_label", "expert_selected", "pnl",
                  "post_entry_price", "future_price", "outcome", "holding"}
    present = sorted(prohibited.intersection(row))
    if present:
        raise AdmissionBlocked("PROHIBITED_NONPUBLIC_FIELD:" + ",".join(present))
    event_identity(row); public_event_time(row)
    require_keys(row, ("pool", "mint", "quote_mint", "pool_base_pre_raw", "pool_quote_pre_raw",
                       "amm_fee_bps", "side", "quote_amount_raw", "pool_identity_proof_ref",
                       "causal_proof_ref", "feature_validity"), "MISSING_PUBLIC_STATE")
    if not str(row["pool"]) or not str(row["mint"]) or int(row["pool_base_pre_raw"]) <= 0 or int(row["pool_quote_pre_raw"]) <= 0:
        raise AdmissionBlocked("INVALID_PUBLIC_POOL_STATE")
    if int(row["amm_fee_bps"]) < 0 or int(row["amm_fee_bps"]) >= 10_000:
        raise AdmissionBlocked("INVALID_FEE")
    if not isinstance(row["feature_validity"], dict):
        raise AdmissionBlocked("INVALID_FEATURE_VALIDITY")


def validate_partition_admission(proof: dict[str, Any]) -> dict[str, Any]:
    """Validate independent capture and universe claims; neither substitutes for the other."""
    if proof.get("schema") != PARTITION_SCHEMA:
        raise AdmissionBlocked("PARTITION_SCHEMA_MISMATCH")
    require_keys(proof, ("partition_id", "capture_start_unix_ns", "capture_end_unix_ns",
                         "transaction_coverage_verified", "transaction_coverage_proof_ref",
                         "canonical_instruction_order_verified", "causal_order_proof_ref",
                         "complete_eligible_pool_universe_verified", "universe_proof_ref",
                         "source_instance_id", "continuity_epoch", "manifest_sha256"),
                 "MISSING_PARTITION_PROOF")
    if int(proof["capture_end_unix_ns"]) <= int(proof["capture_start_unix_ns"]):
        raise AdmissionBlocked("INVALID_PARTITION_INTERVAL")
    gates = (
        ("transaction_coverage_verified", "INCOMPLETE_CAPTURED_TRANSACTION_INTERVAL"),
        ("canonical_instruction_order_verified", "CANONICAL_ORDER_UNPROVEN"),
        ("complete_eligible_pool_universe_verified", "ELIGIBLE_POOL_UNIVERSE_INCOMPLETE"),
    )
    for field, reason in gates:
        if proof[field] is not True:
            raise AdmissionBlocked(reason)
    for field in ("transaction_coverage_proof_ref", "causal_order_proof_ref", "universe_proof_ref", "manifest_sha256"):
        if not isinstance(proof[field], str) or not proof[field]:
            raise AdmissionBlocked("MISSING_IMMUTABLE_PROOF_REF:" + field)
    return {"partition_id": str(proof["partition_id"]), "admitted": True,
            "admission_hash": sha256(proof)}


def validate_decision_boundary(boundary: dict[str, Any], partition_proof: dict[str, Any]) -> dict[str, Any]:
    """Bind a public decision clock to one independently admitted partition."""
    admission = validate_partition_admission(partition_proof)
    if boundary.get("schema") != DECISION_BOUNDARY_SCHEMA:
        raise AdmissionBlocked("DECISION_BOUNDARY_SCHEMA_MISMATCH")
    require_keys(boundary, ("decision_boundary_id", "partition_id", "decision_at_unix_ns",
                            "source_instance_id", "continuity_epoch", "causal_checkpoint",
                            "state_proof_ref", "coverage_proof_ref", "universe_proof_ref"),
                 "MISSING_DECISION_BOUNDARY_FIELD")
    if str(boundary["partition_id"]) != admission["partition_id"]:
        raise AdmissionBlocked("DECISION_BOUNDARY_PARTITION_MISMATCH")
    if int(boundary["decision_at_unix_ns"]) < int(partition_proof["capture_start_unix_ns"]) or int(boundary["decision_at_unix_ns"]) > int(partition_proof["capture_end_unix_ns"]):
        raise AdmissionBlocked("DECISION_BOUNDARY_OUTSIDE_PARTITION")
    if str(boundary["source_instance_id"]) != str(partition_proof["source_instance_id"]) or int(boundary["continuity_epoch"]) != int(partition_proof["continuity_epoch"]):
        raise AdmissionBlocked("DECISION_BOUNDARY_EPOCH_MISMATCH")
    if not isinstance(boundary["causal_checkpoint"], dict) or not boundary["causal_checkpoint"]:
        raise AdmissionBlocked("MISSING_CAUSAL_CHECKPOINT")
    for field in ("state_proof_ref", "coverage_proof_ref", "universe_proof_ref"):
        if not isinstance(boundary[field], str) or not boundary[field]:
            raise AdmissionBlocked("MISSING_DECISION_BOUNDARY_PROOF:" + field)
    return {"decision_boundary_id": str(boundary["decision_boundary_id"]), "boundary_hash": sha256(boundary), **admission}


def inventory_partition_proofs(proofs: Iterable[dict[str, Any]]) -> list[dict[str, str]]:
    """Read-only inventory: returns one admission result per supplied manifest."""
    results = []
    for proof in proofs:
        partition = str(proof.get("partition_id", "UNKNOWN"))
        try:
            result = validate_partition_admission(proof)
            results.append({"partition_id": partition, "status": "ADMITTED", "reason": "", **result})
        except AdmissionBlocked as error:
            results.append({"partition_id": partition, "status": "BLOCKED", "reason": str(error)})
    return results


def _clean_history(history: deque[dict[str, Any]], now_ns: int) -> None:
    while history and public_event_time(history[0]) < now_ns - WINDOWS_NS[-1]:
        history.popleft()


def public_f01_f13(rows: Iterable[dict[str, Any]]) -> list[dict[str, Any]]:
    """Compute strict public analogues of F01-F13 from causally ordered rows.

    The function does not fill unavailable history with zero.  Each value has a
    corresponding mask, allowing later evaluation to reject invalid candidates.
    """
    # State mutation follows final Solana causal order, not delivery order.  A
    # non-monotone availability clock makes a pre-decision public-history claim
    # unprovable for this compact V0 engine, so it fails closed.
    ordered = sorted(rows, key=causal_key)
    seen_ids: set[tuple] = set(); first_seen: dict[str, int] = {}; buys: dict[str, deque[dict[str, Any]]] = defaultdict(deque)
    last_buy_time: dict[str, int] = {}; last_quote: dict[str, int] = {}
    output: list[dict[str, Any]] = []
    prior_observed_at = -1
    for row in ordered:
        validate_input_contract(row)
        identity = event_identity(row)
        if identity in seen_ids:
            raise AdmissionBlocked("DUPLICATE_PUBLIC_EVENT")
        seen_ids.add(identity)
        now_ns, pool = public_event_time(row), str(row["pool"])
        if now_ns < prior_observed_at:
            raise AdmissionBlocked("NONMONOTONIC_PUBLIC_AVAILABILITY_CLOCK")
        prior_observed_at = now_ns
        first_seen.setdefault(pool, now_ns)
        history = buys[pool]; _clean_history(history, now_ns)
        is_buy = str(row["side"]).upper() == "BUY"
        valid = dict(row["feature_validity"])
        history_complete = all(valid.get(key) is True for key in ("state", "pool_age", "flow_5s", "flow_20s", "flow_60s"))
        pre_quote, pre_base = int(row["pool_quote_pre_raw"]), int(row["pool_base_pre_raw"])
        prior_quote = last_quote.get(pool)
        values = {
            "F01_log_pre_pool_quote": math.log(pre_quote),
            "F02_log_pre_pool_base": math.log(pre_base),
            "F03_amm_fee_bps": int(row["amm_fee_bps"]),
            "F04_observed_pool_age_seconds": (now_ns - first_seen[pool]) / 1_000_000_000,
            "F05_seconds_since_previous_observed_buy": None if pool not in last_buy_time else (now_ns - last_buy_time[pool]) / 1_000_000_000,
            "F06_prior_pre_pool_quote_log_change": None if prior_quote is None else math.log(pre_quote) - math.log(prior_quote),
            "F13_no_prior_observed_buy": len(history) == 0,
        }
        for window, count_name, amount_name in zip(WINDOWS_NS, FEATURE_NAMES[6:9], FEATURE_NAMES[9:12]):
            eligible = [item for item in history if public_event_time(item) >= now_ns - window]
            values[count_name] = len(eligible)
            values[amount_name] = sum(int(item["quote_amount_raw"]) for item in eligible)
        masks = {name: history_complete for name in FEATURE_NAMES}
        masks["F05_seconds_since_previous_observed_buy"] = history_complete and pool in last_buy_time
        masks["F06_prior_pre_pool_quote_log_change"] = history_complete and prior_quote is not None
        output.append({"event_identity": identity, "pool": pool, "mint": str(row["mint"]),
                       "decision_at_unix_ns": now_ns, "feature_values": values,
                       "feature_validity": masks, "source_row_hash": sha256(row)})
        last_quote[pool] = pre_quote
        if is_buy:
            history.append(row); last_buy_time[pool] = now_ns
    return output


def deterministic_rank(candidates: Iterable[dict[str, Any]], *, top_k: int, min_score: float = float("-inf")) -> dict[str, Any]:
    """Transparent baseline ranker with stable ties and a genuine no-trade result."""
    if top_k <= 0:
        raise ValueError("top_k must be positive")
    candidate_rows = list(candidates)
    scored = []
    rejections = []
    for item in candidate_rows:
        values, validity = item.get("feature_values", {}), item.get("feature_validity", {})
        if not all(validity.get(name) is True for name in FEATURE_NAMES):
            rejections.append({"event_identity": item.get("event_identity"), "reason": "INVALID_PUBLIC_FEATURE_HISTORY"}); continue
        # Fixed transparent heuristic: depth and recent net public buy pressure,
        # penalized by fee. It has no wallet or learned coefficient dependency.
        score = (0.30 * float(values["F01_log_pre_pool_quote"])
                 + 0.15 * float(values["F02_log_pre_pool_base"])
                 + 0.04 * math.log1p(float(values["F11_prior_buy_quote_20s"]))
                 + 0.02 * float(values["F08_prior_buy_count_20s"])
                 - 0.002 * float(values["F03_amm_fee_bps"]))
        scored.append({**item, "baseline_score": score})
    ordered = sorted(scored, key=lambda item: (-item["baseline_score"], str(item["pool"]), str(item["mint"]), repr(item["event_identity"])))
    selected = [item for item in ordered if item["baseline_score"] >= min_score][:top_k]
    return {"schema": OUTPUT_SCHEMA, "denominator_count": len(candidate_rows), "eligible_count": len(scored),
            "rejections": rejections, "ranked": ordered, "selected": selected,
            "no_trade": len(selected) == 0, "ranker_version": "AOM_V0_TRANSPARENT_BASELINE_V1"}


@dataclass(frozen=True)
class ExecutionAssumptions:
    entry_notional_quote_raw: int
    network_fee_quote_raw: int
    priority_fee_quote_raw: int
    failed_transaction_cost_quote_raw: int
    entry_latency_ns: int
    exit_latency_ns: int
    cost_evidence_ref: str
    schema: str = EXECUTION_SCHEMA

    def validate(self) -> None:
        if self.schema != EXECUTION_SCHEMA or not self.cost_evidence_ref:
            raise AdmissionBlocked("MISSING_EXECUTION_COST_PROOF")
        if min(self.entry_notional_quote_raw, self.network_fee_quote_raw, self.priority_fee_quote_raw,
               self.failed_transaction_cost_quote_raw, self.entry_latency_ns, self.exit_latency_ns) < 0:
            raise AdmissionBlocked("INVALID_EXECUTION_ASSUMPTION")
        if self.entry_notional_quote_raw <= 0:
            raise AdmissionBlocked("NONPOSITIVE_ENTRY_NOTIONAL")


def _constant_product_exact_input(base: int, quote: int, input_amount: int, fee_bps: int, *, direction: str) -> tuple[int, int]:
    if min(base, quote, input_amount) <= 0 or not 0 <= fee_bps < 10_000:
        raise AdmissionBlocked("INVALID_EXACT_TRANSITION_INPUT")
    net = input_amount * (10_000 - fee_bps) // 10_000
    fee = input_amount - net
    if net <= 0:
        raise AdmissionBlocked("ZERO_NET_SWAP_INPUT")
    if direction == "BUY":
        return base * net // (quote + net), fee
    if direction == "SELL":
        return quote * net // (base + net), fee
    raise AdmissionBlocked("UNSUPPORTED_TRANSITION_DIRECTION")


def simulate_exact_supported_transition(entry: dict[str, Any], exit_mark: dict[str, Any] | None,
                                        assumptions: ExecutionAssumptions) -> dict[str, Any]:
    """Bounded cash-flow ledger; AMM fees are embedded in exact swap output once."""
    assumptions.validate()
    if entry.get("transition_model") != "CONSTANT_PRODUCT_EXACT_INPUT_V1" or entry.get("exact_transition_validated") is not True:
        raise AdmissionBlocked("UNSUPPORTED_OR_UNVALIDATED_TRANSITION")
    if int(entry.get("entry_available_at_unix_ns", -1)) < int(entry.get("decision_at_unix_ns", 0)) + assumptions.entry_latency_ns:
        return {"status": "NO_FILL", "reason": "ENTRY_LATENCY_UNMET", "net_quote_raw": -assumptions.failed_transaction_cost_quote_raw,
                "cash_ledger": {"failed_transaction_cost": assumptions.failed_transaction_cost_quote_raw}}
    base_out, entry_amm_fee = _constant_product_exact_input(int(entry["pool_base_pre_raw"]), int(entry["pool_quote_pre_raw"]),
                                                             assumptions.entry_notional_quote_raw, int(entry["amm_fee_bps"]), direction="BUY")
    if base_out <= 0:
        return {"status": "NO_FILL", "reason": "ZERO_BASE_OUTPUT", "net_quote_raw": -assumptions.failed_transaction_cost_quote_raw,
                "cash_ledger": {"failed_transaction_cost": assumptions.failed_transaction_cost_quote_raw}}
    if exit_mark is None:
        return {"status": "NO_FILL", "reason": "MISSING_HORIZON_MARK", "net_quote_raw": -assumptions.failed_transaction_cost_quote_raw,
                "cash_ledger": {"failed_transaction_cost": assumptions.failed_transaction_cost_quote_raw}}
    if exit_mark.get("transition_model") != "CONSTANT_PRODUCT_EXACT_INPUT_V1" or exit_mark.get("exact_transition_validated") is not True:
        raise AdmissionBlocked("UNSUPPORTED_OR_UNVALIDATED_EXIT_TRANSITION")
    if int(exit_mark.get("mark_available_at_unix_ns", -1)) < int(exit_mark.get("target_exit_at_unix_ns", 0)) + assumptions.exit_latency_ns:
        return {"status": "NO_FILL", "reason": "EXIT_LATENCY_UNMET", "net_quote_raw": -assumptions.failed_transaction_cost_quote_raw,
                "cash_ledger": {"failed_transaction_cost": assumptions.failed_transaction_cost_quote_raw}}
    quote_out, exit_amm_fee = _constant_product_exact_input(int(exit_mark["pool_base_pre_raw"]), int(exit_mark["pool_quote_pre_raw"]),
                                                             base_out, int(exit_mark["amm_fee_bps"]), direction="SELL")
    explicit_cost = 2 * (assumptions.network_fee_quote_raw + assumptions.priority_fee_quote_raw)
    net = quote_out - assumptions.entry_notional_quote_raw - explicit_cost - assumptions.failed_transaction_cost_quote_raw
    return {"status": "FILLED", "net_quote_raw": net,
            "cash_ledger": {"entry_notional": assumptions.entry_notional_quote_raw, "entry_amm_fee_embedded": entry_amm_fee,
                            "exit_amm_fee_embedded": exit_amm_fee, "exit_proceeds": quote_out,
                            "network_and_priority_cost": explicit_cost, "failed_transaction_cost": assumptions.failed_transaction_cost_quote_raw},
            "base_received_raw": base_out}


def fixed_horizon_marks(entry: dict[str, Any], marks_by_seconds: dict[int, dict[str, Any] | None],
                        assumptions: ExecutionAssumptions) -> dict[int, dict[str, Any]]:
    """Evaluate only the three preregistered exploratory horizons.

    This helper deliberately receives marks rather than searching a future
    stream.  Search/window construction belongs to a provenance-qualified
    replay adapter, so an analyst cannot silently alter horizon semantics here.
    """
    if set(marks_by_seconds) != set(EXPLORATORY_HORIZONS_SECONDS):
        raise AdmissionBlocked("EXPLORATORY_HORIZON_SET_MISMATCH")
    return {seconds: simulate_exact_supported_transition(entry, marks_by_seconds[seconds], assumptions)
            for seconds in EXPLORATORY_HORIZONS_SECONDS}


def chronological_evaluate(partition_proof: dict[str, Any], decisions: Iterable[dict[str, Any]], *, budget: "ResourceBudget") -> dict[str, Any]:
    """Return blocked results unless real-data provenance and cost evidence pass."""
    admission = validate_partition_admission(partition_proof)
    start = time.monotonic(); cpu_started = time.process_time(); rows = list(decisions)
    if len(rows) > budget.max_rows:
        raise BudgetExceeded("MAX_ROWS_EXCEEDED")
    if time.monotonic() - start > budget.max_runtime_seconds:
        raise BudgetExceeded("RUNTIME_BUDGET_EXCEEDED")
    if time.process_time() - cpu_started > budget.max_cpu_seconds:
        raise BudgetExceeded("CPU_BUDGET_EXCEEDED")
    for row in rows:
        validate_decision_boundary(row.get("decision_boundary", {}), partition_proof)
        if row.get("cost_evidence_ref") in (None, ""):
            return {"status": "BLOCKED", "reason": "MISSING_INDEPENDENT_EXECUTION_COST_EVIDENCE", "admission": admission}
    return {"status": "READY_FOR_EXPLORATORY_EVALUATION", "admission": admission, "decision_count": len(rows)}


@dataclass(frozen=True)
class ResourceBudget:
    max_rows: int = 500_000
    max_output_bytes: int = 64 * 1024 * 1024
    min_free_bytes: int = 20 * 1024 * 1024 * 1024
    max_runtime_seconds: float = 900.0
    max_cpu_seconds: float = 900.0
    max_rpc_requests: int = 0


def write_compact_output(path: pathlib.Path, rows: Iterable[dict[str, Any]], budget: ResourceBudget) -> dict[str, Any]:
    """Write references only and stop before a quota/reserve violation."""
    path.parent.mkdir(parents=True, exist_ok=True)
    # ``disk_usage`` preserves the same protected-reserve invariant on Windows
    # and Linux without making this offline verifier host-platform specific.
    free = shutil.disk_usage(path.parent).free
    if free < budget.min_free_bytes:
        raise BudgetExceeded("PROTECTED_FREE_SPACE_RESERVE")
    if budget.max_rpc_requests != 0:
        raise BudgetExceeded("AOM_V0_OFFLINE_RPC_BUDGET_MUST_BE_ZERO")
    started = time.monotonic(); cpu_started = time.process_time(); written = 0; count = 0
    temporary = None
    try:
        with tempfile.NamedTemporaryFile("xb", dir=path.parent, delete=False) as handle:
            temporary = pathlib.Path(handle.name)
            for row in rows:
                payload = canonical_bytes(row) + b"\n"
                if count >= budget.max_rows or written + len(payload) > budget.max_output_bytes:
                    raise BudgetExceeded("OUTPUT_QUOTA_EXCEEDED")
                if time.monotonic() - started > budget.max_runtime_seconds:
                    raise BudgetExceeded("RUNTIME_BUDGET_EXCEEDED")
                if time.process_time() - cpu_started > budget.max_cpu_seconds:
                    raise BudgetExceeded("CPU_BUDGET_EXCEEDED")
                handle.write(payload); written += len(payload); count += 1
            handle.flush(); os.fsync(handle.fileno())
        os.replace(temporary, path)
    except Exception:
        if temporary:
            temporary.unlink(missing_ok=True)
        raise
    return {"rows": count, "bytes": written, "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
