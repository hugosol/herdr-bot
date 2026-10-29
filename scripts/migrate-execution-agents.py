#!/usr/bin/env python3
"""Offline workflow execution-agent cutover. Stop herdr-bot first; Python 3.11+.

Use the server's actual --root and --config. Only pre-Pi executions lacking
agent gain agent=omp: these records could only be created by the OMP-only
workflow. Current Tasks are not consulted. Existing agent values, Chat history,
append-only workflow history, reports and native sessions are never rewritten.
Original run.json bytes are retained as run.json.pre-execution-agents.
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


def checked_path(root: Path, value: str) -> Path:
    if not isinstance(value, str) or not value:
        raise MigrationConflict("Expected a nonempty path")
    path = Path(os.path.abspath(root / value))
    if any(parent.is_symlink() or (hasattr(parent, "is_junction") and parent.is_junction())
           for parent in (path, *path.parents)):
        raise MigrationConflict(f"Linked path is not supported: {path}")
    return path


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise MigrationConflict(f"Duplicate JSON field: {key}")
        result[key] = value
    return result


def migrate_execution(execution) -> bool:
    if execution is None:
        return False
    if not isinstance(execution, dict) or not {"id", "taskId", "zone", "chat"}.issubset(execution):
        raise MigrationConflict("Invalid execution record")
    if any(not isinstance(execution[key], str) or not execution[key] for key in ("id", "taskId", "zone")):
        raise MigrationConflict("Invalid execution identity, Task or zone")
    if "agent" in execution:
        if execution["agent"] not in ("omp", "pi"):
            raise MigrationConflict("Invalid execution agent")
        return False
    execution["agent"] = "omp"
    return True


def migrate(root: Path, config_path: str) -> int:
    root = root.absolute()
    config = checked_path(root, config_path)
    settings = tomllib.loads(config.read_text(encoding="utf-8-sig"))
    directory = checked_path(root, settings.get("scheduler", {}).get("run_directory", ".data/workflow-runs"))
    if not directory.exists():
        return 0
    planned = []
    for entry in sorted(directory.iterdir()):
        entry = checked_path(root, str(entry))
        if not entry.is_dir():
            continue
        store = checked_path(root, str(entry / "run.json"))
        original = store.read_bytes()
        document = json.loads(original.decode("utf-8-sig"), object_pairs_hook=unique_object)
        if not isinstance(document, dict) or not isinstance(document.get("nodes"), dict):
            raise MigrationConflict(f"Invalid Run nodes: {store}")
        changed = False
        for state in document["nodes"].values():
            if not isinstance(state, dict) or "execution" not in state or not isinstance(state.get("previousExecutions"), list):
                raise MigrationConflict(f"Invalid node execution history: {store}")
            changed = migrate_execution(state["execution"]) or changed
            for fact in state["previousExecutions"]:
                if not isinstance(fact, dict) or "execution" not in fact:
                    raise MigrationConflict(f"Invalid previous execution: {store}")
                changed = migrate_execution(fact["execution"]) or changed
        if not changed:
            continue
        backup = checked_path(root, str(store) + ".pre-execution-agents")
        staging = checked_path(root, str(store) + ".execution-agents.tmp")
        if staging.exists():
            raise MigrationConflict(f"Staging file already exists: {staging}")
        if backup.exists() and backup.read_bytes() != original:
            raise MigrationConflict(f"Backup already exists with different content: {backup}")
        encoded = (json.dumps(document, ensure_ascii=False, indent=2) + "\n").encode("utf-8")
        planned.append((store, original, backup, staging, encoded))
    # Validate every candidate before modifying any Run. This is not a multi-file
    # transaction: retain backups if an I/O failure interrupts the offline cutover.
    for store, original, backup, staging, encoded in planned:
        if store.read_bytes() != original:
            raise MigrationConflict("Run changed during migration; stop the server first")
        if not backup.exists():
            with backup.open("xb") as output:
                output.write(original)
                output.flush()
                os.fsync(output.fileno())
        created = False
        try:
            with staging.open("xb") as output:
                created = True
                output.write(encoded)
                output.flush()
                os.fsync(output.fileno())
            if store.read_bytes() != original:
                raise MigrationConflict("Run changed during migration; stop the server first")
            os.replace(staging, store)
        finally:
            if created and staging.exists():
                staging.unlink()
    return len(planned)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True, help="Server process working directory")
    parser.add_argument("--config", default="herdr-bot.toml", help="Config relative to --root, or absolute")
    args = parser.parse_args()
    try:
        count = migrate(args.root, args.config)
    except (OSError, ValueError, TypeError) as error:
        print(f"Execution agent migration refused: {error}", file=sys.stderr)
        return 1
    print(f"Migrated {count} Run(s); original backups retained; histories and reports unchanged.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
