#!/usr/bin/env python3
"""Fail closed before the local kEFi full-meta lane exhausts laptop storage."""
from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import pathlib
import subprocess
import tempfile


def now() -> str:
    return dt.datetime.now(dt.timezone.utc).isoformat().replace("+00:00", "Z")


def atomic(path: pathlib.Path, value: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile("w", encoding="utf-8", dir=path.parent, delete=False) as handle:
        json.dump(value, handle, sort_keys=True, indent=2)
        handle.write("\n")
        handle.flush(); os.fsync(handle.fileno())
        temporary = pathlib.Path(handle.name)
    os.replace(temporary, path)


def volume(path: pathlib.Path) -> dict:
    stat = os.statvfs(path)
    return {"path": str(path), "freeGiB": round(stat.f_bavail * stat.f_frsize / 1024 ** 3, 3)}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--hot-root", type=pathlib.Path, required=True)
    parser.add_argument("--host-mount", type=pathlib.Path, required=True)
    parser.add_argument("--state", type=pathlib.Path, required=True)
    parser.add_argument("--collector-unit", required=True)
    parser.add_argument("--materializer-unit")
    parser.add_argument("--systemd-scope", choices=("user", "system"), default="user")
    parser.add_argument("--warning-free-gib", type=float, default=180.0)
    parser.add_argument("--emergency-free-gib", type=float, default=150.0)
    args = parser.parse_args()
    if args.emergency_free_gib >= args.warning_free_gib:
        raise SystemExit("INVALID_WATERMARKS")
    volumes = [volume(args.hot_root), volume(args.host_mount)]
    emergency = [item for item in volumes if item["freeGiB"] < args.emergency_free_gib]
    warning = [item for item in volumes if item["freeGiB"] < args.warning_free_gib]
    status = "HEALTHY" if not warning else "WARNING"
    paused = False
    actions: list[dict] = []

    def systemctl(action: str, unit: str) -> bool:
        command = ["systemctl"]
        if args.systemd_scope == "user":
            command.append("--user")
        completed = subprocess.run([*command, action, unit], check=False, capture_output=True, text=True)
        active = subprocess.run([*command, "is-active", "--quiet", unit], check=False).returncode == 0
        actions.append({"action": action, "unit": unit, "returncode": completed.returncode, "activeAfter": active})
        return not active

    # Derived work is disposable/rebuildable. It is stopped before source
    # capture whenever either monitored volume enters warning pressure.
    if warning and args.materializer_unit:
        systemctl("stop", args.materializer_unit)
    if emergency:
        stopped = systemctl("stop", args.collector_unit)
        status = "EMERGENCY_PAUSED_VERIFIED" if stopped else "EMERGENCY_STOP_FAILED"
        pause = {
            "schema": "KEFI_LOCAL_FULL_META_CAPACITY_PAUSE_V1", "recordedAtUtc": now(),
            "reason": "LOCAL_STORAGE_BELOW_EMERGENCY_WATERMARK", "affectedVolumes": emergency,
            "collectorUnit": args.collector_unit,
            "collectorStoppedVerified": stopped,
            "systemdActions": actions,
            "manualResumeRequired": True,
        }
        atomic(args.state.parent / "capacity-pause.json", pause)
        paused = stopped
    atomic(args.state, {
        "schema": "KEFI_LOCAL_FULL_META_CAPACITY_GUARD_V1", "generatedAtUtc": now(),
        "volumes": volumes, "status": status, "paused": paused,
        "systemdActions": actions,
        "warningFreeGiB": args.warning_free_gib, "emergencyFreeGiB": args.emergency_free_gib,
    })
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
