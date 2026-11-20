"""Ticket 12 oracles: installed native tools/UI, public HTTP and actual files.

Approved seams are S2 actual OMP/Pi and S1 public workflow/definition operations.
No Run writes, synthetic Session IDs, internal tool execution or recovery doubles.

Design before observation:
P3/P14: a recovered Chat could be rebound/replayed or consume the live Map mode.
  Change manual to auto, terminate/confirm exit, resume the original native Chat;
  compare business history and public execution, then require its real manual UI.
P11/P24: an old Yes could authorize retry, or waiting UI could lock the Run.
  Keep that real UI pending while bounded public work and fail/retry finish;
  only then return real Yes and observe unchanged new execution for 91 seconds.
P10: source Task or a model-selected identity could grant another execution.
  Compare actual tool catalogs and zero-argument native calls across real roles,
  and reject cross-execution requests with those actual Session identities.
P16: an implementation could bind approval to bytes/report or revoke on change.
  Native writes two distinct literal files/reports after approval; public readback
  must keep the fact without making completion implicit.
P28: an inner approval/finish could automatically approve/complete its parent.
  Finish a real child Run, inspect the still-unapproved parent, submit a separate
  native overall approval in each frozen mode, and explicitly complete later.
Independent expected values are ticket 12 and the literal specimens below.
"""
import hashlib
import json
from pathlib import Path
import shutil
import time
import urllib.error
import urllib.request

from acceptance.host.processes import require, until
from acceptance.host.completion_transport import NativeCompletionTransport
from acceptance.host.model import message_text
from node_approval_failures_acceptance import ApprovalFailuresAcceptance, calls as recorded_calls, messages


def calls(history, name=None, start=0):
    """Decode native calls without mistaking device help or file writes for execution."""
    results = {message.get('toolCallId'): message for message in messages(history, start)
               if message.get('role') == 'toolResult'}
    observed = []
    for call in recorded_calls(history, start=start):
        arguments = call.get('arguments', {})
        route = arguments.get('path')
        result = results.get(call['id'], {})
        details = result.get('details')
        device = details.get('xdev', {}) if isinstance(details, dict) else {}
        if not isinstance(device, dict):
            device = {}
        if call['name'] == 'read' and route in {'xd://approve', 'xd://report', 'xd://run', 'xd://collect'}:
            if result and not result.get('isError'):
                continue  # Public device reads return documentation, never execute the tool.
        if call['name'] == 'write' and device.get('tool') in {'approve', 'report', 'run', 'collect'}:
            if route == 'xd://' + device['tool']:
                if device.get('mode') == 'help' and not result.get('isError'):
                    continue
                if device.get('mode') == 'execute':
                    try:
                        payload = json.loads(arguments.get('content', ''))
                    except (TypeError, ValueError):
                        payload = None
                    if isinstance(payload, dict):
                        call = {**call, 'name': device['tool'], 'arguments': payload,
                                'nativeRoute': {'name': 'write', 'arguments': arguments, 'receipt': device}}
        if name is None or call['name'] == name:
            observed.append(call)
    return observed

PROMISES = ['P3', 'P10', 'P11', 'P14', 'P16', 'P24', 'P28']
COVERAGE_PARTS = {'P10': ('base-roles', 'old-execution', 'dual-role'), 'P28': ('auto', 'manual')}
IDLE = ('只确认收到后等待用户；不自行批准、报告或推进。只使用本 Chat 已提供的原生 workflow 工具，'
        '不以 HTTP、shell、脚本或读取服务存储替代这些工具，不选择或输入任何会话／Run／node／execution 身份。'
        '用户要求哪一个动作就执行一次，真实失败后说明并等待，不重试、不自动补救。')
FIRST = 'LIFECYCLE-MATERIAL-FIRST-731\n'
SECOND = 'LIFECYCLE-MATERIAL-MODIFIED-947\n'


