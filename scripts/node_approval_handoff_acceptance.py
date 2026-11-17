"""Ticket 13's approved S2 seams: real native tools/history, public HTTP, files.

Independent business oracle: seven units at 1200 cents, 25 percent discount,
produce total 6300 cents. Initially the discount is unknown, so a saved issue
report can precede approval; after the user supplies it, the model decides when
work is ready. The driver never mentions tool names/order in business input.
Plausible violations: reporting a normal result before approval, fabricating user
Yes in auto mode, gating issue reports, or waiting for the end Chat to finish.
Raw native calls/results, transparent traffic, actual bytes and public Run facts
are retained. Controlled responses cannot satisfy these checks.
"""
import hashlib
import json
from pathlib import Path
import shutil
import time

from acceptance.host.native import NativeAcceptance
from acceptance.host.processes import require, until
from acceptance.host.wire import decode

ITEMS = ['P20/order', 'P20/progress-before-approval', 'P20/approval-source', 'P30/end-node-finish']
QUOTE = {'quantity': 7, 'unitPriceCents': 1200, 'discountPercent': 25, 'totalCents': 6300}


def calls(history):
    return [part for entry in history['branch'] if entry.get('type') == 'message'
        and entry['message'].get('role') == 'assistant'
        for part in entry['message'].get('content', [])
        if isinstance(part, dict) and part.get('type') == 'toolCall']


def tool_name(call):
    return call.get('name', '').rsplit('.', 1)[-1]


def require_order_input(definition):
    """Fail before any model starts when the published fixture cannot bind order."""
    declarations = definition.get('input_files', [])
    require(len(declarations) == 1 and declarations[0].get('id') == 'order'
        and declarations[0].get('required') is True, 'Handoff Map must declare required order input')
    require(definition['nodes']['work']['input_rule'] == [{'id': 'order', 'type': 'run', 'input_id': 'order'}],
        'Handoff work must consume the declared Run order input')
    require(isinstance(definition.get('input_requirements'), str) and definition['input_requirements'].strip(),
        'Handoff input preparation needs explicit business readiness requirements')


