"""Ticket-owned P23/P24/P25/P26/P27/P34/P35 at approved S2/S3 seams.

Oracle: one immutable literal report per actual sender association; only an
explicit user request plus actual native approval can resend it. A notification
retry, duplicate processing grant, replayed initial input or changed saved body
is a plausible violation. Real HTTP, real process exits, native histories, full
UI observations and 46-second windows distinguish these violations. The external
CLI rejection and postcommit gates are labelled mechanical faults, not model
judgments. All model tool choices and presentation remain real GPT-6-luna.
"""
import concurrent.futures
import hashlib
import json
from pathlib import Path
import runpy
import shutil
import subprocess
import time
import traceback
import urllib.request

from chief_recovery_faults import RecoveryFaultProxy

shared = runpy.run_path(str(Path(__file__).with_name('chief_reports_native_acceptance.py')))
Base, require, until = shared['ChiefReportsAcceptance'], shared['require'], shared['until']
PROMISES = ['P23', 'P24', 'P25', 'P26', 'P27', 'P34', 'P35']


class ChiefRecoveryAcceptance(Base):
    def __init__(self, args):
        super().__init__(args)
        self.evidence.update(scenario='chief-recovery', promises=PROMISES,
            coverage={key: {'verified': False} for key in PROMISES}, cases=[], windows=[],
            scenarioSource={'path': str(Path(__file__).resolve()), 'sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()})
        self.confirmations = self.root / 'native-confirmations'
        self.confirmations.mkdir()
        self.env['CHIEF_MAP_CONFIRMATION_EVIDENCE_DIR'] = str(self.confirmations)
        self.known_pids = set()
        self.chat_pids = {}

    def start_backend(self, role_config):
        proxy = self.root / 'external-cli'
        proxy.mkdir(exist_ok=True)
        executable = proxy / 'recovery_herdr_proxy.exe'
        source = self.args.backend.parent / 'examples/recovery_herdr_proxy.exe'
        require(source.is_file(), 'Build this worktree --example recovery_herdr_proxy')
        shutil.copy2(source, executable)
        (proxy / 'recovery-proxy.json').write_text(json.dumps({'herdr': str(self.args.herdr)}), encoding='utf-8')
        observer = self.root / 'confirmation-observer.mjs'
        shutil.copy2(self.args.repo / 'scripts/chief-ui-observer.mjs', observer)
        with (self.root / 'options.toml').open('a', encoding='utf-8') as stream:
            stream.write('\n[[extensions]]\nid = "recovery-confirmation-observer"\nlabel = "Acceptance UI observer"\nagent = '
                + json.dumps(self.host) + '\nkind = "fixed"\nentrypoint = ' + json.dumps(str(observer)) + '\n')
        real = self.args.herdr
        try:
            self.args.herdr = executable
            super().start_backend(role_config)
        finally:
            self.args.herdr = real
        self.backend_process = self.processes[-1]
        self.evidence['externalFault'] = {'kind': 'Controlled rejection before accepting notification; otherwise installed Herdr forwarding',
            'source': str(self.args.repo / 'backend/examples/recovery_herdr_proxy.rs'),
            'sourceSha256': hashlib.sha256((self.args.repo / 'backend/examples/recovery_herdr_proxy.rs').read_bytes()).hexdigest(),
            'httpFaultSourceSha256': hashlib.sha256((self.args.repo / 'scripts/chief_recovery_faults.py').read_bytes()).hexdigest(),
            'binarySha256': hashlib.sha256(executable.read_bytes()).hexdigest()}

    def prepare(self):
        super().prepare()
        self.backend_wire.close()
        self.backend_wire = RecoveryFaultProxy(self.base, self.root / 'recovery-http')
        self.role_config = self.root / 'toolkit/extensions/workflow-chat/config.yaml'
        self.role_config.write_text(f'endpoint: {self.backend_wire.base}\nrequestTimeoutMs: 30000\nreportConfirmationTimeoutMs: 20000\ncollectTimeoutMs: 5460000\n', encoding='utf-8')

    def track(self, chat):
        pids = self.native_pids()
        owned = pids - self.known_pids
        require(owned, ('No new installed native process', chat))
        self.known_pids.update(pids)
        self.chat_pids[chat['sessionId']] = owned
        self.chat_hosts[chat['sessionId']] = self.host
        self.size(chat)
        return chat

    def creation(self, started):
        return self.track(super().creation(started))

    def size(self, chat):
        argv = [str(self.args.herdr), 'terminal', 'session', 'control', chat['paneId'], '--cols', '2048', '--rows', '2000']
        result = subprocess.run(argv, cwd=self.root, env=self.env, input='', capture_output=True,
            text=True, encoding='utf-8', errors='replace', timeout=15)
        self.evidence.setdefault('nativeViewport', []).append({'argv': argv, 'exit': result.returncode,
            'stdout': result.stdout, 'stderr': result.stderr})
        require(result.returncode == 0, 'Could not resize owned native confirmation terminal')

    def close_chat(self, chat):
        self.native(chat)
        self.herdr('pane', 'close', chat['paneId'])
        owned = self.chat_pids[chat['sessionId']]
        for pid, handle in self.handles:
            if pid in owned:
                until('actual native process exit ' + str(pid), lambda: not handle.alive(), 15)
                self.evidence.setdefault('terminatedNativeProcesses', []).append({'sessionId': chat['sessionId'], 'pid': pid, 'exited': True})
        request = urllib.request.Request(self.base + '/api/sessions/events',
            data=json.dumps({'sessionId': chat['sessionId'], 'kind': 'unregister'}).encode(),
            headers={'Content-Type': 'application/json'})
        try:
            response = urllib.request.urlopen(request, timeout=30)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            body = json.load(response)
            self.evidence.setdefault('closedRegistrations', []).append({'sessionId': chat['sessionId'], 'status': response.status, 'body': body})
            require(response.status == 200 or (response.status == 404 and body.get('code') == 'unknown-session'),
                ('Could not unregister terminated native Chat', response.status, body))

    def restore(self, chat):
        previous = self.native(chat)
        messages = [message for message in self.messages(previous) if message.get('role') in ['user', 'assistant', 'toolResult']]
        reports = self.http('/api/chief/reports')
        requests = len(self.model_wire.records)
        self.close_chat(chat)
        outcome = self.http('/api/chats/resume', {'sessionId': chat['sessionId']})
        require(outcome['outcome'] == 'restored', ('Expected actual new process', outcome))
        restored = self.track({**outcome['target'], 'sessionId': chat['sessionId']})
        actual = self.native(restored)
        require([message for message in self.messages(actual) if message.get('role') in ['user', 'assistant', 'toolResult']] == messages,
            'New-process recovery replayed initial input or changed native business history')
        require(len(self.model_wire.records) == requests and self.http('/api/chief/reports') == reports,
            'Recovery invoked a model or created a business report before observation began')
        self.evidence.setdefault('restores', []).append({'original': chat, 'restored': restored, 'historyBefore': previous, 'historyAfter': actual})
        return restored

    def restart_backend(self):
        before = self.backend_process.child.pid
        self.backend_process.stop()
        require(self.backend_process.child.poll() is not None, 'Original backend survived termination')
        path = self.root / 'backend.toml'
        path.write_text(path.read_text(encoding='utf-8').replace('127.0.0.1:0', self.base.removeprefix('http://')), encoding='utf-8')
        self.backend_process = self.start([self.args.backend, '--config', path], 'actual-restart-' + str(len(self.processes)))
        def ready():
            require(self.backend_process.child.poll() is None, 'Restarted backend exited')
            while not self.backend_process.lines.empty():
                if self.base in self.backend_process.lines.get_nowait():
                    return True
        until('actual backend restart binds same owned port', ready, 45)
        self.evidence.setdefault('backendRestarts', []).append({'oldPid': before, 'newPid': self.backend_process.child.pid, 'oldExited': True})

    def http(self, path, body=None, status=200):
        operation = {'path': path, 'method': 'GET' if body is None else 'POST',
            'request': body, 'expectedStatus': status, 'started': time.monotonic()}
        self.evidence.setdefault('publicOperations', []).append(operation)
        try:
            result = super().http(path, body, status)
            operation['response'] = result
            return result
        finally:
            operation['ended'] = time.monotonic()

    def calls(self):
        path = self.root / 'external-cli/calls.jsonl'
        return [json.loads(line) for line in path.read_text(encoding='utf-8').splitlines()] if path.exists() else []

    def business(self):
        sessions = sorted(item['sessionId'] for item in self.http('/api/sessions/history')['sessions'])
        return {'sessions': sessions,
                'delegations': {session: self.http('/api/chief/delegations?sessionId=' + session)['delegations']
                                for session in sessions},
                'runs': self.http('/api/workflow/runs')['runs']}

    def business_request_count(self):
        count = 0
        for record in self.model_wire.records:
            request = json.loads(record['request'])
            messages = request.get('messages', [])
            system = messages[0] if messages else {}
            if system.get('role') == 'system' and str(system.get('content', '')).startswith('You name coding-agent sessions.'):
                continue
            count += 1
        return count

    def quiet(self, label, chats=()):
        baseline = {'reports': self.http('/api/chief/reports'), 'business': self.business(),
                    'notifications': len([call for call in self.calls() if call['notification']]),
                    'messages': {chat['sessionId']: self.messages(self.native(chat)) for chat in chats},
                    'requests': self.business_request_count()}
        start = time.monotonic()
        while time.monotonic() - start < 46:
            require(self.http('/api/chief/reports') == baseline['reports'], ('Automatic new report', label))
            require(self.business() == baseline['business'], ('Automatic business dispatch', label))
            require(len([call for call in self.calls() if call['notification']]) == baseline['notifications'], ('Automatic notification', label))
            require(self.business_request_count() == baseline['requests'], ('Automatic business model invocation', label))
            for chat in chats:
                require(self.messages(self.native(chat)) == baseline['messages'][chat['sessionId']], ('Automatic conversation replay', label))
            time.sleep(.5)
        self.evidence['windows'].append({'label': label, 'start': start, 'end': time.monotonic(), 'minimumSeconds': 46,
            'budgets': {'serviceStartup': 45, 'nativeStartup': 30, 'request': 30, 'nativeApproval': 20,
                        'online': 10, 'retention': 30, 'maintenanceScan': 5}, 'baseline': baseline})

    def confirmation(self, chat, tool, prior_ids):
        def visible():
            terminal = self.command([self.args.herdr, 'pane', 'read', chat['paneId'], '--lines', '1000', '--format', 'text'])
            candidates = [json.loads(file.read_text(encoding='utf-8')) for file in self.confirmations.glob('*.request.json')]
            candidates = [item for item in candidates if item['sessionId'] == chat['sessionId'] and item['id'] not in prior_ids]
            if not candidates or 'Yes' not in terminal or 'No' not in terminal:
                return None
            history = self.native(chat)
            calls = [part for message in self.messages(history) for part in message.get('content', [])
                     if isinstance(part, dict) and part.get('type') == 'toolCall' and part.get('name') == tool]
            return {'ui': candidates[-1], 'terminal': terminal, 'call': calls[-1], 'history': history} if calls else None
        result = until('actual native confirmation ' + tool, visible, self.args.timeout)
        require(result['ui']['timeoutMs'] == 20000, 'Unexpected actual native approval budget')
        return result

    def confirmation_ids(self):
        return {json.loads(path.read_text(encoding='utf-8'))['id'] for path in self.confirmations.glob('*.request.json')}

    def decide(self, chat, confirmation, approved):
        if not approved:
            self.herdr('agent', 'send-keys', chat['paneId'], 'down')
            until('visible native No selection', lambda: any(marker in self.command([self.args.herdr, 'pane', 'read', chat['paneId'], '--lines', '1000', '--format', 'text']) for marker in ['→ No', '❯ No']), 5)
        at = time.monotonic()
        self.herdr('agent', 'send-keys', chat['paneId'], 'enter')
        path = self.confirmations / (confirmation['ui']['id'] + '.response.json')
        actual = until('actual native confirmation result', lambda: json.loads(path.read_text(encoding='utf-8')) if path.exists() else None, 5)
        require(actual.get('result') is approved, ('Native confirmation mismatch', actual))
        confirmation.update(approvedAt=at, observedResponse=actual)
        self.evidence.setdefault('nativeConfirmations', []).append(confirmation)

    def explicit_resend(self, sender, report, approved=True, before_decision=None):
        previous = self.native(sender)
        start = len(previous['branch'])
        ids = self.confirmation_ids()
        before = len(self.backend_wire.records)
        self.send(sender, '请重发已保存的原chief报告 ' + report['id'] + '，原ID和完整内容不变；不是提交新报告。本次必须再次原生确认，未批准不发送。', 'explicit-original-resend')
        confirmation = self.confirmation(sender, 'chief_resend_report', ids)
        arguments = dict(confirmation['call']['arguments'])
        if self.host == 'omp' and isinstance(arguments.get('i'), str):
            arguments.pop('i')  # Installed OMP's tool intent metadata is not an HTTP/business parameter.
        require(arguments == {'id': report['id']}, 'Real model did not select original-only resend')
        preview_text = confirmation['ui']['message'].split('原报告：\n', 1)
        require(len(preview_text) == 2, 'Native preview omitted original report')
        preview_report, _ = json.JSONDecoder().raw_decode(preview_text[1])
        require(preview_report == report, 'Native preview omitted or changed original ID/content')
        route = '/api/chief/reports/' + report['id'] + '/resend'
        require(not any(record['path'] == route for record in self.backend_wire.records[before:]), 'Resent before native approval')
        if before_decision:
            before_decision()
        self.decide(sender, confirmation, approved)
        result = self.final_answer(sender, start)
        posts = [record for record in self.backend_wire.records[before:] if record['path'] == route]
        require(len(posts) == int(approved), ('Resend count differs from actual approval', posts))
        if posts:
            require(json.loads(posts[0]['request']) == {'sessionId': sender['sessionId']} and posts[0]['started'] >= confirmation['approvedAt'], 'Resend changed body/identity or preceded approval')
        require(self.http('/api/chief/reports/' + report['id']) == report, 'Original report was mutated')
        return result, posts

    def register(self, chat):
        self.native(chat)
        file = None
        for candidate in (self.root / 'agent').rglob('*.jsonl'):
            with candidate.open(encoding='utf-8') as stream:
                headers = []
                for _ in range(2):
                    line = stream.readline()
                    if not line:
                        break
                    headers.append(json.loads(line))
            if any(value.get('type') == 'session' and value.get('id') == chat['sessionId'] for value in headers):
                file = candidate
                break
        require(file, 'Owned original native session file not found')
        self.http('/api/sessions/events', {'sessionId': chat['sessionId'], 'kind': 'register',
            'data': {'cwd': str(self.root / 'zone'), 'agentType': self.host, 'sessionFile': str(file)}})

    def setup_sender(self, chief, kind, task, body):
        bot = 'recovery-' + kind
        target = {'task_id': task}
        if kind == 'manager':
            worker = self.task('unstarted internal worker', 'normal', 'Wait for explicit user authorization.', self.host)
            self.http('/api/scheduler/bots/save', {'id': 'internal-recovery', 'revision': None, 'content': json.dumps({
                'name': 'Unstarted internal node', 'when': 'Internal only', 'plan_guide': '', 'node_guide': '', 'task_id': worker})})
            value = {'title': 'Recovery-only direct Map', 'description': 'No node dispatch is authorized', 'nodes': {
                'pending': {'title': 'Unstarted', 'bot_id': 'internal-recovery', 'input': 'Wait', 'zone': 'reports/owned',
                    'blocked_by': [], 'input_rule': [], 'output_rule': [], 'report_policy': 'manual'}}}
            (self.root / 'maps/recovery.json').write_text(json.dumps(value), encoding='utf-8')
            self.http('/api/scheduler/maps/recovery.json')
            target = {'map_file': 'recovery.json'}
        self.http('/api/scheduler/bots/save', {'id': bot, 'revision': None, 'content': json.dumps({
            'name': bot, 'when': 'Report this isolated evidence only', 'plan_guide': 'Do not execute or dispatch any business', 'node_guide': '', **target})})
        user = 'REPORT-EVIDENCE：已结束操作的独立事实如下。请完整报告这些事实，不执行业务、不派发node、不collect、不创建新Run；本次报告须原生批准。\n' + json.dumps(body, ensure_ascii=False)
        request = {'sessionId': chief['sessionId'], 'botId': bot, 'zoneId': 'reports/owned', 'input': user, 'materials': []}
        if kind == 'manager':
            request['managerTaskId'] = task
        preview = self.http('/api/chief/delegations/preview', request)
        started = self.http('/api/chief/delegations', {'sessionId': chief['sessionId'], 'previewId': preview['previewId']}, 201 if kind == 'manager' else 202)
        if kind == 'manager':
            run = until('actual root Map manager identity', lambda: self.detail(started['id'])['run'].get('manager'), self.args.timeout)
            sender = self.track(run)
        else:
            sender = self.creation({**started, 'delegated': True})
        return sender

    def fresh_chief(self, task):
        chat = self.launch(task, '这是一个全新 chief 会话。请仅回复已就绪，然后等待用户；不调用工具、不查报告、不派发业务。')
        answer = self.final_answer(chat)
        require(not any(part.get('type') == 'toolCall'
                        for message in self.messages(answer['history'])
                        for part in message.get('content', []) if isinstance(part, dict)),
                'Fresh chief performed work before a user recovery request')
        return chat

    def exercise(self):
        self.http('/api/zones', {'id': 'reports/owned', 'path': str(self.root / 'zone')})
        chief_task = self.task('Recovery chief', 'chief', '', self.host)
        chief = self.fresh_chief(chief_task)
        original = {'background': '用户需要私有导出，未授权发布或重跑。', 'execution': '隔离导出 recovery-731', 'result': 'blocked',
            'summary': 'RECOVERY-731：缺少发布许可，已停止，没有导出或业务重跑。', 'deliverables': [], 'pendingDecisions': ['判断本次受阻说明是否满足要求。']}
        reject = self.root / 'external-cli/reject-notification'
        reject.write_text('owned explicit notification fault', encoding='utf-8')
        task = self.task('Recovery sender', 'normal', '只有用户明确提供REPORT-EVIDENCE时报告已结束事实；其余时候等待用户。不执行或重跑业务。保留原报告ID，人工重发使用原报告工具并再次原生确认。', self.host)
        ids = self.confirmation_ids()
        sender = self.setup_sender(chief, 'task', task, original)
        pending = self.confirmation(sender, 'chief_report', ids)
        self.decide(sender, pending, True)
        answer = self.final_answer(sender)
        report = until('actual saved report', lambda: next(iter(self.http('/api/chief/reports?sessionId=' + sender['sessionId'])['reports']), None), self.args.timeout)
        require(report['body']['result'] == 'blocked' and 'RECOVERY-731' in report['body']['summary'], 'Sender changed independent business outcome')
        submit = until('real notification failure result', lambda: next((record for record in self.backend_wire.records if record['path'] == '/api/chief/reports' and record.get('ended')), None), self.args.timeout)
        require(json.loads(submit['response'])['notification']['status'] == 'failed', ('Expected definite external notification failure', submit))
        report_calls = [part['name'] for message in self.messages(answer['history']) for part in message.get('content', [])
            if isinstance(part, dict) and part.get('type') == 'toolCall' and part.get('name') in ['chief_report', 'chief_resend_report']]
        require(report_calls == ['chief_report'], ('Sender automatically retried or opened recovery before explicit user input', report_calls))
        self.evidence['initialFailure'] = {'report': report, 'http': submit, 'senderHistory': answer['history']}
        self.quiet('after-notification-failure', [sender, chief])
        reject.unlink()
        self.close_chat(chief)
        chief = self.fresh_chief(chief_task)
        self.quiet('after-chief-created', [sender, chief])
        chief = self.restore(chief)
        self.quiet('after-chief-new-process-resume', [sender, chief])
        self.register(chief)
        self.quiet('after-chief-reregister', [sender, chief])
        self.restart_backend()
        self.register(chief)
        self.quiet('after-backend-real-restart', [sender, chief])
        self.quiet('elapsed-time-without-explicit-user-input', [sender, chief])
        self.evidence['coverage']['P23'] = {'verified': True, 'windows': [window['label'] for window in self.evidence['windows']]}
        self.explicit_resend(sender, report, False)
        self.quiet('after-resend-native-denial', [sender, chief])
        receive_path = '/api/chief/reports/' + report['id'] + '/receive'
        resend_path = '/api/chief/reports/' + report['id'] + '/resend'
        gate = self.backend_wire.arm(receive_path)
        lost = self.backend_wire.arm(resend_path, drop=True)
        chief_before = self.native(chief)
        def interrupt_after_commit():
            # The approval itself occurs after this callback. A separate worker
            # waits for the real receive commit, then kills only that native Chat.
            def crash():
                require(gate.committed.wait(45), 'No actual native receive reached durable commit')
                require(json.loads(gate.record['response'])['process'] is True, 'First actual native receive did not grant processing')
                require(self.messages(self.native(chief)) == self.messages(chief_before), 'Chief presented before held admission was released')
                require(self.http('/api/chief/reports/' + report['id'] + '/receipt')['received'] is True, 'Receipt not publicly durable before crash')
                self.close_chat(chief)
                gate.release.set()
                require(lost.committed.wait(45), 'No sender resend result committed')
                lost.release.set()
            self.crash_pool = concurrent.futures.ThreadPoolExecutor(max_workers=1)
            self.crash_future = self.crash_pool.submit(crash)
        try:
            self.explicit_resend(sender, report, True, interrupt_after_commit)
            self.crash_future.result(timeout=60)
        finally:
            gate.release.set()
            lost.release.set()
            if hasattr(self, 'crash_pool'):
                self.crash_pool.shutdown(wait=True)
                error = self.crash_future.exception()
                if error:
                    self.evidence.setdefault('faultWorkerErrors', []).append(''.join(traceback.format_exception(error)))
        receipt = self.http('/api/chief/reports/' + report['id'] + '/receipt')
        require(receipt['received'] is True and receipt['receipt']['sessionId'] == chief['sessionId'], 'Lost real persistent receipt')
        require(set(receipt) == {'received', 'receipt'} and set(receipt['receipt']) == {'reportId', 'sessionId', 'claimId', 'at'}, 'Receipt invented presentation/read/acceptance state')
        self.evidence['interruption'] = {'chiefBefore': chief_before, 'receive': gate.record, 'lostSenderResponse': lost.record, 'receipt': receipt}
        self.evidence['coverage']['P24'] = {'verified': True, 'explicitNativeApproveAndDeny': True, 'original': report}
        chief = self.fresh_chief(chief_task)
        fresh = self.native(chief)
        require(fresh and report['id'] not in json.dumps(fresh, ensure_ascii=False)
            and 'RECOVERY-731' not in json.dumps(fresh, ensure_ascii=False), 'Fresh chief inherited old delegation context')
        self.quiet('received-before-presentation-fresh-chief-no-replay', [sender, chief])
        self.evidence['coverage']['P26'] = {'verified': True, 'interruption': 'held original postcommit receive response, terminated native process before model admission'}
        view_start = len((self.native(chief) or {'branch': []})['branch'])
        self.send(chief, '用户明确要求查看已经可靠接收但呈现中断的原报告 ' + report['id'] + '。请读取原报告并准确呈现，只询问是否满足要求后等待；不重新派发或重跑业务。', 'explicit-view-after-presentation-interruption')
        presentation = self.final_answer(chief, view_start)
        views = [record for record in self.backend_wire.records if record['path'] == '/api/chief/reports/' + report['id'] + '/view']
        require(len(views) == 1 and json.loads(views[0]['response'])['report'] == report, 'Fresh chief did not actually read unchanged original')
        receiver_calls = [part for message in self.messages(presentation['history'], view_start) for part in message.get('content', [])
            if isinstance(part, dict) and part.get('type') == 'toolCall']
        require(all(call.get('name') == 'chief_read_report' for call in receiver_calls),
            ('Manual recovery presented the report by performing other business work', receiver_calls))
        self.evaluate('recovery-blocked', report, presentation['answer'], original)
        self.quiet('after-explicit-view-no-business', [sender, chief])
        self.evidence['manualView'] = {'http': views[0], 'history': presentation['history'], 'answer': presentation['answer']}
        self.evidence['coverage']['P27'] = {'verified': True, 'explicitActualRead': views[0]['index']}
        before = len([call for call in self.calls() if call['notification']])
        overlap = self.backend_wire.arm(resend_path)
        def resend_over_wire():
            request = urllib.request.Request(self.backend_wire.base + resend_path,
                data=json.dumps({'sessionId': sender['sessionId']}).encode(), headers={'Content-Type': 'application/json'})
            with urllib.request.urlopen(request, timeout=60) as response:
                return json.load(response)
        with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
            first = pool.submit(resend_over_wire)
            try:
                require(overlap.committed.wait(15), 'Concurrent resend did not reach the real backend')
                second = pool.submit(resend_over_wire)
                second_result = second.result(timeout=15)
                require(not first.done(), 'Concurrent resend scenario did not overlap outstanding original response')
            finally:
                overlap.release.set()
            results = [first.result(timeout=15), second_result]
        require(all(item['received'] and item['report'] == report and item['notification']['status'] == 'not-attempted' for item in results), 'Concurrent resend repeated processing/changed original')
        self.restart_backend()
        self.register(chief)
        require(self.http('/api/chief/reports/' + report['id'] + '/receipt') == receipt, 'Receipt changed/lost across actual backend restart')
        sender = self.restore(sender)
        business = self.business()
        self.quiet('restored-task-sender-does-not-replay-input', [sender, chief])
        _, posts = self.explicit_resend(sender, report)
        feedback = json.loads(posts[0]['response'])
        require(feedback['received'] is True and feedback['notification']['status'] == 'not-attempted'
            and feedback['receipt'] == receipt, 'Native recovered resend did not report the unchanged existing receipt without another notification')
        require(len([call for call in self.calls() if call['notification']]) == before, 'Already received report notified again')
        require(self.business() == business, 'Sender recovery/new resend created a delegation or Run')
        grants = [record for record in self.backend_wire.records if record['path'] == receive_path and record.get('response') and json.loads(record['response']).get('process')]
        require(len(grants) == 1, ('More than one actual processing grant', grants))
        self.evidence['coverage']['P25'] = {'verified': True, 'concurrentResults': results, 'originalReceipt': receipt, 'processingGrants': grants}
        known_sessions = {item['sessionId'] for item in self.http('/api/sessions/history')['sessions']}
        preview = self.http('/api/chats/preview', {'zone': 'reports/owned', 'taskId': task})
        started = self.http('/api/chats', {'zone': 'reports/owned', 'taskId': task, 'chatId': preview['chatId']}, 202)
        def unrelated_ready():
            state = self.http('/api/chats/' + started['id'])
            require(state['stage'] != 'failed', state)
            candidates = [item for item in self.http('/api/sessions/history')['sessions'] if item['sessionId'] not in known_sessions]
            return {**state, 'sessionId': candidates[0]['sessionId']} if state['stage'] == 'succeeded' and len(candidates) == 1 else None
        other = self.track(until('same Task separate actual native identity', unrelated_ready, self.args.timeout))
        # Same source Task, real separate native Chat, restored in a new process.
        self.final_answer(other)
        other = self.restore(other)
        denied = self.http(resend_path, {'sessionId': other['sessionId']}, 403)
        require(self.http('/api/chief/delegations?sessionId=' + other['sessionId'])['delegations'] == [], 'Same Task granted a false association')
        self.evidence['sameTaskUnassociated'] = {'chat': other, 'resend': denied}
        manager_task = self.task('Recovery direct manager', 'manager', '仅在REPORT-EVIDENCE时报告指定已结束事实，不派发node、不collect、不重跑。用户要求重发时保留原ID并原生确认。', self.host)
        ids = self.confirmation_ids()
        manager = self.setup_sender(chief, 'manager', manager_task, original)
        pending = self.confirmation(manager, 'chief_report', ids)
        self.decide(manager, pending, True)
        self.final_answer(manager)
        manager_report = until('direct manager saved report', lambda: next(iter(self.http('/api/chief/reports?sessionId=' + manager['sessionId'])['reports']), None), self.args.timeout)
        self.final_answer(chief, len(presentation['history']['branch']))
        manager = self.restore(manager)
        business = self.business()
        self.quiet('restored-direct-map-manager-does-not-replay-input', [manager])
        _, posts = self.explicit_resend(manager, manager_report)
        require(json.loads(posts[0]['response'])['report'] == manager_report and self.business() == business, 'Restored direct manager lost original association or redispatched')
        self.evidence['coverage']['P34'] = {'verified': True, 'task': sender, 'manager': manager, 'managerReport': manager_report, 'sameTaskDenied': other}
        # The changed result is a NEW report, not a body-bearing resend. Reject
        # that new confirmation and prove neither new ID nor modified old body.
        reports = self.http('/api/chief/reports')
        ids = self.confirmation_ids()
        start = len(self.native(sender)['branch'])
        self.send(sender, '现在有一条新事实：RECOVERY-REVISION-947，用户已补充许可但业务仍未重新执行。请把它作为明确的新报告，不沿用或修改原ID；必须对新内容再次原生确认，尚未批准不发送。', 'explicit-changed-new-report')
        changed = self.confirmation(sender, 'chief_report', ids)
        require('RECOVERY-REVISION-947' in json.dumps(changed['call']['arguments'], ensure_ascii=False), 'New report did not contain independent changed fact')
        self.decide(sender, changed, False)
        self.final_answer(sender, start)
        require(self.http('/api/chief/reports') == reports, 'Denied new report was saved/sent')
        changed_body = {**report['body'], 'summary': 'RECOVERY-REVISION-947'}
        self.http(resend_path, {'sessionId': sender['sessionId'], 'body': changed_body}, 400)
        require(self.http('/api/chief/reports/' + report['id']) == report, 'Body-bearing resend mutated original')
        self.evidence['coverage']['P35'] = {'verified': True, 'oldReport': report, 'newContentNativeDenial': changed, 'bodyBearingResendRejected': True}
        self.check_model_identity(0)
        self.evidence['passed'] = True
