"""Offline process-boundary tests only: no native business/model certification."""
from concurrent.futures import ThreadPoolExecutor
import json
import os
from pathlib import Path
import sys
import tempfile
import time
import unittest

from node_approval_v1_matrix import (cell_root, concurrency, dispatch_cells, run_cell_process,
    scoped_models, selected_cells)

# The process fixture controls only the boundary outcomes. It never runs a
# scenario or writes an aggregate acceptance report.
WORKER = '''
import json, os, sys, time
from pathlib import Path
directory, host, case, mode, control = sys.argv[1:]
control = Path(control)
(control / f'{host}-{case}.pid').write_text(str(os.getpid()), encoding='utf-8')
if mode == 'held':
    until = time.monotonic() + 15
    while not (control / 'release').exists():
        if time.monotonic() >= until:
            sys.exit(17)
        time.sleep(.02)
if mode == 'crash':
    sys.exit(19)
native = Path(directory) / 'native'
run = native / host / 'evidence.json'
run.parent.mkdir(parents=True)
run.write_text('{}', encoding='utf-8')
report = {'managerHost': host, 'case': case, 'cellPassed': False, 'cleanupRisk': False,
          'coverage': {}, 'cleanupErrors': [], 'sharedFailures': [],
          'runs': [{'host': host, 'case': case, 'exit': 1, 'evidence': str(run)}]}
exit_code = 1
if mode == 'auth':
    report['sharedFailures'] = [{'kind': 'auth'}]
if mode == 'cleanup':
    report['cleanupErrors'] = ['owned process exit not confirmed']
if mode in {'partial', 'partial-cleanup'}:
    report['coverage'] = {f'P34/S2/{host}/plan_workflow.json/outline': {'verified': True, 'sourceEvidence': str(run)},
                          f'P34/S3/{host}/plan_workflow.json/outline': {'verified': False}}
    report['sourceFingerprints'] = {'before': {'herdr-bot': 'expected'}, 'after': {'herdr-bot': 'expected'}}
    if mode == 'partial-cleanup':
        report['cleanupErrors'] = ['owned process exit not confirmed']
if mode in {'wrong-keys', 'diagnostic', 'source', 'matching'}:
    exit_code = 0
    report['runs'][0]['exit'] = 0
    report['cellPassed'] = True
    report['coverage'] = {f'P34/S2/{host}/plan_workflow.json/outline': {'verified': True, 'sourceEvidence': str(run)}}
    report['sourceFingerprints'] = {'before': {'herdr-bot': 'expected'}, 'after': {'herdr-bot': 'expected'}}
    if mode == 'wrong-keys':
        report['coverage'] = {'P34/S2/omp/plan_workflow.json/outline': {'verified': True}}
    if mode == 'diagnostic':
        report['passed'] = report.pop('cellPassed')
        report['filtered'] = True
    if mode == 'source':
        report['sourceFingerprints']['before']['herdr-bot'] = 'changed'
(native / 'worker.json').write_text(json.dumps(report), encoding='utf-8')
print(mode)
sys.exit(exit_code)
'''


