#!/usr/bin/env python3
"""Exact, fail-closed mechanics primitives for a future kEfi H3 paper lane.

This module is not an H3 strategy and never opens a cohort.  It only defines
the evidence rules a reviewed H3 protocol must use: writer-sequence ordering,
canonical SOL-quote PumpSwap layouts, delayed entry state, observed exit mark,
deterministic controls, and dependence clusters.  It intentionally refuses to
invent hypothetical fills from BPS fields.
"""
from __future__ import annotations

import hashlib
import json
from collections import defaultdict
from dataclasses import dataclass
from typing import Iterable


SCHEMA = "KEFI_H3_PUMPSWAP_MECHANICS_V1"
WSOL = "So11111111111111111111111111111111111111112"


class NonTradeable(ValueError):
    pass


def event_key(row: dict) -> tuple[str, int, int, int, str]:
    try:
        epoch = int(row["continuity_epoch"])
        sequence = int(row["durable_output_sequence"])
        event_index = int(row["event_index"])
        frame = str(row["source_frame_sha256"])
        instance = str(row["source_instance_id"])
    except (KeyError, TypeError, ValueError) as error:
        raise NonTradeable("MISSING_DETERMINISTIC_SOURCE_ORDER") from error
    if epoch < 0 or sequence <= 0 or event_index < 0 or not frame or not instance:
        raise NonTradeable("INVALID_DETERMINISTIC_SOURCE_ORDER")
    return instance, epoch, sequence, event_index, frame


def availability_key(row: dict) -> tuple[int, str, int, int, int, str]:
    try:
        received = int(row["received_at_unix_ns"])
    except (KeyError, TypeError, ValueError) as error:
        raise NonTradeable("MISSING_AVAILABILITY_CLOCK") from error
    instance, epoch, sequence, event_index, frame = event_key(row)
    return received, instance, epoch, sequence, event_index, frame


def pre_state(row: dict) -> dict[str, int]:
    """Recover the pre-event state from an observed exact transition only."""
    try:
        if row.get("exact_transition_validated") is not True:
            raise NonTradeable("UNVALIDATED_PUMPSWAP_TRANSITION")
        side = str(row["side"]).upper()
        base = int(row["pool_base_raw"])
        quote = int(row["pool_quote_raw"])
        amount = int(row["base_amount"])
        delta = int(row["pool_quote_delta"])
    except (KeyError, TypeError, ValueError) as error:
        raise NonTradeable("MISSING_RAW_PUMPSWAP_TRANSITION") from error
    if min(base, quote, amount, delta) < 0:
        raise NonTradeable("NEGATIVE_RAW_PUMPSWAP_TRANSITION")
    if side == "BUY":
        if quote < delta:
            raise NonTradeable("BUY_PRE_QUOTE_UNDERFLOW")
        return {"pool_base_raw": base + amount, "pool_quote_raw": quote - delta}
    if side == "SELL":
        if base < amount:
            raise NonTradeable("SELL_PRE_BASE_UNDERFLOW")
        return {"pool_base_raw": base - amount, "pool_quote_raw": quote + delta}
    raise NonTradeable("UNSUPPORTED_PUMPSWAP_SIDE")


def tradeable(row: dict) -> dict[str, object]:
    """Return a canonical layout or one stable fail-closed rejection reason."""
    try:
        key = event_key(row)
        if str(row.get("quote_mint", "")) != WSOL or str(row.get("mint", "")) == WSOL:
            raise NonTradeable("NONCANONICAL_QUOTE_LAYOUT")
        if str(row.get("amount_mode", "")) not in {"EXACT_BASE", "EXACT_QUOTE_IN"}:
            raise NonTradeable("UNSUPPORTED_PUMPSWAP_AMOUNT_MODE")
        state = pre_state(row)
        return {"tradeable": True, "reason": "", "event_key": key, "pre_state": state}
    except NonTradeable as error:
        return {"tradeable": False, "reason": str(error), "event_key": None, "pre_state": None}


def sorted_unique(rows: Iterable[dict]) -> list[dict]:
    ordered = sorted(rows, key=availability_key)
    previous = None
    for row in ordered:
        key = event_key(row)
        if key == previous:
            raise NonTradeable("DUPLICATE_DETERMINISTIC_EVENT_ORDER")
        previous = key
    return ordered


