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
        "pool_quote_pre_raw": quote, "amm_fee_bps": 30, "fee_model": "INPUT_AMOUNT_BPS", "fee_denomination": "INPUT_TOKEN", "side": side, "quote_amount_raw": amount,
        "pool_identity_proof_ref": "pool-proof", "causal_proof_ref": "order-proof",
        "feature_validity": {"state": True, "pool_age": True, "flow_5s": True, "flow_20s": True, "flow_60s": True, "full_pool_history": True},
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


def immutable_proof(proof_id, kind, payload):
    value = {"schema": aom.PROOF_SCHEMA, "proof_id": proof_id, "kind": kind,
             "partition_id": "partition-x", "partition_manifest_sha256": "manifest", "payload": payload}
    value["payload_sha256"] = aom.sha256(payload)
    value["proof_sha256"] = aom.hash_without({**value, "proof_sha256": ""}, "proof_sha256")
    return value


def bundle():
    proofs = [
        immutable_proof("block-proof", "FINALIZED_TRANSACTION_COVERAGE", {"finalized_block_inventory_complete": True, "filtered_identity_reconciled": True, "relevant_identity_manifest_sha256": "identities", "block_proof_manifest_sha256": "blocks"}),
        immutable_proof("order-proof", "CANONICAL_CAUSAL_ORDER", {"all_state_events_order_resolved": True, "ambiguous_event_count": 0, "causal_event_set_sha256": "causal"}),
        immutable_proof("universe-proof", "COMPLETE_ELIGIBLE_POOL_UNIVERSE", {"universe_complete": True, "candidate_registry_sha256": "candidates", "potential_pool_inventory_sha256": "inventory", "recovery_status_sha256": "recovery", "unresolved_pool_count": 0, "source_instance_id": "public-source", "continuity_epoch": 1}),
        immutable_proof("state", "PREDECISION_PUBLIC_STATE", {"state_complete": True, "state_at_unix_ns": 100, "causal_checkpoint": {"slot": 101, "transaction_index": 0}}),
    ]
    value = {"schema": aom.PROOF_BUNDLE_SCHEMA, "proofs": proofs}
    value["bundle_sha256"] = aom.hash_without({**value, "bundle_sha256": ""}, "bundle_sha256")
    return value


def test_partition_capture_and_universe_are_independent_gates():
    assert aom.validate_partition_admission(proof(), bundle())["admitted"]
    for key, reason in (("transaction_coverage_verified", "INCOMPLETE_CAPTURED_TRANSACTION_INTERVAL"),
                        ("complete_eligible_pool_universe_verified", "ELIGIBLE_POOL_UNIVERSE_INCOMPLETE")):
        try:
            aom.validate_partition_admission(proof(**{key: False}), bundle())
        except aom.AdmissionBlocked as error:
            assert str(error) == reason
        else:
            raise AssertionError("invalid partition admitted")


def test_boolean_claim_or_tampered_immutable_proof_never_admits_partition():
    try:
        aom.validate_partition_admission(proof())
    except aom.AdmissionBlocked as error:
        assert str(error) == "IMMUTABLE_PROOF_BUNDLE_REQUIRED"
    else:
        raise AssertionError("bare booleans admitted")
    bad = bundle(); bad["proofs"][0]["payload"]["filtered_identity_reconciled"] = False
    try:
        aom.validate_partition_admission(proof(), bad)
    except aom.AdmissionBlocked as error:
        assert str(error).startswith("PROOF_BUNDLE_HASH_MISMATCH")
    else:
        raise AssertionError("tampered proof admitted")


def test_decision_boundary_is_versioned_and_bound_to_admitted_partition_epoch():
    assert aom.validate_decision_boundary(boundary(), proof(), bundle())["decision_boundary_id"] == "d1"
    try:
        aom.validate_decision_boundary(boundary(partition_id="other"), proof(), bundle())
    except aom.AdmissionBlocked as error:
        assert str(error) == "DECISION_BOUNDARY_PARTITION_MISMATCH"
    else:
        raise AssertionError("wrong partition boundary admitted")
    try:
        aom.validate_decision_boundary(boundary(causal_checkpoint={"slot": 102}), proof(), bundle())
    except aom.AdmissionBlocked as error:
        assert str(error) == "DECISION_STATE_PROOF_INSUFFICIENT"
    else:
        raise AssertionError("unbound state proof admitted")


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
    assert one[0]["feature_validity"]["F05_seconds_since_previous_observed_buy"] is True
    assert one[0]["feature_values"]["F05_seconds_since_previous_observed_buy"] is None
    assert one[0]["feature_values"]["F13_no_prior_observed_buy"] is True
    assert one[-1]["feature_values"]["F08_prior_buy_count_20s"] == 2


def test_missing_history_and_duplicate_events_fail_closed():
    row = event(1, 1); row["feature_validity"]["flow_60s"] = False
    output = aom.public_f01_f13([row])
    assert output[0]["feature_validity"]["F01_log_pre_pool_quote"] is False
    assert output[0]["feature_validity"]["F13_no_prior_observed_buy"] is True
    try:
        aom.public_f01_f13([event(1, 1), event(1, 1)])
    except aom.AdmissionBlocked as error:
        assert str(error) == "DUPLICATE_PUBLIC_EVENT"
    else:
        raise AssertionError("duplicate event admitted")


