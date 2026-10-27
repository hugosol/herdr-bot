#!/usr/bin/env python3
"""Offline NodeTemplate -> Bot cutover (Python 3.11+); stop the service first.

Requires an explicit service --cwd and an existing --config (relative to --cwd).
Only scheduler configuration keys, the default definition directory name, and
Map nodes' immediate node_template_id keys change. IDs, nested user values and
Bot definition bytes are untouched. All inputs, backups and staging paths are
checked before writing. Originals are backed up alongside their source with
.pre-node-template-to-bot appended. --dry-run performs the same preflight only.
Real symlink self-tests run only with HERDR_BOT_MANUAL_SYMLINKS=1.
"""
from __future__ import annotations

import argparse
import copy
import json
import os
from pathlib import Path
import re
import shutil
import stat
import sys
import tempfile
import tomllib
import unittest


BACKUP_SUFFIX = ".pre-node-template-to-bot"
TEMP_SUFFIX = ".node-template-to-bot-migration.tmp"
OLD_DIRECTORY = Path(".data/node-templates")
NEW_DIRECTORY = Path(".data/bots")


class MigrationConflict(ValueError):
    pass


def safe_path(path: Path) -> Path:
    """Reject symlinks, junctions and reparse points, including ancestors."""
    path = path if path.is_absolute() else Path.cwd() / path
    current = Path(path.anchor)
    for part in path.parts[1:]:
        current /= part
        try:
            metadata = current.lstat()
        except FileNotFoundError:
            continue
        if stat.S_ISLNK(metadata.st_mode) or getattr(metadata, "st_file_attributes", 0) & 0x400:
            raise MigrationConflict(f"Link/reparse point is not allowed: {current}")
    return Path(os.path.abspath(path))


def configured_path(value, label: str) -> Path:
    if not isinstance(value, str) or not value:
        raise MigrationConflict(f"{label} must be a nonempty path string")
    return Path(value)


def converted_config(raw: bytes):
    text = raw.decode("utf-8-sig")
    parsed = tomllib.loads(text)
    settings = parsed.get("scheduler", {})
    if not isinstance(settings, dict):
        raise MigrationConflict("scheduler must be a TOML table")
    if "node_template_directory" in settings and "bot_directory" in settings:
        raise MigrationConflict("Config contains both node_template_directory and bot_directory")
    directory = configured_path(settings.get("node_template_directory", settings.get("bot_directory", str(OLD_DIRECTORY))), "scheduler definition directory")
    maps = configured_path(settings.get("map_directory", ".data/workflow-maps"), "scheduler.map_directory")
    default_directory = directory in (OLD_DIRECTORY, NEW_DIRECTORY)
    expected = copy.deepcopy(parsed)
    new_settings = expected.get("scheduler", {})
    if "node_template_directory" in new_settings:
        new_settings["bot_directory"] = new_settings.pop("node_template_directory")
    if default_directory and "bot_directory" in new_settings:
        new_settings["bot_directory"] = NEW_DIRECTORY.as_posix()
    # Match only ordinary [scheduler] assignments. Reparse and compare semantics
    # to refuse unsupported TOML layouts, including apparent keys in strings.
    section = False
    output = []
    for line in text.splitlines(keepends=True):
        if re.match(r"^\s*\[scheduler\]\s*(?:#.*)?(?:\r?\n)?$", line):
            section = True
        elif re.match(r"^\s*\[", line):
            section = False
        assignment = re.match(r"^(\s*)(node_template_directory|bot_directory)\s*=", line) if section else None
        if assignment:
            key = assignment[2]
            if key == "node_template_directory":
                line = line[:assignment.start(2)] + "bot_directory" + line[assignment.end(2):]
            if default_directory and settings.get(key) != NEW_DIRECTORY.as_posix():
                # Keep trailing comments and whitespace for normal quoted paths.
                value = re.match(r"^(\s*bot_directory\s*=\s*)(\"[^\"\r\n]*\"|'[^'\r\n]*')(.*)$", line, re.DOTALL)
                if value:
                    line = value[1] + json.dumps(NEW_DIRECTORY.as_posix()) + value[3]
        output.append(line)
    result = "".join(output)
    if tomllib.loads(result) != expected:
        raise MigrationConflict("Unsupported scheduler configuration layout; no files changed")
    converted = raw if result == text else (b"\xef\xbb\xbf" if raw.startswith(b"\xef\xbb\xbf") else b"") + result.encode("utf-8")
    return converted, directory, maps, default_directory


