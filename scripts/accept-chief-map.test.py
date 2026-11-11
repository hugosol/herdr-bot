"""Public CLI safety, not real-model Map/capability acceptance evidence.

Approved seam: the ticket's CLI and its public evidence/exit contract.
Violation: missing dependencies or a single host are reported as full acceptance.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
ENTRY = ROOT / 'scripts/accept-chief-map.py'
TOOLKIT = Path(os.environ['CHIEF_MAP_TOOLKIT'])


class MapEntryContract(unittest.TestCase):
    def invoke(self, root, **overrides):
        values = {'herdr-bot-worktree': str(ROOT), 'toolkit-worktree': str(TOOLKIT),
                  'backend': str(root / 'missing/herdr-bot.exe'), 'hosts': 'omp,pi',
                  'paths-file': str(root / 'paths.toml'), 'model-config': str(root / 'model.json'),
                  'evidence-dir': str(root / 'evidence')}
        values.update(overrides)
        return subprocess.run([sys.executable, str(ENTRY), *[part for name, value in values.items()
            for part in ['--' + name, value]]], cwd=ROOT, capture_output=True, text=True,
            encoding='utf-8', errors='replace', timeout=30)

    def test_missing_dependency_never_claims_owned_coverage(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = self.invoke(root)
            self.assertNotEqual(result.returncode, 0)
            report = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertFalse(report['passed'])
            self.assertEqual(set(report['coverage']), {'omp', 'pi'})
            for host in report['coverage'].values():
                self.assertEqual(set(host), {'P9', 'P10', 'P12', 'P14', 'P15', 'P16'})
                self.assertTrue(all(not item['verified'] for item in host.values()))
            self.assertEqual(report['commands'], [])
            self.assertIn('Missing current worktree backend', report['error'])

    def test_old_evidence_is_untouched(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            evidence = root / 'evidence'
            evidence.mkdir()
            sentinel = evidence / 'evidence.json'
            sentinel.write_bytes(b'original evidence\n')
            result = self.invoke(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(sentinel.read_bytes(), b'original evidence\n')

    def test_relative_backend_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            result = self.invoke(root, backend='backend/target/debug/herdr-bot.exe')
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('absolute', result.stderr)
            self.assertFalse((root / 'evidence').exists())

    def test_other_model_is_rejected_before_native_or_network_use(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'model.json').write_text(json.dumps({'model': 'not-gpt-6-luna'}), encoding='utf-8')
            result = self.invoke(root, backend=str(Path(sys.executable).resolve()))
            self.assertNotEqual(result.returncode, 0)
            report = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertIn('model must be gpt-6-luna', report['error'])
            self.assertEqual(report['commands'], [])


if __name__ == '__main__':
    unittest.main()
