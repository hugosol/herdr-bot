"""S4 Task migration CLI contracts; isolated files, never user data."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).with_name("migrate-task-types.py")


class TaskTypeMigrationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="task-type-migration-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "config.toml").write_text("[tasks]\nstorage_file = 'presets.json'\n[sessions]\nhistory_file = 'chats.json'\n", encoding="utf-8")
        self.task = {"id": "node-manager-stable", "name": "manager", "chatName": "{task.name}",
                     "agent": "pi", "model": "default", "thinking": "default", "skill": None,
                     "prompt": "node\nmanager", "extensions": []}
        self.store = self.root / "presets.json"
        self.store.write_text(json.dumps({"tasks": [self.task]}), encoding="utf-8")
        self.history = self.root / "chats.json"
        self.history.write_text('[{"sessionId":"old","sourceTaskId":"node-manager-stable"}]\n', encoding="utf-8")

    def invoke(self):
        return subprocess.run([sys.executable, str(SCRIPT), "--root", str(self.root), "--config", "config.toml"],
                              capture_output=True, text=True, timeout=10, cwd=self.root)

    def test_conflicting_backup_refuses_without_changing_task_or_chat_bytes(self):
        before = self.store.read_bytes(), self.history.read_bytes()
        backup = self.store.with_name("presets.json.pre-task-types")
        backup.write_bytes(b"independent previous backup")
        result = self.invoke()
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("Backup already exists", result.stderr)
        self.assertEqual((self.store.read_bytes(), self.history.read_bytes()), before)
        self.assertEqual(backup.read_bytes(), b"independent previous backup")

    def test_duplicate_json_fields_refuse_without_overwriting_original(self):
        self.store.write_text('{"tasks":[],"tasks":[]}', encoding="utf-8")
        before = self.store.read_bytes()
        result = self.invoke()
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("Duplicate JSON field", result.stderr)
        self.assertEqual(self.store.read_bytes(), before)
        self.assertFalse(self.store.with_name("presets.json.pre-task-types").exists())

    def test_staging_conflict_is_not_deleted_or_overwritten(self):
        staging = self.store.with_name("presets.json.task-types.tmp")
        staging.write_bytes(b"retain failed migration evidence")
        before = self.store.read_bytes()
        result = self.invoke()
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertEqual(staging.read_bytes(), b"retain failed migration evidence")
        self.assertEqual(self.store.read_bytes(), before)

    def test_prior_task_with_omitted_optional_fields_preserves_its_identity_and_fields(self):
        # The previous Task reader allowed omitted Skill and optional extensions.
        del self.task["skill"]
        del self.task["extensions"]
        self.store.write_text(json.dumps({"tasks": [self.task]}), encoding="utf-8")
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(json.loads(self.store.read_bytes()), {"tasks": [{**self.task, "taskType": "normal"}]})


if __name__ == "__main__":
    unittest.main()
