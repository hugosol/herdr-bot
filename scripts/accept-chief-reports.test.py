"""S2/S3 runner safety, not report acceptance.

Approved seam: executable CLI. Missing dependencies must keep every host/promise
unverified; an existing evidence directory must not be overwritten. This catches
an accidental partial-success exit without supplying any business tool decision.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
TOOLKIT = Path(os.environ['CHIEF_REPORTS_TOOLKIT'])


class ReportsCLI(unittest.TestCase):
    def test_missing_backend_is_nonzero_and_preserves_all_unverified_items(self):
        with tempfile.TemporaryDirectory(prefix='chief-reports-cli-') as temporary:
            root = Path(temporary)
            values = {'herdr-bot-worktree': ROOT, 'toolkit-worktree': TOOLKIT,
                      'backend': root / 'missing.exe', 'hosts': 'omp,pi',
                      'paths-file': root / 'paths.toml', 'model-config': root / 'model.json',
                      'evidence-dir': root / 'evidence'}
            result = subprocess.run([sys.executable, str(ROOT / 'scripts/accept-chief-reports.py'),
                *[part for key, value in values.items() for part in ['--' + key, str(value)]]],
                cwd=ROOT, capture_output=True, text=True, timeout=30)
            self.assertNotEqual(result.returncode, 0)
            report = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertFalse(report['passed'])
            self.assertIn('backend', report['error'].lower())
            self.assertEqual(set(report['coverage']), {'omp', 'pi'})
            for host in report['coverage'].values():
                self.assertEqual(set(host), {'P18', 'P19', 'P20', 'P22', 'P28', 'P36', 'P37'})
                self.assertTrue(all(not item['verified'] for item in host.values()))


if __name__ == '__main__':
    unittest.main()
