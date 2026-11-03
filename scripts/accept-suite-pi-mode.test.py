"""S3 public acceptance CLI prerequisites; not S4 success evidence.

Independent expectation: a missing current-worktree binary cannot prove any Pi
promise. A runner which reports preparation as acceptance must fail this test.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

REPO = Path(__file__).resolve().parent.parent


class AcceptancePrerequisites(unittest.TestCase):
    def test_missing_binary_preserves_all_promises_as_unverified(self):
        toolkit = Path(os.environ['PI_MODE_TOOLKIT_WORKTREE']).resolve()
        with tempfile.TemporaryDirectory(prefix='pi-mode-cli-') as directory:
            root = Path(directory)
            paths = root / 'isolated-paths.toml'
            paths.write_text('[roots]\n', encoding='utf-8')
            evidence = root / 'evidence'
            result = subprocess.run([sys.executable, str(REPO / 'scripts/accept-suite-pi-mode.py'),
                '--herdr-bot-worktree', str(REPO), '--toolkit-worktree', str(toolkit),
                '--backend', str(root / 'missing.exe'), '--paths-file', str(paths),
                '--evidence-dir', str(evidence)], cwd=REPO, capture_output=True,
                text=True, encoding='utf-8', errors='replace', timeout=30)
            self.assertNotEqual(result.returncode, 0)
            self.assertTrue((evidence / 'evidence.json').is_file(), result.stdout + result.stderr)
            record = json.loads((evidence / 'evidence.json').read_text(encoding='utf-8'))
            self.assertFalse(record['passed'])
            self.assertIn('Missing current worktree backend', record['verificationError'])
            self.assertEqual(set(record['coverage']), {'P14', 'P16', 'P23', 'P24', 'P26', 'P27'})
            self.assertTrue(all(not item['verified'] for item in record['coverage'].values()))
            self.assertEqual(record['commands'], [])
            self.assertEqual(record['cleanupErrors'], [])
            self.assertFalse((evidence / 'backend-stdout.log').exists())


if __name__ == '__main__':
    unittest.main()
