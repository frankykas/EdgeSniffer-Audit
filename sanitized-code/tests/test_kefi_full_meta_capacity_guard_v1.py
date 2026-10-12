import importlib.util
from pathlib import Path


GUARD = Path(__file__).parents[1] / "scripts" / "kefi_full_meta_capacity_guard_v1.py"


def module():
    spec = importlib.util.spec_from_file_location("kefi_guard", GUARD)
    assert spec and spec.loader
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


class Result:
    def __init__(self, returncode, stdout="", stderr=""):
        self.returncode = returncode
        self.stdout = stdout
        self.stderr = stderr


def test_failed_or_unknown_stop_is_not_verified(monkeypatch):
    guard = module()

    def run(command, **_):
        if "stop" in command:
            return Result(1, stderr="unknown unit")
        return Result(0, "LoadState=not-found\nActiveState=inactive\nSubState=dead\nMainPID=0\n")

    monkeypatch.setattr(guard.subprocess, "run", run)
    assert guard.verified_stop(["systemctl"], "missing.service")["verifiedStopped"] is False


def test_loaded_dead_unit_requires_successful_stop(monkeypatch):
    guard = module()

    def run(command, **_):
        if "stop" in command:
            return Result(0)
        return Result(0, "LoadState=loaded\nActiveState=inactive\nSubState=dead\nMainPID=0\n")

    monkeypatch.setattr(guard.subprocess, "run", run)
    assert guard.verified_stop(["systemctl"], "collector.service")["verifiedStopped"] is True
