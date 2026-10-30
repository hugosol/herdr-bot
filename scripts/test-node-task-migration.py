"""Consumer CLI contracts for the explicit node Task cutover; temporary data only."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).with_name("migrate-node-tasks-to-normal.py").resolve()


class NodeTaskMigrationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="node-task-migration-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.config = self.root / "custom.toml"
        self.config.write_text(
            "[tasks]\nstorage_file = 'presets.json'\n"
            "[sessions]\nhistory_file = 'chats.json'\n"
            "[scheduler]\nbot_directory = 'bots'\nrun_directory = 'runs'\nmap_directory = 'maps'\n",
            encoding="utf-8")
        self.task = {"id": "stable-node", "name": "重复预设", "chatName": "{task.name}",
                     "agent": "omp", "model": "custom", "thinking": "high", "skill": None,
                     "prompt": "node\nmanager", "extensions": ["omp-node-chat", "custom"],
                     "taskType": "node", "futureField": {"keep": [1, True, None]}}
        self.store = self.root / "presets.json"
        self.backup = self.root / "presets.json.pre-node-tasks-to-normal"
        self.staging = self.root / "presets.json.node-tasks-to-normal.tmp"
        self.seed([self.task])
        references = {
            "chats.json": b'\xef\xbb\xbf[ {"sourceTaskId":"stable-node", "sessionId":"chat-1"} ]\r\n',
            "bots/bot.json": b'{ "id":"bot-1", "taskId":"stable-node" }\n',
            "runs/run-1/run.json": b'{"nodes":{"a":{"execution":{"taskId":"stable-node","chat":"chat-1"}}}}\r\n',
            "runs/run-1/history.jsonl": b'{"taskId":"stable-node","event":"started"}\n',
            "maps/map.json": b'{"botId":"bot-1"}\n',
            "native/session.jsonl": b'{"sessionId":"chat-1","taskId":"stable-node"}\n',
        }
        for name, raw in references.items():
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(raw)

    def seed(self, tasks):
        self.store.write_text(json.dumps({"tasks": tasks}, ensure_ascii=False), encoding="utf-8")

    def snapshot(self):
        return {path.relative_to(self.root).as_posix(): path.read_bytes()
                for path in self.root.rglob("*") if path.is_file()}

    def invoke(self, *extra, config="custom.toml"):
        return subprocess.run([sys.executable, str(SCRIPT), "--root", str(self.root),
                               "--config", config, *extra], capture_output=True, text=True,
                              timeout=10, cwd=self.root)

    def assert_refused_without_writes(self, message):
        before = self.snapshot()
        result = self.invoke()
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn(message, result.stderr)
        self.assertEqual(self.snapshot(), before)

    def test_omp_pi_nodes_convert_without_deduplication_or_reference_rewrites(self):
        tasks = []
        for agent in ("omp", "pi"):
            for role in ("node", "normal", "manager"):
                tasks.append({**self.task, "id": f"{agent}-{role}", "agent": agent, "taskType": role})
        # Equal presets with distinct identities must remain distinct; never merge IDs.
        tasks.extend([{**self.task, "id": "duplicate-a"}, {**self.task, "id": "duplicate-b"}])
        self.seed(tasks)
        original = self.store.read_bytes()
        references = self.snapshot()
        del references["presets.json"]
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        expected = [{**task, "taskType": "normal"} if task["taskType"] == "node" else task for task in tasks]
        self.assertEqual(json.loads(self.store.read_bytes()), {"tasks": expected})
        self.assertEqual(self.backup.read_bytes(), original)
        after = self.snapshot()
        del after["presets.json"]
        del after[self.backup.name]
        self.assertEqual(after, references)
        once = self.snapshot()
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("No node Task changes needed", result.stdout)
        self.assertEqual(self.snapshot(), once)

    def test_normal_manager_only_store_is_byte_identical_without_backup(self):
        self.seed([{**self.task, "taskType": role, "id": role} for role in ("normal", "manager")])
        before = self.snapshot()
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.snapshot(), before)

    def test_absolute_config_path_uses_explicit_root_for_relative_storage(self):
        result = self.invoke(config=str(self.config))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(json.loads(self.store.read_bytes())["tasks"][0], {**self.task, "taskType": "normal"})

    def test_missing_store_is_noop(self):
        self.store.unlink()
        before = self.snapshot()
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.snapshot(), before)

    def test_root_and_config_are_both_required(self):
        before = self.snapshot()
        for args in (("--root", str(self.root)), ("--config", str(self.config))):
            with self.subTest(args=args):
                result = subprocess.run([sys.executable, str(SCRIPT), *args], capture_output=True,
                                        text=True, timeout=10, cwd=self.root)
                self.assertEqual(result.returncode, 2)
        self.assertEqual(self.snapshot(), before)

    def test_conflicting_backup_is_not_overwritten(self):
        self.backup.write_bytes(b"independent backup")
        self.assert_refused_without_writes("Backup already exists")

    def test_matching_existing_backup_is_retained(self):
        original = self.store.read_bytes()
        self.backup.write_bytes(original)
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.backup.read_bytes(), original)
        self.assertEqual(json.loads(self.store.read_bytes())["tasks"][0]["taskType"], "normal")

    def test_existing_staging_file_is_not_deleted_or_overwritten(self):
        self.staging.write_bytes(b"retain interrupted migration evidence")
        self.assert_refused_without_writes("Staging file already exists")

    def test_duplicate_json_fields_and_duplicate_ids_are_conflicts(self):
        for raw in (b'{"tasks":[],"tasks":[]}',
                    json.dumps({"tasks": [self.task, self.task]}).encode()):
            with self.subTest(raw=raw):
                self.store.write_bytes(raw)
                self.assert_refused_without_writes("Duplicate JSON field" if raw.startswith(b'{"tasks":[]')
                                                   else "Task identities must be nonempty and unique")

    def test_invalid_later_task_does_not_partially_convert_earlier_task(self):
        for invalid in ({**self.task, "id": "bad", "agent": "unknown"},
                        {**self.task, "id": "bad", "taskType": "unknown"},
                        {key: value for key, value in {**self.task, "id": "bad"}.items() if key != "taskType"}):
            with self.subTest(invalid=invalid):
                self.seed([self.task, invalid])
                self.assert_refused_without_writes("expected explicit taskType")

    def test_history_and_scheduler_path_aliases_are_conflicts(self):
        config = self.config.read_text(encoding="utf-8")
        variants = [config.replace("history_file = 'chats.json'", "history_file = 'presets.json'"),
                    config.replace("history_file = 'chats.json'", f"history_file = '{self.backup.name}'")]
        for directory in ("bots", "runs", "maps"):
            variants.append(config.replace("storage_file = 'presets.json'", f"storage_file = '{directory}/tasks.json'"))
        for variant in variants:
            with self.subTest(config=variant):
                self.config.write_text(variant, encoding="utf-8")
                self.assert_refused_without_writes("overlap")

    def test_hard_link_store_is_rejected_without_writes(self):
        alias = self.root / "linked-store.json"
        try:
            os.link(self.store, alias)
        except OSError as error:
            self.skipTest(f"Hard links unavailable: {error}")
        self.assert_refused_without_writes("Hard-linked file")

    @unittest.skipUnless(os.environ.get("HERDR_BOT_MANUAL_SYMLINKS") == "1", "manual-only symbolic-link checks")
    def test_symbolic_links_and_linked_parents_are_rejected_without_writes(self):
        original = self.store.read_bytes()
        for target in (self.store, self.backup, self.staging):
            with self.subTest(target=target.name):
                if target == self.store:
                    target.unlink()
                target.symlink_to(self.root / "chats.json")
                self.assert_refused_without_writes("Linked path")
                target.unlink()
                if target == self.store:
                    target.write_bytes(original)
        linked = self.root / "linked"
        linked.symlink_to(self.root / "bots", target_is_directory=True)
        self.config.write_text("[tasks]\nstorage_file = 'linked/tasks.json'\n", encoding="utf-8")
        self.assert_refused_without_writes("Linked path")


if __name__ == "__main__":
    unittest.main()
