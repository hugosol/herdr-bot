"""Public CLI safety checks; these do not establish P45/P46 real-model acceptance.

Approved seams: outer acceptance CLI/evidence, public Chat/Task/Bot/Run HTTP,
native role tools/history, actual output bytes and owned OS processes.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

REPO = Path(__file__).resolve().parent.parent


class NativeTaskUpdateTransport(unittest.TestCase):
    def test_put_reaches_real_upstream_and_records_unchanged_bytes(self):
        import http.server
        import threading
        import urllib.request
        from workflow_nesting_outer_acceptance import roles
        received = []

        class Upstream(http.server.BaseHTTPRequestHandler):
            def log_message(self, *_):
                pass

            def do_PUT(self):
                received.append((self.command, self.path, self.rfile.read(int(self.headers['Content-Length']))))
                self.send_response(200)
                self.end_headers()
                self.wfile.write(b'{"saved":true}')

        server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Upstream)
        thread = threading.Thread(target=server.serve_forever)
        thread.start()
        try:
            with tempfile.TemporaryDirectory(prefix='outer-put-wire-') as directory:
                proxy = roles['WireProxy'](f'http://127.0.0.1:{server.server_port}', Path(directory) / 'wire')
                try:
                    payload = b'{"prompt":"literal revised prompt","suite":"preserved-suite"}'
                    request = urllib.request.Request(proxy.base + '/api/tasks/owned-task', data=payload, method='PUT')
                    with urllib.request.urlopen(request) as response:
                        self.assertEqual(response.status, 200)
                        self.assertEqual(response.read(), b'{"saved":true}')
                finally:
                    proxy.close()
                self.assertEqual(received, [('PUT', '/api/tasks/owned-task', payload)])
                self.assertEqual(proxy.records[0]['request'], payload.decode())
                self.assertEqual(proxy.records[0]['method'], 'PUT')
                self.assertEqual(proxy.records[0]['response'], '{"saved":true}')
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=15)
            self.assertFalse(thread.is_alive())


class NativeFailureDiagnostics(unittest.TestCase):
    def test_terminal_capture_never_replays_input_or_replaces_original_failure(self):
        from unittest.mock import patch
        from workflow_nesting_outer_acceptance import OuterAcceptance, NativePlanningAcceptance
        acceptance = OuterAcceptance.__new__(OuterAcceptance)
        acceptance.evidence = {}
        commands = []
        acceptance.herdr = lambda *args: commands.append(args) or 'actual terminal buffer'
        chat = {'sessionId': 'owned-session', 'paneId': 'w1:p3', 'agent': 'omp'}
        with patch.object(NativePlanningAcceptance, 'native_turn', side_effect=AssertionError('original timeout')):
            with self.assertRaisesRegex(AssertionError, 'original timeout'):
                acceptance.native_turn(chat)
        self.assertEqual(commands, [('agent', 'read', 'w1:p3', '--source', 'recent-unwrapped', '--lines', '200', '--format', 'text')])
        self.assertEqual(acceptance.evidence['nativeTurnFailures'][0]['terminal'], 'actual terminal buffer')
        self.assertIn('original timeout', acceptance.evidence['nativeTurnFailures'][0]['originalError'])


class BusinessEvaluationContext(unittest.TestCase):
    def test_candidate_role_is_explicit_not_inferred_from_case_name(self):
        from workflow_nesting_outer_acceptance import OuterAcceptance
        acceptance = OuterAcceptance.__new__(OuterAcceptance)
        acceptance.confirmed = {}
        acceptance.file_bytes = lambda _: 'literal full candidate'
        acceptance.observe_turn = lambda _: {}
        acceptance.native_read = lambda *_: None
        acceptance.prompt = lambda *_: {}
        evaluated = []
        acceptance.evaluate = lambda *args: evaluated.append(args)
        with tempfile.TemporaryDirectory(prefix='outer-evaluation-role-') as directory:
            for name, expected in [('independent-creator', 'normal Task.prompt'),
                                   ('write_node_guide', 'Bot.node_guide concrete caller business inputs')]:
                proposal = Path(directory) / (name + '.txt')
                proposal.write_text('literal full candidate', encoding='utf-8')
                acceptance.confirm_business({}, proposal, 'opaque-case')
                self.assertEqual(evaluated[-1][1]['candidateRole'], expected)
                self.assertEqual(evaluated[-1][1]['candidate'], 'literal full candidate')


class NativeOriginalInput(unittest.TestCase):
    def launch_with(self, entry):
        from workflow_nesting_outer_acceptance import OuterAcceptance
        acceptance = OuterAcceptance.__new__(OuterAcceptance)
        acceptance.hosts = {}
        acceptance.mapping = {'creator': 'omp'}
        acceptance.launch = lambda *_: ({'sessionId': 'owned-session'}, {'branch': [entry]})
        acceptance.events = lambda: [{'id': 'owned-session', 'tools': []}]
        return acceptance.launch_independent('creator', 'literal original input')

    def test_native_user_and_user_attributed_skill_preserve_original_input(self):
        for entry in [
            {'type': 'message', 'message': {'role': 'user', 'content': 'literal original input'}},
            {'type': 'custom_message', 'attribution': 'user', 'content': 'Skill\nliteral original input'},
        ]:
            with self.subTest(entry=entry):
                self.assertEqual(self.launch_with(entry)[0]['sessionId'], 'owned-session')

    def test_agent_owned_custom_message_cannot_stand_in_for_original_input(self):
        with self.assertRaisesRegex(AssertionError, 'Original independent user input is missing'):
            self.launch_with({'type': 'custom_message', 'attribution': 'agent', 'content': 'literal original input'})


class OuterAcceptancePrerequisites(unittest.TestCase):
    def test_existing_invocation_evidence_is_never_reused(self):
        # Independent ticket requirement: each invocation owns new evidence.
        # Violation: overwriting an earlier successful result after a failure.
        with tempfile.TemporaryDirectory(prefix='outer-owned-evidence-') as directory:
            root = Path(directory)
            output = root / 'already-owned'
            output.mkdir()
            previous = output / 'evidence.json'
            previous.write_bytes(b'previous invocation must remain intact\n')
            result = subprocess.run([sys.executable, str(REPO / 'scripts/accept-workflow-nesting-outer.py'),
                '--backend', str(root / 'missing.exe'), '--toolkit', str(root / 'missing-toolkit'),
                '--hosts', 'omp,pi', '--paths-file', str(root / 'missing.toml'),
                '--model-config', str(root / 'model.json'), '--evidence-dir', str(output)],
                cwd=REPO, capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=30)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(previous.read_bytes(), b'previous invocation must remain intact\n')
            self.assertEqual(sorted(path.name for path in output.iterdir()), ['evidence.json'])
            self.assertEqual(result.stdout, '')

    def test_missing_worktree_backend_cannot_claim_outer_or_pairing_acceptance(self):
        # Independent source: ticket24 requires a current worktree build, not
        # prepared configuration. Violation: reporting passing coverage without
        # ever launching the declared mixed-host business flow.
        toolkit = Path(os.environ['OUTER_ACCEPTANCE_TOOLKIT']).resolve()
        with tempfile.TemporaryDirectory(prefix='outer-cli-') as directory:
            root = Path(directory)
            output = root / 'evidence'
            result = subprocess.run([sys.executable, str(REPO / 'scripts/accept-workflow-nesting-outer.py'),
                '--backend', str(root / 'missing/debug/herdr-bot.exe'), '--toolkit', str(toolkit),
                '--hosts', 'declared', '--paths-file', str(root / 'paths.toml'),
                '--model-config', str(root / 'model.json'), '--evidence-dir', str(output)],
                cwd=REPO, capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=60)
            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
            summary = json.loads(result.stdout)
            evidence = json.loads(Path(summary['evidence']).read_text(encoding='utf-8'))
            self.assertFalse(evidence['passed'])
            self.assertEqual(set(evidence['coverage']), {'P45/creator', 'P45/optimizer', 'P46/outer', 'P46/legacy', 'P46/scope'})
            self.assertTrue(all(not item['verified'] for item in evidence['coverage'].values()))
            self.assertIn('Missing current worktree backend', evidence['verificationError'])
            self.assertEqual(evidence['processes'], [])
            self.assertEqual(evidence['scenarios'], [])
            self.assertEqual(evidence['cleanupErrors'], [])
            self.assertFalse(any(output.rglob('models.json')))


if __name__ == '__main__':
    unittest.main()
