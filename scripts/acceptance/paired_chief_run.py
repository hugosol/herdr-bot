"""S1 public business operations; S2 actual role extensions; S3 installed terminal effects."""
import json
from pathlib import Path
import re
import threading
import time
import traceback
from urllib.parse import quote
from urllib.request import Request, urlopen
import uuid

from .host.processes import require, until
from .paired_close_reminders import Scenario, save
from .paired_reminder_observation import owned_window, launch_tui_client


ZONE = 'hb14/zone'
MARK = 'HB14-'
REPORT_FAILURE = 'HB14 controlled notification refusal'
FOCUS_FAILURE = 'HB14 controlled focus refusal'
# Two distinct paths must not be conflated: launch can retry before write, while
# reports/focus cannot. Inspect the advertised external boundary, not Rust calls.
STARTS = {'tui': ('agent', 'start'), 'app': ('do', 'tab.new')}
FOCUS = {'tui': ('agent', 'focus'), 'app': ('do', 'pane.focus')}
GATES = {'tui': (('agent', 'prompt'), FOCUS['tui']), 'app': (FOCUS['app'],), 'core': (('send', '--json'),)}


def read(path):
    return json.loads(path.read_text(encoding='utf-8'))


def terminal_input(host, argv):
    if host == 'tui' and argv[:2] == ['agent', 'prompt'] and len(argv) == 4:
        return argv[2], argv[3]
    if host == 'core' and argv[:2] == ['send', '--json'] and len(argv) == 5 and argv[3] == '--':
        return argv[2], argv[4]
    return None


def reject(error):
    return {'exit': 1, 'stderr': json.dumps({'error': error})}


