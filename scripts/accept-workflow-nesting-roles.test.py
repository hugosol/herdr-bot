"""Approved seam: ticket14 CLI; not a substitute for native real-model acceptance.
Source: missing authorized model must fail nonzero, retain evidence, start no service.
Plausible violation: provision a controlled provider or silently skip a missing model.
"""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


class MissingModel(unittest.TestCase):
    def test_missing_explicit_model_starts_no_services_and_keeps_unverified_hosts(self):
        with tempfile.TemporaryDirectory(prefix='roles-cli-test-') as directory:
            root = Path(directory)
            paths = root / 'paths.toml'
            paths.write_text('[roots]\n', encoding='utf-8')
            result = subprocess.run([sys.executable, str(Path(__file__).with_name('accept-workflow-nesting-roles.py')),
                '--backend', str(root / 'herdr-bot.exe'), '--toolkit', str(root / 'toolkit'),
                '--hosts', 'omp,pi', '--paths-file', str(paths), '--model-config', str(root / 'missing-model.json'),
                '--evidence-dir', str(root / 'evidence')], capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            evidence = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertFalse(evidence['passed'])
            self.assertEqual(evidence['requestedHosts'], ['omp', 'pi'])
            self.assertIn('model', evidence['verificationError'].lower())
            self.assertEqual(evidence['processes'], [])
            self.assertEqual(evidence['verified'], [])

    def test_declared_model_never_falls_back_to_user_authentication(self):
        with tempfile.TemporaryDirectory(prefix='roles-no-credential-') as directory:
            root = Path(directory)
            paths = root / 'paths.toml'
            paths.write_text('[roots]\n', encoding='utf-8')
            model = root / 'model.json'
            model.write_text(json.dumps({'provider': 'explicit-service', 'model': 'real-model',
                'baseUrl': 'https://unused.invalid/v1', 'apiKeyEnv': 'ROLES_TEST_UNSET_CREDENTIAL_79412'}), encoding='utf-8')
            result = subprocess.run([sys.executable, str(Path(__file__).with_name('accept-workflow-nesting-roles.py')),
                '--backend', str(root / 'herdr-bot.exe'), '--toolkit', str(root / 'toolkit'),
                '--hosts', 'declared', '--paths-file', str(paths), '--model-config', str(model),
                '--evidence-dir', str(root / 'evidence')], capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            evidence = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertFalse(evidence['passed'])
            self.assertIn('credential', evidence['verificationError'].lower())
            self.assertEqual(evidence['processes'], [])
            self.assertEqual(evidence['verified'], [])


if __name__ == '__main__':
    unittest.main()
