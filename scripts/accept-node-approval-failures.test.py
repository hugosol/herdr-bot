"""Public acceptance CLI safety; never substitutes for S2 native evidence.

Approved seam: the executable acceptance entry. Ticket 11 requires both hosts
and all four outcomes; a missing dependency must not certify an empty matrix.
"""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest

ROOT = Path(__file__).resolve().parents[1]
ENTRY = ROOT / 'scripts/accept-node-approval-failures.py'


class FailureAcceptanceCLI(unittest.TestCase):
    def invoke(self, root, **overrides):
        values = {'herdr-bot-worktree': str(ROOT), 'toolkit-worktree': str(root / 'toolkit'),
                  'backend': str(root / 'missing/herdr-bot.exe'), 'hosts': 'omp,pi',
                  'paths-file': str(root / 'paths.toml'), 'model-config': str(root / 'model.json'),
                  'evidence-dir': str(root / 'evidence')}
        values.update(overrides)
        return subprocess.run([sys.executable, str(ENTRY), *[part for key, value in values.items()
            for part in ['--' + key, value]]], cwd=ROOT, capture_output=True,
            text=True, encoding='utf-8', errors='replace', timeout=30)

    def test_missing_backend_retains_every_host_outcome_and_independent_expectation(self):
        with tempfile.TemporaryDirectory(prefix='node-approval-cli-') as directory:
            root = Path(directory)
            result = self.invoke(root)
            self.assertNotEqual(result.returncode, 0)
            report = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertFalse(report['passed'])
            self.assertIn('backend', report['error'].lower())
            self.assertEqual(set(report['coverage']), {'omp', 'pi'})
            for cases in report['coverage'].values():
                self.assertEqual(set(cases), {'no', 'cancel', 'timeout', 'unavailable'})
                for case in cases.values():
                    self.assertFalse(case['verified'])
                    self.assertEqual(set(case['promises']), {'P5/S2/non-Yes', 'P7', 'P8', 'P38/native-failures'})
                    self.assertEqual(case['expected']['approved'], False)
                    self.assertEqual(case['expected']['nodeStatus'], 'running')
                    self.assertEqual(case['expected']['automaticConfirmations'], 0)

    def test_invalid_selection_cannot_launch_or_create_evidence(self):
        for selection in ['', 'unknown', 'timeout,timeout', 'all,timeout']:
            with self.subTest(selection=selection), tempfile.TemporaryDirectory(prefix='node-approval-cli-') as directory:
                root = Path(directory)
                result = self.invoke(root, cases=selection)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse((root / 'evidence').exists())

    def test_filtered_run_retains_exact_missing_cases_and_never_passes(self):
        with tempfile.TemporaryDirectory(prefix='node-approval-cli-') as directory:
            root = Path(directory)
            result = self.invoke(root, hosts='pi', cases='timeout')
            self.assertNotEqual(result.returncode, 0)
            report = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertEqual(report['selectedCases'], ['pi/timeout'])
            self.assertEqual(set(report['missingCases']), {'omp/no', 'omp/cancel', 'omp/timeout',
                'omp/unavailable', 'pi/no', 'pi/cancel', 'pi/unavailable'})
            self.assertFalse(report['passed'])
            self.assertFalse(report['selectedCasesPassed'])

    def test_successful_partial_coverage_or_failed_native_exit_cannot_certify_full_matrix(self):
        import importlib.util
        spec = importlib.util.spec_from_file_location('failure_entry', ENTRY)
        entry = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(entry)
        evidence = {'coverage': {host: {case: {'verified': True} for case in entry.CONDITIONS}
            for host in ['omp', 'pi']}, 'nativeExits': {host + '/' + case: 0
            for host in ['omp', 'pi'] for case in entry.CONDITIONS}}
        self.assertTrue(entry.complete_coverage(evidence))
        del evidence['nativeExits']['pi/unavailable']
        self.assertFalse(entry.complete_coverage(evidence))
        evidence['nativeExits']['pi/unavailable'] = 1
        self.assertFalse(entry.complete_coverage(evidence))
        evidence['nativeExits']['pi/unavailable'] = 0
        evidence['coverage']['pi']['unavailable']['verified'] = False
        self.assertFalse(entry.complete_coverage(evidence))

    def test_wrapper_validates_cases_before_creating_transport(self):
        with tempfile.TemporaryDirectory(prefix='node-approval-wrapper-cli-') as directory:
            root = Path(directory)
            base = [sys.executable, str(ROOT / 'scripts/accept-workflow-codex.py'),
                '--scenario', 'node-approval-failures', '--hosts', 'pi', '--native-only']
            for name in ['backend', 'toolkit', 'paths-file', 'evidence-dir', 'pi-package', 'auth-file']:
                base += ['--' + name, str(root / name)]
            for case in ['unknown', 'timeout,timeout', 'timeout', 'unavailable']:
                result = subprocess.run(base + ['--cases', case], capture_output=True,
                    text=True, encoding='utf-8', errors='replace', timeout=30)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn('unique comma-separated' if case in ['unknown', 'timeout,timeout']
                    else 'Missing explicit dependency', result.stderr)
                self.assertFalse((root / 'evidence-dir').exists())

    def test_old_evidence_is_not_overwritten(self):
        with tempfile.TemporaryDirectory(prefix='node-approval-cli-') as directory:
            root = Path(directory)
            evidence = root / 'evidence'
            evidence.mkdir()
            original = evidence / 'evidence.json'
            original.write_bytes(b'ORIGINAL-NATIVE-FAILURE\n')
            result = self.invoke(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(original.read_bytes(), b'ORIGINAL-NATIVE-FAILURE\n')
            self.assertEqual(list(evidence.iterdir()), [original])

    def test_relative_dependency_cannot_reach_preflight(self):
        with tempfile.TemporaryDirectory(prefix='node-approval-cli-') as directory:
            root = Path(directory)
            result = self.invoke(root, backend='relative/herdr-bot.exe')
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('absolute', result.stderr.lower())
            self.assertFalse((root / 'evidence').exists())

    def test_explicit_mechanical_provider_needs_no_model_credentials(self):
        with tempfile.TemporaryDirectory(prefix='node-approval-cli-') as directory:
            root = Path(directory)
            (root / 'model.json').write_text(json.dumps({'mode': 'controlled-native-failures'}), encoding='utf-8')
            (root / 'toolkit').mkdir()
            (root / 'toolkit/.git').write_text('gitdir: owned-test-only', encoding='utf-8')
            result = self.invoke(root, backend=str(Path(sys.executable).resolve()))
            self.assertNotEqual(result.returncode, 0)
            report = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertIn('backend provenance', report['error'])
            self.assertNotIn('credential environment', report['error'])
            self.assertFalse(report['passed'])

    def test_missing_model_authorization_stays_unverified_without_native_launch(self):
        with tempfile.TemporaryDirectory(prefix='node-approval-cli-') as directory:
            root = Path(directory)
            result = self.invoke(root, backend=str(Path(sys.executable).resolve()))
            self.assertNotEqual(result.returncode, 0)
            report = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertIn('authorized', report['error'].lower())
            self.assertEqual(report['nativeExits'], {})
            self.assertFalse(report['passed'])


class ModelIdentityBoundary(unittest.TestCase):
    """Offline wire literals, not native/model acceptance or response replay."""

    def identity(self, records):
        from node_approval_failures_acceptance import ApprovalFailuresAcceptance
        run = ApprovalFailuresAcceptance.__new__(ApprovalFailuresAcceptance)
        run.model_wire = SimpleNamespace(records=records)
        run.expected_model = 'gpt-6-luna'
        run.evidence = {}
        run.model_identity()
        return run.evidence

    def completion(self, **changes):
        record = {'index': 0, 'method': 'POST', 'path': '/chat/completions', 'ended': 1,
                  'status': 200, 'request': json.dumps({'model': 'gpt-6-luna'}),
                  'response': json.dumps({'model': 'gpt-6-luna'})}
        record.update(changes)
        return record

    def test_unrelated_get_does_not_become_model_failure_or_pending_model_work(self):
        evidence = self.identity([self.completion(),
            {'index': 1, 'method': 'GET', 'path': '/?probe=1', 'status': 501},
            {'index': 2, 'method': 'GET', 'path': '/models', 'status': 200}])
        self.assertEqual(evidence['nonCompletionTrafficCount'], 2)
        self.assertEqual(evidence['modelIdentities'], [
            {'request': 0, 'requested': 'gpt-6-luna', 'returned': ['gpt-6-luna']}])

    def test_failed_native_completion_is_terminal_without_another_model_call(self):
        from acceptance.host.completion_transport import NativeCompletionTransport
        import urllib.error
        import urllib.request
        attempted = []

        def failed(request):
            attempted.append(request)
            raise RuntimeError('owned literal failed completion')

        with tempfile.TemporaryDirectory(prefix='approval-terminal-completion-') as directory:
            with NativeCompletionTransport(failed, Path(directory)) as transport:
                for _ in range(2):
                    request = urllib.request.Request(transport.base + '/chat/completions',
                        data=json.dumps({'model': 'gpt-6-luna', 'messages': [], 'stream': False}).encode(),
                        headers={'Content-Type': 'application/json'})
                    with self.assertRaises(urllib.error.HTTPError):
                        urllib.request.urlopen(request, timeout=5)
                self.assertTrue(transport.failed)
                self.assertEqual(len(attempted), 1)

    def test_zero_actual_completions_still_fails_closed(self):
        with self.assertRaisesRegex(AssertionError, 'No actual authorized model request'):
            self.identity([{'method': 'GET', 'path': '/chat/completions', 'status': 200,
                            'ended': 1, 'response': json.dumps({'model': 'gpt-6-luna'})}])

    def test_real_completion_failures_and_missing_identity_are_never_filtered(self):
        for changes in [{'status': 501}, {'error': 'actual transport error'},
                        {'response': '{}'}, {'response': json.dumps({'model': 'other-model'})},
                        {'request': '{}'}]:
            with self.subTest(changes=changes), self.assertRaises(AssertionError):
                self.identity([self.completion(**changes)])


if __name__ == '__main__':
    unittest.main()
