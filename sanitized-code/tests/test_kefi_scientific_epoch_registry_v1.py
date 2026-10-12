import importlib.util
from pathlib import Path

SCRIPT = Path(__file__).parents[1] / "scripts" / "kefi_scientific_epoch_registry_v1.py"

def test_registry_rejects_duplicate_and_tamper(tmp_path):
    spec = importlib.util.spec_from_file_location("epochs", SCRIPT); module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
    record = {"epoch_id":"one","kind":"OPERATIONAL","status":"OPERATIONAL_ONLY","start_utc":"2026-10-12T00:00:00Z","end_utc":"","reason":"test"}
    module.append(tmp_path, record); module.verify(tmp_path)
    try: module.append(tmp_path, record)
    except RuntimeError: pass
    else: raise AssertionError("duplicate accepted")
    journal = tmp_path / "epochs.jsonl"; journal.write_text(journal.read_text().replace("OPERATIONAL_ONLY", "UNOPENED"))
    try: module.verify(tmp_path)
    except RuntimeError: pass
    else: raise AssertionError("tampering accepted")


def test_future_cohort_policy_rejects_old_epochs_and_allows_unopened(tmp_path):
    spec = importlib.util.spec_from_file_location("epochs", SCRIPT); module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
    def add(epoch_id, kind, status, start, end=""):
        module.append(tmp_path, {"epoch_id":epoch_id,"kind":kind,"status":status,"start_utc":start,"end_utc":end,"reason":"test"})
    add("h1", "H1_OPENED", "DEVELOPMENT_ONLY", "2026-10-10T00:00:00Z", "2026-10-10T01:00:00Z")
    add("h2", "H2_QUARANTINED", "QUARANTINED", "2026-10-10T02:00:00Z", "2026-10-10T03:00:00Z")
    add("ops", "OPERATIONAL", "OPERATIONAL_ONLY", "2026-10-10T04:00:00Z", "2026-10-10T05:00:00Z")
    add("future", "FUTURE_UNOPENED", "UNOPENED", "2026-10-10T06:00:00Z")
    for at, boundary, error in [
        ("2026-10-10T00:30:00Z", "2026-10-09T00:00:00Z", "INELIGIBLE_EPOCH_H1_OPENED"),
        ("2026-10-10T02:30:00Z", "2026-10-09T00:00:00Z", "INELIGIBLE_EPOCH_H2_QUARANTINED"),
        ("2026-10-10T04:30:00Z", "2026-10-09T00:00:00Z", "INELIGIBLE_EPOCH_OPERATIONAL"),
        ("2026-10-10T05:30:00Z", "2026-10-10T06:00:00Z", "INELIGIBLE_PRE_BOUNDARY"),
    ]:
        try: module.eligible(tmp_path, at, boundary)
        except RuntimeError as exc: assert error in str(exc)
        else: raise AssertionError(error + " accepted")
    module.eligible(tmp_path, "2026-10-10T06:30:00Z", "2026-10-10T06:00:00Z")
