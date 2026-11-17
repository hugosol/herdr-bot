"""Ticket 15's approved S2/S3 seams, backed by public HTTP and actual files.

Independent oracle: the four published V2 responsibilities (three manual, one
nested automatic), schema_version=2 business materials and a literal read-only
CSV Task. Violations include a V1 alias, dropped authorization/confirmation,
a forged native Yes, a report standing in for approval, or complete preceding
its latest ready report/declared outputs. No model decision/result is supplied.
"""
import copy
import hashlib
from html.parser import HTMLParser
import json
from pathlib import Path
import re
import shlex
import shutil
import sys
import time
import tomllib
import urllib.parse

from acceptance.host.native import NativeAcceptance
from acceptance.host.processes import require, until
from acceptance.host.wire import decode
from workflow_create_acceptance import prepare_source_resources, read_evidence

MODES = {'create-task-v2.json': {'create_task': False, 'create_bot_v2': True},
    'create-bot-v2.json': {'align_requirements': False, 'design_and_create': False}}
TASK_IDS = ['task-18e2c2827c484128-0', 'task-18e31d30fe69a51c-2',
    'task-18e892cb6456b0d0-0', 'task-18e89651aa593290-1']
QUESTION = '当前会话的成果是否已达到预期，可以交接并推进后续流程？'
PROMPT = '仅在用户提供两份已授权读取的本地CSV文件时，逐行比较并在当前Chat交付差异摘要。只读原文件，不上传、不写文件、不执行其他业务。材料缺失或不可读时说明缺项并等待用户。'
ITEMS = ['P34/V2/S2', 'P34/V2/S3', 'P35/V2/S2']


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def material_json(text, required, optional=()):
    def object_value(pairs):
        value = {}
        for key, item in pairs:
            require(key not in value, ('Duplicate V2 material field', key))
            value[key] = item
        return value
    value = json.loads(text, object_pairs_hook=object_value)
    require(isinstance(value, dict) and set(required) <= set(value) <= set(required) | set(optional), ('Missing or rewritten V2 protocol fields', value))
    return value


def authorize_candidate(stage, action, candidate, expected):
    """Acceptance-driver authorization, never a product approval fact."""
    allowed = {'create_task': {'confirm-task', 'confirm-request'},
        'align_requirements': {'confirm-requirements', 'confirm-target-materials'},
        'design_and_create': {'confirm-bot'}}
    decision = {'authorized': False, 'stage': stage, 'action': action, 'reason': 'stage-forbidden'}
    if action not in allowed.get(stage, set()):
        return decision
    decision['reason'] = 'candidate-not-displayed'
    if not isinstance(candidate, dict):
        return decision
    decision['reason'] = 'candidate-mismatch'
    task = expected['task']
    if action == 'confirm-task':
        valid = 'id' not in candidate and all(key in candidate and candidate[key] == value for key, value in task.items())
    elif action == 'confirm-bot':
        valid = (candidate.get('id') == expected['botId'] and candidate.get('name') == task['name']
            and expected.get('taskId') and candidate.get('task_id') == expected['taskId']
            and not any(key in candidate for key in ['map_file', 'script'])
            and all(isinstance(candidate.get(key), str) and candidate[key].strip() for key in ['when', 'plan_guide', 'node_guide']))
    elif action == 'confirm-request':
        valid = (candidate.get('schema_version') == 2 and candidate.get('kind') == 'request'
            and expected.get('taskId') and candidate.get('target') == {'type': 'task', 'task_id': expected['taskId']}
            and candidate.get('authorization') == {'write_materials': True, 'create_bot': True, 'optimize_task_prompt': False})
    elif action == 'confirm-target-materials':
        target = candidate.get('target', {})
        configuration = target.get('configuration', {})
        valid = (candidate.get('schema_version') == 2 and candidate.get('kind') == 'target_materials'
            and target.get('type') == 'task' and expected.get('taskId') and target.get('task_id') == expected['taskId']
            and all(configuration.get(key) == value for key, value in task.items()))
    else:
        text = candidate.get('requirements', '')
        valid = isinstance(text, str) and all(fact in text for fact in expected['requiredFacts'])
    decision.update(authorized=bool(valid), reason='authorized-current-candidate' if valid else 'candidate-mismatch')
    return decision


def displayed_candidates(text):
    """Only explicit structured blocks; prose is not a complete configuration."""
    blocks = re.findall(r'```([^\n`]*)\n(.*?)```', text, re.S)
    result = []
    for language, block in blocks:
        if language.strip().lower() in ['markdown', 'md']:
            result.append(('confirm-requirements', {'requirements': block}))
            continue
        if language.strip().lower() not in ['', 'json']:
            continue
        try:
            value = json.loads(block)
        except ValueError:
            continue
        if not isinstance(value, dict):
            continue
        if 'taskType' in value:
            result.append(('confirm-task', value))
        elif all(key in value for key in ['when', 'plan_guide', 'node_guide']):
            result.append(('confirm-bot', value))
        elif value.get('kind') in ['request', 'target_materials']:
            result.append(('confirm-request' if value['kind'] == 'request' else 'confirm-target-materials', value))
    return result


def cancelled_scope_ready(pending, history):
    if not pending:
        return False
    messages = [entry['message'] for entry in history['branch'] if entry.get('type') == 'message']
    if not messages:
        return False
    last = messages[-1]
    if last.get('role') != 'assistant':
        return False
    terminal = last.get('stopReason') == 'stop' or (
        last.get('stopReason') == 'aborted' and last.get('errorMessage') == 'Interrupted by user')
    return terminal and any(message.get('role') == 'toolResult' and
        message.get('toolCallId') == pending['toolCallId'] for message in messages)


def tool_calls(history):
    return [part for entry in history['branch'] if entry.get('type') == 'message'
        and entry['message'].get('role') == 'assistant' for part in entry['message'].get('content', [])
        if isinstance(part, dict) and part.get('type') == 'toolCall']


