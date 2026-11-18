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
            expected |= {'P34/' + seam + '/' + host + '/' + filename + '/' + node for seam in ['S2', 'S3']
                for host in ['omp', 'pi'] for filename, nodes in NODES.items() for node in nodes}
            self.assertEqual(set(report['coverage']), expected)
            self.assertEqual([(item['host'], item['case']) for item in report['cells']],
                [(host, case) for host in ('omp', 'pi')
                 for case in ('v1', 'outer', 'revision', 'planning', 'investigation')])
            self.assertTrue(all(item['state'] == 'not-run' for item in report['cells']))
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
                '--auth-file', str(root / 'explicit-only.json'), '--cases', 'revision'], cwd=ROOT, capture_output=True,
                text=True, encoding='utf-8', errors='replace', timeout=30)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('Missing explicit dependency', result.stderr)
            self.assertFalse((root / 'evidence').exists())

    def test_named_case_keeps_full_unverified_coverage_and_partial_status(self):
        # Independent ticket oracle: a diagnostic selection must never erase the
        # other named responsibilities or certify the complete ticket.
        with tempfile.TemporaryDirectory(prefix='published-v1-cli-') as directory:
            root = Path(directory)
            result = self.invoke(root, cases='revision', hosts='omp')
            self.assertNotEqual(result.returncode, 0)
            report = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertEqual(report['caseSelection'], 'revision')
            self.assertTrue(report['filtered'])
            self.assertFalse(report['passed'])
            self.assertEqual(len(report['coverage']), 92)
            self.assertEqual([(item['host'], item['case']) for item in report['cells']], [('omp', 'revision')])
            self.assertEqual(report['cells'][0]['state'], 'not-run')
            self.assertTrue(all(not item['verified'] for item in report['coverage'].values()))
            self.assertEqual(report['commands'], [])
            self.assertEqual(report['runs'], [])

    def test_jobs_outside_one_or_two_are_rejected_before_evidence(self):
        with tempfile.TemporaryDirectory(prefix='published-v1-cli-') as directory:
            root = Path(directory)
            result = self.invoke(root, jobs='3')
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('--jobs', result.stderr)
            self.assertFalse((root / 'evidence').exists())

    def test_worker_requires_exactly_one_manager_and_case(self):
        with tempfile.TemporaryDirectory(prefix='published-v1-cli-') as directory:
            root = Path(directory)
            for hosts, cases in [('omp,pi', 'v1'), ('pi', 'all')]:
                with self.subTest(hosts=hosts, cases=cases):
                    result = self.invoke(root, **{'_matrix-worker-root': str(root / 'matrix'), 'hosts': hosts, 'cases': cases})
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn('exactly one host and one named case', result.stderr)
                    self.assertFalse((root / 'evidence').exists())

    def test_real_worker_reports_only_its_keys_without_running_global_checks(self):
        with tempfile.TemporaryDirectory(prefix='published-v1-worker-') as directory:
            root = Path(directory)
            cell = root / 'cells/pi/investigation'
            cell.mkdir(parents=True)
            # Wrong Bun fails real scenario preflight before any native or model operation.
            (root / 'matrix-preflight.json').write_text(json.dumps({'paths': {'roots': {
                'herdr': str(root), 'pi_package': str(root), 'omp_package': str(root)}},
                'hostVersions': {'bun': 'unavailable'}}), encoding='utf-8')
            result = self.invoke(root, **{'_matrix-worker-root': str(root), 'hosts': 'pi',
                'cases': 'investigation', 'model-config': str(cell / 'model.json'),
                'evidence-dir': str(cell / 'native')})
            self.assertEqual(result.returncode, 1)
            report = json.loads((cell / 'native/worker.json').read_text(encoding='utf-8'))
            self.assertEqual((report['managerHost'], report['case']), ('pi', 'investigation'))
            self.assertFalse(report['cellPassed'])
            self.assertIn('Bun 1.4.4', report['error'])
            self.assertEqual(report['runs'], [])
            self.assertEqual(set(report['coverage']), {f'P34/{seam}/pi/tibo-monitor-then-c-drive-inspection.json/{node}'
                for seam in ('S2', 'S3') for node in ('tibo-monitor', 'c-drive-inspection')})
            self.assertFalse((cell / 'native/evidence.json').exists())

    def test_scoped_endpoint_does_not_fall_back_to_another_cell(self):
        with tempfile.TemporaryDirectory(prefix='published-v1-cli-') as directory:
            root = Path(directory)
            url = 'http://127.0.0.1:49222'
            (root / 'model.json').write_text(json.dumps({'provider': 'codex-completion-transport',
                'model': 'gpt-6-luna', 'apiKeyEnv': 'WORKFLOW_CODEX_TRANSPORT_KEY', 'baseUrl': url,
                'cellBaseUrls': {'omp/revision': url}, 'maxConcurrentRequests': 2}), encoding='utf-8')
            result = self.invoke(root, cases='revision', hosts='pi', jobs='2')
            self.assertNotEqual(result.returncode, 0)
            report = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertIn('Missing exact scoped model endpoint for pi/revision', report['error'])
            self.assertEqual(report['matrix'], {'requestedJobs': 2, 'modelCapacity': 2, 'effectiveJobs': 1})
            self.assertEqual(report['commands'], [])
            self.assertEqual(report['cells'], [{'host': 'pi', 'case': 'revision', 'state': 'not-run'}])
            self.assertFalse((root / 'evidence/cells').exists())

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
