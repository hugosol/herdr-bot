"""S1/S3 entrypoint safety: no host work can be inferred from missing prerequisites.

The ticket requires explicit roots, fresh evidence and four independent primary
cells. A plausible false pass is to accept a filtered invocation or a missing
binary as complete; use the real CLI, no production process or fixture host.
"""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
RUNNER = ROOT / 'scripts/accept-paired-history-recovery.py'


class EntryPoint(unittest.TestCase):
    def setUp(self):
        owned = tempfile.TemporaryDirectory()
        self.addCleanup(owned.cleanup)
        self.owned = Path(owned.name)
        self.evidence = self.owned / 'new-evidence'

    def invoke(self, *extra):
        return subprocess.run([sys.executable, str(RUNNER), '--herdr-bot-worktree', str(ROOT),
            '--toolkit-worktree', str(self.owned / 'missing-toolkit'), '--backend', str(self.owned / 'missing.exe'),
            '--paths-file', str(self.owned / 'missing.toml'), '--evidence-dir', str(self.evidence),
            '--directions', 'tui-to-app', *extra], cwd=ROOT, capture_output=True, text=True, encoding='utf-8')

    def test_unavailable_prerequisites_leave_every_primary_cell_unverified(self):
        child = self.invoke()
        self.assertNotEqual(child.returncode, 0, child.stdout)
        report = json.loads((self.evidence / 'evidence.json').read_text(encoding='utf-8'))
        self.assertFalse(report['passed'])
        self.assertTrue(report['preflightError'])
        self.assertEqual({(cell['agent'], cell['direction']): cell['status'] for cell in report['cells']
            if cell['scenario'] == 'cross-end'}, {('omp', 'tui-to-app'): 'unverified',
            ('pi', 'tui-to-app'): 'unverified', ('omp', 'app-to-tui'): 'unverified',
            ('pi', 'app-to-tui'): 'unverified'})

    def test_existing_evidence_is_not_replaced(self):
        self.evidence.mkdir()
        (self.evidence / 'evidence.json').write_text('prior run', encoding='utf-8')
        child = self.invoke()
        self.assertNotEqual(child.returncode, 0, child.stdout)
        self.assertIn('evidence directory must not exist', child.stderr)
        self.assertEqual((self.evidence / 'evidence.json').read_text(), 'prior run')

    def test_relative_paths_are_rejected_before_creating_evidence(self):
        child = self.invoke('--backend', 'relative.exe')
        self.assertNotEqual(child.returncode, 0, child.stdout)
        self.assertIn('explicit absolute path', child.stderr)
        self.assertFalse(self.evidence.exists())


if __name__ == '__main__':
    unittest.main()
