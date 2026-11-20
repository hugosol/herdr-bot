"""Exercise the opt-in gate at the external CLI process boundary, not a mock backend."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

from acceptance.host.processes import until


class TerminalBoundary(unittest.TestCase):
    def test_capture_before_forward_and_refuse_without_any_child_effect(self):
        recorder = Path(os.environ['PAIRED_NATIVE_RECEIPT'])
        with tempfile.TemporaryDirectory(prefix='hb14-gate-') as work:
            root = Path(work)
            exe = root / recorder.name
            shutil.copy2(recorder, exe)
            (root / 'installed.json').write_text(json.dumps(sys.executable), encoding='utf-8')
            (root / 'gate').write_text(json.dumps([['-c']]), encoding='utf-8')
            effect = root / 'child-effect'
            arguments = ['-c', f'from pathlib import Path; Path({str(effect)!r}).touch()']

            # A recorder that logs an invocation but forwards immediately cannot
            # tell whether saved associations preceded the real terminal write.
            for reject in (True, False):
                known = set(root.glob('*.request'))
                with subprocess.Popen([str(exe), *arguments], stdout=subprocess.PIPE, stderr=subprocess.PIPE) as child:
                    request = until('external request before child forwarding',
                                    lambda: next(iter(set(root.glob('*.request')) - known), None), seconds=5)
                    self.assertEqual(json.loads(request.read_text(encoding='utf-8')), arguments)
                    self.assertFalse(effect.exists(), 'the real child ran before the decision')
                    self.assertIsNone(child.poll(), 'the recorder did not wait for the decision')
                    if reject:
                        probe = subprocess.run([str(exe), '--version'], capture_output=True, timeout=3, check=True)
                        self.assertTrue(probe.stdout.startswith(b'Python '))
                        self.assertIsNone(child.poll(), 'independent observation released the held mutation')
                    decision = request.with_suffix('.decision')
                    pending = decision.with_suffix('.pending')
                    pending.write_text(json.dumps({'exit': 19, 'stderr': 'HB14 external refusal'}) if reject else 'null', encoding='utf-8')
                    pending.replace(decision)
                    stdout, stderr = child.communicate(timeout=5)
                receipt = json.loads(request.with_suffix('.json').read_text(encoding='utf-8'))
                self.assertEqual(receipt['exit'], 19 if reject else 0)
                self.assertEqual(receipt['forwarded'], not reject)
                self.assertEqual((stdout, stderr), (b'', b'HB14 external refusal' if reject else b''))
                self.assertEqual(effect.exists(), not reject)


class EvidenceBoundary(unittest.TestCase):
    def test_existing_evidence_cannot_be_reused(self):
        with tempfile.TemporaryDirectory(prefix='hb14-existing-') as work:
            evidence = Path(work) / 'summary.json'
            evidence.write_bytes(b'original failure')
            result = subprocess.run([sys.executable, str(Path(__file__).with_name('accept-paired-chief-run.py')),
                '--herdr-bot-worktree', work, '--toolkit-worktree', work, '--backend', work,
                '--isolated-config', work, '--scenarios', 'all', '--evidence-dir', work], capture_output=True, timeout=10)
            self.assertEqual(result.returncode, 1)
            self.assertEqual(evidence.read_bytes(), b'original failure')
            self.assertEqual(list(Path(work).iterdir()), [evidence])


if __name__ == '__main__':
    unittest.main()
