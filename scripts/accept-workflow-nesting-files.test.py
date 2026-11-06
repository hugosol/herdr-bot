"""Public CLI seam: prerequisites cannot turn into native consumption evidence.

Source: nesting-files acceptance requires real files/hosts/model and nonzero on
missing dependencies. Violation: reporting a prepared scenario as verified.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

REPO = Path(__file__).resolve().parents[1]


class NestingFilesPrerequisites(unittest.TestCase):
    def test_missing_backend_keeps_all_promises_unverified_and_starts_no_service(self):
        toolkit = Path(os.environ['WORKFLOW_HANDOFF_TOOLKIT']).resolve()
        with tempfile.TemporaryDirectory(prefix='nesting-files-cli-') as directory:
            root = Path(directory)
            paths = root / 'paths.toml'
            paths.write_text('[roots]\n', encoding='utf-8')
            target = root / 'target'
            evidence = root / 'evidence'
            result = subprocess.run([sys.executable, str(REPO / 'scripts/accept-workflow-nesting-files.py'),
                '--backend', str(target / 'debug/herdr-bot.exe'), '--toolkit', str(toolkit),
                '--hosts', 'declared', '--paths-file', str(paths), '--model-config', str(root / 'model.json'),
                '--evidence-dir', str(evidence)], cwd=REPO, env=dict(os.environ, CARGO_TARGET_DIR=str(target)),
                capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=30)
            self.assertNotEqual(result.returncode, 0)
            self.assertTrue((evidence / 'evidence.json').is_file(), result.stdout + result.stderr)
            observed = json.loads((evidence / 'evidence.json').read_text(encoding='utf-8'))
            self.assertFalse(observed['passed'])
            self.assertIn('Missing current worktree backend', observed['verificationError'])
            self.assertEqual(observed['verifiedHosts'], [])
            self.assertTrue(all(not item['verified'] for item in observed['coverage'].values()))
            self.assertEqual(observed['commands'], [])
            self.assertEqual(observed['cleanupErrors'], [])
            self.assertFalse((evidence / 'backend-stdout.log').exists())
            self.assertFalse((evidence / 'agent/models.json').exists())


if __name__ == '__main__':
    unittest.main()
