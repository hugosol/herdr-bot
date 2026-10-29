#!/usr/bin/env python3
"""Offline node Task -> normal cutover; stop herdr-bot first. Python 3.11+.

Usage: python scripts/migrate-node-tasks-to-normal.py --root SERVER_CWD --config CONFIG
Both paths must be explicit; relative CONFIG and storage paths resolve from root.
Only taskType=node changes, for omp and pi alike. Task identities, duplicate
presets and all other fields survive unchanged. Bot, Run and Chat references are
never rewritten. Original Task bytes remain in STORE.pre-node-tasks-to-normal;
repeat execution is a no-op. Do not run against a running server.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import sys
import tomllib


class MigrationConflict(ValueError):
    pass


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise MigrationConflict(f"Duplicate JSON field: {key}")
        result[key] = value
    return result


def checked_path(root: Path, value, label: str) -> Path:
    if not isinstance(value, str) or not value:
        raise MigrationConflict(f"{label} must be a nonempty path")
    path = Path(value)
    path = path if path.is_absolute() else root / path
    # Inspect before normalizing, so a link followed by '..' cannot be hidden.
    for parent in (path, *path.parents):
        if parent.is_symlink() or (hasattr(parent, "is_junction") and parent.is_junction()):
            raise MigrationConflict(f"Linked path is not supported: {parent}")
    path = Path(os.path.abspath(path))
    if path.is_file() and path.stat().st_nlink != 1:
        raise MigrationConflict(f"Hard-linked file is not supported: {path}")
    return path


def migrate(root: Path, config_path: str) -> bool:
    root = root.absolute()
    config = checked_path(root, config_path, "config")
    settings = tomllib.loads(config.read_text(encoding="utf-8-sig"))
    for section in ("tasks", "sessions", "scheduler"):
        if section in settings and not isinstance(settings[section], dict):
            raise MigrationConflict(f"{section} must be a TOML table")
    store = checked_path(root, settings.get("tasks", {}).get("storage_file", ".data/tasks.json"), "tasks.storage_file")
    history = checked_path(root, settings.get("sessions", {}).get("history_file", ".data/session-history.json"), "sessions.history_file")
    backup = checked_path(root, str(store) + ".pre-node-tasks-to-normal", "backup")
    staging = checked_path(root, str(store) + ".node-tasks-to-normal.tmp", "staging")
    files = (config, store, history, backup, staging)
    if len({os.path.normcase(str(path)) for path in files}) != len(files):
        raise MigrationConflict("Configuration, Task store, Chat history and migration paths overlap")
    scheduler = settings.get("scheduler", {})
    for key, default in (("bot_directory", ".data/bots"), ("run_directory", ".data/workflow-runs"),
                         ("map_directory", ".data/workflow-maps")):
        directory = checked_path(root, scheduler.get(key, default), f"scheduler.{key}")
        for target in (store, backup, staging):
            if target == directory or directory in target.parents or target in directory.parents:
                raise MigrationConflict(f"Task migration path overlaps scheduler.{key}: {target}")
    if not store.exists():
        return False
    original = store.read_bytes()
    document = json.loads(original.decode("utf-8-sig"), object_pairs_hook=unique_object)
    if not isinstance(document, dict) or set(document) != {"tasks"} or not isinstance(document["tasks"], list):
        raise MigrationConflict("Task store must contain only a tasks array")
    seen = set()
    changed = False
    for task in document["tasks"]:
        if not isinstance(task, dict):
            raise MigrationConflict("Every Task must be an object")
        identity = task.get("id")
        if not isinstance(identity, str) or not identity.strip() or identity in seen:
            raise MigrationConflict("Task identities must be nonempty and unique")
        seen.add(identity)
        if task.get("agent") not in ("omp", "pi") or task.get("taskType") not in ("normal", "manager", "node"):
            raise MigrationConflict(f"Task {identity}: expected explicit taskType and omp/pi agent; run earlier cutovers first")
        if task["taskType"] == "node":
            task["taskType"] = "normal"
            changed = True
    if not changed:
        return False
    if staging.exists():
        raise MigrationConflict(f"Staging file already exists: {staging}")
    if backup.exists() and backup.read_bytes() != original:
        raise MigrationConflict(f"Backup already exists with different content: {backup}")
    encoded = (json.dumps(document, ensure_ascii=False, indent=2) + "\n").encode("utf-8")
    if not backup.exists():
        with backup.open("xb") as output:
            output.write(original)
            output.flush()
            os.fsync(output.fileno())
    staging_created = False
    try:
        with staging.open("xb") as output:
            staging_created = True
            output.write(encoded)
            output.flush()
            os.fsync(output.fileno())
        if store.read_bytes() != original:
            raise MigrationConflict("Task store changed during migration; stop the server first")
        os.replace(staging, store)
    finally:
        if staging_created and staging.exists():
            staging.unlink()
    return True


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--root", type=Path, required=True, help="Actual server process working directory")
    parser.add_argument("--config", required=True, help="Actual config path, relative to --root unless absolute")
    args = parser.parse_args()
    try:
        changed = migrate(args.root, args.config)
    except (OSError, ValueError, TypeError) as error:
        print(f"Node Task migration refused: {error}", file=sys.stderr)
        return 1
    print("Migrated node Tasks to normal; original backup retained; references unchanged."
          if changed else "No node Task changes needed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
