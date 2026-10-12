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
