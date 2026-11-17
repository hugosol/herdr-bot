"""Approved seam: the executable V2 acceptance command, not private modules.

Independent ticket oracle: missing authorization cannot certify either host or
P34/S2, P34/S3, P35/S2. A plausible violation is an empty/filtered matrix marked
passed. These checks do not claim real-model, native Yes or browser acceptance.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
ENTRY = ROOT / 'scripts/accept-node-approval-v2.py'
ITEMS = {'P34/V2/S2', 'P34/V2/S3', 'P35/V2/S2'}


class PublishedV2CLI(unittest.TestCase):
    def invoke(self, root, **overrides):
        values = {'herdr-bot-worktree': str(ROOT), 'toolkit-worktree': str(root / 'toolkit'),
            'backend': str(root / 'herdr-bot.exe'), 'hosts': 'omp,pi',
            'paths-file': str(root / 'paths.toml'), 'model-config': str(root / 'model.json'),
            'evidence-dir': str(root / 'evidence')}
        values.update(overrides)
        return subprocess.run([sys.executable, str(ENTRY), *[part for key, value in values.items()
            for part in (['--' + key] if value is None else ['--' + key, value])]], cwd=ROOT, capture_output=True,
            text=True, encoding='utf-8', errors='replace', timeout=30)

    def test_missing_authorization_keeps_both_hosts_and_every_partition_unverified(self):
        with tempfile.TemporaryDirectory(prefix='v2-cli-') as directory:
            root = Path(directory)
            result = self.invoke(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertTrue((root / 'evidence/evidence.json').is_file(), result.stderr)
            report = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertFalse(report['passed'])
            self.assertIn('authorized', report['error'].lower())
            self.assertEqual(set(report['coverage']), {'omp', 'pi'})
            for host in report['coverage'].values():
                self.assertEqual(set(host), ITEMS)
                for item in host.values():
                    self.assertFalse(item['verified'])
                    self.assertTrue(item['expected'])
            self.assertEqual(report['commands'], [])
            self.assertEqual(report['cleanupErrors'], [])

    def test_native_only_never_certifies_missing_authorization_or_repository_checks(self):
        with tempfile.TemporaryDirectory(prefix='v2-native-only-') as directory:
            root = Path(directory)
            result = self.invoke(root, **{'native-only': None})
            self.assertNotEqual(result.returncode, 0)
            self.assertTrue((root / 'evidence/evidence.json').is_file(), result.stderr)
            report = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertIn('authorized', report['error'])
            self.assertTrue(report['nativeOnly'])
            self.assertFalse(report['nativeCoveragePassed'])
            self.assertFalse(report['passed'])
            self.assertEqual(report['commands'], [])

    def test_authorized_transport_recognizes_v2_before_dependency_refusal(self):
        with tempfile.TemporaryDirectory(prefix='v2-transport-cli-') as directory:
            root = Path(directory)
            result = subprocess.run([sys.executable, str(ROOT / 'scripts/accept-workflow-codex.py'),
                '--scenario', 'node-approval-v2', '--hosts', 'omp,pi', '--backend', str(root / 'absent.exe'),
                '--toolkit', str(root / 'toolkit'), '--paths-file', str(root / 'paths.toml'),
                '--evidence-dir', str(root / 'evidence'), '--pi-package', str(root / 'pi'),
                '--auth-file', str(root / 'explicit-authorized-only.json')], cwd=ROOT,
                capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=30)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('Missing explicit dependency', result.stderr)
            self.assertFalse((root / 'evidence').exists())

    def test_existing_evidence_preserves_original_failure_bytes(self):
        with tempfile.TemporaryDirectory(prefix='v2-cli-') as directory:
            root = Path(directory)
            evidence = root / 'evidence'
            evidence.mkdir()
            previous = evidence / 'evidence.json'
            previous.write_bytes(b'ORIGINAL-V2-FAILURE-815\r\n')
            result = self.invoke(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(previous.read_bytes(), b'ORIGINAL-V2-FAILURE-815\r\n')
            self.assertEqual(list(evidence.iterdir()), [previous])

    def test_relative_dependency_path_is_refused_without_creating_evidence(self):
        with tempfile.TemporaryDirectory(prefix='v2-cli-') as directory:
            root = Path(directory)
            result = self.invoke(root, backend='relative/herdr-bot.exe')
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('absolute', result.stderr.lower())
            self.assertFalse((root / 'evidence').exists())


class NativeConfiguration(unittest.TestCase):
    def test_shared_parent_shell_is_effective_in_installed_omp_yaml_priority(self):
        from argparse import Namespace
        from unittest.mock import patch
        from acceptance.host.native import NativeAcceptance
        package = Path(os.environ['NODE_APPROVAL_HANDOFF_OMP_PACKAGE'])
        # A real installed non-default shell distinguishes the override from
        # automatic Git Bash resolution; no shell is installed or executed here.
        selected = Path(os.environ['SystemRoot']) / 'System32/WindowsPowerShell/v1.0/powershell.exe'
        self.assertTrue(selected.is_file())
        with tempfile.TemporaryDirectory(prefix='native-effective-shell-') as directory:
            root = Path(directory)
            agent = root / 'agent'
            agent.mkdir()
            config = root / 'model.json'
            config.write_text(json.dumps({'provider': 'owned', 'model': 'mechanical',
                'baseUrl': 'http://127.0.0.1:9', 'apiKeyEnv': 'OWNED_NATIVE_CONFIG_MARKER'}), encoding='utf-8')
            fixture = NativeAcceptance.__new__(NativeAcceptance)
            fixture.args = Namespace(model_config=config, shell_path=selected)
            fixture.root, fixture.hosts, fixture.evidence, fixture.env = root, ['omp'], {}, {}
            try:
                with patch.dict(os.environ, OWNED_NATIVE_CONFIG_MARKER='non-secret-owned-marker'):
                    NativeAcceptance.configure_model(fixture)
                script = ('import {Settings} from ' + json.dumps((package / 'src/config/settings.ts').as_uri())
                    + ';const s=await Settings.loadReadOnly({agentDir:' + json.dumps(str(agent))
                    + ',cwd:' + json.dumps(str(root)) + '});const c=s.getShellConfig();'
                    + 'console.log(JSON.stringify({shell:c.shell,args:c.args}));')
                environment = dict(os.environ, HOME=str(root), USERPROFILE=str(root), OMP_CONFIG_DIR=str(agent),
                    OMP_CODING_AGENT_DIR=str(agent), PI_CONFIG_FILES='', TMP=str(root), TEMP=str(root), TMPDIR=str(root))
                process = subprocess.run(['bun', '--eval', script], cwd=root, env=environment,
                    capture_output=True, text=True, encoding='utf-8', timeout=30)
                self.assertEqual(process.returncode, 0, process.stdout + process.stderr)
                effective = json.loads(process.stdout)
                self.assertEqual(Path(effective['shell']).resolve(), selected.resolve())
                self.assertIn('-Command', effective['args'])
            finally:
                if getattr(fixture, 'model_wire', None):
                    fixture.model_wire.close()


class BusinessAuthorization(unittest.TestCase):
    """Main-approved acceptance-helper seam, not native business coverage.

    Original Task creation forbids Bot creation; only design_and_create may
    save an actually displayed complete Bot. A blanket positive confirmation
    would wrongly grant the same candidate during create_task.
    """
    def policy(self, stage, action, candidate):
        from node_approval_v2_acceptance import authorize_candidate
        expected = {'task': {'name': 'Owned CSV', 'chatName': '{chat.id}', 'taskType': 'normal',
            'agent': 'pi', 'model': 'probe', 'thinking': 'default', 'skill': None, 'suite': None,
            'prompt': 'Read only two explicitly authorized CSV files; never upload or write them.'},
            'taskId': 'task-owned', 'botId': 'owned-csv', 'requiredFacts': ['CSV', '只读', '不上传', '不写']}
        return authorize_candidate(stage, action, candidate, expected)

    def bot(self):
        return {'id': 'owned-csv', 'name': 'Owned CSV', 'task_id': 'task-owned',
            'when': '用户授权两份CSV时使用', 'plan_guide': '只读CSV，不上传，不写原文件',
            'node_guide': '比较明确授权的两份文件，缺失则等待用户'}

    def test_task_and_alignment_never_authorize_a_bot_save(self):
        for stage in ['create_task', 'align_requirements']:
            with self.subTest(stage=stage):
                decision = self.policy(stage, 'confirm-bot', self.bot())
                self.assertFalse(decision['authorized'])
                self.assertEqual(decision['reason'], 'stage-forbidden')

    def test_design_authorizes_only_the_displayed_complete_bound_bot(self):
        self.assertTrue(self.policy('design_and_create', 'confirm-bot', self.bot())['authorized'])
        self.assertFalse(self.policy('design_and_create', 'confirm-bot', None)['authorized'])
        self.assertFalse(self.policy('design_and_create', 'confirm-bot', dict(self.bot(), task_id='other-task'))['authorized'])
        incomplete = self.bot()
        del incomplete['node_guide']
        self.assertFalse(self.policy('design_and_create', 'confirm-bot', incomplete)['authorized'])

    def test_task_confirmation_does_not_allow_prompt_rewriting_or_future_actions(self):
        task = {'name': 'Owned CSV', 'chatName': '{chat.id}', 'taskType': 'normal', 'agent': 'pi',
            'model': 'probe', 'thinking': 'default', 'skill': None, 'suite': None,
            'prompt': 'Read only two explicitly authorized CSV files; never upload or write them.'}
        self.assertTrue(self.policy('create_task', 'confirm-task', task)['authorized'])
        self.assertFalse(self.policy('create_task', 'confirm-task', dict(task, prompt='Upload the CSV'))['authorized'])
        self.assertFalse(self.policy('create_task', 'unknown-save', task)['authorized'])
        self.assertFalse(self.policy('unknown-stage', 'confirm-task', task)['authorized'])

    def test_alignment_confirms_only_its_displayed_business_requirements(self):
        candidate = {'requirements': '# 本次要求\n两份CSV只读比较，不上传、不写原文件；缺失时等待用户。'}
        self.assertTrue(self.policy('align_requirements', 'confirm-requirements', candidate)['authorized'])
        self.assertFalse(self.policy('create_task', 'confirm-requirements', candidate)['authorized'])
        self.assertFalse(self.policy('align_requirements', 'confirm-requirements', {'requirements': '请确认未来保存'})['authorized'])

    def test_actual_form_bot_configuration_does_not_expand_task_stage_authority(self):
        from html import escape
        from node_approval_v2_acceptance import BusinessForm, displayed_candidates
        form = BusinessForm('<form><pre>' + escape(json.dumps(self.bot()))
            + '</pre><label><input type="radio" name="create" value="yes">是：保存完整候选</label></form>')
        candidates = displayed_candidates('\n'.join('```json\n' + block + '\n```' for block in form.blocks))
        self.assertEqual(candidates, [('confirm-bot', self.bot())])
        self.assertFalse(self.policy('create_task', *candidates[0])['authorized'])
        self.assertTrue(self.policy('design_and_create', *candidates[0])['authorized'])

    def test_scope_clarification_requires_its_actual_cancelled_call_boundary(self):
        from node_approval_v2_acceptance import cancelled_scope_ready
        pending = {'toolCallId': 'ask-owned', 'message': 'Only this stage is authorized.'}
        history = {'branch': [
            {'type': 'message', 'message': {'role': 'toolResult', 'toolCallId': 'ask-owned', 'isError': True,
                'content': [{'type': 'text', 'text': 'Ask tool was cancelled by the user'}]}},
            {'type': 'message', 'message': {'role': 'assistant', 'stopReason': 'aborted', 'errorMessage': 'Interrupted by user'}}]}
        self.assertTrue(cancelled_scope_ready(pending, history))
        self.assertFalse(cancelled_scope_ready(dict(pending, toolCallId='other-ask'), history))
        self.assertFalse(cancelled_scope_ready(None, history))
        history['branch'][-1]['message'] = {'role': 'assistant', 'stopReason': 'error', 'errorMessage': 'provider-error'}
        self.assertFalse(cancelled_scope_ready(pending, history))
        history['branch'][-1]['message'] = {'role': 'assistant', 'stopReason': 'aborted', 'errorMessage': 'Unknown interruption'}
        self.assertFalse(cancelled_scope_ready(pending, history))

    def test_unstructured_prose_cannot_be_mistaken_for_a_displayed_configuration(self):
        from node_approval_v2_acceptance import displayed_candidates
        self.assertEqual(displayed_candidates('请确认创建完整 Bot；when plan_guide node_guide 全部完成。'), [])
        shown = displayed_candidates('完整候选：\n```json\n' + json.dumps(self.bot()) + '\n```')
        self.assertEqual(shown, [('confirm-bot', self.bot())])


if __name__ == '__main__':
    unittest.main()
