import importlib.util
import pathlib
import sys
import tempfile


SCRIPT = pathlib.Path(__file__).parents[1] / "scripts" / "aom_v0_public_universe_replay.py"
spec = importlib.util.spec_from_file_location("aom", SCRIPT); assert spec and spec.loader
aom = importlib.util.module_from_spec(spec); sys.modules[spec.name] = aom; spec.loader.exec_module(aom)


def event(index, timestamp, *, pool="pool-a", mint="mint-a", side="BUY", quote=1_000, base=1_000, amount=10):
    return {
        "schema": aom.INPUT_SCHEMA, "source_instance_id": "public-source", "continuity_epoch": 1,
        "slot": 100 + index, "transaction_index": index, "instruction_path": [0], "semantic_ordinal": 0,
        "signature": f"sig-{index}", "event_id": f"event-{index}", "observed_at_unix_ns": timestamp,
        "pool": pool, "mint": mint, "quote_mint": "WSOL", "pool_base_pre_raw": base,
        "pool_quote_pre_raw": quote, "amm_fee_bps": 30, "side": side, "quote_amount_raw": amount,
        "pool_identity_proof_ref": "pool-proof", "causal_proof_ref": "order-proof",
        "feature_validity": {"state": True, "pool_age": True, "flow_5s": True, "flow_20s": True, "flow_60s": True},
    }


def proof(**changes):
    value = {
        "schema": aom.PARTITION_SCHEMA, "partition_id": "partition-x", "capture_start_unix_ns": 1,
        "capture_end_unix_ns": 999, "transaction_coverage_verified": True,
        "transaction_coverage_proof_ref": "block-proof", "canonical_instruction_order_verified": True,
        "causal_order_proof_ref": "order-proof", "complete_eligible_pool_universe_verified": True,
        "universe_proof_ref": "universe-proof", "source_instance_id": "public-source", "continuity_epoch": 1,
        "manifest_sha256": "manifest",
    }
    value.update(changes); return value


def boundary(**changes):
    value = {"schema": aom.DECISION_BOUNDARY_SCHEMA, "decision_boundary_id": "d1", "partition_id": "partition-x",
             "decision_at_unix_ns": 100, "source_instance_id": "public-source", "continuity_epoch": 1,
             "causal_checkpoint": {"slot": 101, "transaction_index": 0}, "state_proof_ref": "state",
             "coverage_proof_ref": "block-proof", "universe_proof_ref": "universe-proof"}
    value.update(changes); return value


def test_partition_capture_and_universe_are_independent_gates():
    assert aom.validate_partition_admission(proof())["admitted"]
    for key, reason in (("transaction_coverage_verified", "INCOMPLETE_CAPTURED_TRANSACTION_INTERVAL"),
                        ("complete_eligible_pool_universe_verified", "ELIGIBLE_POOL_UNIVERSE_INCOMPLETE")):
        try:
            aom.validate_partition_admission(proof(**{key: False}))
        except aom.AdmissionBlocked as error:
            assert str(error) == reason
        else:
            raise AssertionError("invalid partition admitted")


def test_decision_boundary_is_versioned_and_bound_to_admitted_partition_epoch():
    assert aom.validate_decision_boundary(boundary(), proof())["decision_boundary_id"] == "d1"
    try:
        aom.validate_decision_boundary(boundary(partition_id="other"), proof())
    except aom.AdmissionBlocked as error:
        assert str(error) == "DECISION_BOUNDARY_PARTITION_MISMATCH"
    else:
        raise AssertionError("wrong partition boundary admitted")


def test_public_contract_rejects_wallet_and_lookahead_fields():
    for field in ("wallet", "teacher_label", "future_price", "pnl"):
        row = event(1, 1); row[field] = "forbidden"
        try:
            aom.validate_input_contract(row)
        except aom.AdmissionBlocked as error:
            assert str(error).startswith("PROHIBITED_NONPUBLIC_FIELD")
        else:
            raise AssertionError("wallet/lookahead field accepted")


def test_features_require_valid_history_and_are_reproducible_under_replay():
    rows = [event(1, 1_000_000_000), event(2, 5_000_000_000), event(3, 20_000_000_000)]
    one = aom.public_f01_f13(rows); two = aom.public_f01_f13(list(reversed(rows)))
    assert one == two
    assert one[0]["feature_validity"]["F05_seconds_since_previous_observed_buy"] is False
    assert one[-1]["feature_values"]["F08_prior_buy_count_20s"] == 2


def test_missing_history_and_duplicate_events_fail_closed():
    row = event(1, 1); row["feature_validity"]["flow_60s"] = False
    output = aom.public_f01_f13([row])
    assert not any(output[0]["feature_validity"].values())
    try:
        aom.public_f01_f13([event(1, 1), event(1, 1)])
    except aom.AdmissionBlocked as error:
        assert str(error) == "DUPLICATE_PUBLIC_EVENT"
    else:
        raise AssertionError("duplicate event admitted")


