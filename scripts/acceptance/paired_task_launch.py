"""S1 product HTTP and built page, S2 public subscription, S3 installed terminals.

The startup witness pauses a public native hook; it does not own submission.
A too-early ready, a forced modal answer, or a replay can therefore still cause
an observable violation in the real CLI, editor, provider or native journal.
"""
import copy
import json
from pathlib import Path
import shutil
import threading
import time
import tomllib
import traceback
import urllib.request
import uuid

from .host.processes import require, until
from .host.provenance import digest
from .host.wire import WireProxy, decode
from .paired_history import ANSWER, INITIAL, SKILL, Recovery, dump, same_path

NAMES = ('bot-playground', 'paired-launch-alternate')
MARKER = 'PAIRED-TASK-BUSINESS-528: literal "quoted" & text'
TEMPLATE_COMMAND = '/paired-native-template "paired argument"'
TEMPLATE_EXPANDED = 'PAIRED TEMPLATE BODY: [paired argument]'
ZONE = 'paired-owned'
TITLE = 'Paired Task startup modal'
QUESTION = 'Close this test confirmation without answering the Task.'


def read_rows(file):
    if not file.is_file():
        return []
    raw = file.read_text(encoding='utf-8')
    return [json.loads(row) for row in raw[:raw.rfind('\n') + 1].splitlines()]


def user_rows(entries):
    # OMP persists a native Skill invocation as a displayed skill-prompt entry;
    # Pi persists the expanded content as the native user message.
    return [row for row in entries if
            (row.get('type') == 'message' and row.get('message', {}).get('role') == 'user') or
            (row.get('type') == 'custom_message' and row.get('customType') == 'skill-prompt' and row.get('display') is True)]


def text_parts(value):
    if isinstance(value, str):
        yield value
    elif isinstance(value, dict):
        for child in value.values():
            yield from text_parts(child)
    elif isinstance(value, list):
        for child in value:
            yield from text_parts(child)


def contains_markers(value, *markers):
    text = '\n'.join(text_parts(value))
    return all(marker in text for marker in markers)


