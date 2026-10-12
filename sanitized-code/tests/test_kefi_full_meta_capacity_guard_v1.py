import importlib.util
import sys
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


def test_active_or_nonzero_pid_is_not_verified(monkeypatch):
    guard = module()

    def run(command, **_):
        if "stop" in command:
            return Result(0)
        return Result(0, "LoadState=loaded\nActiveState=active\nSubState=running\nMainPID=42\n")

    monkeypatch.setattr(guard.subprocess, "run", run)
    assert guard.verified_stop(["systemctl"], "collector.service")["verifiedStopped"] is False


def test_failed_systemctl_show_is_not_verified(monkeypatch):
    guard = module()

    def run(command, **_):
        return Result(0) if "stop" in command else Result(1, stderr="dbus unavailable")

    monkeypatch.setattr(guard.subprocess, "run", run)
    assert guard.verified_stop(["systemctl"], "collector.service")["verifiedStopped"] is False


def test_statvfs_failure_still_reaches_collector_stop(monkeypatch, tmp_path):
    guard = module()
    calls, writes = [], []
    monkeypatch.setattr(guard, "safe_volume", lambda path: (
        (None, {"path": str(path), "reason": "STATVFS_FAILED"})
        if "hot" in str(path) else ({"path": str(path), "freeGiB": 500}, None)
    ))
    monkeypatch.setattr(guard, "mounted_source", lambda _: {"findmntReturncode": 0, "source": "/dev/vdc1"})
    monkeypatch.setattr(guard, "verified_stop", lambda _, unit: calls.append(unit) or {"unit": unit, "verifiedStopped": True})
    monkeypatch.setattr(guard, "atomic", lambda _, value: writes.append(value))
    monkeypatch.setattr(sys, "argv", ["guard", "--hot-root", str(tmp_path / "hot"), "--host-mount", str(tmp_path), "--state", str(tmp_path / "state"), "--collector-unit", "collector", "--materializer-timer-unit", "timer", "--materializer-service-unit", "service", "--hot-mount", str(tmp_path), "--expected-hot-source", "/dev/vdc1", "--systemd-scope", "system"])
    assert guard.main() == 0
    assert calls == ["timer", "service", "collector"]
    assert writes[-1]["status"] == "EMERGENCY_PAUSED_VERIFIED"


def test_warning_reports_failed_materializer_stop(monkeypatch, tmp_path):
    guard = module()
    calls, writes = [], []
    monkeypatch.setattr(guard, "safe_volume", lambda path: ({"path": str(path), "freeGiB": 80}, None))
    monkeypatch.setattr(guard, "mounted_source", lambda _: {"findmntReturncode": 0, "source": "/dev/vdc1"})
    monkeypatch.setattr(guard, "verified_stop", lambda _, unit: calls.append(unit) or {"unit": unit, "verifiedStopped": unit != "service"})
    monkeypatch.setattr(guard, "atomic", lambda _, value: writes.append(value))
    monkeypatch.setattr(sys, "argv", ["guard", "--hot-root", str(tmp_path / "hot"), "--host-mount", str(tmp_path), "--state", str(tmp_path / "state"), "--collector-unit", "collector", "--materializer-timer-unit", "timer", "--materializer-service-unit", "service", "--hot-mount", str(tmp_path), "--expected-hot-source", "/dev/vdc1", "--systemd-scope", "system", "--warning-free-gib", "100", "--emergency-free-gib", "70"])
    assert guard.main() == 0
    assert calls == ["timer", "service"]
    assert writes[-1]["status"] == "WARNING_MATERIALIZER_STOP_FAILED"
