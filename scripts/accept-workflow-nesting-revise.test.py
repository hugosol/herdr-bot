"""Approved seam: revision acceptance CLI, with actual prerequisites absent.

The ticket requires missing build/model dependencies to fail closed before hosts
start. A plausible violation labels preparation or inherited create evidence as
revision success. Observe exit, revision-only coverage, and no started commands.
These prerequisite tests are not real-model business acceptance.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

REPO = Path(__file__).resolve().parents[1]


class RevisionPrerequisites(unittest.TestCase):
    def invoke(self, root, backend_exists=False):
        target = root / 'target'
        backend = target / 'debug/herdr-bot.exe'
        if backend_exists:
            backend.parent.mkdir(parents=True)
            backend.write_bytes(b'NEVER-EXECUTE-MISSING-MODEL')
        evidence = root / 'evidence'
        command = [sys.executable, str(REPO / 'scripts/accept-workflow-nesting-revise.py'),
            '--backend', str(backend), '--toolkit', os.environ['WORKFLOW_HANDOFF_TOOLKIT'],
            '--hosts', 'declared', '--paths-file', str(root / 'paths.toml'),
            '--model-config', str(root / 'missing-model.json'), '--evidence-dir', str(evidence)]
        result = subprocess.run(command, cwd=REPO, env=dict(os.environ, CARGO_TARGET_DIR=str(target)),
            capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=30)
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue((evidence / 'evidence.json').is_file(), result.stdout + result.stderr)
        observed = json.loads((evidence / 'evidence.json').read_text(encoding='utf-8'))
        self.assertFalse(observed['passed'])
        self.assertEqual(observed['scenario'], 'workflow-nesting-revise')
        self.assertEqual(set(observed['coverage']), {'P40', 'P42', 'P43', 'P44', 'P45'})
        self.assertTrue(all(not item['verified'] for item in observed['coverage'].values()))
        self.assertEqual(observed['taskPairs'], {'task-revise-bot-baseline-v1': {'standalone': False, 'workflow': False}})
        self.assertEqual(observed['commands'], [])
        self.assertEqual(observed['verifiedHosts'], [])
        self.assertEqual(observed['cleanupErrors'], [])
        self.assertFalse((evidence / 'backend-stdout.log').exists())
        return command, observed

    def test_case_selection_rejects_empty_and_unknown_before_starting(self):
        with tempfile.TemporaryDirectory(prefix='revise-select-') as directory:
            root = Path(directory)
            for selection in ['', 'task-no-materials,', 'unknown', 'all,task-no-materials']:
                evidence = root / 'evidence'
                result = subprocess.run([sys.executable, str(REPO / 'scripts/accept-workflow-nesting-revise.py'),
                    '--backend', str(root / 'missing.exe'), '--toolkit', os.environ['WORKFLOW_HANDOFF_TOOLKIT'],
                    '--hosts', 'declared', '--paths-file', str(root / 'paths.toml'),
                    '--model-config', str(root / 'model.json'), '--evidence-dir', str(evidence),
                    '--cases', selection], cwd=REPO, capture_output=True, text=True, timeout=30)
                self.assertEqual(result.returncode, 2)
                self.assertFalse(evidence.exists())

    def test_filtered_case_selection_is_recorded_without_full_coverage(self):
        with tempfile.TemporaryDirectory(prefix='revise-select-evidence-') as directory:
            root = Path(directory)
            evidence = root / 'evidence'
            result = subprocess.run([sys.executable, str(REPO / 'scripts/accept-workflow-nesting-revise.py'),
                '--backend', str(root / 'missing.exe'), '--toolkit', os.environ['WORKFLOW_HANDOFF_TOOLKIT'],
                '--hosts', 'declared', '--paths-file', str(root / 'paths.toml'),
                '--model-config', str(root / 'model.json'), '--evidence-dir', str(evidence),
                '--cases', 'task-no-materials'], cwd=REPO, capture_output=True, timeout=30)
            self.assertNotEqual(result.returncode, 0)
            observed = json.loads((evidence / 'evidence.json').read_text(encoding='utf-8'))
            self.assertEqual(observed['requestedCases'], ['task-no-materials'])
            self.assertTrue(observed['filtered'])
            self.assertFalse(observed['passed'])
            self.assertFalse(observed['subsetPassed'])
            self.assertFalse(observed['verifiedHosts'])
            self.assertTrue(all(not item['verified'] for item in observed['coverage'].values()))

    def test_missing_backend_stays_unverified_and_preserves_existing_evidence(self):
        with tempfile.TemporaryDirectory(prefix='revise-cli-') as directory:
            root = Path(directory)
            command, observed = self.invoke(root)
            self.assertIn('Missing current worktree backend', observed['verificationError'])
            sentinel = root / 'evidence/not-owned.bin'
            sentinel.write_bytes(b'KEEP-EXISTING-REVISION-EVIDENCE\x00\xff')
            again = subprocess.run(command, cwd=REPO, capture_output=True, timeout=30)
            self.assertNotEqual(again.returncode, 0)
            self.assertEqual(sentinel.read_bytes(), b'KEEP-EXISTING-REVISION-EVIDENCE\x00\xff')
            self.assertEqual(json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8')), observed)

    def test_missing_authorized_model_never_executes_backend_sentinel(self):
        with tempfile.TemporaryDirectory(prefix='revise-model-cli-') as directory:
            _, observed = self.invoke(Path(directory), backend_exists=True)
            self.assertIn('Missing explicit authorized model configuration', observed['verificationError'])

    def test_declared_service_without_explicit_credential_never_starts_a_host(self):
        with tempfile.TemporaryDirectory(prefix='revise-authorization-cli-') as directory:
            root = Path(directory)
            (root / 'missing-model.json').write_text(json.dumps({'provider': 'acceptance-only', 'model': 'actual-service',
                'baseUrl': 'https://not-contacted.invalid/v1', 'apiKeyEnv': 'P22_DELIBERATELY_ABSENT_CREDENTIAL'}), encoding='utf-8')
            _, observed = self.invoke(root, backend_exists=True)
            self.assertIn('Missing explicit authorized model credential environment', observed['verificationError'])

    def test_two_requested_hosts_both_remain_unverified_when_backend_is_missing(self):
        with tempfile.TemporaryDirectory(prefix='revise-dual-cli-') as directory:
            root = Path(directory)
            target = root / 'target'
            evidence = root / 'evidence'
            result = subprocess.run([sys.executable, str(REPO / 'scripts/accept-workflow-nesting-revise.py'),
                '--backend', str(target / 'debug/herdr-bot.exe'), '--toolkit', os.environ['WORKFLOW_HANDOFF_TOOLKIT'],
                '--hosts', 'omp,pi', '--paths-file', str(root / 'paths.toml'), '--model-config', str(root / 'model.json'),
                '--evidence-dir', str(evidence)], cwd=REPO, env=dict(os.environ, CARGO_TARGET_DIR=str(target)),
                capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=30)
            self.assertNotEqual(result.returncode, 0)
            observed = json.loads((evidence / 'evidence.json').read_text(encoding='utf-8'))
            self.assertFalse(observed['passed'])
            self.assertEqual(observed['requestedHosts'], ['omp', 'pi'])
            self.assertEqual(observed['verifiedHosts'], [])
            self.assertEqual(set(observed['hosts']), {'omp', 'pi'})
            self.assertTrue(all(not item['verified'] for item in observed['coverage'].values()))
            for host in ['omp', 'pi']:
                self.assertNotEqual(observed['hosts'][host]['exit'], 0)
                detail = json.loads((evidence / host / 'evidence.json').read_text(encoding='utf-8'))
                self.assertFalse(detail['passed'])
                self.assertEqual(detail['commands'], [])
                self.assertEqual(detail['cleanupErrors'], [])
                self.assertIn('Missing current worktree backend', detail['verificationError'])


if __name__ == '__main__':
    unittest.main()
