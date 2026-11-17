"""Ticket 10 public CLI safety; not native acceptance evidence.

Approved seam: the supplied acceptance command and its evidence/exit contract.
Independent criterion: absent dependencies cannot pass any S2 coverage; a plausible
violation is treating an empty coverage map or one host as complete acceptance.
"""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
ENTRY = ROOT / 'scripts/accept-node-approval-report.py'


class ApprovalEntryContract(unittest.TestCase):
    def invoke(self, root, **overrides):
        values = {'herdr-bot-worktree': str(ROOT), 'toolkit-worktree': str(root / 'toolkit'),
                  'backend': str(root / 'missing/herdr-bot.exe'), 'hosts': 'omp,pi',
                  'paths-file': str(root / 'paths.toml'), 'model-config': str(root / 'model.json'),
                  'evidence-dir': str(root / 'evidence')}
        values.update(overrides)
        return subprocess.run([sys.executable, str(ENTRY), *[part for name, value in values.items()
            for part in ['--' + name, value]]], cwd=ROOT, capture_output=True, text=True,
            encoding='utf-8', errors='replace', timeout=30)

    def test_missing_dependency_keeps_every_host_and_criterion_unverified(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = self.invoke(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertTrue((root / 'evidence/evidence.json').is_file(), result.stderr)
            report = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertFalse(report['passed'])
            self.assertEqual(set(report['coverage']), {'omp', 'pi'})
            expected = {'P2', 'P4', 'P5', 'P6', 'P9', 'P12', 'P13', 'P17', 'P18', 'P19', 'P23', 'P37', 'P38'}
            for host in report['coverage'].values():
                self.assertEqual(set(host), expected)
                self.assertTrue(all(not item['verified'] and item['expected'] for item in host.values()))
            self.assertIn('Missing explicit paths file', report['error'])
            self.assertEqual(report['commands'], [])

    def test_old_evidence_is_never_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'evidence').mkdir()
            sentinel = root / 'evidence/evidence.json'
            sentinel.write_bytes(b'original invocation evidence\n')
            result = self.invoke(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(sentinel.read_bytes(), b'original invocation evidence\n')

    def test_relative_dependency_is_rejected_before_evidence_or_processes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = self.invoke(root, backend='backend/target/debug/herdr-bot.exe')
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('absolute', result.stderr)
            self.assertFalse((root / 'evidence').exists())


if __name__ == '__main__':
    unittest.main()
