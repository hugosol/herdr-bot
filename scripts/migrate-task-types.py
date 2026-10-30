#!/usr/bin/env python3
"""Offline Task purpose cutover. Stop herdr-bot first; Python 3.11+ required.

Use the server's actual --root (process cwd) and --config. Adds taskType=normal
without inferring roles or changing any other Task field. Chat history and native
sessions are never rewritten. The original store is retained as .pre-task-types;
a second invocation is a no-op. Conflicts fail before modifying the store.
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


def read_json(raw: bytes):
    return json.loads(raw.decode("utf-8-sig"), object_pairs_hook=unique_object)


def checked_path(root: Path, value, label: str) -> Path:
    if not isinstance(value, str) or not value:
        raise MigrationConflict(f"{label} must be a nonempty path")
    path = Path(value)
    path = path if path.is_absolute() else root / path
    # Refuse links rather than following a target outside the stated data paths.
    if any(parent.is_symlink() or (hasattr(parent, "is_junction") and parent.is_junction())
           for parent in (path, *path.parents)):
        raise MigrationConflict(f"Linked path is not supported: {path}")
    return path.absolute()


def migrate(root: Path, config_path: str) -> bool:
    root = root.absolute()
    config = checked_path(root, config_path, "config")
    settings = tomllib.loads(config.read_text(encoding="utf-8-sig"))
    store = checked_path(root, settings.get("tasks", {}).get("storage_file", ".data/tasks.json"), "tasks.storage_file")
    history = checked_path(root, settings.get("sessions", {}).get("history_file", ".data/session-history.json"), "sessions.history_file")
    if len({os.path.normcase(str(path)) for path in (config, store, history)}) != 3:
        raise MigrationConflict("Configuration, Task store and Chat history paths overlap")
    if history.exists():
        records = read_json(history.read_bytes())
        if not isinstance(records, list) or any(not isinstance(record, dict) for record in records):
            raise MigrationConflict("Chat history must be an array of records; not rewritten")
    if not store.exists():
        return False
    original = store.read_bytes()
    document = read_json(original)
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
        required = {"id", "name", "chatName", "agent", "model", "thinking", "prompt"}
        if not required.issubset(task) or set(task) - required - {"skill", "extensions", "taskType"}:
            raise MigrationConflict(f"Task {identity}: unexpected or missing fields; run earlier cutovers first")
        if any(not isinstance(task[field], str) for field in required):
            raise MigrationConflict(f"Task {identity}: string field has invalid type")
        if task["agent"] not in ("omp", "pi") or not (task.get("skill") is None or isinstance(task["skill"], str)):
            raise MigrationConflict(f"Task {identity}: invalid agent or Skill")
        if "extensions" in task and (not isinstance(task["extensions"], list) or any(not isinstance(item, str) for item in task["extensions"])):
            raise MigrationConflict(f"Task {identity}: invalid extensions")
        if "taskType" not in task:
            task["taskType"] = "normal"
            changed = True
        elif task["taskType"] not in ("normal", "manager", "node"):
            raise MigrationConflict(f"Task {identity}: invalid taskType/agent combination")
    if not changed:
        return False
    backup = checked_path(root, str(store) + ".pre-task-types", "backup")
    staging = checked_path(root, str(store) + ".task-types.tmp", "staging")
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
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True, help="Server process working directory")
    parser.add_argument("--config", default="herdr-bot.toml", help="Config path, relative to --root unless absolute")
    args = parser.parse_args()
    try:
        changed = migrate(args.root, args.config)
    except (OSError, ValueError, TypeError) as error:
        print(f"Task type migration refused: {error}", file=sys.stderr)
        return 1
    print("Migrated Tasks to explicit normal type; original backup retained; Chat history unchanged."
          if changed else "No Task type changes needed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
