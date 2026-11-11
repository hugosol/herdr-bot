"""Public entry CLI safety; these checks never claim P7/P29/P30 success.

Approved seam: the ticket's executable CLI. A missing dependency must preserve
all six host/promise items as unverified and retain a nonzero original failure;
an existing evidence directory must never be overwritten. No native/model
substitute participates in these tests.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
ENTRY = ROOT / 'scripts/accept-chief-entry.py'
TOOLKIT = Path(os.environ['CHIEF_ENTRY_TOOLKIT'])


class EntryContract(unittest.TestCase):
    def command(self, root, evidence, **overrides):
        values = {'herdr-bot-worktree': str(ROOT), 'toolkit-worktree': str(TOOLKIT),
                  'backend': str(root / 'missing/herdr-bot.exe'), 'hosts': 'omp,pi',
                  'paths-file': str(root / 'paths.toml'), 'model-config': str(root / 'model.json'),
                  'evidence-dir': str(evidence)}
        values.update(overrides)
        return subprocess.run([sys.executable, str(ENTRY), *[part for key, value in values.items()
            for part in ['--' + key, value]]], cwd=ROOT, capture_output=True,
            text=True, encoding='utf-8', errors='replace', timeout=30)

    def test_missing_dependency_retains_every_unverified_item(self):
        with tempfile.TemporaryDirectory(prefix='chief-entry-cli-') as directory:
            root = Path(directory)
            result = self.command(root, root / 'evidence')
            self.assertNotEqual(result.returncode, 0)
            report = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertFalse(report['passed'])
            self.assertIn('backend', report['error'].lower())
            self.assertEqual(set(report['coverage']), {'omp', 'pi'})
            for host in ['omp', 'pi']:
                self.assertEqual(set(report['coverage'][host]), {'P7', 'P29', 'P30'})
                self.assertTrue(all(not value['verified'] for value in report['coverage'][host].values()))

    def test_existing_evidence_is_not_reused(self):
        with tempfile.TemporaryDirectory(prefix='chief-entry-cli-') as directory:
            root = Path(directory)
            evidence = root / 'evidence'
            evidence.mkdir()
            previous = evidence / 'evidence.json'
            original = b'ORIGINAL-FAILURE-MUST-SURVIVE-704\n'
            previous.write_bytes(original)
            result = self.command(root, evidence)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(previous.read_bytes(), original)
            self.assertEqual(list(evidence.iterdir()), [previous])

    def test_different_model_is_rejected_before_any_native_or_network_operation(self):
        with tempfile.TemporaryDirectory(prefix='chief-entry-cli-') as directory:
            root = Path(directory)
            (root / 'model.json').write_text(json.dumps({'provider': 'acceptance-only', 'model': 'not-gpt-6-luna',
                'baseUrl': 'http://127.0.0.1:1', 'apiKeyEnv': 'CHIEF_ENTRY_NEVER_READ'}), encoding='utf-8')
            result = self.command(root, root / 'evidence', backend=str(Path(sys.executable).resolve()))
            self.assertNotEqual(result.returncode, 0)
            report = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertIn('model must be gpt-6-luna', report['error'])
            self.assertEqual(report['commands'], [])
            self.assertFalse(report['passed'])

    def test_relative_explicit_dependency_is_refused(self):
        with tempfile.TemporaryDirectory(prefix='chief-entry-cli-') as directory:
            result = self.command(Path(directory), Path(directory) / 'evidence', backend='backend/target/debug/herdr-bot.exe')
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('absolute', result.stderr.lower())


if __name__ == '__main__':
    unittest.main()
