"""S1 launch configuration boundary: incomplete runs must never certify P54/P55."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

REPO = Path(__file__).resolve().parents[1]
CLI = REPO / 'scripts/accept-paired-task-launch.py'


class PairedLaunchBoundary(unittest.TestCase):
    def run_cli(self, root, evidence, *options):
        return subprocess.run([sys.executable, str(CLI), '--herdr-bot-worktree', str(REPO),
                               '--toolkit-worktree', str(root / 'missing-toolkit'),
                               '--backend', str(root / 'missing.exe'), '--isolated-config', str(root / 'missing.json'),
                               '--evidence-dir', str(evidence), '--combinations', 'all', *options],
                              cwd=REPO, capture_output=True, text=True, timeout=15)

    def test_missing_dependencies_leave_entire_matrix_unverified(self):
        # A false-success runner can stop at preflight without listing the missing
        # native cells. Require a nonzero exit and the independent four-cell matrix.
        with tempfile.TemporaryDirectory() as path:
            root = Path(path)
            result = self.run_cli(root, root / 'new')
            self.assertEqual(result.returncode, 1, result.stderr)
            report = json.loads((root / 'new/summary.json').read_text(encoding='utf-8'))
            self.assertFalse(report['passed'])
            self.assertIn('preflightError', report)
            self.assertEqual({name: cell['status'] for name, cell in report['combinations'].items()}, {
                'omp-tui': 'unverified', 'omp-app': 'unverified', 'pi-tui': 'unverified', 'pi-app': 'unverified',
            })
            self.assertTrue(all(cell['status'] == 'unverified' for cell in report['additional'].values()))

    def test_refuses_to_overwrite_existing_evidence(self):
        with tempfile.TemporaryDirectory() as path:
            root = Path(path)
            original = root / 'summary.json'
            original.write_bytes(b'prior independent evidence')
            result = self.run_cli(root, root)
            self.assertEqual(result.returncode, 2, result.stderr)
            self.assertIn('evidence directory must not exist', result.stderr)
            self.assertEqual(original.read_bytes(), b'prior independent evidence')


if __name__ == '__main__':
    unittest.main()
