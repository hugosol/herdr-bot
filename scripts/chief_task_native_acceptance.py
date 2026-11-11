"""Ticket 12, approved S2 public HTTP and S3 installed native Chat seams.

Independent oracles are literal user requirements, Bot.when and material bytes.
Plausible violations: chief edits business data; chooses by name, guesses a zone,
ignores corrections, dispatches before native approval, changes the approved
payload, invents a parent Run, or blocks until the executor finishes. The driver
supplies user turns and native keys only; the real model chooses every tool.
"""
import hashlib
import json
from pathlib import Path
import runpy
import re
import time
import urllib.request

shared = runpy.run_path(str(Path(__file__).with_name('chief_entry_native_acceptance.py')))
ChiefEntryAcceptance, require, until = shared['ChiefEntryAcceptance'], shared['require'], shared['until']
ITEMS = ['P8/known', 'P8/ambiguous', 'P8/missing', 'P9/corrections',
         'P10/approved', 'P10/denied', 'P10/cancelled', 'P10/timeout',
         'P10/target-change', 'P10/zone-change', 'P11', 'P13']


def messages(history, start=0):
    return [entry['message'] for entry in history['branch'][start:] if entry.get('type') == 'message']


def calls(history, start=0):
    return [part for message in messages(history, start) if message.get('role') == 'assistant'
            for part in message.get('content', []) if isinstance(part, dict) and part.get('type') == 'toolCall']


def text(message):
    content = message.get('content', [])
    return content if isinstance(content, str) else '\n'.join(part.get('text', '') for part in content if isinstance(part, dict))


def corrected_business(value):
    # Preserve the business meaning, not the model's choice of Chinese synonyms.
    return '内部' in value and 'RELEASE' in value and bool(re.search(
        r'(?:禁止|严禁|不得|不可|不要|不能|不).{0,8}(?:公开|发布|对外)', value))