class ChiefRun(Scenario):
    _prefix = 'hb14-'
    _zone_id = ZONE
    _promise = 'P23/P37–P42/P52/P54/P55'
    _retain_native_root = True

    def _archive_ignored(self, path, names):
        ignored = super()._archive_ignored(path, names)
        if path == self.root / 'home/.omp' and 'natives' in names:
            ignored.append('natives')  # installed runtime extraction cache, not invocation observations
        return ignored

    def __init__(self, args, host, agent, archive):
        super().__init__(args, host, agent, 'chief-run', archive)
        self.first = {}
        self.probes = {}
        self.attempts = {}
        self.faults = {}
        self.gate_errors = []
        self.evidence.update(scope='Actual installed hosts, production backend and toolkit; controlled local model, no real network authorization used',
                             preForward=[], verified=[])

    def _uses_sound(self):
        return False

    def _fixed_roles(self):
        return (*super()._fixed_roles(), 'chief-chat', 'chief-report')

    def _model_decision(self, request):
        # Native roles can append guidance after the user's message. OMP also
        # asks for titles/compaction with no interactive tool catalog.
        markers = [marker for row in request.get('messages', []) if row.get('role') == 'user'
                   for marker in re.findall(r'HB14-[a-z0-9-]+', json.dumps(row.get('content', '')))]
        marker = markers[-1] if markers else None
        catalog = {row['function']['name'] for row in request.get('tools', []) if 'function' in row}
        if {'read', 'write'} <= catalog and marker in self.probes and marker not in self.called:
            expected, absent, tool = self.probes[marker]
            require(set(expected) <= catalog and not (set(absent) & catalog), ('native role tool catalog', marker, sorted(catalog)))
            self.called.add(marker)
            if tool:
                name, arguments = tool
                return {'content': '', 'tool_calls': [{'name': name, 'arguments': arguments}]}
        return {'content': 'HB14-ACK ' + (marker or 'unmarked'), 'tool_calls': []}

    def prepare(self):
        super().prepare()
        if self.host == 'tui':
            title = 'HB14 ' + uuid.uuid4().hex
            attach = self.root / 'tui-attach'
            self.console_client = launch_tui_client(self.args.herdr, self.env['HERDR_SESSION'], title,
                                                    self.root, self.env, attach)
            self.tui_window = owned_window(title=title)
            self.note('owned-tui-window', window=self.tui_window)
            attach.touch(exist_ok=False)
        else:
            self.note('owned-app-window', window=owned_window(pid=self.window_process.child.pid))
        stop = threading.Event()
        worker = threading.Thread(target=self._gate, args=(stop,), daemon=True)
        for host, root in self.recorders.items():
            save(root / 'gate', GATES[host])
        worker.start()

        def stop_gate():
            stop.set()
            worker.join(timeout=10)
            require(not worker.is_alive(), 'owned CLI gate thread did not exit')
            self.evidence['gateErrors'] = self.gate_errors

        self.stack.callback(stop_gate)

    def _gate(self, stop):
        while not stop.is_set():
            for host, root in self.recorders.items():
                for request in sorted(root.glob('*.request')):
                    decision = request.with_suffix('.decision')
                    if decision.exists():
                        continue
                    argv = read(request)
                    if tuple(argv[:2]) not in GATES[host]:
                        continue
                    response = None
                    try:
                        response = self._before_forward(host, argv, request)
                    except Exception:
                        self.gate_errors.append(traceback.format_exc())
                        response = reject({'code': 'hb14-observer-failed', 'message': self.gate_errors[-1]})
                    pending = request.with_suffix('.pending')
                    save(pending, response)
                    pending.replace(decision)
            stop.wait(0.02)

    def _calls(self, *, writes_only=False):
        return {str(path.relative_to(self.root)): (host, argv)
                for host, root in self.recorders.items() for path in root.glob('*.request')
                if (argv := read(path)) and (not writes_only or terminal_input(host, argv) or
                    (host in STARTS and tuple(argv[:2]) in {STARTS[host], FOCUS[host], ('tab', 'create')}) or
                    (host == 'app' and argv[:2] == ['do', 'tab.launch']))}

    def _before_forward(self, host, argv, request):
        call = terminal_input(host, argv)
        if call:
            address, text = call
            markers = [key for key in self.first if key in text]
            require(len(markers) <= 1, ('ambiguous business markers in terminal input', markers))
            if markers:
                marker = markers[0]
                plan = self.first[marker]
                if 'approvedPrompt' in plan:
                    require(text == plan['approvedPrompt'], ('terminal input does not match the approved preview', text))
                attempts = self.attempts.setdefault(marker, [])
                observation = self._association_at_write(host, address, plan)
                attempts.append((request, time.monotonic(), observation))
                self.evidence['preForward'].append({'marker': marker, 'request': request.name, **observation})
                if plan['retry'] and len(attempts) == 1:
                    message = 'HB14 definite pre-write refusal; no child invoked'
                    return reject(dict({'code': 'agent_blocked'} if self.host == 'tui' else {'kind': 'blocked'}, message=message))
            for marker, observe in tuple(self.faults.items()):
                if marker in text and marker != 'focus':
                    self.evidence['preForward'].append({'marker': marker, 'request': request.name, 'saved': observe()})
                    return reject({'code': 'hb14-notification-refused', 'message': REPORT_FAILURE})
        if host in FOCUS and tuple(argv[:2]) == FOCUS[host] and 'focus' in self.faults:
            return reject({'code': 'hb14-focus-refused', 'message': FOCUS_FAILURE})
        return None

    def _association_at_write(self, host, address, plan):
        def latest_ready():
            for entry in reversed(self.wire.records[plan['wireStart']:]):
                if entry['path'] == '/api/sessions/events' and entry.get('status') == 200 and entry.get('ended'):
                    event = json.loads(entry['request'])
                    if event.get('kind') == 'launch_ready':
                        return event
            return None

        ready = until('real toolkit ready for this launch', latest_ready, 2)
        sid = ready['sessionId']
        public = self.http('/api/sessions/history')
        saved = read(self.root / 'data/history.json')
        row, = [row for row in saved if row['sessionId'] == sid]
        history, = [row for row in public['sessions'] if row['sessionId'] == sid]
        native = Path(row['sessionFile'])
        require(native.resolve().is_relative_to(self.root.resolve()), ('not an owned native history', native))
        # ADR 0014: both installed agents may allocate the path before writing
        # the file. Keep the SDK's accepted report and verify the actual header
        # once the first real turn has caused native persistence.
        header = next(row for row in map(json.loads, native.read_text(encoding='utf-8').splitlines())
                      if row['type'] == 'session') if native.is_file() else None
        require((header is None or (header['type'] == 'session' and header['id'] == sid)) and
                Path(ready['data']['sessionFile']).resolve() == native.resolve() and history['agentType'] == self.agent and
                row['sourceTaskId'] == plan['task'] and history['originalWorkspaceId'] == self.space,
                ('Chat/source/agent association before input', ready, header, history, row))
        result = {'publicHistory': history, 'persistedHistory': row, 'nativeHeader': header, 'ready': ready}
        run_id = plan.get('run')
        delegation = None
        if plan.get('delegation'):
            delegation, = self.http('/api/chief/delegations?sessionId=' + quote(sid))['delegations']
            require(delegation['id'] == plan['delegation'] and delegation['sessionId'] == sid, delegation)
            result['delegation'] = delegation
            run_id = run_id or delegation.get('runId')
        if plan['role'] == 'manager' and run_id is None:
            run_id, = [path.parent.name for path in (self.root / 'data/runs').glob('*/run.json')
                       if (read(path).get('manager') or {}).get('sessionId') == sid]
        if run_id:
            run = self.detail(run_id)['run']
            durable = read(self.root / 'data/runs' / run_id / 'run.json')
            require(durable['manager'] == run['manager'] and durable['nodes'] == run['nodes'],
                    ('public Run and persisted associations disagree before input', run, durable))
            result.update(run=run, persistedRun=durable)
            if plan.get('node'):
                execution = run['nodes'][plan['node']]['execution']
                chat = execution['chat']
                require(bool(execution.get('endNode')) == (plan['role'] == 'end'), execution)
                if plan['role'] == 'nested':
                    child = self.detail(execution['childRunId'])['run']
                    require(child['manager'] == chat and child['parent'] ==
                            {'runId': run_id, 'nodeId': plan['node'], 'executionId': execution['id']}, child)
                    result['child'] = child
            else:
                chat = run['manager']
            require(chat['sessionId'] == sid, ('late or wrong Run binding', chat, sid))
        else:
            placement = self.http('/api/chats/' + delegation['id'])
            chat = {**placement, 'workspaceId': history['originalWorkspaceId'], 'sessionId': sid}
            result['publicPlacement'] = placement
        require(chat['workspaceId'] == self.space, chat)
        result['nativeTarget'] = self.native_target(chat)
        if host == 'tui':
            value = self.herdr('agent', 'get', address)
            agent = value.get('agent', value)
            require((agent['pane_id'], agent['tab_id']) == (chat['paneId'], chat['tabId']), (agent, chat))
        else:
            require(result['nativeTarget']['address'] == address, (address, result['nativeTarget']))
        return result

    def native_target(self, chat):
        if self.host == 'tui':
            pane = self.herdr('pane', 'get', chat['paneId'])['pane']
            require((pane['pane_id'], pane['tab_id'], pane['workspace_id']) ==
                    (chat['paneId'], chat['tabId'], chat['workspaceId']), (pane, chat))
            return pane
        entry, = [entry for entry in self.layout()[chat['workspaceId']][chat['tabId']] if entry['pane'] == chat['paneId']]
        core = self.core('get', entry['terminal'])
        require(core['labels']['herdr.space'] == chat['workspaceId'] and core['labels']['herdr.tab'] == chat['tabId'] and
                core['address'] == entry['terminal'] and core['state'] == 'live', (entry, core, chat))
        return core

    def expect_first(self, marker, task, role, *, retry=False, **association):
        self.first[marker] = dict(task=task, role=role, wireStart=len(self.wire.records), retry=retry, **association)
        return self._calls(writes_only=True)

    def check_first(self, marker, chat, before):
        require(not self.gate_errors, self.gate_errors)
        attempts = self.attempts[marker]
        expected_attempts = 2 if self.first[marker]['retry'] else 1
        require(len(attempts) == expected_attempts, (marker, attempts))
        calls = [value for name, value in self._calls(writes_only=True).items() if name not in before]
        starts = [argv for host, argv in calls if host in STARTS and tuple(argv[:2]) == STARTS[host]]
        require(len(starts) == 1 and marker not in json.dumps(starts), ('create/start replay or prefilled input', calls))
        if self.host == 'tui':
            require(sum(argv[:2] == ['tab', 'create'] for _, argv in calls) == 1, calls)
        receipts = [until('actual external input exit', lambda path=path: read(path) if path.is_file() else None, 10)
                    for request, _, _ in attempts for path in [request.with_suffix('.json')]]
        require(receipts[-1]['forwarded'] and receipts[-1]['exit'] == 0, receipts)
        if expected_attempts == 2:
            rejected, submitted = attempts
            # Age of the completed refusal is measured in the same wall-clock
            # namespace as the independently stamped second CLI invocation.
            retry_wall = int(submitted[0].stem.split('-', 1)[0]) / 1e9
            elapsed = retry_wall - rejected[0].with_suffix('.json').stat().st_mtime
            require(not receipts[0]['forwarded'] and elapsed >= 5, ('wrong pre-write retry', elapsed, receipts))
            require(receipts[0]['argv'] == receipts[1]['argv'], ('retry changed target/input', receipts))
            self.note('real-initial-retry', secondsAfterRefusal=elapsed, receipts=receipts)
        for _, _, observed in attempts:
            require(observed['ready']['sessionId'] == chat['sessionId'], (observed, chat))
        rows = self.history(chat, 'HB14-ACK ' + marker)
        persisted_path = attempts[-1][2]['persistedHistory']['sessionFile']
        native_path = self.evidence['nativeHistories'][chat['sessionId']]['path']
        header, = [row for row in rows if row['type'] == 'session']
        require(Path(native_path).resolve() == Path(persisted_path).resolve() and header['id'] == chat['sessionId'] and
                sum(row.get('message', {}).get('role') == 'user' and marker in json.dumps(row['message'])
                    for row in rows) == 1, ('first real terminal input was not stored once in the saved native file', marker, rows))
        self.own_agent(chat)
        del self.first[marker]

    def own_agent(self, chat, start=0):
        event = until('owned toolkit launch ready PID', lambda: next((event for event in
            self.accepted_events(chat['sessionId'], start) if event['kind'] == 'launch_ready'), None), self.args.timeout)
        self.evidence.setdefault('ownedAgentProcesses', {})[chat['tabId']] = event
        handle = self.remember(event['data']['pid'])
        require(handle.alive(), ('agent exited before observation', event))
        self.agent_processes[chat['tabId']] = handle
        return handle

    def detail(self, run):
        return self.http('/api/workflow/runs/' + run)

    def manager(self, chat, action, **body):
        return self.http('/api/workflow/manager/' + action, {'sessionId': chat['sessionId'], **body})

    def start_chat(self, task):
        start = len(self.wire.records)
        args = {'taskId': task, 'zone': ZONE}
        args['chatId'] = self.http('/api/chats/preview', args)['chatId']
        started = self.http('/api/chats', args, 202)
        path = '/api/chats/' + started['id']
        chat = until('public Chat launch', lambda: self.finished_chat(path), self.args.timeout)
        ready = until('native owner of created Chat', lambda: next((json.loads(record['request'])
            for record in self.wire.records[start:] if record['path'] == '/api/sessions/events'
            and record.get('status') == 200 and record.get('ended')
            and json.loads(record['request']).get('kind') == 'launch_ready'), None), self.args.timeout)
        chat = {**chat, 'workspaceId': self.space, 'sessionId': ready['sessionId']}
        self.native_target(chat)
        self.own_agent(chat, start)
        self.history(chat, 'HB14-ACK')
        return chat

    def finished_chat(self, path):
        chat = self.http(path)
        require(chat['stage'] != 'failed', chat)
        return chat if chat['stage'] == 'succeeded' else None

    def ready_chief(self, task):
        chat = self.start_chat(task)
        current = self.http('/api/chief')['current']
        require(current['sessionId'] == chat['sessionId'] and current['phase'] == 'registered', current)
        return chat, current

    def delegate(self, chief, bot, task, marker, *, as_map=False):
        path = '/api/chief/delegations'
        req = {'sessionId': chief['sessionId'], 'botId': bot, 'zoneId': ZONE, 'input': marker, 'materials': []}
        if as_map:
            req['managerTaskId'] = task
        before = self._calls(writes_only=True)
        denied = self.http(path + '/preview', {**req, 'sessionId': 'unrelated-session'}, 409)
        require(denied['code'] == 'chief-not-current', denied)
        preview = self.http(path + '/preview', req)
        require(preview['facts']['request'] == req and preview['facts']['task']['id'] == task, preview)
        commit = {'sessionId': chief['sessionId'], 'previewId': preview['previewId']}
        self.http(path, {**commit, 'input': 'unapproved replacement'}, 400)
        require(before == self._calls(writes_only=True), 'unauthorized or preview request touched a terminal')
        before = self.expect_first(marker, task, 'manager' if as_map else 'delegated',
                                   delegation=preview['previewId'], approvedPrompt=preview['facts']['prompt'], retry=not as_map)
        reply = self.http(path, commit, 201 if as_map else 202)
        if as_map:
            chat = reply['run']['manager']
        else:
            chat = until('approved delegated Chat', lambda: self.finished_chat('/api/chats/' + reply['id']), self.args.timeout)
            saved = self.http(path + '/' + reply['id'])
            chat = {**chat, 'sessionId': saved['sessionId'], 'workspaceId': self.space}
        self.check_first(marker, chat, before)
        return reply, chat

    def input_target(self, chat):
        def probe():
            target = self.native_target(chat)
            status = target.get('agent_status') if self.host == 'tui' else (target.get('status') or {}).get('state')
            return target if status in ('idle', 'done') else None
        return until('owned native input target is idle', probe, self.args.timeout)

    def send(self, chat, text, *, observe_idle=True):
        target = self.input_target(chat) if observe_idle else self.native_target(chat)
        if self.host == 'tui':
            self.herdr('agent', 'prompt', chat['paneId'], text)
        else:
            self.native_command(self.args.core, 'send', '--json', target['address'], '--', text)

    def role(self, chat, marker, *, present=(), absent=(), tool=None, route=None, observe_idle=True):
        self.probes[marker] = (present, absent, tool)
        before = len(self.wire.records)
        self.send(chat, marker, observe_idle=observe_idle)
        self.history(chat, 'HB14-ACK ' + marker)
        require(marker in self.called, ('no installed model round observed for role probe', marker))
        if not route:
            return None
        def result():
            matches = [record for record in self.wire.records[before:] if record['path'] == route and
                       record.get('ended') and json.loads(record['request']).get('sessionId') == chat['sessionId']]
            require(len(matches) <= 1, ('native tool was replayed', matches))
            return matches[0] if matches else None
        record = until('actual native tool HTTP result ' + marker, result, self.args.timeout)
        require(record['status'] == 200, record)
        return json.loads(record['response'])

    def verify(self, promise, **evidence):
        self.evidence['verified'].append({'promise': promise, **evidence})

    def quiet(self, baseline, seconds=5.5):
        began = time.monotonic()
        while time.monotonic() - began < seconds:
            require(self._calls(writes_only=True) == baseline, 'unrequested external terminal side effect after operation')
            time.sleep(0.2)
        return time.monotonic() - began

    def reports(self, sender, delegation, old, current, receiver):
        require(self.agent_processes[old['tabId']].alive() and old['sessionId'] != receiver['sessionId'],
                ('the original chief did not remain a live decoy', old, receiver))
        require(self.http('/api/chief')['current'] == current, current)
        route = '/api/chief/reports'
        def submit(marker, fail=False):
            payload = {'background': 'Owned isolated request; retain the original result',
                       'execution': 'No external files or publication', 'result': 'failure' if fail else 'success',
                       'summary': 'Mechanical provider only: ' + marker, 'deliverables': [], 'pendingDecisions': []}
            def readback():
                report, = [item for item in self.http(route + '?sessionId=' + quote(sender['sessionId']))['reports']
                           if item['body'] == payload]
                return report
            if not fail:
                self.input_target(receiver)
            before = self._calls(writes_only=True)
            if fail:
                self.faults[marker] = readback
            result = self.http(route, {'sessionId': sender['sessionId'], 'body': payload})
            self.faults.pop(marker, None)
            expected = 'failed' if fail else 'submitted'
            require(result['saved'] and result['notification']['status'] == expected, result)
            report = result['report']
            require(report['from'] == sender['sessionId'] and report['delegationId'] == delegation and
                    report['body'] == payload and self.http(route + '/' + report['id']) == report, report)
            viewed = self.http(route + '/' + report['id'] + '/view', {'sessionId': receiver['sessionId']})
            require(viewed['report'] == report, viewed)
            attempts = self.matching_input(before, report['id'], receiver)
            if fail:
                require(REPORT_FAILURE in json.dumps(result['notification'], ensure_ascii=False), result)
                observation, = [row for row in self.evidence['preForward'] if row['marker'] == marker]
                require(observation['saved'] == report and not attempts['forwarded'], (observation, attempts))
            else:
                rows = self.history(receiver, report['id'])
                require(any(row.get('message', {}).get('role') == 'user' and report['id'] in json.dumps(row['message'])
                            for row in rows) and attempts['forwarded'], (result, attempts))
                old_rows = self.history(old, 'HB14-ACK')
                require(report['id'] not in json.dumps(old_rows), 'the original delegator received the result')
            return report
        delivered = submit(MARK + 'chief-receiver')
        failed = submit(MARK + 'chief-failure', fail=True)
        self.verify('P38', receiver=receiver, originalChief=old, actualReport=delivered, failedReport=failed)
        return failed

    def matching_input(self, before, marker, target):
        path, host, call = next((path, host, call) for path, (host, argv) in self._calls(writes_only=True).items()
                                if path not in before and (call := terminal_input(host, argv)) and marker in call[1])
        attempts = [(h, argv) for path, (h, argv) in self._calls(writes_only=True).items() if path not in before]
        require(len(attempts) == 1, ('notification must make one external attempt', attempts))
        if self.host == 'app':
            require(host == 'core' and call[0] == self.native_target(target)['address'], (call, target))
        else:
            value = self.herdr('agent', 'get', call[0])
            require(host == 'tui' and value.get('agent', value)['pane_id'] == target['paneId'], (call, target))
        return until('notification external receipt', lambda: read(self.root / Path(path).with_suffix('.json'))
                     if (self.root / Path(path).with_suffix('.json')).is_file() else None, 10)

    def run_report_failure(self, run, manager, worker, execution):
        marker = MARK + 'run-report-refused'
        def existing_report():
            report, = [item for item in self.detail(run)['reports'] if item['body'] == marker]
            return report
        self.faults[marker] = existing_report
        before = self._calls(writes_only=True)
        result = self.http('/api/workflow/node/report', {'sessionId': worker['sessionId'], 'body': marker, 'canComplete': False})
        self.faults.pop(marker)
        receipt = self.matching_input(before, marker, manager)
        report = result['report']
        require(result['saved'] and result['notification']['status'] == 'failed' and not receipt['forwarded'] and
                REPORT_FAILURE in json.dumps(result['notification'], ensure_ascii=False) and report['executionId'] == execution['id'] and
                report['to']['sessionId'] == manager['sessionId'] and report == existing_report(), result)
        observation, = [row for row in self.evidence['preForward'] if row['marker'] == marker]
        require(observation['saved'] == report, (observation, report))
        self.verify('P40', failedRunReport=report)
        return report

    def observe_focus(self, target):
        tab, record = self.focused_tab()
        require(tab == target['tabId'], (target, record))
        if self.host == 'app':
            require((record['space'], record['focused_pane']) == (target['workspaceId'], target['paneId']), record)
        else:
            pane = self.native_target(target)
            require(pane.get('focused') is True, pane)
        return record

    def locate(self, run, target, execution):
        route = '/api/workflow/runs/' + run + '/locate'
        if self.host == 'app':
            self.app_do('tab.select', id=self.sentinel)
        else:
            self.herdr('tab', 'focus', self.sentinel)
        require(self.focused_tab()[0] == self.sentinel, 'the owned decoy was not focused')
        before = self._calls(writes_only=True)
        def inventory():
            return {space: set(tabs) for space, tabs in self.layout().items()}
        layout = inventory()
        wrong = self.http(route, {'executionId': 'owned-but-not-an-execution'}, 404)
        require(wrong['code'] == 'run-chat-not-found' and self.focused_tab()[0] == self.sentinel and
                self._calls(writes_only=True) == before, wrong)
        self.faults['focus'] = None
        failed = self.http(route, {'executionId': execution}, 500)
        self.faults.pop('focus')
        require(failed['code'] == 'run-chat-focus-failed' and FOCUS_FAILURE in json.dumps(failed, ensure_ascii=False) and
                self.focused_tab()[0] == self.sentinel and inventory() == layout, failed)
        calls = [value for key, value in self._calls(writes_only=True).items() if key not in before]
        require(len(calls) == 1 and tuple(calls[0][1][:2]) == FOCUS[self.host], calls)
        located = self.http(route, {'executionId': execution})
        require(located['outcome'] == 'located' and located['target']['paneId'] == target['paneId'], located)
        focused = self.observe_focus(target)
        self.verify('P41', locate=located, nativeFocus=focused, wrong=wrong, focusFailure=failed,
                    osForegroundClaim=False)

    def close_chat(self, chat):
        target = self.input_target(chat)
        _, handles = self.members(chat['workspaceId'], chat['tabId'], self.layout(), chat['paneId'])
        agent = self.agent_processes[chat['tabId']]
        require(agent in handles and agent.alive(), ('lost owned agent exit handle', chat))
        begin = len(self.wire.records)
        if self.host == 'app':
            self.core('keys', target['address'], 'ctrl+d')
        else:
            self.herdr('agent', 'send-keys', chat['paneId'], 'ctrl+d')
        until('owned native agent gracefully exited', lambda: not agent.alive(), self.args.timeout)
        if self.host == 'app':
            ended = until('App agent released its own launch owner', lambda: next((event for event in
                self.accepted_events(chat['sessionId'], begin) if event['kind'] == 'launch_ended'), None), self.args.timeout)
            self.note('owned-app-launch-ended', event=ended)
        if chat['tabId'] in self.layout()[chat['workspaceId']]:
            if self.host == 'app':
                require(self.app_do('tab.close', id=chat['tabId'], confirmed=True)['ok'], chat)
            else:
                self.herdr('tab', 'close', chat['tabId'])
        until('closed owned native Chat and process', lambda: chat['tabId'] not in self.layout()[chat['workspaceId']]
              and all(not handle.alive() for handle in handles), self.args.timeout)
        self.note('closed-before-resume', chat=chat, exited=[handle.pid for handle in handles])

    def own_tui_recovery(self, previous, tab, original):
        pane, = [entry for entry in self.herdr('pane', 'list', '--workspace', self.space)['panes'] if entry['tab_id'] == tab]
        context = pane['agent_session']
        require(context['agent'] == self.agent and context['kind'] == 'path' and
                Path(context['value']).resolve() == original.resolve(), (pane, original))
        info = self.herdr('pane', 'process-info', '--pane', pane['pane_id'])['process_info']
        package = getattr(self.args, self.agent + '_package')
        native_cli = (package / ('dist/cli.js' if self.agent == 'omp' else 'dist/bundle/cli.js')).resolve()
        process, = [entry for entry in info['foreground_processes'] if any(
            Path(arg).resolve() == native_cli for arg in entry.get('argv', [])[1:2])]
        handle = self.remember(process['pid'])
        require(handle.alive() and handle.pid != self.agent_processes[previous['tabId']].pid and
                Path(process['cwd']).resolve() == (self.root / 'zone').resolve(), (previous, process))
        self.agent_processes[tab] = handle
        self.note('restored-tui-native-owner', pane=pane, process=process)

    def restore(self, chat, *, degraded=False, fail=False, runs=()):
        self.close_chat(chat)
        before = self._calls(writes_only=True)
        layout = self.layout()
        saved_runs = {run: self.detail(run) for run in runs}
        listing = self.http('/api/workflow/runs')
        identity = chat['sessionId']
        history, = [row for row in read(self.root / 'data/history.json') if row['sessionId'] == identity]
        original = Path(history['sessionFile'])
        def conversation():
            return [row for row in map(json.loads, original.read_text(encoding='utf-8').splitlines())
                    if row['type'] == 'message' and row.get('message', {}).get('role') in ('user', 'assistant')]
        before_native = conversation()
        wire_start = len(self.wire.records)
        if fail:
            self.faults['focus'] = None
        resume = self.http('/api/chats/resume', {'sessionId': identity}, 502 if fail else 200)
        self.faults.pop('focus', None)
        if fail:
            require(resume['outcome'] == 'failed' and resume['stage'] == 'focus' and
                    FOCUS_FAILURE in json.dumps(resume, ensure_ascii=False), resume)
        elif degraded:
            warning, = [entry for entry in resume['degradations'] if entry['element'] == 'extensions']
            require('原生默认扩展发现' in warning['strategy'] and '不强制接入 connect-bot' in warning['strategy'] and
                    resume['outcome'] in ('restored', 'unconfirmed'), resume)
        else:
            require(resume['outcome'] == 'restored' and resume['stage'] == 'focused' and not resume.get('degradations'), resume)
        self.quiet(self._calls(writes_only=True))
        calls = [value for name, value in self._calls(writes_only=True).items() if name not in before]
        starts = [argv for host, argv in calls if host in STARTS and tuple(argv[:2]) == STARTS[host]]
        require(len(starts) == 1 and not any(terminal_input(host, argv) for host, argv in calls) and
                (self.host != 'tui' or sum(argv[:2] == ['tab', 'create'] for _, argv in calls) == 1), calls)
        require(not degraded or not any(name in json.dumps(starts) for name in ['--extension', '--no-extensions', 'node-chat.ts', 'manager-chat.ts']), starts)
        native = self.layout()
        new = [(space, tab) for space, tabs in native.items() for tab in tabs if tab not in layout.get(space, {})]
        require(len(new) == 1 and new[0][0] == self.space and self.http('/api/workflow/runs') == listing and
                all(self.detail(run) == detail for run, detail in saved_runs.items()) and
                conversation() == before_native, ('Resume changed saved Run or native conversation', new, resume))
        self.note('resume-effects', original=chat, reply=resume, external=calls, newNativeTabs=new)
        if not degraded and self.host == 'tui':
            # The native TUI Resume path confirms the official Pane reference;
            # unlike App's new-process binding, it does not issue launch_ready.
            self.own_tui_recovery(chat, new[0][1], original)
        elif not degraded:
            recovered = {'tabId': new[0][1], 'sessionId': identity}
            handle = self.own_agent(recovered, wire_start)
            require(handle.pid != self.agent_processes[chat['tabId']].pid, ('same process after Resume', recovered))
            owned_event = self.evidence['ownedAgentProcesses'][recovered['tabId']]
            require(Path(owned_event['data']['sessionFile']).resolve() == original.resolve(), owned_event)
        if fail:
            return None
        restored = {**resume['target'], 'sessionId': identity}
        require(restored['tabId'] == new[0][1], (restored, new))
        self.native_target(restored)
        self.observe_focus(restored)
        return restored

    def prepare_run(self, manager):
        report = self.manager(manager, 'prepare_inputs', fileInputs=[], confirmed=True, reason='Fixture inputs reviewed')
        require(report['snapshot']['inputsReady'] and not report.get('historyError'), report)

    def dispatch_node(self, run, manager, node, task, marker, role):
        before = self.expect_first(marker, task, role, run=run, node=node)
        result = self.manager(manager, 'dispatch', nodes=[node])
        view = result['snapshot']['nodes'][node]
        require(view['status'] == 'running' and result['results'][0]['status'] == 'running', result)
        execution = view['execution']
        chat = execution['chat']
        self.check_first(marker, chat, before)
        return chat, execution

    def manager_role(self, chat, run, marker, *, nested=False):
        reply = self.role(chat, MARK + marker, present=('run', 'collect', *(['report'] if nested else [])),
                          absent=() if nested else ('report', 'approve'), tool=('run', {'action': 'status'}),
                          route='/api/workflow/manager/status')
        require(reply['snapshot']['id'] == run, reply)

    def approve_report(self, chat, run, execution, label):
        approval = self.role(chat, MARK + label + '-approve', present=('approve', 'report'),
                             tool=('approve', {}), route='/api/workflow/node/approve')
        require(approval['approved'], approval)
        report = self.role(chat, MARK + label + '-report', present=('approve', 'report'),
                           tool=('report', {'body': 'Owned handoff ' + label, 'can_complete': True}),
                           route='/api/workflow/node/report')
        fact = report['report']
        require(report['saved'] and fact['runId'] == run and fact['executionId'] == execution['id'] and
                fact['body'] == 'Owned handoff ' + label and fact in self.detail(run)['reports'], report)

    def definitions(self, manager, worker, end_task):
        def bot(name, **target):
            save(self.root / 'bots' / (name + '.json'), dict(id=name, name=name, when='', plan_guide='', node_guide='', **target))
        def node(name, bot_id, **facts):
            return dict(title=name, bot_id=bot_id, input=MARK + name, zone=ZONE,
                        output_rule=[], auto_approved=True, auto_close=False, **facts)
        bot('worker', task_id=worker)
        bot('handoff', task_id=end_task)
        bot('nested', map_file='inner.json')
        bot('root', map_file='root.json')
        save(self.root / 'maps/inner.json', {'title': 'Inner', 'description': '', 'nodes': {
            'E': node('inner-handoff', 'handoff', blocked_by=[], end_node=True)}})
        save(self.root / 'maps/root.json', {'title': 'Root', 'description': '', 'nodes': {
            'W': node('root-worker', 'worker', blocked_by=[]),
            'N': node('nested-manager', 'nested', manager_task_id=manager, blocked_by=['W'])}})

    def _exercise(self):
        task_view = self.http('/api/tasks', {'name': 'owned-chief', 'taskType': 'chief', 'agent': self.agent,
                              'chatName': 'Same title chief', 'model': 'probe', 'thinking': 'default',
                              'skill': None, 'prompt': 'Reply without tools.'})
        chief_task, = [task['id'] for task in task_view['tasks'] if task['name'] == 'owned-chief']
        manager_task = self.task('owned-manager', 'manager')
        worker_task = self.task('owned-worker', 'normal')
        end_task = self.task('owned-end', 'normal')
        self.definitions(manager_task, worker_task, end_task)
        old, authority = self.ready_chief(chief_task)
        delegation, delegated = self.delegate(old, 'worker', worker_task, MARK + 'chief-worker')
        assigned, assigned_manager = self.delegate(old, 'root', manager_task, MARK + 'chief-map', as_map=True)
        self.manager_role(assigned_manager, assigned['id'], 'delegated-map-role')
        self.verify('P37', standaloneDelegation=delegation['id'], mapDelegation=assigned['id'], author=authority)
        self.http('/api/chief/release', {'claimId': authority['claimId']})
        receiver, current = self.ready_chief(chief_task)
        require(receiver['name'] == old['name'] and receiver['tabId'] != old['tabId'], (old, receiver))
        before = self._calls(writes_only=True)
        denied = self.http('/api/chief/delegations/preview', {'sessionId': old['sessionId'], 'botId': 'worker',
                            'zoneId': ZONE, 'input': 'A revoked chief cannot delegate', 'materials': []}, 409)
        require(denied['code'] == 'chief-not-current' and self._calls(writes_only=True) == before, denied)
        chief_failure = self.reports(delegated, delegation['id'], old, current, receiver)

        marker = MARK + 'root-manager'
        before = self.expect_first(marker, manager_task, 'manager')
        started = self.http('/api/scheduler/maps/root.json/runs', {'managerTaskId': manager_task,
                            'zone': ZONE, 'userInput': marker}, 201)
        run, manager = started['id'], started['run']['manager']
        self.check_first(marker, manager, before)
        self.prepare_run(manager)
        worker, execution = self.dispatch_node(run, manager, 'W', worker_task, MARK + 'root-worker', 'node')
        self.locate(run, worker, execution['id'])
        run_failure = self.run_report_failure(run, manager, worker, execution)
        seconds = self.quiet(self._calls(writes_only=True), 46)
        require(self.http('/api/chief/reports/' + chief_failure['id']) == chief_failure and
                run_failure in self.detail(run)['reports'], (chief_failure, run_failure))
        self.verify('P52', notificationQuietSeconds=seconds, chiefReportId=chief_failure['id'], runReportId=run_failure['id'])

        worker = self.restore(worker, runs=(run,))
        self.approve_report(worker, run, execution, 'ordinary-node')
        completed = self.manager(manager, 'complete', nodeId='W', reason='Verified original execution handoff')
        require(completed['snapshot']['nodes']['W']['status'] == 'completed', completed)
        child_manager, parent_execution = self.dispatch_node(run, manager, 'N', manager_task, MARK + 'nested-manager', 'nested')
        child = parent_execution['childRunId']
        child_manager = self.restore(child_manager, runs=(run, child))
        self.manager_role(child_manager, child, 'restored-dual-role', nested=True)
        self.prepare_run(child_manager)
        end, end_execution = self.dispatch_node(child, child_manager, 'E', end_task, MARK + 'inner-handoff', 'end')
        self.role(end, MARK + 'end-ordinary', absent=('approve', 'report', 'run', 'collect', 'chief_report'))
        self.http('/api/workflow/node/report', {'sessionId': end['sessionId'], 'body': 'not a reporting node', 'canComplete': True}, 403)
        finished = self.manager(child_manager, 'finish')
        require(finished['snapshot']['finished']['reason'] == 'handed-to-user' and
                finished['snapshot']['nodes']['E']['status'] == 'running', finished)
        self.approve_report(child_manager, run, parent_execution, 'nested-parent')
        completed = self.manager(manager, 'complete', nodeId='N', reason='Verified nested handoff')
        require(completed['snapshot']['nodes']['N']['status'] == 'completed' and
                completed['snapshot']['nodes']['N']['execution']['id'] == parent_execution['id'], completed)
        self.verify('P23/P39', run=run, child=child, workerExecution=execution['id'],
                    nestedExecution=parent_execution['id'], endExecution=end_execution['id'])
        manager = self.restore(manager, runs=(run, child))
        self.manager_role(manager, run, 'restored-root-role')
        assigned_id = assigned['id']
        mapping = '/api/chief/delegations?sessionId=' + quote(assigned_manager['sessionId'])
        linked = self.http(mapping)
        assigned_manager = self.restore(assigned_manager, runs=(assigned_id,))
        self.manager_role(assigned_manager, assigned_id, 'restored-delegated-map-role')
        require(self.http(mapping) == linked, 'Resume changed approved Map delegation')
        self.restore(delegated, fail=True, runs=(assigned_id, run, child))
        with urlopen(Request(self.base + '/api/tasks/' + quote(worker_task), method='DELETE'), timeout=5) as response:
            self.note('owned-source-task-deleted', status=response.status, body=json.load(response))
        degraded = self.restore(worker, degraded=True, runs=(run, child))
        self.role(degraded, MARK + 'degraded-no-role', observe_idle=False,
                  absent=('approve', 'report', 'run', 'collect', 'chief_report'))
        require(not self.gate_errors, self.gate_errors)
        calls = list(self._calls(writes_only=True).values())
        # Two chief Chats, two approved delegations, one normal manager and
        # three node launches; four normal, one failed and one degraded Resume.
        starts = [argv for host, argv in calls if host in STARTS and tuple(argv[:2]) == STARTS[host]]
        require(len(starts) == 14, ('creation/start was replayed', starts))
        if self.host == 'tui':
            require(sum(argv[:2] == ['tab', 'create'] for _, argv in calls) == 14, calls)
        for marker, attempts in self.attempts.items():
            text = read(attempts[-1][0])[-1]
            # A child manager may legitimately receive the parent input as
            # context. Count replay of the entire submitted message, not a
            # marker that can also occur within a different business input.
            actual = [argv for host, argv in calls if (candidate := terminal_input(host, argv)) and text == candidate[1]]
            require(len(actual) == len(attempts), ('late initial input replay', marker, actual))
        self.verify('P42/P52', run=run, child=child, originalExecution=execution['id'], launchAttempts=len(starts),
                    failedResumeDidNotReplay=True, degradationClaimsNoRoleResources=True)