class ApprovalLifecycleAcceptance(ApprovalFailuresAcceptance):
    def __init__(self, args):
        super().__init__(args)
        self.evidence.update(promise=None, promises=PROMISES, scenario='node-approval-lifecycle',
            coverage={key: {'verified': False} for key in PROMISES}, cases=[],
            budgets={'confirmationSeconds': 60, 'requestSeconds': 30, 'boundedPublicSeconds': 2,
                     'staleObservationSeconds': 91,
                     'reason': '91s covers 60s approval confirmation + 30s request + 1s margin'},
            scenarioSource={'path': str(Path(__file__).resolve()),
                            'sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()})
        self.chat_pids = {}
        self.selected_cases = getattr(args, 'cases', ['persistence', 'nested-auto', 'nested-manual', 'lifecycle'])
        self.evidence.update(selectedCases=self.selected_cases, selectedCasesPassed=False,
            caseResults={}, nestedModes={}, coverageParts={key: {} for key in COVERAGE_PARTS})
        self.gate_config = self.root / 'retry-gate.json'
        self.gate_release = self.root / 'retry-gate-release'
        self.gate_evidence = self.root / 'retry-gate'
        self.gate_evidence.mkdir()
        self.env['NODE_APPROVAL_LIFECYCLE_GATE'] = str(self.gate_config)
        self.evidence['independentExpected'] = {
            'P3': 'Recovered original manual mode; new retry uses changed auto mode.',
            'P10': 'Actual Session alone selects authority; extra fields cannot switch execution.',
            'P11': 'Real old Yes cannot alter the gated new execution or cause business/notification effects.',
            'P14': 'Actual native/backend exits preserve history/approval/mode without replay or old-Run takeover.',
            'P16': 'Two literal changed materials and later reports retain approval.',
            'P24': 'Public query, progress/ready reports and independent completion finish within 2s each while real confirmation is pending.',
            'P28': 'Actual finished child and independent parent approval in each frozen mode; explicit parent completion.'}

    def record_part(self, promise, part, evidence):
        parts = self.evidence['coverageParts'][promise]
        require(part in COVERAGE_PARTS[promise], ('Unknown coverage part', promise, part))
        parts[part] = {'verified': True, 'evidence': evidence}
        self.evidence['coverage'][promise] = {'verified': all(parts.get(name, {}).get('verified')
            for name in COVERAGE_PARTS[promise]), 'parts': parts}

    def configure_model(self):
        config = json.loads(self.args.model_config.read_text(encoding='utf-8'))
        require('mode' not in config, 'Lifecycle requires explicitly authorized real model, never controlled choices')
        super().configure_model()

    def start(self, argv, label):
        process = super().start(argv, label)
        if str(argv[0]) == str(self.args.backend):
            self.backend_process = process
        return process

    def start_backend(self, role_config):
        proxy = self.root / 'external-cli'
        proxy.mkdir()
        source = self.args.backend.parent / 'examples/recovery_herdr_proxy.exe'
        require(source.is_file(), 'Build existing recovery_herdr_proxy for transparent notification/input observation')
        executable = proxy / 'recovery_herdr_proxy.exe'
        shutil.copy2(source, executable)
        real = self.args.herdr
        gate_observer = self.root / 'node-approval-lifecycle-gate.mjs'
        gate_source = self.args.repo / 'scripts/node-approval-lifecycle-gate.mjs'
        shutil.copy2(gate_source, gate_observer)
        with (self.root / 'options.toml').open('a', encoding='utf-8') as stream:
            stream.write('\n[[extensions]]\nid = ' + json.dumps(self.host + '-lifecycle-gate')
                + '\nlabel = "Owned predecision scheduling"\nagent = ' + json.dumps(self.host)
                + '\nkind = "fixed"\nentrypoint = ' + json.dumps(str(gate_observer)) + '\n')
        self.evidence['nativeSchedulerSource'] = {'path': str(gate_source),
            'sha256': hashlib.sha256(gate_source.read_bytes()).hexdigest(), 'hookBudgetMs': 25000,
            'existingOmpHookBudgetMs': 30000, 'changesProductTimeouts': False}
        (proxy / 'recovery-proxy.json').write_text(json.dumps({'herdr': str(real)}), encoding='utf-8')
        try:
            self.args.herdr = executable
            super().start_backend(role_config)
        finally:
            self.args.herdr = real
        self.evidence['externalRecorder'] = {'source': str(self.args.repo / 'backend/examples/recovery_herdr_proxy.rs'),
            'binarySha256': hashlib.sha256(executable.read_bytes()).hexdigest(), 'faultEnabled': False,
            'scope': 'All actual backend CLI argv are recorded before forwarding to installed Herdr unchanged.'}

    def external(self):
        return [{'file': path.name, **json.loads(path.read_text(encoding='utf-8'))}
            for path in sorted((self.root / 'external-cli/calls').glob('*.json'))]

    def http(self, path, body=None, status=200, budget=100):
        record = {'path': path, 'method': 'GET' if body is None else 'POST', 'request': body,
                  'expectedStatus': status, 'started': time.monotonic(), 'budgetSeconds': budget}
        self.evidence.setdefault('publicOperations', []).append(record)
        request = urllib.request.Request(self.base + path, data=None if body is None else json.dumps(body).encode(),
                                         headers={'Content-Type': 'application/json'})
        try:
            try:
                response = urllib.request.urlopen(request, timeout=budget)
            except urllib.error.HTTPError as error:
                response = error
            with response:
                record.update(status=response.status, response=json.load(response))
            require(record['status'] == status, record)
            return record['response']
        finally:
            record.update(ended=time.monotonic(), elapsedSeconds=time.monotonic() - record['started'])
            if budget != 100:
                require(record['elapsedSeconds'] <= budget, ('Public operation exceeded bound', record))

    def settled(self, chat, start=0, observe=None):
        def probe():
            if observe is not None:
                observe()
            history = self.native(chat)
            current = [message for message in messages(history, start)
                if message.get('role') in ['user', 'assistant', 'toolResult']] if history else []
            # A preceding assistant reply is not evidence that a later admitted
            # user input has stopped. Observe the actual final business message.
            return history if current and current[-1].get('role') == 'assistant' \
                and current[-1].get('stopReason') == 'stop' and not any(
                    part.get('type') == 'toolCall' for part in current[-1].get('content', [])
                    if isinstance(part, dict)) else None
        return until('current native input reaches actual assistant stop', probe, self.args.timeout)

    def track(self, chat):
        require(chat and chat.get('sessionId'), ('Missing real native identity', chat))
        self.chat_hosts[chat['sessionId']] = self.host
        def association():
            observed = self.host_pids()
            path = self.confirmations / 'native-events.jsonl'
            if not path.exists():
                return None
            # Only complete append-only public SDK observations are consumed.
            events = [json.loads(line) for line in path.read_text(encoding='utf-8').split('\n')[:-1] if line]
            owners = {event['pid'] for event in events if event.get('type') == 'session_start'
                and event.get('sessionId') == chat['sessionId'] and event['pid'] in observed}
            require(len(owners) <= 1, ('Ambiguous live native Session process', chat, owners))
            return owners or None
        owners = until('actual SDK Session to installed live process association', association, self.args.timeout)
        self.chat_pids[chat['sessionId']] = owners
        self.evidence.setdefault('nativeAssociations', []).append({'chat': chat, 'pids': sorted(owners),
            'at': time.monotonic(), 'source': 'Public native session_start + actual Windows process argv/held handle'})
        return chat

    def action(self, manager, action, extra=None, status=200, budget=100):
        return self.http('/api/workflow/manager/' + action,
                         {'sessionId': manager['sessionId'], **(extra or {})}, status, budget)

    def define_map(self, name, nodes):
        value = {'title': 'Owned lifecycle ' + name, 'description': '', 'nodes': nodes}
        path = self.root / 'maps' / (name + '.json')
        path.write_text(json.dumps(value, ensure_ascii=False), encoding='utf-8')
        actual = self.http('/api/scheduler/maps/' + path.name)
        self.evidence.setdefault('definitions', []).append({'name': name, 'literal': value, 'public': actual})
        return path

    def node(self, title, auto=True, blocked=None, bot='worker', **extra):
        return {'title': title, 'bot_id': bot, 'input': IDLE, 'zone': 'lifecycle/worker',
                'blocked_by': blocked or [], 'auto_approved': auto, 'auto_close': False,
                'input_rule': [], 'output_rule': [], **extra}

    def new_run(self, name, user_input=IDLE, native_management=False):
        started = self.http('/api/scheduler/maps/' + name + '.json/runs',
            {'managerTaskId': self.manager_task, 'zone': 'lifecycle/manager', 'userInput': user_input}, 201)
        run_id = started['id']
        manager = self.track(until('actual manager identity', lambda: self.detail(run_id)['run'].get('manager'), self.args.timeout))
        if not native_management:
            self.action(manager, 'prepare_inputs', {'fileInputs': [], 'confirmed': True, 'reason': 'This owned scenario has no file inputs.'})
        return run_id, manager

    def native_dispatch(self, run_id, node):
        def running():
            state = self.detail(run_id)['run']['nodes'][node]
            require(state['status'] not in ['failed', 'completed'], ('Native dispatch did not leave owned work running', state))
            return state.get('execution') if state['status'] == 'running' else None
        execution = until('actual native manager dispatch of ' + node, running, self.args.timeout)
        return execution, self.track(execution['chat'])

    def dispatch(self, manager, node, run_id):
        before = self.detail(run_id)
        state = before['run']['nodes'][node]
        if state['status'] == 'pending':
            response = self.action(manager, 'dispatch', {'nodes': [node]})
        else:
            require(state['status'] == 'running' and state.get('execution'), ('Node not legally dispatchable/running', before))
            response = {'snapshot': before['run'], 'origin': 'Actual native manager already dispatched this fresh Run node'}
        execution = response['snapshot']['nodes'][node]['execution']
        chat = self.track(execution['chat'])
        require(self.detail(run_id)['run']['nodes'][node]['status'] == 'running', response)
        self.evidence.setdefault('dispatchObservations', []).append({'runId': run_id, 'nodeId': node,
            'before': before, 'response': response, 'actualNativeDispatches': self.wire('/api/workflow/manager/dispatch')})
        return execution, chat

    def business_arguments(self, call):
        arguments = dict(call['arguments'])
        if self.host == 'omp' and 'i' in arguments:
            require(isinstance(arguments['i'], str), 'Installed OMP intent metadata is not a string')
            del arguments['i']
        return arguments

    def tool(self, chat, name, arguments, manual=False, error=False, observe=None):
        # Existing report notifications may still be executing readonly native
        # turns. Observe that work stop before the unchanged 10s admission check.
        self.settled(chat)
        start = len(self.native(chat)['branch'])
        before = len(self.confirmation_requests(chat))
        self.send(chat, '用户现在明确请求一次原生工具 ' + name + '，参数严格为 ' + json.dumps(arguments, ensure_ascii=False)
            + '。不要调用其他工具或添加上述对象以外的业务参数；返回后只说明真实结果并等待，失败不重试。', 'explicit-native-' + name)
        if manual:
            pending = self.pending(before, chat)
            self.evidence.setdefault('nativeYes', []).append(pending)
            self.decide(pending, 'Yes', chat)
        history = self.settled(chat, start, observe=observe)
        invoked = calls(history, start=start)
        require(len(invoked) == 1 and invoked[0]['name'] == name and self.business_arguments(invoked[0]) == arguments,
                ('Unexpected actual native calls; no scripted substitution', invoked))
        results = [message for message in messages(history, start) if message.get('role') == 'toolResult'
                   and message.get('toolCallId') == invoked[0]['id']]
        require(len(results) == 1 and bool(results[0].get('isError')) == error, ('Actual tool result differs', results))
        require(len(self.confirmation_requests(chat)) == before + int(manual), 'Unexpected native confirmation')
        record = {'chat': chat, 'name': name, 'arguments': arguments, 'history': history,
                  'result': results[0], 'ended': time.monotonic()}
        self.evidence.setdefault('nativeInvocations', []).append(record)
        return record

    def identity_fault(self, chat, arguments):
        self.model_identity()
        original_native = self.native(chat)
        session = None
        for candidate in (self.root / 'agent').rglob('*.jsonl'):
            with candidate.open(encoding='utf-8') as stream:
                header = json.loads(stream.readline())
                if header.get('type') == 'title':
                    header = json.loads(stream.readline())
            if header.get('type') == 'session' and header.get('id') == chat['sessionId']:
                session = candidate
                break
        require(session is not None, 'Missing actual invocation-owned history for native identity fault')
        source = self.root / 'identity-fault-source.jsonl'
        shutil.copy2(session, source)
        original_history_hash = hashlib.sha256(session.read_bytes()).hexdigest()
        output = self.root / 'identity-fault'
        output.mkdir()
        def mechanical(request):
            conversation = request.get('messages', [])
            last = max((index for index, message in enumerate(conversation) if message.get('role') == 'user'), default=-1)
            prompt = message_text(conversation[last]) if last >= 0 else ''
            results = [message for message in conversation[last + 1:] if message.get('role') == 'tool']
            decision = {'content': 'Controlled malformed-identity observation complete; no model judgment.', 'tool_calls': [],
                        'usage': {'prompt_tokens': 1, 'completion_tokens': 1, 'total_tokens': 2}}
            if 'TICKET12-IDENTITY-FAULT' in prompt and not results:
                require(any(tool.get('function', {}).get('name') == 'approve' for tool in request.get('tools', [])),
                        'Actual installed host did not expose approve')
                decision.update(content='', tool_calls=[{'name': 'approve', 'arguments': arguments}])
            return decision
        with NativeCompletionTransport(mechanical, output / 'controlled-provider') as provider:
            config = {'host': self.host, 'agent': str(self.root / 'agent'), 'deployment': str(self.root / 'toolkit'),
                'session': str(source), 'sessionId': chat['sessionId'], 'evidence': str(output),
                'model': self.provider_model, 'endpoint': provider.base, 'node': str(self.args.node),
                'ompCli': str(self.args.omp_package / 'dist/cli.js'), 'piCli': str(self.args.pi_package / 'dist/bundle/cli.js')}
            path = self.root / 'identity-fault.json'
            path.write_text(json.dumps(config), encoding='utf-8')
            self.command([self.args.bun, self.args.toolkit / 'scripts/node-approval-identity-fault.ts', path], seconds=120)
            require(not provider.failed, 'Controlled mechanical transport failed; no substitute success')
        result = json.loads((output / 'result.json').read_text(encoding='utf-8'))
        require(result['passed'] and not result['cleanupErrors'], ('Actual native identity fault failed', result))
        starts = [frame for frame in result['frames'] if frame.get('type') == 'tool_execution_start']
        require(len(starts) == 1 and starts[0].get('toolName') == 'approve', 'Missing actual native malformed tool start')
        require(messages(self.native(chat)) == messages(original_native),
                'Mechanical caller changed the original native business history instead of its owned copy')
        result.update(expectedArguments=arguments, originalHistorySha256=original_history_hash,
            originalHistoryAfterSha256=hashlib.sha256(session.read_bytes()).hexdigest(),
            faultHistorySha256=hashlib.sha256(source.read_bytes()).hexdigest(),
            callerSha256=hashlib.sha256((self.args.toolkit / 'scripts/node-approval-identity-fault.ts').read_bytes()).hexdigest())
        return result

    def role(self, chat, expected, label):
        def observed():
            path = self.root / 'native-model-catalogs.jsonl'
            if not path.exists():
                return None
            frames = [json.loads(line) for line in path.read_text(encoding='utf-8').split('\n')[:-1] if line]
            frames = [frame for frame in frames if frame['sessionId'] == chat['sessionId']
                and frame['pid'] in self.chat_pids[chat['sessionId']]]
            signatures = {frame['signature'] for frame in frames}
            matches = []
            for record in self.model_wire.records:
                if record.get('method') != 'POST' or not record['path'].split('?', 1)[0].rstrip('/').endswith('/chat/completions'):
                    continue
                if record.get('status') != 200 or not record.get('ended'):
                    continue
                request = json.loads(record['request'])
                if not request.get('tools'):
                    continue  # An auxiliary title request is not the business tool catalog.
                projected = {key: request.get(key, [] if key != 'model' else None) for key in ['model', 'messages', 'tools']}
                signature = hashlib.sha256(json.dumps(projected, ensure_ascii=False, sort_keys=True,
                    separators=(',', ':')).encode()).hexdigest()
                if signature in signatures:
                    matches.append(request)
            return (frames, matches) if matches else None
        frames, requests = until('actual Session-attributed model tool catalog ' + label, observed, self.args.timeout)
        # The earliest actual business request proves the startup role; later
        # legitimate Run completion can change the active role contributions.
        requests = requests[:1]
        for request in requests:
            tools = {tool['function']['name']: tool['function'] for tool in request.get('tools', [])}
            actual = set(tools) & {'approve', 'report', 'run', 'collect'}
            require(actual == set(expected), ('Actual native role catalog differs', label, actual, expected))
            if 'approve' in tools:
                schema = tools['approve']['parameters']
                # Installed adapters deliberately pass unknown fields to business
                # validation (additionalProperties=true). OMP additionally injects
                # its nonbusiness i/intent field. Neither declares an identity
                # selector; actual native calls and public receipts below verify
                # zero business arguments and Session-bound authorization.
                allowed = {'i'} if self.host == 'omp' else set()
                require(set(schema.get('properties', {})) <= allowed,
                        ('Native approval declares model-selected business/identity parameters', schema))
        self.evidence.setdefault('roles', []).append({'label': label, 'chat': chat, 'expected': expected,
            'history': self.native(chat), 'nativeRequestAttribution': frames, 'actualRequests': requests})

    def report(self, chat, body, ready=False, outputs=None, observe=None):
        return self.tool(chat, 'report', {'body': body, 'can_complete': ready, 'outputs': outputs or []}, observe=observe)

    def approved(self, run_id, node, execution_id, source):
        detail = self.detail(run_id)
        current = detail['run']['nodes'][node]
        execution = current['execution']
        require(execution['id'] == execution_id and execution['approved'] is True
            and execution['approvalSource'] == source and current['status'] == 'running', detail)
        return detail

    def close_native(self, chat):
        previous = self.native(chat)
        self.model_identity()
        self.herdr('pane', 'close', chat['paneId'])
        pids = self.chat_pids[chat['sessionId']]
        for pid, handle in self.handles:
            if pid in pids:
                until('actual owned native process exits ' + str(pid), lambda: not handle.alive(), 20)
                self.evidence.setdefault('nativeExits', []).append({'sessionId': chat['sessionId'], 'pid': pid,
                    'exited': True, 'at': time.monotonic(), 'observation': 'Held Windows process handle'})
        # Native closure may already have unregistered; only the actual identity
        # is used, never a fake heartbeat or a hand-edited execution association.
        request = urllib.request.Request(self.base + '/api/sessions/events',
            data=json.dumps({'sessionId': chat['sessionId'], 'kind': 'unregister'}).encode(),
            headers={'Content-Type': 'application/json'})
        try:
            response = urllib.request.urlopen(request, timeout=30)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            value = json.load(response)
            require(response.status == 200 or (response.status == 404 and value.get('code') == 'unknown-session'), value)
        return previous

    def restore(self, chat, run_id, node, execution_id):
        associated = self.detail(run_id)['run']['nodes'][node]['execution']
        require(associated['id'] == execution_id and associated['chat']['sessionId'] == chat['sessionId'],
                ('Recovery fixture must target this exact public execution Chat', associated, chat))
        old_pids = set(self.chat_pids[chat['sessionId']])
        previous = self.close_native(chat)
        public_before = self.detail(run_id)
        wire_start = len(self.backend_wire.records)
        external_start = len(self.external())
        restored = self.http('/api/chats/resume', {'sessionId': chat['sessionId']})
        require(restored['outcome'] == 'restored', ('Must start a different native process', restored))
        actual = self.track({**restored['target'], 'sessionId': chat['sessionId']})
        require(old_pids.isdisjoint(self.chat_pids[chat['sessionId']]), 'Recovery reused old native process')
        current = self.native(actual)
        business = lambda history: [message for message in messages(history)
            if message.get('role') in ['user', 'assistant', 'toolResult']]
        require(business(current) == business(previous), 'Native recovery replayed business input or changed history')
        public_after = self.detail(run_id)
        execution = public_after['run']['nodes'][node]['execution']
        old = public_before['run']['nodes'][node]['execution']
        require(execution['id'] == execution_id and all(execution[key] == old[key]
            for key in ['autoApproved', 'approved', 'approvalSource']), 'Recovery rebound execution or changed approval/mode')
        require(public_after['reports'] == public_before['reports'], 'Recovery replayed report')
        require(not [record for record in self.backend_wire.records[wire_start:] if record['path'].endswith(
            ('/approve', '/report', '/dispatch', '/complete', '/finish'))], 'Recovery replayed business HTTP')
        resume_calls = self.external()[external_start:]
        require(not [call for call in resume_calls if call['argv'][:2] == ['agent', 'prompt']],
                ('Native recovery replayed a business prompt through actual Herdr', resume_calls))
        self.evidence.setdefault('nativeRestores', []).append({'oldChat': chat, 'newChat': actual, 'externalCalls': resume_calls,
            'oldPids': sorted(old_pids), 'newPids': sorted(self.chat_pids[chat['sessionId']]), 'outcome': restored,
            'beforeHistory': previous, 'afterHistory': current, 'publicBefore': public_before, 'publicAfter': public_after,
            'publicHistory': self.http('/api/sessions/history')})
        return actual

    def independent(self, task, label):
        known_sessions = {item['sessionId'] for item in self.http('/api/sessions/history')['sessions']}
        preview = self.http('/api/chats/preview', {'zone': 'lifecycle/worker', 'taskId': task})
        started = self.http('/api/chats', {'zone': 'lifecycle/worker', 'taskId': task,
            'chatId': preview['chatId'], 'input': IDLE + ' ' + label}, 202)
        def ready():
            state = self.http('/api/chats/' + started['id'])
            require(state['stage'] != 'failed', state)
            if state['stage'] != 'succeeded':
                return None
            fresh = [item for item in self.http('/api/sessions/history')['sessions']
                     if item['sessionId'] not in known_sessions]
            require(len(fresh) <= 1, ('Independent creation published ambiguous native identity', fresh))
            return {**state, 'sessionId': fresh[0]['sessionId']} if fresh else None
        return self.track(until('actual independent Chat', ready, self.args.timeout))

    def exercise_case(self):
        for role in ['manager', 'worker']:
            directory = self.root / ('zone-' + role)
            directory.mkdir()
            self.http('/api/zones', {'id': 'lifecycle/' + role, 'path': str(directory)})
            self.herdr('workspace', 'create', '--cwd', str(directory), '--label', directory.name, '--no-focus')
        self.manager_task = self.task('lifecycle-manager', 'manager', IDLE, self.host)
        self.worker_task = self.task('lifecycle-worker', 'normal', IDLE, self.host)
        (self.root / 'bots/worker.json').write_text(json.dumps({'id': 'worker', 'name': 'Owned worker',
            'when': '', 'plan_guide': '', 'node_guide': '', 'task_id': self.worker_task}), encoding='utf-8')
        # Canonical order keeps independent persistence/material/nesting
        # evidence ahead of the in-flight case. A subset is diagnostic only.
        actions = {'persistence': self.persistence, 'nested-auto': lambda: self.nested(True),
                   'nested-manual': lambda: self.nested(False), 'lifecycle': self.lifecycle}
        for name in ['persistence', 'nested-auto', 'nested-manual', 'lifecycle']:
            if name not in self.selected_cases:
                continue
            result = {'verified': False, 'started': time.monotonic()}
            self.evidence['caseResults'][name] = result
            try:
                actions[name]()
                self.model_identity()
                result['verified'] = True
            except BaseException as error:
                result['error'] = repr(error)
                raise
            finally:
                result.update(ended=time.monotonic(), coverage=json.loads(json.dumps(self.evidence['coverage'])),
                    coverageParts=json.loads(json.dumps(self.evidence['coverageParts'])))
        self.model_identity()
        self.observe_processes()
        self.evidence['selectedCasesPassed'] = all(item['verified'] for item in self.evidence['caseResults'].values()) \
            and len(self.evidence['caseResults']) == len(self.selected_cases)
        self.evidence['passed'] = self.evidence['selectedCasesPassed'] \
            and all(item['verified'] for item in self.evidence['coverage'].values())

    def persistence(self):
        self.define_map('persistence', {'work': self.node('Persistent actual material', output_rule=[{'id': 'material'}]),
            'later': self.node('Never implicitly dispatched after material changes', blocked=['work'])})
        self.run_id, self.manager = self.new_run('persistence')
        execution, self.worker = self.dispatch(self.manager, 'work', self.run_id)
        self.execution_id = execution['id']
        # Public pause only stops new dispatches; the existing node can still
        # approve, report and recover. P16 does not require manager autonomy.
        self.evidence['persistencePause'] = self.action(self.manager, 'pause', {})
        require(self.detail(self.run_id)['run']['status'] == 'paused', 'Owned persistence Run did not pause')
        self.tool(self.worker, 'approve', {})
        self.material_change(execution)
        self.worker = self.restore(self.worker, self.run_id, 'work', execution['id'])
        self.restart()

    def lifecycle(self):
        path = self.define_map('lifecycle', {'work': self.node('Original manual', False, output_rule=[{'id': 'material'}]),
            'branch': self.node('Independent branch'), 'later': self.node('Never implicitly dispatched', blocked=['work'])})
        self.run_id, self.manager = self.new_run('lifecycle')
        # This manager has no running child yet and was explicitly asked to idle.
        # Drain each idle startup before admitting another to the single-slot
        # model transport. Registration itself must remain nonblocking for collect.
        self.settled(self.manager)
        self.model_identity()
        original, self.worker = self.dispatch(self.manager, 'work', self.run_id)
        self.execution_id = original['id']
        self.settled(self.worker)
        self.model_identity()
        self.role(self.worker, ['approve', 'report'], 'ordinary')
        self.role(self.manager, ['run', 'collect'], 'root-manager')
        independent = self.independent(self.worker_task, 'Same Task but independent Chat')
        self.settled(independent)
        self.model_identity()
        unrelated_task = self.task('lifecycle-unassociated', 'normal', IDLE, self.host)
        unrelated = self.independent(unrelated_task, 'No workflow association')
        self.settled(unrelated)
        self.model_identity()
        for label, chat in [('independent-same-task', independent), ('unassociated', unrelated)]:
            self.role(chat, [], label)
            rejected = self.http('/api/workflow/node/context', {'sessionId': chat['sessionId']}, 403)
            self.evidence.setdefault('identityRejections', []).append({'label': label, 'chat': chat, 'response': rejected})
        self.http('/api/workflow/node/context', {'sessionId': self.manager['sessionId']}, 403)
        self.define_map('other', {'work': self.node('Other Run')})
        other_run, other_manager = self.new_run('other')
        self.settled(other_manager)
        self.model_identity()
        other_execution, other_chat = self.dispatch(other_manager, 'work', other_run)
        self.settled(other_chat)
        self.model_identity()
        branch_execution, branch = self.dispatch(self.manager, 'branch', self.run_id)
        self.settled(branch)
        self.model_identity()
        original_before = self.detail(self.run_id)
        other_before = self.detail(other_run)
        wire_before = len(self.backend_wire.records)
        rejected_native = self.identity_fault(other_chat, {'executionId': original['id'],
            'sessionId': self.worker['sessionId'], 'runId': self.run_id, 'nodeId': 'work', 'source': 'manual'})
        original_after, other_after = self.detail(self.run_id), self.detail(other_run)
        require(original_after == original_before, 'Extra native identity parameters switched to the other actual Run')
        actual_http = [record for record in self.backend_wire.records[wire_before:]
            if record['path'] in ['/api/workflow/node/context', '/api/workflow/node/approve']]
        if not actual_http:
            require(other_after == other_before, 'Rejected native arguments changed the caller Run')
            result = rejected_native['results'][0]
            require(result.get('isError') is True or result.get('result', {}).get('isError') is True,
                    ('Native rejection must have actual error feedback', result))
            outcome = 'rejected-before-identity-lookup'
        else:
            # A host may strip unknown fields before calling the shipped tool.
            # That is valid only if its real caller remains the sole authority.
            for record in actual_http:
                payload = json.loads(record['request'])
                require(payload.get('sessionId') == other_chat['sessionId'], 'Host accepted model-selected Session')
                if record['path'].endswith('/approve'):
                    require(payload == {'sessionId': other_chat['sessionId'], 'executionId': other_execution['id'],
                        'source': 'auto'}, 'Stripped native call selected another execution or mode')
            expected_other = json.loads(json.dumps(other_before))
            expected_execution = expected_other['run']['nodes']['work']['execution']
            expected_execution.update(approved=True, approvalSource='auto')
            require(other_after == expected_other, 'Stripped native call changed more than its own frozen approval')
            outcome = 'stripped-and-bound-only-to-actual-caller'
        self.evidence['extraIdentityIsolation'] = {'native': rejected_native, 'outcome': outcome,
            'originalBefore': original_before, 'originalAfter': original_after, 'otherBefore': other_before, 'otherAfter': other_after,
            'actualHTTPDuringInvocation': self.backend_wire.records[wire_before:],
            'expected': 'Actual native adapter rejects or strips extra fields; actual Session/frozen execution remains sole authority and the target Run never changes.'}
        for run_id, node, execution, chat in [(other_run, 'work', other_execution, other_chat),
                                             (self.run_id, 'branch', branch_execution, branch)]:
            before = self.detail(self.run_id)['run']['nodes']['work']['execution']
            require(before['approved'] is False, 'Role/context query improperly approved original execution')
            self.role(chat, ['approve', 'report'], 'other-run' if run_id == other_run else 'other-node')
            self.tool(chat, 'approve', {})
            self.approved(run_id, node, execution['id'], 'auto')
            require(self.detail(self.run_id)['run']['nodes']['work']['execution'] == before,
                    'Another native Run/node approval leaked to original')
            wrong = self.http('/api/workflow/node/approve', {'sessionId': chat['sessionId'],
                'executionId': original['id'], 'source': 'auto'}, 400)
            require(wrong['code'] == 'approval-execution-mismatch', wrong)
        self.record_part('P10', 'base-roles', ['roles', 'identityRejections', 'extraIdentityIsolation', 'nativeInvocations'])
        self.report(branch, 'Independent branch progress before the other confirmation', False)
        definition = json.loads(path.read_text(encoding='utf-8'))
        definition['nodes']['work']['auto_approved'] = True
        self.gate_marker = 'LIFECYCLE-RETRY-HOLD-' + self.root.name
        definition['nodes']['work']['input'] += '\n' + self.gate_marker
        self.gate_config.write_text(json.dumps({'marker': self.gate_marker, 'release': str(self.gate_release),
            'evidence': str(self.gate_evidence)}), encoding='utf-8')
        path.write_text(json.dumps(definition, ensure_ascii=False), encoding='utf-8')
        self.evidence['changedMap'] = self.http('/api/scheduler/maps/lifecycle.json')
        self.worker = self.restore(self.worker, self.run_id, 'work', original['id'])
        require(self.detail(self.run_id)['run']['nodes']['work']['execution']['autoApproved'] is False,
                'Recovery consumed current Map instead of frozen execution')
        self.race(original, branch, other_run)
        current = self.detail(self.run_id)['run']['nodes']['work']['execution']
        self.execution_id = current['id']
        self.worker = self.track(current['chat'])
        require(current['autoApproved'] is True, 'Legal retry did not consume changed current definition')
        self.tool(self.worker, 'approve', {})
        self.approved(self.run_id, 'work', current['id'], 'auto')
        self.evidence['coverage']['P3'] = {'verified': True, 'expected': 'Recovered original execution asks native manual confirmation; new legal retry approves automatically without UI.', 'evidence': ['changedMap', 'nativeRestores', 'inflightRace', 'nativeInvocations'],
            'oldExecutionId': original['id'], 'newExecutionId': current['id'], 'oldMode': False, 'newMode': True}

    def retry_model_requests(self):
        matches = []
        for record in self.model_wire.records:
            if record.get('method') != 'POST' or not record['path'].split('?', 1)[0].rstrip('/').endswith('/chat/completions'):
                continue
            request = json.loads(record['request'])
            tools = {tool.get('function', {}).get('name') for tool in request.get('tools', [])}
            users = [message for message in request.get('messages', []) if message.get('role') == 'user']
            if tools & {'approve', 'report'} and any(self.gate_marker in message_text(message) for message in users):
                matches.append(record)
        return matches

    def gate_held(self, chat):
        path = self.gate_evidence / 'events.jsonl'
        if not path.exists():
            return None
        events = [json.loads(line) for line in path.read_text(encoding='utf-8').split('\n')[:-1] if line]
        if not events:
            return None
        require(all(event['sessionId'] == chat['sessionId'] for event in events)
                and len({event['pid'] for event in events}) == 1,
                ('Scheduler gate changed its actual native owner', events, chat))
        require(not self.gate_release.exists() and not self.retry_model_requests(),
                'New retry reached an actual business model request before scheduler release')
        starts = [event for event in events if event['state'] == 'held-before-first-business-request']
        ends = {event['sequence']: event for event in events if event['state'] != 'held-before-first-business-request'}
        require(starts and len({event['stage'] for event in starts}) == len(starts),
                ('A concurrent context invocation reused the owned gate', events))
        for previous, current in zip(starts, starts[1:]):
            require(current['stage'] == previous['stage'] + 1 and previous['sequence'] in ends
                    and ends[previous['sequence']]['monotonicMs'] <= current['monotonicMs'],
                    ('Installed context handlers were not serialized', events))
        require(not any(event['state'] in ['exhausted', 'released'] for event in ends.values()),
                'Native scheduling window ended before the required observation finished')
        return events[-1]

    def exercise(self):
        try:
            super().exercise()
        finally:
            if self.gate_config.exists() and not self.gate_release.exists():
                try:
                    self.gate_release.write_text('Owned cleanup release; not successful business evidence.', encoding='utf-8')
                except OSError as error:
                    self.evidence['cleanupErrors'].append('Owned scheduler release failed: ' + repr(error))

    def pending_still_open(self, pending):
        request = pending['request']['id']
        require(not any((self.confirmations / (request + '.' + kind + '.json')).exists()
                        for kind in ['response', 'error', 'abort']), 'Real confirmation already ended; cannot claim in-flight work')

    def race(self, original, branch, other_run):
        old_worker = self.worker
        start = len(self.native(old_worker)['branch'])
        before_confirmations = len(self.confirmation_requests())
        self.send(old_worker, '用户现在明确请求本次成果批准。只调用 approve {} 一次。返回后等候，不重试、不报告、不标失败。', 'inflight-old-approve')
        pending = self.pending(before_confirmations)
        self.evidence['inflightRace'] = {'oldExecution': original, 'oldChat': old_worker, 'pending': pending,
            'processPid': pending['request']['pid'], 'windowStarted': time.monotonic(), 'operations': []}
        race = self.evidence['inflightRace']
        operations = [('query', lambda: self.http('/api/workflow/runs/' + self.run_id, budget=2)),
            ('progress-report', lambda: self.http('/api/workflow/node/report', {'sessionId': old_worker['sessionId'],
                'body': 'P24 real confirmation remains pending', 'canComplete': False, 'outputs': []}, budget=2)),
            ('independent-branch-ready-report', lambda: self.http('/api/workflow/node/report',
                {'sessionId': branch['sessionId'], 'body': 'Independent branch ready while another node awaits native Yes',
                 'canComplete': True, 'outputs': []}, budget=2)),
            ('independent-branch-complete', lambda: self.action(self.manager, 'complete', {'nodeId': 'branch',
                'reason': 'Accept already-approved independent branch while other confirmation is pending'}, budget=2))]
        for label, operation in operations:
            self.pending_still_open(pending)
            value = operation()
            self.pending_still_open(pending)
            race['operations'].append({'label': label, 'operation': self.evidence['publicOperations'][-1], 'response': value,
                'confirmedPendingAfter': time.monotonic()})
        self.evidence['coverage']['P24'] = {'verified': True, 'evidence': 'inflightRace/operations',
            'boundedSeconds': 2, 'nativeConfirmationReturnedOnlyAfterOperations': True}
        self.action(self.manager, 'fail', {'nodeId': 'work', 'reason': 'Explicitly verified old native process is still awaiting user; new independent execution required.'})
        self.pending_still_open(pending)
        retry = self.action(self.manager, 'retry', {'nodeId': 'work', 'confirmed': True,
            'reason': 'User explicitly requests an independent retry; old UI remains in flight, no rollback of old effects.'})
        current = retry['snapshot']['nodes']['work']['execution']
        race.update(retry=retry, retryReturned=time.monotonic(), newExecution=current)
        require(current['id'] != original['id'] and current['chat']['sessionId'] != old_worker['sessionId']
            and current['approved'] is False, 'Legal retry did not produce a distinct unapproved execution')
        self.pending_still_open(pending)
        self.chat_hosts[current['chat']['sessionId']] = self.host
        held = until('actual retry native predecision gate with matching Session',
            lambda: self.gate_held(current['chat']), 5)
        self.pending_still_open(pending)
        require(held['pid'] in self.host_pids(), 'Gate record does not belong to an actual installed owned host process')
        self.chat_pids[current['chat']['sessionId']] = {held['pid']}
        race['actualWorkingWhileGated'] = self.herdr('agent', 'wait', current['chat']['paneId'],
            '--until', 'working', '--timeout', '1000')
        self.pending_still_open(pending)
        race.update(newNativeHistoryBeforeModel=self.native(current['chat']), predecisionGate=held,
            predecisionObserved=time.monotonic(), beforeOldYes=self.detail(self.run_id),
            externalBeforeOldYes=self.external(), managerHistoryBeforeOldYes=self.native(self.manager),
            wireBaselineIndex=len(self.backend_wire.records),
            schedulerScope='Controlled scheduling only; no native business decision/request/result is supplied or replaced.')
        before_yes = race['beforeOldYes']
        require(before_yes['run']['nodes']['work']['execution']['approved'] is False,
                'Initial new-worker work approved before the old Yes; not an eligible stale-confirmation scenario')
        race['newInitialReports'] = [report for report in before_yes['reports'] if report['executionId'] == current['id']]
        require(not race['newInitialReports'] and not self.retry_model_requests(),
                'Deterministically gated retry performed business work before the old Yes')
        self.decide(pending, 'Yes')
        race['oldYesReturned'] = time.monotonic()
        history = self.settled(old_worker, start)
        invoked = calls(history, 'approve', start)
        results = [message for message in messages(history, start) if message.get('role') == 'toolResult'
                   and invoked and message.get('toolCallId') == invoked[0]['id']]
        require(len(invoked) == 1 and self.business_arguments(invoked[0]) == {} and len(results) == 1
            and results[0].get('isError') is True, ('Stale native Yes must return actual rejection', results))
        submits = [record for record in self.wire('/api/workflow/node/approve')
            if json.loads(record['request']).get('sessionId') == old_worker['sessionId']]
        require(len(submits) == 1 and submits[0]['status'] == 400
            and json.loads(submits[0]['request']) == {'sessionId': old_worker['sessionId'],
                'executionId': original['id'], 'source': 'manual'}
            and json.loads(submits[0]['response'])['code'] == 'approval-execution-stale',
            ('Old Yes was rebound to retry or did not exercise the genuine stale commit', submits))
        race.update(oldNativeHistory=history, oldApprovalHTTP=submits, afterOldYes=self.detail(self.run_id))
        self.record_part('P10', 'old-execution', ['inflightRace/oldNativeHistory', 'inflightRace/oldApprovalHTTP'])
        baseline = race['afterOldYes']
        business_facts = lambda detail: {'run': detail['run'], 'reports': detail['reports']}
        require(baseline['run']['nodes']['work']['execution']['approved'] is False
            and business_facts(baseline) == business_facts(before_yes)
            and baseline['run']['nodes']['later']['status'] == 'pending', 'Old Yes changed the settled retry business baseline')
        require(self.detail(other_run)['run']['nodes']['work']['execution']['approved'] is True, 'Stale Yes changed other Run')
        started = time.monotonic()
        window = {'started': started, 'requiredSeconds': 91, 'samples': []}
        race['observation'] = window
        while time.monotonic() - started < 91:
            held = self.gate_held(current['chat'])
            require(held, 'Native predecision scheduling evidence disappeared')
            detail = self.detail(self.run_id)
            require(business_facts(detail) == business_facts(before_yes),
                    'Old Yes caused a new approval/report/completion/dispatch after the settled initial work')
            external = self.external()
            require(external == race['externalBeforeOldYes'], 'Old Yes produced another actual notification/input or external business action')
            attempts = [record for record in self.backend_wire.records[race['wireBaselineIndex']:]
                if record['path'].endswith(('/approve', '/report', '/complete', '/dispatch', '/finish', '/retry', '/fail'))]
            require(attempts == submits, 'Old Yes caused another native business attempt')
            window['samples'].append({'at': time.monotonic(), 'public': detail, 'externalCalls': external,
                'businessAttempts': attempts, 'actualNativeGate': held})
            time.sleep(0.5)
        window.update(ended=time.monotonic(), actualSeconds=time.monotonic() - started)
        require(race['retryReturned'] < race['oldYesReturned'], 'Race order was not actually observed')
        race['schedulerRelease'] = {'at': time.monotonic(), 'businessRequestsBeforeRelease': self.retry_model_requests()}
        self.gate_release.write_text('Explicit owner release after old-Yes observation.', encoding='utf-8')
        race['newInitialHistoryAfterRelease'] = self.settled(current['chat'])
        race['businessRequestsAfterRelease'] = self.retry_model_requests()
        require(race['businessRequestsAfterRelease'], 'Released real native worker never made an actual model request')
        race['publicAfterNewInitialWork'] = self.detail(self.run_id)
        gate_events = [json.loads(line) for line in (self.gate_evidence / 'events.jsonl').read_text(encoding='utf-8').splitlines()]
        starts = {event['sequence'] for event in gate_events if event['state'] == 'held-before-first-business-request'}
        ends = {event['sequence']: event for event in gate_events if event['state'] in ['released', 'advancing-to-next-pre-request-hook']}
        require(starts and starts <= ends.keys() and any(event['state'] == 'released' for event in ends.values()),
                ('A native scheduling hook was not released/completed', gate_events))
        require(all(event['elapsedMs'] < 30000 for event in ends.values()),
                ('A native scheduling hook exceeded the installed 30s handler budget', gate_events))
        require(all(event['sessionId'] == current['chat']['sessionId'] and event['pid'] == held['pid'] for event in gate_events),
                'Scheduling gate events changed native owner')
        race['nativeGateEvents'] = gate_events
        self.evidence['coverage']['P11'] = {'verified': True, 'evidence': 'inflightRace'}

    def material_change(self, execution):
        material = self.root / 'zone-worker/material.txt'
        before = self.approved(self.run_id, 'work', execution['id'], 'auto')
        versions = []
        for content, body in [(FIRST, 'P16 initial actual material'), (SECOND, 'P16 changed actual material')]:
            self.tool(self.worker, 'write', {'path': str(material), 'content': content})
            require(material.read_bytes() == content.encode(), 'Actual native file bytes differ from independent specimen')
            self.report(self.worker, body, True, [{'id': 'material', 'path': str(material)}])
            public = self.detail(self.run_id)
            current = public['run']['nodes']['work']['execution']
            require(current['id'] == execution['id'] and current['approved'] is True
                and current['approvalSource'] == 'auto' and current['autoApproved'] is True,
                ('Material changes revoked or replaced the original approval', public))
            reports = [report for report in public['reports'] if report['executionId'] == execution['id']]
            require(reports[-1]['body'] == body and reports[-1]['canComplete'] is True, 'Later actual report not publicly saved')
            # Reports legitimately notify the manager, which may explicitly
            # dispatch the consumer. P16 promises approval persistence, not an
            # idle manager or consumer rereads. Drain actual admitted work so
            # cleanup does not interrupt its original model request.
            downstream = public['run']['nodes']['later'].get('execution')
            downstream_history = None
            if downstream:
                downstream_history = self.settled(self.track(downstream['chat']))
            versions.append({'expectedUtf8': content, 'actualUtf8': material.read_text(encoding='utf-8'),
                'sha256': hashlib.sha256(material.read_bytes()).hexdigest(), 'path': str(material), 'public': public,
                'downstreamHistory': downstream_history})
        require(versions[0]['sha256'] != versions[1]['sha256'], 'No real material modification occurred')
        self.evidence['materialChange'] = {'before': before, 'versions': versions,
            'boundary': 'No assertion that already-dispatched consumers automatically consume modified bytes'}
        self.evidence['coverage']['P16'] = {'verified': True, 'evidence': 'materialChange'}

    def nested(self, auto):
        label = 'nested-auto' if auto else 'nested-manual'
        self.define_map(label + '-child', {'inside': self.node('Actual internal work', input=
            '本次inside成果为确认收到 INNER-ACK-731。先等待用户认可；可自由报告can_complete=false进度，正式ready报告须等待用户后续明确请求。只使用原生workflow工具。')})
        (self.root / 'bots' / (label + '.json')).write_text(json.dumps({'id': label, 'name': label, 'when': '',
            'plan_guide': '', 'node_guide': '', 'map_file': label + '-child.json'}), encoding='utf-8')
        child_manager_task = self.task(label + '-manager', 'manager', IDLE +
            '你同时承担内部Run管理者与外层节点执行者职责；等待用户逐项明确请求，'
            '收到报告通知只确认收到，不自主编排或调用collect。', self.host)
        path = self.define_map(label, {'call': self.node('Overall child delivery', auto, bot=label,
            manager_task_id=child_manager_task, input='业务目标：由内部inside节点确认收到INNER-ACK-731，交付该内部流程的整体成果。'
                '本段只说明流程目标；普通节点仅执行各自Business input，不承担管理者的准备、派发、complete或finish职责。')})
        parent_id, manager = self.new_run(label, IDLE +
            '本流程交付call的整体成果。等待用户逐项请求；报告通知只确认收到，不自主编排或调用collect。',
            native_management=True)
        self.tool(manager, 'run', {'action': 'prepare_inputs', 'fileInputs': [], 'confirmed': True,
            'reason': 'The owned parent Map has no required file or non-file inputs.'})
        self.tool(manager, 'run', {'action': 'dispatch', 'nodes': ['call']})
        parent_execution, dual = self.native_dispatch(parent_id, 'call')
        child_id = parent_execution['childRunId']
        child = self.detail(child_id)
        require(child['run']['parent'] == {'runId': parent_id, 'nodeId': 'call', 'executionId': parent_execution['id']}
            and child['run']['manager']['sessionId'] == dual['sessionId'], 'Actual child linkage/dual identity differs')
        self.role(dual, ['approve', 'report', 'run', 'collect'], label + '-dual-role')
        # Each explicit user request still passes through the real model and
        # installed native tool; there is no competing direct HTTP writer.
        self.tool(dual, 'run', {'action': 'prepare_inputs', 'fileInputs': [], 'confirmed': True,
            'reason': 'The owned child Map has no required file or non-file inputs.'})
        self.tool(dual, 'run', {'action': 'dispatch', 'nodes': ['inside']})
        inner_execution, inner = self.native_dispatch(child_id, 'inside')
        self.tool(inner, 'approve', {})
        internal_approved = self.detail(child_id)
        parent_before = self.detail(parent_id)
        require(internal_approved['run']['nodes']['inside']['execution']['approved'] is True
            and parent_before['run']['nodes']['call']['execution']['approved'] is False,
            'Internal approval substituted for independent overall parent approval')
        definition = json.loads(path.read_text(encoding='utf-8'))
        definition['nodes']['call']['auto_approved'] = not auto
        path.write_text(json.dumps(definition, ensure_ascii=False), encoding='utf-8')
        changed_map = self.http('/api/scheduler/maps/' + path.name)
        known_confirmations = {request['id'] for request in self.confirmation_requests(dual)}
        observation = {'case': label, 'parentRunId': parent_id, 'parentExecution': parent_execution,
            'dualChat': dual, 'childRunId': child_id, 'internalExecution': inner_execution,
            'internalApproved': internal_approved, 'parentBeforeReady': parent_before, 'changedMap': changed_map,
            'nativeYes': [], 'samples': []}
        self.evidence['cases'].append(observation)
        answered = set()
        def observe_confirmation():
            for index, request in enumerate(self.confirmation_requests(dual)):
                if request['id'] in known_confirmations or request['id'] in answered:
                    continue
                require(not auto, 'Frozen automatic mode requested a native confirmation')
                require(self.detail(child_id)['run']['status'] == 'completed',
                    'Overall approval confirmation preceded child finish')
                pending = self.pending(index, dual)
                self.decide(pending, 'Yes', dual)
                observation['nativeYes'].append(pending)
                answered.add(request['id'])
        observation['innerReadyInvocation'] = self.report(inner,
            'Internal result accepted; not overall parent approval', True, observe=observe_confirmation)
        # A real notification may already have prompted legal native work.
        # Drain it and observe its effect before requesting a still-missing step.
        self.settled(dual, observe=observe_confirmation)
        if self.detail(child_id)['run']['nodes']['inside']['status'] != 'completed':
            self.tool(dual, 'run', {'action': 'complete', 'nodeId': 'inside',
                'reason': 'The current inside execution is approved and its latest report is ready.'},
                observe=observe_confirmation)
        self.settled(dual, observe=observe_confirmation)
        if self.detail(child_id)['run']['status'] != 'completed':
            self.tool(dual, 'run', {'action': 'finish'}, observe=observe_confirmation)
        child_finished = self.detail(child_id)
        parent_after_finish = self.detail(parent_id)
        require(child_finished['run']['status'] == 'completed', 'Native child finish did not finish the child Run')
        observation['samples'].append({'at': time.monotonic(), 'child': child_finished, 'parent': parent_after_finish})
        self.settled(dual, observe=observe_confirmation)
        if not self.detail(parent_id)['run']['nodes']['call']['execution']['approved']:
            yes_start = len(self.evidence.get('nativeYes', []))
            observation['overallApprovalInvocation'] = self.tool(dual, 'approve', {}, manual=not auto)
            native_yes = self.evidence.get('nativeYes', [])[yes_start:]
            observation['nativeYes'].extend(native_yes)
            answered.update(pending['request']['id'] for pending in native_yes)
        observation['afterOverallApproval'] = self.detail(parent_id)
        self.settled(dual, observe=observe_confirmation)
        ready = [report for report in self.detail(parent_id)['reports']
            if report['executionId'] == parent_execution['id']]
        if not ready or ready[-1]['canComplete'] is not True:
            observation['overallReadyInvocation'] = self.report(dual, 'Whole child result accepted', True)
        self.settled(manager, observe=observe_confirmation)
        if self.detail(parent_id)['run']['nodes']['call']['status'] != 'completed':
            self.tool(manager, 'run', {'action': 'complete', 'nodeId': 'call',
                'reason': 'The parent execution is independently approved and its latest overall report is ready.'})
        public = self.detail(parent_id)
        require(public['run']['nodes']['call']['status'] == 'completed', 'Native parent complete did not complete call')
        observation['samples'].append({'at': time.monotonic(), 'child': self.detail(child_id), 'parent': public})
        current = public['run']['nodes']['call']['execution']
        require(current['id'] == parent_execution['id'] and current['autoApproved'] is auto
            and current['approvalSource'] == ('auto' if auto else 'manual'), 'Nested overall approval lost its frozen identity/mode')
        # Public completion precedes the native tool return and final model turn.
        # Drain both managers before identity checks or process cleanup.
        history = self.settled(dual)
        parent_history = self.settled(manager)
        approvals = calls(history, 'approve')
        require(approvals and all(self.business_arguments(call) == {} for call in approvals),
                'Overall result was not approved by the actual zero-business-argument native tool')
        submits = [record for record in self.wire('/api/workflow/node/approve')
            if json.loads(record['request']).get('sessionId') == dual['sessionId'] and record.get('status') == 200]
        require(submits and all(json.loads(record['request']) == {'sessionId': dual['sessionId'],
            'executionId': parent_execution['id'], 'source': 'auto' if auto else 'manual'} for record in submits),
            'Native overall approval selected another execution or source')
        require(auto or observation['nativeYes'], 'Manual overall approval has no actual native Yes/return')
        reports = [report for report in public['reports'] if report['executionId'] == parent_execution['id']]
        require(reports and reports[-1]['canComplete'] is True and reports[-1]['from']['sessionId'] == dual['sessionId'],
                'Missing actual ready overall report from the dual-role manager')
        completions = [record for record in self.wire('/api/workflow/manager/complete')
            if json.loads(record['request']).get('sessionId') == manager['sessionId']
            and json.loads(record['request']).get('nodeId') == 'call' and record.get('status') == 200]
        require(completions and any(self.business_arguments(call).get('action') == 'complete'
            and self.business_arguments(call).get('nodeId') == 'call' for call in calls(parent_history, 'run')),
            'Parent completion was not a separate actual native manager action')
        finishes = [record for record in self.wire('/api/workflow/manager/finish')
            if json.loads(record['request']).get('sessionId') == dual['sessionId'] and record.get('status') == 200]
        ready_reports = [record for record in self.wire('/api/workflow/node/report')
            if json.loads(record['request']).get('sessionId') == dual['sessionId']
            and json.loads(record['request']).get('canComplete') is True and record.get('status') == 200]
        require(finishes and ready_reports and all(record.get('ended') for record in finishes + submits),
                'Missing actual completed child finish/approval or ready report HTTP')
        require(any(self.business_arguments(call).get('action') == 'finish' for call in calls(history, 'run')),
                'Child finish was not an actual native manager tool call')
        require(json.loads(submits[0]['response']).get('changed') is True,
                'Child finish or internal approval substituted for independent parent approval')
        require(json.loads(completions[0]['response']).get('changed') is True,
                'Approval or report implicitly completed the parent before its manager acted')
        require(finishes[0]['ended'] <= submits[0]['started']
                and submits[0]['ended'] <= ready_reports[-1]['started']
                and all(record['started'] >= ready_reports[-1]['started'] for record in completions),
                'Native child finish, parent approval, ready report and parent completion lost causal ordering')
        confirmation_ids = {request['id'] for request in self.confirmation_requests(dual)} - known_confirmations
        require(len(confirmation_ids) == int(not auto) and len(observation['nativeYes']) == int(not auto),
                'Nested mode has an unobserved confirmation or missing actual Yes')
        approval_results = [message for message in messages(history) if message.get('role') == 'toolResult'
            and message.get('toolCallId') in {call['id'] for call in approvals}]
        require(approval_results and any(not result.get('isError') for result in approval_results),
                'Missing actual successful native overall approval result')
        observation.update(childFinished=child_finished, approvedParent=public, overallHistory=history,
            parentHistory=parent_history, childFinishHTTP=finishes, approvalHTTP=submits,
            readyReportHTTP=ready_reports, parentCompleteHTTP=completions)
        self.evidence['nestedModes']['auto' if auto else 'manual'] = {'verified': True,
            'evidence': 'cases/' + str(len(self.evidence['cases']) - 1)}
        self.record_part('P28', 'auto' if auto else 'manual', 'cases/' + str(len(self.evidence['cases']) - 1))
        self.record_part('P10', 'dual-role', ['roles', 'cases/' + str(len(self.evidence['cases']) - 1)])

    def restart(self):
        self.settled(self.worker)
        self.settled(self.manager)
        self.model_identity()
        before = self.detail(self.run_id)
        external_before = self.external()
        histories = {chat['sessionId']: self.native(chat) for chat in [self.worker, self.manager]}
        wire_start = len(self.backend_wire.records)
        old = self.backend_process
        old_pid = old.child.pid
        old.stop()
        require(old.child.poll() is not None, 'Actual backend did not exit')
        exited = time.monotonic()
        current = self.start([self.args.backend, '--config', self.root / 'backend.toml'], 'actual-backend-restart')
        def ready():
            require(current.child.poll() is None, 'New backend exited; retain raw logs')
            while not current.lines.empty():
                if self.base in current.lines.get_nowait():
                    return True
        until('actual restarted backend at same exclusive numeric loopback', ready, 45)
        require(current.child.pid != old_pid, 'Backend PID did not change')
        after = self.detail(self.run_id)
        require(after['run'] == before['run'] and after['reports'] == before['reports'] and after['managed'] is False,
                'Backend restart lost approval/frozen mode or reclaimed old Run')
        histories_after = {chat['sessionId']: self.native(chat) for chat in [self.worker, self.manager]}
        business_history = lambda history: [message for message in messages(history)
            if message.get('role') in ['user', 'assistant', 'toolResult']]
        for session_id, history in histories.items():
            require(business_history(histories_after[session_id]) == business_history(history),
                    'Backend restart replayed native business input')
        self.tool(self.worker, 'approve', {}, error=True)
        self.tool(self.manager, 'run', {'action': 'status'}, error=True)
        require(self.detail(self.run_id)['run'] == before['run'], 'Old native identity reclaimed old Run')
        business = [record for record in self.backend_wire.records[wire_start:]
            if record['path'].endswith(('/approve', '/report', '/dispatch', '/complete', '/finish')) and record.get('status') == 200]
        require(not business, ('Backend restart replayed business HTTP', business))
        require(self.external() == external_before, 'Backend restart replayed actual external input or startup')
        self.evidence['backendRestart'] = {'oldPid': old_pid, 'newPid': current.child.pid, 'oldExited': True,
            'exitObserved': exited, 'ready': time.monotonic(), 'before': before, 'after': after,
            'historiesBefore': histories, 'historiesAfter': histories_after,
            'publicHistoryAfter': self.http('/api/sessions/history'),
            'postRestartHTTP': self.backend_wire.records[wire_start:],
            'externalBefore': external_before, 'externalAfter': self.external()}
        self.evidence['coverage']['P14'] = {'verified': True, 'evidence': ['nativeRestores', 'nativeExits', 'backendRestart'],
            'oldRunReclaimed': False, 'businessInputReplayed': False}