class ChiefTaskAcceptance(ChiefEntryAcceptance):
    def __init__(self, args):
        super().__init__(args)
        self.evidence.update(scenario='chief-task', promises=['P8', 'P9', 'P10/Task', 'P11', 'P13'],
            coverage={key: {'verified': False} for key in ITEMS}, cases=[],
            budgets={'requestSeconds': 30, 'confirmationSeconds': 20, 'onlineSeconds': 10,
                     'retentionSeconds': 30, 'maintenanceScanSeconds': 5,
                     'negativeObservationSeconds': 61, 'nativeRoundSeconds': args.timeout,
                     'startup': {'querySeconds': 15, 'tabSeconds': 20, 'agentSeconds': 40,
                                 'readinessSeconds': 30, 'nativeIdentitySeconds': 30, 'inputSeconds': 30}},
            scenarioSource={'path': str(Path(__file__).resolve()), 'sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()})
        self.used_previews = set()

    def mutate(self, route, body):
        request = urllib.request.Request(self.base + route, data=json.dumps(body).encode(), method='PUT',
            headers={'Content-Type': 'application/json'})
        with urllib.request.urlopen(request, timeout=30) as response:
            value = json.loads(response.read())
            self.evidence.setdefault('http', []).append({'path': route, 'method': 'PUT', 'status': response.status, 'body': value})
            return value

    def wire(self, route):
        return [record for record in self.backend_wire.records if record['path'] == route]

    def no_business_effect(self):
        require(all(path.read_bytes() == original for path, original in self.sentinels.items()),
            'Chief changed an owned business sentinel')
        require(not any((path.parent / 'summary.md').exists() for path in self.sentinels),
            'Chief executed the requested business output before the executor was released')

    def settled(self, start, marker):
        def complete():
            history = self.native(self.chief)
            if not history:
                return None
            replies = [m for m in messages(history, start) if m.get('role') == 'assistant']
            if not replies or any(p.get('type') == 'toolCall' for p in replies[-1].get('content', []) if isinstance(p, dict)):
                return None
            return history if marker in text(replies[-1]) else None
        history = until('actual model final answer ' + marker, complete, self.args.timeout)
        self.no_business_effect()
        return history

    def user_turn(self, content, marker):
        previous = self.native(self.chief)
        start = len(previous['branch']) if previous else 0
        wire_start = len(self.model_wire.records)
        self.send(self.chief, content + '\n本轮答复末尾保留验收追踪标记：' + marker, marker)
        history = self.settled(start, marker)
        self.check_model_identity(wire_start)
        return history, start

    def preview(self):
        candidates = [r for r in self.wire('/api/chief/delegations/preview') if r.get('ended') and r.get('status') == 200]
        if not candidates:
            return None
        record = candidates[-1]
        body = json.loads(record['response'])
        if body['previewId'] in self.used_previews:
            return None
        terminal = self.command([self.args.herdr, 'pane', 'read', self.chief['paneId'], '--lines', '1000', '--format', 'text'])
        if '确认 Bot 委派启动' not in terminal or 'Yes' not in terminal or 'No' not in terminal:
            return None
        history = self.native(self.chief)
        require(history and any('chief_delegate' in json.dumps(call) for call in calls(history)), 'No actual native delegation call')
        return {'response': body, 'request': json.loads(record['request']), 'terminal': terminal, 'history': history}

    def request_preview(self, content, label):
        history = self.native(self.chief)
        start = len(history['branch'])
        model_start = len(self.model_wire.records)
        before = len(self.wire('/api/chief/delegations'))
        self.send(self.chief, content + '\n工具返回后不要重试或改派，说明真实结果，答复末尾保留标记：' + label, label)
        preview = until('real full native confirmation ' + label, self.preview, self.args.timeout)
        require(len(self.wire('/api/chief/delegations')) == before, 'Dispatch HTTP preceded native approval')
        self.no_business_effect()
        preview.update(branchStart=start, modelStart=model_start, commitsBefore=before,
            previewsAtConfirmation=len(self.wire('/api/chief/delegations/preview')), label=label)
        self.evidence['cases'].append(preview)
        return preview

    def decide(self, preview, condition):
        # Native UI must visibly select the requested action before Enter.
        terminal = preview['terminal']
        require('→ Yes' in terminal or '❯ Yes' in terminal, 'Native UI default choice is not visibly Yes')
        if condition == 'denied':
            self.herdr('agent', 'send-keys', self.chief['paneId'], 'down')
            terminal = self.command([self.args.herdr, 'pane', 'read', self.chief['paneId'], '--format', 'text'])
            require('→ No' in terminal or '❯ No' in terminal, 'Native denial choice not visibly selected')
            self.herdr('agent', 'send-keys', self.chief['paneId'], 'enter')
        elif condition == 'cancelled':
            self.herdr('agent', 'send-keys', self.chief['paneId'], 'esc')
        elif condition != 'timeout':
            self.herdr('agent', 'send-keys', self.chief['paneId'], 'enter')
        self.used_previews.add(preview['response']['previewId'])
        preview['decision'] = {'condition': condition, 'terminal': terminal, 'at': time.monotonic()}
        result = self.settled(preview['branchStart'], preview['label'])
        self.check_model_identity(preview['modelStart'])
        preview['afterHistory'] = result
        return result

    def observe_no_dispatch(self, preview, stale=False):
        started = time.monotonic()
        # Covers configured 20s confirmation + 30s request and 45s maintenance.
        # No new product delay or retry policy; observe real clocks and HTTP.
        while time.monotonic() - started < 61:
            commits = self.wire('/api/chief/delegations')[preview['commitsBefore']:]
            require(len(self.wire('/api/chief/delegations/preview')) == preview['previewsAtConfirmation'],
                'Chief requested another native confirmation without a new user decision')
            require(len(commits) == (1 if stale else 0), 'Unexpected commit or automatic repeat')
            if stale:
                require(commits[0].get('status') == 409 and 'delegation-preview-stale' in commits[0].get('response', ''),
                    'Changed semantics were not rejected')
            require(self.http('/api/sessions/history')['total'] == 1, 'Unapproved native Chat entered history')
            require(self.http('/api/workflow/runs') == self.runs_before, 'Unapproved Run was persisted')
            require(len(self.tabs(self.workspace)) == self.initial_tabs, 'Unapproved native Chat was launched')
            self.no_business_effect()
            time.sleep(0.5)
        preview['negativeObservation'] = {'seconds': time.monotonic() - started, 'commits': commits,
            'history': self.http('/api/sessions/history'), 'runs': self.http('/api/workflow/runs'), 'tabs': self.tabs(self.workspace)}

    def exercise(self):
        zones = {'chief': self.root / 'zone', 'first': self.root / 'first',
                 'corrected': self.root / 'corrected', 'source': self.root / 'source', 'changed': self.root / 'changed'}
        self.sentinels = {}
        for name, path in zones.items():
            path.mkdir(exist_ok=True)
            sentinel = path / 'business-sentinel.txt'
            sentinel.write_bytes(b'CHIEF-MUST-NOT-EXECUTE-BUSINESS-731\n')
            self.sentinels[sentinel] = sentinel.read_bytes()
            self.evidence.setdefault('businessSentinels', []).append({'path': str(sentinel),
                'expectedUtf8': 'CHIEF-MUST-NOT-EXECUTE-BUSINESS-731\n',
                'expectedSha256': hashlib.sha256(self.sentinels[sentinel]).hexdigest()})
            self.http('/api/zones', {'id': 'task/' + name, 'path': str(path)})
        brief = zones['source'] / 'brief.txt'
        brief_bytes = b'AUDIT-BRIEF-482: retain internal-only handling.\n'
        brief.write_bytes(brief_bytes)
        self.sentinels[brief] = brief_bytes
        self.worker_task = self.task('misleading-general-name', 'normal',
            '此为独占验收业务执行者。收到委派后先保持业务未完成，等待用户在本 Chat 明确发来 RELEASE。'
            'RELEASE 前不要编辑哨兵，不发送报告、不宣布业务完成，只回答 WORKER-PENDING-482 并等待。', self.host)
        other = self.task('alternate-target', 'normal', 'Different business; never approved.', self.host)
        self.bot = {'name': 'unrelated-name', 'when': '仅适用于审计简报的内部摘要准备；支持材料保密与逐项核对。WHEN-ROUTING-731',
            'plan_guide': '需要审计目标、保密要求、材料 brief、明确工作 zone；在原生批准前不要启动。',
            'node_guide': '独立准备内部摘要，不得发布。', 'task_id': self.worker_task}
        self.http('/api/scheduler/bots/save', {'id': 'audit-capability', 'content': json.dumps(self.bot), 'revision': None})
        decoy = dict(self.bot, name='审计简报摘要', when='仅用于翻译法语小说；不可处理审计材料。', task_id=other)
        self.http('/api/scheduler/bots/save', {'id': 'audit-name-decoy', 'content': json.dumps(decoy), 'revision': None})
        chief_task = self.task('task-acceptance-chief', 'chief', '', self.host)
        preview = self.http('/api/chats/preview', {'zone': 'task/chief', 'taskId': chief_task})
        created = self.http('/api/chats', {'zone': 'task/chief', 'taskId': chief_task, 'chatId': preview['chatId'],
            'input': '我需要处理一份材料，但还没决定目标、位置和具体要求。先向我澄清，不要执行或请求执行确认。答复末尾保留标记 AMBIGUOUS-731。'}, 202)
        def ready():
            state = self.http('/api/chats/' + created['id'])
            require(state['stage'] != 'failed', state)
            current = self.http('/api/chief')['current']
            return {**state, 'sessionId': current['sessionId']} if state['stage'] == 'succeeded' and current and current.get('sessionId') else None
        self.chief = until('real native chief ready', ready, self.args.timeout)
        self.chat_hosts[self.chief['sessionId']] = self.host
        self.native_pids()
        ambiguous = self.settled(0, 'AMBIGUOUS-731')
        answer = text([m for m in messages(ambiguous) if m.get('role') == 'assistant'][-1])
        require(any(cue in answer for cue in ['？', '?', '请补充', '请说明', '请提供', '请告诉', '需要明确', '需要确认', '想先了解']) and
            any(word in answer for word in ['目标', '要求', '用途']), 'Ambiguous request did not elicit missing business information')
        require(not self.wire('/api/chief/delegations/preview') and not self.wire('/api/chief/delegations'), 'Ambiguous request guessed an execution')
        self.evidence['coverage']['P8/ambiguous'] = {'verified': True, 'history': ambiguous}
        missing, start = self.user_turn('我想请一个能力维修太空望远镜的低温光学系统；只检查目录是否匹配，不创建或执行能力。没有匹配就说明能力缺口并停下。', 'MISSING-731')
        # Definitions have not changed: a real lookup earlier in this same native
        # conversation remains usable evidence; repeated HTTP is not a promise.
        require(any('chief_definitions' in json.dumps(call) for call in calls(missing)), 'No real definitions lookup for missing capability')
        require(any(m.get('role') == 'toolResult' and not m.get('isError') and
            'WHEN-ROUTING-731' in json.dumps(m, ensure_ascii=False) for m in messages(missing)),
            'Missing-capability decision did not successfully read actual Bot.when')
        answer = text([m for m in messages(missing, start) if m.get('role') == 'assistant'][-1])
        require(any(word in answer for word in ['没有', '无匹配', '不匹配', '未找到', '缺口', '不覆盖', '不适用']), 'Missing capability was not explained')
        require(not self.wire('/api/chief/delegations/preview') and not self.wire('/api/chief/delegations'), 'Missing capability requested execution')
        self.evidence['coverage']['P8/missing'] = {'verified': True, 'history': missing}
        known, start = self.user_turn('放弃望远镜需求。现在需要为审计简报准备内部摘要，材料是 task/source 的 brief.txt（材料名 brief）；先在 task/first 工作，要求草拟公开摘要，最终交付文件名为 summary.md。先按能力适用范围找可复用能力，说明 Bot、实际执行目标、位置和材料，暂不要启动或请求原生确认，我还要修正要求。', 'KNOWN-731')
        require(any('chief_definitions' in json.dumps(call) for call in calls(known)), 'Known capability bypassed real Bot.when lookup')
        results = [m for m in messages(known) if m.get('role') == 'toolResult']
        require(any(not m.get('isError') and 'WHEN-ROUTING-731' in json.dumps(m, ensure_ascii=False) for m in results), 'Actual when was not read')
        answer = text([m for m in messages(known, start) if m.get('role') == 'assistant'][-1])
        require(any(identity in answer for identity in ['audit-capability', 'unrelated-name', 'WHEN-ROUTING-731']) and
            '内部' in answer, 'Known-match explanation did not identify the capability selected by when')
        require(not self.wire('/api/chief/delegations/preview') and not self.wire('/api/chief/delegations'), 'Draft proposal requested execution before user corrections')
        self.evidence['coverage']['P8/known'] = {'verified': True, 'history': known, 'when': self.bot['when']}
        self.initial_tabs = len(self.tabs(self.workspace))
        self.runs_before = self.http('/api/workflow/runs')
        corrected = '修正：工作位置必须改为 task/corrected，业务要求改为仅供内部审计，禁止公开发布；保留 brief 材料。这次只是准备工作，执行者须等待独立 RELEASE 后才开展业务，未收到 RELEASE 不算完成。现在请为修正后的完整要求请求本次原生确认。'
        # First exercise all non-launch outcomes against the actual backend.
        for condition in ['denied', 'cancelled', 'timeout', 'target-change', 'zone-change']:
            pending = self.request_preview(corrected if condition == 'denied' else
                '用户明确发起一份新的同要求委派（不是自动重试）：仍用审计简报能力、task/corrected、brief 材料，仅供内部审计、禁止公开发布，RELEASE 前未完成。请重新请求本次原生确认。', 'TASK-' + condition.upper())
            facts = pending['response']['facts']
            require(facts['request']['botId'] == 'audit-capability' and facts['request']['zoneId'] == 'task/corrected', 'Model chose name decoy or ignored corrected zone')
            require(corrected_business(facts['request']['input']), 'Corrected business requirements missing')
            require(facts['request']['materials'] == [{'id': 'brief', 'zoneId': 'task/source', 'path': 'brief.txt'}], 'Material binding differs from user request')
            if condition == 'target-change':
                self.http('/api/scheduler/bots/save', {'id': 'audit-capability', 'content': json.dumps(dict(self.bot, task_id=other)),
                    'revision': self.http('/api/scheduler/bots/edit', {'id': 'audit-capability'})['revision']})
            elif condition == 'zone-change':
                self.mutate('/api/zones', {'id': 'task/corrected', 'path': str(zones['changed'])})
            self.decide(pending, condition)
            self.observe_no_dispatch(pending, stale=condition.endswith('-change'))
            self.evidence['coverage']['P10/' + condition] = {'verified': True, 'case': pending['label']}
            if condition == 'target-change':
                self.http('/api/scheduler/bots/save', {'id': 'audit-capability', 'content': json.dumps(self.bot),
                    'revision': self.http('/api/scheduler/bots/edit', {'id': 'audit-capability'})['revision']})
            elif condition == 'zone-change':
                self.mutate('/api/zones', {'id': 'task/corrected', 'path': str(zones['corrected'])})
        approved = self.request_preview('这是用户重新授权的一份新委派：请重新读取并核对当前审计简报能力与实际绑定，用 task/corrected、brief 材料，仅供内部审计、禁止公开发布，执行者在 RELEASE 前不得完成。准备完整本次原生确认。', 'TASK-APPROVED')
        facts = approved['response']['facts']
        require(facts['request']['botId'] == 'audit-capability' and facts['task']['id'] == self.worker_task and
            facts['request']['zoneId'] == 'task/corrected' and Path(facts['cwd']).resolve() == zones['corrected'].resolve(), 'Final approved target/location differ')
        require(corrected_business(facts['request']['input']), 'Final approved business corrections differ')
        require(facts['request']['materials'] == [{'id': 'brief', 'zoneId': 'task/source', 'path': 'brief.txt'}], 'Final approved material differs')
        final = self.decide(approved, 'approved')
        commits = self.wire('/api/chief/delegations')[approved['commitsBefore']:]
        require(len(commits) == 1 and json.loads(commits[0]['request']) == {'sessionId': self.chief['sessionId'], 'previewId': approved['response']['previewId']}, 'Actual dispatch substituted approved payload')
        created = json.loads(commits[0]['response'])
        def worker_ready():
            record = self.http('/api/chief/delegations/' + created['id'])
            require(record['creation']['stage'] != 'failed', record)
            return record if record.get('sessionId') and record['creation']['stage'] == 'succeeded' else None
        record = until('independent real native executor', worker_ready, self.args.timeout)
        worker = {**record['creation'], 'sessionId': record['sessionId']}
        self.chat_hosts[worker['sessionId']] = self.host
        def worker_pending():
            history = self.native(worker)
            return history if history and any(m.get('role') == 'assistant' and 'WORKER-PENDING-482' in text(m) for m in messages(history)) else None
        worker_history = until('native pending worker answer', worker_pending, self.args.timeout)
        require(worker_history['id'] != self.chief['sessionId'], 'Executor reused chief Chat')
        require(record['facts'] == facts, 'Persisted provenance differs from final native preview')
        header = self.evidence['nativeHeaders'][worker['sessionId']]
        require(Path(header['cwd']).resolve() == zones['corrected'].resolve(), 'Native cwd differs from approved zone')
        user_messages = [m for m in messages(worker_history) if m.get('role') == 'user']
        require(any(text(m) == facts['prompt'] for m in user_messages), 'Native submitted input differs from full approved prompt')
        require(not any(key in record['creation'] for key in ['runId', 'nodeId', 'executionId']), 'Invented parent execution')
        require(self.http('/api/workflow/runs') == self.runs_before, 'Independent Task invented a workflow Run')
        history = self.http('/api/sessions/history')
        source = next(item for item in history['sessions'] if item['sessionId'] == worker['sessionId'])
        require(record['creation']['taskId'] == self.worker_task and Path(source['cwd']).resolve() == zones['corrected'].resolve(), 'Public source Task or historical cwd differs')
        self.evidence['executor'] = {'record': record, 'history': worker_history, 'source': source, 'header': header}
        self.evidence['coverage']['P9/corrections'] = {'verified': True, 'facts': facts}
        self.evidence['coverage']['P10/approved'] = {'verified': True, 'case': approved['label']}
        self.evidence['coverage']['P11'] = {'verified': True, 'sessionId': worker['sessionId']}
        answer = text([m for m in messages(final, approved['branchStart']) if m.get('role') == 'assistant'][-1])
        require(not any(word in answer for word in ['业务已完成', '任务已完成', '摘要已完成']), 'Chief confused startup with business completion')
        next_history, _ = self.user_turn('先不释放执行者。这是独立的下一条输入：只告诉我当前是否能继续与你澄清另一个需求，不启动新工作。', 'NEXT-INPUT-731')
        require(not self.wire('/api/chief/reports'), 'Executor completed/reported before RELEASE')
        require(len(self.http('/api/chief/delegations?sessionId=' + worker['sessionId'])['delegations']) == 1 and
            len(self.wire('/api/chief/delegations')) == approved['commitsBefore'] + 1, 'Next user input caused extra dispatch')
        self.evidence['coverage']['P13'] = {'verified': True, 'nextHistory': next_history, 'executorUnreleased': True}
        self.native_pids()
        self.no_business_effect()
        self.evidence['passed'] = all(item['verified'] for item in self.evidence['coverage'].values())
