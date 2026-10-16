#!/usr/bin/env python3
"""Read-only R4 source-index and source-completeness reproducer.

This is intentionally an audit harness, not production capture code. It uses
the persisted manifest/checkpoint pair and mirrors the two relevant decisions
in `reconcile_finalized_block_with_policy` plus range readiness:

* source/finalized identity sets must agree;
* when requested, every same-signature source index must equal the finalized
  transaction index;
* without an index equality requirement, a missing expected identity stays
  AWAITING_SOURCE_IDENTITIES rather than becoming a complete interval.

`evaluate_recommended_slot` is a deliberately separate prototype of the
recommended future policy. It requires an explicit source closure proof before
it can return MATCHED, detects duplicate source signatures, and rejects a
source/RPC block-identity mismatch if both identities are available.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path


def load_jsonl(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line]


def evaluate_slot(source: list[dict], expected: list[dict], require_index: bool) -> str:
    by_signature = {record["signature"]: record for record in source}
    expected_by_signature = {record["signature"]: record for record in expected}
    source_set = set(by_signature)
    expected_set = set(expected_by_signature)
    if source_set < expected_set:
        return "MISSING_FILTERED_IDENTITY"
    if expected_set < source_set:
        return "EXTRA_FILTERED_IDENTITY"
    if source_set != expected_set:
        return "FILTERED_IDENTITY_SET_CONFLICT"
    if require_index:
        for signature in sorted(source_set):
            if int(by_signature[signature]["observedArrivalIndex"]) != int(
                expected_by_signature[signature]["transactionIndex"]
            ):
                return "FILTERED_SOURCE_TRANSACTION_INDEX_MISMATCH"
    return "MATCHED"


def evaluate_recommended_slot(
    source: list[dict],
    expected: list[dict],
    *,
    source_closed: bool,
    skipped: bool = False,
) -> str:
    """Model B: signature-set equality plus a block-bound closure proof.

    `sourceBlockhash` is optional because R4 did not persist it. When both it
    and the finalized `blockhash` are present, disagreement is fail-closed.
    A skipped slot is explicit inventory evidence, never source silence.
    """
    if skipped:
        return "SKIPPED_SLOT_RECORDED" if not source and not expected else "SKIPPED_SLOT_CONFLICT"
    source_signatures = [record["signature"] for record in source]
    if len(source_signatures) != len(set(source_signatures)):
        return "DUPLICATE_FILTERED_IDENTITY"
    current = evaluate_slot(source, expected, False)
    if current != "MATCHED":
        return current
    for record in source:
        expected_record = next(item for item in expected if item["signature"] == record["signature"])
        source_blockhash = record.get("sourceBlockhash")
        finalized_blockhash = expected_record.get("blockhash")
        if source_blockhash and finalized_blockhash and source_blockhash != finalized_blockhash:
            return "SOURCE_BLOCK_IDENTITY_MISMATCH"
    return "MATCHED" if source_closed else "AWAITING_SOURCE_CLOSURE"


def evaluate(root: Path, require_index: bool) -> dict:
    checkpoint = json.loads((root / "capture-run.checkpoint.json").read_text(encoding="utf-8"))
    source = load_jsonl(root / "canary-epoch-1.filtered-identities.jsonl")
    expected = checkpoint["expectedIdentities"]
    slots = sorted({record["slot"] for record in expected}, key=int)
    rows = []
    for slot in slots:
        actual = [record for record in source if record["slot"] == slot]
        wanted = [record for record in expected if record["slot"] == slot]
        rows.append(
            {
                "slot": slot,
                "sourceCount": len(actual),
                "expectedCount": len(wanted),
                "result": evaluate_slot(actual, wanted, require_index),
            }
        )
    first_terminal = next((row for row in rows if row["result"] not in {"MATCHED", "MISSING_FILTERED_IDENTITY"}), None)
    pending = next((row for row in rows if row["result"] == "MISSING_FILTERED_IDENTITY"), None)
    return {
        "requireIndexEquality": require_index,
        "slots": rows,
        "firstHardFailure": first_terminal,
        "awaitingSourceIdentities": pending,
        "rangeReady": all(row["result"] == "MATCHED" for row in rows),
    }


def self_test() -> None:
    source = [{"signature": "a", "observedArrivalIndex": "4"}]
    expected = [{"signature": "a", "transactionIndex": "4"}]
    assert evaluate_slot(source, expected, True) == "MATCHED"
    source[0]["observedArrivalIndex"] = "7"
    assert evaluate_slot(source, expected, True) == "FILTERED_SOURCE_TRANSACTION_INDEX_MISMATCH"
    assert evaluate_slot(source, expected, False) == "MATCHED"
    assert evaluate_slot([], expected, False) == "MISSING_FILTERED_IDENTITY"
    assert evaluate_slot(source, [], False) == "EXTRA_FILTERED_IDENTITY"
    assert evaluate_recommended_slot(
        [{"signature": "a", "observedArrivalIndex": "7"}],
        [{"signature": "a", "transactionIndex": "4", "blockhash": "b"}],
        source_closed=True,
    ) == "MATCHED"  # index disagreement is telemetry, not canonical order
    assert evaluate_recommended_slot([], expected, source_closed=False) == "MISSING_FILTERED_IDENTITY"
    assert evaluate_recommended_slot(
        [{"signature": "a"}, {"signature": "a"}], expected, source_closed=True
    ) == "DUPLICATE_FILTERED_IDENTITY"
    assert evaluate_recommended_slot(
        [{"signature": "a", "sourceBlockhash": "wrong"}],
        [{"signature": "a", "blockhash": "right"}],
        source_closed=True,
    ) == "SOURCE_BLOCK_IDENTITY_MISMATCH"
    assert evaluate_recommended_slot([], [], source_closed=False) == "AWAITING_SOURCE_CLOSURE"
    assert evaluate_recommended_slot([], [], source_closed=True) == "MATCHED"
    assert evaluate_recommended_slot([], [], source_closed=True, skipped=True) == "SKIPPED_SLOT_RECORDED"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("root", type=Path, nargs="?")
    parser.add_argument("--ignore-source-index", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
        print("SELF_TEST_PASS")
        return
    if args.root is None:
        parser.error("root is required unless --self-test is supplied")
    print(json.dumps(evaluate(args.root, not args.ignore_source_index), indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
