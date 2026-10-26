#!/usr/bin/env python3
"""Offline Bot -> Task cutover (Python 3.11+). Stop herdr-bot before running.

Use --root for the server working directory and --config for its configuration.
Original changed/retired files are retained as *.pre-bot-to-task. All input,
backup and staging conflicts are checked before any write. Re-running is a no-op.
Native sessions, prompts, IDs, agent paths and unrelated values are never renamed.
"""
from __future__ import annotations

import argparse
import copy
import json
from pathlib import Path
import re
import sys
import tempfile
import tomllib
import unittest


BACKUP_SUFFIX = ".pre-bot-to-task"
TEMP_SUFFIX = ".bot-to-task-migration.tmp"


class MigrationConflict(ValueError):
    pass


def rename_field(record: dict, old: str, new: str) -> None:
    if old in record:
        if new in record:
            raise MigrationConflict(f"Conflicting fields: {old} and {new}")
        record[new] = record.pop(old)


def read_json(raw: bytes):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise MigrationConflict(f"Duplicate JSON field: {key}")
            result[key] = value
        return result

    return json.loads(raw, object_pairs_hook=unique)


def encoded_json(value, original, raw: bytes) -> bytes:
    if value == original:
        return raw
    return (json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode("utf-8")


def converted_store(raw: bytes) -> bytes:
    original = read_json(raw)
    store = copy.deepcopy(original)
    if not isinstance(store, dict):
        raise MigrationConflict("Preset store must be a JSON object")
    rename_field(store, "bots", "tasks")
    if not isinstance(store.get("tasks"), list):
        raise MigrationConflict("Preset store must contain a bots/tasks array")
    for task in store["tasks"]:
        if not isinstance(task, dict):
            raise MigrationConflict("Every preset must be a JSON object")
        # Only this naming field is a template. No traversal/replacement in prompts.
        if "chatName" in task:
            if not isinstance(task["chatName"], str):
                raise MigrationConflict("chatName must be a string")
            task["chatName"] = task["chatName"].replace("{bot.name}", "{task.name}")
    return encoded_json(store, original, raw)


def converted_history(raw: bytes) -> bytes:
    original = read_json(raw)
    records = copy.deepcopy(original)
    if not isinstance(records, list):
        raise MigrationConflict("History must be a JSON array")
    for record in records:
        if not isinstance(record, dict):
            raise MigrationConflict("Every history record must be a JSON object")
        rename_field(record, "sourceBotId", "sourceTaskId")
        rename_field(record, "source_bot_id", "source_task_id")
    return encoded_json(records, original, raw)


def configured_path(value, label: str) -> Path:
    if not isinstance(value, str) or not value:
        raise MigrationConflict(f"{label} must be a nonempty path string")
    return Path(value)


def converted_config(raw: bytes):
    text = raw.decode("utf-8-sig")
    parsed = tomllib.loads(text)
    if "bots" in parsed and "tasks" in parsed:
        raise MigrationConflict("Config contains both [bots] and [tasks]")
    if "bot_templates" in parsed:
        raise MigrationConflict("Pre-Bot configuration is not supported by this Bot -> Task migration")
    settings = parsed.get("bots", parsed.get("tasks", {}))
    if not isinstance(settings, dict):
        raise MigrationConflict("Task settings must be a table")
    old_store = configured_path(settings.get("storage_file", ".data/bots.json"), "storage_file")
    old_catalog = configured_path(settings.get("metadata_file", "meta-data/bot-options.toml"), "metadata_file")
    new_store = old_store.with_name("tasks.json") if old_store.name == "bots.json" else old_store
    new_catalog = old_catalog.with_name("task-options.toml") if old_catalog.name == "bot-options.toml" else old_catalog
    expected = copy.deepcopy(parsed)
    if "bots" in expected:
        expected["tasks"] = expected.pop("bots")
    replacements = {}
    for key, before, after in (("storage_file", old_store, new_store), ("metadata_file", old_catalog, new_catalog)):
        if before != after and key in settings:
            replacements[key] = after.as_posix()
            expected["tasks"][key] = after.as_posix()
    # Preserve comments and unrelated configuration rather than serializing TOML.
    # Semantic validation below refuses unsupported layouts or accidental matches
    # (including apparent table headers embedded inside multiline strings).
    section = None
    output = []
    for line in text.splitlines(keepends=True):
        header = re.match(r"^(\s*)\[(bots|tasks)\](\s*(?:#.*)?)(\r?\n)?$", line)
        if header:
            section = "tasks"
            if header[2] == "bots":
                line = line[:header.start(2)] + "tasks" + line[header.end(2):]
        elif re.match(r"^\s*\[", line):
            section = None
        if section == "tasks":
            assignment = re.match(r"^(\s*)(storage_file|metadata_file)\s*=", line)
            if assignment and assignment[2] in replacements:
                key = assignment[2]
                # JSON quoted strings are valid TOML basic strings for paths.
                newline = "\r\n" if line.endswith("\r\n") else "\n" if line.endswith("\n") else ""
                line = f"{assignment[1]}{key} = {json.dumps(replacements[key], ensure_ascii=False)}{newline}"
        output.append(line)
    result = "".join(output)
    if tomllib.loads(result) != expected:
        raise MigrationConflict("Unsupported configuration layout; no files changed")
    if result == text:
        converted = raw
    else:
        converted = (b"\xef\xbb\xbf" if raw.startswith(b"\xef\xbb\xbf") else b"") + result.encode("utf-8")
    history = parsed.get("sessions", {}).get("history_file", ".data/session-history.json")
    return converted, old_store, new_store, old_catalog, new_catalog, configured_path(history, "history_file")


def migrate(root: Path, config_path: Path = Path("herdr-bot.toml")) -> list[Path]:
    root = root.resolve()

    def absolute(path: Path) -> Path:
        return (path if path.is_absolute() else root / path).resolve()

    config = absolute(config_path)
    original = config.read_bytes()
    config_data, old_store, new_store, old_catalog, new_catalog, history = converted_config(original)
    changes: dict[Path, bytes] = {}
    retired: list[Path] = []
    occupied = {config}
    if config_data != original:
        changes[config] = config_data

    def plan_file(old: Path, new: Path, transform, legacy_name: str) -> None:
        target = absolute(new)
        source = absolute(old)
        # A tracked config may already name the new default/custom basename.
        candidates = {source, target}
        if target.name in ("tasks.json", "task-options.toml"):
            candidates.add(target.with_name(legacy_name))
        if occupied.intersection(candidates):
            raise MigrationConflict(f"Configured data paths overlap: {target}")
        occupied.update(candidates)
        existing = [path for path in candidates if path.exists()]
        if len(existing) > 1:
            raise MigrationConflict("Old/new files coexist: " + ", ".join(str(p) for p in sorted(existing)))
        if not existing:
            return
        source = existing[0]
        raw = source.read_bytes()
        data = transform(raw)
        if source != target or data != raw:
            changes[target] = data
        if source != target:
            retired.append(source)

    def catalog(raw: bytes) -> bytes:
        tomllib.loads(raw.decode("utf-8-sig"))
        return raw

    plan_file(old_store, new_store, converted_store, "bots.json")
    plan_file(old_catalog, new_catalog, catalog, "bot-options.toml")
    plan_file(history, history, converted_history, "session-history.json")
    backups = list(dict.fromkeys([p for p in changes if p.exists()] + retired))
    originals = {path: path.read_bytes() for path in backups}
    # Preflight the entire plan, including recovery/staging paths, before mutation.
    reserved = occupied | set(changes) | set(retired)
    recovery = set()
    for path in backups:
        backup = path.with_name(path.name + BACKUP_SUFFIX)
        if backup in reserved or backup in recovery:
            raise MigrationConflict(f"Backup path overlaps migration input: {backup}")
        recovery.add(backup)
        if backup.exists() and backup.read_bytes() != originals[path]:
            raise MigrationConflict(f"Backup already exists with different content: {backup}")
    for path in changes:
        temporary = path.with_name(path.name + TEMP_SUFFIX)
        if temporary.exists() or temporary in reserved or temporary in recovery:
            raise MigrationConflict(f"Staging path already exists or overlaps input: {temporary}")
        parent = path.parent
        while not parent.exists():
            parent = parent.parent
        if not parent.is_dir():
            raise MigrationConflict(f"Target parent is not a directory: {parent}")
    for path in backups:
        backup = path.with_name(path.name + BACKUP_SUFFIX)
        if not backup.exists():
            with backup.open("xb") as stream:
                stream.write(originals[path])
    # Stage every output before replacing originals. Backups survive I/O failures.
    staged = []
    try:
        for path, data in changes.items():
            path.parent.mkdir(parents=True, exist_ok=True)
            temporary = path.with_name(path.name + TEMP_SUFFIX)
            with temporary.open("xb") as stream:
                stream.write(data)
            staged.append((temporary, path))
        for temporary, path in staged:
            temporary.replace(path)
        for path in retired:
            path.unlink()
    finally:
        for temporary, _ in staged:
            temporary.unlink(missing_ok=True)
    return list(changes) + retired


class MigrationTests(unittest.TestCase):
    def seed(self, root: Path, config: str = "[bots]\n", store: str = ".data/bots.json", history: str = ".data/session-history.json") -> None:
        (root / "herdr-bot.toml").write_text(config, encoding="utf-8")
        self.put(root / store, json.dumps({"bots": [{"id": "bot-opaque", "name": "Bot {bot.name}", "chatName": "{bot.name}-{chat.id}-{task.name}", "prompt": "{bot.name} sourceBotId bots", "extensions": ["connect-bot"]}], "extra": "bots"}))
        self.put(root / history, json.dumps([{"sessionId": "bot-opaque", "sourceBotId": "bot-opaque", "memo": "{bot.name}", "sessionFile": "native/bot.jsonl", "originalWorkspaceId": "bots", "extra": {"sourceBotId": "literal"}}]))

    @staticmethod
    def put(path: Path, text: str) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")

    @staticmethod
    def snapshot(root: Path):
        return {path.relative_to(root): path.read_bytes() for path in root.rglob("*") if path.is_file()}

    def test_semantic_preservation_and_repeat(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.seed(root)
            self.put(root / "meta-data/bot-options.toml", "[[agents]]\nid = 'connect-bot'\n")
            self.put(root / ".data/native/context.jsonl", "untouched {bot.name}")
            before = self.snapshot(root)
            migrate(root)
            task = read_json((root / ".data/tasks.json").read_bytes())["tasks"][0]
            self.assertEqual(task, {"id": "bot-opaque", "name": "Bot {bot.name}", "chatName": "{task.name}-{chat.id}-{task.name}", "prompt": "{bot.name} sourceBotId bots", "extensions": ["connect-bot"]})
            record = read_json((root / ".data/session-history.json").read_bytes())[0]
            expected = json.loads(before[Path(".data/session-history.json")])[0]
            expected["sourceTaskId"] = expected.pop("sourceBotId")
            self.assertEqual(record, expected)
            self.assertEqual((root / "meta-data/task-options.toml").read_bytes(), before[Path("meta-data/bot-options.toml")])
            for name in ("herdr-bot.toml", ".data/bots.json", ".data/session-history.json", "meta-data/bot-options.toml"):
                self.assertEqual((root / (name + BACKUP_SUFFIX)).read_bytes(), before[Path(name)])
            self.assertFalse((root / ".data/bots.json").exists())
            self.assertFalse((root / "meta-data/bot-options.toml").exists())
            after = self.snapshot(root)
            self.assertEqual(migrate(root), [])
            self.assertEqual(after, self.snapshot(root))
            self.assertEqual(after[Path(".data/native/context.jsonl")], before[Path(".data/native/context.jsonl")])

    def test_new_tracked_config_with_old_data(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.seed(root, "[tasks]\nstorage_file = '.data/tasks.json'\nmetadata_file = 'meta-data/task-options.toml'\n")
            self.put(root / "meta-data/task-options.toml", "agents = []\n")
            config = (root / "herdr-bot.toml").read_bytes()
            migrate(root)
            self.assertEqual((root / "herdr-bot.toml").read_bytes(), config)
            self.assertEqual(read_json((root / ".data/tasks.json").read_bytes())["tasks"][0]["id"], "bot-opaque")
            self.assertEqual(read_json((root / ".data/session-history.json").read_bytes())[0]["sourceTaskId"], "bot-opaque")

    def test_custom_paths_and_config(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.seed(root, "[bots]\nstorage_file = 'private/presets.json'\nmetadata_file = 'catalog/bot-options.toml'\n[sessions]\nhistory_file = 'private/history.json'\n", "private/presets.json", "private/history.json")
            (root / "herdr-bot.toml").rename(root / "custom.toml")
            self.put(root / "catalog/bot-options.toml", "models = []\n")
            migrate(root, root / "custom.toml")
            settings = tomllib.loads((root / "custom.toml").read_text())["tasks"]
            self.assertEqual(settings["storage_file"], "private/presets.json")
            self.assertEqual(settings["metadata_file"], "catalog/task-options.toml")
            self.assertIn("tasks", read_json((root / "private/presets.json").read_bytes()))
            self.assertIn("sourceTaskId", read_json((root / "private/history.json").read_bytes())[0])
            self.assertEqual(migrate(root, Path("custom.toml")), [])

    def test_new_custom_config_recovers_old_sibling(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.seed(root, "[tasks]\nstorage_file = 'private/tasks.json'\nmetadata_file = 'catalog/task-options.toml'\n[sessions]\nhistory_file = 'private/history.json'\n", "private/bots.json", "private/history.json")
            self.put(root / "catalog/bot-options.toml", "agents = []\n")
            migrate(root)
            self.assertEqual(read_json((root / "private/tasks.json").read_bytes())["tasks"][0]["id"], "bot-opaque")
            self.assertTrue((root / "catalog/task-options.toml").exists())
            self.assertEqual(migrate(root), [])

    def test_missing_optional_data(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.put(root / "herdr-bot.toml", "[bots]\n")
            migrate(root)
            self.assertEqual(tomllib.loads((root / "herdr-bot.toml").read_text()), {"tasks": {}})
            self.assertEqual(migrate(root), [])
            self.assertFalse((root / ".data").exists())

    def test_conflicts_and_invalid_inputs_never_mutate(self) -> None:
        cases = {
            "store coexistence": (".data/tasks.json", '{"tasks": []}'),
            "catalog coexistence": ("meta-data/task-options.toml", "agents = []"),
            "root fields": (".data/bots.json", '{"bots": [], "tasks": []}'),
            "source fields": (".data/session-history.json", '[{"sourceBotId":"x", "sourceTaskId":"x"}]'),
            "snake source fields": (".data/session-history.json", '[{"source_bot_id":"x", "source_task_id":"x"}]'),
            "config sections": ("herdr-bot.toml", "[bots]\n[tasks]\n"),
            "backup": (".data/bots.json" + BACKUP_SUFFIX, "different"),
            "staging": (".data/tasks.json" + TEMP_SUFFIX, "interrupted"),
            "malformed history": (".data/session-history.json", "{}"),
            "duplicate fields": (".data/bots.json", '{"bots": [], "bots": []}'),
            "invalid naming": (".data/bots.json", '{"bots": [{"chatName": 1}]}'),
            "bad catalog": ("meta-data/bot-options.toml", "not valid toml"),
            "overlap": ("herdr-bot.toml", "[bots]\nmetadata_file = '.data/bots.json'\n"),
        }
        for label, (path, text) in cases.items():
            with self.subTest(label=label), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                self.seed(root)
                self.put(root / "meta-data/bot-options.toml", "agents = []")
                self.put(root / path, text)
                before = self.snapshot(root)
                with self.assertRaises((MigrationConflict, ValueError)):
                    migrate(root)
                self.assertEqual(self.snapshot(root), before)

    def test_one_pass_and_snake_source(self) -> None:
        raw = b'{"tasks":[{"chatName":"{bot.name}{task.name}","prompt":"{bot.name}","name":"{bot.name}"}]}'
        converted = converted_store(raw)
        self.assertEqual(read_json(converted)["tasks"][0]["chatName"], "{task.name}{task.name}")
        self.assertEqual(converted_store(converted), converted)
        self.assertEqual(read_json(converted_history(b'[{"source_bot_id":"opaque"}]')), [{"source_task_id": "opaque"}])


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd(), help="Server working directory (relative data paths resolve here)")
    parser.add_argument("--config", type=Path, default=Path("herdr-bot.toml"), help="Config path, relative to --root unless absolute")
    parser.add_argument("--self-test", action="store_true", help="Run isolated migration regressions only")
    args = parser.parse_args()
    if args.self_test:
        unittest.main(argv=[sys.argv[0]])
    else:
        try:
            changed = migrate(args.root, args.config)
        except (OSError, ValueError, TypeError) as error:
            parser.exit(1, f"Migration refused/failed: {error}\nOriginal files/recovery backups have not been discarded.\n")
        for path in changed:
            print(path)
        print("Bot -> Task migration complete; native sessions and unrelated values untouched.")
