#!/usr/bin/env python3
"""Offline Bot/Chat cutover. Stop herdr-bot first; use --root for its working directory.

Requires Python 3.11+. Original changed files are retained as *.pre-bot-chat.
Only configuration and preset JSON are touched, never sessions, history or zones.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import sys
import tempfile
import tomllib
import unittest


class MigrationConflict(ValueError):
    pass


def naming_rule(value: str) -> str:
    # One pass: the new bot.name token must not be interpreted as an old token.
    return re.sub(r"\{template\.name\}|\{bot\.id\}",
                  lambda match: {"{template.name}": "{bot.name}",
                                 "{bot.id}": "{chat.id}"}[match[0]], value)


def converted_store(raw: bytes) -> bytes:
    store = json.loads(raw)
    if not isinstance(store, dict):
        raise MigrationConflict("Preset store must be a JSON object")
    if "templates" in store:
        if "bots" in store:
            raise MigrationConflict("Store contains both templates and bots")
        store["bots"] = store.pop("templates")
    if not isinstance(store.get("bots"), list):
        raise MigrationConflict("Preset store must contain a bots/templates array")
    for bot in store["bots"]:
        if not isinstance(bot, dict):
            raise MigrationConflict("Every preset must be a JSON object")
        if "botName" in bot:
            if "chatName" in bot:
                raise MigrationConflict(f"Preset {bot.get('id')!r} has both naming fields")
            bot["chatName"] = naming_rule(bot.pop("botName"))
        # Already-new chatName rules are deliberately not rewritten: {bot.id}
        # can be literal text in the new grammar.
    return (json.dumps(store, ensure_ascii=False, indent=2) + "\n").encode("utf-8")


def migrate(root: Path, config_path: Path = Path("herdr-bot.toml")) -> list[Path]:
    root = root.resolve()
    config = config_path if config_path.is_absolute() else root / config_path
    original = config.read_bytes()
    text = original.decode("utf-8-sig")
    parsed = tomllib.loads(text)
    if "bot_templates" in parsed and "bots" in parsed:
        raise MigrationConflict("Config contains both [bot_templates] and [bots]")
    old_section = parsed.get("bot_templates")
    settings = old_section if old_section is not None else parsed.get("bots", {})
    configured = settings.get("storage_file", ".data/bot-templates.json" if old_section is not None else ".data/bots.json")
    old_path = Path(configured)
    new_path = old_path.with_name("bots.json") if old_path.name == "bot-templates.json" else old_path
    if old_section is not None:
        text = re.sub(r"(?m)^(\s*\[)bot_templates(\]\s*(?:#.*)?)$", r"\1bots\2", text)
    # Update the configured path inside [bots] only, without reserializing other settings.
    if str(new_path) != configured:
        sections = re.split(r"(?m)^(?=\s*\[)", text)
        for index, section in enumerate(sections):
            if re.match(r"\s*\[bots\]", section):
                sections[index] = re.sub(r"(?m)^(\s*storage_file\s*=\s*)(['\"])(.*?)\2", lambda m: m[1] + m[2] + str(new_path).replace('\\', '/') + m[2], section)
        text = "".join(sections)
    # A legacy section with no explicit path now inherits the new runtime default.
    tomllib.loads(text)
    target = new_path if new_path.is_absolute() else root / new_path
    sources = [old_path if old_path.is_absolute() else root / old_path,
               root / ".data/bot-templates.json", root / "bot-templates.json"]
    if target.name == "bots.json":
        # Also recover an explicitly renamed custom location after an interrupted
        # config write, or when the tracked config has already been cut over.
        sources.append(target.with_name("bot-templates.json"))
    # Gather/validate every source before changing anything. Different stores must
    # be reconciled explicitly, not concatenated or selected by precedence.
    existing = list(dict.fromkeys(path.resolve() for path in sources + [target] if path.exists()))
    stores = [(path, converted_store(path.read_bytes())) for path in existing]
    if stores and any(json.loads(data) != json.loads(stores[0][1]) for _, data in stores[1:]):
        raise MigrationConflict("Conflicting preset stores: " + ", ".join(str(path) for path, _ in stores))
    changes: dict[Path, bytes] = {}
    if text.encode("utf-8") != original:
        changes[config] = text.encode("utf-8")
    if stores:
        data = stores[0][1]
        if not target.exists() or json.loads(target.read_bytes()) != json.loads(data):
            changes[target] = data
    retired = [path for path, _ in stores if path != target.resolve()]
    backups = list(dict.fromkeys([path for path in changes if path.exists()] + retired))
    for path in backups:
        backup = path.with_name(path.name + ".pre-bot-chat")
        if backup.exists() and backup.read_bytes() != path.read_bytes():
            raise MigrationConflict(f"Backup already exists with different content: {backup}")
    # Backups use exclusive creation; never overwrite the recovery copy.
    for path in backups:
        backup = path.with_name(path.name + ".pre-bot-chat")
        if not backup.exists():
            with backup.open("xb") as stream:
                stream.write(path.read_bytes())
    for path, data in changes.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        temporary = path.with_name(path.name + ".bot-chat-migration.tmp")
        with temporary.open("xb") as stream:
            stream.write(data)
        temporary.replace(path)
    for path in retired:
        path.unlink()  # Original bytes remain in the checked recovery copy.
    return list(changes) + retired


class MigrationTests(unittest.TestCase):
    def seed(self, root: Path, config: str = "[bot_templates]\n") -> None:
        (root / "herdr-bot.toml").write_text(config, encoding="utf-8")
        (root / ".data").mkdir()
        (root / ".data/bot-templates.json").write_text(json.dumps({"templates": [{
            "id": "tpl-opaque", "name": "Review", "botName": "{template.name}-{bot.id}",
            "prompt": "literal {template.name} {bot.id}", "extensions": ["keep"]}]}), encoding="utf-8")

    def test_preserves_data_and_is_repeatable(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.seed(root)
            untouched = [".data/session-history.json", ".data/zones.json", ".data/bot-sessions/native/context.jsonl"]
            for name in untouched:
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b"native\0\xff")
            migrate(root)
            bot = json.loads((root / ".data/bots.json").read_bytes())["bots"][0]
            self.assertEqual(bot["id"], "tpl-opaque")
            self.assertEqual(bot["chatName"], "{bot.name}-{chat.id}")
            self.assertEqual(bot["prompt"], "literal {template.name} {bot.id}")
            self.assertEqual(bot["extensions"], ["keep"])
            self.assertTrue((root / ".data/bot-templates.json.pre-bot-chat").exists())
            self.assertEqual(migrate(root), [])
            for name in untouched:
                self.assertEqual((root / name).read_bytes(), b"native\0\xff")

    def test_new_config_finds_legacy_default_store(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.seed(root, "[bots]\nstorage_file = '.data/bots.json'\nmetadata_file = 'meta-data/bot-options.toml'\n")
            original_config = (root / "herdr-bot.toml").read_bytes()
            migrate(root)
            self.assertEqual((root / "herdr-bot.toml").read_bytes(), original_config)
            bot = json.loads((root / ".data/bots.json").read_bytes())["bots"][0]
            self.assertEqual(bot["id"], "tpl-opaque")
            self.assertEqual(bot["chatName"], "{bot.name}-{chat.id}")
            self.assertTrue((root / ".data/bot-templates.json.pre-bot-chat").exists())
            self.assertEqual(migrate(root), [])

    def test_new_config_finds_old_store(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.seed(root, "[bots]\nstorage_file = 'private/bots.json'\n")
            (root / "private").mkdir()
            (root / ".data/bot-templates.json").rename(root / "private/bot-templates.json")
            migrate(root)
            self.assertEqual(json.loads((root / "private/bots.json").read_bytes())["bots"][0]["id"], "tpl-opaque")
            self.assertTrue((root / "private/bot-templates.json.pre-bot-chat").exists())

    def test_conflicts_make_no_changes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.seed(root)
            (root / ".data/bots.json").write_text('{"bots": []}', encoding="utf-8")
            before = {path.relative_to(root): path.read_bytes() for path in root.rglob("*") if path.is_file()}
            with self.assertRaises(MigrationConflict):
                migrate(root)
            self.assertEqual(before, {path.relative_to(root): path.read_bytes() for path in root.rglob("*") if path.is_file()})

    def test_explicit_custom_path(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.seed(root, "[bot_templates]\nstorage_file = 'custom.json'\nmetadata_file = 'meta-data/bot-options.toml'\n")
            (root / ".data/bot-templates.json").rename(root / "custom.json")
            migrate(root)
            parsed = tomllib.loads((root / "herdr-bot.toml").read_text(encoding="utf-8"))
            self.assertEqual(parsed["bots"]["storage_file"], "custom.json")
            self.assertEqual(parsed["bots"]["metadata_file"], "meta-data/bot-options.toml")
            self.assertIn("chatName", json.loads((root / "custom.json").read_bytes())["bots"][0])


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd(), help="Server working directory (relative storage paths resolve here)")
    parser.add_argument("--config", type=Path, default=Path("herdr-bot.toml"), help="Config path, relative to --root unless absolute")
    parser.add_argument("--self-test", action="store_true", help="Run isolated migration regressions only")
    args = parser.parse_args()
    if args.self_test:
        unittest.main(argv=[sys.argv[0]])
    else:
        try:
            changed = migrate(args.root, args.config)
        except (OSError, ValueError, TypeError) as error:
            parser.exit(1, f"Migration refused/failed: {error}\nOriginal files/backups have not been discarded.\n")
        for path in changed:
            print(path)
        print("Bot/Chat migration complete; native sessions, history and zones untouched.")
