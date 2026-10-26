#!/usr/bin/env python3
"""Offline Bot title -> name migration (Python 3.11+); stop the service first.

Requires --cwd; --config defaults to herdr-bot.toml relative to that cwd.
Only immediate JSON files in scheduler.bot_directory (default .data/bots)
are considered. Only the top-level title key token changes: all other bytes
remain untouched. Every input and recovery path is preflighted before writes.
Backups use .pre-bot-title-to-name; --dry-run performs preflight without writes.
--self-test exercises isolated temporary directories, never the supplied cwd.
Real symlink checks run only with HERDR_BOT_MANUAL_SYMLINKS=1.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import stat
import sys
import tempfile
import tomllib
import unittest


BACKUP_SUFFIX = ".pre-bot-title-to-name"
TEMP_SUFFIX = ".bot-title-to-name-migration.tmp"


class MigrationConflict(ValueError):
    pass


def safe_path(path: Path) -> Path:
    """Reject symlinks, junctions and other reparse points in all ancestors."""
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


def regular_file(path: Path) -> None:
    safe_path(path)
    metadata = path.stat()
    if not stat.S_ISREG(metadata.st_mode):
        raise MigrationConflict(f"Not a regular file: {path}")
    if metadata.st_nlink > 1:
        raise MigrationConflict(f"Hard-linked file is not allowed: {path}")


def converted_bot(raw: bytes) -> bytes:
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise MigrationConflict(f"Duplicate JSON key: {key}")
            result[key] = value
        return result

    def invalid_constant(value):
        raise MigrationConflict(f"Invalid JSON constant: {value}")

    text = raw.decode("utf-8-sig")
    original = json.loads(text, object_pairs_hook=unique, parse_constant=invalid_constant)
    if not isinstance(original, dict):
        raise MigrationConflict("Bot must be a JSON object")
    if "title" in original and "name" in original:
        raise MigrationConflict("Bot contains both title and name")
    if "title" not in original:
        return raw
    # Decode only to locate top-level key spans, never to serialize values.
    # This preserves numeric lexemes, escapes, whitespace, BOM and line endings.
    decoder = json.JSONDecoder()

    def whitespace(position):
        while position < len(text) and text[position] in " \t\r\n":
            position += 1
        return position

    position = whitespace(whitespace(0) + 1)
    while text[position] != "}":
        start = position
        key, end = decoder.raw_decode(text, start)
        if key == "title":
            output = text[:start] + '"name"' + text[end:]
            return (b"\xef\xbb\xbf" if raw.startswith(b"\xef\xbb\xbf") else b"") + output.encode("utf-8")
        value_start = whitespace(whitespace(end) + 1)
        _, value_end = decoder.raw_decode(text, value_start)
        position = whitespace(value_end)
        if text[position] != "}":
            position = whitespace(position + 1)
    raise MigrationConflict("Cannot locate top-level title key")


def migrate(cwd: Path, config_path: Path = Path("herdr-bot.toml"), *, dry_run: bool = False) -> list[str]:
    root = safe_path(cwd)
    if not root.is_dir():
        raise MigrationConflict(f"Service cwd is not a directory: {root}")

    def absolute(path: Path) -> Path:
        return safe_path(path if path.is_absolute() else root / path)

    config = absolute(config_path)
    regular_file(config)
    parsed = tomllib.loads(config.read_bytes().decode("utf-8-sig"))
    settings = parsed.get("scheduler", {})
    if not isinstance(settings, dict):
        raise MigrationConflict("scheduler must be a TOML table")
    directory = settings.get("bot_directory", ".data/bots")
    if not isinstance(directory, str) or not directory:
        raise MigrationConflict("scheduler.bot_directory must be a nonempty path string")
    definitions = absolute(Path(directory))
    if not definitions.exists():
        return []
    if not definitions.is_dir():
        raise MigrationConflict(f"Bot directory is not a directory: {definitions}")
    changes: dict[Path, tuple[bytes, bytes]] = {}
    for path in sorted(definitions.iterdir()):
        # Links are rejected even when their targets are directories/non-JSON.
        # Ordinary subdirectories are not traversed: only direct Bot JSON files.
        safe_path(path)
        if path.suffix.lower() != ".json":
            continue
        regular_file(path)
        raw = path.read_bytes()
        try:
            converted = converted_bot(raw)
        except (ValueError, TypeError, RecursionError) as error:
            raise MigrationConflict(f"Bot {path}: {error}") from error
        if raw != converted:
            changes[path] = (raw, converted)
    backups = {path: path.with_name(path.name + BACKUP_SUFFIX) for path in changes}
    staged = {path: path.with_name(path.name + TEMP_SUFFIX) for path in changes}
    for path in [*backups.values(), *staged.values()]:
        safe_path(path)
        if path.exists():
            raise MigrationConflict(f"Backup/staging target already exists: {path}")
    actions = [f"BACKUP {path} -> {backup}" for path, backup in backups.items()]
    actions += [f"WRITE {path}" for path in changes]
    if dry_run:
        return actions
    # All discovery, JSON validation and recovery-path checks finish above.
    # The service must remain stopped throughout this offline migration.
    for path, backup in backups.items():
        with backup.open("xb") as stream:
            stream.write(changes[path][0])
    written = []
    try:
        for path, (_, data) in changes.items():
            with staged[path].open("xb") as stream:
                written.append(staged[path])
                stream.write(data)
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
        return {path.relative_to(root).as_posix(): ("directory" if path.is_dir() else path.read_bytes())
                for path in root.rglob("*")}

    def seed(self, root: Path, config: bytes = b"[scheduler]\n") -> Path:
        self.put(root / "herdr-bot.toml", config)
        bot = root / ".data/bots/bot.json"
        self.put(bot, b'{"id":"opaque-id","title":"Bot title"}\n')
        return bot

    def test_preserves_every_byte_except_top_level_key(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            bot = self.seed(root)
            original = (b'\xef\xbb\xbf{\r\n  "id":"opaque-title-id", "ti\\u0074le" : "\\u540d title",'
                        b' "parameters":{"title":"nested", "name":"also keep"},'
                        b' "description":"title name", "number":1.2300e+04, "huge":1e9999\r\n}\r\n')
            expected = (b'\xef\xbb\xbf{\r\n  "id":"opaque-title-id", "name" : "\\u540d title",'
                        b' "parameters":{"title":"nested", "name":"also keep"},'
                        b' "description":"title name", "number":1.2300e+04, "huge":1e9999\r\n}\r\n')
            bot.write_bytes(original)
            self.put(root / ".data/workflow-maps/map.json", b'{"title":"Map", "nodes":{"x":{"title":"node"}}}')
            self.put(root / ".data/bots/nested/not-a-bot.json", b'{invalid nested JSON')
            self.put(root / ".data/bots/readme.txt", b'title')
            before = self.snapshot(root)
            self.assertTrue(migrate(root, dry_run=True))
            self.assertEqual(self.snapshot(root), before)
            migrate(root)
            self.assertEqual(bot.read_bytes(), expected)
            self.assertEqual(bot.with_name(bot.name + BACKUP_SUFFIX).read_bytes(), original)
            for path, content in before.items():
                if path != ".data/bots/bot.json":
                    self.assertEqual(self.snapshot(root)[path], content)
            after = self.snapshot(root)
            self.assertEqual(migrate(root), [])
            self.assertEqual(self.snapshot(root), after)

    def test_all_conflicts_leave_every_original_and_recovery_path_untouched(self):
        cases = [b'{"title":"old","name":"new"}', b'{"title":"a","title":"b"}',
                 b'{"title":"a","ti\\u0074le":"b"}',
                 b'{"title":"a","parameters":{"x":1,"x":2}}', b'{"title":',
                 b'{"title":NaN}', b'[]', b'\xff', b'{"title":"a"} trailing']
        for bad in cases:
            with self.subTest(bad=bad), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                self.seed(root)
                self.put(root / ".data/bots/z-invalid.json", bad)
                before = self.snapshot(root)
                with self.assertRaises(MigrationConflict):
                    migrate(root)
                self.assertEqual(self.snapshot(root), before)
        for suffix in (BACKUP_SUFFIX, TEMP_SUFFIX):
            with self.subTest(suffix=suffix), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                bot = self.seed(root)
                self.put(root / ".data/bots/a-valid.json", b'{"title":"first"}')
                self.put(bot.with_name(bot.name + suffix), b"existing recovery data")
                before = self.snapshot(root)
                with self.assertRaises(MigrationConflict):
                    migrate(root)
                self.assertEqual(self.snapshot(root), before)

    def test_custom_cwd_relative_and_absolute_paths(self):
        with tempfile.TemporaryDirectory() as temporary:
            outer = Path(temporary)
            root = outer / "service working directory"
            config = root / "configuration/custom.toml"
            self.put(config, b'[scheduler]\nbot_directory = "custom bots"\n')
            bot = root / "custom bots/custom.json"
            self.put(bot, b'{ "title": "custom" }')
            migrate(root, Path("configuration/custom.toml"))
            self.assertEqual(bot.read_bytes(), b'{ "name": "custom" }')
            external = outer / "absolute bots"
            self.put(external / "absolute.json", b'{"title":"external"}')
            config.write_text("[scheduler]\nbot_directory = " + json.dumps(external.as_posix()) + "\n", encoding="utf-8")
            migrate(root, config)
            self.assertEqual((external / "absolute.json").read_bytes(), b'{"name":"external"}')

    def test_name_only_files_are_unchanged(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            bot = self.seed(root)
            bot.write_bytes(b'{"name":"already migrated", "parameters":{"title":"keep"}}\n')
            before = self.snapshot(root)
            self.assertEqual(migrate(root), [])
            self.assertEqual(self.snapshot(root), before)

    @unittest.skipUnless(os.environ.get("HERDR_BOT_MANUAL_SYMLINKS") == "1",
                         "Manual-only: set HERDR_BOT_MANUAL_SYMLINKS=1 to exercise real symbolic links")
    def test_symlinks_are_rejected_without_writes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            bot = self.seed(root)
            target = root / "outside.json"
            self.put(target, b'{"title":"outside"}')
            link = bot.parent / "z-link.json"
            try:
                link.symlink_to(target)
            except (OSError, NotImplementedError) as error:
                print(f"Symlink regression unavailable: {type(error).__name__}: {error}", file=sys.stderr)
                self.skipTest(f"symlink creation failed: {type(error).__name__}: {error}")
            original = bot.read_bytes()
            outside = target.read_bytes()
            with self.assertRaises(MigrationConflict):
                migrate(root)
            self.assertEqual(bot.read_bytes(), original)
            self.assertEqual(target.read_bytes(), outside)
            self.assertFalse(bot.with_name(bot.name + BACKUP_SUFFIX).exists())
            self.assertFalse(bot.with_name(bot.name + TEMP_SUFFIX).exists())
            link.unlink()
            directory_link = root / "linked bots"
            directory_link.symlink_to(bot.parent, target_is_directory=True)
            (root / "herdr-bot.toml").write_text('[scheduler]\nbot_directory="linked bots"\n', encoding="utf-8")
            with self.assertRaises(MigrationConflict):
                migrate(root)
            self.assertEqual(bot.read_bytes(), original)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cwd", type=Path, required=True, help="Explicit service working directory; stop the service first")
    parser.add_argument("--config", type=Path, default=Path("herdr-bot.toml"), help="Config path, relative to --cwd or absolute")
    parser.add_argument("--dry-run", action="store_true", help="Run full preflight and print actions without writing")
    parser.add_argument("--self-test", action="store_true", help="Run isolated temporary-directory regressions only")
    args = parser.parse_args()
    if args.self_test:
        result = unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromTestCase(MigrationTests))
        return 0 if result.wasSuccessful() else 1
    try:
        actions = migrate(args.cwd, args.config, dry_run=args.dry_run)
    except (OSError, ValueError, RecursionError) as error:
        print(f"Migration refused: {error}", file=sys.stderr)
        return 1
    print("DRY RUN: no files changed" if args.dry_run else "Offline Bot title -> name migration complete")
    print("\n".join(actions) if actions else "No changes required")
    return 0


if __name__ == "__main__":
    sys.exit(main())
