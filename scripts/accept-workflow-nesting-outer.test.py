"""Public CLI safety checks; these do not establish P45/P46 real-model acceptance.

Approved seams: outer acceptance CLI/evidence, public Chat/Task/Bot/Run HTTP,
native role tools/history, actual output bytes and owned OS processes.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

REPO = Path(__file__).resolve().parent.parent


class OuterAcceptancePrerequisites(unittest.TestCase):
    def test_existing_invocation_evidence_is_never_reused(self):
        # Independent ticket requirement: each invocation owns new evidence.
        # Violation: overwriting an earlier successful result after a failure.
        with tempfile.TemporaryDirectory(prefix='outer-owned-evidence-') as directory:
            root = Path(directory)
            output = root / 'already-owned'
            output.mkdir()
            previous = output / 'evidence.json'
            previous.write_bytes(b'previous invocation must remain intact\n')
            result = subprocess.run([sys.executable, str(REPO / 'scripts/accept-workflow-nesting-outer.py'),
                '--backend', str(root / 'missing.exe'), '--toolkit', str(root / 'missing-toolkit'),
                '--hosts', 'omp,pi', '--paths-file', str(root / 'missing.toml'),
                '--model-config', str(root / 'model.json'), '--evidence-dir', str(output)],
                cwd=REPO, capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=30)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(previous.read_bytes(), b'previous invocation must remain intact\n')
            self.assertEqual(sorted(path.name for path in output.iterdir()), ['evidence.json'])
            self.assertEqual(result.stdout, '')

    def test_missing_worktree_backend_cannot_claim_outer_or_pairing_acceptance(self):
        # Independent source: ticket24 requires a current worktree build, not
        # prepared configuration. Violation: reporting passing coverage without
        # ever launching the declared mixed-host business flow.
        toolkit = Path(os.environ['OUTER_ACCEPTANCE_TOOLKIT']).resolve()
        with tempfile.TemporaryDirectory(prefix='outer-cli-') as directory:
            root = Path(directory)
            output = root / 'evidence'
            result = subprocess.run([sys.executable, str(REPO / 'scripts/accept-workflow-nesting-outer.py'),
                '--backend', str(root / 'missing/debug/herdr-bot.exe'), '--toolkit', str(toolkit),
                '--hosts', 'declared', '--paths-file', str(root / 'paths.toml'),
                '--model-config', str(root / 'model.json'), '--evidence-dir', str(output)],
                cwd=REPO, capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=60)
            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
            summary = json.loads(result.stdout)
            evidence = json.loads(Path(summary['evidence']).read_text(encoding='utf-8'))
            self.assertFalse(evidence['passed'])
            self.assertEqual(set(evidence['coverage']), {'P45/creator', 'P45/optimizer', 'P46/outer', 'P46/legacy', 'P46/scope'})
            self.assertTrue(all(not item['verified'] for item in evidence['coverage'].values()))
            self.assertIn('Missing current worktree backend', evidence['verificationError'])
            self.assertEqual(evidence['processes'], [])
            self.assertEqual(evidence['scenarios'], [])
            self.assertEqual(evidence['cleanupErrors'], [])
            self.assertFalse(any(output.rglob('models.json')))


if __name__ == '__main__':
    unittest.main()