def read_json(raw: bytes):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise MigrationConflict(f"Duplicate JSON key: {key}")
            result[key] = value
        return result

    def invalid_constant(value):
        raise MigrationConflict(f"Invalid JSON constant: {value}")

    return json.loads(raw, object_pairs_hook=unique, parse_constant=invalid_constant)


def converted_map(raw: bytes) -> bytes:
    original = read_json(raw)
    if not isinstance(original, dict) or not isinstance(original.get("nodes"), dict):
        raise MigrationConflict("Map must be a JSON object with a nodes object")
    for node_id, node in original["nodes"].items():
        if not isinstance(node, dict):
            raise MigrationConflict(f"Map node {node_id!r} must be an object")
        if "node_template_id" in node and "bot_id" in node:
            raise MigrationConflict(f"Map node {node_id!r} has both node_template_id and bot_id")
    # Locate only the immediate node key tokens. Do not serialize user values:
    # this also preserves whitespace, escaped strings and exact numeric lexemes.
    text = raw.decode("utf-8-sig")
    decoder = json.JSONDecoder()

    def whitespace(position):
        while position < len(text) and text[position] in " \t\r\n":
            position += 1
        return position

    def members(position):
        position = whitespace(position + 1)  # opening object brace
        while text[position] != "}":
            key_start = position
            key, key_end = decoder.raw_decode(text, position)
            value_start = whitespace(whitespace(key_end) + 1)  # colon
            _, value_end = decoder.raw_decode(text, value_start)
            yield key, key_start, key_end, value_start
            position = whitespace(value_end)
            if text[position] == "}":
                break
            position = whitespace(position + 1)  # comma

    replacements = []
    for key, _, _, nodes_start in members(whitespace(0)):
        if key != "nodes":
            continue
        for _, _, _, node_start in members(nodes_start):
            for field, start, end, _ in members(node_start):
                if field == "node_template_id":
                    replacements.append((start, end))
    if not replacements:
        return raw
    for start, end in reversed(replacements):
        text = text[:start] + '"bot_id"' + text[end:]
    return (b"\xef\xbb\xbf" if raw.startswith(b"\xef\xbb\xbf") else b"") + text.encode("utf-8")


def inspect_tree(directory: Path) -> None:
    safe_path(directory)
    if not directory.is_dir():
        raise MigrationConflict(f"Not a directory: {directory}")
    for child in directory.iterdir():
        safe_path(child)
        if child.is_dir():
            inspect_tree(child)
        elif not child.is_file():
            raise MigrationConflict(f"Not a regular file: {child}")


