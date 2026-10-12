import importlib.util
from pathlib import Path


SCRIPT = Path(__file__).parents[1] / "scripts" / "kefi_h3_pumpswap_mechanics_v1.py"
spec = importlib.util.spec_from_file_location("mechanics", SCRIPT); assert spec and spec.loader
m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)


def row(seq, received, *, side="BUY", mint="token", pool="pool", base=90, quote=110, amount=10, delta=10):
    return {"source_instance_id": "source-a", "continuity_epoch": 3, "durable_output_sequence": seq, "event_index": 0,
            "source_frame_sha256": f"frame-{seq}", "received_at_unix_ns": received,
            "exact_transition_validated": True, "quote_mint": m.WSOL, "mint": mint,
            "amount_mode": "EXACT_BASE", "side": side, "pool": pool,
            "pool_base_raw": base, "pool_quote_raw": quote, "base_amount": amount,
            "pool_quote_delta": delta}


def test_exact_pre_state_and_nontradeable_rejections():
    assert m.pre_state(row(1, 10))["pool_quote_raw"] == 100
    sell = row(2, 20, side="SELL", base=100, quote=90, amount=10, delta=10)
    assert m.pre_state(sell) == {"pool_base_raw": 90, "pool_quote_raw": 100}
    bad = row(3, 30, quote=5, delta=10)
    assert m.tradeable(bad)["reason"] == "BUY_PRE_QUOTE_UNDERFLOW"


def test_delay_exit_and_controls_are_deterministic_and_epoch_scoped():
    signal = row(1, 100, mint="signal")
    entry = row(2, 120, mint="signal")
    mark = row(3, 200, mint="signal")
    controls = [row(4, 121, mint="control-a"), row(5, 122, mint="control-b")]
    assert m.delayed_entry_state([signal, entry, mark], signal, 15)["status"] == "TRADEABLE"
    assert m.observed_exit_mark([signal, entry, mark], entry, 50)["status"] == "MARK_AVAILABLE"
    first = m.deterministic_controls([signal], controls, bucket_ns=60, per_target=2, seed="frozen")
    second = m.deterministic_controls([signal], list(reversed(controls)), bucket_ns=60, per_target=2, seed="frozen")
    assert first == second


def test_duplicate_tie_order_fails_closed():
    first = row(1, 10); second = dict(first)
    try:
        m.sorted_unique([first, second])
    except m.NonTradeable as error:
        assert str(error) == "DUPLICATE_DETERMINISTIC_EVENT_ORDER"
    else:
        raise AssertionError("duplicate order accepted")


def test_same_receipt_timestamp_uses_durable_sequence_then_event_index():
    later = row(9, 100, mint="later")
    earlier = row(8, 100, mint="earlier")
    assert [item["mint"] for item in m.sorted_unique([later, earlier])] == ["earlier", "later"]
    same_sequence_one = row(10, 100, mint="one"); same_sequence_one["event_index"] = 1
    same_sequence_zero = row(10, 100, mint="zero"); same_sequence_zero["event_index"] = 0
    assert [item["mint"] for item in m.sorted_unique([same_sequence_one, same_sequence_zero])] == ["zero", "one"]


def test_entry_exit_and_controls_never_cross_source_instance_or_epoch():
    signal = row(1, 100, mint="signal")
    foreign_instance = row(2, 200, mint="signal"); foreign_instance["source_instance_id"] = "source-b"
    foreign_epoch = row(3, 200, mint="signal"); foreign_epoch["continuity_epoch"] = 4
    assert m.delayed_entry_state([signal, foreign_instance, foreign_epoch], signal, 1)["reason"] == "NO_SUPPORTED_POST_DELAY_ENTRY_STATE"
    controls = m.deterministic_controls([signal], [foreign_instance, foreign_epoch], bucket_ns=60, per_target=2, seed="frozen")
    assert controls[m.event_key(signal)] == []
