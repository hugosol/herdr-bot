"""Public CLI prerequisite contract; does not claim real-model role acceptance.

Approved seam: the delivered scenario CLI + its evidence output. Independent
source: missing real dependencies must return nonzero, preserve the error and
not claim an executed role scenario. A runner that prints preparation as success
or fabricates a completed Run fails this test. No host/provider is substituted.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


REPO = Path(__file__).resolve().parent.parent


class RoleAcceptancePrerequisites(unittest.TestCase):
    def test_fan_in_missing_backend_preserves_error_without_claiming_business_evidence(self):
        # Source: ticket P23/P24 must fail closed without actual dependencies.
        # Violation: accepting fan-in CLI preparation as successful execution.
        # The public command must select fan-in, write a failure evidence record,
        # and never fabricate real runs, native reads or independent artifacts.
        self.check_missing_backend('fan-in')

    def test_missing_worktree_backend_fails_with_original_error_and_no_role_claim(self):
        self.check_missing_backend('roles')

    def test_plan_workflow_missing_backend_fails_without_claiming_business_acceptance(self):
        # P27/P28: a prepared definition or absent host is never a passing run.
        # The CLI must reach its real prerequisites, not reject this scenario as
        # an unsupported option or invent native confirmation/completion facts.
        self.check_missing_backend('plan-workflow')

    def check_missing_backend(self, scenario):
        toolkit = Path(os.environ['WORKFLOW_HANDOFF_TOOLKIT']).resolve()
        self.assertTrue((toolkit / 'bun.lock').is_file(), 'Set WORKFLOW_HANDOFF_TOOLKIT to the participating actual toolkit source')
        with tempfile.TemporaryDirectory(prefix='workflow-roles-cli-prerequisite-') as directory:
            root = Path(directory)
            target = root / 'not-built-target'
            backend = target / 'debug' / ('herdr-bot.exe' if os.name == 'nt' else 'herdr-bot')
            paths = root / 'isolated-paths.toml'
            paths.write_text('[roots]\n', encoding='utf-8')
            result = subprocess.run([sys.executable, str(REPO / 'scripts/test-workflow-file-handoff.py'),
                '--backend', str(backend), '--toolkit', str(toolkit), '--scenario', scenario,
                '--paths-file', str(paths), '--herdr', str(root / 'unavailable-herdr.exe'),
                '--pi-package', str(root / 'unavailable-pi'), '--pi-state-extension', str(root / 'unavailable-state.ts'),
                '--model-config', str(root / 'unavailable-model.json'),
                '--shell-path', str(root / 'unavailable-native-bash.exe')], cwd=REPO,
                env=dict(os.environ, CARGO_TARGET_DIR=str(target)), capture_output=True,
                text=True, encoding='utf-8', errors='replace', timeout=30)
            # The toolkit argument must identify a real source root for source attribution;
            # this CLI-only test reaches its missing-backend check before any role loads.
            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
            summary = json.loads(result.stdout)
            self.assertFalse(summary['passed'])
            evidence_file = Path(summary['evidence'])
            evidence = json.loads(evidence_file.read_text(encoding='utf-8'))
            self.assertIn('Missing current worktree backend', evidence['verificationError'])
            self.assertNotIn('started', evidence)
            self.assertNotIn('final', evidence)
            self.assertEqual(evidence['commands'], [])
            self.assertEqual(evidence['cleanupErrors'], [])
            self.assertFalse((evidence_file.parent / 'agent/models.json').exists())
            self.assertFalse((evidence_file.parent / 'backend-stdout.log').exists())


if __name__ == '__main__':
    unittest.main()
