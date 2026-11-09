"""CLI safety checks, not a substitute for P03's real-model native scenarios.

Approved seam: delivered acceptance command and its persisted evidence. A missing
worktree build must fail before launching hosts, and cannot be called P03 passed.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

REPO = Path(__file__).resolve().parent.parent


class PlanningAcceptancePrerequisites(unittest.TestCase):
    def test_existing_evidence_directory_is_not_reused_or_overwritten(self):
        # Ticket23 requires an exclusive invocation root. A runner that merges
        # records into an old successful run could fabricate passing evidence.
        with tempfile.TemporaryDirectory(prefix='planning-existing-evidence-') as directory:
            root = Path(directory)
            output = root / 'already-owned'
            output.mkdir()
            old = output / 'evidence.json'
            old.write_bytes(b'old invocation bytes must remain unchanged\n')
            result = subprocess.run([sys.executable, str(REPO / 'scripts/accept-workflow-nesting-planning.py'),
                '--backend', str(root / 'missing.exe'), '--toolkit', str(root / 'missing-toolkit'),
                '--hosts', 'omp,pi', '--paths-file', str(root / 'missing.toml'),
                '--model-config', str(root / 'missing-model.json'), '--evidence-dir', str(output)],
                cwd=REPO, capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=30)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(old.read_bytes(), b'old invocation bytes must remain unchanged\n')
            self.assertEqual(sorted(path.name for path in output.iterdir()), ['evidence.json'])
            self.assertEqual(result.stdout, '')

    def test_missing_backend_records_unverified_p03_without_starting_a_host(self):
        # Independent source: ticket23 requires the exact built worktree backend.
        # Plausible violation: treating prepared input/configuration as success.
        # Observe only public CLI exit, evidence and absence of process launches.
        toolkit = Path(os.environ['PLANNING_ACCEPTANCE_TOOLKIT']).resolve()
        with tempfile.TemporaryDirectory(prefix='planning-cli-') as directory:
            root = Path(directory)
            paths = root / 'paths.toml'
            paths.write_text('[roots]\n', encoding='utf-8')
            output = root / 'evidence'
            result = subprocess.run([sys.executable, str(REPO / 'scripts/accept-workflow-nesting-planning.py'),
                '--backend', str(root / 'not-built/debug/herdr-bot.exe'),
                '--toolkit', str(toolkit), '--hosts', 'declared', '--paths-file', str(paths),
                '--model-config', str(root / 'missing-model.json'), '--evidence-dir', str(output)],
                cwd=REPO, capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=30)
            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
            summary = json.loads(result.stdout)
            evidence = json.loads(Path(summary['evidence']).read_text(encoding='utf-8'))
            self.assertFalse(evidence['passed'])
            self.assertFalse(evidence['coverage']['P03']['verified'])
            self.assertIn('Missing current worktree backend', evidence['verificationError'])
            self.assertEqual(evidence['processes'], [])
            self.assertEqual(evidence['scenarios'], [])
            self.assertEqual(evidence['cleanupErrors'], [])
            self.assertEqual(evidence['requestedHosts'], 'declared')
            self.assertFalse(any(output.rglob('models.json')))


if __name__ == '__main__':
    unittest.main()
