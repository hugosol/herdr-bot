"""Owned S2 cases for P5/non-Yes, P7, P8 and P38/native-failures.

Design: No, Esc, real 20-second timeout or real headless UI absence must leave
approved=false and running. A plausible violation retries approve automatically,
marks the execution failed, or still gates report behind the old send dialog.
Observe actual native requests/returns, HTTP and public Run state continuously;
then the installed host edits bytes and reports before a distinct user approval.
An explicit controlled provider may select tools, never confirmation results;
that path proves native mechanics, not real model business selection.
"""
import hashlib
import json
from pathlib import Path
import shutil
import time
import traceback

from acceptance.host.native import NativeAcceptance
from acceptance.host.processes import Process, require, until
from acceptance.host.completion_transport import CompletionTransport, NativeCompletionTransport
from acceptance.host.wire import WireProxy
from acceptance.host.model import message_text

QUESTION = '当前会话的成果是否已达到预期，可以交接并推进后续流程？'
CONTINUED = b'CONTINUED-MATERIAL-731\n'
REPORT = 'P7-CONTINUED-731'


def messages(history, start=0):
    return [entry['message'] for entry in history['branch'][start:] if entry.get('type') == 'message']


def calls(history, name=None, start=0):
    return [part for message in messages(history, start) if message.get('role') == 'assistant'
            for part in message.get('content', []) if isinstance(part, dict) and part.get('type') == 'toolCall'
            and (name is None or part.get('name') == name)]


