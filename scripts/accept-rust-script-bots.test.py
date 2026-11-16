"""CLI safety seam only; never evidence of Rust business or H1/H2."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
ENTRY = ROOT / 'scripts/accept-rust-script-bots.py'
TOOLKIT = Path(os.environ['RUST_SCRIPT_TOOLKIT']).resolve()


class AcceptanceSafety(unittest.TestCase):
    def invoke(self, directory, **changes):
        values = {'herdr-bot-worktree': ROOT, 'toolkit-worktree': TOOLKIT,
                  'backend': directory / 'herdr-bot.exe', 'static-dir': ROOT / 'backend/static',
                  'native-entry': TOOLKIT / 'scripts/rust-script-acceptance.ts',
                  'isolation-config': directory / 'isolation.json', 'evidence-dir': directory / 'new'}
        values.update(changes)
        command = [sys.executable, str(ENTRY)]
        for key, value in values.items():
            command += ['--' + key, str(value)]
        return subprocess.run(command, cwd=ROOT, capture_output=True, text=True)

    def test_relative_paths_are_rejected_without_evidence(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            result = self.invoke(root, backend=Path('herdr-bot.exe'))
            self.assertEqual(result.returncode, 2, result.stderr)
            self.assertIn('explicit absolute path', result.stderr)
            self.assertFalse((root / 'new').exists())

    def test_existing_evidence_is_never_overwritten(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'new').mkdir()
            sentinel = root / 'new/evidence.json'
            sentinel.write_bytes(b'ORIGINAL-FAILED-EVIDENCE')
            result = self.invoke(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(sentinel.read_bytes(), b'ORIGINAL-FAILED-EVIDENCE')

    def test_foreign_backend_is_rejected_before_any_native_command(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'herdr-bot.exe').write_bytes(b'NOT-AN-EXECUTABLE')
            (root / 'herdr-bot.d').write_text('C:/foreign/backend/src/main.rs', encoding='utf-8')
            result = self.invoke(root)
            self.assertEqual(result.returncode, 1, result.stderr)
            evidence = json.loads((root / 'new/evidence.json').read_text(encoding='utf-8'))
            self.assertIn('another worktree', evidence['error'])
            self.assertEqual(evidence['commands'], [])
            self.assertFalse(evidence['automatedPassed'])

    def test_stale_backend_is_rejected_before_native_commands(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            binary = root / 'herdr-bot.exe'
            binary.write_bytes(b'OLD-BACKEND')
            (root / 'herdr-bot.d').write_text(str(ROOT / 'backend/src/main.rs'), encoding='utf-8')
            os.utime(binary, (1, 1))
            result = self.invoke(root)
            self.assertEqual(result.returncode, 1, result.stderr)
            evidence = json.loads((root / 'new/evidence.json').read_text(encoding='utf-8'))
            self.assertIn('predates current Rust source', evidence['error'])
            self.assertEqual(evidence['commands'], [])

    def test_missing_backend_is_failure_not_human_pending_success(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            result = self.invoke(root)
            self.assertEqual(result.returncode, 1, result.stderr)
            evidence = json.loads((root / 'new/evidence.json').read_text(encoding='utf-8'))
            self.assertFalse(evidence['automatedPassed'])
            self.assertIn('backend', evidence['error'])
            self.assertEqual(evidence['human']['H1']['status'], 'unverified')
            self.assertEqual(evidence['human']['H2']['status'], 'unverified')


if __name__ == '__main__':
    unittest.main()
