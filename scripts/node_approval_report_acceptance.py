"""Ticket 10 oracles at installed native Chat/UI, public HTTP and external CLI.

Independent expectations are literal approval facts, the exact confirmation text,
and four named report classes. A report-driven approval, approval notification,
swallowed storage failure or serialized 'concurrent' call must fail, not pass by
construction. The provider scripts choices only: all product mechanisms stay real.
"""
import copy
import ctypes
import hashlib
import http.server
import json
from pathlib import Path
import re
import shutil
import threading
import time
import traceback
import urllib.error
import urllib.request

from acceptance.host.completion_transport import NativeCompletionTransport
from acceptance.host.native import NativeAcceptance
from acceptance.host.processes import require, until

QUESTION = '当前会话的成果是否已达到预期，可以交接并推进后续流程？'
CLAIMS = {
    'P2': ['empty-auto', 'empty-manual'], 'P4': ['empty-auto'],
    'P5': ['empty-manual', 'reports-manual'], 'P6': ['empty-manual'],
    'P9': ['empty-auto', 'empty-manual'], 'P12': ['fault-auto', 'fault-manual'],
    'P13': ['concurrent'], 'P17': ['reports-auto', 'reports-manual'],
    'P18': ['empty-auto', 'empty-manual', 'reports-auto', 'reports-manual'],
    'P19': ['empty-auto', 'empty-manual', 'reports-auto', 'reports-manual', 'concurrent'],
    'P23': ['empty-auto', 'empty-manual', 'reports-auto', 'reports-manual', 'concurrent'],
    'P37': ['chief'], 'P38': ['reports-auto', 'reports-manual'],
}


def messages(history, start=0):
    return [entry['message'] for entry in history['branch'][start:] if entry.get('type') == 'message']


def tool_calls(history, start=0):
    return [part for message in messages(history, start) if message.get('role') == 'assistant'
        for part in message.get('content', []) if isinstance(part, dict) and part.get('type') == 'toolCall']


class ApprovalWire:
    """Transparent HTTP recorder; an armed two-arrival gate proves actual overlap.

    Only scheduling of test-owned requests changes. Both responses are obtained
    from the actual backend; no response, approval fact or persistence is faked.
    """
    def __init__(self, upstream, directory):
        self.directory = directory
        directory.mkdir()
        self.records = []
        self.lock = threading.Lock()
        self.barrier = None
        self.arrivals = []
        owner = self
        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *_):
                pass
            def do_GET(self):
                self.forward()
            def do_POST(self):
                self.forward()
            def forward(self):
                raw = self.rfile.read(int(self.headers.get('Content-Length', '0')))
                with owner.lock:
                    record = {'index': len(owner.records), 'method': self.command, 'path': self.path,
                        'request': raw.decode(), 'started': time.monotonic()}
                    owner.records.append(record)
                    gate = owner.barrier if self.path == '/api/workflow/node/approve' else None
                    if gate:
                        owner.arrivals.append(record)
                        if len(owner.arrivals) == 2:
                            gate.set()
                try:
                    if gate:
                        require(gate.wait(5), 'Two native approval requests did not overlap within 5s; serial repetition is not concurrency')
                        record['gateReleased'] = time.monotonic()
                    request = urllib.request.Request(upstream + self.path, data=raw if raw else None,
                        method=self.command, headers={key: value for key, value in self.headers.items()
                        if key.lower() not in ['host', 'connection', 'content-length', 'accept-encoding']})
                    try:
                        response = urllib.request.urlopen(request, timeout=100)
                    except urllib.error.HTTPError as error:
                        response = error
                    with response:
                        payload = response.read()
                        record.update(status=response.status, response=payload.decode('utf-8', errors='replace'))
                        self.send_response(response.status)
                        self.send_header('Content-Type', response.headers.get('Content-Type', 'application/json'))
                        self.send_header('Content-Length', str(len(payload)))
                        self.end_headers()
                        self.wfile.write(payload)
                except BaseException:
                    record['error'] = traceback.format_exc()
                    self.send_error(502, 'Actual transport failed; original evidence retained')
                finally:
                    record['ended'] = time.monotonic()
                    (directory / (str(record['index']) + '.json')).write_text(json.dumps(record, ensure_ascii=False, indent=2), encoding='utf-8')
        self.server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.base = 'http://127.0.0.1:' + str(self.server.server_port)

    def close(self):
        if self.barrier:
            self.barrier.set()
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(15)
        require(not self.thread.is_alive(), 'Approval HTTP observer survived cleanup')
        (self.directory / 'index.json').write_text(json.dumps(self.records, ensure_ascii=False, indent=2), encoding='utf-8')


