"""Public CLI seam: missing authorization must retain every owned criterion.

Independent source: ticket 12's seven coverage partitions require both real hosts.
Plausible violation: an empty/filtered run certifies success. No host/model double.
Native behavioral oracles live in the executable scenario, not these CLI checks.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
ENTRY = ROOT / 'scripts/accept-node-approval-lifecycle.py'
PROMISES = {'P3', 'P10', 'P11', 'P14', 'P16', 'P24', 'P28'}


class LifecycleCLI(unittest.TestCase):
    def invoke(self, root, **changes):
        values = {'herdr-bot-worktree': str(ROOT), 'toolkit-worktree': str(root / 'toolkit'),
                  'backend': str(root / 'missing/herdr-bot.exe'), 'hosts': 'omp,pi',
                  'paths-file': str(root / 'paths.toml'), 'model-config': str(root / 'model.json'),
                  'evidence-dir': str(root / 'evidence')}
        values.update(changes)
        return subprocess.run([sys.executable, str(ENTRY), *[part for key, value in values.items()
            for part in ['--' + key, value]]], cwd=ROOT, capture_output=True,
            text=True, encoding='utf-8', errors='replace', timeout=30)

    def test_missing_authorization_preserves_seven_dual_host_partitions(self):
        with tempfile.TemporaryDirectory(prefix='lifecycle-cli-') as directory:
            root = Path(directory)
            result = self.invoke(root)
            self.assertNotEqual(result.returncode, 0)
            report = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertFalse(report['passed'])
            self.assertIn('authorized', report['error'].lower())
            self.assertEqual(set(report['coverage']), {'omp', 'pi'})
            for host in ['omp', 'pi']:
                self.assertEqual(set(report['coverage'][host]), PROMISES)
                self.assertTrue(all(not item['verified'] for item in report['coverage'][host].values()))
            self.assertEqual(report['checks'], [])
            self.assertEqual(report['nativeExits'], {})

    def test_invalid_cases_fail_before_creating_evidence_or_model_resources(self):
        for cases in ['unknown', 'lifecycle,lifecycle', '', 'persistence, nested-auto']:
            with self.subTest(cases=cases), tempfile.TemporaryDirectory(prefix='lifecycle-cases-') as directory:
                root = Path(directory)
                result = self.invoke(root, cases=cases)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn('distinct comma-separated', result.stderr)
                self.assertFalse((root / 'evidence').exists())

    def test_partial_cases_keep_full_missing_coverage_unverified(self):
        with tempfile.TemporaryDirectory(prefix='lifecycle-partial-') as directory:
            root = Path(directory)
            result = self.invoke(root, hosts='pi', cases='lifecycle')
            self.assertNotEqual(result.returncode, 0)
            report = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertEqual(report['selectedCases'], ['pi/lifecycle'])
            self.assertEqual(len(report['missingCases']), 7)
            self.assertFalse(report['selectedCasesPassed'])
            self.assertFalse(report['passed'])
            self.assertTrue(all(not item['verified'] for host in report['coverage'].values() for item in host.values()))

    def test_composition_rejects_missing_inputs_without_model_authorization(self):
        with tempfile.TemporaryDirectory(prefix='lifecycle-compose-') as directory:
            root = Path(directory)
            result = self.invoke(root, combine=str(root / 'missing-original.json'))
            report = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('distinct existing original evidence', report['error'])
            self.assertFalse(report['passed'])
            self.assertEqual(report['nativeExits'], {})

    def test_composition_cannot_hide_missing_host_or_case(self):
        with tempfile.TemporaryDirectory(prefix='lifecycle-compose-filter-') as directory:
            root = Path(directory)
            result = self.invoke(root, combine=str(root / 'original.json'), hosts='omp', cases='lifecycle')
            report = json.loads((root / 'evidence/evidence.json').read_text(encoding='utf-8'))
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('full dual-host matrix', report['error'])
            self.assertFalse(report['passed'])
            self.assertEqual(report['checks'], [])

    def test_existing_evidence_is_not_overwritten(self):
        with tempfile.TemporaryDirectory(prefix='lifecycle-cli-') as directory:
            root = Path(directory)
            (root / 'evidence').mkdir()
            previous = root / 'evidence/evidence.json'
            previous.write_bytes(b'ORIGINAL-FAILURE-731\n')
            result = self.invoke(root)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(previous.read_bytes(), b'ORIGINAL-FAILURE-731\n')

    def test_codex_routes_lifecycle_without_auth_discovery(self):
        with tempfile.TemporaryDirectory(prefix='lifecycle-transport-cli-') as directory:
            root = Path(directory)
            result = subprocess.run([sys.executable, str(ROOT / 'scripts/accept-workflow-codex.py'),
                '--scenario', 'node-approval-lifecycle', '--hosts', 'omp,pi', '--cases', 'lifecycle',
                '--backend', str(root / 'absent.exe'), '--toolkit', str(root / 'toolkit'),
                '--paths-file', str(root / 'paths.toml'), '--evidence-dir', str(root / 'evidence'),
                '--pi-package', str(root / 'pi'), '--auth-file', str(root / 'explicit-only.json')],
                cwd=ROOT, capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=30)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('Missing explicit dependency', result.stderr)
            self.assertFalse((root / 'evidence').exists())

    def test_relative_paths_are_refused_before_creating_evidence(self):
        with tempfile.TemporaryDirectory(prefix='lifecycle-cli-') as directory:
            root = Path(directory)
            result = self.invoke(root, backend='relative/herdr-bot.exe')
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('absolute', result.stderr.lower())
            self.assertFalse((root / 'evidence').exists())


class NativeDeviceHistory(unittest.TestCase):
    """Public native history decoding; no claim of native business coverage."""

    @staticmethod
    def history():
        entries = [
            {'role': 'assistant', 'content': [{'type': 'toolCall', 'id': 'schema', 'name': 'write',
                'arguments': {'path': 'xd://approve', 'content': ''}}]},
            {'role': 'toolResult', 'toolCallId': 'schema', 'toolName': 'write', 'isError': False,
                'details': {'xdev': {'tool': 'approve', 'mode': 'help'}}},
            {'role': 'assistant', 'content': [{'type': 'toolCall', 'id': 'approval', 'name': 'write',
                'arguments': {'path': 'xd://approve', 'content': '{}'}}]},
            {'role': 'toolResult', 'toolCallId': 'approval', 'toolName': 'write', 'isError': False,
                'details': {'xdev': {'tool': 'approve', 'mode': 'execute', 'args': {}}}},
        ]
        return {'branch': [{'type': 'message', 'message': entry} for entry in entries]}

    def test_help_then_routed_approval_is_one_receipt_linked_business_call(self):
        from node_approval_lifecycle_acceptance import calls
        history = self.history()
        decoded = calls(history)
        self.assertEqual([(call['id'], call['name'], call['arguments']) for call in decoded],
                         [('approval', 'approve', {})])
        self.assertEqual(decoded[0]['nativeRoute']['arguments'],
                         {'path': 'xd://approve', 'content': '{}'})
        self.assertEqual(calls(history, 'approve', start=2), decoded)

    def test_device_address_without_dispatch_receipt_is_not_approval(self):
        from node_approval_lifecycle_acceptance import calls
        history = self.history()
        history['branch'][3]['message']['details'] = {}
        self.assertEqual(calls(history, 'approve'), [])
        self.assertEqual([call['name'] for call in calls(history)], ['write'])

    def test_validated_receipt_does_not_hide_model_selected_identity(self):
        from node_approval_lifecycle_acceptance import calls
        history = self.history()
        history['branch'][2]['message']['content'][0]['arguments']['content'] = '{"executionId":"other-execution"}'
        self.assertEqual(calls(history, 'approve')[0]['arguments'], {'executionId': 'other-execution'})

    def test_unrelated_file_write_is_not_routed_approval(self):
        from node_approval_lifecycle_acceptance import calls
        history = self.history()
        history['branch'][2]['message']['content'][0]['arguments']['path'] = 'C:/owned/approve.json'
        self.assertEqual(calls(history, 'approve'), [])
        self.assertEqual([call['name'] for call in calls(history)], ['write'])


class NativeRoutingSafety(unittest.TestCase):
    """Offline facility safety only, never native Session/recovery coverage.

    Public facility methods consume actual local HTTP and an external recording
    CLI. Literal IDs deliberately represent a misrouted request, not a real Run.
    No successful restoration or native business result is simulated.
    """
    def test_explicit_chat_targets_recording_cli_on_success_and_error(self):
        from types import SimpleNamespace
        from node_approval_failures_acceptance import ApprovalFailuresAcceptance
        with tempfile.TemporaryDirectory(prefix='native-routing-cli-') as directory:
            root = Path(directory)
            (root / 'pane').write_text('import os,sys\nif os.environ.get("ROUTING_FAILURE")=="1": sys.exit(7)\nprint("TARGET="+sys.argv[2])\n', encoding='utf-8')
            run = ApprovalFailuresAcceptance.__new__(ApprovalFailuresAcceptance)
            run.root, run.args, run.evidence = root, SimpleNamespace(herdr=Path(sys.executable)), {'commands': []}
            run.env = dict(__import__('os').environ)
            original = {'sessionId': 'OFFLINE-ORIGINAL', 'paneId': 'original-pane'}
            target = {'sessionId': 'OFFLINE-TARGET', 'paneId': 'target-pane'}
            run.worker = original
            self.assertIn('TARGET=target-pane', run.terminal(target))
            self.assertEqual(run.evidence['commands'][-1]['argv'][3], 'target-pane')
            self.assertEqual(run.worker, original)
            run.env['ROUTING_FAILURE'] = '1'
            with self.assertRaises(AssertionError):
                run.terminal(target)
            self.assertEqual(run.evidence['commands'][-1]['argv'][3], 'target-pane')
            self.assertEqual(run.worker, original)

    def test_gate_holds_only_the_marked_public_native_event_until_owner_release(self):
        # External extension API boundary only. This exercises actual observer
        # filesystem/await behavior, not an installed host or model decision.
        with tempfile.TemporaryDirectory(prefix='native-gate-protocol-') as directory:
            root = Path(directory)
            code = '''
import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';
const root = process.argv[1];
const release = join(root, 'release');
const configuration = join(root, 'config.json');
writeFileSync(configuration, JSON.stringify({marker:'OFFLINE-OWNED-INPUT',release,evidence:root}));
process.env.NODE_APPROVAL_LIFECYCLE_GATE = configuration;
const hooks = new Map();
const observer = (await import(_OBSERVER_)).default;
observer({on:(name,handler)=>hooks.set(name,handler)});
const ctx = {sessionManager:{getSessionId:()=> 'OFFLINE-NATIVE-PROTOCOL'}};
await hooks.get('before_agent_start')({prompt:'UNRELATED-INPUT'},ctx);
await hooks.get('context')({type:'context'},ctx);
const unrelatedObserved = existsSync(join(root,'events.jsonl'));
await hooks.get('before_agent_start')({prompt:'OFFLINE-OWNED-INPUT'},ctx);
let finished = false;
const pending = hooks.get('context')({type:'context'},ctx).then(value=>{finished=true;return value;});
await new Promise(resolve=>setTimeout(resolve,50));
const latest = () => JSON.parse(readFileSync(join(root,'events.jsonl'),'utf8').trim().split('\\n').at(-1));
const held = latest();
const pendingBeforeRelease = !finished;
writeFileSync(release,'explicit owner release');
const result = await pending;
const released = latest();
console.log(JSON.stringify({unrelatedObserved,pendingBeforeRelease,held,released,unchangedPayload:result===undefined}));
'''.replace('_OBSERVER_', json.dumps((ROOT / 'scripts/node-approval-lifecycle-gate.mjs').as_uri()))
            process = subprocess.run(['node', '--input-type=module', '--eval', code, str(root)],
                capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=10)
            self.assertEqual(process.returncode, 0, process.stderr)
            actual = json.loads(process.stdout)
            self.assertFalse(actual['unrelatedObserved'])
            self.assertTrue(actual['pendingBeforeRelease'])
            self.assertEqual(actual['held']['sessionId'], 'OFFLINE-NATIVE-PROTOCOL')
            self.assertEqual(actual['held']['state'], 'held-before-first-business-request')
            self.assertEqual(actual['released']['state'], 'released')
            self.assertTrue(actual['unchangedPayload'])

    def test_wrong_restore_chat_is_rejected_after_public_lookup_before_native_exit(self):
        import http.server
        import threading
        from node_approval_lifecycle_acceptance import ApprovalLifecycleAcceptance
        requests = []
        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *_):
                pass
            def do_GET(self):
                requests.append(self.path)
                raw = json.dumps({'run': {'nodes': {'work': {'execution': {'id': 'OFFLINE-EXECUTION',
                    'chat': {'sessionId': 'OFFLINE-ORIGINAL'}}}}}}).encode()
                self.send_response(200)
                self.send_header('Content-Length', str(len(raw)))
                self.end_headers()
                self.wfile.write(raw)
        import errno
        import secrets
        for attempt in range(32):
            try:
                server = http.server.ThreadingHTTPServer(('127.0.0.1', 49152 + secrets.randbelow(16384)), Handler)
                break
            except OSError as error:
                if error.errno != errno.EADDRINUSE or attempt == 31:
                    raise
        thread = threading.Thread(target=server.serve_forever)
        thread.start()
        try:
            run = ApprovalLifecycleAcceptance.__new__(ApprovalLifecycleAcceptance)
            run.base = 'http://127.0.0.1:' + str(server.server_port)
            run.evidence = {'commands': []}
            # No process/Session fixture exists. Without the preflight identity
            # guard, restore would attempt native work and fail differently.
            with self.assertRaisesRegex(AssertionError, 'exact public execution Chat'):
                run.restore({'sessionId': 'OFFLINE-WRONG', 'paneId': 'wrong-pane'},
                            'OFFLINE-RUN', 'work', 'OFFLINE-EXECUTION')
            self.assertEqual(requests, ['/api/workflow/runs/OFFLINE-RUN'])
            self.assertEqual(run.evidence['commands'], [])
        finally:
            server.shutdown()
            server.server_close()
            thread.join()


@unittest.skipUnless(os.environ.get('NODE_APPROVAL_LIFECYCLE_FIXTURE_TOOLKIT'),
                     'Supply explicit owned worktree/binary/paths for the native fixture regression')
class CollectRegistrationFixture(unittest.TestCase):
    """Controlled model choices, real installed Pi and production HTTP only.

    This is facility-enabling evidence, not real-model judgment or any P promise.
    The old stop-wait in track/role deadlocks before the child operation that can
    release collect. No private client method is mocked and no identity is made up.
    """
    def test_real_collect_can_remain_pending_across_registration_then_child_progress(self):
        import time
        from types import SimpleNamespace
        from acceptance.host.completion_transport import NativeCompletionTransport
        from acceptance.host.processes import require, until
        from acceptance.host.model import message_text
        from node_approval_lifecycle_acceptance import ApprovalLifecycleAcceptance, IDLE
        toolkit = Path(os.environ['NODE_APPROVAL_LIFECYCLE_FIXTURE_TOOLKIT']).resolve()
        backend = Path(os.environ['NODE_APPROVAL_LIFECYCLE_FIXTURE_BACKEND']).resolve()
        paths = Path(os.environ['NODE_APPROVAL_LIFECYCLE_FIXTURE_PATHS']).resolve()
        roots = json.loads(subprocess.run([str(backend.parent / 'herdr-paths.exe'), '--paths', str(paths)],
            capture_output=True, text=True, check=True, encoding='utf-8').stdout)
        def mechanical(request):
            tools = {tool.get('function', {}).get('name') for tool in request.get('tools', [])}
            users = [message for message in request.get('messages', []) if message.get('role') == 'user']
            collect = 'collect' in tools and users and 'FIXTURE_WAIT_COLLECT_731' in message_text(users[-1])
            return {'content': '' if collect else 'Controlled fixture acknowledgment; no business decision.',
                'tool_calls': [{'name': 'collect', 'arguments': {}}] if collect else [],
                'usage': {'prompt_tokens': 1, 'completion_tokens': 1, 'total_tokens': 2}}
        class Fixture(ApprovalLifecycleAcceptance):
            def exercise(self):
                self.evidence.update(promises=[], coverage={}, scenario='fixture-native-collect-registration',
                    claimsRealModelJudgment=False,
                    independentExpected={'registration': 'Returns while actual native collect remains pending.',
                        'advance': 'A public child progress report completes and releases that legitimate waiting collect.'},
                    scope='Controlled local model only; actual native Session/collect and real production HTTP. No owned P criterion is certified.')
                for role in ['manager', 'worker']:
                    directory = self.root / ('zone-' + role)
                    directory.mkdir()
                    self.http('/api/zones', {'id': 'lifecycle/' + role, 'path': str(directory)})
                    self.herdr('workspace', 'create', '--cwd', str(directory), '--label', directory.name, '--no-focus')
                self.manager_task = self.task('fixture-manager', 'manager', IDLE, self.host)
                worker = self.task('fixture-worker', 'normal', IDLE, self.host)
                (self.root / 'bots/worker.json').write_text(json.dumps({'id': 'worker', 'name': 'Fixture worker',
                    'when': '', 'plan_guide': '', 'node_guide': '', 'task_id': worker}), encoding='utf-8')
                self.define_map('fixture-child', {'inside': self.node('Owned inside work')})
                (self.root / 'bots/nested.json').write_text(json.dumps({'id': 'nested', 'name': 'Fixture nested',
                    'when': '', 'plan_guide': '', 'node_guide': '', 'map_file': 'fixture-child.json'}), encoding='utf-8')
                self.define_map('fixture-parent', {'call': self.node('Owned nested call', bot='nested', manager_task_id=self.manager_task)})
                parent_id, manager = self.new_run('fixture-parent')
                execution, dual = self.dispatch(manager, 'call', parent_id)
                child_id = execution['childRunId']
                self.action(dual, 'prepare_inputs', {'fileInputs': [], 'confirmed': True, 'reason': 'Owned empty fixture inputs'})
                inner_execution, inner = self.dispatch(dual, 'inside', child_id)
                self.send(dual, 'FIXTURE_WAIT_COLLECT_731: controlled fixture begins its native collect observation.', 'fixture-collect-stimulus')
                def waiting():
                    return next((record for record in self.wire('/api/workflow/manager/collect')
                        if json.loads(record['request']).get('sessionId') == dual['sessionId'] and not record.get('ended')
                        and time.perf_counter() - record['started'] >= 0.25), None)
                pending = until('actual installed manager collect HTTP remains pending', waiting, 10)
                before = json.loads(json.dumps(pending))
                started = time.monotonic()
                self.track(dual)
                self.role(dual, ['run', 'collect', 'approve', 'report'], 'controlled-fixture-dual')
                registered = time.monotonic()
                require(registered - started < 5 and not pending.get('ended'),
                    'Registration/catalog waited for the collect it must not wait on')
                report = self.http('/api/workflow/node/report', {'sessionId': inner['sessionId'],
                    'body': 'Fixture child progress while actual manager collect waits', 'canComplete': False, 'outputs': []}, budget=2)
                require(report['saved'] is True, 'Actual child progress was not saved')
                until('actual collect wakes from the real child report', lambda: pending.get('ended'), 5)
                require(json.loads(pending['response'])['reason'] == 'notification', 'Collect did not observe actual child progress')
                public = self.detail(child_id)
                require(public['run']['nodes']['inside']['status'] == 'running'
                    and report['report']['executionId'] == inner_execution['id'], 'Child progress changed execution/lifecycle')
                self.evidence['fixtureObservation'] = {'collectBefore': before, 'collectAfter': pending,
                    'registrationStarted': started, 'registrationReturned': registered,
                    'advanceEnded': time.monotonic(), 'child': public, 'actualInnerExecution': inner_execution, 'progressReport': report}
                self.evidence['passed'] = True
        with tempfile.TemporaryDirectory(prefix='collect-fixture-config-') as directory:
            root = Path(directory)
            selected = os.environ.get('NODE_APPROVAL_LIFECYCLE_FIXTURE_EVIDENCE')
            evidence = Path(selected).resolve() if selected else root / 'evidence'
            evidence.mkdir(parents=True, exist_ok=False)
            with NativeCompletionTransport(mechanical, evidence / 'controlled-model') as provider:
                config = root / 'model.json'
                config.write_text(json.dumps({'provider': 'owned-fixture', 'model': 'fixture-no-external-model',
                    'baseUrl': provider.base, 'apiKeyEnv': 'OWNED_COLLECT_FIXTURE_MARKER'}), encoding='utf-8')
                args = SimpleNamespace(repo=ROOT, toolkit=toolkit, backend=backend, herdr=Path(roots['herdr']) / 'herdr.exe',
                    omp_package=Path(roots['omp_package']), pi_package=Path(roots['pi_package']),
                    omp_state_extension=Path(roots['omp_state_extension']), pi_state_extension=Path(roots['pi_state_extension']),
                    model_config=config, evidence_dir=evidence, host='pi', condition='fixture', cases=[], timeout=5,
                    node=os.environ.get('PI_NODE', 'node'), bun='bun', shell_path=None)
                previous = os.environ.get('OWNED_COLLECT_FIXTURE_MARKER')
                os.environ['OWNED_COLLECT_FIXTURE_MARKER'] = 'controlled-local-fixture-only'
                try:
                    run = Fixture(args)
                    status = run.run()
                finally:
                    if previous is None:
                        os.environ.pop('OWNED_COLLECT_FIXTURE_MARKER', None)
                    else:
                        os.environ['OWNED_COLLECT_FIXTURE_MARKER'] = previous
                self.assertEqual(status, 0, run.evidence.get('verificationError'))
                self.assertFalse(run.evidence['cleanupErrors'])
                self.assertFalse(provider.failed)


@unittest.skipUnless(os.environ.get('NODE_APPROVAL_LIFECYCLE_FIXTURE_TOOLKIT'),
                     'Supply explicit owned worktree/binary/paths for the native gate regression')
class NativeGateFixture(unittest.TestCase):
    """Installed hosts must stay working before the first request for 91 seconds.

    The local provider only acknowledges inputs. It does not choose approval or
    supply UI results; this checks scheduling, not ticket 12 business coverage.
    """
    def test_omp_holds_working_session_until_explicit_release(self):
        self.run_gate('omp')

    def test_pi_holds_working_session_until_explicit_release(self):
        self.run_gate('pi')

    def run_gate(self, host):
        import time
        from types import SimpleNamespace
        from acceptance.host.completion_transport import NativeCompletionTransport
        from acceptance.host.processes import require, until
        from node_approval_lifecycle_acceptance import ApprovalLifecycleAcceptance, IDLE
        toolkit = Path(os.environ['NODE_APPROVAL_LIFECYCLE_FIXTURE_TOOLKIT']).resolve()
        backend = Path(os.environ['NODE_APPROVAL_LIFECYCLE_FIXTURE_BACKEND']).resolve()
        paths = Path(os.environ['NODE_APPROVAL_LIFECYCLE_FIXTURE_PATHS']).resolve()
        roots = json.loads(subprocess.run([str(backend.parent / 'herdr-paths.exe'), '--paths', str(paths)],
            capture_output=True, text=True, check=True, encoding='utf-8').stdout)
        def mechanical(_request):
            return {'content': 'Controlled scheduling fixture acknowledgment.', 'tool_calls': [],
                'usage': {'prompt_tokens': 1, 'completion_tokens': 1, 'total_tokens': 2}}
        class Fixture(ApprovalLifecycleAcceptance):
            def exercise_case(self):
                self.evidence.update(promises=[], coverage={}, scenario='fixture-native-predecision-gate',
                    claimsRealModelJudgment=False, scope='Actual installed host scheduling; local mechanical acknowledgment only.')
                for role in ['manager', 'worker']:
                    directory = self.root / ('zone-' + role)
                    directory.mkdir()
                    self.http('/api/zones', {'id': 'lifecycle/' + role, 'path': str(directory)})
                    self.herdr('workspace', 'create', '--cwd', str(directory), '--label', directory.name, '--no-focus')
                self.manager_task = self.task('fixture-manager', 'manager', IDLE, self.host)
                worker = self.task('fixture-worker', 'normal', IDLE, self.host)
                (self.root / 'bots/worker.json').write_text(json.dumps({'id': 'worker', 'name': 'Fixture worker',
                    'when': '', 'plan_guide': '', 'node_guide': '', 'task_id': worker}), encoding='utf-8')
                self.gate_marker = 'FIXTURE-NATIVE-GATE-' + self.host
                self.gate_config.write_text(json.dumps({'marker': self.gate_marker, 'release': str(self.gate_release),
                    'evidence': str(self.gate_evidence)}), encoding='utf-8')
                self.define_map('gate', {'work': self.node('Owned gated initial work', input=IDLE + '\n' + self.gate_marker)})
                self.run_id, self.manager = self.new_run('gate')
                execution, self.worker = self.dispatch(self.manager, 'work', self.run_id)
                held = until('installed host predecision gate', lambda: self.gate_held(self.worker), 5)
                working = self.herdr('agent', 'wait', self.worker['paneId'], '--until', 'working', '--timeout', '1000')
                started = time.monotonic()
                observation = {'started': started, 'requiredSeconds': 91, 'working': working,
                    'execution': execution, 'initialGate': held, 'samples': []}
                self.evidence['gateObservation'] = observation
                while time.monotonic() - started < 91:
                    observation['samples'].append({'at': time.monotonic(), 'gate': self.gate_held(self.worker)})
                    time.sleep(0.25)
                observation.update(ended=time.monotonic(), actualSeconds=time.monotonic() - started)
                require(not self.retry_model_requests(), 'Business request escaped before explicit release')
                self.gate_release.write_text('Explicit fixture release after complete observation.', encoding='utf-8')
                observation['historyAfterRelease'] = self.settled(self.worker)
                observation['requestsAfterRelease'] = self.retry_model_requests()
                require(observation['requestsAfterRelease'], 'Released actual host did not continue to the provider')
                observation['events'] = [json.loads(line) for line in
                    (self.gate_evidence / 'events.jsonl').read_text(encoding='utf-8').splitlines()]
                require(all(event.get('elapsedMs', 0) < 30000 for event in observation['events']),
                    'A scheduling handler exceeded the existing host budget')
                self.evidence['passed'] = True
        with tempfile.TemporaryDirectory(prefix='native-gate-config-') as directory:
            root = Path(directory)
            selected = os.environ.get('NODE_APPROVAL_LIFECYCLE_FIXTURE_EVIDENCE')
            evidence = Path(selected).resolve() / ('gate-' + host) if selected else root / 'evidence'
            evidence.mkdir(parents=True, exist_ok=False)
            with NativeCompletionTransport(mechanical, evidence / 'controlled-model') as provider:
                config = root / 'model.json'
                config.write_text(json.dumps({'provider': 'owned-fixture', 'model': 'fixture-no-external-model',
                    'baseUrl': provider.base, 'apiKeyEnv': 'OWNED_GATE_FIXTURE_MARKER'}), encoding='utf-8')
                args = SimpleNamespace(repo=ROOT, toolkit=toolkit, backend=backend, herdr=Path(roots['herdr']) / 'herdr.exe',
                    omp_package=Path(roots['omp_package']), pi_package=Path(roots['pi_package']),
                    omp_state_extension=Path(roots['omp_state_extension']), pi_state_extension=Path(roots['pi_state_extension']),
                    model_config=config, evidence_dir=evidence, host=host, condition='fixture', cases=[], timeout=20,
                    node=os.environ.get('PI_NODE', 'node'), bun='bun', shell_path=None)
                previous = os.environ.get('OWNED_GATE_FIXTURE_MARKER')
                os.environ['OWNED_GATE_FIXTURE_MARKER'] = 'controlled-local-fixture-only'
                try:
                    run = Fixture(args)
                    status = run.run()
                finally:
                    if previous is None:
                        os.environ.pop('OWNED_GATE_FIXTURE_MARKER', None)
                    else:
                        os.environ['OWNED_GATE_FIXTURE_MARKER'] = previous
                self.assertEqual(status, 0, run.evidence.get('verificationError'))
                self.assertFalse(run.evidence['cleanupErrors'])
                self.assertFalse(provider.failed)


if __name__ == '__main__':
    unittest.main()
