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


class PresentationEvidence(unittest.TestCase):
    def test_context_quotes_must_come_from_the_actual_answer(self):
        import io
        import runpy
        from types import SimpleNamespace
        from unittest.mock import patch
        runner = runpy.run_path(str(ROOT / 'scripts/chief_reports_native_acceptance.py'))['ChiefReportsAcceptance']
        rubric = ['context', 'outcome', 'delivery', 'wait', 'facts']
        answer = 'Original request: private summary, no publication. Executor: results-reporter. Blocked; no artifact. Does this meet your requirements?'
        valid = {'backgroundQuote': 'private summary, no publication', 'executionQuote': 'Executor: results-reporter'}
        with tempfile.TemporaryDirectory(prefix='chief-report-evaluator-') as temporary:
            config = Path(temporary) / 'model.json'
            config.write_text(json.dumps({'apiKeyEnv': 'REPORT_EVALUATOR_TEST_KEY'}), encoding='utf-8')
            run = runner.__new__(runner)
            run.args = SimpleNamespace(model_config=config, timeout=1)
            run.model_wire = SimpleNamespace(base='http://127.0.0.1:1')
            run.evidence = {}
            for quotes, passes in [(valid, True), ({**valid, 'backgroundQuote': 'invented background'}, False),
                                   ({**valid, 'executionQuote': ''}, False), ({}, False)]:
                decision = {'checks': {key: True for key in rubric},
                    'reasons': {key: 'Independent fixed criterion satisfied by the quoted actual answer.' for key in rubric},
                    'contextEvidence': quotes}
                response = {'choices': [{'message': {'content': json.dumps(decision)}}]}
                with patch.dict(os.environ, {'REPORT_EVALUATOR_TEST_KEY': 'isolated-test-marker'}), \
                     patch('urllib.request.urlopen', return_value=io.BytesIO(json.dumps(response).encode())):
                    if passes:
                        run.evaluate('blocked', {}, answer, {})
                    else:
                        with self.assertRaisesRegex(AssertionError, 'lacks exact answer evidence'):
                            run.evaluate('blocked', {}, answer, {})


if __name__ == '__main__':
    unittest.main()
