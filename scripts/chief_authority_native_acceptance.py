"""P31 at approved S2/S3 seams: actual installed hosts, native admission and HTTP.

Independent oracle is ticket 20: replacing authority revokes future dispatch and
receipt grants, not a previously submitted external input. A plausible violation
caches the recipient before a suspension and grants the old Session on arrival.
The barrier holds the *real native receive request before backend authorization*,
not a fabricated result; a fresh report subsequently enters the new native chief.
No model tool decision/result is supplied. No same-ID resend/recovery is claimed.
"""
import hashlib
import http.server
import json
from pathlib import Path
import runpy
import shutil
import sys
import threading
import time
import traceback
import urllib.error
import urllib.request

reports = runpy.run_path(str(Path(__file__).with_name('chief_reports_native_acceptance.py')))
ChiefReportsAcceptance, require, until = reports['ChiefReportsAcceptance'], reports['require'], reports['until']
MapAcceptance = runpy.run_path(str(Path(__file__).with_name('chief_map_native_acceptance.py')))['ChiefMapAcceptance']


class ReceiveBarrier:
    """Transparent HTTP transport scheduling only; all responses are real backend."""
    def __init__(self, upstream):
        self.entered = threading.Event()
        self.release = threading.Event()
        self.records = []
        self.armed = True
        self.lock = threading.Lock()
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
                record = {'path': self.path, 'method': self.command, 'request': raw.decode(), 'arrived': time.monotonic()}
                with owner.lock:
                    held = owner.armed and self.path.endswith('/receive')
                    if held:
                        owner.armed = False
                    owner.records.append(record)
                try:
                    if held:
                        record['held'] = True
                        owner.entered.set()
                        require(owner.release.wait(18), 'Native receive handoff barrier exceeded 18s; no retry')
                    record['forwarded'] = time.monotonic()
                    request = urllib.request.Request(upstream + self.path, data=raw if raw else None,
                        method=self.command, headers={'Content-Type': 'application/json'})
                    try:
                        response = urllib.request.urlopen(request, timeout=30)
                    except urllib.error.HTTPError as error:
                        response = error
                    with response:
                        body = response.read()
                        record.update(status=response.status, response=body.decode())
                        self.send_response(response.status)
                        self.send_header('Content-Type', 'application/json')
                        self.send_header('Content-Length', str(len(body)))
                        self.end_headers()
                        self.wfile.write(body)
                except BaseException:
                    record['error'] = traceback.format_exc()
                    self.send_error(502)
                finally:
                    record['ended'] = time.monotonic()

        self.server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        # server_close joins owned workers; evidence cannot outlive a handler.
        # Upstream calls retain their explicit 30s transport timeout.
        self.server.daemon_threads = False
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.base = f'http://127.0.0.1:{self.server.server_port}'

    def close(self):
        self.release.set()
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(15)
        require(not self.thread.is_alive(), 'Receipt barrier server did not exit')