def test_f13_lifetime_semantics_survive_rolling_window_expiry_and_missing_lifetime_history():
    early = event(1, 1_000_000_000)
    late = event(2, 100_000_000_000)
    values = aom.public_f01_f13([early, late])
    assert values[-1]["feature_values"]["F07_prior_buy_count_5s"] == 0
    assert values[-1]["feature_values"]["F13_no_prior_observed_buy"] is False
    unknown = event(3, 101_000_000_000); unknown["feature_validity"]["full_pool_history"] = False
    result = aom.public_f01_f13([unknown])[-1]
    assert result["feature_values"]["F13_no_prior_observed_buy"] is None
    assert result["feature_missingness"]["F13_no_prior_observed_buy"] == "LIFETIME_HISTORY_UNPROVEN"


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
    foreign_epoch = event(3, 3_000_000_000); foreign_epoch["continuity_epoch"] = 2
    try:
        aom.public_f01_f13([first, foreign_epoch])
    except aom.AdmissionBlocked as error:
        assert str(error) == "CROSS_EPOCH_PUBLIC_FEATURE_INPUT"
    else:
        raise AssertionError("cross epoch state accepted")


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
             "pool_quote_pre_raw": 10_000, "amm_fee_bps": 100, "fee_model": "INPUT_AMOUNT_BPS", "fee_denomination": "INPUT_TOKEN", "entry_attempt_status": "FILLED"}
    exit_mark = {"transition_model": "CONSTANT_PRODUCT_EXACT_INPUT_V1", "exact_transition_validated": True,
                 "target_exit_at_unix_ns": 20, "mark_available_at_unix_ns": 21, "pool_base_pre_raw": 9_000,
                 "pool_quote_pre_raw": 11_000, "amm_fee_bps": 100, "fee_model": "INPUT_AMOUNT_BPS", "fee_denomination": "INPUT_TOKEN", "exit_attempt_status": "FILLED"}
    filled = aom.simulate_exact_supported_transition(entry, exit_mark, assumptions)
    assert filled["status"] == "CLOSED_FILLED"
    ledger = filled["cash_ledger"]
    assert "entry_amm_fee_embedded" in ledger and "exit_amm_fee_embedded" in ledger
    assert filled["realized_net_quote_raw"] == ledger["exit_proceeds"] - ledger["entry_notional"] - ledger["entry_network_and_priority_cost"] - ledger["exit_network_and_priority_cost"]
    no_fill = aom.simulate_exact_supported_transition({**entry, "entry_available_at_unix_ns": 10}, exit_mark, assumptions)
    assert no_fill == {"status": "ENTRY_NO_FILL", "reason": "ENTRY_LATENCY_UNMET", "realized_net_quote_raw": 0, "position_base_raw": 0, "cash_ledger": {}}
    results = aom.fixed_horizon_marks(entry, {30: exit_mark, 60: exit_mark, 120: exit_mark}, assumptions)
    assert list(results) == [30, 60, 120]
    try:
        aom.fixed_horizon_marks(entry, {30: exit_mark}, assumptions)
    except aom.AdmissionBlocked as error:
        assert str(error) == "EXPLORATORY_HORIZON_SET_MISMATCH"
    else:
        raise AssertionError("unexpected horizon set accepted")
    try:
        aom.simulate_exact_supported_transition({**entry, "fee_denomination": "QUOTE_TOKEN"}, exit_mark, assumptions)
    except aom.AdmissionBlocked as error:
        assert str(error) == "UNSUPPORTED_EXECUTION_FEE_DENOMINATION"
    else:
        raise AssertionError("ambiguous execution fee denomination accepted")


def test_failed_attempts_need_evidence_and_open_positions_are_not_discarded():
    assumptions = aom.ExecutionAssumptions(100, 2, 3, 7, 1, 1, "cost-proof")
    entry = {"transition_model": "CONSTANT_PRODUCT_EXACT_INPUT_V1", "exact_transition_validated": True,
             "decision_at_unix_ns": 10, "entry_available_at_unix_ns": 11, "pool_base_pre_raw": 10_000,
             "pool_quote_pre_raw": 10_000, "amm_fee_bps": 100, "fee_model": "INPUT_AMOUNT_BPS", "fee_denomination": "INPUT_TOKEN", "entry_attempt_status": "FAILED"}
    try:
        aom.simulate_exact_supported_transition(entry, None, assumptions)
    except aom.AdmissionBlocked as error:
        assert str(error) == "FAILED_ENTRY_EVIDENCE_REQUIRED"
    else:
        raise AssertionError("unproven failed entry charged")
    failed = aom.simulate_exact_supported_transition({**entry, "entry_failure_evidence_ref": "attempt"}, None, assumptions)
    assert failed["status"] == "ENTRY_FAILED" and failed["realized_net_quote_raw"] == -7
    filled_entry = {**entry, "entry_attempt_status": "FILLED", "entry_failure_evidence_ref": ""}
    opened = aom.simulate_exact_supported_transition(filled_entry, None, assumptions)
    assert opened["status"] == "OPEN_POSITION" and opened["position_base_raw"] > 0 and opened["realized_net_quote_raw"] is None


def test_chronological_evaluation_blocks_without_cost_evidence():
    result = aom.chronological_evaluate(proof(), [{"decision": "x", "decision_boundary": boundary()}], budget=aom.ResourceBudget(max_rows=2), proof_bundle=bundle())
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