class BusinessForm(HTMLParser):
    """Read actual public Ask HTML; never replace its renderer or result."""
    def __init__(self, source):
        super().__init__()
        self.fields = []
        self.blocks = []
        self.pre = None
        self.label = None
        self.textarea = None
        self.select = None
        self.option = None
        self.labels = {}
        self.feed(source)
        for field in self.fields:
            if not field['label'] and field.get('id') in self.labels:
                field['label'] = self.labels[field['id']]

    def handle_starttag(self, tag, attributes):
        attributes = dict(attributes)
        if tag == 'pre':
            self.pre = ''
        if tag == 'label':
            self.label = {'text': '', 'fields': [], 'for': attributes.get('for')}
        if tag == 'input' and attributes.get('type', 'text') not in ['hidden', 'submit', 'button']:
            field = {**attributes, 'tag': tag, 'label': attributes.get('aria-label', '')}
            self.fields.append(field)
            if self.label is not None:
                self.label['fields'].append(field)
        if tag == 'textarea':
            self.textarea = {**attributes, 'tag': tag, 'value': '', 'label': ''}
            self.fields.append(self.textarea)
            if self.label is not None:
                self.label['fields'].append(self.textarea)
        if tag == 'select':
            self.select = {**attributes, 'type': 'select', 'tag': tag, 'label': attributes.get('aria-label', ''), 'options': []}
            self.fields.append(self.select)
            if self.label is not None:
                self.label['fields'].append(self.select)
        if tag == 'option' and self.select is not None:
            self.option = {**attributes, 'label': ''}
            self.select['options'].append(self.option)

    def handle_data(self, data):
        if self.pre is not None:
            self.pre += data
        if self.label is not None:
            self.label['text'] += data
        if self.textarea is not None:
            self.textarea['value'] += data
        if self.option is not None:
            self.option['label'] += data

    def handle_endtag(self, tag):
        if tag == 'pre' and self.pre is not None:
            self.blocks.append(self.pre)
            self.pre = None
        if tag == 'label' and self.label is not None:
            for field in self.label['fields']:
                field['label'] = self.label['text'].strip()
            if self.label['for']:
                self.labels[self.label['for']] = self.label['text'].strip()
            self.label = None
        if tag == 'textarea':
            self.textarea = None
        if tag == 'select':
            self.select = None
        if tag == 'option':
            self.option = None