class ChiefAuthorityAcceptance(ChiefReportsAcceptance):
    def __init__(self, args):
        super().__init__(args)
        self.barrier = None
        self.evidence.update(scenario='chief-authority', promises=['P31'], coverage={'P31': {'verified': False}},
            order=[], timing={'nativeReceiveBudgetSeconds': 20, 'transportBarrierSeconds': 18,
                'nativeConfirmationSeconds': 20, 'requestSeconds': 30, 'startupObservationSeconds': args.timeout,
                'nativeAgentReadyBudgetSeconds': 30, 'isolatedServiceReadyHarnessBudgetSeconds': 45,
                'sessionPolicy': 'Production defaults; owned backend.toml does not override lifecycle thresholds',
                'onlineSeconds': 10, 'retentionSeconds': 30, 'maintenanceSeconds': 5, 'observationSeconds': 46},
            scenarioSource={'path': str(Path(__file__).resolve()), 'sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()})

    def http(self, path, body=None, status=200):
        before = len(self.evidence.setdefault('http', []))
        started = time.monotonic()
        try:
            return super().http(path, body, status)
        finally:
            for record in self.evidence['http'][before:]:
                record.update(method='GET' if body is None else 'POST', request=body,
                    started=started, ended=time.monotonic())

    def mark(self, name, **facts):
        event = {'name': name, 'at': time.monotonic(), **facts}
        self.evidence['order'].append(event)
        return event

    def start_backend(self, role_config):
        observer = self.root / 'confirmation-observer.mjs'
        source = self.args.repo / 'scripts/chief-ui-observer.mjs'
        shutil.copy2(source, observer)
        self.confirmation_directory = self.root / 'confirmations'
        self.env['CHIEF_MAP_CONFIRMATION_EVIDENCE_DIR'] = str(self.confirmation_directory)
        with (self.root / 'options.toml').open('a', encoding='utf-8') as options:
            options.write('\n[[extensions]]\nid = "authority-ui-observer"\nlabel = "Acceptance UI observation"\nagent = '
                + json.dumps(self.host) + '\nkind = "fixed"\nentrypoint = ' + json.dumps(str(observer)) + '\n')
        self.evidence['confirmationObserver'] = {'source': str(source), 'sha256': hashlib.sha256(source.read_bytes()).hexdigest(),
            'scope': 'Pass-through public UI arguments/results only; never chooses tools or approves'}
        super().start_backend(role_config)

    def prepare(self):
        super().prepare()
        self.barrier = ReceiveBarrier(self.base)
        self.backend_wire.upstream = self.barrier.base

    def cleanup_extra(self):
        try:
            if self.barrier:
                self.evidence['receiptTransport'] = self.barrier.records
                self.barrier.close()
        finally:
            super().cleanup_extra()

    def authoritative(self, chief):
        def registered():
            current = self.http('/api/chief')['current']
            require(current and current.get('sessionId') == chief['sessionId'], ('Wrong current authority', current, chief))
            return current if current.get('phase') == 'registered' else None
        current = until('actual current chief registration', registered, self.args.timeout)
        self.http('/api/chief/qualify', {'sessionId': chief['sessionId']})
        return current

    def old_alive(self):
        for pid, handle in self.handles:
            if pid in self.old_pids:
                require(handle.alive(), ('Old chief unexpectedly terminated', pid))
        self.mark('old-process-still-alive', pids=sorted(self.old_pids))

    def size_terminal(self, chat):
        MapAcceptance.size_terminal(self, chat)

    def report_input(self, sender, label):
        self.size_terminal(sender)
        history = self.native(sender)
        self.sender_report_start = len(history['branch']) if history else 0
        output = self.command([sys.executable, '-c', 'print(' + repr(label) + ')'])
        require(output.strip() == label, 'Independent marker operation did not produce its expected literal')
        operation = self.evidence['commands'][-1]
        self.mark('independent-marker-operation-completed', label=label, operation=operation)
        self.send(sender, '向当前chief报告本次独立已结束的核对结果：背景是只读交接验证；执行对象为独立标记进程；'
            + '实际进程结果证据为 ' + json.dumps(operation, ensure_ascii=False)
            + '。结果成功，摘要保留实际stdout标记 ' + label + '；没有文件交付，没有后续业务授权。仅询问用户是否满足要求。'
            + '这是新的独立结果，不是重发任何已有报告。等待本次原生发送确认；不要重新执行业务。', 'new-report-' + label)
        def current_approval():
            pending = self.approval(sender)
            return pending if label in json.dumps(pending['pending']['arguments']['body'], ensure_ascii=False) and label in pending['terminal'] else None
        pending = until('current new report native confirmation', current_approval, self.args.timeout)
        body = pending['pending']['arguments']['body']
        observed = self.observed_confirmation(sender, '发送 chief 报告？（仅批准本次完整报告）', label)
        require({'sessionId': sender['sessionId'], 'body': body} in self.json_objects(observed['message']), 'Native confirmation altered the complete report request')
        self.mark('native-report-approval', label=label, fullPreview=pending, invocation=observed)
        self.herdr('agent', 'send-keys', sender['paneId'], 'enter')
        return body

    def dispatch_facts(self):
        return {'historySessionIds': sorted(session['sessionId'] for session in self.http('/api/sessions/history')['sessions']),
            'runIds': sorted(run['id'] for run in self.http('/api/workflow/runs')['runs']),
            'nativePids': sorted(self.native_pids()), 'ownedWorkspaceTabCount': len(self.tabs(self.workspace))}

    def native_denied(self, chief, bot, manager=None):
        before = len(self.backend_wire.records)
        history = self.native(chief)
        start = len(history['branch']) if history else 0
        self.send(chief, '请把一次只读交接标记核对委派给能力 ' + bot + '，工作位置reports/owned，'
            + (('指定manager Task ID为' + manager + '，') if manager else '')
            + '业务输入为“只确认收到 AUTHORITY-DIAGNOSTIC 后等待”。本轮是该不同能力的一次独立失权诊断：即使上一轮另一能力已被拒绝，仍请通过现有正常委派入口尝试本次一次，核对实际拒绝。不得绕过权限；如被拒绝忠实反馈并停止，不重试、不改用HTTP或其它会话。', 'old-chief-' + bot)
        result = self.final_answer(chief, start)
        calls = [record for record in self.backend_wire.records[before:] if record['path'] == '/api/chief/delegations/preview' and record.get('ended')]
        require(len(calls) == 1 and calls[0]['status'] == 409
            and json.loads(calls[0]['response']).get('code') == 'chief-not-current', ('No single real native authority-denied dispatch', bot, calls))
        request = json.loads(calls[0]['request'])
        require(request['sessionId'] == chief['sessionId'] and request['botId'] == bot, ('Wrong native caller/target', request))
        require(not any(record['path'] == '/api/chief/delegations' for record in self.backend_wire.records[before:]), 'Denied preview still committed')
        require(any(message.get('role') == 'toolResult' for message in self.messages(result['history'], start)), 'Missing actual refusal tool result')
        self.mark('old-native-dispatch-denied', bot=bot, http=calls[0], history=result['history'])

    @staticmethod
    def json_objects(message):
        decoder, values, position = json.JSONDecoder(), [], 0
        while (start := message.find('{', position)) >= 0:
            try:
                value, consumed = decoder.raw_decode(message[start:])
            except ValueError:
                position = start + 1
            else:
                values.append(value)
                position = start + consumed
        return values

    def observed_confirmation(self, chat, title, token):
        def observed():
            for path in self.confirmation_directory.glob('*.request.json'):
                value = json.loads(path.read_text(encoding='utf-8'))
                if value.get('sessionId') == chat['sessionId'] and value.get('title') == title and token in value.get('message', ''):
                    return value
        return until('actual public native UI invocation ' + title, observed, 5)

    def native_allowed(self, chief, bot, manager=None):
        # Observe the same installed public UI as the other native acceptance
        # runners; the driver supplies a user request and approves, not a tool.
        self.size_terminal(chief)
        before = len(self.backend_wire.records)
        history = self.native(chief)
        start = len(history['branch']) if history else 0
        self.send(chief, '请把一次只读交接标记核对委派给能力 ' + bot + '，工作位置reports/owned，'
            + (('指定manager Task ID为' + manager + '，') if manager else '')
            + '业务要求：只确认收到 AUTHORITY-NEW-VALID 后等待，不派发内部节点、不自动报告；不需要材料。等待本次原生批准，发起后返回，不等待业务完成。', 'new-chief-' + bot)
        def pending():
            candidates = [record for record in self.backend_wire.records[before:] if record['path'] == '/api/chief/delegations/preview' and record.get('ended')]
            if not candidates:
                return None
            record = candidates[-1]
            require(record['status'] == 200, ('New chief valid preview refused', record))
            screen = self.command([self.args.herdr, 'pane', 'read', chief['paneId'], '--lines', '1000', '--format', 'text'])
            preview_id = json.loads(record['response'])['previewId']
            return (record, screen) if '确认 Bot 委派启动' in screen and 'Yes' in screen and 'No' in screen and preview_id in screen else None
        preview, screen = until('new chief actual native delegation confirmation', pending, self.args.timeout)
        request, resolved = json.loads(preview['request']), json.loads(preview['response'])
        require(request['sessionId'] == chief['sessionId'] and request['botId'] == bot, ('Wrong new native dispatcher', request))
        require(not any(record['path'] == '/api/chief/delegations' for record in self.backend_wire.records[before:]), 'Native dispatch preceded approval')
        observed = self.observed_confirmation(chief, '确认 Bot 委派启动', resolved['previewId'])
        objects = self.json_objects(observed['message'])
        require(resolved['facts'] in objects and {'sessionId': chief['sessionId'], 'previewId': resolved['previewId']} in objects, 'Native approval omitted resolved facts/commit')
        approved = self.mark('new-native-dispatch-approval', bot=bot, invocation=observed, terminal=screen, preview=preview)
        self.herdr('agent', 'send-keys', chief['paneId'], 'enter')
        answer = self.final_answer(chief, start)
        commit = until('new chief real native commit result', lambda: next((r for r in self.backend_wire.records[before:] if r['path'] == '/api/chief/delegations' and r.get('ended')), None), self.args.timeout)
        require(commit['status'] == (201 if manager else 202) and commit['started'] >= approved['at'], ('Invalid new chief native operation', commit))
        result = json.loads(commit['response'])
        if manager:
            detail = self.detail(result['id'])
            require(detail['run']['parent'] is None and detail['run']['managerTaskId'] == manager, 'New valid Map dispatch did not create the actual independent Run')
        else:
            result = self.creation({**result, 'delegated': True})
        self.authoritative(chief)
        self.observe_processes()
        self.mark('new-native-dispatch-succeeded', bot=bot, commit=commit, publicResult=result, history=answer['history'])

    def exercise(self):
        self.http('/api/zones', {'id': 'reports/owned', 'path': str(self.root / 'zone')})
        chief_task = self.task('authority-chief', 'chief', '', self.host)
        worker = self.task('authority-worker', 'normal', '只确认收到本次标记，然后等待。不要自动报告、创建或执行业务。', self.host)
        sender_task = self.task('authority-reporter', 'normal', '你是独立报告员，只在用户明确要求时向当前chief报告新的已结束结果，每次等待原生发送批准；不重发、不重新执行业务。', self.host)
        manager = self.task('authority-manager', 'manager', '只确认收到本Run本次要求，然后等待。不准备输入、不派发节点、不自动报告。', self.host)
        for identity, target in [('authority-task', {'task_id': worker}), ('authority-reporter', {'task_id': sender_task})]:
            self.http('/api/scheduler/bots/save', {'id': identity, 'revision': None, 'content': json.dumps({
                'name': identity, 'when': 'User explicitly requests ' + identity, 'plan_guide': 'Only the explicit one-shot user input.',
                'node_guide': 'Wait for user.', **target})})
        (self.root / 'maps/authority.json').write_text(json.dumps({'title': 'Authority check Map', 'description': 'One explicit marker only', 'nodes': {
            'check': {'title': 'Check', 'bot_id': 'authority-task', 'input': 'Acknowledge marker only', 'zone': 'reports/owned', 'blocked_by': []}}}), encoding='utf-8')
        self.http('/api/scheduler/maps/authority.json')
        self.http('/api/scheduler/bots/save', {'id': 'authority-map', 'revision': None, 'content': json.dumps({
            'name': 'authority-map', 'when': 'User explicitly requests authority-map managed marker check',
            'plan_guide': 'Use explicit manager Task and zone. No node dispatch authorized.', 'node_guide': 'Wait.', 'map_file': 'authority.json'})})
        old = self.launch(chief_task)
        self.old_pids = self.native_pids()
        require(self.old_pids, 'Old native process was not observed')
        self.authoritative(old)
        previews = []
        for bot in ['authority-task', 'authority-map']:
            payload = {'sessionId': old['sessionId'], 'botId': bot, 'zoneId': 'reports/owned', 'input': 'Only acknowledge AUTHORITY-DIAGNOSTIC and wait', 'materials': []}
            if bot == 'authority-map':
                payload['managerTaskId'] = manager
            previews.append(self.http('/api/chief/delegations/preview', payload))
        preview = self.http('/api/chief/delegations/preview', {'sessionId': old['sessionId'], 'botId': 'authority-reporter',
            'zoneId': 'reports/owned', 'input': '等待用户明确给出已结束结果，不立即报告。', 'materials': []})
        started = self.http('/api/chief/delegations', {'sessionId': old['sessionId'], 'previewId': preview['previewId']}, 202)
        sender = self.creation({**started, 'delegated': True})
        self.final_answer(sender)
        race_body = self.report_input(sender, 'AUTHORITY-BEFORE-HANDOFF-731')
        require(self.barrier.entered.wait(self.args.timeout), 'No actual native report receive request reached the barrier')
        held = next(record for record in self.barrier.records if record.get('held'))
        require(held['method'] == 'POST' and json.loads(held['request']) == {'sessionId': old['sessionId']},
            ('Handoff barrier did not suspend the actual old native chief', held, old))
        report_id = held['path'].split('/')[-2]
        race_report = self.http('/api/chief/reports/' + report_id)
        require(race_report['body'] == race_body, 'Report not persisted before real native admission')
        self.mark('external-input-already-entered-old-native-admission', request=held.copy(), report=race_report)
        self.http('/api/sessions/events', {'sessionId': old['sessionId'], 'kind': 'unregister'})
        self.mark('old-registration-removed', sessionId=old['sessionId'])
        new = self.launch(chief_task)
        require(new['sessionId'] != old['sessionId'], 'Replacement did not acquire a distinct native chief identity')
        self.mark('new-chief-acquired', current=self.authoritative(new))
        self.old_alive()
        self.barrier.release.set()
        until('actual backend denied old native receive after handoff', lambda: held.get('ended'), 30)
        require(held.get('status') == 409 and not held.get('error')
            and json.loads(held['response']).get('code') == 'chief-not-current', ('Old native receive was not refused by current authority', held))
        self.mark('old-native-receive-refused', response=held.copy())
        self.http('/api/sessions/events', {'sessionId': old['sessionId'], 'kind': 'register',
            'data': {'agentType': self.host, 'cwd': str(self.root / 'zone')}}, 409)
        self.authoritative(new)
        self.mark('late-register-denied', old=old['sessionId'], current=new['sessionId'])
        before_denied_dispatch = self.dispatch_facts()
        for preview in previews:
            denied = self.http('/api/chief/delegations', {'sessionId': old['sessionId'], 'previewId': preview['previewId']}, 409)
            require(denied.get('code') == 'chief-not-current', ('Old previously valid preview dispatch failed for a different reason', denied))
        self.native_denied(old, 'authority-task')
        self.native_denied(old, 'authority-map', manager)
        after_denied_dispatch = self.dispatch_facts()
        require(after_denied_dispatch == before_denied_dispatch, ('Denied old dispatch still produced external effects', before_denied_dispatch, after_denied_dispatch))
        self.mark('old-dispatch-no-external-effects', before=before_denied_dispatch, after=after_denied_dispatch)
        self.authoritative(new)
        self.old_alive()
        self.http('/api/chief/reports/' + report_id + '/receive', {'sessionId': old['sessionId']}, 409)
        require(self.http('/api/chief/reports/' + report_id + '/receipt') == {'received': False, 'receipt': None}, 'Revoked old native acquired receipt')
        submitted = until('native sender original report result', lambda: next((r for r in self.backend_wire.records if r['path'] == '/api/chief/reports' and r.get('ended')), None), self.args.timeout)
        original = json.loads(submitted['response'])
        require(submitted['status'] == 200 and original['notification']['status'] == 'submitted', ('Previously submitted effect was erased', original))
        self.mark('pre-handoff-submitted-effect-preserved', nativeResult=original, http=submitted)
        self.final_answer(sender, self.sender_report_start)
        revoked_phase_requests = len(self.model_wire.records)
        next_body = self.report_input(sender, 'AUTHORITY-AFTER-HANDOFF-619')
        presentation = self.final_answer(new)
        new_reports = self.http('/api/chief/reports?sessionId=' + sender['sessionId'])['reports']
        fresh = next((report for report in new_reports if report['body'] == next_body), None)
        require(fresh and fresh['id'] != report_id and len(new_reports) == 2, 'Expected a second new independent result, not resend')
        receipt = self.http('/api/chief/reports/' + fresh['id'] + '/receipt')
        current_new = self.authoritative(new)
        require(receipt['received'] and receipt['receipt']['sessionId'] == new['sessionId']
            and receipt['receipt']['claimId'] == current_new['claimId'], ('Wrong current recipient/authority claim', receipt, current_new))
        grants = [record for record in self.backend_wire.records if record['path'] == '/api/chief/reports/' + fresh['id'] + '/receive' and record.get('ended')]
        require(len(grants) == 1 and grants[0]['status'] == 200
            and json.loads(grants[0]['request']) == {'sessionId': new['sessionId']}
            and json.loads(grants[0]['response']).get('process') is True
            and json.loads(grants[0]['response']).get('receipt') == receipt['receipt'], ('Missing actual new native processing grant', grants))
        # P31 owns who can process, not P19/P28's presentation-quality rubric.
        require(any('AUTHORITY-AFTER-HANDOFF-619' in json.dumps(json.loads(record['request']), ensure_ascii=False)
            and '你是 chief：用户需求的澄清与能力选择入口' in json.dumps(json.loads(record['request']), ensure_ascii=False)
            for record in self.model_wire.records), 'No actual chief model consumed the new reliably received report')
        self.mark('new-native-recipient-granted-and-presented', report=fresh, receipt=receipt, nativeGrant=grants[0], history=presentation['history'])
        self.native_allowed(new, 'authority-task')
        self.native_allowed(new, 'authority-map', manager)
        # Input storage/display is not processing. Observe actual provider input,
        # not whether a host happens to retain a refused input in native history.
        chief_requests = [json.dumps(json.loads(record['request']), ensure_ascii=False) for record in self.model_wire.records[:revoked_phase_requests]
            if '你是 chief：用户需求的澄清与能力选择入口' in json.dumps(json.loads(record['request']), ensure_ascii=False)]
        require(chief_requests and all('AUTHORITY-BEFORE-HANDOFF-731' not in request for request in chief_requests), 'A chief model processed the revoked in-flight report')
        old_receives = [record for record in self.backend_wire.records if record['path'].endswith('/receive')
            and json.loads(record['request']).get('sessionId') == old['sessionId']]
        require(old_receives and all(record.get('status') == 409 for record in old_receives), 'Old chief obtained new native receipt authorization')
        self.mark('revoked-report-not-processed', oldHistory=self.native(old), nativeReceives=old_receives)
        snapshot = self.http('/api/chief/reports')
        histories = {chat['sessionId']: self.messages(self.native(chat)) for chat in [old, new]}
        native_operations = len(self.backend_wire.records)
        start = time.monotonic()
        while time.monotonic() - start < 46:
            self.authoritative(new)
            for chat in [old, new]:
                require(self.messages(self.native(chat)) == histories[chat['sessionId']], 'A chief performed an unrequested business action after handoff')
            require(not any(record['path'].endswith('/receive') or record['path'].startswith('/api/chief/delegations')
                for record in self.backend_wire.records[native_operations:]), 'Unrequested new receipt/dispatch after handoff')
            require(self.http('/api/chief/reports') == snapshot, 'Unrequested new report/replay during handoff observation')
            require(self.http('/api/chief/reports/' + report_id + '/receipt') == {'received': False, 'receipt': None}, 'Old pending report acquired automatic receipt')
            time.sleep(.5)
        observed_end = time.monotonic()
        self.old_alive()
        self.observe_processes()
        self.check_model_identity(0)
        self.mark('bounded-observation-complete', start=start, end=observed_end, requiredSeconds=46, actualSeconds=observed_end - start)
        self.evidence['coverage']['P31'] = {'verified': True, 'oldChief': old, 'newChief': new,
            'dispatchTargets': ['authority-task', 'authority-map'], 'newNativeDispatchTargets': ['authority-task', 'authority-map'], 'preSubmittedReportId': report_id,
            'newNativeReceivedReportId': fresh['id'], 'physicalRevocationClaimed': False, 'sameIdRecoveryClaimed': False}
        self.evidence['passed'] = True
