"""S3 CLI contract: invalid provenance must fail before any owned server starts.

Independent violation: accepting an unrelated executable could report tests against
another worktree. Observe nonzero, explicit mismatch and all coverage unverified.
This test verifies the runner boundary, not the real S4 resource/MCP behavior.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent


class CliContract(unittest.TestCase):
    def test_foreign_binary_cannot_be_reported_as_this_source(self):
        toolkit = Path(os.environ['SUITE_ACCEPTANCE_TOOLKIT']).resolve()
        with tempfile.TemporaryDirectory(prefix='pi-resource-cli-') as directory:
            root = Path(directory)
            paths = root / 'isolated.toml'
            paths.write_text('[roots]\n')
            result = subprocess.run([sys.executable, str(ROOT / 'scripts/accept-suite-pi-resources.py'),
                '--herdr-bot-worktree', str(ROOT), '--toolkit-worktree', str(toolkit),
                '--backend', sys.executable, '--paths-file', str(paths),
                '--evidence-dir', str(root / 'evidence'), '--scenario', 'all'],
                capture_output=True, text=True, timeout=30)
            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
            evidence = json.loads((root / 'evidence/evidence.json').read_text())
            self.assertIn('backend provenance', evidence['error'])
            self.assertFalse(evidence['passed'])
            self.assertEqual(evidence['startedProcesses'], [])
            self.assertTrue(evidence['coverage'])
            self.assertTrue(all(not item['verified'] for item in evidence['coverage'].values()))


if __name__ == '__main__':
    unittest.main()
