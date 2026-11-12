"""S2/S3 acceptance CLI safety, not business success.

Independent source: ticket 12 requires both hosts and every owned item to remain
unverified when a real dependency is absent. A plausible violation is reporting
an empty/filtered matrix as passed. Exercise the public executable, not helpers.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
ENTRY = ROOT / 'scripts/accept-chief-task.py'
TOOLKIT = Path(os.environ['CHIEF_TASK_TOOLKIT'])
ITEMS = {'P8/known', 'P8/ambiguous', 'P8/missing', 'P9/corrections',
         'P10/approved', 'P10/denied', 'P10/cancelled', 'P10/timeout',
         'P10/unavailable', 'P10/target-change', 'P10/zone-change', 'P11', 'P13'}


class TaskCLI(unittest.TestCase):
    def command(self, root, evidence, **overrides):
        values = {'herdr-bot-worktree': str(ROOT), 'toolkit-worktree': str(TOOLKIT),
                  'backend': str(root / 'missing/herdr-bot.exe'), 'hosts': 'omp,pi',
                  'paths-file': str(root / 'paths.toml'), 'model-config': str(root / 'model.json'),
                  'evidence-dir': str(evidence)}
        values.update(overrides)
        return subprocess.run([sys.executable, str(ENTRY), *[part for key, value in values.items()
            for part in ['--' + key, value]]], cwd=ROOT, capture_output=True,
            text=True, encoding='utf-8', errors='replace', timeout=30)

    def test_missing_dependency_preserves_complete_unverified_matrix(self):
        with tempfile.TemporaryDirectory(prefix='chief-task-cli-') as directory:
            root = Path(directory)
            result = self.command(root, root / 'evidence')
            self.assertNotEqual(result.returncode, 0)
            report = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertFalse(report['passed'])
            self.assertIn('backend', report['error'].lower())
            self.assertEqual(set(report['coverage']), {'omp', 'pi'})
            for host in ['omp', 'pi']:
                self.assertEqual(set(report['coverage'][host]), ITEMS)
                self.assertTrue(all(not item['verified'] for item in report['coverage'][host].values()))

    def test_existing_evidence_is_never_overwritten(self):
        with tempfile.TemporaryDirectory(prefix='chief-task-cli-') as directory:
            root = Path(directory)
            evidence = root / 'evidence'
            evidence.mkdir()
            previous = evidence / 'evidence.json'
            previous.write_bytes(b'ORIGINAL-FAILURE-731\n')
            result = self.command(root, evidence)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(previous.read_bytes(), b'ORIGINAL-FAILURE-731\n')
            self.assertEqual(list(evidence.iterdir()), [previous])

    def test_relative_paths_are_refused(self):
        with tempfile.TemporaryDirectory(prefix='chief-task-cli-') as directory:
            root = Path(directory)
            result = self.command(root, root / 'evidence', backend='relative/herdr-bot.exe')
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('absolute', result.stderr.lower())
            self.assertFalse((root / 'evidence').exists())

    def test_different_model_never_reaches_native_or_network_operations(self):
        with tempfile.TemporaryDirectory(prefix='chief-task-cli-') as directory:
            root = Path(directory)
            (root / 'model.json').write_text(json.dumps({'model': 'not-gpt-6-luna'}), encoding='utf-8')
            result = self.command(root, root / 'evidence', backend=str(Path(sys.executable).resolve()))
            self.assertNotEqual(result.returncode, 0)
            report = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertIn('model must be gpt-6-luna', report['error'])
            self.assertEqual(report['commands'], [])

    def test_authorized_codex_wrapper_routes_task_scenario_without_credential_discovery(self):
        # The wrapper must accept the scenario but fail on explicit missing files;
        # argparse rejecting the scenario would leave the approved transport unusable.
        with tempfile.TemporaryDirectory(prefix='chief-task-wrapper-') as directory:
            root = Path(directory)
            result = subprocess.run([sys.executable, str(ROOT / 'scripts/accept-workflow-codex.py'),
                '--scenario', 'chief-task', '--hosts', 'omp,pi', '--backend', str(root / 'absent.exe'),
                '--toolkit', str(TOOLKIT), '--paths-file', str(root / 'paths.toml'),
                '--evidence-dir', str(root / 'evidence'), '--pi-package', str(root / 'pi'),
                '--auth-file', str(root / 'authorized-only.json')], cwd=ROOT, capture_output=True,
                text=True, encoding='utf-8', errors='replace', timeout=30)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('Missing explicit dependency', result.stderr)
            self.assertFalse((root / 'evidence').exists())


if __name__ == '__main__':
    unittest.main()
