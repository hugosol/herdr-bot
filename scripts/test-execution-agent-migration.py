"""Offline execution-agent cutover contracts; isolated files, never deployment data."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).with_name("migrate-execution-agents.py")


class ExecutionAgentMigrationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="execution-agent-migration-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "config.toml").write_text("[scheduler]\nrun_directory = 'runs'\n", encoding="utf-8")
        self.directory = self.root / "runs" / "old-run"
        self.directory.mkdir(parents=True)
        self.store = self.directory / "run.json"
        self.execution = {"id": "old-execution", "taskId": "changed-task", "zone": "D:/work", "chat": None}
        self.document = {"id": "old-run", "nodes": {"A": {"status": "failed", "execution": self.execution,
            "previousExecutions": [{"execution": {**self.execution, "id": "earlier"}, "failure": None}]}},
            "userInput": "原文保留"}
        self.store.write_text(json.dumps(self.document, ensure_ascii=False), encoding="utf-8")
        self.history = self.directory / "history.jsonl"
        self.history.write_bytes(b'{"action":"dispatch","execution":{"id":"untyped-history"}}\n')

    def invoke(self):
        return subprocess.run([sys.executable, str(SCRIPT), "--root", str(self.root), "--config", "config.toml"],
                              capture_output=True, text=True, timeout=10, cwd=self.root)

    def test_migrates_current_and_previous_execution_without_rewriting_history_then_is_noop(self):
        original, history = self.store.read_bytes(), self.history.read_bytes()
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        expected = json.loads(original)
        expected["nodes"]["A"]["execution"]["agent"] = "omp"
        expected["nodes"]["A"]["previousExecutions"][0]["execution"]["agent"] = "omp"
        self.assertEqual(json.loads(self.store.read_bytes()), expected)
        backup = self.store.with_name("run.json.pre-execution-agents")
        self.assertEqual(backup.read_bytes(), original)
        migrated = self.store.read_bytes()
        self.assertEqual(self.invoke().returncode, 0)
        self.assertEqual(self.store.read_bytes(), migrated)
        self.assertEqual(self.history.read_bytes(), history)
        self.assertEqual(backup.read_bytes(), original)

    def test_existing_pi_and_null_execution_are_preserved_without_current_task_lookup(self):
        self.document["nodes"]["A"]["execution"] = {**self.execution, "agent": "pi"}
        self.document["nodes"]["A"]["previousExecutions"] = [{"execution": None, "failure": {"stage": "start"}}]
        self.store.write_text(json.dumps(self.document), encoding="utf-8")
        original = self.store.read_bytes()
        result = self.invoke()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(self.store.read_bytes(), original)
        self.assertFalse(self.store.with_name("run.json.pre-execution-agents").exists())

    def test_preflights_every_run_before_writing_any_document(self):
        other = self.root / "runs" / "broken-run"
        other.mkdir()
        (other / "run.json").write_text('{"nodes":{"A":{"execution":{"agent":"unknown"},"previousExecutions":[]}}}', encoding="utf-8")
        original = self.store.read_bytes()
        result = self.invoke()
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertEqual(self.store.read_bytes(), original)
        self.assertFalse(self.store.with_name("run.json.pre-execution-agents").exists())

    def test_conflicting_backup_refuses_without_overwriting(self):
        backup = self.store.with_name("run.json.pre-execution-agents")
        backup.write_bytes(b"independent backup")
        original = self.store.read_bytes()
        result = self.invoke()
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertEqual(self.store.read_bytes(), original)
        self.assertEqual(backup.read_bytes(), b"independent backup")


if __name__ == "__main__":
    unittest.main()