def migrate(cwd: Path, config_path: Path = Path("herdr-bot.toml"), *, dry_run: bool = False) -> list[str]:
    root = safe_path(cwd)
    if not root.is_dir():
        raise MigrationConflict(f"Service cwd is not a directory: {root}")

    def absolute(path: Path) -> Path:
        return safe_path(path if path.is_absolute() else root / path)

    config = absolute(config_path)
    raw_config = config.read_bytes()
    config_data, directory, map_directory, default_directory = converted_config(raw_config)
    maps = absolute(map_directory)
    definitions = absolute(NEW_DIRECTORY if default_directory else directory)
    old = absolute(OLD_DIRECTORY) if default_directory else definitions
    if default_directory and old.exists() and definitions.exists():
        raise MigrationConflict(f"Old/new directories coexist: {old}, {definitions}")
    moving = default_directory and old.exists()
    source = old if moving else definitions
    # Disallow path overlap before discovering files or reserving recovery paths.
    roots = [config, maps, definitions] + ([old] if default_directory else [])
    for index, left in enumerate(roots):
        for right in roots[index + 1:]:
            if left == right or left in right.parents or right in left.parents:
                raise MigrationConflict(f"Configured paths overlap: {left}, {right}")
    if source.exists():
        inspect_tree(source)
    changes: dict[Path, bytes] = {}
    if config_data != raw_config:
        changes[config] = config_data
    map_inputs = []
    if maps.exists():
        if not maps.is_dir():
            raise MigrationConflict(f"Map directory is not a directory: {maps}")
        for path in sorted(maps.iterdir()):
            # Match the consumer: immediate JSON files only; no recursive maps.
            if path.suffix.lower() != ".json":
                continue
            safe_path(path)
            if not path.is_file():
                raise MigrationConflict(f"Map JSON is not a regular file: {path}")
            map_inputs.append(path)
            raw = path.read_bytes()
            try:
                data = converted_map(raw)
            except (ValueError, TypeError) as error:
                raise MigrationConflict(f"Map {path}: {error}") from error
            if data != raw:
                changes[path] = data
    backups = {path: path.with_name(path.name + BACKUP_SUFFIX) for path in changes}
    if moving:
        backups[old] = old.with_name(old.name + BACKUP_SUFFIX)
    staged = {path: path.with_name(path.name + TEMP_SUFFIX) for path in changes}
    reserved = [config, definitions] + ([old] if default_directory else []) + map_inputs
    recovery = list(backups.values()) + list(staged.values())
    for index, path in enumerate(recovery):
        safe_path(path)
        if path.exists():
            raise MigrationConflict(f"Backup/staging target already exists: {path}")
        if path == maps or path in maps.parents:
            raise MigrationConflict(f"Recovery path overlaps Map directory: {path}, {maps}")
        for other in reserved + recovery[:index]:
            if path == other or path in other.parents or other in path.parents:
                raise MigrationConflict(f"Recovery path overlaps input or output: {path}, {other}")
        if not path.parent.is_dir():
            raise MigrationConflict(f"Backup/staging parent is not a directory: {path.parent}")
    actions = [f"BACKUP {path} -> {backup}" for path, backup in backups.items()]
    actions += [f"WRITE {path}" for path in changes]
    if moving:
        actions.append(f"RENAME {old} -> {definitions} (definition bytes unchanged)")
    if dry_run:
        return actions
    # No mutation above this point. Back up every original before staging outputs.
    for path, backup in backups.items():
        if path.is_dir():
            shutil.copytree(path, backup)
        else:
            with backup.open("xb") as stream:
                stream.write(path.read_bytes())
    written = []
    try:
        for path, data in changes.items():
            with staged[path].open("xb") as stream:
                written.append(staged[path])
                stream.write(data)
        # The service must remain stopped: this is an offline, not concurrent,
        # migration. Recheck the rename target so it never merges directories.
        if moving:
            safe_path(definitions)
            if definitions.exists():
                raise MigrationConflict(f"Rename target already exists: {definitions}")
            old.rename(definitions)
        for path in changes:
            staged[path].replace(path)
    finally:
        for path in written:
            path.unlink(missing_ok=True)
    return actions


