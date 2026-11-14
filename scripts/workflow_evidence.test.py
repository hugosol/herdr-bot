"""Owned evidence trees retain native history bytes beyond Windows MAX_PATH."""
import os
from pathlib import Path
import shutil
import tempfile
import unittest

from acceptance.host.evidence import copy_evidence


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
            copy_evidence(source, target)
            self.assertEqual(Path(native(target / 'native.jsonl')).read_bytes(), history)
            self.assertEqual((source / 'native.jsonl').read_bytes(), history)
            (source / 'native.jsonl').write_bytes(b'NEW-RUN-MUST-NOT-OVERWRITE-OLD-EVIDENCE')
            with self.assertRaises(FileExistsError):
                copy_evidence(source, target)
            self.assertEqual(Path(native(target / 'native.jsonl')).read_bytes(), history)
        finally:
            shutil.rmtree(native(root))


if __name__ == '__main__':
    unittest.main()
