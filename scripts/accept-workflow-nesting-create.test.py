"""Approved seam: the acceptance CLI, not a product-private implementation.

Independent source: missing build/model/host prerequisites must be nonzero and
must never become P39–P45 verification. A plausible violation calls preparation
success, starts a user host, or reuses someone else's evidence directory.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

REPO = Path(__file__).resolve().parents[1]


class CreateAcceptancePrerequisites(unittest.TestCase):
    def test_missing_backend_records_unverified_coverage_without_starting_services(self):
        with tempfile.TemporaryDirectory(prefix='create-cli-') as directory:
            root = Path(directory)
            target = root / 'target'
            paths = root / 'paths.toml'
            paths.write_text('[roots]\n', encoding='utf-8')
            evidence = root / 'evidence'
            command = [sys.executable, str(REPO / 'scripts/accept-workflow-nesting-create.py'),
                '--backend', str(target / 'debug/herdr-bot.exe'),
                '--toolkit', os.environ['WORKFLOW_HANDOFF_TOOLKIT'], '--hosts', 'declared',
                '--paths-file', str(paths), '--model-config', str(root / 'model.json'),
                '--evidence-dir', str(evidence)]
            result = subprocess.run(command, cwd=REPO, env=dict(os.environ, CARGO_TARGET_DIR=str(target)),
                capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=30)
            self.assertNotEqual(result.returncode, 0)
            self.assertTrue((evidence / 'evidence.json').is_file(), result.stdout + result.stderr)
            observed = json.loads((evidence / 'evidence.json').read_text(encoding='utf-8'))
            self.assertFalse(observed['passed'])
            self.assertIn('Missing current worktree backend', observed['verificationError'])
            self.assertEqual(observed['verifiedHosts'], [])
            self.assertEqual(observed['commands'], [])
            self.assertEqual(observed['cleanupErrors'], [])
            self.assertEqual(set(observed['coverage']), {'P39', 'P40', 'P41', 'P43', 'P44', 'P45'})
            self.assertTrue(all(not value['verified'] for value in observed['coverage'].values()))
            self.assertFalse((evidence / 'backend-stdout.log').exists())
            sentinel = evidence / 'not-owned.txt'
            sentinel.write_bytes(b'KEEP-EXISTING-EVIDENCE\r\n')
            again = subprocess.run(command, cwd=REPO, capture_output=True, timeout=30)
            self.assertNotEqual(again.returncode, 0)
            self.assertEqual(sentinel.read_bytes(), b'KEEP-EXISTING-EVIDENCE\r\n')
            self.assertEqual(json.loads((evidence / 'evidence.json').read_text(encoding='utf-8')), observed)

    def test_missing_authorized_model_fails_before_any_dependency_or_host_command(self):
        with tempfile.TemporaryDirectory(prefix='create-model-cli-') as directory:
            root = Path(directory)
            target = root / 'target'
            backend = target / 'debug/herdr-bot.exe'
            backend.parent.mkdir(parents=True)
            # Not executable: the model prerequisite must reject this before
            # attempting any product/dependency command. Never a success fixture.
            backend.write_bytes(b'NEVER-EXECUTE-PREREQUISITE-SENTINEL')
            evidence = root / 'evidence'
            result = subprocess.run([sys.executable, str(REPO / 'scripts/accept-workflow-nesting-create.py'),
                '--backend', str(backend), '--toolkit', os.environ['WORKFLOW_HANDOFF_TOOLKIT'],
                '--hosts', 'declared', '--paths-file', str(root / 'missing-paths.toml'),
                '--model-config', str(root / 'missing-authorized-model.json'), '--evidence-dir', str(evidence)],
                cwd=REPO, env=dict(os.environ, CARGO_TARGET_DIR=str(target)), capture_output=True,
                text=True, encoding='utf-8', errors='replace', timeout=30)
            self.assertNotEqual(result.returncode, 0)
            observed = json.loads((evidence / 'evidence.json').read_text(encoding='utf-8'))
            self.assertIn('Missing explicit authorized model configuration', observed['verificationError'])
            self.assertFalse(observed['passed'])
            self.assertEqual(observed['commands'], [])
            self.assertEqual(observed['verifiedHosts'], [])
            self.assertTrue(all(not item['verified'] for item in observed['coverage'].values()))
            self.assertFalse((evidence / 'agent/models.json').exists())


if __name__ == '__main__':
    unittest.main()
