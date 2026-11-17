"""Approved seam: the executable V2 acceptance command, not private modules.

Independent ticket oracle: missing authorization cannot certify either host or
P34/S2, P34/S3, P35/S2. A plausible violation is an empty/filtered matrix marked
passed. These checks do not claim real-model, native Yes or browser acceptance.
"""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
ENTRY = ROOT / 'scripts/accept-node-approval-v2.py'
ITEMS = {'P34/V2/S2', 'P34/V2/S3', 'P35/V2/S2'}


class PublishedV2CLI(unittest.TestCase):
    def invoke(self, root, **overrides):
        values = {'herdr-bot-worktree': str(ROOT), 'toolkit-worktree': str(root / 'toolkit'),
            'backend': str(root / 'herdr-bot.exe'), 'hosts': 'omp,pi',
            'paths-file': str(root / 'paths.toml'), 'model-config': str(root / 'model.json'),
            'evidence-dir': str(root / 'evidence')}
        values.update(overrides)
        return subprocess.run([sys.executable, str(ENTRY), *[part for key, value in values.items()
            for part in (['--' + key] if value is None else ['--' + key, value])]], cwd=ROOT, capture_output=True,
            text=True, encoding='utf-8', errors='replace', timeout=30)

    def test_missing_authorization_keeps_both_hosts_and_every_partition_unverified(self):
        with tempfile.TemporaryDirectory(prefix='v2-cli-') as directory:
            root = Path(directory)
            result = self.invoke(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertTrue((root / 'evidence/evidence.json').is_file(), result.stderr)
            report = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertFalse(report['passed'])
            self.assertIn('authorized', report['error'].lower())
            self.assertEqual(set(report['coverage']), {'omp', 'pi'})
            for host in report['coverage'].values():
                self.assertEqual(set(host), ITEMS)
                for item in host.values():
                    self.assertFalse(item['verified'])
                    self.assertTrue(item['expected'])
            self.assertEqual(report['commands'], [])
            self.assertEqual(report['cleanupErrors'], [])

    def test_native_only_never_certifies_missing_authorization_or_repository_checks(self):
        with tempfile.TemporaryDirectory(prefix='v2-native-only-') as directory:
            root = Path(directory)
            result = self.invoke(root, **{'native-only': None})
            self.assertNotEqual(result.returncode, 0)
            self.assertTrue((root / 'evidence/evidence.json').is_file(), result.stderr)
            report = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertIn('authorized', report['error'])
            self.assertTrue(report['nativeOnly'])
            self.assertFalse(report['nativeCoveragePassed'])
            self.assertFalse(report['passed'])
            self.assertEqual(report['commands'], [])

    def test_authorized_transport_recognizes_v2_before_dependency_refusal(self):
        with tempfile.TemporaryDirectory(prefix='v2-transport-cli-') as directory:
            root = Path(directory)
            result = subprocess.run([sys.executable, str(ROOT / 'scripts/accept-workflow-codex.py'),
                '--scenario', 'node-approval-v2', '--hosts', 'omp,pi', '--backend', str(root / 'absent.exe'),
                '--toolkit', str(root / 'toolkit'), '--paths-file', str(root / 'paths.toml'),
                '--evidence-dir', str(root / 'evidence'), '--pi-package', str(root / 'pi'),
                '--auth-file', str(root / 'explicit-authorized-only.json')], cwd=ROOT,
                capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=30)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('Missing explicit dependency', result.stderr)
            self.assertFalse((root / 'evidence').exists())

    def test_existing_evidence_preserves_original_failure_bytes(self):
        with tempfile.TemporaryDirectory(prefix='v2-cli-') as directory:
            root = Path(directory)
            evidence = root / 'evidence'
            evidence.mkdir()
            previous = evidence / 'evidence.json'
            previous.write_bytes(b'ORIGINAL-V2-FAILURE-815\r\n')
            result = self.invoke(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(previous.read_bytes(), b'ORIGINAL-V2-FAILURE-815\r\n')
            self.assertEqual(list(evidence.iterdir()), [previous])

    def test_relative_dependency_path_is_refused_without_creating_evidence(self):
        with tempfile.TemporaryDirectory(prefix='v2-cli-') as directory:
            root = Path(directory)
            result = self.invoke(root, backend='relative/herdr-bot.exe')
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('absolute', result.stderr.lower())
            self.assertFalse((root / 'evidence').exists())


if __name__ == '__main__':
    unittest.main()