def delayed_entry_state(rows: Iterable[dict], signal: dict, entry_delay_ns: int) -> dict:
    if entry_delay_ns <= 0:
        raise NonTradeable("NONPOSITIVE_ENTRY_DELAY")
    signal_time, instance, epoch, _, _, _ = availability_key(signal)
    target = signal_time + entry_delay_ns
    pool, mint = str(signal["pool"]), str(signal["mint"])
    for row in sorted_unique(rows):
        received, row_instance, row_epoch, *_ = availability_key(row)
        if row_instance != instance or row_epoch != epoch:
            continue
        if received < target or str(row.get("pool")) != pool or str(row.get("mint")) != mint:
            continue
        result = tradeable(row)
        if result["tradeable"]:
            return {"status": "TRADEABLE", "entry_available_at_unix_ns": received,
                    "entry_event_key": result["event_key"], "pre_state": result["pre_state"]}
        return {"status": "NON_TRADEABLE", "reason": result["reason"]}
    return {"status": "NON_TRADEABLE", "reason": "NO_SUPPORTED_POST_DELAY_ENTRY_STATE"}


def observed_exit_mark(rows: Iterable[dict], entry: dict, exit_after_ns: int) -> dict:
    """Find a same-epoch observed state mark; never synthesize an exit fill."""
    if exit_after_ns <= 0:
        raise NonTradeable("NONPOSITIVE_EXIT_DELAY")
    entry_time, instance, epoch, _, _, _ = availability_key(entry)
    target = entry_time + exit_after_ns
    pool, mint = str(entry["pool"]), str(entry["mint"])
    for row in sorted_unique(rows):
        received, row_instance, row_epoch, *_ = availability_key(row)
        if row_instance != instance or row_epoch != epoch or received < target:
            continue
        if str(row.get("pool")) != pool or str(row.get("mint")) != mint:
            continue
        result = tradeable(row)
        if result["tradeable"]:
            return {"status": "MARK_AVAILABLE", "mark_available_at_unix_ns": received,
                    "mark_event_key": result["event_key"], "pre_state": result["pre_state"]}
        return {"status": "NON_TRADEABLE", "reason": result["reason"]}
    return {"status": "NON_TRADEABLE", "reason": "NO_SUPPORTED_EXIT_MARK"}


def cluster_key(row: dict, bucket_ns: int) -> str:
    if bucket_ns <= 0:
        raise ValueError("bucket_ns must be positive")
    received, instance, epoch, *_ = availability_key(row)
    return f"source={instance}|epoch={epoch}|pool={row['pool']}|bucket={received // bucket_ns}"


def deterministic_controls(targets: Iterable[dict], candidates: Iterable[dict], *, bucket_ns: int,
                           per_target: int, seed: str) -> dict[tuple, list[dict]]:
    """Choose within-epoch, same-layout, different-mint controls reproducibly."""
    if per_target <= 0:
        raise ValueError("per_target must be positive")
    indexed: dict[tuple[int, str], list[dict]] = defaultdict(list)
    for row in sorted_unique(candidates):
        check = tradeable(row)
        if check["tradeable"]:
            _, instance, epoch, *_ = availability_key(row)
            indexed[(instance, epoch, str(row["quote_mint"]))].append(row)
    chosen: dict[tuple, list[dict]] = {}
    for target in sorted_unique(targets):
        target_check = tradeable(target)
        if not target_check["tradeable"]:
            continue
        _, instance, epoch, *_ = availability_key(target)
        options = [row for row in indexed[(instance, epoch, str(target["quote_mint"]))]
                   if str(row.get("mint")) != str(target.get("mint"))]
        identity = "|".join(map(str, event_key(target)))
        options.sort(key=lambda row: hashlib.sha256(
            f"{seed}|{identity}|{'|'.join(map(str, event_key(row)))}".encode()
        ).hexdigest())
        chosen[event_key(target)] = [{**row, "dependency_cluster": cluster_key(row, bucket_ns)}
                                     for row in options[:per_target]]
    return chosen


def contract() -> dict:
    return {
        "schema": SCHEMA,
        "availability_clock": "received_at_unix_ns",
        "deterministic_tie_order": ["source_instance_id", "continuity_epoch", "durable_output_sequence", "event_index", "source_frame_sha256"],
        "entry_delay": "POSITIVE_PREREGISTERED_NS; entry is first supported same-epoch state at or after delay",
        "entry_transition": "RAW_INSTRUCTION_EVENT_VALIDATED_ONLY",
        "exit": "SAME_EPOCH_NEXT_OBSERVED_SUPPORTED_STATE_MARK; no inferred hypothetical fill",
        "non_tradeable": "FAIL_CLOSED_WITH_STABLE_REASON",
        "controls": "DETERMINISTIC_SAME_EPOCH_SAME_QUOTE_LAYOUT_DIFFERENT_MINT",
        "clustering": "DEPENDENCY_CLUSTER=EPOCH_POOL_FIXED_TIME_BUCKET",
        "pnl_accessed": False,
        "outcome_blind": True,
    }
