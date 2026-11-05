"""P27/P28 at S2/S3: shipped planning Tasks/Map in isolated native Chats.

This mixin uses the roles runner's actual host/model, source attribution, native
storage and cleanup. Only user messages and legal pre-run business capabilities
are controlled. No provider decision, workflow transition or runtime record is
supplied by this scenario. All oracles below precede the model run.
"""
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import time
import urllib.parse

from developer_paths import REPO


class PlanWorkflowScenario:
    def __init__(self, args):
        super().__init__(args)
        self.evidence.update(scenario='plan-workflow', promises=['P27', 'P28'],
            interventions=[], runs=[], coverage={},
            oracle='Independent business-requirements.json, shipped Task nine-section contract, native user inputs, actual tool/HTTP observations')
        self.native_paths = {}
        self.native_cache = {}
        self.run_ids = []
        self.confirmations = {}

    def check(self, condition, detail):
        if not condition:
            raise AssertionError(detail)

    def configure_model(self):
        super().configure_model()
        # Copy only the explicit read-only dependency skills, never user state.
        source = self.args.pi_state_extension.parent.parent / 'skills'
        for name in ['grill-with-docs', 'grilling', 'domain-modeling']:
            self.check((source / name / 'SKILL.md').is_file(), ('Missing actual planning skill dependency', str(source / name)))
            shutil.copytree(source / name, self.root / 'agent/skills' / name)
        self.evidence['planningSkills'] = {name: {
            'source': str(source / name),
            'sha256': hashlib.sha256((source / name / 'SKILL.md').read_bytes()).hexdigest()
        } for name in ['grill-with-docs', 'grilling', 'domain-modeling']}

    def write_business_program(self):
        # RolesAcceptance.prepare invokes this after provisioning the real host.
        # Deploy the shipped definitions; record every environment relocation.
        options = self.root / 'options.toml'
        suite_entry = self.root / 'toolkit/entrypoints/pi-auto/pi-suite.ts'
        self.check(suite_entry.is_file(), ('Missing actual Pi Suite factory', str(suite_entry)))
        options.write_text(options.read_text(encoding='utf-8').replace('skills = []\n', '') +
            '\n[[skills]]\nid = "grill-with-docs"\nlabel = "/grill-with-docs"\nnative_name = "grill-with-docs"\n' +
            '\n[[extensions]]\nid = "pi-suite"\nlabel = "Pi Suite"\nagent = "pi"\nkind = "fixed"\nentrypoint = ' + json.dumps(str(suite_entry)) + '\n', encoding='utf-8')
        # The isolated backend owns this Suite; no machine/user deployment is read.
        planning_suite = {'id': 'acceptance-planning', 'name': 'Isolated planning',
            'agents': ['pi'], 'skills': {'pi': [str(self.root / 'agent/skills' / name)
                for name in ['grill-with-docs', 'grilling', 'domain-modeling']]},
            'extensions': {}, 'prompts': [], 'lsp': {}, 'mcp': [], 'pi': {'codemode': 'on'}}
        self.http('/api/suites', planning_suite)
        self.evidence['planningSuite'] = planning_suite
        # The backend resolves its default prompts.local.toml beside backend.toml.
        # Keep shipped definition placeholders raw and bind only this deployment's
        # values; never read the developer's dictionary or interpolate user input.
        dictionary = self.root / 'prompts.local.toml'
        dictionary_values = {
            'herdr-bot-path': REPO.as_posix(),
            'herdr-bot-api-url': self.backend_wire.base,
        }
        dictionary.write_text(''.join(f'{key} = {json.dumps(value, ensure_ascii=False)}\n'
            for key, value in dictionary_values.items()), encoding='utf-8')
        self.evidence['promptDictionary'] = {'path': str(dictionary),
            'values': dictionary_values, 'sha256': self.digest(dictionary)}
        self.ids = {}
        sources = self.root / 'source-evidence'
        sources.mkdir()
        for path in [Path(__file__), REPO / 'scripts/test-workflow-file-handoff.py',
                REPO / 'meta-data/tasks.json', REPO / 'meta-data/workflow-maps/plan_workflow.json']:
            copied = sources / path.name
            shutil.copy2(path, copied)
            self.evidence.setdefault('exactSourceBytes', []).append({
                'source': str(path), 'raw': str(copied), 'sha256': self.digest(copied)})
        tasks = json.loads((REPO / 'meta-data/tasks.json').read_text(encoding='utf-8'))['tasks']
        self.deployment = {'tasks': [], 'bots': [], 'map': {}, 'relocations': {
            'repository': str(REPO), 'service': self.backend_wire.base, 'zone': str(self.root / 'zone'),
            'host': 'Pi', 'model': self.provider_model,
            'note': 'Task business prompts and initial skill preserved; host/model, selected Suite resources, fixed Pi Suite factory and physical locations relocated to explicit isolated deployment.'}}
        for name in ['manager', '概要规划', '概要方案转 Map']:
            original = next(task for task in tasks if task['name'] == name)
            deployed = {key: value for key, value in original.items() if key != 'id'}
            deployed.update(agent='pi', model='probe', thinking='default', suite=planning_suite['id'] if original.get('suite') else None)
            result = self.http('/api/tasks', deployed)
            saved = next(task for task in result['tasks'] if task['name'] == name)
            self.ids[name] = saved['id']
            self.deployment['tasks'].append({'original': original, 'deployed': saved})
        for filename in ['bot-1793304246857557100-58200-0.json', 'bot-1793305512701853300-58200-1.json']:
            original = json.loads((REPO / 'meta-data/bots' / filename).read_text(encoding='utf-8'))
            deployed = json.loads(json.dumps(original, ensure_ascii=False))
            deployed['task_id'] = self.ids[deployed['name']]
            path = self.root / 'bots' / filename
            path.write_text(json.dumps(deployed, ensure_ascii=False, indent=2), encoding='utf-8')
            self.deployment['bots'].append({'source': str(REPO / 'meta-data/bots' / filename), 'original': original, 'deployed': deployed})
        original = json.loads((REPO / 'meta-data/workflow-maps/plan_workflow.json').read_text(encoding='utf-8'))
        deployed = json.loads(json.dumps(original))
        for node in deployed['nodes'].values():
            node['zone'] = 'acceptance/planning'
        path = self.root / 'maps/plan_workflow.json'
        path.write_text(json.dumps(deployed, ensure_ascii=False, indent=2), encoding='utf-8')
        self.deployment['map'] = {'original': original, 'deployed': deployed, 'sha256': self.digest(path)}
        (self.root / 'deployment.json').write_text(json.dumps(self.deployment, ensure_ascii=False, indent=2), encoding='utf-8')
        self.http('/api/zones', {'id': 'acceptance/planning', 'path': str(self.root / 'zone')})
        self.provision_business()
        # Immutable baselines exclude only the new business Map and native data.
        self.baseline = self.configuration_bytes()

    @staticmethod
    def digest(path):
        return hashlib.sha256(path.read_bytes()).hexdigest()

    def configuration_bytes(self):
        return {str(path): self.digest(path) for path in [self.root / 'prompts.local.toml', self.root / 'data/tasks.json', self.root / 'meta-data/suites.json', self.root / 'maps/plan_workflow.json',
            *sorted((self.root / 'bots').glob('*.json'))]}

    def provision_business(self):
        zone = self.root / 'zone'
        (zone / 'orders.csv').write_text('category,total_cents\nbooks,1200\nbooks,300\nfood,800\n', encoding='utf-8')
        program = zone / 'business.py'
        program.write_text('''import csv, json, pathlib, sys
root = pathlib.Path(__file__).resolve().parent
if sys.argv[1] == 'aggregate':
    totals = {}
    with (root / 'orders.csv').open(newline='', encoding='utf-8') as stream:
        for row in csv.DictReader(stream):
            totals[row['category']] = totals.get(row['category'], 0) + int(row['total_cents'])
    with (root / 'totals.csv').open('w', newline='', encoding='utf-8') as stream:
        writer = csv.writer(stream)
        writer.writerow(['category', 'total_cents', 'currency'])
        writer.writerows([key, totals[key], 'CNY'] for key in sorted(totals))
elif sys.argv[1] == 'review':
    with (root / 'totals.csv').open(newline='', encoding='utf-8') as stream:
        rows = list(csv.DictReader(stream))
    assert rows == [{'category': 'books', 'total_cents': '1500', 'currency': 'CNY'}, {'category': 'food', 'total_cents': '800', 'currency': 'CNY'}], rows
    (root / 'review.json').write_text(json.dumps({'accepted': True, 'rows': 2}), encoding='utf-8')
else:
    raise ValueError('Only aggregate and review are supported; no external publication capability exists')
''', encoding='utf-8')
        self.evidence['businessFixture'] = {'scope': 'Real local program and legal Tasks/Bots provisioned before either Run; never executed by planning',
            'program': str(program), 'programSha256': self.digest(program),
            'input': str(zone / 'orders.csv'), 'inputSha256': self.digest(zone / 'orders.csv')}
        command = subprocess.list2cmdline([Path(shutil.which('python')).resolve().as_posix(), program.as_posix()])
        for bot_id, mode, description in [
            ('acceptance-aggregate', 'aggregate', '按分类汇总 orders.csv 的 total_cents，输出 totals.csv，列为 category,total_cents,currency；currency 固定 CNY。'),
            ('acceptance-review', 'review', '独立核对 totals.csv 的分类、金额及 CNY 币种，输出 review.json。')]:
            name = 'P27-' + mode
            task = {'name': name, 'taskType': 'normal', 'agent': 'pi', 'chatName': '{chat.id}',
                'model': 'probe', 'thinking': 'default', 'skill': None,
                'prompt': f'仅在用户后续单独授权的业务 Run 执行：{description} 使用 {command} {mode}。只读/写本 zone 的指定业务文件，不联网、不发布、不启动其他 Chat。'}
            result = self.http('/api/tasks', task)
            task_id = next(item['id'] for item in result['tasks'] if item['name'] == name)
            bot = {'id': bot_id, 'name': name, 'task_id': task_id,
                'when': description + ' 不用于对外发布或税务系统。',
                'plan_guide': description + f' 输入输出均在已登记 zone {zone}；依赖材料必须写明，配置存在不代表本次业务已执行。',
                'node_guide': f'显式提供 zone {zone}、读取文件和输出文件、职责、权限及完成标准。{description} 仅允许执行 {command} {mode}，不许外部调用。'}
            (self.root / 'bots' / (bot_id + '.json')).write_text(json.dumps(bot, ensure_ascii=False, indent=2), encoding='utf-8')
            self.ids[bot_id] = task_id
        self.requirements = {
            'planId': 'p27-business-plan', 'mapFile': 'p27-business-plan.json', 'zone': 'acceptance/planning',
            'W1': {'node': 'w1', 'bot': 'acceptance-aggregate', 'task': self.ids['acceptance-aggregate'],
                'input': str(zone / 'orders.csv'), 'output': str(zone / 'totals.csv'), 'blocked_by': []},
            'W2': {'node': 'w2', 'bot': 'acceptance-review', 'task': self.ids['acceptance-review'],
                'input': str(zone / 'totals.csv'), 'output': str(zone / 'review.json'), 'blocked_by': ['w1']},
            'revisionRequest': 'totals.csv 必须含 category,total_cents,currency 三列，currency 固定 CNY；W2 必须核对币种。不增加第三项工作。',
            'completionCriteria': {'booksTotalCents': 1500, 'foodTotalCents': 800, 'currency': 'CNY', 'reviewAccepted': True},
            'permissions': '只交付规划及静态 Map；不运行业务、不创建 Task/Bot、不自动启动新 Map；未提供新业务管理者配置，不做 preflight。',
            'missingCapability': '发布到真实税务生产系统；没有 API、代码、凭据或用户授权，必须保留 G1 真实阻塞，不创建或调度能力。',
            'nineSections': ['方案身份与确认', '目标与权限', '工作划分', '能力选用', '依赖与材料交接', '执行条件', '证据与未决项', '下游要求', '可选能力建设索引']}
        (self.root / 'business-requirements.json').write_text(json.dumps(self.requirements, ensure_ascii=False, indent=2), encoding='utf-8')
        self.evidence['independentRequirements'] = str(self.root / 'business-requirements.json')

    def native(self, chat):
        session = chat['sessionId']
        if session not in self.native_paths:
            module = (self.args.pi_package / 'dist/core/session-manager.js').as_uri()
            code = f'import {{SessionManager}} from {json.dumps(module)}; console.log(JSON.stringify(await SessionManager.list({json.dumps(str(self.root / "zone"))}, {json.dumps(str(self.root / "agent/sessions"))})));'
            files = json.loads(self.command([self.args.node, '--input-type=module', '-e', code]))
            found = next((item for item in files if item['id'] == session), None)
            if not found:
                return None
            path = Path(found['path'])
            self.check(path.is_relative_to(self.root), ('Native history escaped isolated storage', str(path)))
            self.native_paths[session] = path
        path = self.native_paths[session]
        fingerprint = self.digest(path)
        cached = self.native_cache.get(session)
        if cached and cached[0] == fingerprint:
            return cached[1]
        history = self.history(str(path))
        self.native_cache[session] = (fingerprint, history)
        return history

    @staticmethod
    def native_messages(history):
        return [entry['message'] for entry in history['branch'] if entry.get('type') == 'message']

    @classmethod
    def calls(cls, history):
        return [part for message in cls.native_messages(history) if message.get('role') == 'assistant'
            for part in message.get('content', []) if isinstance(part, dict) and part.get('type') == 'toolCall']

    def await_chat(self, chat, label, after=None):
        def probe():
            history = self.native(chat)
            if not history:
                return None
            messages = self.native_messages(history)
            if after and not any(message.get('role') == 'user' and after in self.message_text(message) for message in messages):
                return None
            if not messages or messages[-1].get('role') != 'assistant':
                return None
            if any(part.get('type') == 'toolCall' for part in messages[-1].get('content', []) if isinstance(part, dict)):
                return None
            return history
        return self.wait_for(label, probe)

    def wait_for(self, label, probe):
        deadline = time.monotonic() + self.args.timeout
        while True:
            result = probe()
            if result:
                return result
            if time.monotonic() >= deadline:
                raise AssertionError('Timed out: ' + label)
            time.sleep(0.25)

    def user_input(self, chat, message, purpose):
        record = {'purpose': purpose, 'chat': chat, 'text': message, 'at': time.monotonic()}
        self.evidence['interventions'].append(record)
        # Real native input; never append native JSONL or submit a report for a node.
        record['result'] = self.herdr('agent', 'prompt', chat['paneId'], message)
        return self.await_chat(chat, purpose, after=message)

    def documents(self, history):
        # Exact native tool addresses, never mtime/directory guesses. A legal
        # copy + edit + read is as real as write + read; the creation tool is not
        # a business acceptance condition. Product/Skill reads are not outputs.
        paths = []
        for call in self.calls(history):
            if call.get('name') in ['read', 'write', 'edit']:
                value = call.get('arguments', {}).get('path', '')
                if value.endswith('.md'):
                    path = Path(value)
                    if not path.is_absolute():
                        path = self.root / 'zone' / path
                    if call['name'] != 'read':
                        self.check(path.is_relative_to(self.root), ('Planning file outside owned root', str(path)))
                    if path.is_relative_to(self.root / 'zone') and path.is_file() and path not in paths:
                        paths.append(path)
        return paths

    def preserve_documents(self, history, label):
        destination = self.root / 'planning-revisions' / label
        destination.mkdir(parents=True, exist_ok=True)
        for index, path in enumerate(self.documents(history)):
            copied = destination / (str(index) + '-' + path.name)
            shutil.copy2(path, copied)
            self.evidence.setdefault('planningRevisions', []).append({
                'stage': label, 'source': str(path), 'raw': str(copied), 'sha256': self.digest(copied)})

    def section_check(self, path):
        body = path.read_text(encoding='utf-8')
        # Fixed numbered sections are part of the pre-run requested format, not
        # a classifier that could bless a four-heading draft as nine sections.
        for number in range(1, 10):
            self.check(re.search(r'^#{1,4}\s*' + str(number) + r'[.、．)）\s]', body, re.M),
                ('Missing Task nine-section contract', number, str(path)))
        return body

    def start_plan(self, user, label):
        # Explicit control of the manager's user conversation is recorded. The
        # manager—not this driver—chooses all workflow tool actions.
        started = self.http('/api/scheduler/maps/plan_workflow.json/runs', {
            'managerTaskId': self.ids['manager'], 'zone': 'acceptance/planning', 'userInput': user}, 201)
        self.run_id = started['id']
        self.run_ids.append(self.run_id)
        self.evidence['runs'].append({'label': label, 'started': started})
        return started

    def exercise(self):
        zone = str(self.root / 'zone')
        context = (f'本次为隔离的真实业务规划，方案 ID=p27-business-plan，生成文件 p27-business-plan.json。'
            f'业务在已登记 zone ID acceptance/planning（本机目录 {zone}），现有 orders.csv 为只读输入。只安排 W1→W2，node ID w1→w2；'
            f'W1 用 acceptance-aggregate（绑定 {self.ids["acceptance-aggregate"]}）汇总分类金额为 totals.csv；'
            f'W2 用 acceptance-review（绑定 {self.ids["acceptance-review"]}）独立验证后写 review.json。'
            '首次草案先列 category,total_cents，之后我会在 outline Chat 提交修订；不要预先猜测确认或发报告。'
            '保持 Task 约定的九个编号章节，不接受四标题替代。方案各修订保存独立 Markdown 并读回。'
            '各工作完成条件明确：W1 产出 books=1500、food=800 的分类金额；W2 独立验证金额、列及币种后交付 accepted=true 的 review.json。'
            f'配置服务 {self.backend_wire.base}；配置源 {self.root / "backend.toml"}，服务 cwd {self.root}；产品文档根 {REPO}。'
            '缺新业务 managerTaskId/管理者 zone/userInput，不授权代选或从本自举 Run 继承，不做启动预检。'
            '只授权只读调查、规划材料和静态新 Map，不执行汇总/核对业务，不启动生成 Map，不修改已有配置和运行记录。'
            '当前没有 Skill 工具时，直接读取隔离 skills 下真实 grilling/domain-modeling 指引完成讨论；不为讨论另建或改写产品领域文档。'
            '管理者先 dispatch outline，再 collect 等待报告；读报告后核实用户确认范围及 manual 发送依据，显式 complete outline、dispatch generate_map。'
            '生成节点完成后核对报告并显式 complete、finish。管理者无需读取详细业务文件，报告不自动推进。')
        self.evidence['initialUserInput'] = context
        started = self.start_plan(context, 'success')
        def running_outline():
            detail = self.detail()
            execution = detail['run']['nodes']['outline'].get('execution')
            return execution['chat'] if execution and execution.get('chat') else None
        chat = self.wait_for('native manager dispatches actual outline', running_outline)
        first = self.await_chat(chat, 'outline discusses independent business requirement', after=context)
        self.preserve_documents(first, 'before-revision')
        # A real user correction precedes approval, so r1 cannot be silently handed off.
        revision = '用户修订 P28-REVISION：' + self.requirements['revisionRequest'] + ' 更新为 r2，明确取代 r1，重新展示完整方案并等待我的方案确认；还未授权发送报告。'
        second = self.user_input(chat, revision, 'request revised planning version')
        self.preserve_documents(second, 'revised-before-approval')
        candidates = self.documents(second)
        revised = [path for path in candidates if 'currency' in path.read_text(encoding='utf-8') and 'r2' in path.read_text(encoding='utf-8')]
        self.check(revised, ('Revised actual outline was not saved from native write', [str(path) for path in candidates]))
        draft = revised[-1]
        before = self.section_check(draft)
        self.verify_plan(before, confirmed=False)
        self.check(not any(report['nodeId'] == 'outline' and report.get('canComplete') is True for report in self.detail()['reports']),
            'Outline claimed handoff before actual user approval')
        approval = (f'用户方案确认 P28-APPROVE-R2：我已阅读 {draft} 的 p27-business-plan r2 九章节方案，明确确认当前 r2 取代 r1，'
            '确认范围包括 W1/w1→W2/w2、真实 Bot/Task 绑定、材料位置、CNY 币种及核对要求、依赖、zone、权限与完成标准。'
            '只交付新 Map，不执行新业务；新业务管理者配置仍未提供，预检不执行。请在方案确认依据中记录本条 P28-APPROVE-R2 标识，保存该确认版本并完整读回。此消息只确认方案，尚未确认发送 manual 报告。')
        approved = self.user_input(chat, approval, 'explicit user approval of revised nine-section plan')
        self.preserve_documents(approved, 'user-confirmed')
        self.check(not self.report_calls(), 'manual report sent before separate user send approval')
        approved_documents = self.documents(approved)
        confirmed = [path for path in approved_documents if 'P28-APPROVE-R2' in path.read_text(encoding='utf-8')]
        self.check(confirmed, 'Actual saved plan lacks user confirmation source')
        confirmed_path = confirmed[-1]
        confirmed_body = self.section_check(confirmed_path)
        self.verify_plan(confirmed_body)
        self.confirmations['success'] = {'revision': revision, 'approval': approval, 'path': str(confirmed_path), 'draftSha256': hashlib.sha256(before.encode()).hexdigest()}
        send = '用户报告发送确认 P28-SEND-R2：同意现在发送当前已确认 p27-business-plan r2 的 manual 报告，登记刚刚保存读回的概要方案绝对路径为 outline 命名输出；只声明实际完成情况，不启动新业务。'
        self.user_input(chat, send, 'separate explicit manual report send approval')
        final = self.wait_for('native manager explicitly finishes outline to generated Map', lambda: (detail if (detail := self.detail())['run']['status'] == 'completed' else None))
        self.evidence['successFinal'] = final
        self.verify_success(final, chat, confirmed_path, approval, send)
        self.exercise_gap(context)
        self.check(self.configuration_bytes() == self.baseline, 'Shipped bootstrap Map, Task or Bot was modified by planning')
        self.evidence['configurationUnchanged'] = self.baseline
        self.evidence['confirmations'] = self.confirmations
        self.evidence['passed'] = True

    def verify_plan(self, body, *, confirmed=True):
        normalized = body.replace('\\', '/')
        if confirmed:
            self.check('P28-APPROVE-R2' in body, 'Confirmed plan lacks actual user confirmation reference')
        for value in ['p27-business-plan', 'r2', 'r1', 'W1', 'W2', 'currency', 'CNY', '1500', '800',
                self.ids['acceptance-aggregate'], self.ids['acceptance-review'], 'acceptance-aggregate', 'acceptance-review',
                str(self.root / 'zone/orders.csv'), str(self.root / 'zone/totals.csv'), str(self.root / 'zone/review.json')]:
            self.check(value.replace('\\', '/') in normalized, ('Confirmed plan omits independent business requirement', value))
        self.check('未' in body and '预检' in body, 'Plan must disclose absent new-business startup configuration')

    def preserve_history(self, chat, label):
        history = self.native(chat)
        self.check(history and history['id'] == chat['sessionId'], ('Native identity mismatch', chat))
        source = self.native_paths[chat['sessionId']]
        destination = self.root / 'native-evidence' / (label + '.jsonl')
        destination.parent.mkdir(exist_ok=True)
        shutil.copy2(source, destination)
        self.evidence.setdefault('nativeHistories', []).append({'label': label, 'chat': chat, 'native': history,
            'raw': str(destination), 'sha256': self.digest(destination)})
        return history

    def verify_success(self, final, outline_chat, confirmed_path, approval, send):
        outline = final['run']['nodes']['outline']
        generate = final['run']['nodes']['generate_map']
        identities = [final['run']['manager'], outline_chat, generate['execution']['chat']]
        self.check(len({chat['sessionId'] for chat in identities}) == 3, 'Native manager/outline/generator identities must differ')
        histories = [self.preserve_history(chat, label) for chat, label in zip(identities, ['success-manager', 'success-outline', 'success-generator'])]
        native_outline = self.native_messages(histories[1])
        users = [message for message in native_outline if message.get('role') == 'user']
        self.check(all(any(value in self.message_text(message) for message in users) for value in [approval, send]),
            'Explicit confirmation inputs are absent from actual native user history')
        report = next(report for report in reversed(final['reports']) if report['nodeId'] == 'outline')
        self.check(report.get('canComplete') is True and Path(next((item['path'] for item in report.get('outputs', []) if item['id'] == 'outline'), '')) == confirmed_path, ('Wrong named outline output', report))
        for saved in final['reports']:
            chat = outline_chat if saved['nodeId'] == 'outline' else identities[2]
            self.check(saved['from']['sessionId'] == chat['sessionId'] and saved['executionId'] == final['run']['nodes'][saved['nodeId']]['execution']['id'], ('Report identity/execution mismatch', saved))
        gen_calls = self.calls(histories[2])
        reads = [call for call in gen_calls if call.get('name') == 'read']
        self.check(any(Path(call.get('arguments', {}).get('path', '')) == confirmed_path for call in reads),
            'Actual generator never read the registered outline via its native read tool')
        for call in gen_calls:
            args = json.dumps(call.get('arguments', {}), ensure_ascii=False).lower()
            self.check(not any(value in args for value in ['messages.jsonl', 'history.jsonl', 'run.json', 'lastwritetime']),
                ('Generator searched runtime records or guessed a recent file', call))
        provider_requests = [record for record in self.model_wire.records if any(
            '[Named file inputs]' in self.message_text(message) and confirmed_path.as_posix() in self.message_text(message).replace('\\', '/')
            for message in json.loads(record.get('request', '{}')).get('messages', []))]
        self.check(provider_requests, 'Actual generator initialization lacks the named address')
        self.check(any(self.evidence['initialUserInput'] in self.message_text(message) for message in native_outline),
            'Actual outline initialization did not receive the independent requirement')
        records = self.backend_wire.records
        def request(record):
            try:
                return json.loads(record.get('request', ''))
            except ValueError:
                return {}
        complete = next(record for record in records if record['path'].endswith('/manager/complete') and request(record).get('nodeId') == 'outline')
        dispatch = next(record for record in records if record['path'].endswith('/manager/dispatch') and 'generate_map' in request(record).get('nodes', []))
        report_wire = next(record for record in records if record['path'].endswith('/node/report') and Path(next((item['path'] for item in request(record).get('outputs', []) if item['id'] == 'outline'), '')) == confirmed_path)
        self.check(report_wire['ended'] < complete['started'] < dispatch['started'], 'Report/explicit complete/explicit dispatch order is wrong')
        complete_response = json.loads(complete['response'])
        self.check(complete_response['snapshot']['nodes']['generate_map']['status'] == 'pending', 'complete auto-dispatched downstream')
        manager_calls = self.calls(histories[0])
        self.check(any(call.get('name') in ['read', 'bash'] and ('messages.jsonl' in json.dumps(call) or '/api/workflow/runs/' in json.dumps(call)) for call in manager_calls),
            'Manager did not actually read saved report content')
        target = self.root / 'maps/p27-business-plan.json'
        self.check(target.is_file(), 'Actual generator did not save the requested Map')
        delivered = json.loads(target.read_bytes())
        catalog = self.http('/api/scheduler/maps')
        visible = self.http('/api/scheduler/maps/' + urllib.parse.quote(target.name))
        self.check(target.name in catalog['files'], ('New business Map not service-visible', catalog))
        self.check(visible['nodes'].keys() == {'w1', 'w2'}, ('Business work-to-node mapping changed', visible))
        for work in ['W1', 'W2']:
            expected = self.requirements[work]
            node = visible['nodes'][expected['node']]
            self.check(node['bot_id'] == expected['bot'] and node['blocked_by'] == expected['blocked_by'], ('Wrong business binding/dependencies', work, node))
            self.check(Path(node['zone']) == self.root / 'zone', ('Wrong business zone', node))
            content = node['input'].replace('\\', '/')
            for path in [expected['input'], expected['output']]:
                self.check(path.replace('\\', '/') in content, ('Business node input is not self-contained', work, path))
            self.check('CNY' in content and 'currency' in content, ('User revision was not translated into business node', work, node))
            self.check(all(node.get(key) == delivered['nodes'][expected['node']].get(key) for key in ['bot_id', 'blocked_by', 'zone', 'input']), 'Service readback differs from saved Map')
        self.check(any(record['method'] == 'GET' and record['path'] == '/api/scheduler/maps/p27-business-plan.json' for record in records),
            'Generator did not verify saved Map through service interface')
        self.check(not any('/preflight' in record['path'] or (record['method'] == 'POST' and '/runs' in record['path']) for record in records),
            'A node performed unauthorized preflight or Run startup')
        runs = self.http('/api/workflow/runs')['runs']
        self.check(len(runs) == 1 and runs[0]['id'] == self.run_id, ('Generated business Map was automatically started', runs))
        self.check(not (self.root / 'zone/totals.csv').exists() and not (self.root / 'zone/review.json').exists(), 'Planning executed future business work')
        for path in [confirmed_path, target]:
            self.evidence.setdefault('files', []).append({'path': str(path), 'sha256': self.digest(path), 'bytes': path.stat().st_size})
        self.delivered_map_sha256 = self.digest(target)
        self.evidence['serviceReadback'] = {'catalog': catalog, 'map': visible, 'rawMap': str(target), 'runs': runs}
        self.evidence['coverage']['P27'] = {'nativeHistories': ['success-manager', 'success-outline', 'success-generator'],
            'reportId': report['id'], 'execution': outline['execution']['id'], 'registeredOutline': str(confirmed_path),
            'completeWire': complete['index'], 'dispatchWire': dispatch['index'], 'initialProviderRequests': [item['index'] for item in provider_requests],
            'mapReadback': 'serviceReadback', 'definitionDeployment': 'deployment.json'}
        self.evidence['coverage']['P28'] = {'nativeApproval': approval, 'separateManualSend': send,
            'nineSectionPlan': str(confirmed_path), 'independentRequirement': 'business-requirements.json',
            'sourceTaskContract': str(REPO / 'meta-data/tasks.json'), 'startup': 'Missing explicit business manager configuration; no preflight or automatic start in observed HTTP and persisted Run catalog'}

    def exercise_gap(self, context):
        user = context + '\n独立缺口场景：本次改为只规划 W1 发布到真实税务生产系统。没有 API、实现代码、凭据或生产授权；现有汇总/核对能力不能代替发布。保留 G1 真实能力阻塞，交付独立能力建设需求；用户尚未补齐，不允许创建 Task/Bot、调度能力创建者或生成业务 Map。不要沿用上一 Run 的确认。'
        started = self.start_plan(user, 'missing-capability')
        chat = self.wait_for('gap outline dispatched by native manager', lambda: (
            execution['chat'] if (execution := self.detail()['run']['nodes']['outline'].get('execution')) and execution.get('chat') else None))
        self.await_chat(chat, 'gap remains a real user-facing blocker', after=user)
        clarification = ('用户缺口范围澄清 P28-GAP-SCOPE：业务对象为本公司本期税务申报表，目标是尚未指定的真实税务生产系统。'
            '系统名称、接口、数据格式、实现代码、凭据和生产授权全部尚未提供，明确记为待定及 G1 阻塞，不自行补全。'
            '现阶段只交付独立能力建设需求，写清上述已知事实、未知项、禁止操作和补齐后需核实的返回契约；'
            '现在保存并读回该需求文档即可，不必等待未知项答案才记录阻塞。此输入不补齐能力、不确认概要方案、不授权报告发送，'
            '不创建 Task/Bot、不调度创建者、不生成或执行 Map。')
        history = self.user_input(chat, clarification, 'record explicit gap scope without supplying missing capability')
        self.preserve_documents(history, 'missing-capability')
        self.check(self.digest(self.root / 'maps/p27-business-plan.json') == self.delivered_map_sha256,
            'Gap scenario modified the independently delivered business Map')
        documents = self.documents(history)
        gap_documents = [path for path in documents if 'G1' in path.read_text(encoding='utf-8') and '税务' in path.read_text(encoding='utf-8')]
        self.check(gap_documents, 'Missing real capability did not produce its independent user handoff document')
        detail = self.detail()
        self.check(not any(report.get('canComplete') is True for report in detail['reports']), 'Missing capability falsely declared completable')
        self.check(detail['run']['nodes']['outline']['status'] == 'running' and detail['run']['nodes']['generate_map']['status'] == 'pending',
            'Missing capability advanced instead of remaining blocked')
        self.check(self.configuration_bytes() == self.baseline, 'Gap scenario auto-created/changed Task, Bot or bootstrap Map')
        self.check({path.name for path in (self.root / 'maps').glob('*.json')} == {'plan_workflow.json', 'p27-business-plan.json'},
            'Gap scenario generated an unauthorized business Map')
        self.preserve_history(chat, 'gap-outline')
        self.preserve_history(detail['run']['manager'], 'gap-manager')
        self.evidence['gap'] = {'started': started, 'finalBlockedSnapshot': detail, 'documents': [{'path': str(path), 'sha256': self.digest(path)} for path in gap_documents],
            'resolution': 'No user-provided replacement capability; intentional real blocker preserved, never claimed as success chain'}
        self.evidence['coverage']['P28']['gap'] = 'gap'
        runs = self.http('/api/workflow/runs')['runs']
        self.check({run['id'] for run in runs} == set(self.run_ids), 'Unexpected generated business Run exists')
        self.evidence['allRunsReadback'] = runs

    def cleanup_extra(self):
        # Preserve partially completed native conversations even on a failed
        # assertion; the normal success path also records SessionManager reads.
        native = self.root / 'agent/sessions'
        if native.is_dir():
            shutil.copytree(native, self.root / 'native-storage-copy')
        # Preserve every owned Run without rewriting any persistent fact.
        for run_id in self.run_ids:
            source = self.root / 'data/runs' / run_id
            if source.is_dir():
                shutil.copytree(source, self.root / 'run-record-copies' / run_id)
        super().cleanup_extra()
