"""Owned evidence trees retain native history bytes beyond Windows MAX_PATH."""
import os
from pathlib import Path
import runpy
import shutil
import tempfile
import unittest

runtime = runpy.run_path(str(Path(__file__).with_name('test-workflow-file-handoff.py')))


class EvidenceRetention(unittest.TestCase):
    def test_long_archive_retains_native_history_and_refuses_existing_destination(self):
        root = Path(tempfile.mkdtemp(prefix='workflow-evidence-'))
        # Extended-path reads/teardown are independent of the archive implementation.
        def native(path):
            return '\\\\?\\' + str(path.resolve()) if os.name == 'nt' else str(path)
        try:
            source = root / 'owned'
            source.mkdir()
            history = b'{"type":"session","id":"owned"}\n{"type":"message","text":"native"}\n'
            (source / 'native.jsonl').write_bytes(history)
            target = root.joinpath(*['archive-' + 'x' * 64] * 4)
            runtime['copy_evidence'](source, target)
            self.assertEqual(Path(native(target / 'native.jsonl')).read_bytes(), history)
            self.assertEqual((source / 'native.jsonl').read_bytes(), history)
            (source / 'native.jsonl').write_bytes(b'NEW-RUN-MUST-NOT-OVERWRITE-OLD-EVIDENCE')
            with self.assertRaises(FileExistsError):
                runtime['copy_evidence'](source, target)
            self.assertEqual(Path(native(target / 'native.jsonl')).read_bytes(), history)
        finally:
            shutil.rmtree(native(root))


if __name__ == '__main__':
    unittest.main()
