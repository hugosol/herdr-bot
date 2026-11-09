"""Approved seam: ticket 15 public CLI and evidence, not private helpers.

Independent oracle: missing host/model/backend must fail, remain unverified and
start no services. A runner that calls preparation a passing recovery violates
that oracle. This subprocess reaches the absent-backend prerequisite with both
hosts requested; no provider or native process is substituted.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

REPO = Path(__file__).resolve().parents[1]


class RecoveryPrerequisites(unittest.TestCase):
    def test_missing_backend_preserves_unverified_both_hosts_without_starting_services(self):
        toolkit = Path(os.environ['WORKFLOW_HANDOFF_TOOLKIT']).resolve()
        with tempfile.TemporaryDirectory(prefix='recovery-prerequisites-') as directory:
            root = Path(directory)
            paths = root / 'paths.toml'
            paths.write_text('[roots]\n', encoding='utf-8')
            result = subprocess.run([sys.executable, str(REPO / 'scripts/accept-workflow-nesting-recovery.py'),
                '--backend', str(root / 'absent.exe'), '--toolkit', str(toolkit), '--hosts', 'omp,pi',
                '--paths-file', str(paths), '--model-config', str(root / 'model.json'),
                '--evidence-dir', str(root / 'evidence')], cwd=REPO, capture_output=True,
                text=True, encoding='utf-8', errors='replace', timeout=30)
            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
            evidence = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertFalse(evidence['passed'])
            self.assertIn('Missing current worktree backend', evidence['verificationError'])
            self.assertEqual(evidence['hosts'], ['omp', 'pi'])
            self.assertEqual(evidence['runs'], [])
            self.assertEqual(evidence['cleanupErrors'], [])
            self.assertTrue(all(not item['verified'] for item in evidence['coverage'].values()))
            self.assertEqual(list((root / 'evidence').glob('**/*stdout.log')), [])


if __name__ == '__main__':
    unittest.main()