class PublishedV2Acceptance(NativeAcceptance):
    def __init__(self, args):
        super().__init__(args)
        self.evidence.update(promise=None, promises=ITEMS, scenario='published-v2',
            coverage={key: {'verified': False} for key in ITEMS}, phases=[],
            independentExpected={'modes': MODES, 'targetPrompt': PROMPT, 'schemaVersion': 2},
            scenarioSource={'path': str(Path(__file__).resolve()), 'sha256': digest(Path(__file__))},
            budgets={'confirmationMs': 20000, 'requestMs': 30000, 'observationSeconds': 51,
                'reason': 'Existing 20s native confirmation + 30s HTTP budget + 1s margin; no product policy changes.'})
        for key, name in [('OMP_CODING_AGENT_DIR', 'agent'), ('XDG_DATA_HOME', 'state'), ('XDG_CACHE_HOME', 'cache'), ('TEMP', 'temp'), ('TMP', 'temp'), ('TMPDIR', 'temp')]:
            directory = self.root / name
            directory.mkdir(exist_ok=True)
            self.env[key] = str(directory)
            self.evidence['isolation'][key] = str(directory)
        self.confirmations = self.root / 'confirmations'
        self.confirmations.mkdir()
        self.env['CHIEF_MAP_CONFIRMATION_EVIDENCE_DIR'] = str(self.confirmations)
        self.histories = {}
        self.chats = {}
        self.answered = set()
        self.yes = {}
        self.stages = {}
        self.pending_scope = {}

    def configure_model(self):
        super().configure_model()
        settings = self.root / 'agent' / ('config.yml' if self.host == 'omp' else 'settings.json')
        self.evidence['nativeShellConfiguration'] = {'path': str(settings), 'text': settings.read_text(encoding='utf-8'),
            'scope': 'Installed host public configuration, not an outer launcher shell override'}
        prepare_source_resources(self)

    def role_catalog(self, toolkit):
        metadata = tomllib.loads((self.args.repo / 'meta-data/task-options.toml').read_text(encoding='utf-8'))
        catalog = ''.join('[[agents]]\nid = ' + json.dumps(item['id']) + '\nlabel = ' + json.dumps(item['label']) + '\n'
            for item in metadata['agents'])
        catalog += '[[models]]\nid = "default"\nlabel = "Default"\n[[models]]\nid = "probe"\nlabel = "Authorized V2 model"\nprovider_model = ' + json.dumps(self.provider_model) + '\nthinking = { omp = ["off"], pi = ["off"] }\n'
        for skill in metadata['skills']:
            catalog += '\n[[skills]]\n' + ''.join(key + ' = ' + json.dumps(value) + '\n' for key, value in skill.items())
        for extension in metadata['extensions']:
            entry = dict(extension)
            for name, value in self.deployment_roots.items():
                entry['entrypoint'] = entry['entrypoint'].replace('${roots.' + name + '}', value)
            require('${' not in entry['entrypoint'] and Path(entry['entrypoint']).exists(), ('Missing shipped fixed entry', entry))
            catalog += '\n[[extensions]]\n' + ''.join(key + ' = ' + json.dumps(value) + '\n' for key, value in entry.items())
        observer = self.root / 'confirmation-observer.mjs'
        shutil.copy2(self.args.repo / 'scripts/node-approval-observer.mjs', observer)
        shutil.copy2(self.args.repo / 'scripts/chief-ui-observer.mjs', self.root / 'chief-ui-observer.mjs')
        self.evidence['formObserverSource'] = {'path': 'scripts/node-approval-observer.mjs', 'sha256': digest(self.args.repo / 'scripts/node-approval-observer.mjs'),
            'upstreamCommit': '084330c03fe4b77024671b3c4319d54d7e2ff047', 'scope': 'Public UI pass-through; never supplies a decision or return'}
        catalog += '\n[[extensions]]\nid = "v2-ui-observer"\nlabel = "Native UI observation"\nagent = ' + json.dumps(self.host) + '\nkind = "fixed"\nentrypoint = ' + json.dumps(str(observer)) + '\n'
        self.evidence['fixedMetadata'] = {'source': str(self.args.repo / 'meta-data/task-options.toml'), 'sha256': digest(self.args.repo / 'meta-data/task-options.toml')}
        return catalog

    def start_backend(self, role_config):
        self.original_tasks = {task['id']: task for task in json.loads((self.args.repo / 'meta-data/tasks.json').read_text(encoding='utf-8'))['tasks']}
        self.deployed = {key: dict(self.original_tasks[key], agent=self.host, model='probe', thinking='default') for key in TASK_IDS}
        (self.root / 'data/tasks.json').write_text(json.dumps({'tasks': list(self.deployed.values())}, ensure_ascii=False), encoding='utf-8')
        super().start_backend(role_config)

    def start(self, argv, label):
        if label == 'herdr':
            path = self.root / 'herdr.toml'
            path.write_text(path.read_text(encoding='utf-8').replace('default_shell = "cmd.exe"', 'default_shell = "powershell.exe"') + '\n[server]\nheadless_cols = 220\nheadless_rows = 220\n', encoding='utf-8')
        if label.startswith('backend-'):
            path = self.root / 'backend.toml'
            path.write_text('paths_file = "paths.local.toml"\n' + path.read_text(encoding='utf-8'), encoding='utf-8')
        return super().start(argv, label)

    def prepare(self):
        super().prepare()
        for address in [self.base, self.backend_wire.base, self.model_wire.base]:
            require(address.startswith('http://127.0.0.1:') and 49152 <= int(address.rsplit(':', 1)[1]) <= 65535, ('Non-isolated listener', address))
        shell_probe = self.root / 'native-shell-probe.txt'
        shell_probe.write_text('V2-NATIVE-SHELL-815\n', encoding='utf-8')
        path = shlex.quote(shell_probe.as_posix())
        command = f"test -f {path} && cat {path} && curl --noproxy '*' --fail --silent --max-time 10 {shlex.quote(self.base + '/api/tasks')}"
        observed = self.command([self.args.shell_path, '-lc', command], seconds=20)
        require(observed.startswith('V2-NATIVE-SHELL-815\n'), ('Selected native shell cannot read the actual owned Windows file', observed))
        tasks = json.loads(observed[len('V2-NATIVE-SHELL-815\n'):])
        require({task['id'] for task in tasks['tasks']} == set(TASK_IDS), 'Selected native shell cannot read this isolated Windows backend')
        self.evidence['nativeShellProbe'] = {'shell': str(self.args.shell_path), 'sha256': digest(self.args.shell_path),
            'file': str(shell_probe), 'backend': self.base, 'taskIds': sorted(task['id'] for task in tasks['tasks']),
            'scope': 'Real dependency namespace check before model launch, not V2 business success'}
        temporary = self.root / 'temp'
        temp_code = 'import os,json;print(json.dumps({key:os.environ[key] for key in ["TMP","TEMP","TMPDIR"]}))'
        temp_command = shlex.quote(Path(sys.executable).as_posix()) + ' -c ' + shlex.quote(temp_code)
        inherited = json.loads(self.command([self.args.shell_path, '-lc', temp_command], seconds=20))
        require(all(Path(value).resolve() == temporary.resolve() for value in inherited.values()), ('Native shell temporary environment escaped owned root', inherited))
        self.evidence['nativeTemporaryProbe'] = {'inheritedByShellChild': inherited, 'expectedDirectory': str(temporary),
            'scope': 'Actual selected shell child environment; no OS sandbox claim and no shared /tmp access'}
        # Match the existing native planning runner: one read-only repository
        # root for Task document references and scheduler.run_instructions.
        repository = self.args.repo
        dictionary = {'herdr-bot-path': str(repository).replace('\\', '/'), 'herdr-bot-api-url': self.backend_wire.base}
        (self.root / 'prompts.local.toml').write_text(''.join(key + ' = ' + json.dumps(value) + '\n' for key, value in dictionary.items()), encoding='utf-8')
        for relative, field, value in [('extensions/connect-bot/config.yaml', 'endpoint', self.backend_wire.base),
            ('extensions/model-cost/config.yaml', 'storageDir', str(self.root / 'cost-archive'))]:
            path = self.root / 'toolkit' / relative
            lines = path.read_text(encoding='utf-8').splitlines()
            require(sum(line.startswith(field + ':') for line in lines) == 1, ('Cannot relocate shipped setting', relative))
            path.write_text('\n'.join(field + ': ' + json.dumps(value) if line.startswith(field + ':') else line for line in lines) + '\n', encoding='utf-8')
        config = self.root / 'toolkit/framework/config.yaml'
        config.write_text(config.read_text(encoding='utf-8').replace('      enabled: true', '      enabled: false'), encoding='utf-8')
        for key, task in self.deployed.items():
            require(self.http('/api/tasks/' + key)['task'] == task, ('Published Task changed beyond explicit host/model relocation', key))
        self.evidence['deployment'] = {'originalTasks': {key: self.original_tasks[key] for key in TASK_IDS}, 'deployedTasks': self.deployed,
            'dictionary': dictionary, 'roots': self.deployment_roots, 'suites': self.http('/api/suites')}

    def browser(self, action, **values):
        prefix = self.root / ('browser-' + action)
        request = {'action': action, 'base': self.base, 'chrome': str(self.args.chrome),
            'profile': str(prefix) + '-profile', 'evidence': str(prefix) + '.json', **values}
        path = self.root / ('browser-' + action + '-request.json')
        path.write_text(json.dumps(request, ensure_ascii=False), encoding='utf-8')
        raw = self.command([self.args.node, self.args.repo / 'scripts/node-approval-v2-browser.mjs', path], seconds=180)
        result = json.loads(Path(request['evidence']).read_text(encoding='utf-8'))
        self.evidence.setdefault('browser', {})[action] = result
        require(result['passed'] and not result['cleanupErrors'], ('Browser did not verify delivered V2 facts', raw))
        return result

    @staticmethod
    def messages(history):
        return [entry['message'] for entry in history['branch'] if entry.get('type') == 'message']

    def send(self, chat, message, kind):
        ready = self.herdr('agent', 'wait', chat['paneId'], '--until', 'idle', '--until', 'done', '--timeout', '10000')
        record = {'kind': kind, 'sessionId': chat['sessionId'], 'input': message, 'at': time.monotonic(), 'ready': ready}
        self.evidence.setdefault('userInputs', []).append(record)
        result = self.command([self.args.herdr, 'agent', 'prompt', chat['paneId'], message,
            '--wait', '--until', 'working', '--until', 'blocked', '--timeout', '10000'], seconds=15)
        record['admission'] = json.loads(result)['result']
        return record['admission']

    def records(self, path, session=None):
        return [record for record in self.backend_wire.records if record['path'] == path and
            (session is None or (decode(record, 'request') or {}).get('sessionId') == session)]

    def remember(self, chat):
        if not chat or not chat.get('sessionId'):
            return
        identity = chat['sessionId']
        self.chat_hosts[identity] = self.host
        self.chats[identity] = chat
        history = self.native(chat)
        if history:
            self.histories[identity] = history
        if identity not in self.evidence.setdefault('observedSessions', []):
            self.observe_processes()
            self.evidence['observedSessions'].append(identity)

    def screen(self, chat):
        return self.command([self.args.herdr, 'pane', 'read', chat['paneId'], '--source', 'visible', '--format', 'text'])

    def native_yes(self, chat, run, node):
        requests = [json.loads(path.read_text(encoding='utf-8')) for path in self.confirmations.glob('*.request.json')]
        for request in requests:
            if request['sessionId'] != chat['sessionId'] or request['id'] in self.yes:
                continue
            require(request['title'] == '成果批准' and request['message'] == QUESTION and request['hasUI'] is True and request['timeoutMs'] == 20000, request)
            screen = self.screen(chat)
            require(QUESTION in screen and 'Yes' in screen and 'No' in screen, ('Native question not fully visible', screen))
            selected = lambda text: any(arrow + ' Yes' in text for arrow in ['→', '❯'])
            if not selected(screen):
                self.herdr('agent', 'send-keys', chat['paneId'], 'up')
                screen = until('actual native Yes selection', lambda: value if selected(value := self.screen(chat)) else None, 5)
            before = self.detail(run)
            require(before['run']['nodes'][node]['execution']['approved'] is False, before)
            self.herdr('agent', 'send-keys', chat['paneId'], 'enter')
            response = self.confirmations / (request['id'] + '.response.json')
            result = until('native confirmation true return', lambda: json.loads(response.read_text(encoding='utf-8')) if response.is_file() else None, 20)
            require(result.get('result') is True, ('Keystroke acknowledgement is not Yes', result))
            self.yes[request['id']] = {'request': request, 'selectedScreen': screen, 'response': result, 'publicBefore': before}
            self.evidence['nativeYes'] = self.yes
            if not self.evidence.get('approvedWithoutReadyReport'):
                def approved():
                    detail = self.detail(run)
                    return detail if detail['run']['nodes'][node]['execution']['approved'] else None
                actual = until('native Yes committed actual V2 approval', approved, 30)
                execution = actual['run']['nodes'][node]['execution']
                reports = [report for report in actual['reports'] if report['executionId'] == execution['id']]
                require(actual['run']['nodes'][node]['status'] == 'running' and (not reports or reports[-1]['canComplete'] is not True), ('Required V2 approved-without-ready-report condition was not reached', actual))
                refused = self.http('/api/workflow/manager/complete', {'sessionId': actual['run']['manager']['sessionId'], 'nodeId': node,
                    'reason': 'V2 consumer probe: approval cannot replace this execution latest ready report'}, 409)
                require(refused['code'] == 'report-not-completable', refused)
                after = self.detail(run)
                require(after['run']['nodes'][node]['status'] == 'running', 'Approval alone completed the published V2 node')
                self.evidence['approvedWithoutReadyReport'] = {'before': actual, 'refusal': refused, 'after': after,
                    'scope': 'Public HTTP consumer probe in the real V2 Run; not a fabricated native manager tool call', 'at': time.monotonic()}

    def authorization(self, chat, text):
        stage = self.stages.get(chat['sessionId'])
        expected_task = {'name': self.target_name, 'chatName': '{chat.id}', 'taskType': 'normal',
            'agent': self.host, 'model': 'probe', 'thinking': 'default', 'skill': None, 'suite': None, 'prompt': PROMPT}
        targets = [task for task in self.http('/api/tasks')['tasks']
            if all(task.get(key) == value for key, value in expected_task.items())]
        expected = {'task': expected_task, 'taskId': targets[0]['id'] if len(targets) == 1 else None,
            'botId': 'v2-csv-' + self.host, 'requiredFacts': ['CSV', '只读', '不上传', '不写']}
        candidates = displayed_candidates(text)
        if len(candidates) != 1:
            decision = authorize_candidate(stage, 'unknown-action', None, expected)
            decision['reason'] = 'missing-or-ambiguous-displayed-candidate'
        else:
            action, candidate = candidates[0]
            decision = authorize_candidate(stage, action, candidate, expected)
            decision['candidate'] = candidate
        self.evidence.setdefault('businessAuthorization', []).append({'sessionId': chat['sessionId'], **decision, 'at': time.monotonic()})
        return decision

    def scope_message(self, decision):
        scope = {
            'create_task': '本阶段只处理本次normal Task完整配置、创建及本节点委托材料；不创建Bot，不批准后续三指南正文或Bot保存。',
            'align_requirements': '本阶段只整理并确认本次统一需求和目标材料；不创建Bot，不批准后续三指南正文或Bot保存。',
            'design_and_create': '本阶段只可在完整已展示且符合本次要求的Bot候选获确认后，创建本次新Bot及本阶段交付；不修改Task.prompt或任何既有对象。',
        }.get(decision.get('stage'), '只允许既有管理职责；不授权额外业务保存或未知正文。')
        action = {'confirm-task': '本次Task完整候选及其新建', 'confirm-bot': '本次Bot完整候选及其新建',
            'confirm-request': '本次已展示V2委托材料', 'confirm-target-materials': '本次已展示完整目标材料',
            'confirm-requirements': '本次已展示统一需求'}.get(decision.get('action'), '未知动作')
        reply = ('仅确认刚展示的' + action + '。') if decision['authorized'] else '此次没有批准新增保存或未知正文；请明确本阶段待确认的完整候选与动作范围。'
        return reply + scope + '已给定CSV只读、不上传、不写原文件及Task.prompt全文约束不变。新材料只能在既有授权目录以新文件保存，不覆盖。' + self.environment_scope

    def business_form(self, chat, history):
        messages = self.messages(history)
        finished = {message.get('toolCallId') for message in messages if message.get('role') == 'toolResult'}
        pending = [call for call in tool_calls(history) if call['name'].rsplit('.', 1)[-1] == 'ask' and call['id'] not in finished and call['id'] not in self.answered]
        if not pending:
            return False
        require(len(pending) == 1, ('Ambiguous concurrent business questions', pending))
        call = pending[0]
        html = call['arguments'].get('html', '')
        require(html and any(word in html for word in ['CSV', '需求', '指南', 'prompt', '配置', '保存']), ('Unrecognized business form', call))
        screen = self.screen(chat)
        if 'Ask' not in screen or 'Submit' not in screen:
            return True
        observed = []
        for path in (self.confirmations / 'forms').glob('*.request.json'):
            value = json.loads(path.read_text(encoding='utf-8'))
            if value.get('sessionId') == chat['sessionId'] and value.get('html') == html:
                observed.append(value)
        if not observed:
            return True
        require(len(observed) == 1 and observed[0].get('hasUI') is True, ('Ambiguous native business form invocation', observed))
        response_path = self.confirmations / 'forms' / (observed[0]['id'] + '.response.json')
        require(not response_path.exists(), 'Business form already answered before this explicit user input')
        form = BusinessForm(html)
        decision = self.authorization(chat, '\n'.join('```json\n' + block + '\n```' for block in form.blocks))
        if not decision['authorized']:
            self.herdr('agent', 'send-keys', chat['paneId'], 'esc')
            self.answered.add(call['id'])
            returned = until('actual native form rejection return', lambda: json.loads(response_path.read_text(encoding='utf-8')) if response_path.is_file() else None, 20)
            self.evidence.setdefault('businessFormRefusals', []).append({'call': call, 'nativeInvocation': observed[0],
                'nativeReturn': returned, 'decision': decision, 'selectedAction': 'Esc; no authorization'})
            self.pending_scope[chat['sessionId']] = {'toolCallId': call['id'], 'message': self.scope_message(decision)}
            return True
        groups = []
        for field in form.fields:
            if field.get('type') == 'radio':
                existing = next((group for group in groups if group[0].get('type') == 'radio' and group[0].get('name') == field.get('name')), None)
                if existing is None:
                    groups.append([field])
                else:
                    existing.append(field)
            else:
                groups.append([field])
        require(groups, ('Native business Ask has no observable controls', call))
        record = {'sessionId': chat['sessionId'], 'call': call, 'beforeScreen': screen, 'choices': [], 'at': time.monotonic()}
        self.evidence.setdefault('businessForms', []).append(record)
        for group in groups:
            field = group[0]
            if field.get('type') == 'radio':
                allowed = [index for index, option in enumerate(group) if (any(word in option['label'] for word in ['确认', '同意', '按当前', '保持', '保留', '不优化', '不修改', '允许', '继续', 'Yes'])
                    or re.match(r'^\s*是[：:、，]', option['label']))
                    and not any(word in option['label'] for word in ['不确认', '不同意', '暂不', '取消', '拒绝', '修改后', '需要修改'])]
                require(len(allowed) == 1, ('Business form lacks one independently authorized choice', group))
                chosen = allowed[0]
                # A newly presented native radio form focuses its first option.
                # Every actual key is followed by observation, never replayed.
                for _ in range(chosen):
                    self.herdr('agent', 'send-keys', chat['paneId'], 'down')
                    self.screen(chat)
                self.herdr('agent', 'send-keys', chat['paneId'], 'space')
                label = group[chosen]['label']
                selected = until('native business radio value visible', lambda: value if label in (value := self.screen(chat)) and any(mark in value for mark in ['●', '◉', '[x]', '[✓]', '☑']) else None, 5)
                record['choices'].append({'name': field.get('name'), 'value': group[chosen].get('value', 'on'), 'selectedScreen': selected})
            elif field.get('type') == 'select':
                options = field['options']
                allowed = [index for index, option in enumerate(options) if option.get('value') in [self.host, 'probe', 'default'] or
                    (any(word in option['label'] for word in ['确认', '同意', '保留', '保持', '不优化', '不修改']) and not any(word in option['label'] for word in ['不确认', '不同意', '修改后', '需要修改']))]
                require(len(allowed) == 1, ('Dropdown has no unique authorized business value', field))
                chosen = allowed[0]
                current = next((index for index, option in enumerate(options) if 'selected' in option), 0)
                for _ in range(abs(chosen - current)):
                    self.herdr('agent', 'send-keys', chat['paneId'], 'right' if chosen > current else 'left')
                    self.screen(chat)
                selected = self.screen(chat)
                require(options[chosen]['label'].strip() in selected, ('Selected dropdown value is not visible', selected, field))
                record['choices'].append({'name': field['name'], 'value': options[chosen].get('value', options[chosen]['label']), 'selectedScreen': selected})
            elif field.get('type') == 'checkbox':
                require(any(word in field['label'] for word in ['确认', '同意', '授权']) and not any(word in field['label'] for word in ['不确认', '不同意', '修改既有']), ('Unknown checkbox decision', field))
                if 'checked' not in field:
                    self.herdr('agent', 'send-keys', chat['paneId'], 'space')
                record['choices'].append({'name': field.get('name'), 'value': True, 'selectedScreen': self.screen(chat)})
            else:
                text = field.get('value', '')
                if not text and 'required' in field:
                    question = (field.get('name', '') + ' ' + field['label']).lower()
                    if 'prompt' in question:
                        text = PROMPT
                    elif any(word in question for word in ['model', '模型']):
                        text = 'probe'
                    elif any(word in question for word in ['agent', '宿主']):
                        text = self.host
                    elif any(word in question for word in ['thinking', '思考']):
                        text = 'default'
                    elif any(word in question for word in ['目录', '位置', 'path']):
                        text = str(self.delivery_directory)
                    elif any(word in question for word in ['bot_id', 'bot id']):
                        text = 'v2-csv-' + self.host
                    elif any(word in question for word in ['name', '名称', '命名']):
                        text = self.target_name
                    else:
                        require(any(word in question for word in ['确认', '授权', '意见', 'confirm', 'approval']), ('Business form requests a fact not supplied by the user', field))
                        text = self.scope_message(decision)
                    self.herdr('pane', 'send-text', chat['paneId'], text)
                record['choices'].append({'name': field.get('name'), 'value': text, 'selectedScreen': self.screen(chat)})
            self.herdr('agent', 'send-keys', chat['paneId'], 'tab')
        record['beforeSubmit'] = self.screen(chat)
        require('Submit' in record['beforeSubmit'], 'Native business form Submit not visible')
        self.herdr('agent', 'send-keys', chat['paneId'], 'enter')
        self.answered.add(call['id'])
        def returned():
            actual = self.native(chat)
            return next((message for message in self.messages(actual) if message.get('role') == 'toolResult' and message.get('toolCallId') == call['id']), None) if actual else None
        result = until('actual native business form result', returned, self.args.timeout)
        require(not result.get('isError') and not any(word in json.dumps(result, ensure_ascii=False).lower() for word in ['cancelled', 'timedout', 'timed_out']), ('Business form did not submit actual user input', result))
        raw_return = until('actual public native formDialog return', lambda: json.loads(response_path.read_text(encoding='utf-8')) if response_path.is_file() else None, 20)
        require(isinstance(raw_return.get('result', {}).get('data'), dict), ('Native form did not return submitted values', raw_return))
        data = raw_return['result']['data']
        for choice in record['choices']:
            require(data.get(choice['name']) == choice['value'], ('Native form returned a different business choice', choice, data))
        record.update(result=result, nativeInvocation=observed[0], nativeReturn=raw_return)
        return True

    def business_reply(self, chat):
        history = self.histories.get(chat['sessionId'])
        if not history:
            return
        if self.business_form(chat, history):
            return
        messages = [entry for entry in history['branch'] if entry.get('type') == 'message']
        if not messages:
            return
        last = messages[-1]
        message = last['message']
        pending = self.pending_scope.get(chat['sessionId'])
        if cancelled_scope_ready(pending, history):
            self.answered.add(last['id'])
            self.pending_scope.pop(chat['sessionId'])
            self.send(chat, pending['message'], 'business-scope-refusal-and-clarification')
            return
        require(message.get('stopReason') not in ['error', 'aborted'], ('Actual model turn failed; no retry', last))
        if message.get('role') != 'assistant' or message.get('stopReason') != 'stop' or last['id'] in self.answered:
            return
        # A question is not evidence of a complete candidate. Only structured
        # content actually displayed by this assistant can receive authorization.
        text = '\n'.join(part.get('text', '') for part in message.get('content', []) if isinstance(part, dict) and part.get('type') == 'text')
        if not re.search(r'是否|请(?:你|用户)?确认|等待(?:你|用户)?确认|待[^。\n]{0,20}确认|(?:建议|尚未)[^。\n]{0,80}确认|请[^。\n]{0,30}授权|需要[^。\n]{0,30}授权|请提供', text):
            return
        require(any(word in text for word in ['CSV', '需求', '指南', 'prompt', '配置', '保存']), ('Unexpected business decision needs explicit criteria', text))
        self.answered.add(last['id'])
        decision = self.authorization(chat, text)
        self.evidence.setdefault('businessCandidateConfirmations', []).append({'sessionId': chat['sessionId'], 'candidate': last, 'decision': decision, 'at': time.monotonic()})
        self.send(chat, self.scope_message(decision), 'actual-current-stage-candidate-authorized' if decision['authorized'] else 'business-scope-not-authorized')

    def drive(self, root_id):
        started = time.monotonic()
        final = {}
        while time.monotonic() - started < self.args.timeout * 6:
            root = self.detail(root_id)
            runs = [root]
            parent = root['run']['nodes']['create_bot_v2'].get('execution')
            if parent and parent.get('childRunId'):
                runs.append(self.detail(parent['childRunId']))
            active = set()
            for detail in runs:
                manager = detail['run'].get('manager')
                self.remember(manager)
                if manager and detail['run']['status'] == 'running':
                    active.add(manager['sessionId'])
                for node, state in detail['run']['nodes'].items():
                    execution = state.get('execution')
                    if not execution:
                        continue
                    chat = execution.get('chat')
                    self.remember(chat)
                    if chat:
                        self.stages[chat['sessionId']] = node
                    if chat and state['status'] == 'running':
                        active.add(chat['sessionId'])
                    if chat and state['status'] == 'running' and not execution['autoApproved']:
                        self.native_yes(chat, detail['run']['id'], node)
                final[detail['run']['id']] = detail
            for identity in active:
                self.business_reply(self.chats[identity])
            self.evidence['lastPublicRuns'] = final
            require(not any(node['status'] == 'failed' for detail in runs for node in detail['run']['nodes'].values()), ('Actual V2 node failed; preserve the manager decision, do not retry or wait for a success', runs))
            if len(runs) == 2 and all(detail['run']['status'] == 'completed' for detail in runs):
                return runs
            require(all(detail['run']['status'] not in ['failed', 'cancelled'] for detail in runs), ('Actual V2 run failed; no retry', runs))
            time.sleep(0.25)
        raise AssertionError('Actual V2 model business did not finish; preserve native histories/screens, no retry')

    def material(self, path):
        path = Path(path)
        require(path.is_absolute() and path.resolve().is_relative_to(self.root.resolve()) and path.is_file(), ('Material escaped owned root or is missing', str(path)))
        current = self.root
        for part in path.relative_to(self.root).parts:
            current /= part
            require(not current.is_symlink() and not (current.stat().st_file_attributes & 0x400), ('Reparse material path', str(current)))
        text = path.read_text(encoding='utf-8')
        self.evidence.setdefault('materials', {})[str(path)] = {'text': text, 'sha256': digest(path)}
        return text

    def verify_run(self, detail, map_file):
        require(set(detail['run']['nodes']) == set(MODES[map_file]), ('Replaced V2 topology', detail))
        manager = detail['run']['manager']['sessionId']
        manager_calls = tool_calls(self.histories[manager])
        require(any(call['name'].rsplit('.', 1)[-1] == 'run' and call.get('arguments', {}).get('action') == 'finish' for call in manager_calls), 'No real native explicit manager finish')
        delivered = {}
        for node, automatic in MODES[map_file].items():
            state = detail['run']['nodes'][node]
            execution = state['execution']
            require(state['status'] == 'completed' and execution['approved'] is True and execution['autoApproved'] is automatic and execution['approvalSource'] == ('auto' if automatic else 'manual'), ('Approval responsibility/source mismatch', state))
            session = execution['chat']['sessionId']
            approvals = self.records('/api/workflow/node/approve', session)
            require(len(approvals) == 1 and approvals[0].get('status') == 200 and decode(approvals[0], 'request')['source'] == ('auto' if automatic else 'manual'), approvals)
            native = tool_calls(self.histories[session])
            require(any(call['name'].rsplit('.', 1)[-1] == 'approve' for call in native), ('No actual native executor approval', session))
            require(any(call['name'].rsplit('.', 1)[-1] == 'report' and call.get('arguments', {}).get('can_complete') is True for call in native), ('No actual native normal handoff report', session))
            require(any(call['name'].rsplit('.', 1)[-1] == 'run' and call.get('arguments', {}).get('action') == 'complete' and call.get('arguments', {}).get('nodeId') == node for call in manager_calls), ('HTTP complete did not come from native manager tool', node))
            ui = [value for value in self.yes.values() if value['request']['sessionId'] == session]
            require(len(ui) == (0 if automatic else 1), ('Manual Yes mixed with automatic executor judgment', node, ui))
            reports = [report for report in detail['reports'] if report['executionId'] == execution['id']]
            require(reports and reports[-1]['canComplete'] is True, ('Approval cannot replace latest ready report', node, reports))
            paths = {item['id']: item['path'] for report in reports for item in report.get('outputs', [])}
            require(set(paths) == {item['id'] for item in execution['outputRule']}, ('Missing declared actual delivery', node, paths))
            delivered[node] = {key: (path, self.material(path)) for key, path in paths.items()}
            # The intentional approval-without-report probe is a real 409,
            # not a manager handoff. Keep it in the wire evidence, but count
            # only successful commits for the exactly-once completion oracle.
            completes = [record for record in self.records('/api/workflow/manager/complete', manager)
                if decode(record, 'request').get('nodeId') == node and record.get('status') == 200]
            require(len(completes) == 1 and completes[0].get('status') == 200 and approvals[0]['ended'] < completes[0]['started'], ('No explicit successful native manager complete after approval', completes))
            report_calls = self.records('/api/workflow/node/report', session)
            ready = [record for record in report_calls if decode(record, 'request').get('canComplete') is True and record.get('status') == 200]
            require(ready and ready[-1]['ended'] < completes[0]['started'], ('Manager complete preceded latest ready report', ready, completes))
            prior_reports = [record for record in report_calls if record.get('ended', float('inf')) < completes[0]['started'] and record.get('status') == 200]
            require(prior_reports and decode(prior_reports[-1], 'request').get('canComplete') is True, ('An earlier ready report hid a latest non-ready report', prior_reports))
            require(approvals[0]['ended'] < ready[0]['started'], ('Normal handoff report preceded actual result approval', approvals, ready))
            if automatic:
                child_id = execution['childRunId']
                require(self.records('/api/workflow/manager/finish', session)[0]['ended'] < approvals[0]['started'], ('Parent approved before real child finish', child_id))
            else:
                for input_item in execution.get('inputs', []):
                    read_evidence(self, self.histories[session], Path(input_item['path']))
        return delivered

    def check_model_identity(self, verify=True):
        configured = json.loads(self.args.model_config.read_text(encoding='utf-8'))['model']
        records = [record for record in self.model_wire.records if record['method'] == 'POST' and record['path'].split('?', 1)[0].endswith('/chat/completions')]
        identities, errors = [], []
        if not records:
            errors.append({'error': 'No actual completion requests'})
        try:
            until('in-flight actual completions finish within existing request budget', lambda: all(record.get('ended') for record in records), 30)
        except AssertionError as error:
            errors.append({'error': str(error)})
        for record in records:
            if record.get('status') != 200 or record.get('error'):
                errors.append({'request': record['index'], 'status': record.get('status'), 'error': record.get('error', 'Actual completion failed; no retry')})
                continue
            try:
                request = decode(record, 'request')
                raw = record['response']
                packets = [json.loads(raw)] if raw.lstrip().startswith('{') else [json.loads(line[5:].strip()) for line in raw.splitlines() if line.startswith('data:') and line[5:].strip() not in ['', '[DONE]']]
                returned = sorted({packet['model'] for packet in packets if packet.get('model')})
                identities.append({'request': record['index'], 'requested': request.get('model'), 'returned': returned})
                if request.get('model') != configured or returned != [configured]:
                    errors.append({'request': record['index'], 'error': 'Actual model identity mismatch', 'returned': returned})
            except (ValueError, TypeError, AttributeError) as error:
                errors.append({'request': record['index'], 'error': str(error)})
        self.evidence['modelIdentities'] = identities
        self.evidence['modelIdentityErrors'] = errors
        if errors:
            self.evidence['passed'] = False
        if verify:
            require(not errors, ('Model identity/transport verification failed; raw evidence retained, no retry', errors))
        return not errors

    def check_guidance(self):
        records = [record for record in self.model_wire.records if record['method'] == 'POST' and record['path'].split('?', 1)[0].endswith('/chat/completions')]
        requests = '\n'.join(json.dumps(decode(record, 'request').get('messages', []), ensure_ascii=False) for record in records)
        consumed = []
        for key in TASK_IDS:
            prompt = self.original_tasks[key]['prompt']
            if not prompt:
                continue
            for name, value in self.evidence['deployment']['dictionary'].items():
                prompt = prompt.replace('${' + name + '}', value)
            require(json.dumps(prompt, ensure_ascii=False)[1:-1] in requests, ('Actual model did not consume the published business prompt', key))
            consumed.append({'taskId': key, 'originalPromptSha256': hashlib.sha256(self.original_tasks[key]['prompt'].encode()).hexdigest()})
        self.evidence['modelConsumedPublishedPrompts'] = consumed

    def exercise(self):
        self.v1_before = {str(path.relative_to(self.args.repo)): digest(path) for path in (self.args.repo / 'meta-data').rglob('*') if path.is_file()}
        for path in (self.args.repo / 'meta-data/bots').glob('*.json'):
            shutil.copy2(path, self.root / 'bots' / path.name)
        contract = self.root / 'zone/docs/contracts'
        contract.mkdir(parents=True)
        shutil.copy2(self.args.repo / 'docs/contracts/create-bot-v2-materials.md', contract / 'create-bot-v2-materials.md')
        self.http('/api/zones', {'id': 'v2/owned', 'path': str(self.root / 'zone')})
        definitions = {}
        for name, modes in MODES.items():
            original = json.loads((self.args.repo / 'meta-data/workflow-maps' / name).read_text(encoding='utf-8'))
            require({key: value['auto_approved'] for key, value in original['nodes'].items()} == modes, ('Published responsibility drift', name))
            definition = copy.deepcopy(original)
            for node in definition['nodes'].values():
                node['zone'] = 'v2/owned'
                node['input'] = node['input'].replace('E:/Developer/myhub/herdr-bot/docs/contracts/create-bot-v2-materials.md', str(contract / 'create-bot-v2-materials.md'))
                for rule in node.get('input_rule', []):
                    if rule['type'] == 'file':
                        rule['zone_id'] = 'v2/owned'
            for item in definition.get('input_files', []):
                item['description'] = item['description'].replace('E:/Developer/myhub/herdr-bot/docs/contracts/create-bot-v2-materials.md', str(contract / 'create-bot-v2-materials.md'))
            (self.root / 'maps' / name).write_text(json.dumps(definition, ensure_ascii=False), encoding='utf-8')
            definitions[name] = self.http('/api/scheduler/maps/' + name)
        self.evidence['definitions'] = definitions
        protected = {str(path.relative_to(self.root)): digest(path) for directory in ['maps', 'bots'] for path in (self.root / directory).glob('*.json')}
        self.evidence['ownedDefinitionBaseline'] = protected
        self.browser('configuration', definitions=definitions, modes=MODES)
        delivery = self.root / 'zone/deliveries'
        delivery.mkdir()
        name = 'V2-CSV只读摘要-' + self.host
        self.target_name, self.delivery_directory = name, delivery
        self.environment_scope = ('环境事实：只读文档与源码根为' + self.args.repo.as_posix()
            + '；只读Skill根为' + self.args.wayfinder.as_posix() + '及' + self.args.wayfinder_skills.as_posix()
            + '。唯一可写业务材料目录为' + delivery.as_posix() + '，唯一可写临时目录为' + (self.root / 'temp').as_posix()
            + '；禁止共享Temp、/tmp或其他目录中的发现式查找和写入，不读取其他Chat/Run或认证；不安装、刷新凭据或修改Skill/文档。'
            + '当前服务由本次配置提供；不得因猜出的相对路径失败而扫描其他目录。')
        self.evidence['filesystemAuthority'] = {'readOnlyRepository': self.args.repo.as_posix(),
            'readOnlySkills': [self.args.wayfinder.as_posix(), self.args.wayfinder_skills.as_posix()],
            'writableMaterials': delivery.as_posix(), 'writableTemporary': (self.root / 'temp').as_posix(),
            'temporaryEnvironment': {key: self.env[key] for key in ['TMP', 'TEMP', 'TMPDIR']},
            'scope': 'Explicit owned configuration and business authorization; not an OS filesystem sandbox'}
        business = ('仅在本次隔离服务创建一个normal Task及绑定它的完整Bot，使用发布V2，不调用V1或运行目标。Task名称为' + name + '；agent=' + self.host + '，model=probe，thinking=default，skill=null，suite=null，chatName={chat.id}。Task.prompt必须逐字使用：' + PROMPT + '\n'
            'Bot名称为' + name + '，ID为v2-csv-' + self.host + '。三指南按现有职责设计；禁止优化或改写Task.prompt。'
            '授权只读本次文件、当前隔离配置和文档，通过当前服务新建上述Task/Bot；候选全文在各阶段展示后再确认，初始委托不批准未知正文。'
            '只在' + str(delivery) + '新建交付文件，UTF-8，不覆盖；正式交付遵循V2材料契约。原始CSV不写、不上传、不执行业务，不修改既有对象、Skill、共享文档或运行记录。')
        business += '\n' + self.environment_scope
        launched = self.browser('launch', mapTitle='创建task-V2', manager=TASK_IDS[0], zone='v2/owned', business=business)
        root_id = launched['runId']
        runs = self.drive(root_id)
        for identity, chat in self.chats.items():
            history = self.native(chat)
            if history:
                self.histories[identity] = history
        self.evidence['histories'] = self.histories
        root, child = runs
        outer = self.verify_run(root, 'create-task-v2.json')
        inner = self.verify_run(child, 'create-bot-v2.json')
        request = material_json(outer['create_task']['request'][1], ['schema_version', 'kind', 'target', 'requirements', 'constraints', 'non_goals', 'resource_limits', 'authorization', 'confirmation'], ['bot'])
        require(request['schema_version'] == 2 and request['kind'] == 'request' and request['authorization'] == {'write_materials': True, 'create_bot': True, 'optimize_task_prompt': False} and request['confirmation']['scope'] == 'request' and request['confirmation']['evidence'], ('V2 request was rewritten or authorization confused with result approval', request))
        require(type(request['schema_version']) is int and all(type(value) is bool for value in request['authorization'].values())
            and isinstance(request['requirements'], str) and request['requirements'].strip()
            and all(isinstance(request[key], list) and all(isinstance(value, str) for value in request[key]) for key in ['constraints', 'non_goals', 'resource_limits'])
            and set(request['confirmation']) == {'scope', 'evidence'} and isinstance(request['confirmation']['evidence'], str), ('Malformed original V2 request protocol', request))
        require(set(request['target']) == {'type', 'task_id'} and request['target']['type'] == 'task', ('Rewritten V2 target locator', request['target']))
        target = self.http('/api/tasks/' + urllib.parse.quote(request['target']['task_id'], safe=''))['task']
        require(target['name'] == name and target['prompt'] == PROMPT and target['agent'] == self.host, ('Wrong actual V2 target or unauthorized prompt change', target))
        captured = material_json(inner['align_requirements']['target_materials'][1], ['schema_version', 'kind', 'target', 'captured_at', 'references'])
        require(captured['schema_version'] == 2 and captured['kind'] == 'target_materials' and captured['target']['configuration'] == target, captured)
        saved = material_json(inner['design_and_create']['saved'][1], ['schema_version', 'kind', 'bot', 'revision', 'target'])
        handoff = material_json(inner['design_and_create']['handoff'][1], ['schema_version', 'kind', 'stage', 'bot_id', 'target', 'confirmation_evidence', 'sources', 'changes', 'checks', 'unverified'])
        require(saved['schema_version'] == handoff['schema_version'] == 2 and saved['kind'] == 'saved' and handoff['kind'] == 'handoff' and handoff['stage'] == 'saved', ('Replaced V2 output protocol', saved, handoff))
        bot = self.http('/api/scheduler/bots/edit', {'id': 'v2-csv-' + self.host})
        actual_bot = {**bot['content'], 'id': bot['id']}
        require(saved['bot'] == actual_bot and saved['revision'] == bot['revision'] and saved['target']['configuration'] == target, ('Delivered saved is not actual complete readback', saved, bot))
        require(handoff['bot_id'] == actual_bot['id'] and handoff['target'] == request['target'] and handoff['confirmation_evidence'] and handoff['unverified'], handoff)
        require(handoff['checks'] == {'bot_read_back': True, 'target_read_back': True, 'task_prompt_changed': False, 'non_target_unchanged': True, 'semantics_preserved': True}, handoff)
        require(all(actual_bot.get(field, '').strip() for field in ['when', 'plan_guide', 'node_guide']) and actual_bot['task_id'] == target['id'], actual_bot)
        for key in ['saved', 'handoff']:
            require(outer['create_bot_v2'][key] == inner['design_and_create'][key], ('Parent borrowed/replaced child actual delivery', key))
        require(root['run']['id'] != child['run']['id'] and child['run']['parent']['runId'] == root['run']['id'], ('V2 Run identities collapsed', runs))
        self.evidence['finalRuns'] = runs
        self.evidence['actualTarget'] = target
        self.evidence['actualBot'] = bot
        protected_after = {name: digest(self.root / name) if (self.root / name).is_file() else None for name in protected}
        require(protected_after == protected, ('V2 modified an owned existing V1/V2 definition', protected, protected_after))
        for key, expected in self.deployed.items():
            require(self.http('/api/tasks/' + key)['task'] == expected, ('V2 changed an existing published Task', key))
        self.evidence['ownedDefinitionReadback'] = protected_after
        self.evidence['v1Preservation'] = {'before': self.v1_before, 'after': {str(path.relative_to(self.args.repo)): digest(path) for path in (self.args.repo / 'meta-data').rglob('*') if path.is_file()}}
        require(self.evidence['v1Preservation']['before'] == self.evidence['v1Preservation']['after'], 'Original V1/V2 definitions changed')
        self.browser('facts', runs=runs, modes=MODES)
        start = time.monotonic()
        samples = []
        baseline = len(self.backend_wire.records)
        while time.monotonic() - start < 51:
            require([self.detail(detail['run']['id']) for detail in runs] == runs, 'V2 Run/report facts advanced without explicit manager action')
            samples.append(time.monotonic())
            time.sleep(0.25)
        require(not any(record['method'] != 'GET' for record in self.backend_wire.records[baseline:]), 'Unrequested V2 business mutation after delivery')
        self.evidence['quietWindow'] = {'started': start, 'ended': time.monotonic(), 'samples': samples, 'budgets': self.evidence['budgets']}
        self.check_model_identity()
        self.check_guidance()
        self.evidence['coverage'] = {key: {'verified': True, 'evidenceKeys': {
            'P34/V2/S2': ['definitions', 'nativeYes', 'histories', 'finalRuns'],
            'P34/V2/S3': ['browser', 'definitions', 'finalRuns'],
            'P35/V2/S2': ['materials', 'histories', 'modelIdentities', 'modelConsumedPublishedPrompts', 'actualTarget', 'actualBot', 'finalRuns', 'approvedWithoutReadyReport', 'v1Preservation', 'ownedDefinitionReadback', 'quietWindow'],
        }[key]} for key in ITEMS}
        self.evidence['passed'] = True

    def cleanup_extra(self):
        try:
            super().cleanup_extra()
        finally:
            if self.model_wire and self.model_wire.records:
                self.check_model_identity(verify=False)