class ApprovalFailuresAcceptance(NativeAcceptance):
    def __init__(self, args):
        super().__init__(args)
        self.condition = args.condition
        self.env.update(OMP_CODING_AGENT_DIR=str(self.root / 'agent'), XDG_DATA_HOME=str(self.root / 'state'))
        self.confirmations = self.root / 'native-confirmations'
        self.confirmations.mkdir()
        self.env['CHIEF_MAP_CONFIRMATION_EVIDENCE_DIR'] = str(self.confirmations)
        self.evidence.update(promise=None, promises=['P5/S2/non-Yes', 'P7', 'P8', 'P38/native-failures'],
            scenario='node-approval-' + self.condition, coverage={'verified': False},
            budgets={'confirmationSeconds': 20, 'requestSeconds': 30, 'onlineSeconds': 10,
                'retentionSeconds': 30, 'maintenanceSeconds': 5, 'observationSeconds': 61,
                'reason': '61s exceeds 20s confirmation + 30s request and 10s online + 30s retention + 5s scan; no product timeout/retry changes'},
            independentExpected={'approved': False, 'nodeStatus': 'running', 'automaticConfirmations': 0,
                'materialUtf8': CONTINUED.decode(), 'reportBody': REPORT, 'canComplete': False},
            scenarioSource={'path': str(Path(__file__).resolve()), 'sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()})
        self.evidence['isolation'].update({key: self.env[key] for key in
            ['OMP_CODING_AGENT_DIR', 'XDG_DATA_HOME', 'CHIEF_MAP_CONFIRMATION_EVIDENCE_DIR']})

    def start(self, argv, label):
        if label == 'herdr':
            config = self.root / 'herdr.toml'
            config.write_text(config.read_text(encoding='utf-8') +
                '\n[server]\nheadless_cols = 200\nheadless_rows = 220\n', encoding='utf-8')
        return super().start(argv, label)

    def start_backend(self, role_config):
        for name in ['chief-ui-observer.mjs', 'node-approval-observer.mjs']:
            shutil.copy2(self.args.repo / 'scripts' / name, self.root / name)
        observer = self.root / 'node-approval-observer.mjs'
        with (self.root / 'options.toml').open('a', encoding='utf-8') as stream:
            stream.write(f'\n[[extensions]]\nid = "{self.host}-failure-observer"\nlabel = "Native approval observation"\nagent = "{self.host}"\nkind = "fixed"\nentrypoint = {json.dumps(str(observer))}\n')
        self.evidence['observer'] = {'source': str(self.args.repo / 'scripts/node-approval-observer.mjs'),
            'sha256': hashlib.sha256(observer.read_bytes()).hexdigest(),
            'scope': 'Public native UI/event pass-through only; no tool choice, decision or return substitution'}
        super().start_backend(role_config)

    def configure_model(self):
        config = json.loads(self.args.model_config.read_text(encoding='utf-8'))
        self.mechanical = config.get('mode') == 'controlled-native-failures'
        self.provider_transport = None
        if not self.mechanical:
            super().configure_model()
            self.expected_model = config['model']
            return
        # Authorized mechanical provider: choose tool instructions only. The
        # installed host performs every write, native interaction and HTTP call.
        self.provider_transport = NativeCompletionTransport(self.mechanical_decision, self.root / 'controlled-provider')
        self.provider_transport.__enter__()
        require(49152 <= self.provider_transport.server.server_port <= 65535, 'Provider must use dynamic numeric loopback')
        self.model_wire = WireProxy(self.provider_transport.base, self.root / 'model-wire')
        self.expected_model = 'native-failure-mechanics'
        self.provider_model = 'approval-mechanical/' + self.expected_model
        self.evidence['model'] = {'mode': 'controlled-native-failures', 'model': self.expected_model,
            'scope': 'Mechanical conditions only; no actual model selection or quality claim, no external credentials'}
        agent = self.root / 'agent'
        settings = {'sessionDir': str(agent / 'sessions')}
        if self.args.shell_path:
            require(self.args.shell_path.is_file(), 'Missing explicitly selected shell')
            settings['shellPath'] = str(self.args.shell_path)
        (agent / 'settings.json').write_text(json.dumps(settings), encoding='utf-8')
        models = {'providers': {'approval-mechanical': {'baseUrl': self.model_wire.base, 'api': 'openai-completions',
            'apiKey': 'owned-mechanical-provider-only', 'models': [{'id': self.expected_model, 'name': 'Native failure mechanics',
                'input': ['text'], 'contextWindow': 128000, 'maxTokens': 4096,
                'cost': {'input': 0, 'output': 0, 'cacheRead': 0, 'cacheWrite': 0}}]}}}
        for name in ['models.json', 'models.yml']:
            (agent / name).write_text(json.dumps(models), encoding='utf-8')
        (agent / 'config.yml').write_text('autoResume: false\nstartup:\n  setupWizard: false\n  showSplash: false\n  checkUpdate: false\nmarketplace:\n  autoUpdate: off\n', encoding='utf-8')

    def mechanical_decision(self, request):
        conversation = request.get('messages', [])
        last_user = max((index for index, message in enumerate(conversation) if message.get('role') == 'user'), default=-1)
        user = message_text(conversation[last_user]) if last_user >= 0 else ''
        results = [message for message in conversation[last_user + 1:] if message.get('role') == 'tool']
        material = str(self.root / 'zone-worker/material.txt')
        steps = []
        if 'CONTINUED-MATERIAL-731' in user:
            steps = [('write', {'path': material, 'content': CONTINUED.decode()}),
                     ('report', {'body': REPORT, 'can_complete': False, 'outputs': [{'id': 'material', 'path': material}]})]
        elif '用户现在明确请求本次成果批准' in user or '这是观察窗口结束后用户主动发起的新请求' in user:
            steps = [('approve', {})]
        elif 'INITIAL-MATERIAL-731' in user:
            steps = [('write', {'path': material, 'content': 'INITIAL-MATERIAL-731\n'})]
        index = len(results)
        decision = {'content': 'Controlled mechanical turn complete; waiting for new user input.', 'tool_calls': [],
                    'usage': {'prompt_tokens': 1, 'completion_tokens': 1, 'total_tokens': 2}}
        if index < len(steps):
            name, arguments = steps[index]
            available = {tool.get('function', {}).get('name') for tool in request.get('tools', [])}
            require(name in available, ('Installed native tool missing; no substitute tool', name, sorted(available)))
            decision.update(content='', tool_calls=[{'name': name, 'arguments': arguments}])
        return decision

    def cleanup_extra(self):
        try:
            super().cleanup_extra()
        finally:
            if getattr(self, 'provider_transport', None):
                self.provider_transport.__exit__(None, None, None)

    def wire(self, route):
        return [record for record in self.backend_wire.records if record['path'] == route]

    def public(self):
        detail = self.detail(self.run_id)
        node = detail['run']['nodes']['work']
        require(node['status'] == 'running', ('Confirmation must not fail or complete the node', detail))
        require(node['execution']['id'] == self.execution_id, 'Confirmation changed execution identity')
        return detail

    def unapproved(self):
        detail = self.public()
        execution = detail['run']['nodes']['work']['execution']
        require(execution['approved'] is False and execution['approvalSource'] is None,
                ('No explicit native Yes; public execution must remain unapproved', detail))
        return detail

    def history(self):
        return self.native(self.worker)

    def settled(self, chat, start=0):
        def probe():
            history = self.native(chat)
            replies = [message for message in messages(history, start) if message.get('role') == 'assistant'] if history else []
            return history if replies and replies[-1].get('stopReason') == 'stop' and not calls(
                {'branch': [{'type': 'message', 'message': replies[-1]}]}) else None
        return until('actual native completed assistant response', probe, self.args.timeout)

    def send(self, chat, content, label):
        ready = self.herdr('agent', 'wait', chat['paneId'], '--until', 'idle', '--until', 'done', '--timeout', '10000')
        record = {'kind': label, 'sessionId': chat['sessionId'], 'input': content,
                  'at': time.monotonic(), 'ready': ready}
        self.evidence.setdefault('userInputs', []).append(record)
        record['admission'] = json.loads(self.command([self.args.herdr, 'agent', 'prompt', chat['paneId'], content,
            '--wait', '--until', 'working', '--until', 'blocked', '--timeout', '10000'], seconds=15))['result']

    def confirmation_requests(self):
        records = [json.loads(path.read_text(encoding='utf-8')) for path in self.confirmations.glob('*.request.json')]
        return sorted((record for record in records if record.get('sessionId') == self.worker['sessionId']),
                      key=lambda record: record['at'])

    def terminal(self):
        return self.command([self.args.herdr, 'pane', 'read', self.worker['paneId'], '--source', 'visible', '--format', 'text'])

    def pending(self, before):
        def probe():
            records = self.confirmation_requests()
            require(len(records) <= before + 1, 'More than one native confirmation for one explicit user request')
            if len(records) != before + 1:
                return None
            screen = self.terminal()
            if '成果批准' not in screen or 'Yes' not in screen or 'No' not in screen:
                return None
            request = records[-1]
            require(request['title'] == '成果批准' and request['message'] == QUESTION,
                    ('Approval must ask the fixed result question, not a report-send preview', request))
            return {'request': request, 'screen': screen, 'at': time.monotonic()}
        return until('actual result approval UI and raw native request', probe, self.args.timeout)

    def decide(self, pending, choice):
        if choice in ['Yes', 'No']:
            def selected(screen):
                return any(arrow + ' ' + choice in screen for arrow in ['→', '❯'])
            screen = pending['screen']
            if not selected(screen):
                self.herdr('agent', 'send-keys', self.worker['paneId'], 'down' if choice == 'No' else 'up')
                screen = until('native selected value ' + choice,
                    lambda: (value if selected(value := self.terminal()) else None), 5)
            require(selected(screen), 'Key acknowledgement is not a native selection')
            pending['selectedScreen'] = screen
            self.herdr('agent', 'send-keys', self.worker['paneId'], 'enter')
        elif choice == 'cancel':
            self.herdr('agent', 'send-keys', self.worker['paneId'], 'esc')
        elif choice != 'timeout':
            raise AssertionError('Unknown native decision')
        pending['userDecision'] = {'choice': choice, 'at': time.monotonic()}
        request_id = pending['request']['id']
        def returned():
            for kind in ['response', 'error']:
                path = self.confirmations / (request_id + '.' + kind + '.json')
                if path.exists():
                    return {'kind': kind, **json.loads(path.read_text(encoding='utf-8'))}
        actual = until('real native return or exception', returned, 25)
        pending['nativeReturn'] = actual
        if choice == 'Yes':
            require(actual.get('result') is True, ('Selected Yes did not return native true', actual))
        elif choice == 'No':
            require(actual['kind'] == 'response' and actual.get('result') is False,
                    ('Selected No must return actual native false, not another fault category', actual))
        else:
            require(actual.get('result') is not True, 'Native non-Yes unexpectedly approved')
        if choice == 'timeout':
            require(actual['elapsedMs'] >= 19_900, ('Not the real configured 20-second timeout', actual))
        return actual

    def result(self, history, start):
        approval_calls = calls(history, 'approve', start)
        require(len(approval_calls) == 1, ('Exactly one explicit native approval call expected', approval_calls))
        call_id = approval_calls[0]['id']
        results = [message for message in messages(history, start) if message.get('role') == 'toolResult'
                   and message.get('toolCallId') == call_id]
        require(len(results) == 1, ('Missing actual approval tool result', history))
        body = results[0].get('details', {}).get('body', {})
        require(results[0].get('isError') is not True and body.get('approved') is False and body.get('sent') is False
                and body.get('reason') in (['confirmation-unavailable', 'confirmation-not-approved']
                    if self.condition == 'unavailable' else ['confirmation-not-approved']),
                ('Non-approval feedback must distinguish not-sent from execution failure', results))
        require('继续工作' in body.get('guidance', '') and '不自动重试' in body.get('guidance', ''), results)
        return results[0]

    def quiet(self, history):
        requests = self.confirmation_requests()
        wire_counts = {route: len(self.wire(route)) for route in
            ['/api/workflow/node/context', '/api/workflow/node/approve', '/api/workflow/node/report']}
        native_calls = calls(history)
        before = self.unapproved()
        started = time.monotonic()
        observation = {'started': started, 'secondsRequired': 61, 'samples': [], 'wireCounts': wire_counts,
                       'confirmationCount': len(requests), 'nativeCalls': native_calls}
        self.evidence['noAutomaticActionWindow'] = observation
        while time.monotonic() - started < 61:
            detail = self.unapproved()
            require(detail == before, 'Public facts changed without new user input')
            require(self.confirmation_requests() == requests, 'System automatically requested another confirmation')
            require(all(len(self.wire(route)) == count for route, count in wire_counts.items()),
                    'System automatically issued a confirmation/context/approval/report HTTP request')
            current = self.history()
            require(current and calls(current) == native_calls, 'System automatically invoked a new native tool')
            observation['samples'].append({'at': time.monotonic(), 'public': detail,
                'nativeCallCount': len(calls(current)), 'confirmationCount': len(self.confirmation_requests())})
            time.sleep(0.5)
        observation.update(ended=time.monotonic(), actualSeconds=time.monotonic() - started)

    def model_identity(self):
        records = self.model_wire.records
        require(records, 'No actual authorized model request')
        identities = []
        for record in records:
            require(record.get('ended') and record.get('status') == 200 and not record.get('error'),
                    ('Actual model failed; do not substitute controlled success', record))
            request = json.loads(record['request'])
            raw = record['response']
            packets = [json.loads(raw)] if raw.lstrip().startswith('{') else [json.loads(line[5:])
                for line in raw.splitlines() if line.startswith('data:') and line[5:].strip() != '[DONE]']
            returned = {packet['model'] for packet in packets if packet.get('model')}
            require(request.get('model') == self.expected_model and returned == {self.expected_model},
                    ('Actual requested/returned model identity mismatch', returned))
            identities.append({'request': record['index'], 'requested': request['model'], 'returned': sorted(returned)})
        self.evidence['modelIdentities'] = identities

    def host_pids(self):
        entry = Path(self.evidence['installedHosts'][self.host]['entry']).resolve()
        return {item['ProcessId'] for item in self.observe_processes() if any(
            Path(argument).resolve() == entry for argument in item['argv'] if argument.lower().endswith(('.js', '.ts')))}

    def stop_worker(self):
        self.herdr('pane', 'close', self.worker['paneId'])
        for pid, handle in self.handles:
            if pid in self.worker_pids:
                until('owned node native process exits before headless resume', lambda: not handle.alive(), 20)
                self.evidence.setdefault('preResumeExitConfirmations', []).append({'pid': pid, 'exited': True})

    def unavailable(self, start):
        # Keep the genuine node Session/execution; switch only its owned process
        # to the installed host's print/JSON mode. Never replace confirm().
        self.stop_worker()
        def owns_session(path):
            with path.open(encoding='utf-8') as stream:
                first = json.loads(stream.readline())
                if first.get('type') == 'title':
                    first = json.loads(stream.readline())
                return first.get('type') == 'session' and first.get('id') == self.worker['sessionId']
        session = next(path for path in (self.root / 'agent').rglob('*.jsonl') if owns_session(path))
        chosen = False
        def completion(_prompt):
            nonlocal chosen
            decision = {'content': 'Mechanical no-UI invocation complete.', 'tool_calls': []}
            if not chosen:
                chosen = True
                decision = {'content': '', 'tool_calls': [{'name': 'approve', 'arguments': {}}]}
            decision['usage'] = {'prompt_tokens': 1, 'completion_tokens': 1, 'total_tokens': 2}
            return json.dumps(decision)
        with CompletionTransport(completion, self.root / 'mechanical-provider') as provider:
            require(49152 <= provider.server.server_port <= 65535, 'Mechanical provider must bind dynamic numeric loopback')
            for name in ['models.json', 'models.yml']:
                path = self.root / 'agent' / name
                if not path.exists():
                    continue
                config = json.loads(path.read_text(encoding='utf-8'))
                config['providers']['failure-mechanical'] = {'baseUrl': provider.base, 'api': 'openai-completions',
                    'apiKey': 'owned-no-ui-fixture', 'models': [{'id': 'no-ui', 'name': 'Controlled mechanical failure',
                    'input': ['text'], 'contextWindow': 128000, 'maxTokens': 1024,
                    'cost': {'input': 0, 'output': 0, 'cacheRead': 0, 'cacheWrite': 0}}]}
                path.write_text(json.dumps(config), encoding='utf-8')
            cli = Path(self.evidence['installedHosts'][self.host]['entry'])
            argv = [self.args.bun if self.host == 'omp' else self.args.node, str(cli), '--mode', 'json', '--print',
                '--no-extensions', *(['--no-mcp'] if self.host == 'pi' else
                ['--no-lsp', '--no-pty', '--no-skills', '--no-rules', '--no-context-files', '--no-title', '--thinking', 'off']),
                '-e', str(self.root / f'toolkit/entrypoints/{self.host}-auto/node-chat.ts'),
                '-e', str(self.root / 'node-approval-observer.mjs'), '--model', 'failure-mechanical/no-ui',
                '--session', str(session), 'User explicitly requests approve once in no-UI mode; do not retry.']
            self.evidence.setdefault('userInputs', []).append({'kind': 'explicit-controlled-no-ui-request',
                'at': time.monotonic(), 'sessionId': self.worker['sessionId'], 'argv': argv})
            process = Process(argv, self.root / 'zone-worker', self.env, self.root, 'headless-native')
            self.processes.append(process)
            exit_code = process.child.wait(timeout=self.args.timeout)
            process.stop()
            self.evidence['noUI'] = {'scope': 'Controlled provider only for mechanical no-UI fault; not real model tool selection',
                'argv': argv, 'pid': process.child.pid, 'exit': exit_code, 'exited': process.child.poll() is not None,
                'providerRequests': provider.records, 'stdout': str(self.root / 'headless-native-stdout.log'),
                'stderr': str(self.root / 'headless-native-stderr.log'), 'session': str(session)}
            require(exit_code == 0, ('Actual headless host failed', self.evidence['noUI']))
        events = [json.loads(line) for line in (self.confirmations / 'native-events.jsonl').read_text(encoding='utf-8').splitlines()]
        require(any(event['sessionId'] == self.worker['sessionId'] and event['pid'] == process.child.pid
            and event['type'] == 'session_start' and event['hasUI'] is False for event in events),
            'Missing real installed no-UI session observation')
        history = self.history()
        require(history and history['id'] == self.worker['sessionId'], 'Headless process changed native identity')
        self.result(history, start)
        return history

    def exercise(self):
        try:
            self.exercise_case()
        except BaseException:
            if hasattr(self, 'worker'):
                try:
                    self.evidence['failureTerminal'] = self.terminal()
                except BaseException:
                    self.evidence['failureTerminalError'] = traceback.format_exc()
            raise

    def exercise_case(self):
        for role in ['manager', 'worker']:
            directory = self.root / ('zone-' + role)
            directory.mkdir()
            self.http('/api/zones', {'id': 'failure/' + role, 'path': str(directory)})
            self.herdr('workspace', 'create', '--cwd', str(directory), '--label', directory.name, '--no-focus')
        manager = self.task('failure-manager', 'manager',
            '只确认收到本次 Run 后等待用户，不自行 prepare/dispatch/collect/complete/fail/finish，不因任何节点报告采取动作。', self.host)
        worker = self.task('failure-worker', 'normal',
            '仅在用户明确要求本次批准时调用 approve 一次；任何非批准返回后等待，不重试、不标失败。'
            '初始任务只写指定文件并等待，不报告、不请求批准。用户后续要求的进度 report 自由发送，不需要批准。'
            '只使用本 Chat 原生工具，不通过 HTTP/shell 代替 workflow 工具，不读取其他 Chat 或服务存储。', self.host)
        self.material = self.root / 'zone-worker/material.txt'
        bot = {'id': 'worker', 'name': 'failure worker', 'when': '', 'plan_guide': '', 'node_guide': '', 'task_id': worker}
        (self.root / 'bots/worker.json').write_text(json.dumps(bot), encoding='utf-8')
        definition = {'title': 'Owned native approval failure', 'description': '', 'nodes': {'work': {
            'title': 'Work', 'bot_id': 'worker', 'input': f'仅写绝对路径 {self.material}，UTF-8 内容严格为 INITIAL-MATERIAL-731 后跟一个 LF，然后等待用户。不要请求批准或发送报告。',
            'zone': 'failure/worker', 'blocked_by': [], 'auto_approved': False, 'output_rule': [{'id': 'material'}]}}}
        (self.root / 'maps/failure.json').write_text(json.dumps(definition, ensure_ascii=False), encoding='utf-8')
        self.evidence['definition'] = self.http('/api/scheduler/maps/failure.json')
        started = self.http('/api/scheduler/maps/failure.json/runs', {'managerTaskId': manager,
            'zone': 'failure/manager', 'userInput': '只确认收到，然后等待用户。'}, 201)
        self.run_id = started['id']
        def manager_ready():
            detail = self.detail(self.run_id)
            return detail['run'].get('manager')
        manager_chat = until('real manager Chat identity', manager_ready, self.args.timeout)
        self.chat_hosts[manager_chat['sessionId']] = self.host
        self.settled(manager_chat)
        self.http('/api/workflow/manager/prepare_inputs', {'sessionId': manager_chat['sessionId'],
            'fileInputs': [], 'confirmed': True, 'reason': '本场景无文件输入；执行者自行写独占材料。'})
        before_pids = self.host_pids()
        dispatched = self.http('/api/workflow/manager/dispatch', {'sessionId': manager_chat['sessionId'], 'nodes': ['work']})
        execution = dispatched['snapshot']['nodes']['work']['execution']
        self.execution_id, self.worker = execution['id'], execution['chat']
        self.chat_hosts[self.worker['sessionId']] = self.host
        self.worker_pids = self.host_pids() - before_pids
        require(self.worker_pids, 'Missing separately observed node native process')
        first = self.settled(self.worker)
        require(self.material.read_bytes() == b'INITIAL-MATERIAL-731\n', 'Actual model did not write the independently specified initial material')
        self.unapproved()
        require(not self.confirmation_requests() and not self.wire('/api/workflow/node/approve'),
                'Initial work requested unsolicited approval')
        start = len(first['branch'])
        if self.condition == 'unavailable':
            history = self.unavailable(start)
        else:
            self.send(self.worker, '用户现在明确请求本次成果批准。调用 approve {} 一次。工具返回后说明真实结果，然后等待；不要自动重试、报告或标失败。', 'explicit-negative-approval')
            pending = self.pending(0)
            self.evidence['negativeConfirmation'] = pending
            self.decide(pending, 'No' if self.condition == 'no' else self.condition)
            history = self.settled(self.worker, start)
        self.evidence['negativeToolResult'] = self.result(history, start)
        self.evidence['afterNegative'] = self.unapproved()
        require(not self.wire('/api/workflow/node/approve'), 'Unapproved native outcome reached approval HTTP')
        self.quiet(history)
        if self.condition == 'unavailable':
            # The observation window has ended. Only this explicit test/user
            # recovery restores UI; no automatic system recovery is supplied.
            restored = self.http('/api/chats/resume', {'sessionId': self.worker['sessionId']})
            require(restored['outcome'] == 'restored', restored)
            self.worker = {**restored['target'], 'sessionId': self.worker['sessionId']}
            self.observe_processes()
        continuation_start = len(self.history()['branch'])
        confirmations_before = len(self.confirmation_requests())
        self.send(self.worker, '用户要求继续修改材料：将当前 material.txt 改为严格 UTF-8 文本 CONTINUED-MATERIAL-731 后跟一个 LF，'
            '然后调用 report，body 严格为 P7-CONTINUED-731，can_complete=false，outputs 注册 material 为该真实文件绝对路径。'
            '这是进度报告，不请求或重试批准；发送后等待。', 'explicit-continue-work-before-new-approval')
        continued = self.settled(self.worker, continuation_start)
        require(self.material.read_bytes() == CONTINUED, 'Failed approval blocked actual continued material editing')
        require(len(calls(continued, 'report', continuation_start)) == 1 and not calls(continued, 'approve', continuation_start),
                'Progress report was gated by another approval')
        detail = self.unapproved()
        reports = [report for report in detail['reports'] if report['executionId'] == self.execution_id]
        require(len(reports) == 1, ('Progress report did not save exactly once', reports))
        report = reports[0]
        require(report['body'] == REPORT and report['canComplete'] is False and report['from']['sessionId'] == self.worker['sessionId']
            and report['runId'] == self.run_id and report['nodeId'] == 'work'
            and len(report['outputs']) == 1 and report['outputs'][0]['id'] == 'material'
            and Path(report['outputs'][0]['path']).resolve() == self.material.resolve(), ('Public continued report differs', report))
        require(len(self.confirmation_requests()) == confirmations_before, 'Report still opens obsolete send confirmation')
        self.evidence['continuedWork'] = {'history': continued, 'public': detail, 'material': {'path': str(self.material),
            'utf8': self.material.read_text(encoding='utf-8'), 'sha256': hashlib.sha256(self.material.read_bytes()).hexdigest()}}
        new_start = len(continued['branch'])
        self.send(self.worker, '这是观察窗口结束后用户主动发起的新请求，不是系统重试。现在重新调用 approve {} 一次请求本次成果批准；'
            '工具返回后等待，不追加报告、不 complete/fail。', 'explicit-new-approval-after-observation-and-report')
        pending = self.pending(confirmations_before)
        self.evidence['explicitNewConfirmation'] = pending
        self.decide(pending, 'Yes')
        final = self.settled(self.worker, new_start)
        require(len(calls(final, 'approve', new_start)) == 1, 'Explicit new request made extra native approval calls')
        public = self.public()
        current = public['run']['nodes']['work']['execution']
        require(current['approved'] is True and current['approvalSource'] == 'manual', ('Explicit native Yes did not persist', public))
        require(public['reports'] == detail['reports'], 'Approval changed or invented reports')
        submissions = self.wire('/api/workflow/node/approve')
        require(len(submissions) == 1 and submissions[0].get('status') == 200,
                ('Only the new explicit Yes may reach approval HTTP', submissions))
        self.evidence['final'] = {'history': final, 'public': public, 'approvalHTTP': submissions}
        self.model_identity()
        self.observe_processes()
        self.evidence['coverage'] = {'verified': True, 'condition': self.condition,
            'negativeToolResult': self.evidence['negativeToolResult'], 'observation': self.evidence['noAutomaticActionWindow'],
            'continuedWork': 'continuedWork', 'newRequest': 'explicitNewConfirmation',
            'scope': 'Controlled provider, real installed hosts/UI/backend/material/report mechanics; not real model business selection'
                     if self.mechanical else ('Actual model and installed native UI' if self.condition != 'unavailable' else
                     'Actual model work and recovery; controlled provider only for real installed no-UI approval invocation')}
        self.evidence['passed'] = True