class Launch(Recovery):
    def __init__(self, args, label, combination):
        agent, host = combination or ('pi', 'app' if label.startswith('app-') or label in ('foreign-core', 'alternate-app') else 'tui')
        super().__init__(args, dict(agent=agent, direction=f'{host}-to-{host}', scenario=label, promises='paired Task launch'))
        self.label = label
        self.host_kind = host
        self.phase = 'origin'
        self.args = copy.copy(args)
        self.args.roots = args.roots.copy()
        if host == 'app':
            for name in ('omp', 'pi'):
                self.args.roots[f'{name}_state_extension'] = self.root / f'absent-{name}-tui-state.ts'
        self.trace = self.root / 'launch-probe.jsonl'
        self.template = label.startswith('alternate-')
        self.modal = '-modal-' in label
        self.hold = combination is not None or self.modal or label == 'app-ambiguous-start'
        self.page_expected = label in ('pi-app', 'app-ambiguous-start') or (self.modal and label.endswith(('release', 'twice')))
        self.page = None
        self.step = 0
        self.wire = None
        self.allow_ready = threading.Event()
        self.env.update(PAIRED_AGENT=agent, PAIRED_TRACE=str(self.trace), PAIRED_HOLD=str(int(self.hold)),
                        PAIRED_RELEASE=str(self.root / 'startup-release'), PAIRED_MODAL=str(int(self.modal)),
                        PAIRED_HISTORY_HOLD=str(int(label == 'app-ambiguous-start')),
                        PAIRED_HISTORY_RELEASE=str(self.root / 'history-release'))
        self.evidence.update(label=label, scope='S1 product HTTP and built page; S2 public subscription; S3 installed host',
                             statusSamples=[], scenarios=[])

    def preset(self, variant):
        task = dict(super().preset(variant), chatName='Task {zone.name} | {task.name} | {agent} | {chat.id}')
        if self.template:
            task.update(skill=None, prompt=TEMPLATE_COMMAND)
        return task

    def configure(self):
        super().configure()
        probe = self.deployment / 'paired-task-launch-probe.mjs'
        scripts = self.args.herdr_bot_worktree / 'scripts'
        shutil.copy2(scripts / 'fixtures/paired-task-launch-probe.mjs', probe)
        self.evidence['acceptanceSourceSha256'] = {path.name: digest(path) for path in (
            Path(__file__), probe, scripts / 'paired-task-launch-browser.mjs')}
        suites = json.loads(self.suites_path.read_text(encoding='utf-8'))
        suite = suites['suites'][0]
        suite['extensions'][self.agent].append(str(probe))
        if self.template:
            template = self.root / 'paired-native-template.md'
            template.write_text('---\ndescription: Owned launch probe\n---\nPAIRED TEMPLATE BODY: [$1]\n', encoding='utf-8')
            suite['prompts'] = [str(template)]
        dump(self.suites_path, suites)
        for kind, path in self.recorders.items():
            config = path.parent / 'recovery-proxy.json'
            dump(config, dict(json.loads(config.read_text(encoding='utf-8')), recordSubmission=True,
                              startedFalse=(kind == 'core' and self.label == 'app-ambiguous-start')))
        if self.host_kind == 'app':
            require(all(not self.args.roots[f'{agent}_state_extension'].exists() for agent in ('omp', 'pi')) and
                    not list(self.agent_dir.rglob('*herdr*state*')), 'App fixture must not have an installed TUI state extension')

    def note(self, event, **details):
        self.evidence['scenarios'].append(dict(event=event, at=time.monotonic(), **details))

    def launch_wire(self):
        def before_forward(record):
            if record['path'] == '/api/sessions/events' and (decode(record, 'request') or {}).get('kind') == 'launch_ready':
                require(self.allow_ready.wait(18), 'Ready transport was never released by the terminal observer')
        self.wire = WireProxy(self.base, self.root / f'{self.phase}-session-wire', before_forward=before_forward)
        (self.deployment / 'extensions/connect-bot/config.yaml').write_text(
            f'endpoint: "{self.wire.base}"\nrequestTimeoutMs: 23000\n', encoding='utf-8')

    def messages(self, kind):
        return [(row, event) for row in list(self.wire.records) if row['path'] == '/api/sessions/events' and
                (event := decode(row, 'request')) and event.get('kind') == kind]

    def probe_rows(self, kind, session=None):
        return [row for row in read_rows(self.trace) if row['kind'] == kind and (session is None or row.get('sessionId') == session)]

    def one_probe(self, kind, session=None):
        require(not self.probe_rows('native-confirmation-error'), self.probe_rows('native-confirmation-error'))
        values = self.probe_rows(kind, session)
        return values[0] if values else None

    def inputs(self):
        kind = 'tui' if self.host_kind == 'tui' else 'core'
        return [json.loads(file.read_text(encoding='utf-8')) for file in sorted(self.recorders[kind].parent.joinpath('submissions').glob('*.json'))]

    def attempt_count(self):
        commands = self.calls_since({})
        return sum(args['argv'][:2] == ['agent', 'prompt'] for args in commands['tui']) + sum(
            args['argv'][:1] == ['send'] for args in commands['core'])

    def status(self, stage=None, seconds=20):
        def poll():
            state = self.http('/api/chats/' + self.creation)
            if not self.evidence['statusSamples'] or self.evidence['statusSamples'][-1]['body'] != state:
                self.evidence['statusSamples'].append(dict(at=time.monotonic(), body=state))
            if stage is not None:
                require(stage == state['stage'] or state['stage'] not in ('failed', 'succeeded'), ('unexpected terminal status', stage, state))
            return state if stage in (None, state['stage']) else None
        return until(f'public Task creation stage {stage}', poll, seconds=seconds)

    def start_creation(self, placement):
        zones = self.http('/api/zones')['zones']
        if not any(zone['id'] == ZONE for zone in zones):
            self.http('/api/zones', {'id': ZONE, 'path': str(self.origin)})
        available = self.http('/api/tasks')['tasks']
        task = next((task for task in available if task['name'] == 'paired-source'), None)
        if task is None:
            task = self.http('/api/tasks', self.preset('before'))['tasks'][-1]
        self.task = task['id']
        initial_input = '' if self.template else MARKER
        if self.page_expected:
            script = self.args.herdr_bot_worktree / 'scripts/paired-task-launch-browser.mjs'
            config = self.root / 'browser-config.json'
            dump(config, dict(base=self.base, task=self.task, workspace=placement, chrome=str(self.args.roots['chrome']),
                              profile=str(self.root / 'chrome-profile'), dir=str(self.root), input=initial_input))
            self.page = self.start([self.args.roots['node'], script, config], 'browser')
            accepted = until('real page Task POST', lambda: self.browser_file('accepted'), seconds=25)
        else:
            preview = self.http('/api/chats/preview', {'zone': ZONE, 'taskId': self.task})
            require(preview['name'] == self.name(preview['chatId']), preview)
            accepted = self.http('/api/chats', {'zone': ZONE, 'taskId': self.task, 'chatId': preview['chatId'],
                                               'expectedName': preview['name'], 'workspaceTarget': placement, 'input': initial_input}, status=202)
        self.creation = accepted['id']
        require(accepted['name'] == self.name(accepted['name'].rsplit(' | ', 1)[1]), accepted)
        self.evidence['accepted'] = accepted

    def name(self, short_id):
        require(len(short_id) == 6 and set(short_id) <= set('0123456789abcdef'), ('invalid Chat naming token', short_id))
        return f'Task zone | paired-source | {self.agent} | {short_id}'

    def browser_file(self, name):
        file = self.root / f'browser-{name}.json'
        if file.is_file():
            return json.loads(file.read_text(encoding='utf-8'))
        if self.page.child.poll() is not None:
            raise AssertionError(('packaged page driver exited', self.page.child.poll(), self.browser_file('result') if name != 'result' else None))

    def browser(self, action, expected=None):
        if not self.page:
            return
        file = self.root / f'browser-step-{self.step}.json'
        pending = file.with_suffix('.tmp')
        dump(pending, dict(action=action, expected=expected))
        pending.rename(file)
        if action == 'done':
            result = until('packaged page and owned browser exit evidence', lambda: self.browser_file('result'), seconds=35)
            require(self.page.child.wait(timeout=10) == 0 and result['passed'] and len(result['creates']) == 1, result)
        else:
            result = until(f'packaged page {action}', lambda: self.browser_file(f'ack-{self.step}'), seconds=13)
            require(result['creates'] == 1, result)
        self.step += 1

    def screen(self, target):
        return self.command([self.args.herdr, 'pane', 'read', target['paneId'], '--source', 'visible', '--format', 'text']) if self.host_kind == 'tui' else self.command([self.args.core, 'read', self.terminal(target)])

    def terminal(self, target):
        layout = self.native(self.args.app, 'ls')
        space = next(space for space in layout['spaces'] if space['id'] == target['workspaceId'])
        tab = next(tab for tab in space['tabs'] if tab['id'] == target['tabId'])
        return next(pane['terminal'] for pane in tab['terminals'] if pane['pane'] == target['paneId'])

    def operator_key(self, target, key):
        if self.host_kind == 'tui':
            self.herdr('pane', 'send-keys', target['paneId'], key)
        else:
            self.native(self.args.core, 'keys', '--json', self.terminal(target), key)

    def no_native_business(self, native):
        file = Path(native['file']) if native.get('file') else None
        native_rows = read_rows(file) if file else []
        require(not user_rows(native_rows) and not self.model.requests and
                not self.probe_rows('native-confirmation-result'), ('input entered modal or journal', native_rows, self.model.requests))
        self.note('no-native-user-or-confirmation-result', nativeRecords=native_rows, providerRequests=len(self.model.requests))

    def assert_negative_identity(self, native, good):
        # Preserve the real production wire payload; vary only one externally supplied identity at a time.
        for kind, body in (
            ('wrong-target', {**good, 'data': {**good['data'], 'targetId': 'paired-wrong-target'}}),
            ('wrong-owner-pid', {**good, 'data': {**good['data'], 'pid': self.host_pid}}),
            ('wrong-session', {**good, 'sessionId': f'unrelated-{native["id"]}'}),
        ):
            status, reply = self.request('POST', '/api/sessions/events', body, expected=None)
            self.note('controlled-event', fault=kind, request=body, status=status, response=reply)
            require(status == 409, ('unrelated launch ready should be rejected', kind, status, reply))
            require(self.attempt_count() == 0, ('unrelated launch event unlocked initial submission', kind))

    def observe_start(self):
        start = until('explicit native startup facts', lambda: self.one_probe('native-session-start'), seconds=15)
        self.remember(self.phase, start['pid'], 'installed-' + self.agent)
        require(start['model'] == 'launch-only' and start['provider'] == 'paired-owned' and
                start['thinking'] == 'high' and same_path(start['cwd'], self.origin), start)
        session = start['sessionId']
        if self.hold:
            gate = 'omp-native-gate' if self.agent == 'omp' else 'pi-native-initializing'
            edge = until('real startup held', lambda: self.one_probe(gate, session), seconds=10)
            require(self.agent != 'omp' or edge.get('disabled') is True, edge)
            # Allow the actual adapter to emit if it confuses idle/registration with input readiness.
            time.sleep(0.35)
            require(not self.probe_rows('session_ready', session), ('premature semantic ready', read_rows(self.trace)))
            if self.agent == 'omp':
                require(not self.messages('launch_ready'), 'production connect-bot advertised while OMP gate closed')
            require(self.attempt_count() == 0, 'backend delivered input during native initialization')
            self.note('native-start-held', edge=edge, public=self.status())
            Path(self.env['PAIRED_RELEASE']).touch(exist_ok=False)
        return start

    def assert_ready(self, native, target, start):
        session = native['id']
        require(start['pid'] == native['pid'] and start['sessionId'] == session, (start, native))
        semantic = until('public framework session_ready', lambda: self.one_probe('session_ready', session), seconds=20)
        if self.hold:
            release = self.one_probe('native-hook-released', session)
            require(release and release['unixMs'] <= semantic['unixMs'], (release, semantic))
        row, good = until('actual connect-bot launch_ready', lambda: next(iter(self.messages('launch_ready')), None), seconds=20)
        require(good['sessionId'] == session and good['data']['pid'] == native['pid'] and
                good['data']['agentType'] == self.agent and same_path(good['data']['cwd'], self.origin) and
                good['data']['launchId'] == start['envLaunch'] and good['data']['targetId'] == start['envTarget'], good)
        state = self.status('initializing')
        require(not state['inputSubmitted'] and not self.attempt_count(), state)
        self.browser('initializing', '正在初始化输入')
        if self.modal:
            opened = until('native modal opened after the ready event', lambda: self.one_probe('native-confirmation-opened', session), seconds=10)
            screen = until('native confirmation visibly pending', lambda: self.modal_screen(target), seconds=10)
            require(opened['unixMs'] >= semantic['unixMs'], (opened, semantic))
            self.note('modal-visible', native=opened, screen=screen)
        if self.hold and self.agent == 'pi':
            self.assert_negative_identity(native, good)
        self.allow_ready.set()
        until('production accepted ready over HTTP', lambda: 'ended' in row, seconds=20)
        require(row.get('status') == 200, row)
        return good

    def modal_screen(self, target):
        screen = self.screen(target)
        return screen if TITLE in screen and QUESTION in screen and 'Yes' in screen and 'No' in screen else None

    def success(self, native, target, *, browser=True):
        state = self.status('succeeded', seconds=85)
        require(state['inputSubmitted'] is True and 'inputOutcome' not in state, state)
        if browser:
            self.browser('succeeded', '创建成功，初始指示已提交')
        self.verify_native_input(native)
        self.assert_sources(native, {'configured', 'suite-before', 'fixed-old'})
        end = until('independent public connect-bot registration state', lambda: self.panel(native['id']), seconds=30)
        public = self.public_history(native['id']) if self.label != 'backend-switch' else next(
            row for row in self.http('/api/sessions/history')['sessions'] if row['sessionId'] == native['id'])
        require(public['originalWorkspaceId'] == target['workspaceId'] and public['agentType'] == self.agent, public)
        self.note('product-and-native-input-complete', public=public, panel=end)

    def panel(self, session):
        matches = [(row, event) for row, event in self.messages('state') if event['sessionId'] == session and row.get('status') == 200]
        if not matches:
            return None
        request = urllib.request.Request(self.base + '/api/sessions/events', headers={'Accept': 'text/event-stream'})
        with urllib.request.build_opener(urllib.request.ProxyHandler({})).open(request, timeout=5) as response:
            require(response.headers.get_content_type() == 'text/event-stream', 'public session endpoint did not return SSE')
            event, data, lines = None, [], []
            for raw in response:
                if not raw.strip():
                    break
                line = raw.decode('utf-8').rstrip('\r\n')
                lines.append(line)
                field, _, value = line.partition(':')
                if field == 'event':
                    event = value.strip()
                if field == 'data':
                    data.append(value.removeprefix(' '))
            require(event == 'snapshot' and data, lines)
            snapshot = json.loads('\n'.join(data))
        row = next((row for row in snapshot['sessions'] if row['sessionId'] == session), None)
        require(row and row['agentType'] == self.agent and row['model'] == 'launch-only' and
                row['online'] is True and same_path(row['cwd'], self.origin), snapshot)
        self.note('public-session-snapshot', raw=lines)
        return dict(report=matches[-1][1], public=row)

    def verify_native_input(self, native):
        def persisted():
            file = Path(native['file']) if native.get('file') else self.session_file(native['id'])
            if file is None or not file.is_file() or ANSWER not in file.read_text(encoding='utf-8'):
                return None
            return file
        file = until('actual installed native history written with mechanical reply', persisted, seconds=40)
        shutil.copy2(file, self.root / 'native-persisted.jsonl')
        result = self.manager(file)
        markers = (TEMPLATE_EXPANDED,) if self.template else (MARKER, INITIAL, SKILL)
        business = user_rows(result['branch'])
        require(result['id'] == native['id'] and len(business) == 1 and contains_markers(business[0], *markers) and
                contains_markers(result, ANSWER), ('native template/Skill expansion and initial input', result))
        # OMP title/prediction side channels may re-use the completed round.
        # Count new business requests, not title requests or requests containing the existing reply.
        provider = [request for request in self.model.requests if
                    contains_markers(request['body']['messages'], *markers) and
                    not contains_markers(request['body']['messages'], ANSWER)]
        require(len(provider) == 1 and provider[0]['body']['model'] == 'launch-only', provider)
        self.note('native-session-manager', facts=result, actualFile=str(file))
        attempts = self.inputs()
        expected_input = TEMPLATE_COMMAND if self.template else f'/skill:paired-initial {INITIAL}\n\n{MARKER}'
        require(attempts and '--force' not in attempts[-1]['argv'] and attempts[-1]['argv'][-1] == expected_input, attempts)

    def session_file(self, session):
        return next((path for path in self.agent_dir.rglob('*.jsonl') if any(row.get('type') == 'session' and row.get('id') == session
                     for row in read_rows(path)[:1])), None)

    def quiet(self, attempts, seconds=6):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            require(self.attempt_count() == attempts, ('extra external submission', attempts, self.calls_since({})))
            time.sleep(0.1)
        self.note('quiet-window-complete', attemptCount=attempts, seconds=seconds)

    def assert_rejection(self, receipt):
        args = receipt['argv']
        require('--force' not in args and receipt['exit'] != 0, receipt)
        error = json.loads(receipt['stdout'] or receipt['stderr'])['error']
        require(error.get('code') == 'agent_blocked' if self.host_kind == 'tui' else error.get('kind') == 'blocked', receipt)

    def dismiss(self, native, target):
        self.operator_key(target, 'esc')
        outcome = until('native modal dismissed with Escape', lambda: self.one_probe('native-confirmation-result', native['id']), seconds=6)
        require(outcome['result'] is False, outcome)
        self.note('operator-dismissed-modal', native=outcome)

    def modal_outcome(self, native, target, good):
        waiting = self.status('retrying', seconds=85)
        require(waiting['inputOutcome'] == 'not-submitted' and not waiting['inputSubmitted'] and waiting.get('retryReason'), waiting)
        first = until('actual blocked native receipt', lambda: next(iter(self.inputs()), None), seconds=5)
        self.assert_rejection(first)
        self.no_native_business(native)
        require(self.modal_screen(target), 'initial instruction dismissed or answered real modal')
        self.browser('retrying', '初始指示未提交，5 秒后重试一次')
        ending = self.label.rsplit('-', 1)[1]
        if ending == 'release':
            self.dismiss(native, target)
            for kind in ('launch_ready', 'launch_identity', 'launch_ready'):
                self.http('/api/sessions/events', {**good, 'kind': kind})
            self.browser('refresh')
            self.success(native, target, browser=False)
        elif ending == 'twice':
            failed = self.status('failed', seconds=18)
            require(failed['inputOutcome'] == 'not-submitted' and failed['failure']['message'], failed)
            self.browser('failed', '初始指示未提交，不会自动重发')
            self.no_native_business(native)
            require(self.modal_screen(target), 'second attempt answered native modal')
        elif ending == 'exit':
            if self.host_kind == 'tui':
                self.herdr('tab', 'close', target['tabId'])
            else:
                self.native(self.args.app, 'do', 'tab.close', json.dumps({'id': target['tabId'], 'confirmed': True}))
            owned = self.handles[self.phase, native['pid']][1]
            until('original Task process really exited', lambda: not owned.alive(), seconds=12)
        elif ending == 'replace':
            self.dismiss(native, target)
            self.native(self.args.core, 'send', '--json', self.terminal(target), '--', '/new')
            replacement = until('actual native replacement session', lambda: next((row for row in read_rows(self.trace)
                if row['kind'] in ('native-session-start', 'native-session-switch') and row['sessionId'] != native['id']), None), seconds=10)
            status, body = self.request('POST', '/api/sessions/events', good, expected=None)
            require(status == 409, ('old session ready was accepted after native replacement', status, body))
            self.note('native-replacement', native=replacement, staleReady={'status': status, 'body': body})
        if ending in ('exit', 'replace'):
            failed = self.status('failed', seconds=18)
            require(failed['inputOutcome'] == 'not-submitted' and failed['failure']['code'] == 'initial-input-retry-cancelled', failed)
            require(not self.model.requests, self.model.requests)
            self.quiet(1)
        else:
            later = self.inputs()
            require(len(later) == 2 and later[0]['argv'] == later[1]['argv'] and later[1]['beganMs'] - first['finishedMs'] >= 5000,
                    ('retry was not exactly once on original target at least 5 seconds after rejection', later))
            if ending == 'twice':
                self.assert_rejection(later[1])
            self.quiet(2)

    def unknown(self, native):
        state = self.status('failed', seconds=85)
        require(state['inputOutcome'] == 'unknown' and 'started:false' in state['failure']['message'] and not state.get('retryReason'), state)
        self.browser('unknown', '初始指示的投递结果未确认')
        pending = until('real native before-agent-start after send', lambda: self.one_probe('native-input-pending', native['id']), seconds=8)
        file = self.session_file(native['id'])
        rows = [] if file is None else read_rows(file)
        require(not self.model.requests, 'controlled provider received work before the history release boundary')
        self.note('independent-native-history-at-unknown', pending=pending, users=user_rows(rows))
        Path(self.env['PAIRED_HISTORY_RELEASE']).touch(exist_ok=False)
        self.verify_native_input(native)
        receipts = self.inputs()
        require(len(receipts) == 1 and receipts[0]['injectedStartedFalse'] and receipts[0]['exit'] == 0, receipts)
        self.quiet(1)

    def start_host_pid(self):
        return next(pid for (phase, pid), (kind, _) in self.handles.items()
                    if phase == self.phase and kind in ('tui-server', 'core'))

    def observe_inventory(self, before, expected, target):
        after = self.inventory(self.host_kind)
        removed = self.modal and self.label.endswith('-exit')
        exact = {key: sorted(values | ({target['tabId']} if key == expected and not removed else set())) for key, values in before.items()}
        require({key: sorted(values) for key, values in after.items()} == exact, ('unexpected native tabs or replacement process', before, after, target))
        external = self.calls_since({})
        if self.host_kind == 'app':
            require(not external['tui'] and sum(row['argv'][:2] == ['do', 'tab.new'] for row in external['app']) == 1, external)
        else:
            require(not external['app'] and not external['core'] and sum(row['argv'][:2] == ['tab', 'create'] for row in external['tui']) == 1 and
                    sum(row['argv'][:2] == ['agent', 'start'] for row in external['tui']) == 1, external)
        self.note('native-tab-delta', before={key: sorted(items) for key, items in before.items()},
                  after={key: sorted(items) for key, items in after.items()}, expected=target)

    def launch_experiment(self, configured_name, placement):
        spaces = {name: self.new_container(self.host_kind, name, self.default_cwd) for name in (*NAMES, self.origin.name)}
        expected = spaces[self.origin.name if placement == 'current' else configured_name]
        before = self.inventory(self.host_kind)
        self.launch_wire()
        try:
            self.start_creation(placement)
            startup = self.observe_start()
            native = self.start_event('configured', self.phase)
            self.host_pid = self.start_host_pid()
            view = until('public target for real native Task', lambda: (view if view.get('paneId') else None) if (view := self.status()) else None, seconds=12)
            require(view['stage'] != 'failed' and same_path(native['cwd'], self.origin), (view, native))
            target = dict(workspaceId=expected, tabId=view['tabId'], paneId=view['paneId'])
            if self.host_kind == 'tui':
                tabs = self.herdr('tab', 'list', '--workspace', expected)['tabs']
                title = next(tab['label'] for tab in tabs if tab['tab_id'] == view['tabId'])
            else:
                layout = self.native(self.args.app, 'ls')
                space = next(space for space in layout['spaces'] if space['id'] == expected)
                title = next(tab['name'] for tab in space['tabs'] if tab['id'] == view['tabId'])
            require(title == self.evidence['accepted']['name'], (title, self.evidence['accepted']))
            self.observe_host(self.host_kind, self.phase, target, native, focused=False)
            good = self.assert_ready(native, target, startup)
            if self.modal:
                self.modal_outcome(native, target, good)
            elif self.label == 'app-ambiguous-start':
                self.unknown(native)
            else:
                self.success(native, target)
                self.quiet(1, seconds=0.65)
            self.observe_inventory(before, expected, target)
        finally:
            self.allow_ready.set()
            for key in ('PAIRED_RELEASE', 'PAIRED_HISTORY_RELEASE'):
                Path(self.env[key]).touch(exist_ok=True)
            if self.page:
                try:
                    self.browser('done')
                except Exception:
                    self.evidence['cleanupErrors'].append(traceback.format_exc())
                finally:
                    try:
                        self.page.stop()
                    except Exception:
                        self.evidence['cleanupErrors'].append(traceback.format_exc())
                    self.processes.remove(self.page)

    def wrong_core(self):
        selected = self.new_container('app', NAMES[0], self.origin)
        opened = self.native(self.args.app, 'do', 'tab.new', json.dumps({'space': selected, 'name': 'independent App witness', 'focus': False}))
        original = self.env
        foreign = dict(original, HC_SESSION='hb5-foreign-' + uuid.uuid4().hex)
        self.env = foreign
        try:
            started = self.native(self.args.core, 'server', 'start', '--json')
            self.core_status('foreign')
            # Allocate owned terminals until the other core exposes the same short address.
            for _ in range(6):
                witness = self.native(self.args.core, 'run', '--bg', '--json', '--label', 'herdr.space=other', '--label', 'herdr.tab=other')
                if witness['address'] == opened['terminal']:
                    break
            else:
                raise AssertionError(('native core address collision not constructed', opened, witness))
            before = self.native(self.args.core, 'ls', '--all', '--json')
            layout = self.inventory('app')
            require(started['core']['id'] != self.evidence['selectedCore'], started)
            with self.backend('app', NAMES[0], 'mismatch'):
                self.start_creation('default')
                failure = self.status('failed')
            require(failure['failure']['stage'] == 'tab' and 'App/core 实例不一致' in failure['failure']['message'] and 'tabId' not in failure, failure)
            require(self.inventory('app') == layout and self.native(self.args.core, 'ls', '--all', '--json') == before and
                    not self.events() and not self.model.requests and not self.attempt_count(), 'wrong core mutated an owned terminal or started work')
            self.note('independent-core-collision-rejected', app=opened, foreign=witness, core=started, state=failure)
        finally:
            self.env = foreign
            try:
                self.core_status('foreign')
                self.native(self.args.core, 'server', 'stop', '--json')
            except Exception:
                self.evidence['cleanupErrors'].append(traceback.format_exc())
            self.evidence['cleanupErrors'].extend(self.exit_confirmation('foreign'))
            self.env = original

    def selected_host(self, selection, configured_name, placement):
        try:
            with self.host(self.host_kind, self.phase):
                with self.backend(selection, configured_name, self.phase):
                    self.launch_experiment(configured_name, placement)
        finally:
            if self.wire:
                self.wire.close()
                self.evidence.setdefault('wire', {})[self.phase] = self.wire.records
                self.wire = None

    def switch(self):
        self.selected_host('tui', NAMES[0], 'default')
        require(not self.evidence['cleanupErrors'], 'first native host must really exit before the backend-only switch')
        # Keep deployment, Task, paths and default name unchanged, but preserve
        # separate raw observations for two different real processes/terminals.
        first = dict(accepted=self.evidence['accepted'], receipts=self.inputs(), events=read_rows(self.trace),
                     provider=list(self.model.requests), calls=self.calls_since({}))
        dump(self.root / 'before-backend-switch.json', first)
        for leaf in (self.trace, self.events_path, self.root / 'native-persisted.jsonl'):
            leaf.rename(leaf.with_name('tui-' + leaf.name))
        for path in self.recorders.values():
            for name in ('calls', 'submissions'):
                source = path.parent / name
                if source.exists():
                    source.rename(path.parent / ('tui-' + name))
        self.model.requests.clear()
        self.allow_ready.clear()
        self.host_kind, self.phase = 'app', 'switch'
        self.selected_host('app', NAMES[0], 'default')
        original, switched = [tomllib.loads((self.root / f'backend-{phase}.toml').read_text(encoding='utf-8')) for phase in ('origin', 'switch')]
        original.pop('listen_addr')
        switched.pop('listen_addr')
        require(original['herdr'].pop('backend') == 'tui' and switched['herdr'].pop('backend') == 'app' and
                original == switched, ('only the backend may vary within the experiment', original, switched))
        self.note('only-herdr-backend-changed', roles=switched['herdr'], reusedTask=self.task)

    def exercise(self):
        self.configure()
        if self.label == 'foreign-core':
            with self.host('app', self.phase):
                self.evidence['selectedCore'] = self.core_status(self.phase)
                self.wrong_core()
        elif self.label == 'backend-switch':
            self.switch()
        else:
            configured_name = NAMES[int(self.label.startswith('alternate-'))]
            selection = None if self.label == 'legacy-tui' else self.host_kind
            placement = 'current' if self.label in ('omp-tui', 'pi-app') else 'default'
            self.selected_host(selection, configured_name, placement)

    def run(self, target):
        evidence = super().run(target)
        try:
            for pattern in ('browser-*.png', 'paired-native-template.md'):
                for path in self.root.glob(pattern):
                    shutil.copy2(path, target / path.name)
            for directory in self.root.glob('*-session-wire'):
                shutil.copytree(directory, target / directory.name)
            dump(target / 'native-submission-receipts.json', self.inputs())
            dump(target / 'native-installed-events.json', read_rows(self.trace))
        except Exception:
            evidence['cleanupErrors'].append(traceback.format_exc())
            evidence['passed'] = False
        dump(target / 'evidence.json', evidence)
        return evidence