class NodeApprovalHandoff(NativeAcceptance):
    def __init__(self, args):
        super().__init__(args)
        self.mode = args.mode
        self.evidence.update(promise=None, promises=['P20', 'P30/S2'], scenario='node-approval-handoff', mode=self.mode,
            coverage={key: {'verified': False} for key in ITEMS}, modelIdentities=[],
            independentExpectedQuote=QUOTE,
            scenarioSource={'path': str(Path(__file__).resolve()), 'sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()},
            budgets={'confirmationMs': 20000, 'requestMs': 30000, 'observationSeconds': 51,
                'reason': 'Existing 20s native confirmation + 30s HTTP request budgets + 1s observation margin; no product timeout changes.'})
        self.env.update(OMP_CODING_AGENT_DIR=str(self.root / 'agent'), XDG_DATA_HOME=str(self.root / 'state'))
        self.evidence['isolation'].update({key: self.env[key] for key in ['OMP_CODING_AGENT_DIR', 'XDG_DATA_HOME']})
        self.confirmations = self.root / 'native-confirmations'
        self.confirmations.mkdir()
        self.env['CHIEF_MAP_CONFIRMATION_EVIDENCE_DIR'] = str(self.confirmations)
        self.clock = {'wall': time.time(), 'monotonic': time.monotonic()}
        self.evidence['clockReference'] = self.clock

    def start_backend(self, role_config):
        observer = self.root / 'confirmation-observer.mjs'
        source = self.args.repo / 'scripts/chief-ui-observer.mjs'
        shutil.copy2(source, observer)
        with (self.root / 'options.toml').open('a', encoding='utf-8') as stream:
            stream.write('\n[[extensions]]\nid = ' + json.dumps(self.host + '-handoff-observer')
                + '\nlabel = "Native UI observation"\nagent = ' + json.dumps(self.host)
                + '\nkind = "fixed"\nentrypoint = ' + json.dumps(str(observer)) + '\n')
        self.evidence['confirmationObserver'] = {'source': str(source), 'sha256': hashlib.sha256(source.read_bytes()).hexdigest(),
            'scope': 'Pass-through of installed host public UI; never supplies arguments, decisions or returns.'}
        super().start_backend(role_config)

    def prepare(self):
        super().prepare()
        addresses = {'backend': self.base, 'backendObserver': self.backend_wire.base, 'modelObserver': self.model_wire.base}
        self.evidence['ownedLoopbackAddresses'] = addresses
        for name, address in addresses.items():
            port = int(address.rsplit(':', 1)[1])
            require(address.startswith('http://127.0.0.1:') and 49152 <= port <= 65535,
                ('Owned listener must use numeric dynamic loopback', name, address))

    def records(self, route, session=None):
        return [record for record in self.backend_wire.records if record['path'] == route
            and (session is None or (decode(record, 'request') or {}).get('sessionId') == session)]

    def chat(self, value):
        self.chat_hosts[value['sessionId']] = self.host
        return value

    def turn(self, chat):
        def ready():
            history = self.native(chat)
            messages = [entry['message'] for entry in history['branch'] if entry.get('type') == 'message'] if history else []
            if not messages or messages[-1].get('role') != 'assistant':
                return None
            last = messages[-1]
            require(last.get('stopReason') not in ['error', 'aborted'], ('Actual native turn failed; no retry', last))
            return history if last.get('stopReason') == 'stop' else None
        return until('actual native completed assistant turn', ready, self.args.timeout)

    def quote(self):
        path = self.root / 'work-zone/quote.json'
        require(path.is_file(), ('Missing real business material', str(path)))
        actual = json.loads(path.read_text(encoding='utf-8-sig'))
        require(actual == QUOTE, ('Business quote differs from independently worked literal', actual, QUOTE))
        return {'path': str(path), 'bytes': path.read_text(encoding='utf-8-sig'),
            'sha256': hashlib.sha256(path.read_bytes()).hexdigest(), 'mtime': path.stat().st_mtime, 'observedAt': time.monotonic()}

    def approve_native_yes(self, chat):
        def visible():
            requests = [json.loads(path.read_text(encoding='utf-8')) for path in self.confirmations.glob('*.request.json')]
            invocation = next((value for value in requests if value.get('sessionId') == chat['sessionId']
                and value.get('title') == '成果批准'), None)
            if not invocation:
                return None
            screen = self.command([self.args.herdr, 'pane', 'read', chat['paneId'], '--lines', '1000', '--format', 'text'])
            return (invocation, screen) if '当前会话的成果是否已达到预期，可以交接并推进后续流程？' in screen and 'Yes' in screen and 'No' in screen else None
        invocation, screen = until('actual native result approval with visible Yes/No', visible, self.args.timeout)
        require(invocation['hasUI'] is True and invocation['timeoutMs'] == 20000, invocation)
        require(invocation['message'] == '当前会话的成果是否已达到预期，可以交接并推进后续流程？', invocation)
        require('→ Yes' in screen or '❯ Yes' in screen, 'Native Yes is not visibly selected; do not guess keys')
        require(not self.records('/api/workflow/node/approve', chat['sessionId']), 'Manual approval submitted before actual user Yes')
        before = self.detail(self.run_id)
        require(before['run']['nodes']['work']['execution']['approved'] is False, before)
        material = self.quote()
        self.evidence['beforeNativeYes'] = {'screen': screen, 'invocation': invocation, 'public': before, 'material': material,
            'at': time.monotonic()}
        self.herdr('agent', 'send-keys', chat['paneId'], 'enter')
        response = self.confirmations / (invocation['id'] + '.response.json')
        decision = until('actual public native confirmation return', lambda: json.loads(response.read_text(encoding='utf-8')) if response.is_file() else None, 20)
        require(decision.get('result') is True, ('A key write is not native Yes', decision))
        self.evidence['nativeYes'] = decision

    def check_model_identity(self):
        configured = json.loads(self.args.model_config.read_text(encoding='utf-8'))['model']
        def completion(record):
            return record.get('method') == 'POST' and record.get('path') == '/chat/completions'
        # Native title generation may still be finishing; observe, never resend.
        until('owned model requests finish within the existing request budget',
            lambda: all(record.get('ended') for record in self.model_wire.records if completion(record)), 30)
        records = [record for record in self.model_wire.records if completion(record)]
        self.evidence['nonCompletionTrafficCount'] = len(self.model_wire.records) - len(records)
        require(records, 'No actual model completion traffic')
        identities = []
        for record in records:
            require(record.get('ended') and record.get('status') == 200 and not record.get('error'), ('Actual model request failed; no retry', record))
            request = decode(record, 'request')
            require(request.get('model') == configured, ('Requested model differs from authorization', request.get('model')))
            raw = record['response']
            packets = [json.loads(raw)] if raw.lstrip().startswith('{') else [json.loads(line[5:].strip()) for line in raw.splitlines()
                if line.startswith('data:') and line[5:].strip() not in ['', '[DONE]']]
            actual = sorted({packet['model'] for packet in packets if packet.get('model')})
            require(actual == [configured], ('Actual returned model identity differs from authorization', record['index'], actual))
            identities.append({'request': record['index'], 'requested': configured, 'returned': actual})
        self.evidence['modelIdentities'] = identities

    def exercise(self):
        for name in ['work', 'end']:
            directory = self.root / (name + '-zone')
            directory.mkdir()
            self.http('/api/zones', {'id': 'handoff/' + name, 'path': str(directory)})
            self.herdr('workspace', 'create', '--cwd', str(directory), '--label', name + '-zone', '--no-focus')
        self.http('/api/zones', {'id': 'handoff/manager', 'path': str(self.root / 'zone')})
        (self.root / 'work-zone/order.json').write_text('{"quantity":7,"unitPriceCents":1200,"discountPercent":null}\n', encoding='utf-8')
        business = ('为本地订单准备可交付报价，再交给用户补齐发票抬头。订单数量7、每件1200分；折扣尚未提供，'
            '不得推测。仅本地文件，不上传，不开具发票。缺信息时向管理者说明阻塞并等待用户补充。')
        manager = self.task('Quote coordination', 'manager', business + '按既有管理职责协调工作；用户尚未给出发票抬头时，'
            '后续开票会话交给用户继续，不代表开票业务完成。不要亲自计算或编辑报价。不得读取验收脚本、凭据或其他Chat。', self.host)
        worker = self.task('Quote preparation', 'normal', '仅执行本次报价业务，读取命名订单材料。不得读取验收脚本、凭据或其他Chat。', self.host)
        end_task = self.task('User invoice continuation', 'normal', '本会话供用户继续开票准备。开票抬头尚未提供，'
            '请先说明缺少抬头并等待用户；不得生成invoice-issued.txt，不得声称开票业务完成。不得读取验收脚本、凭据或其他Chat。', self.host)
        for identity, task in [('quote-worker', worker), ('invoice-continuation', end_task)]:
            (self.root / 'bots' / (identity + '.json')).write_text(json.dumps({'id': identity, 'name': identity,
                'when': '', 'plan_guide': '', 'node_guide': '', 'task_id': task}), encoding='utf-8')
        definition = {'title': 'Quote handoff to user', 'description': business,
            'input_files': [{'id': 'order', 'required': True,
                'description': '原始订单 JSON：数量和单价必需；discountPercent 可为 null，由报价节点识别缺项并与用户继续讨论。'}],
            'input_requirements': '开始工作的条件是订单数量、单价及本地范围明确。折扣允许尚未确定；识别并说明缺失信息属于报价节点的工作，不是管理者输入准备的前置条件。最终报价仍须等待用户补充真实折扣，不得推测。',
            'nodes': {
            'work': {'title': 'Prepare local quote', 'bot_id': 'quote-worker', 'input': business +
                '收到折扣后，在本工作目录生成quote.json。JSON只含quantity、unitPriceCents、discountPercent、totalCents；'
                '金额按分计算。交付的命名输出为quote。', 'zone': 'handoff/work', 'blocked_by': [],
                'input_rule': [{'id': 'order', 'type': 'run', 'input_id': 'order'}],
                'output_rule': [{'id': 'quote'}], 'auto_approved': self.mode == 'auto', 'auto_close': False},
            'end': {'title': 'Continue invoice with user', 'bot_id': 'invoice-continuation', 'input': '开票抬头尚未提供，等待用户补充；请勿开票。',
                'zone': 'handoff/end', 'blocked_by': ['work'], 'end_node': True, 'auto_approved': False, 'auto_close': False,
                'input_rule': [{'id': 'quote', 'type': 'node', 'node_id': 'work', 'output_id': 'quote'}], 'output_rule': []}}}
        (self.root / 'maps/handoff.json').write_text(json.dumps(definition, ensure_ascii=False), encoding='utf-8')
        self.evidence['definition'] = self.http('/api/scheduler/maps/handoff.json')
        require_order_input(self.evidence['definition'])
        started = self.http('/api/scheduler/maps/handoff.json/runs', {'managerTaskId': manager, 'zone': 'handoff/manager',
            'userInput': business, 'fileInputs': [{'id': 'order', 'path': str(self.root / 'work-zone/order.json')}]}, 201)
        self.run_id = started['id']

        def issue_saved():
            detail = self.detail(self.run_id)
            execution = detail['run']['nodes']['work'].get('execution')
            issues = [report for report in detail['reports'] if execution and report['executionId'] == execution['id'] and report.get('canComplete') is False]
            return (detail, execution, issues) if issues else None
        blocked, execution, issues = until('actual saved missing-discount report before approval', issue_saved, self.args.timeout)
        worker_chat = self.chat(execution['chat'])
        manager_chat = self.chat(blocked['run']['manager'])
        self.observe_processes()
        require(execution['approved'] is False and execution.get('approvalSource') is None, ('Progress must be allowed before approval', blocked))
        require(not self.records('/api/workflow/node/approve', worker_chat['sessionId']), 'Model approved an incomplete quote')
        require(all(report['body'].strip() for report in issues), issues)
        initial_history = self.turn(worker_chat)
        self.evidence['progressBeforeApproval'] = {'public': blocked, 'history': initial_history, 'at': time.monotonic()}
        self.send(worker_chat, '本订单折扣为25%，即按原价的75%计价。现在所需报价信息已齐全；发票抬头仍未提供，开票留给用户后续办理。', 'business-missing-discount-supplied')
        if self.mode == 'manual':
            self.approve_native_yes(worker_chat)

        def finished():
            detail = self.detail(self.run_id)
            return detail if detail['run']['status'] == 'completed' else None
        final = until('actual native manager explicitly finishes user handoff', finished, self.args.timeout)
        ordinary = final['run']['nodes']['work']
        end = final['run']['nodes']['end']
        end_chat = self.chat(end['execution']['chat'])
        self.observe_processes()
        histories = {name: self.turn(chat) for name, chat in [('worker', worker_chat), ('manager', manager_chat), ('end', end_chat)]}
        self.evidence['histories'] = histories
        material = self.quote()
        self.evidence['material'] = material
        session = worker_chat['sessionId']
        approvals = self.records('/api/workflow/node/approve', session)
        reports = self.records('/api/workflow/node/report', session)
        ready = [record for record in reports if (decode(record, 'request') or {}).get('canComplete') is True]
        require(len(approvals) == 1 and ready, ('Expected one actual approval and a normal result report', approvals, ready))
        approval = approvals[0]
        require(approval.get('status') == 200 and (decode(approval, 'response') or {}).get('approved') is True, approval)
        require(approval.get('ended', float('inf')) < ready[0]['started'], 'Normal report was sent before successful approval')
        require(all(record.get('status') == 200 for record in reports), ('Report failed', reports))
        approval_wall = self.clock['wall'] + approval['started'] - self.clock['monotonic']
        require(material['mtime'] <= approval_wall, 'Actual material was written only after approval')
        self.evidence['materialBeforeApproval'] = {'materialMtime': material['mtime'], 'approvalWallEstimate': approval_wall,
            'clockReference': self.clock, 'approvalRequest': approval['index']}
        worker_calls = calls(histories['worker'])
        approve_positions = [index for index, call in enumerate(worker_calls) if tool_name(call) == 'approve']
        ready_positions = [index for index, call in enumerate(worker_calls) if tool_name(call) == 'report' and call.get('arguments', {}).get('can_complete') is True]
        require(len(approve_positions) == 1 and ready_positions and approve_positions[0] < ready_positions[0], 'Actual native call order differs from approve then normal report')
        self.evidence['nativeCallOrder'] = worker_calls
        source = 'auto' if self.mode == 'auto' else 'manual'
        require(ordinary['execution']['approved'] is True and ordinary['execution']['approvalSource'] == source, ordinary)
        require(decode(approval, 'request')['source'] == source, approval)
        observations = [json.loads(path.read_text(encoding='utf-8')) for path in self.confirmations.glob('*.request.json')]
        worker_ui = [value for value in observations if value.get('sessionId') == session]
        require(len(worker_ui) == (1 if self.mode == 'manual' else 0), ('Unexpected report confirmation or fabricated auto Yes', worker_ui))
        self.evidence['nativeConfirmations'] = observations
        # Report readback is the public authority for named outputs, not filename inference.
        current_reports = [report for report in final['reports'] if report['executionId'] == execution['id']]
        require(any(output.get('id') == 'quote' and Path(output['path']).resolve() == Path(material['path']).resolve()
            for report in current_reports for output in report.get('outputs', [])), ('Real material was not handed off by declared ID', current_reports))
        require(ordinary['status'] == 'completed', ordinary)
        require(end['status'] == 'running' and end.get('completion') is None and end['execution']['endNode'] is True, end)
        require(end['execution']['approved'] is False and end['execution'].get('approvalSource') is None, end)
        require(not [report for report in final['reports'] if report['executionId'] == end['execution']['id']], 'End Chat acquired a report obligation')
        require(final['run']['finished']['reason'] == 'handed-to-user', final)
        require(not any(tool_name(call) in ['approve', 'report'] for call in calls(histories['end'])), 'End Chat used node duties')
        end_requests = []
        for record in self.model_wire.records:
            request = decode(record, 'request') or {}
            # This is the actual Task business instruction, unique to the end Chat;
            # manager snapshots do not contain the Task's fixed prompt.
            if '本会话供用户继续开票准备。开票抬头尚未提供' not in json.dumps(request.get('messages', []), ensure_ascii=False):
                continue
            offered = [tool.get('function', {}).get('name', '').rsplit('.', 1)[-1] for tool in request.get('tools', [])]
            require(not any(name in ['approve', 'report'] for name in offered), ('End model was offered node approval/report duties', record['index'], offered))
            end_requests.append({'request': record['index'], 'offeredTools': offered})
        require(end_requests, 'Missing actual end-Chat model request; native tool absence remains unverified')
        self.evidence['endModelRequests'] = end_requests
        manager_calls = calls(histories['manager'])
        require(any(tool_name(call) == 'run' and call.get('arguments', {}).get('action') == 'finish' for call in manager_calls), 'No real native explicit finish')
        finishes = self.records('/api/workflow/manager/finish', manager_chat['sessionId'])
        require(len(finishes) == 1 and finishes[0].get('status') == 200, ('No successful actual finish or unrequested decision retries', finishes))
        self.http('/api/workflow/node/context', {'sessionId': end_chat['sessionId']}, 403)
        self.evidence['final'] = final
        start = time.monotonic()
        samples = []
        baseline = len(self.backend_wire.records)
        while time.monotonic() - start < 51:
            observed = self.detail(self.run_id)
            require(observed['run'] == final['run'] and observed['reports'] == final['reports'], 'Handoff spontaneously advanced or required end approval/report')
            require(not (self.root / 'end-zone/invoice-issued.txt').exists(), 'Missing user invoice data was bypassed')
            samples.append(time.monotonic())
            time.sleep(0.25)
        extra = self.backend_wire.records[baseline:]
        require(not any(record['path'] in ['/api/workflow/node/approve', '/api/workflow/node/report', '/api/workflow/manager/dispatch', '/api/workflow/manager/finish'] for record in extra), 'Automatic business or repeated workflow action after handoff')
        self.evidence['noAutomaticActionWindow'] = {'started': start, 'ended': time.monotonic(), 'samples': samples,
            'budgets': self.evidence['budgets'], 'wireStart': baseline}
        self.check_model_identity()
        self.evidence['coverage'] = {key: {'verified': True, 'evidenceKeys': {
            'P20/order': ['material', 'materialBeforeApproval', 'nativeCallOrder', 'histories', 'modelIdentities', 'final'],
            'P20/progress-before-approval': ['progressBeforeApproval'],
            'P20/approval-source': ['nativeYes' if self.mode == 'manual' else 'nativeConfirmations', 'final'],
            'P30/end-node-finish': ['histories', 'final', 'noAutomaticActionWindow'],
        }[key]} for key in ITEMS}
        self.evidence['passed'] = True

    def cleanup_extra(self):
        try:
            super().cleanup_extra()
        finally:
            if self.model_wire and self.model_wire.records:
                self.check_model_identity()
