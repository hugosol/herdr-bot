"""Executable CLI seam; safety only, never evidence of P31 business success.

Oracle: ticket 20 requires dual-host evidence and exclusive evidence ownership.
A plausible violation exits zero on missing dependencies or overwrites a prior run.
These subprocess observations distinguish both without inventing native decisions.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
TOOLKIT = Path(os.environ['CHIEF_AUTHORITY_TOOLKIT'])


class AuthorityCLI(unittest.TestCase):
    def invoke(self, root, **overrides):
        values = {'herdr-bot-worktree': ROOT, 'toolkit-worktree': TOOLKIT,
                  'backend': root / 'missing.exe', 'hosts': 'omp,pi',
                  'paths-file': root / 'paths.toml', 'model-config': root / 'model.json',
                  'evidence-dir': root / 'evidence', **overrides}
        return subprocess.run([sys.executable, str(ROOT / 'scripts/accept-chief-authority.py'),
            *[part for key, value in values.items() for part in ['--' + key, str(value)]]],
            cwd=ROOT, capture_output=True, text=True, encoding='utf-8', timeout=30)

    def test_missing_backend_retains_each_host_unverified(self):
        with tempfile.TemporaryDirectory(prefix='authority-cli-') as temporary:
            root = Path(temporary)
            result = self.invoke(root)
            self.assertNotEqual(result.returncode, 0)
            report = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertFalse(report['passed'])
            self.assertIn('backend', report['error'].lower())
            self.assertEqual(set(report['coverage']), {'omp', 'pi'})
            for host in report['coverage'].values():
                self.assertEqual(set(host), {'P31'})
                self.assertFalse(host['P31']['verified'])

    def test_wrong_model_keeps_both_hosts_unverified(self):
        with tempfile.TemporaryDirectory(prefix='authority-cli-') as temporary:
            root = Path(temporary)
            backend = root / 'not-executed.exe'
            backend.write_bytes(b'An inert dependency; model selection must fail first')
            (root / 'model.json').write_text(json.dumps({'model': 'not-gpt-6-luna'}), encoding='utf-8')
            result = self.invoke(root, backend=backend)
            self.assertNotEqual(result.returncode, 0)
            report = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertIn('must be gpt-6-luna', report['error'])
            self.assertFalse(report['passed'])
            self.assertTrue(all(not host['P31']['verified'] for host in report['coverage'].values()))
            self.assertEqual(sorted(path.name for path in (root / 'evidence').iterdir()), ['evidence.json'])

    def test_existing_evidence_is_not_overwritten(self):
        with tempfile.TemporaryDirectory(prefix='authority-cli-') as temporary:
            root = Path(temporary)
            (root / 'evidence').mkdir()
            marker = root / 'evidence/evidence.json'
            marker.write_text('owned previous evidence', encoding='utf-8')
            self.assertNotEqual(self.invoke(root).returncode, 0)
            self.assertEqual(marker.read_text(encoding='utf-8'), 'owned previous evidence')

    def test_relative_dependency_is_rejected_before_creating_evidence(self):
        with tempfile.TemporaryDirectory(prefix='authority-cli-') as temporary:
            root = Path(temporary)
            result = self.invoke(root, backend='relative.exe')
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('absolute path', result.stderr)
            self.assertFalse((root / 'evidence').exists())


if __name__ == '__main__':
    unittest.main()