def test_causal_order_not_arrival_order_and_nonmonotone_availability_fails_closed():
    first = event(1, 1_000_000_000); second = event(2, 2_000_000_000)
    assert [item["event_identity"] for item in aom.public_f01_f13([second, first])] == [aom.event_identity(first), aom.event_identity(second)]
    bad = event(2, 500_000_000)
    try:
        aom.public_f01_f13([first, bad])
    except aom.AdmissionBlocked as error:
        assert str(error) == "NONMONOTONIC_PUBLIC_AVAILABILITY_CLOCK"
    else:
        raise AssertionError("inconsistent causal/public timing accepted")


def test_ranker_accounts_for_full_denominator_stable_ties_and_no_trade():
    rows = aom.public_f01_f13([event(1, 1, pool="a", mint="a"), event(2, 2, pool="b", mint="b")])
    # Make a valid standalone candidate set to test deterministic stable ordering.
    for row in rows:
        row["feature_validity"] = {name: True for name in aom.FEATURE_NAMES}
        row["feature_values"].update({"F05_seconds_since_previous_observed_buy": 1.0, "F06_prior_pre_pool_quote_log_change": 0.0})
    ranked = aom.deterministic_rank(list(reversed(rows)), top_k=1)
    assert ranked["denominator_count"] == 2 and ranked["selected"][0]["pool"] == "a"
    assert aom.deterministic_rank(rows, top_k=1, min_score=1e9)["no_trade"] is True


def test_cash_ledger_embeds_amm_fee_once_and_handles_no_fill():
    assumptions = aom.ExecutionAssumptions(100, 2, 3, 7, 1, 1, "synthetic-cost-proof")
    entry = {"transition_model": "CONSTANT_PRODUCT_EXACT_INPUT_V1", "exact_transition_validated": True,
             "decision_at_unix_ns": 10, "entry_available_at_unix_ns": 11, "pool_base_pre_raw": 10_000,
             "pool_quote_pre_raw": 10_000, "amm_fee_bps": 100}
    exit_mark = {"transition_model": "CONSTANT_PRODUCT_EXACT_INPUT_V1", "exact_transition_validated": True,
                 "target_exit_at_unix_ns": 20, "mark_available_at_unix_ns": 21, "pool_base_pre_raw": 9_000,
                 "pool_quote_pre_raw": 11_000, "amm_fee_bps": 100}
    filled = aom.simulate_exact_supported_transition(entry, exit_mark, assumptions)
    assert filled["status"] == "FILLED"
    ledger = filled["cash_ledger"]
    assert "entry_amm_fee_embedded" in ledger and "exit_amm_fee_embedded" in ledger
    assert filled["net_quote_raw"] == ledger["exit_proceeds"] - ledger["entry_notional"] - ledger["network_and_priority_cost"] - ledger["failed_transaction_cost"]
    no_fill = aom.simulate_exact_supported_transition({**entry, "entry_available_at_unix_ns": 10}, exit_mark, assumptions)
    assert no_fill == {"status": "NO_FILL", "reason": "ENTRY_LATENCY_UNMET", "net_quote_raw": -7, "cash_ledger": {"failed_transaction_cost": 7}}
    results = aom.fixed_horizon_marks(entry, {30: exit_mark, 60: exit_mark, 120: exit_mark}, assumptions)
    assert list(results) == [30, 60, 120]
    try:
        aom.fixed_horizon_marks(entry, {30: exit_mark}, assumptions)
    except aom.AdmissionBlocked as error:
        assert str(error) == "EXPLORATORY_HORIZON_SET_MISMATCH"
    else:
        raise AssertionError("unexpected horizon set accepted")


def test_chronological_evaluation_blocks_without_cost_evidence():
    result = aom.chronological_evaluate(proof(), [{"decision": "x", "decision_boundary": boundary()}], budget=aom.ResourceBudget(max_rows=2))
    assert result["status"] == "BLOCKED" and result["reason"] == "MISSING_INDEPENDENT_EXECUTION_COST_EVIDENCE"


def test_compact_writer_enforces_quota_and_is_replay_stable():
    with tempfile.TemporaryDirectory() as temp:
        target = pathlib.Path(temp) / "out.jsonl"
        result = aom.write_compact_output(target, [{"reference": "a"}, {"reference": "b"}], aom.ResourceBudget(max_rows=3, max_output_bytes=1000, min_free_bytes=0))
        assert result["rows"] == 2 and result["bytes"] > 0
        try:
            aom.write_compact_output(pathlib.Path(temp) / "small.jsonl", [{"reference": "large"}], aom.ResourceBudget(max_rows=3, max_output_bytes=1, min_free_bytes=0))
        except aom.BudgetExceeded as error:
            assert str(error) == "OUTPUT_QUOTA_EXCEEDED"
        else:
            raise AssertionError("quota bypass")
        assert not (pathlib.Path(temp) / "small.jsonl").exists()
        try:
            aom.write_compact_output(pathlib.Path(temp) / "rpc.jsonl", [], aom.ResourceBudget(min_free_bytes=0, max_rpc_requests=1))
        except aom.BudgetExceeded as error:
            assert str(error) == "AOM_V0_OFFLINE_RPC_BUDGET_MUST_BE_ZERO"
        else:
            raise AssertionError("offline RPC budget bypass")