class MigrationTests(unittest.TestCase):
    @staticmethod
    def put(path: Path, data: bytes) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)

    @staticmethod
    def snapshot(root: Path):
        return {path.relative_to(root).as_posix(): ("directory" if path.is_dir() else path.read_bytes()) for path in root.rglob("*")}

    def seed(self, root: Path, config: bytes = b"[scheduler]\n") -> None:
        self.put(root / "herdr-bot.toml", config)
        self.put(root / ".data/node-templates/opaque-node_template_id.json", b'{ "id": "node-template-opaque", "description": "node_template_id", "parameters": {"template_id": "literal"}}\r\n')
        self.put(root / ".data/workflow-maps/map.json", json.dumps({"title": "node_template_id", "description": "template-id literal", "nodes": {"node_template_id": {"node_template_id": "node-template-opaque", "parameters": {"node_template_id": "keep", "nested": [{"node_template_id": "literal", "bot_id": "also keep"}]}, "description": "node_template_id"}}}).encode())

    def test_preservation_backups_dry_run_and_repeat(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.seed(root, b"[scheduler]\r\nnode_template_directory = '.data/node-templates/' # keep\r\n")
            before = self.snapshot(root)
            self.assertTrue(migrate(root, dry_run=True))
            self.assertEqual(self.snapshot(root), before)
            migrate(root)
            config = tomllib.loads((root / "herdr-bot.toml").read_text())
            self.assertEqual(config["scheduler"]["bot_directory"], ".data/bots")
            self.assertIn("# keep", (root / "herdr-bot.toml").read_text())
            old_map = read_json(before[".data/workflow-maps/map.json"])
            expected = copy.deepcopy(old_map)
            expected["nodes"]["node_template_id"]["bot_id"] = expected["nodes"]["node_template_id"].pop("node_template_id")
            self.assertEqual(read_json((root / ".data/workflow-maps/map.json").read_bytes()), expected)
            definition = "opaque-node_template_id.json"
            self.assertEqual((root / ".data/bots" / definition).read_bytes(), before[f".data/node-templates/{definition}"])
            self.assertEqual((root / (".data/node-templates" + BACKUP_SUFFIX) / definition).read_bytes(), before[f".data/node-templates/{definition}"])
            for name in ("herdr-bot.toml", ".data/workflow-maps/map.json"):
                self.assertEqual((root / (name + BACKUP_SUFFIX)).read_bytes(), before[name])
            self.assertFalse((root / ".data/node-templates").exists())
            after = self.snapshot(root)
            self.assertEqual(migrate(root), [])
            self.assertEqual(self.snapshot(root), after)

    def test_custom_paths_resolve_from_service_cwd(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.put(root / "config/custom.toml", b"[scheduler]\nnode_template_directory = 'private/capabilities'\nmap_directory = 'private/maps'\n")
            self.put(root / "private/capabilities/a.json", b"unchanged bot bytes\x00")
            self.put(root / "private/maps/a.JSON", b'{"nodes":{"x":{"node_template_id":"opaque","parameters":{"node_template_id":"literal"}}}}')
            migrate(root, Path("config/custom.toml"))
            self.assertEqual(tomllib.loads((root / "config/custom.toml").read_text())["scheduler"], {"bot_directory": "private/capabilities", "map_directory": "private/maps"})
            self.assertEqual((root / "private/capabilities/a.json").read_bytes(), b"unchanged bot bytes\x00")
            self.assertEqual(read_json((root / "private/maps/a.JSON").read_bytes())["nodes"]["x"], {"bot_id": "opaque", "parameters": {"node_template_id": "literal"}})
            self.assertFalse((root / ".data").exists())

    def test_conflicts_preflight_before_any_write(self):
        cases = {
            "directories": (".data/bots/a.json", b"{}"),
            "config": ("herdr-bot.toml", b"[scheduler]\nnode_template_directory='a'\nbot_directory='b'\n"),
            "node": (".data/workflow-maps/z.json", b'{"nodes":{"x":{"node_template_id":"a","bot_id":"b"}}}'),
            "duplicate": (".data/workflow-maps/z.json", b'{"nodes":{},"nodes":{}}'),
            "invalid": (".data/workflow-maps/z.json", b"not json"),
            "backup": (".data/workflow-maps/map.json" + BACKUP_SUFFIX, b"occupied"),
            "directory backup": (".data/node-templates" + BACKUP_SUFFIX + "/a.json", b"occupied"),
            "staging": (".data/workflow-maps/map.json" + TEMP_SUFFIX, b"occupied"),
            "root overlap": ("herdr-bot.toml", b"[scheduler]\nmap_directory='.data/node-templates/maps'\n"),
            "recovery overlap": ("herdr-bot.toml", b"[scheduler]\nmap_directory='.data/node-templates.pre-node-template-to-bot'\n"),
            "unsupported config": ("herdr-bot.toml", b"scheduler = {node_template_directory = '.data/node-templates'}\n"),
        }
        for label, (name, data) in cases.items():
            with self.subTest(label=label), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                self.seed(root)
                self.put(root / name, data)
                before = self.snapshot(root)
                for dry_run in (True, False):
                    with self.assertRaises(MigrationConflict):
                        migrate(root, dry_run=dry_run)
                    self.assertEqual(self.snapshot(root), before)

    def test_map_only_rewrites_immediate_key_tokens(self):
        raw = b'\xef\xbb\xbf{ "description":"node_template_id", "nodes": {"node_template_id": {"node_template_\\u0069d" : "opaque-node-template", "parameters":{"node_template_id":"literal", "float":1.234567890123456789, "negative":-0, "huge":1e999}}}}\r\n'
        expected = raw.replace(b'"node_template_\\u0069d" :', b'"bot_id" :')
        self.assertEqual(converted_map(raw), expected)
        self.assertEqual(converted_map(expected), expected)

    def test_updated_config_recovers_default_old_directory(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.seed(root, b"[scheduler]\nbot_directory = '.data/bots'\n")
            original_config = (root / "herdr-bot.toml").read_bytes()
            migrate(root)
            self.assertEqual((root / "herdr-bot.toml").read_bytes(), original_config)
            self.assertTrue((root / ".data/bots/opaque-node_template_id.json").is_file())
            self.assertEqual(migrate(root), [])

    def test_absolute_custom_paths_remain_in_place(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            definitions = root / "custom definitions"
            maps = root / "custom maps"
            config = ("[scheduler]\nnode_template_directory = " + json.dumps(definitions.as_posix()) + "\nmap_directory = " + json.dumps(maps.as_posix()) + "\n").encode()
            self.put(root / "herdr-bot.toml", config)
            self.put(definitions / "a.json", b"original definition\r\n")
            self.put(maps / "a.json", b'{"nodes":{"a":{"node_template_id":"opaque"}}}')
            migrate(root)
            self.assertEqual(tomllib.loads((root / "herdr-bot.toml").read_text())["scheduler"]["bot_directory"], definitions.as_posix())
            self.assertEqual((definitions / "a.json").read_bytes(), b"original definition\r\n")
            self.assertFalse((root / ".data").exists())

    def test_missing_config_does_not_create_data(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.assertRaises(FileNotFoundError):
                migrate(root)
            self.assertEqual(self.snapshot(root), {})

    @unittest.skipUnless(os.environ.get("HERDR_BOT_MANUAL_SYMLINKS") == "1",
                         "Manual-only: set HERDR_BOT_MANUAL_SYMLINKS=1 to exercise real symbolic links")
    def test_link_input_rejected_without_writing(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.seed(root)
            outside = root / "outside.json"
            outside.write_bytes(b'{"nodes":{}}')
            link = root / ".data/workflow-maps/linked.json"
            try:
                link.symlink_to(outside)
            except OSError:
                self.skipTest("Creating symlinks is not permitted on this host")
            before = self.snapshot(root)
            with self.assertRaises(MigrationConflict):
                migrate(root)
            self.assertEqual(self.snapshot(root), before)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--cwd", type=Path, help="Required service working directory; all relative config/data paths resolve here")
    parser.add_argument("--config", type=Path, default=Path("herdr-bot.toml"), help="Existing TOML configuration, relative to --cwd unless absolute (default: herdr-bot.toml)")
    parser.add_argument("--dry-run", action="store_true", help="Preflight and print the exact backup/write/rename plan without writing")
    parser.add_argument("--self-test", action="store_true", help="Run only isolated temporary-directory migration regressions (no --cwd needed)")
    args = parser.parse_args()
    if args.self_test:
        unittest.main(argv=[sys.argv[0]])
    if args.cwd is None:
        parser.error("--cwd is required; pass the stopped service's working directory explicitly")
    try:
        actions = migrate(args.cwd, args.config, dry_run=args.dry_run)
    except (OSError, ValueError, TypeError) as error:
        parser.exit(1, f"Migration refused/failed: {error}\nRecovery backups, if written, have not been discarded.\n")
    for action in actions:
        print(action)
    print("Dry-run complete; no files written." if args.dry_run else "NodeTemplate -> Bot migration complete; unrelated values and Bot definition bytes untouched.")
    if not actions:
        print("No changes required.")