class NodeApprovalReportAcceptance(NativeAcceptance):
    def __init__(self, args):
        super().__init__(args)
        self.evidence.update(promise=None, promises=list(CLAIMS), scenario='node-approval-report',
            scope='Controlled provider; real installed hosts, visible native UI, production backend/storage and installed Herdr; no real-model business judgment',
            coverage={key: {'verified': False} for key in CLAIMS}, cases=[],
            scenarioSource={'path': str(Path(__file__).resolve()), 'sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()},
            budgets={'requestSeconds': 30, 'confirmationSeconds': 20, 'choiceObservationSeconds': 5,
                'negativeObservationSeconds': 46, 'reason': '46s exceeds 30s HTTP and 10s online + 30s retention + 5s maintenance; no product budget is changed',
                'nativeRoundSeconds': args.timeout, 'concurrentArrivalSeconds': 5})
        self.actions = {}
        self.consumed = set()
        self.counter = 0
        self.provider = None
        self.owned_chats = []
        self.confirmations = self.root / 'confirmations'
        self.confirmations.mkdir()
        self.env.update(OMP_CODING_AGENT_DIR=str(self.root / 'agent'), XDG_DATA_HOME=str(self.root / 'state'),
            CHIEF_MAP_CONFIRMATION_EVIDENCE_DIR=str(self.confirmations))
        self.evidence['isolation'].update({key: self.env[key] for key in ['OMP_CODING_AGENT_DIR', 'XDG_DATA_HOME', 'CHIEF_MAP_CONFIRMATION_EVIDENCE_DIR']})

    def configure_model(self):
        def decision(request):
            candidates = []
            for message in request.get('messages', []):
                if message.get('role') == 'user':
                    candidates.extend(re.findall(r'TICKET10:([a-z0-9-]+)', json.dumps(message.get('content', ''), ensure_ascii=False)))
            label = candidates[-1] if candidates else 'idle'
            actions = self.actions.get(label, []) if label not in self.consumed else []
            available = {tool.get('function', {}).get('name') for tool in request.get('tools', [])}
            if actions and not all(name in available for name, _ in actions):
                return {'content': 'Controlled auxiliary request; no business action', 'tool_calls': []}
            self.consumed.add(label)
            return {'content': '' if actions else 'TICKET10-DONE:' + label,
                'tool_calls': [{'name': name, 'arguments': params} for name, params in actions]}
        self.provider = NativeCompletionTransport(decision, self.root / 'controlled-provider')
        self.provider.__enter__()
        self.provider_model = 'ticket10/mechanical'
        model = {'providers': {'ticket10': {'baseUrl': self.provider.base, 'api': 'openai-completions',
            'apiKey': 'owned-controlled-provider-not-a-credential', 'models': [{'id': 'mechanical', 'name': 'Controlled mechanical choices',
                'reasoning': False, 'input': ['text'], 'contextWindow': 128000, 'maxTokens': 4096,
                'cost': {'input': 0, 'output': 0, 'cacheRead': 0, 'cacheWrite': 0}}]}}}
        agent = self.root / 'agent'
        for name in ['models.json', 'models.yml']:
            (agent / name).write_text(json.dumps(model), encoding='utf-8')
        (agent / 'settings.json').write_text(json.dumps({'sessionDir': str(agent / 'sessions')}), encoding='utf-8')
        (agent / 'config.yml').write_text('autoResume: false\nstartup:\n  setupWizard: false\n  showSplash: false\n  checkUpdate: false\nmarketplace:\n  autoUpdate: off\n', encoding='utf-8')
        self.evidence['provider'] = {'mode': 'controlled', 'requests': str(self.provider.directory), 'claimsRealModelJudgment': False}

    def role_catalog(self, toolkit):
        catalog = super().role_catalog(toolkit)
        for role in ['chief-chat', 'chief-report']:
            catalog += '\n[[extensions]]\nid = ' + json.dumps(self.host + '-' + role) + '\nlabel = ' + json.dumps(role)
            catalog += '\nagent = ' + json.dumps(self.host) + '\nkind = "fixed"\nentrypoint = ' + json.dumps(str(toolkit / 'entrypoints' / (self.host + '-auto') / (role + '.ts'))) + '\n'
        return catalog

    def start(self, argv, label):
        if label == 'herdr':
            path = self.root / 'herdr.toml'
            path.write_text(path.read_text(encoding='utf-8') + '\n[server]\nheadless_cols = 240\nheadless_rows = 240\n', encoding='utf-8')
        return super().start(argv, label)

    def start_backend(self, role_config):
        proxy = self.root / 'external-cli'
        proxy.mkdir()
        executable = proxy / 'recovery_herdr_proxy.exe'
        source = self.args.backend.parent / 'examples/recovery_herdr_proxy.exe'
        require(source.is_file(), 'Build the existing recovery_herdr_proxy example for transparent external CLI recording')
        shutil.copy2(source, executable)
        (proxy / 'recovery-proxy.json').write_text(json.dumps({'herdr': str(self.args.herdr)}), encoding='utf-8')
        observer = self.root / 'confirmation-observer.mjs'
        shutil.copy2(self.args.repo / 'scripts/chief-ui-observer.mjs', observer)
        with (self.root / 'options.toml').open('a', encoding='utf-8') as stream:
            stream.write('\n[[extensions]]\nid = "ticket10-ui-observer"\nlabel = "Pass-through native UI observation"\nagent = ' + json.dumps(self.host)
                + '\nkind = "fixed"\nentrypoint = ' + json.dumps(str(observer)) + '\n')
        real = self.args.herdr
        try:
            self.args.herdr = executable
            super().start_backend(role_config)
        finally:
            self.args.herdr = real
        self.evidence['externalRecorder'] = {'source': str(self.args.repo / 'backend/examples/recovery_herdr_proxy.rs'),
            'sha256': hashlib.sha256(executable.read_bytes()).hexdigest(), 'faultEnabled': False}

    def prepare(self):
        super().prepare()
        self.backend_wire.close()
        self.backend_wire = ApprovalWire(self.base, self.root / 'approval-wire')
        (self.root / 'toolkit/extensions/workflow-chat/config.yaml').write_text(
            f'endpoint: {self.backend_wire.base}\nrequestTimeoutMs: 30000\nreportConfirmationTimeoutMs: 20000\ncollectTimeoutMs: 5460000\n', encoding='utf-8')
        for role in ['manager', 'node', 'chief']:
            directory = self.root / ('zone-' + role)
            directory.mkdir()
            self.http('/api/zones', {'id': 'ticket10/' + role, 'path': str(directory)})
            self.herdr('workspace', 'create', '--cwd', str(directory), '--label', directory.name, '--no-focus')
        self.manager_task = self.task('Mechanical manager', 'manager', 'Acknowledge inputs and notifications only. No implicit workflow operations.', self.host)
        self.worker_task = self.task('Mechanical worker', 'normal', 'No files or artifacts are needed; explicit tool choices are supplied by the controlled provider.', self.host)
        self.http('/api/scheduler/bots/save', {'id': 'ticket10-worker', 'revision': None,
            'content': json.dumps({'name': 'Mechanical worker', 'when': 'Ticket-owned mechanical approval/report acceptance only',
                'plan_guide': '', 'node_guide': '', 'task_id': self.worker_task})})

    def next_action(self, actions):
        self.counter += 1
        label = 'action-' + str(self.counter)
        self.actions[label] = actions
        return label

    def track(self, chat):
        self.owned_chats.append(chat)
        self.chat_hosts[chat['sessionId']] = self.host
        self.observe_processes()
        return chat

    def settled(self, chat, label, start=0):
        def ready():
            history = self.native(chat)
            if not history:
                return None
            replies = [m for m in messages(history, start) if m.get('role') == 'assistant']
            return history if replies and replies[-1].get('stopReason') == 'stop' and 'TICKET10-DONE:' + label in json.dumps(replies[-1]) else None
        return until('native completed controlled round ' + label, ready, self.args.timeout)

    def begin(self, chat, actions, text='Explicit mechanical scenario action'):
        old = self.native(chat)
        start = len(old['branch']) if old else 0
        label = self.next_action(actions)
        self.herdr('agent', 'wait', chat['paneId'], '--until', 'idle', '--until', 'done', '--timeout', '10000')
        self.evidence.setdefault('userInputs', []).append({'sessionId': chat['sessionId'], 'label': label, 'text': text, 'at': time.monotonic()})
        self.command([self.args.herdr, 'agent', 'prompt', chat['paneId'], text + '\nTICKET10:' + label,
            '--wait', '--until', 'working', '--until', 'blocked', '--timeout', '10000'], seconds=15)
        return label, start

    def observer_requests(self, chat):
        return [json.loads(path.read_text(encoding='utf-8')) for path in sorted(self.confirmations.glob('*.request.json'))
            if json.loads(path.read_text(encoding='utf-8')).get('sessionId') == chat['sessionId']]

    def yes(self, chat, previous, exact=None):
        def visible():
            requests = self.observer_requests(chat)
            if len(requests) != previous + 1:
                return None
            request = requests[-1]
            screen = self.command([self.args.herdr, 'pane', 'read', chat['paneId'], '--source', 'visible', '--format', 'text'])
            if request['title'] not in screen or 'Yes' not in screen or 'No' not in screen:
                return None
            if exact is not None:
                require(request['message'] == exact, ('Native confirmation changed or appended a payload', request))
                require(exact in screen, ('Exact approval question not visible', screen))
            return {'request': request, 'screen': screen}
        observed = until('actual native confirmation visible', visible, self.args.timeout)
        def selected(screen):
            return any(arrow + ' Yes' in screen for arrow in ['→', '❯'])
        if not selected(observed['screen']):
            self.herdr('agent', 'send-keys', chat['paneId'], 'up')
            observed['screen'] = until('actual native Yes visibly selected', lambda: (screen if selected(screen) else None)
                if (screen := self.command([self.args.herdr, 'pane', 'read', chat['paneId'], '--source', 'visible', '--format', 'text'])) else None, 5)
        require(selected(observed['screen']), 'Native Yes was not visibly selected')
        observed['selectedAt'] = time.monotonic()
        self.herdr('agent', 'send-keys', chat['paneId'], 'enter')
        path = self.confirmations / (observed['request']['id'] + '.response.json')
        response = until('actual native confirmation return', lambda: json.loads(path.read_text(encoding='utf-8')) if path.exists() else None, 5)
        require(response.get('result') is True, ('Native confirmation did not return true', response))
        observed['response'] = response
        self.case.setdefault('nativeYes', []).append(observed)
        return observed

    def invoke(self, chat, name, params, manual=False, text='Explicit mechanical native call'):
        previous = len(self.observer_requests(chat))
        label, start = self.begin(chat, [(name, params)], text)
        if manual:
            self.yes(chat, previous, QUESTION if name == 'approve' else None)
        history = self.settled(chat, label, start)
        calls = tool_calls(history, start)
        require(len(calls) == 1 and calls[0]['name'] == name and calls[0]['arguments'] == params, ('Native tool call/arguments mismatch', calls))
        results = [m for m in messages(history, start) if m.get('role') == 'toolResult']
        require(len(results) == 1, ('Missing actual native tool result', results))
        require(len(self.observer_requests(chat)) == previous + int(manual), 'Unexpected native report/automatic-approval confirmation')
        self.case.setdefault('nativeCalls', []).append({'name': name, 'parameters': params, 'history': history, 'result': results[0]})
        return results[0]

    def external(self):
        directory = self.root / 'external-cli/calls'
        return [{'file': path.name, **json.loads(path.read_text(encoding='utf-8'))} for path in sorted(directory.glob('*.json'))]

    def setup_run(self, auto):
        label = self.next_action([])
        name = 'case-' + str(self.counter) + '.json'
        value = {'title': 'Ticket 10 owned approval execution', 'description': '', 'nodes': {
            'work': {'title': 'Ordinary work', 'bot_id': 'ticket10-worker', 'zone': 'ticket10/node',
                'input': 'Legal work requires no output, report or file. TICKET10:' + label, 'blocked_by': [],
                'auto_approved': auto, 'auto_close': False, 'output_rule': []},
            'later': {'title': 'Never automatically dispatched', 'bot_id': 'ticket10-worker', 'zone': 'ticket10/node',
                'input': 'Do not start implicitly', 'blocked_by': ['work'], 'auto_approved': auto, 'output_rule': []}}}
        (self.root / 'maps' / name).write_text(json.dumps(value), encoding='utf-8')
        self.http('/api/scheduler/maps/' + name)
        manager_label = self.next_action([])
        started = self.http('/api/scheduler/maps/' + name + '/runs', {'managerTaskId': self.manager_task,
            'zone': 'ticket10/manager', 'userInput': 'Mechanical acceptance; do not advance. TICKET10:' + manager_label}, 201)
        self.run_id = started['id']
        self.manager = self.track(started['run']['manager'])
        self.settled(self.manager, manager_label)
        self.http('/api/workflow/manager/prepare_inputs', {'sessionId': self.manager['sessionId'], 'fileInputs': [], 'confirmed': True, 'reason': 'No file inputs in this literal scenario'})
        dispatched = self.http('/api/workflow/manager/dispatch', {'sessionId': self.manager['sessionId'], 'nodes': ['work']})
        self.worker = self.track(dispatched['snapshot']['nodes']['work']['execution']['chat'])
        self.settled(self.worker, label)
        detail = self.detail(self.run_id)
        execution = detail['run']['nodes']['work']['execution']
        require(execution['approved'] is False and execution['approvalSource'] is None and execution['autoApproved'] is auto, detail)
        require(detail['reports'] == [] and detail['run']['nodes']['work'].get('outputs', []) in [[], {}], detail)
        self.case.update(runId=self.run_id, autoApproved=auto, before=detail, externalBefore=self.external())
        return detail

    def manager_input(self):
        history = self.native(self.manager)
        return [m for m in messages(history) if m.get('role') == 'user'] if history else []

    def assert_approved_only(self, before, after, source):
        expected = copy.deepcopy(before['run'])
        expected['nodes']['work']['execution'].update(approved=True, approvalSource=source)
        require(after['run'] == expected, ('Approval changed facts beyond approval', before, after))
        require(after['reports'] == before['reports'] and after['pendingReports'] == before['pendingReports'], 'Approval created or altered a report')

    def quiet(self, expected, external, manager_input):
        start = time.monotonic()
        samples = 0
        wire_start = len(self.backend_wire.records)
        while time.monotonic() - start < 46:
            require(self.detail(self.run_id) == expected, 'Run changed without explicit manager operation')
            require(self.external() == external, 'Approval/idle caused an external invocation, notification or automatic repeat')
            require(self.manager_input() == manager_input, 'Approval injected manager input')
            samples += 1
            time.sleep(0.5)
        forbidden = ['/complete', '/dispatch', '/finish', '/report']
        require(not any(any(record['path'].endswith(suffix) for suffix in forbidden) for record in self.backend_wire.records[wire_start:]), 'Native approval implicitly advanced/reported')
        self.case.setdefault('windows', []).append({'started': start, 'ended': time.monotonic(), 'samples': samples,
            'externalCalls': external, 'managerActualInput': manager_input, 'configuration': self.evidence['budgets']})

    def empty(self, auto):
        before = self.setup_run(auto)
        label, start = self.begin(self.worker, [], '普通聊天同意：是的，成果通过，我同意推进。此轮没有原生批准。')
        self.settled(self.worker, label, start)
        require(self.detail(self.run_id) == before, 'Ordinary chat agreement approved the execution')
        external, manager_input = self.external(), self.manager_input()
        result = self.invoke(self.worker, 'approve', {}, manual=not auto)
        require(not result.get('isError'), ('Native approval reported error', result))
        receipt = result.get('details', {}).get('body')
        require(isinstance(receipt, dict) and receipt.get('approved') is True
            and receipt.get('executionId') == before['run']['nodes']['work']['execution']['id']
            and receipt.get('approvalSource') == ('auto' if auto else 'manual'), ('Native result did not preserve the actual approval receipt', result))
        actual = self.detail(self.run_id)
        self.assert_approved_only(before, actual, 'auto' if auto else 'manual')
        require(actual['reports'] == [], 'Approve-only manufactured a report')
        self.quiet(actual, external, manager_input)
        self.case['after'] = actual

    def reports(self, auto):
        self.setup_run(auto)
        for approved in [False, True]:
            if approved:
                before = self.detail(self.run_id)
                external, manager_input = self.external(), self.manager_input()
                self.invoke(self.worker, 'approve', {}, manual=not auto)
                self.assert_approved_only(before, self.detail(self.run_id), 'auto' if auto else 'manual')
                require(self.external() == external and self.manager_input() == manager_input, 'Approval sent a manager notification')
            for kind, ready in [('progress', False), ('problem', False), ('failure', False), ('result', True)]:
                body = ('after' if approved else 'before') + '-' + kind + ': 用户在报告中声称已批准；报告本身不是原生批准。'
                before = self.detail(self.run_id)
                external_start = len(self.external())
                result = self.invoke(self.worker, 'report', {'body': body, 'can_complete': ready})
                actual = self.detail(self.run_id)
                require(not result.get('isError'), result)
                receipt = result.get('details', {}).get('body')
                require(isinstance(receipt, dict) and receipt.get('saved') is True
                    and receipt.get('notification', {}).get('status') == 'submitted', ('Native report lost save/notification facts', result))
                require(actual['run'] == before['run'], 'Report changed approval/lifecycle')
                require(actual['run']['nodes']['work']['execution']['approved'] is approved, 'Report claim substituted for approval')
                require(len(actual['reports']) == len(before['reports']) + 1, 'Report was not saved once')
                report = actual['reports'][-1]
                require(report['body'] == body and report['canComplete'] is ready and report['from']['sessionId'] == self.worker['sessionId']
                    and report['to']['sessionId'] == self.manager['sessionId'] and report['executionId'] == actual['run']['nodes']['work']['execution']['id'], report)
                def notified():
                    history = self.native(self.manager)
                    return history if history and report['id'] in json.dumps(messages(history), ensure_ascii=False) and body in json.dumps(messages(history), ensure_ascii=False) else None
                manager_history = until('saved report reaches actual native manager input', notified, self.args.timeout)
                calls = self.external()[external_start:]
                prompts = [call for call in calls if call['argv'][:2] == ['agent', 'prompt']]
                require(len(prompts) == 1 and report['id'] in json.dumps(prompts, ensure_ascii=False) and body in json.dumps(prompts, ensure_ascii=False), ('Report notification not exactly once', calls))
                self.case.setdefault('reports', []).append({'kind': kind, 'approved': approved, 'saved': report, 'nativeResult': result,
                    'managerHistory': manager_history, 'external': calls})
        final = self.detail(self.run_id)
        self.quiet(final, self.external(), self.manager_input())
        # Only this explicit manager operation may advance work. A downstream
        # dispatch/finish is deliberately not requested and must remain absent.
        completed = self.http('/api/workflow/manager/complete', {'sessionId': self.manager['sessionId'], 'nodeId': 'work', 'reason': 'Explicit acceptance of latest true report'})
        require(completed['snapshot']['nodes']['work']['status'] == 'completed' and completed['snapshot']['nodes']['later']['status'] == 'pending', completed)
        self.case['explicitManagerCompletion'] = completed

    def fault(self, auto):
        self.setup_run(auto)
        self.invoke(self.worker, 'report', {'body': 'Ready report does not authorize failed approval', 'can_complete': True})
        before = self.detail(self.run_id)
        # Same real deny-delete fault as ticket 02, discovered by owned record
        # content rather than assuming a private run.json or temporary layout.
        execution_id = before['run']['nodes']['work']['execution']['id']
        records = []
        for path in (self.root / 'data').rglob('*.json'):
            try:
                record = json.loads(path.read_text(encoding='utf-8'))
                if isinstance(record, dict) and record.get('id') == self.run_id and record.get('nodes', {}).get('work', {}).get('execution', {}).get('id') == execution_id:
                    records.append(path)
            except (OSError, ValueError):
                pass
        require(len(records) == 1, ('Cannot identify the owned execution record for actual storage fault', records))
        kernel = ctypes.WinDLL('kernel32', use_last_error=True)
        kernel.CreateFileW.argtypes = [ctypes.c_wchar_p, ctypes.c_ulong, ctypes.c_ulong, ctypes.c_void_p, ctypes.c_ulong, ctypes.c_ulong, ctypes.c_void_p]
        kernel.CreateFileW.restype = ctypes.c_void_p
        kernel.CloseHandle.argtypes = [ctypes.c_void_p]
        handle = kernel.CreateFileW(str(records[0]), 0x80000000, 3, None, 3, 0x80, None)
        require(handle not in [None, ctypes.c_void_p(-1).value], ('Cannot acquire owned deny-delete handle', ctypes.get_last_error()))
        try:
            external = self.external()
            result = self.invoke(self.worker, 'approve', {}, manual=not auto)
            failures = [record for record in self.backend_wire.records if record['path'] == '/api/workflow/node/approve']
            actual = failures[-1]
            require(actual.get('status') == 500, ('Real storage fault did not reject approval', actual))
            error = json.loads(actual['response'])
            require(error.get('code') == 'approval-save-failed' and 'os error 5' in error.get('message', ''), error)
            require(result.get('isError') is True and error['message'] in json.dumps(result, ensure_ascii=False), ('Native tool lost original persistence error or claimed success', result, error))
            require(self.detail(self.run_id) == before, 'Failed persistence published memory-only approval')
            denied = self.http('/api/workflow/manager/complete', {'sessionId': self.manager['sessionId'], 'nodeId': 'work', 'reason': 'A failed approval cannot authorize complete'}, 409)
            require(denied['code'] == 'node-not-approved', denied)
            require(self.external() == external, 'Failed approval produced external effects')
            self.case.update(fault={'kind': 'Real Windows deny FILE_SHARE_DELETE handle; backend response is not substituted',
                'ownedRecord': str(records[0]), 'backendError': error, 'nativeResult': result, 'completeRefusal': denied}, after=self.detail(self.run_id))
        finally:
            require(kernel.CloseHandle(handle), 'Could not release owned storage fault handle')

    def concurrent(self):
        before = self.setup_run(True)
        external, manager_input = self.external(), self.manager_input()
        self.native(self.worker)
        histories = []
        for path in (self.root / 'histories').glob('*.jsonl'):
            with path.open(encoding='utf-8') as stream:
                first = json.loads(stream.readline())
                if first.get('type') == 'title':
                    first = json.loads(stream.readline())
            if first.get('type') == 'session' and first.get('id') == self.worker['sessionId']:
                histories.append(path)
        require(len(histories) == 1, ('Missing owned native history for public switch_session', histories))
        second_label = self.next_action([('approve', {})])
        invocation = self.root / 'concurrent-invocation.json'
        output = self.root / 'concurrent-native'
        release = self.root / 'concurrent-start'
        invocation.write_text(json.dumps({'host': self.host, 'agent': str(self.root / 'agent'),
            'deployment': str(self.root / 'toolkit'), 'session': str(histories[0]),
            'sessionId': self.worker['sessionId'], 'label': second_label, 'evidence': str(output), 'start': str(release)}), encoding='utf-8')
        process = self.start([self.args.bun, self.args.toolkit / 'scripts/node-approval-concurrent.ts', invocation], 'concurrent-native')
        def recovered():
            require(process.child.poll() is None, 'Concurrent installed host exited before readiness; see raw logs')
            path = output / 'ready.json'
            return json.loads(path.read_text(encoding='utf-8')) if path.exists() else None
        native = until('second actual native process recovers the same public Session identity', recovered, self.args.timeout)
        require(native['nativeId'] == self.worker['sessionId'], native)
        self.observe_processes()
        self.backend_wire.arrivals = []
        self.backend_wire.barrier = threading.Event()
        try:
            label, start = self.begin(self.worker, [('approve', {})], 'First simultaneous native approval for this execution')
            release.write_text('release second actual native caller', encoding='utf-8')
            history = self.settled(self.worker, label, start)
            require(process.child.wait(timeout=self.args.timeout) == 0, 'Concurrent native process failed; see raw result')
            second = json.loads((output / 'result.json').read_text(encoding='utf-8'))
            require(second['passed'] and not second['cleanupErrors'], second)
            calls = tool_calls(history, start)
            require(len(calls) == 1 and calls[0]['name'] == 'approve' and calls[0]['arguments'] == {}, ('Missing first actual native approve call', calls))
            arrivals = self.backend_wire.arrivals
            require(len(arrivals) == 2 and all(record.get('status') == 200 for record in arrivals)
                and max(record['started'] for record in arrivals) < min(record['gateReleased'] for record in arrivals),
                ('Native approvals were not concurrently in flight', arrivals))
            receipts = [json.loads(record['response']) for record in arrivals]
            require(sorted(receipt['changed'] for receipt in receipts) == [False, True], receipts)
            require(all(receipt['executionId'] == before['run']['nodes']['work']['execution']['id'] and receipt['approved'] is True and receipt['approvalSource'] == 'auto' for receipt in receipts), receipts)
            self.case['concurrentEvidence'] = {'actualNativeHistory': history, 'secondNativeCaller': second,
                'arrivals': copy.deepcopy(arrivals), 'receipts': receipts}
        finally:
            self.backend_wire.barrier = None
        approved = self.detail(self.run_id)
        self.assert_approved_only(before, approved, 'auto')
        self.invoke(self.worker, 'approve', {})
        require(json.loads([record for record in self.backend_wire.records if record['path'] == '/api/workflow/node/approve'][-1]['response'])['changed'] is False, 'Repeated native call repeated approval effects')
        self.quiet(approved, external, manager_input)

    def creation(self, started):
        def ready():
            state = self.http('/api/chats/' + started['id'])
            require(state['stage'] != 'failed', state)
            return state if state['stage'] == 'succeeded' else None
        return until('actual owned native Chat launch', ready, self.args.timeout)

    def chief(self):
        chief_task = self.task('Mechanical chief confirmation', 'chief', '', self.host)
        label = self.next_action([])
        preview = self.http('/api/chats/preview', {'zone': 'ticket10/chief', 'taskId': chief_task})
        started = self.http('/api/chats', {'zone': 'ticket10/chief', 'taskId': chief_task,
            'chatId': preview['chatId'], 'input': 'No business execution; confirmation regression. TICKET10:' + label}, 202)
        chief = self.creation(started)
        current = self.http('/api/chief')['current']
        require(current and current.get('sessionId'), 'Actual chief did not acquire authority')
        chief = self.track({**chief, 'sessionId': current['sessionId']})
        self.settled(chief, label)
        delegated_label = self.next_action([])
        params = {'botId': 'ticket10-worker', 'zoneId': 'ticket10/node', 'input': 'Mechanical chief boundary; no files. TICKET10:' + delegated_label, 'materials': []}
        self.invoke(chief, 'chief_delegate', params, manual=True)
        delegates = [record for record in self.backend_wire.records if record['path'] == '/api/chief/delegations']
        require(len(delegates) == 1 and delegates[0].get('status') == 202, delegates)
        created = json.loads(delegates[0]['response'])
        def delegated_ready():
            record = self.http('/api/chief/delegations/' + created['id'])
            require(record['creation']['stage'] != 'failed', record)
            return record if record.get('sessionId') and record['creation']['stage'] == 'succeeded' else None
        record = until('public delegated sender identity', delegated_ready, self.args.timeout)
        sender = self.track({**record['creation'], 'sessionId': record['sessionId']})
        self.settled(sender, delegated_label)
        body = {'background': 'Ticket 10 chief regression only', 'execution': 'Actual chief-delegated standalone Chat',
            'result': 'success', 'summary': 'No business capability was executed', 'deliverables': [], 'pendingDecisions': []}
        self.invoke(sender, 'chief_report', {'body': body}, manual=True)
        reports = self.http('/api/chief/reports')
        require(body in [report['body'] for report in reports['reports']], ('Chief report not publicly saved', reports))
        dialogs = self.case['nativeYes']
        require(len(dialogs) == 2 and all(dialog['request']['message'] != QUESTION for dialog in dialogs), 'Chief confirmations were replaced by node approval')
        self.case['chiefPublicReports'] = reports

    def exercise(self):
        operations = [('empty-auto', lambda: self.empty(True)), ('empty-manual', lambda: self.empty(False)),
            ('reports-auto', lambda: self.reports(True)), ('reports-manual', lambda: self.reports(False)),
            ('fault-auto', lambda: self.fault(True)), ('fault-manual', lambda: self.fault(False)),
            ('concurrent', self.concurrent), ('chief', self.chief)]
        for name, operation in operations:
            self.case = {'name': name, 'verified': False, 'started': time.monotonic()}
            self.evidence['cases'].append(self.case)
            first_chat = len(self.owned_chats)
            try:
                operation()
                self.case['verified'] = True
            except BaseException:
                self.case['error'] = traceback.format_exc()
                for role in ['worker', 'manager']:
                    chat = getattr(self, role, None)
                    if chat:
                        try:
                            self.case[role + 'FailureTerminal'] = self.command([self.args.herdr, 'pane', 'read', chat['paneId'], '--source', 'visible', '--format', 'text'])
                        except BaseException:
                            self.case[role + 'ObservationError'] = traceback.format_exc()
            for chat in self.owned_chats[first_chat:]:
                try:
                    self.native(chat)
                    self.herdr('pane', 'close', chat['paneId'])
                except BaseException:
                    self.evidence['cleanupErrors'].append(traceback.format_exc())
                    self.case['verified'] = False
            self.case['ended'] = time.monotonic()
        indexed = {case['name']: case for case in self.evidence['cases']}
        for promise, names in CLAIMS.items():
            self.evidence['coverage'][promise] = {'verified': all(indexed[name]['verified'] for name in names), 'cases': names,
                'scope': 'S2 mechanical native behavior; no real-model business judgment'}
        self.evidence['passed'] = all(item['verified'] for item in self.evidence['coverage'].values())

    def cleanup_extra(self):
        errors = []
        try:
            super().cleanup_extra()
        except BaseException:
            errors.append(traceback.format_exc())
        if self.provider:
            try:
                self.provider.__exit__(None, None, None)
            except BaseException:
                errors.append(traceback.format_exc())
        require(not errors, errors)