class Matrix(unittest.TestCase):
    def start_fixture(self, root, mode):
        def run(host, case):
            directory = cell_root(root, host, case)
            directory.mkdir(parents=True)
            seams = ('S2', 'S3') if mode.startswith('partial') else ('S2',)
            return run_cell_process(host, case, directory,
                [sys.executable, '-c', WORKER, str(directory), host, case, mode, str(root)],
                {f'P34/{seam}/{host}/plan_workflow.json/outline' for seam in seams}, cwd=root, env=os.environ,
                source_fingerprints={'herdr-bot': 'expected'})
        return run

    def test_two_held_workers_overlap_in_distinct_processes_and_drain(self):
        # The third cell cannot be dispatched while both authorized slots are occupied.
        cells = [('omp', 'v1'), ('pi', 'v1'), ('omp', 'outer')]
        with tempfile.TemporaryDirectory(prefix='v1-matrix-') as directory, ThreadPoolExecutor(1) as caller:
            root = Path(directory)
            jobs = concurrency({'maxConcurrentRequests': 2}, 2, len(cells))['effectiveJobs']
            future = caller.submit(dispatch_cells, cells, jobs, self.start_fixture(root, 'held'))
            first = root / 'omp-v1.pid'
            second = root / 'pi-v1.pid'
            try:
                deadline = time.monotonic() + 10
                while time.monotonic() < deadline and not all(path.exists() and path.stat().st_size for path in (first, second)):
                    time.sleep(.02)
                self.assertTrue(first.is_file() and second.is_file())
                pids = {int(path.read_text(encoding='utf-8')) for path in (first, second)}
                self.assertEqual(len(pids), 2)
                self.assertNotIn(os.getpid(), pids)
                self.assertFalse((root / 'omp-outer.pid').exists())
            finally:
                (root / 'release').touch()
            results = future.result(timeout=10)
            self.assertEqual([(item['host'], item['case']) for item in results], cells)
            self.assertEqual([item['exit'] for item in results], [1, 1, 1])
            self.assertEqual({results[0]['pid'], results[1]['pid']}, pids)
            self.assertTrue(all(item['state'] == 'failed' and 'haltReason' not in item for item in results))
            self.assertEqual(len({item['cellRoot'] for item in results}), 3)

    def test_shared_failure_cleanup_or_crash_stops_further_launches(self):
        cells = [('pi', 'revision'), ('omp', 'v1')]
        for mode, reason in [('auth', 'shared-failure'), ('cleanup', 'worker-cleanup-risk'),
                             ('crash', 'worker-cleanup-unconfirmed')]:
            with self.subTest(mode=mode), tempfile.TemporaryDirectory(prefix='v1-matrix-') as directory:
                root = Path(directory)
                jobs = concurrency({}, 2, len(cells))['effectiveJobs']
                self.assertEqual(jobs, 1)
                first, next_cell = dispatch_cells(cells, jobs, self.start_fixture(root, mode))
                self.assertEqual(first['haltReason'], reason)
                if mode == 'auth':
                    self.assertEqual(first['sharedFailures'], [{'kind': 'auth'}])
                self.assertEqual(next_cell['state'], 'not-run')
                self.assertFalse((root / 'omp-v1.pid').exists())

    def test_zero_exit_does_not_trust_diagnostic_wrong_keys_or_different_source(self):
        for mode, reason in [('wrong-keys', 'worker-protocol-unconfirmed'),
                             ('diagnostic', 'worker-cleanup-unconfirmed'), ('source', 'shared-failure')]:
            with self.subTest(mode=mode), tempfile.TemporaryDirectory(prefix='v1-matrix-') as directory:
                result, = dispatch_cells([('pi', 'planning')], 1, self.start_fixture(Path(directory), mode))
                self.assertEqual(result['exit'], 0)
                self.assertEqual(result['state'], 'failed')
                self.assertEqual(result['haltReason'], reason)
                self.assertNotIn('coverage', result)

    def test_failed_cell_preserves_independent_coverage_only_with_clean_ownership(self):
        for mode in ('partial', 'partial-cleanup'):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory(prefix='v1-matrix-') as directory:
                result, = dispatch_cells([('pi', 'planning')], 1, self.start_fixture(Path(directory), mode))
                self.assertEqual(result['exit'], 1)
                self.assertEqual(result['state'], 'failed')
                if mode == 'partial':
                    self.assertEqual(set(result['coverage']), {'P34/S2/pi/plan_workflow.json/outline'})
                    self.assertNotIn('haltReason', result)
                else:
                    self.assertNotIn('coverage', result)
                    self.assertEqual(result['haltReason'], 'worker-cleanup-risk')

    def test_matching_protocol_retains_the_actual_cell_evidence_path(self):
        with tempfile.TemporaryDirectory(prefix='v1-matrix-') as directory:
            root = Path(directory)
            result, = dispatch_cells([('pi', 'planning')], 1, self.start_fixture(root, 'matching'))
            self.assertEqual(result['state'], 'passed')
            evidence = root / 'cells/pi/planning/native/pi/evidence.json'
            self.assertEqual(result['coverage'], {'P34/S2/pi/plan_workflow.json/outline':
                {'verified': True, 'sourceEvidence': str(evidence)}})
            self.assertFalse((root / 'evidence.json').exists())  # No aggregate/business report.

    def test_scope_selects_only_exact_unique_loopback_endpoints(self):
        first, second = 'http://127.0.0.1:49222', 'http://127.0.0.1:49223'
        urls = {'omp/revision': first, 'pi/revision': second}
        config = {'provider': 'codex-completion-transport', 'apiKeyEnv': 'WORKFLOW_CODEX_TRANSPORT_KEY',
                  'baseUrl': first, 'cellBaseUrls': urls, 'maxConcurrentRequests': 2}
        cells = selected_cells('omp,pi', 'revision')
        self.assertEqual(cells, [('omp', 'revision'), ('pi', 'revision')])
        scoped = scoped_models(config, cells)
        self.assertEqual([item['baseUrl'] for item in scoped.values()], [first, second])
        self.assertTrue(all('cellBaseUrls' not in item for item in scoped.values()))
        self.assertEqual(config['cellBaseUrls'], urls)
        for endpoint in [first, 'http://localhost:49223', 'http://127.0.0.1:443', second + '/v1']:
            with self.subTest(endpoint=endpoint), self.assertRaises(ValueError):
                scoped_models(dict(config, cellBaseUrls=dict(urls, **{'pi/revision': endpoint})), cells)
        with self.assertRaises(ValueError):
            scoped_models(dict(config, apiKeyEnv='REAL_PROVIDER_KEY'), cells)

    def test_job_limit_never_overrides_explicit_capacity(self):
        self.assertEqual(concurrency({'maxConcurrentRequests': 1}, 2, 10)['effectiveJobs'], 1)
        self.assertEqual(concurrency({'maxConcurrentRequests': 2}, 1, 10)['effectiveJobs'], 1)
        for invalid in [True, 0, '2', None]:
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                concurrency({'maxConcurrentRequests': invalid}, 2, 10)


if __name__ == '__main__':
    unittest.main()
