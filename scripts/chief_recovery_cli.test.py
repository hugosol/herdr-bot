"""Approved external CLI seam: fault only report notifications, preserve argv.

A broad agent-prompt fault would prevent the real sender's initial input and
never reach report recovery. Use the installed test-owned CLI fixture downstream,
not a fake product result; this does not prove native/model recovery acceptance.
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


class RecoveryCLIProxy(unittest.TestCase):
    def test_rejection_does_not_change_or_block_initial_input(self):
        with tempfile.TemporaryDirectory(prefix='recovery-cli-fault-') as temporary:
            root = Path(temporary)
            proxy = root / 'recovery_herdr_proxy.exe'
            fixture = root / 'herdr-fixture.exe'
            shutil.copy2(os.environ['CHIEF_RECOVERY_PROXY'], proxy)
            shutil.copy2(os.environ['HERDR_FIXTURE'], fixture)
            (root / 'recovery-proxy.json').write_text(json.dumps({'herdr': str(fixture)}), encoding='utf-8')
            (root / 'reject-notification').write_text('owned fault', encoding='utf-8')
            initial = 'REPORT-EVIDENCE: private user input\n"quotes" & shell characters are data'
            first = subprocess.run([proxy, 'agent', 'prompt', 'actual-sender', initial], cwd=root,
                capture_output=True, text=True, encoding='utf-8', timeout=10)
            self.assertEqual(first.returncode, 0, first.stderr)
            submitted = (root / 'submitted.jsonl').read_text(encoding='utf-8')
            records = [json.loads(line) for line in submitted.splitlines()]
            self.assertEqual(len(records), 1)
            self.assertEqual(records[0]['text'], initial)
            notification = 'Chief report\nReport: immutable-731\n\n{"summary":"original"}'
            failed = subprocess.run([proxy, 'agent', 'prompt', 'actual-chief', notification], cwd=root,
                capture_output=True, text=True, encoding='utf-8', timeout=10)
            self.assertNotEqual(failed.returncode, 0)
            self.assertEqual(json.loads(failed.stdout)['error']['code'], 'acceptance-notification-rejected')
            self.assertEqual((root / 'submitted.jsonl').read_text(encoding='utf-8'), submitted)
            (root / 'reject-notification').unlink()
            accepted = subprocess.run([proxy, 'agent', 'prompt', 'actual-chief', notification], cwd=root,
                capture_output=True, text=True, encoding='utf-8', timeout=10)
            self.assertEqual(accepted.returncode, 0, accepted.stderr)
            records = [json.loads(line) for line in (root / 'submitted.jsonl').read_text(encoding='utf-8').splitlines()]
            self.assertEqual([record['text'] for record in records], [initial, notification])


if __name__ == '__main__':
    unittest.main()
