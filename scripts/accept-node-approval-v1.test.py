"""Ticket 14 CLI safety, not native business evidence.

Approved seam: the required executable acceptance entrypoint. Independent ticket
oracle: two V1 hosts plus every non-V2 published node need named S2/S3 evidence.
Plausible violation: missing authorization or partial coverage becomes a pass.
The scenario with no authorized model must preserve the entire unverified matrix,
start no checks/hosts and retain the original error. No doubles are introduced.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
ENTRY = ROOT / 'scripts/accept-node-approval-v1.py'
NODES = {
    'create-bot-wrapper.json': ['create_blank', 'write_when', 'write_plan_guide', 'write_node_guide',
        'optimize_when', 'optimize_plan_guide', 'optimize_node_guide', 'save_bot'],
    'create-bot-task-workflow--bc-61ce48b80a4c.json': ['w1', 'w2', 'wrap_bot', 'optimize_bot'],
    'enhance-existing-bot--bc-301b25d9f8d6.json': ['w1', 'w2', 'w3', 'w4', 'w5'],
    'plan_workflow.json': ['outline', 'generate_map', 'optimize_map'],
    'tibo-monitor-then-c-drive-inspection.json': ['tibo-monitor', 'c-drive-inspection'],
}


class PublishedV1CLI(unittest.TestCase):
    def invoke(self, root, **overrides):
        values = {'herdr-bot-worktree': str(ROOT), 'toolkit-worktree': str(root / 'toolkit'),
            'backend': str(root / 'target/debug/herdr-bot.exe'), 'hosts': 'omp,pi',
            'paths-file': str(root / 'paths.toml'), 'model-config': str(root / 'model.json'),
            'evidence-dir': str(root / 'evidence')}
        values.update(overrides)
        return subprocess.run([sys.executable, str(ENTRY), *[part for key, value in values.items()
            for part in ['--' + key, value]]], cwd=ROOT, capture_output=True,
            text=True, encoding='utf-8', errors='replace', timeout=30)

    def test_missing_authorization_preserves_every_owned_partition(self):
        with tempfile.TemporaryDirectory(prefix='published-v1-cli-') as directory:
            root = Path(directory)
            result = self.invoke(root)
            self.assertNotEqual(result.returncode, 0)
            report = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertFalse(report['passed'])
            self.assertIn('authorized', report['error'].lower())
            expected = {'P31/S2/' + host for host in ['omp', 'pi']} | {'P35/S2/' + host for host in ['omp', 'pi']}
            expected |= {'P34/' + seam + '/' + filename + '/' + node for seam in ['S2', 'S3']
                for filename, nodes in NODES.items() for node in nodes}
            self.assertEqual(set(report['coverage']), expected)
            self.assertTrue(all(not item['verified'] and item['expected'] for item in report['coverage'].values()))
            self.assertEqual(report['commands'], [])
            self.assertEqual(report['runs'], [])

    def test_codex_wrapper_recognizes_v1_without_discovering_auth(self):
        with tempfile.TemporaryDirectory(prefix='published-v1-wrapper-') as directory:
            root = Path(directory)
            result = subprocess.run([sys.executable, str(ROOT / 'scripts/accept-workflow-codex.py'),
                '--scenario', 'node-approval-v1', '--hosts', 'omp,pi', '--backend', str(root / 'missing.exe'),
                '--toolkit', str(root / 'toolkit'), '--paths-file', str(root / 'paths.toml'),
                '--evidence-dir', str(root / 'evidence'), '--pi-package', str(root / 'pi'),
                '--auth-file', str(root / 'explicit-only.json')], cwd=ROOT, capture_output=True,
                text=True, encoding='utf-8', errors='replace', timeout=30)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('Missing explicit dependency', result.stderr)
            self.assertFalse((root / 'evidence').exists())

    def test_existing_evidence_is_not_overwritten(self):
        with tempfile.TemporaryDirectory(prefix='published-v1-cli-') as directory:
            root = Path(directory)
            (root / 'evidence').mkdir()
            original = root / 'evidence/evidence.json'
            original.write_bytes(b'ORIGINAL-NATIVE-FAILURE-1491\n')
            result = self.invoke(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(original.read_bytes(), b'ORIGINAL-NATIVE-FAILURE-1491\n')
            self.assertEqual(list(original.parent.iterdir()), [original])

    def test_relative_input_is_refused_before_creating_evidence(self):
        with tempfile.TemporaryDirectory(prefix='published-v1-cli-') as directory:
            root = Path(directory)
            result = self.invoke(root, backend='relative/herdr-bot.exe')
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('absolute', result.stderr.lower())
            self.assertFalse((root / 'evidence').exists())

    def test_no_model_product_restriction_or_credential_discovery(self):
        with tempfile.TemporaryDirectory(prefix='published-v1-cli-') as directory:
            root = Path(directory)
            variable = 'PUBLISHED_V1_UNAVAILABLE_EXPLICIT_KEY_1491'
            self.assertNotIn(variable, os.environ)
            (root / 'model.json').write_text(json.dumps({'provider': 'authorized-provider',
                'model': 'authorized-business-model', 'baseUrl': 'http://127.0.0.1:9/v1',
                'apiKeyEnv': variable}), encoding='utf-8')
            result = self.invoke(root)
            self.assertNotEqual(result.returncode, 0)
            report = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertIn('Missing explicitly authorized credential environment: ' + variable, report['error'])
            self.assertEqual(report['commands'], [])
            self.assertFalse(report['passed'])


if __name__ == '__main__':
    unittest.main()
