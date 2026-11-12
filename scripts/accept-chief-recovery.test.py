"""Approved executable seam: fail closed without erasing partial evidence.

P23–P35 require actual native runs; these checks do not establish business
acceptance. Independent oracle: a missing backend cannot verify any promise,
and existing evidence must remain byte-for-byte untouched.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
TOOLKIT = Path(os.environ['CHIEF_RECOVERY_TOOLKIT'])
PROMISES = {'P23', 'P24', 'P25', 'P26', 'P27', 'P34', 'P35'}


class RecoveryCLI(unittest.TestCase):
    def invoke(self, root, **overrides):
        values = {'herdr-bot-worktree': str(ROOT), 'toolkit-worktree': str(TOOLKIT),
                  'backend': str(root / 'missing.exe'), 'hosts': 'omp,pi',
                  'paths-file': str(root / 'paths.toml'), 'model-config': str(root / 'model.json'),
                  'evidence-dir': str(root / 'evidence'), **overrides}
        return subprocess.run([sys.executable, str(ROOT / 'scripts/accept-chief-recovery.py'),
            *[part for key, value in values.items() for part in ['--' + key, value]]],
            cwd=ROOT, capture_output=True, text=True, timeout=30)

    def test_missing_backend_preserves_every_unverified_criterion(self):
        with tempfile.TemporaryDirectory(prefix='chief-recovery-cli-') as temporary:
            root = Path(temporary)
            result = self.invoke(root)
            self.assertNotEqual(result.returncode, 0)
            report = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertFalse(report['passed'])
            self.assertIn('backend', report['error'].lower())
            self.assertEqual(set(report['coverage']), {'omp', 'pi'})
            for host in report['coverage'].values():
                self.assertEqual(set(host), PROMISES)
                self.assertTrue(all(not item['verified'] for item in host.values()))

    def test_wrong_model_does_not_fall_back_or_start_hosts(self):
        with tempfile.TemporaryDirectory(prefix='chief-recovery-model-') as temporary:
            root = Path(temporary)
            (root / 'candidate.exe').write_bytes(b'not-executed')
            (root / 'model.json').write_text(json.dumps({'model': 'different-model'}), encoding='utf-8')
            result = self.invoke(root, backend=str(root / 'candidate.exe'))
            self.assertNotEqual(result.returncode, 0)
            report = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertIn('gpt-6-luna', report['error'])
            self.assertEqual(report['commands'], [])
            self.assertFalse(report['passed'])
            self.assertTrue(all(not item['verified'] for host in report['coverage'].values() for item in host.values()))

    def test_relative_paths_are_rejected_before_creating_evidence(self):
        with tempfile.TemporaryDirectory(prefix='chief-recovery-cli-') as temporary:
            root = Path(temporary)
            result = self.invoke(root, backend='relative.exe')
            self.assertEqual(result.returncode, 2)
            self.assertIn('absolute', result.stderr)
            self.assertFalse((root / 'evidence').exists())

    def test_existing_evidence_is_not_overwritten(self):
        with tempfile.TemporaryDirectory(prefix='chief-recovery-cli-') as temporary:
            root = Path(temporary)
            (root / 'evidence').mkdir()
            original = b'original failed run\n'
            (root / 'evidence/evidence.json').write_bytes(original)
            self.assertNotEqual(self.invoke(root).returncode, 0)
            self.assertEqual((root / 'evidence/evidence.json').read_bytes(), original)


if __name__ == '__main__':
    unittest.main()
