"""Public acceptance CLI safety; never evidence of real-model compliance.

Approved seam: the ticket's executable acceptance entrypoint. Independent source:
P20 requires OMP/Pi × auto/manual; absent authorization must leave every item
unverified and exit nonzero. Violation: a filtered/empty matrix reports success.
No provider or tool double is introduced.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
ENTRY = ROOT / 'scripts/accept-node-approval-handoff.py'
TOOLKIT = Path(os.environ['NODE_APPROVAL_HANDOFF_TOOLKIT']).resolve()
ITEMS = {'P20/order', 'P20/progress-before-approval', 'P20/approval-source', 'P30/end-node-finish'}


class HandoffCLI(unittest.TestCase):
    def invoke(self, root, flags=(), **overrides):
        values = {'herdr-bot-worktree': str(ROOT), 'toolkit-worktree': str(TOOLKIT),
                  'backend': str(root / 'missing/herdr-bot.exe'), 'hosts': 'omp,pi',
                  'paths-file': str(root / 'paths.toml'), 'model-config': str(root / 'model.json'),
                  'evidence-dir': str(root / 'evidence')}
        values.update(overrides)
        return subprocess.run([sys.executable, str(ENTRY), *[part for key, value in values.items()
            for part in ['--' + key, value]], *flags], cwd=ROOT, capture_output=True,
            text=True, encoding='utf-8', errors='replace', timeout=30)

    def test_missing_authorization_preserves_all_four_unverified_cells(self):
        with tempfile.TemporaryDirectory(prefix='handoff-cli-') as directory:
            root = Path(directory)
            result = self.invoke(root)
            self.assertNotEqual(result.returncode, 0)
            report = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertFalse(report['passed'])
            self.assertIn('authorized', report['error'].lower())
            self.assertEqual(set(report['coverage']), {'omp', 'pi'})
            for host in ['omp', 'pi']:
                self.assertEqual(set(report['coverage'][host]), {'auto', 'manual'})
                for mode in ['auto', 'manual']:
                    self.assertEqual(set(report['coverage'][host][mode]), ITEMS)
                    self.assertTrue(all(not item['verified'] for item in report['coverage'][host][mode].values()))
            self.assertEqual(report['commands'], [])

    def test_native_only_keeps_missing_authorization_unverified(self):
        with tempfile.TemporaryDirectory(prefix='handoff-native-cli-') as directory:
            root = Path(directory)
            result = self.invoke(root, flags=['--native-only'])
            self.assertNotEqual(result.returncode, 0)
            report = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertTrue(report['diagnosticNativeOnly'])
            self.assertFalse(report['passed'])
            self.assertEqual(report['commands'], [])

    def test_existing_evidence_is_never_overwritten(self):
        with tempfile.TemporaryDirectory(prefix='handoff-cli-') as directory:
            root = Path(directory)
            evidence = root / 'evidence'
            evidence.mkdir()
            previous = evidence / 'evidence.json'
            previous.write_bytes(b'ORIGINAL-FAILURE-731\n')
            result = self.invoke(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(previous.read_bytes(), b'ORIGINAL-FAILURE-731\n')
            self.assertEqual(list(evidence.iterdir()), [previous])

    def test_relative_paths_are_refused_before_creating_evidence(self):
        with tempfile.TemporaryDirectory(prefix='handoff-cli-') as directory:
            root = Path(directory)
            result = self.invoke(root, backend='relative/herdr-bot.exe')
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('absolute', result.stderr.lower())
            self.assertFalse((root / 'evidence').exists())

    def test_different_model_is_not_rejected_as_a_product_restriction(self):
        with tempfile.TemporaryDirectory(prefix='handoff-cli-') as directory:
            root = Path(directory)
            variable = 'HANDOFF_EXPLICITLY_UNAVAILABLE_CREDENTIAL_731'
            self.assertNotIn(variable, os.environ)
            (root / 'model.json').write_text(json.dumps({'provider': 'explicit-provider',
                'model': 'independently-authorized-model', 'baseUrl': 'http://127.0.0.1:9/v1',
                'apiKeyEnv': variable}), encoding='utf-8')
            result = self.invoke(root)
            self.assertNotEqual(result.returncode, 0)
            report = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertIn('Missing explicitly authorized credential environment: ' + variable, report['error'])
            self.assertEqual(report['commands'], [])
            self.assertFalse(report['passed'])

    def test_explicit_codex_transport_routes_handoff_without_auth_discovery(self):
        # Public wrapper seam: a recognized scenario reaches explicit dependency
        # refusal. It must not discover/read credentials or start a host here.
        with tempfile.TemporaryDirectory(prefix='handoff-transport-cli-') as directory:
            root = Path(directory)
            result = subprocess.run([sys.executable, str(ROOT / 'scripts/accept-workflow-codex.py'),
                '--scenario', 'node-approval-handoff', '--hosts', 'omp,pi',
                '--backend', str(root / 'absent.exe'), '--toolkit', str(TOOLKIT),
                '--paths-file', str(root / 'paths.toml'), '--evidence-dir', str(root / 'evidence'),
                '--pi-package', str(root / 'pi'), '--auth-file', str(root / 'explicit-only.json')],
                cwd=ROOT, capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=30)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('Missing explicit dependency', result.stderr)
            self.assertFalse((root / 'evidence').exists())


class ModelIdentityBoundary(unittest.TestCase):
    """Offline identity classification, not real-model business evidence."""

    def identity(self, records):
        from types import SimpleNamespace
        from node_approval_handoff_acceptance import NodeApprovalHandoff
        with tempfile.TemporaryDirectory(prefix='handoff-identity-') as directory:
            config = Path(directory) / 'model.json'
            config.write_text(json.dumps({'model': 'gpt-6-luna'}), encoding='utf-8')
            run = NodeApprovalHandoff.__new__(NodeApprovalHandoff)
            run.args = SimpleNamespace(model_config=config)
            run.model_wire = SimpleNamespace(records=records)
            run.evidence = {}
            run.check_model_identity()
            return run.evidence

    def completion(self, **changes):
        record = {'index': 0, 'method': 'POST', 'path': '/chat/completions', 'ended': 1,
                  'status': 200, 'request': json.dumps({'model': 'gpt-6-luna'}),
                  'response': json.dumps({'model': 'gpt-6-luna'})}
        record.update(changes)
        return record

    def test_unrelated_get_is_not_a_completion_or_pending_model_work(self):
        evidence = self.identity([self.completion(),
            {'index': 1, 'method': 'GET', 'path': '/?probe=1', 'status': 501}])
        self.assertEqual(evidence['nonCompletionTrafficCount'], 1)
        self.assertEqual(evidence['modelIdentities'], [
            {'request': 0, 'requested': 'gpt-6-luna', 'returned': ['gpt-6-luna']}])

    def test_inflight_completion_is_observed_without_resending(self):
        from threading import Timer
        record = self.completion(ended=None)
        completion = Timer(0.02, lambda: record.update(ended=1))
        completion.start()
        try:
            evidence = self.identity([record])
            self.assertEqual(len(evidence['modelIdentities']), 1)
        finally:
            completion.join()

    def test_zero_completions_fails_closed(self):
        with self.assertRaisesRegex(AssertionError, 'No actual model completion traffic'):
            self.identity([{'method': 'GET', 'path': '/chat/completions', 'ended': 1}])

    def test_actual_completion_failures_or_wrong_identity_are_not_filtered(self):
        for changes in [{'status': 501}, {'error': 'actual transport error'},
                        {'response': '{}'}, {'response': json.dumps({'model': 'other-model'})},
                        {'request': '{}'}]:
            with self.subTest(changes=changes), self.assertRaises(AssertionError):
                self.identity([self.completion(**changes)])


if __name__ == '__main__':
    unittest.main()
