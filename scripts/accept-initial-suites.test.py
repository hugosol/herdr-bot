"""S3 delivery acceptance: foreign binaries cannot certify native discovery.

Approved seams: S3 CLI/deployment, S2 Task/Chat HTTP and S4 installed hosts
through Herdr (the latter two are exercised by the real runner, not this test).
Oracle: ticket 14 requires the current worktree's delivered backend. A plausible
violation is accepting any executable, then reporting manifest-only success.
Observe nonzero, all four promises unverified, and no started host processes.
"""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent


class DeliveryCli(unittest.TestCase):
    def test_unrelated_executable_cannot_certify_initial_native_suites(self):
        with tempfile.TemporaryDirectory(prefix='initial-suite-cli-') as directory:
            root = Path(directory)
            paths = root / 'dependencies.toml'
            paths.write_text('[roots]\n', encoding='utf-8')
            result = subprocess.run([sys.executable, str(ROOT / 'scripts/accept-initial-suites.py'),
                '--herdr-bot-worktree', str(ROOT), '--toolkit-worktree', str(ROOT),
                '--backend', sys.executable, '--paths-file', str(paths),
                '--herdr-bot-base', 'HEAD', '--toolkit-base', 'HEAD',
                '--evidence-dir', str(root / 'evidence')], capture_output=True, text=True, timeout=30)
            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
            evidence_file = root / 'evidence/evidence.json'
            self.assertTrue(evidence_file.is_file(), result.stdout + result.stderr)
            evidence = json.loads(evidence_file.read_text(encoding='utf-8'))
            self.assertIn('backend provenance', evidence['error'])
            self.assertFalse(evidence['passed'])
            self.assertEqual(evidence['startedProcesses'], [])
            self.assertEqual(set(evidence['coverage']), {
                'P11/exploration-omp', 'P11/exploration-pi', 'P11/implementation-omp',
                'P42', 'P44'})
            self.assertTrue(all(not item['verified'] for item in evidence['coverage'].values()))


if __name__ == '__main__':
    unittest.main()
