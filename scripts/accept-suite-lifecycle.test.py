"""S3 acceptance CLI safety, not substitute S4 evidence.

Approved seams: S2 HTTP, S3 deployment/CLI, S4 real Herdr native agents.
Independent oracle: an absent current-worktree backend proves none of the eight
owned promises. Plausible violation: prerequisites are reported as success or
failed coverage vanishes. Observe nonzero exit and explicit unverified coverage.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

REPO = Path(__file__).resolve().parent.parent


class LifecyclePrerequisites(unittest.TestCase):
    def test_missing_backend_cannot_certify_lifecycle(self):
        toolkit = Path(os.environ['LIFECYCLE_TOOLKIT_WORKTREE']).resolve()
        with tempfile.TemporaryDirectory(prefix='lifecycle-cli-') as directory:
            root = Path(directory)
            paths = root / 'isolated-paths.toml'
            paths.write_text('[roots]\n', encoding='utf-8')
            evidence = root / 'evidence'
            result = subprocess.run([sys.executable, str(REPO / 'scripts/accept-suite-lifecycle.py'),
                '--herdr-bot-worktree', str(REPO), '--toolkit-worktree', str(toolkit),
                '--backend', str(root / 'missing.exe'), '--paths-file', str(paths),
                '--evidence-dir', str(evidence)], cwd=REPO, capture_output=True,
                text=True, encoding='utf-8', errors='replace', timeout=30)
            self.assertNotEqual(result.returncode, 0)
            self.assertTrue((evidence / 'evidence.json').is_file(), result.stdout + result.stderr)
            record = json.loads((evidence / 'evidence.json').read_text(encoding='utf-8'))
            self.assertFalse(record['passed'])
            self.assertIn('Missing current worktree backend', record['error'])
            self.assertEqual(set(record['coverage']), {'P12', 'P13', 'P28', 'P29', 'P30', 'P31', 'P32', 'P35'})
            self.assertTrue(all(not item['verified'] for item in record['coverage'].values()))
            self.assertEqual(record['startedProcesses'], [])
            self.assertEqual(record['cleanupErrors'], [])

    def test_existing_evidence_is_never_reused_or_overwritten(self):
        # Independent S3 oracle: a caller-owned existing directory is not ours.
        # A runner that truncates an earlier failure would violate attribution.
        toolkit = Path(os.environ['LIFECYCLE_TOOLKIT_WORKTREE']).resolve()
        with tempfile.TemporaryDirectory(prefix='lifecycle-owned-evidence-') as directory:
            root = Path(directory)
            evidence = root / 'existing'
            evidence.mkdir()
            marker = evidence / 'evidence.json'
            marker.write_text('EARLIER-FAILURE-MUST-SURVIVE-739', encoding='utf-8')
            result = subprocess.run([sys.executable, str(REPO / 'scripts/accept-suite-lifecycle.py'),
                '--herdr-bot-worktree', str(REPO), '--toolkit-worktree', str(toolkit),
                '--backend', str(root / 'missing.exe'), '--paths-file', str(root / 'missing.toml'),
                '--evidence-dir', str(evidence)], cwd=REPO, capture_output=True,
                text=True, encoding='utf-8', errors='replace', timeout=30)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(marker.read_text(encoding='utf-8'), 'EARLIER-FAILURE-MUST-SURVIVE-739')
            self.assertEqual(sorted(p.name for p in evidence.iterdir()), ['evidence.json'])


if __name__ == '__main__':
    unittest.main()
