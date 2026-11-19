"""Independent S1/S3 evidence gates, not substitutes for native acceptance."""
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from acceptance.paired_close_reminders import check_close, check_sound


class CompletionEvidence(unittest.TestCase):
    # A host can acknowledge a close and remove the layout while the target
    # process still runs. The public receipt and OS exit are independent.
    def test_receipt_and_layout_do_not_prove_process_exit(self):
        before = {'main': {'node', 'unrelated'}, 'other': {'control'}}
        after = {'main': {'unrelated'}, 'other': {'control'}}
        with self.assertRaisesRegex(AssertionError, 'target process'):
            check_close(before, after, 'main', 'node', False, True, False)
        check_close(before, after, 'main', 'node', True, True, False)

    def test_last_tab_must_remove_its_container(self):
        before = {'main': {'node'}, 'other': {'control'}}
        with self.assertRaisesRegex(AssertionError, 'layout'):
            check_close(before, {'main': set(), 'other': {'control'}}, 'main', 'node', True, True, False)
        check_close(before, {'other': {'control'}}, 'main', 'node', True, True, False)

    def test_refused_group_must_not_change_any_owned_process_or_tab(self):
        before = {'main': {'node'}, 'group-peer': {'unrelated'}}
        with self.assertRaisesRegex(AssertionError, 'unrelated process'):
            check_close(before, before, 'main', 'node', False, False, True)
        check_close(before, before, 'main', 'node', False, True, True)


class SoundEvidence(unittest.TestCase):
    # A single transition must not be counted as evidence for two audio onsets.
    # Missing recording is a distinct error even for a silence control.
    def test_duplicate_or_missing_observation_cannot_pass(self):
        with self.assertRaisesRegex(AssertionError, 'playback'):
            check_sound('done', ['done', 'done'])
        with self.assertRaisesRegex(AssertionError, 'unavailable'):
            check_sound(None, None)
        check_sound('done', ['done'])
        check_sound(None, [])


class EvidenceDirectory(unittest.TestCase):
    # Reusing a previous run could silently merge lost/failed observations
    # into a new green matrix. Existing evidence must be left byte-identical.
    def test_existing_archive_is_not_touched_even_with_requested_cases(self):
        with tempfile.TemporaryDirectory(prefix='paired-close-archive-') as folder:
            evidence = Path(folder)
            summary = evidence / 'summary.json'
            summary.write_bytes(b'original failure')
            result = subprocess.run([sys.executable, str(Path(__file__).with_name('accept-paired-close-reminders.py')),
                '--herdr-bot-worktree', folder, '--toolkit-worktree', folder, '--backend', folder,
                '--isolated-config', folder, '--close-cases', 'all', '--reminder-cases', 'all', '--evidence-dir', folder],
                capture_output=True, timeout=10)
            self.assertEqual(result.returncode, 1)
            self.assertEqual(summary.read_bytes(), b'original failure')
            self.assertEqual(list(evidence.iterdir()), [summary])


if __name__ == '__main__':
    unittest.main()
