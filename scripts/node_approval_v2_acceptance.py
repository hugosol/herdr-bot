"""Ticket 15's approved S2/S3 seams, backed by public HTTP and actual files.

Independent oracle: the four published V2 responsibilities (three manual, one
nested automatic), schema_version=2 business materials and a literal read-only
CSV Task. Violations include a V1 alias, dropped authorization/confirmation,
a forged native Yes, a report standing in for approval, or complete preceding
its latest ready report/declared outputs. No model decision/result is supplied.
"""
import copy
from datetime import datetime
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
from workflow_create_acceptance import prepare_source_resources

MODES = {'create-task-v2.json': {'create_task': False, 'create_bot_v2': True},
    'create-bot-v2.json': {'align_requirements': False, 'design_and_create': False}}
TASK_IDS = ['task-18e2c2827c484128-0', 'task-18e31d30fe69a51c-2',
    'task-18e892cb6456b0d0-0', 'task-18e89651aa593290-1']
QUESTION = '当前会话的成果是否已达到预期，可以交接并推进后续流程？'
PROMPT = '仅在用户提供两份已授权读取的本地CSV文件时，逐行比较并在当前Chat交付差异摘要。只读原文件，不上传、不写文件、不执行其他业务。材料缺失或不可读时说明缺项并等待用户。'
ITEMS = ['P34/V2/S2', 'P34/V2/S3', 'P35/V2/S2']


OUTPUT_IDS = {'create_task': {'request', 'handoff'}, 'create_bot_v2': {'saved', 'handoff'},
    'align_requirements': {'bot_requirements', 'target_materials'}, 'design_and_create': {'saved', 'handoff'}}


def report_paths(reports):
    return {item['id']: item['path'] for report in reports for item in report.get('outputs', [])}


def execution_reports(reports, node, execution_id):
    return [report for report in reports if report['nodeId'] == node and report['executionId'] == execution_id]


def committed_handoff(run, node, receipt, output_ids):
    state = run['nodes'][node]
    snapshot = receipt.get('snapshot', {})
    require(receipt.get('changed') is True and snapshot.get('id') == run['id']
        and snapshot.get('manager') == run['manager'] and snapshot.get('nodes', {}).get(node) == state
        and state['status'] == 'completed' and state['execution']['approved'] is True
        and state['completion']['actor'] == run['manager'], ('Not this manager’s first committed handoff', receipt))
    reports = execution_reports(receipt['reports'], node, state['execution']['id'])
    require(reports and reports[-1]['canComplete'] is True and set(report_paths(reports)) == output_ids,
        ('First complete lacks current ready report or expected delivery', node, reports))
    return reports


def history_before(history, call_id):
    for index, entry in enumerate(history['branch']):
        message = entry.get('message', {})
        if entry.get('type') == 'message' and message.get('role') == 'assistant' and (entry.get('id') == call_id or any(
                part.get('id') == call_id for part in message.get('content', []) if isinstance(part, dict))):
            return {**history, 'branch': history['branch'][:index]}
    raise AssertionError(('Missing native action boundary', history['id'], call_id))


def native_result(history, name, body, **arguments):
    results = {entry['message'].get('toolCallId'): entry['message'] for entry in history['branch']
        if entry.get('type') == 'message' and entry['message'].get('role') == 'toolResult'}
    for call in tool_calls(history):
        if call['name'].rsplit('.', 1)[-1] != name or not all(call.get('arguments', {}).get(key) == value for key, value in arguments.items()):
            continue
        result = results.get(call['id'], {})
        if not result or result.get('isError'):
            continue
        if (result.get('details') or {}).get('body') == body:
            return call['id']
        for part in result.get('content', []):
            if part.get('type') == 'text' and decode({'text': part.get('text', '')}, 'text') == body:
                return call['id']
    return None


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


AUTHORIZATION = {'write_materials': True, 'create_bot': True, 'optimize_task_prompt': False}
REQUEST_FIELDS = {'schema_version', 'kind', 'target', 'requirements', 'constraints', 'non_goals', 'resource_limits', 'authorization', 'confirmation'}
TARGET_FIELDS = {'schema_version', 'kind', 'target', 'captured_at', 'references'}
BOT_FIELDS = {'id', 'name', 'task_id', 'when', 'plan_guide', 'node_guide'}


def string(value):
    return isinstance(value, str) and bool(value.strip())


def strings(value):
    return isinstance(value, list) and all(string(item) for item in value)


def csv_scope(text):
    # Independent business sample; do not require model-authored prose verbatim.
    return string(text) and all(re.search(pattern, text, re.IGNORECASE) for pattern in (
        r'CSV', r'两份|两个|2\s*(?:份|个)|\btwo\b', r'授权|authori[sz]', r'只读|只读取|read.only',
        r'不(?:得|能)?上传|禁止上传|no upload|never upload',
        r'不(?:得|能)?(?:写|修改)|禁止写|never (?:write|modify)', r'比较|对比|compare',
        r'差异|不同|difference', r'缺(?:失|少|项)|missing', r'等待|wait'))


def task_configuration(task):
    return {**{key: value for key, value in task.items() if key not in ('id', 'group')}, 'suite': task.get('suite'),
        **({'group': task['group']} if task.get('group') is not None else {})}


def request_valid(value, task_id):
    if not isinstance(value, dict) or not REQUEST_FIELDS <= value.keys() <= REQUEST_FIELDS | {'bot'}:
        return False
    authorization, confirmation = value['authorization'], value['confirmation']
    bot = value.get('bot', {})
    return (type(value['schema_version']) is int and value['schema_version'] == 2 and value['kind'] == 'request'
        and string(task_id) and value['target'] == {'type': 'task', 'task_id': task_id}
        and isinstance(authorization, dict) and authorization.keys() == AUTHORIZATION.keys()
        and all(authorization[key] is flag for key, flag in AUTHORIZATION.items())
        and isinstance(confirmation, dict) and confirmation.keys() == {'scope', 'evidence'}
        and confirmation['scope'] == 'request' and string(confirmation['evidence'])
        and string(value['requirements']) and all(strings(value[key]) for key in ('constraints', 'non_goals', 'resource_limits'))
        and isinstance(bot, dict) and bot.keys() <= {'id', 'name', 'group'} and all(string(item) for item in bot.values()))


def target_materials_valid(value, target):
    if not isinstance(value, dict) or value.keys() != TARGET_FIELDS or not isinstance(target, dict):
        return False
    try:
        captured = datetime.fromisoformat(value['captured_at'])
    except (TypeError, ValueError):
        return False
    return (type(value['schema_version']) is int and value['schema_version'] == 2 and value['kind'] == 'target_materials'
        and captured.utcoffset() is not None and value['references'] == []
        and value['target'] == {'type': 'task', 'task_id': target['id'], 'configuration': target})


def strict_json(text):
    def object_value(pairs):
        value = {}
        for key, item in pairs:
            require(key not in value, ('Duplicate V2 material field', key))
            value[key] = item
        return value
    def invalid_constant(value):
        raise ValueError('Non-JSON numeric constant: ' + value)
    return json.loads(text, object_pairs_hook=object_value, parse_constant=invalid_constant)


def material_json(text, required, optional=()):
    value = strict_json(text)
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
        valid = (task.keys() <= candidate.keys() <= task.keys() | {'group'} and candidate.get('group') is None
            and all(candidate[key] == value for key, value in task.items()))
    elif action == 'confirm-bot':
        guides = [candidate.get(key) for key in ('when', 'plan_guide', 'node_guide')]
        valid = (BOT_FIELDS <= candidate.keys() <= BOT_FIELDS | {'group'} and candidate.get('group') is None
            and candidate['id'] == expected['botId'] and candidate['name'] == task['name']
            and string(expected['taskId']) and candidate['task_id'] == expected['taskId']
            and all(string(guide) for guide in guides) and csv_scope('\n'.join(guides)))
    elif action == 'confirm-request':
        valid = request_valid(candidate, expected['taskId']) and csv_scope('\n'.join([candidate['requirements'],
            *candidate['constraints'], *candidate['non_goals'], *candidate['resource_limits']]))
    elif action == 'confirm-target-materials':
        valid = target_materials_valid(candidate, expected.get('target'))
    else:
        valid = candidate.keys() == {'requirements'} and csv_scope(candidate['requirements'])
    decision.update(authorized=bool(valid), reason='authorized-current-candidate' if valid else 'candidate-mismatch')
    return decision


def displayed_candidates(text, *, form_blocks=()):
    """Only explicit structured blocks; prose is not a complete configuration."""
    blocks = re.findall(r'```([^\n`]*)\n(.*?)```', text, re.S)
    for block in form_blocks:
        fenced = re.fullmatch(r'```([^\n`]*)\n(.*?)```', block.strip(), re.S)
        blocks.append(fenced.groups() if fenced else ('markdown' if re.match(r'\s*#{1,6}\s', block) else 'json', block))
    result = []
    for language, block in blocks:
        if language.strip().lower() in ['markdown', 'md']:
            result.append(('confirm-requirements', {'requirements': block}))
            continue
        if language.strip().lower() not in ['', 'json']:
            continue
        try:
            value = strict_json(block)
        except (ValueError, AssertionError):
            result.append(('unknown-action', None))
            continue
        if not isinstance(value, dict):
            result.append(('unknown-action', None))
            continue
        if 'taskType' in value:
            result.append(('confirm-task', value))
        elif all(key in value for key in ['when', 'plan_guide', 'node_guide']):
            result.append(('confirm-bot', value))
        elif value.get('kind') in ['request', 'target_materials']:
            result.append(('confirm-request' if value['kind'] == 'request' else 'confirm-target-materials', value))
        else:
            result.append(('unknown-action', None))
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
        self.hidden = False
        self.pre = None
        self.label = None
        self.textarea = None
        self.select = None
        self.option = None
        self.labels = {}
        self.table_stack = []
        self.table_rows = []
        self.table_count = 0
        self.table_valid = True
        self.feed(source)
        self.close()
        task = self.table_task()
        if task is not None:
            self.blocks[:] = [json.dumps(task, ensure_ascii=False)]
        for field in self.fields:
            if not field['label'] and field.get('id') in self.labels:
                field['label'] = self.labels[field['id']]

    def table_task(self):
        if not self.table_valid or self.table_count != 1 or self.table_stack or len(self.blocks) != 1:
            return None
        rows = self.table_rows
        if not rows or rows[0] != ['字段', '候选值'] or any(len(row) != 2 for row in rows):
            return None
        fields = {'name': ('name',), 'chatName': ('chatName',), 'taskType': ('taskType',),
            'agent': ('agent',), 'model': ('model',), 'thinking': ('thinking',), 'skill': ('skill',), 'suite': ('suite',),
            'agent/model/thinking': ('agent', 'model', 'thinking'), 'model/thinking': ('model', 'thinking'),
            'skill/suite': ('skill', 'suite')}
        candidate = {}
        for label, value in rows[1:]:
            keys = fields.get(re.sub(r'\s+', '', label), ())
            values = [part.strip() for part in value.split('/')] if len(keys) > 1 else [value]
            if len(keys) != len(values) or any(key in candidate for key in keys):
                return None
            candidate.update((key, None if key in ('skill', 'suite') and item == 'null' else item)
                for key, item in zip(keys, values))
        if candidate.keys() != {'name', 'chatName', 'taskType', 'agent', 'model', 'thinking', 'skill', 'suite'}:
            return None
        return {**candidate, 'prompt': self.blocks[0]}

    def handle_starttag(self, tag, attributes):
        attributes = dict(attributes)
        if (tag in ('template', 'script', 'style', 'noscript') or {'hidden', 'style'} & attributes.keys()
                or (attributes.get('aria-hidden') or '').lower() == 'true'):
            self.table_valid = False
        if self.pre is not None:
            self.table_valid = False
        if tag == 'table':
            self.table_count += 1
            self.table_valid &= not self.table_stack and not self.blocks and self.pre is None
            self.table_stack.append(tag)
        elif self.table_stack:
            parent = self.table_stack[-1]
            allowed = {'table': ('thead', 'tbody', 'tr'), 'thead': ('tr',), 'tbody': ('tr',), 'tr': ('th', 'td')}
            self.table_valid &= not attributes and tag in allowed.get(parent, ())
            self.table_stack.append(tag)
            if self.table_valid and tag == 'tr':
                self.table_rows.append([])
            if self.table_valid and tag in ('th', 'td'):
                self.table_rows[-1].append('')
        if tag == 'pre':
            self.pre = ''
        if tag == 'label':
            self.label = {'text': '', 'fields': [], 'for': attributes.get('for')}
        if tag == 'input' and attributes.get('type') == 'hidden':
            self.hidden = True
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
        if self.table_valid and self.table_stack:
            if self.table_stack[-1] in ('th', 'td'):
                self.table_rows[-1][-1] += data
            elif data.strip():
                self.table_valid = False
        if self.pre is not None:
            self.pre += data
        if self.label is not None:
            self.label['text'] += data
        if self.textarea is not None:
            self.textarea['value'] += data
        if self.option is not None:
            self.option['label'] += data

    def handle_endtag(self, tag):
        if self.table_stack:
            if self.table_stack[-1] != tag:
                self.table_valid = False
            else:
                self.table_stack.pop()
                if self.table_valid and tag in ('th', 'td'):
                    self.table_rows[-1][-1] = self.table_rows[-1][-1].strip()
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
            self.yes[request['id']] = {'request': request, 'selectedScreen': screen, 'response': result, 'publicBefore': before, 'executionId': before['run']['nodes'][node]['execution']['id']}
            self.evidence['nativeYes'] = self.yes


    def authorization(self, chat, text, *, form_blocks=()):
        stage = self.stages.get(chat['sessionId'])
        expected_task = {'name': self.target_name, 'chatName': '{chat.id}', 'taskType': 'normal',
            'agent': self.host, 'model': 'probe', 'thinking': 'default', 'skill': None, 'suite': None, 'prompt': PROMPT}
        targets = [task for task in self.http('/api/tasks')['tasks']
            if task_configuration(task) == expected_task]
        target = targets[0] if len(targets) == 1 else None
        expected = {'task': expected_task, 'taskId': target['id'] if target else None, 'target': target,
            'botId': 'v2-csv-' + self.host}
        candidates = displayed_candidates(text, form_blocks=form_blocks)
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
        if decision.get('stage') == 'create_task' and not decision['authorized']:
            reply += '请每次仅展示一个待授权动作：创建Task时用一个JSON块展示完整配置和逐字prompt；不要把创建Task与写入材料合并确认。'
        return reply + scope + '已给定CSV只读、不上传、不写原文件及Task.prompt全文约束不变。新材料只能在既有授权目录以新文件保存，不覆盖。' + self.environment_scope

    def form_plan(self, form, decision):
        subjects = {'confirm-task': r'Task(?:完整)?(?:候选|配置)?', 'confirm-request': r'(?:V2)?(?:委托|request)(?:材料)?',
            'confirm-requirements': r'(?:统一|Bot)?需求(?:候选)?', 'confirm-target-materials': r'(?:完整)?目标材料',
            'confirm-bot': r'(?:Bot(?:完整)?(?:候选|配置)?|完整候选)'}
        subject = subjects[decision['action']]
        question_pattern = rf'(?:是否)?(?:确认|同意|授权)(?:保存|新建|创建|使用)?(?:本次|上述|当前)?{subject}[？?。]?'
        def confirms(label):
            return bool(re.fullmatch(question_pattern, re.sub(r'\s+', '', label), re.IGNORECASE))
        def yes(label, question):
            label = re.sub(r'^\s*(?:是|Yes)\s*[：:、，]\s*', '', label, flags=re.IGNORECASE)
            return (confirms(label) or bool(re.fullmatch(r'(?:是|Yes|确认|同意)', label.strip(), re.IGNORECASE) and confirms(question))
                or bool(decision['action'] == 'confirm-bot' and re.fullmatch(r'保存(?:本次|当前)?完整候选', label.strip()))
                or bool(decision['action'] == 'confirm-task' and re.fullmatch(
                    r'仅(?:创建|新建)并完整读回(?:本次|上述|当前)(?:唯一)?(?:新)?Task[。]?', re.sub(r'\s+', '', label), re.IGNORECASE)))
        require(form.fields and not form.hidden, 'Native Ask has no observable controls or includes hidden fields')
        groups = {}
        for field in form.fields:
            name = field.get('name')
            require(string(name) and not any(key in field for key in ('disabled', 'readonly')), ('Unknown business control', field))
            group = groups.setdefault(name, [])
            require(not group or (group[-1].get('type') == field.get('type') == 'radio'
                and list(groups)[-1] == name), ('Duplicate or interleaved native business control', field))
            group.append(field)
        facts = {'agent': (self.host, r'agent|宿主'), 'model': ('probe', r'model|模型'),
            'thinking': ('default', r'thinking|思考(?:级别)?'), 'prompt': (PROMPT, r'(?:Task\.)?prompt'),
            'name': (self.target_name, r'name|(?:Task|Bot)?名称'), 'bot_id': ('v2-csv-' + self.host, r'bot_id|BotID'),
            'path': (str(self.delivery_directory), r'path|(?:材料|交付)目录')}
        plan = []
        confirmations = 0
        for name, group in groups.items():
            field = group[0]
            kind = field.get('type') or ('textarea' if field['tag'] == 'textarea' else 'text')
            question = field['label'] or name
            fixed = facts.get(name)
            if fixed:
                require(re.fullmatch(fixed[1], re.sub(r'\s+', '', question), re.IGNORECASE), ('Ambiguous fact field', field))
            else:
                confirmations += 1
            if kind in ('radio', 'select'):
                options = group if kind == 'radio' else field['options']
                choices = [index for index, option in enumerate(options) if (fixed and option.get('value') == fixed[0])
                    or (not fixed and yes(option['label'], question))]
                require(len(choices) == 1, ('No unique authorized business option', group))
                chosen = choices[0]
                default = next((i for i, option in enumerate(options) if 'selected' in option), 0)
                option = options[chosen]
                plan.append((name, kind, option.get('value', 'on' if kind == 'radio' else option['label']), option['label'], chosen, default, 'checked' in option))
            elif kind == 'checkbox':
                require(not fixed and confirms(question), ('Unknown checkbox decision', field))
                plan.append((name, kind, True, question, 0, 0, 'checked' in field))
            else:
                require(kind in ('text', 'textarea') and (fixed or confirms(question)), ('Unrecognized business field', field))
                value = fixed[0] if fixed else self.scope_message(decision)
                require(field.get('value', '') in ('', value), ('Conflicting prefilled business field', field))
                plan.append((name, kind, value, question, 0, 0, bool(field.get('value'))))
        require(confirmations == 1, ('An Ask must present one unambiguous business confirmation', form.fields))
        return plan

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
            if (value.get('sessionId') == chat['sessionId'] and value.get('html') == html
                    and not path.with_name(value['id'] + '.response.json').exists()):
                observed.append(value)
        if not observed:
            return True
        require(len(observed) == 1 and observed[0].get('hasUI') is True, ('Ambiguous native business form invocation', observed))
        response_path = self.confirmations / 'forms' / (observed[0]['id'] + '.response.json')
        require(not response_path.exists(), 'Business form already answered before this explicit user input')
        form = BusinessForm(html)
        decision = self.authorization(chat, '', form_blocks=form.blocks)
        plan = None
        if decision['authorized']:
            try:
                plan = self.form_plan(form, decision)
            except AssertionError as error:
                decision = {**decision, 'authorized': False, 'reason': str(error)}
        if plan is None:
            self.herdr('agent', 'send-keys', chat['paneId'], 'esc')
            self.answered.add(call['id'])
            returned = until('actual native form rejection return', lambda: json.loads(response_path.read_text(encoding='utf-8')) if response_path.is_file() else None, 20)
            self.evidence.setdefault('businessFormRefusals', []).append({'call': call, 'nativeInvocation': observed[0],
                'nativeReturn': returned, 'decision': decision, 'selectedAction': 'Esc; no authorization'})
            self.pending_scope[chat['sessionId']] = {'toolCallId': call['id'], 'message': self.scope_message(decision)}
            return True
        record = {'sessionId': chat['sessionId'], 'call': call, 'beforeScreen': screen, 'choices': [], 'at': time.monotonic()}
        self.evidence.setdefault('businessForms', []).append(record)
        for name, kind, value, label, chosen, default, prefilled in plan:
            if kind == 'radio':
                for _ in range(chosen):
                    self.herdr('agent', 'send-keys', chat['paneId'], 'down')
                if not prefilled:
                    self.herdr('agent', 'send-keys', chat['paneId'], 'space')
                selected = until('native business radio value visible', lambda: text if label in (text := self.screen(chat))
                    and any(mark in text for mark in ['●', '◉', '[x]', '[✓]', '☑']) else None, 5)
            elif kind == 'select':
                for _ in range(abs(chosen - default)):
                    self.herdr('agent', 'send-keys', chat['paneId'], 'right' if chosen > default else 'left')
                selected = self.screen(chat)
                require(label.strip() in selected, ('Selected dropdown value is not visible', selected, label))
            else:
                if not prefilled:
                    if kind == 'checkbox':
                        self.herdr('agent', 'send-keys', chat['paneId'], 'space')
                    else:
                        self.herdr('pane', 'send-text', chat['paneId'], value)
                selected = self.screen(chat)
            record['choices'].append({'name': name, 'value': value, 'selectedScreen': selected})
            self.herdr('agent', 'send-keys', chat['paneId'], 'tab')
        record['beforeSubmit'] = self.screen(chat)
        require('Submit' in record['beforeSubmit'], 'Native business form Submit not visible')
        submitted_at = time.monotonic()
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
        answers = {choice['name']: choice['value'] for choice in record['choices']}
        require(data == answers, ('Native form returned different business choices', answers, data))
        normalized = (result.get('details') or {}).get('answers')
        require(isinstance(normalized, list) and len(normalized) == len(plan)
            and all(isinstance(item, dict) and string(item.get('name')) and 'answer' in item for item in normalized),
            ('Missing native Ask tool answer evidence', result))
        by_name = {item['name']: item['answer'] for item in normalized}
        require(by_name.keys() == answers.keys(), ('Ask tool returned different fields', by_name))
        for name, kind, value, label, *_ in plan:
            accepted = (value, label) if kind in ('radio', 'select') else (value,)
            if kind == 'checkbox':
                accepted = (True, 'true', 'Yes', '是', label)
            require(by_name[name] in accepted, ('Ask tool changed the authorized choice', name, by_name[name]))
        record.update(result=result, nativeInvocation=observed[0], nativeReturn=raw_return)
        self.record_grant(chat, decision, call['id'], submitted_at)
        return True

    def record_grant(self, chat, decision, boundary, at):
        self.evidence.setdefault('businessGrants', []).append({**decision, 'sessionId': chat['sessionId'],
            'boundary': boundary, 'at': at})

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
        if not re.search(r'(?:^|[。！？\n])\s*(?:[-*]\s*)?(?:\*\*)?(?:是否(?:同意|确认|允许|授权|可以)|(?:请|麻烦)(?:你|您|用户)?(?:确认|授权|提供|选择)|(?:你|您)是否)', text):
            return
        # Only the candidate policy may grant authority. An explicit but
        # incomplete or unknown request receives a scoped refusal, not a grant.
        self.answered.add(last['id'])
        decision = self.authorization(chat, text)
        self.evidence.setdefault('businessCandidateConfirmations', []).append({'sessionId': chat['sessionId'], 'candidate': last, 'decision': decision, 'at': time.monotonic()})
        at = time.monotonic()
        self.send(chat, self.scope_message(decision), 'actual-current-stage-candidate-authorized' if decision['authorized'] else 'business-scope-not-authorized')
        if decision['authorized']:
            self.record_grant(chat, decision, last['id'], at)

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
            self.evidence['lastPublicRuns'] = final
            require(not any(node['status'] == 'failed' for detail in runs for node in detail['run']['nodes'].values()), ('Actual V2 node failed; preserve the manager decision, do not retry or wait for a success', runs))
            for identity in active:
                self.business_reply(self.chats[identity])
            if len(runs) == 2 and all(detail['run']['status'] == 'completed' for detail in runs):
                return runs
            require(all(detail['run']['status'] not in ['failed', 'cancelled'] for detail in runs), ('Actual V2 run failed; no retry', runs))
            time.sleep(0.25)
        raise AssertionError('Actual V2 model business did not finish; preserve native histories/screens, no retry')

    def material(self, path):
        path = Path(path)
        require(path.is_absolute() and path.is_relative_to(self.delivery_directory)
            and path.resolve().is_relative_to(self.delivery_directory.resolve()) and path.is_file(),
            ('Material escaped the authorized delivery directory or is missing', str(path)))
        current = self.root
        for part in path.relative_to(self.root).parts:
            current /= part
            require(not current.is_symlink() and not (current.stat().st_file_attributes & 0x400), ('Reparse material path', str(current)))
        raw = path.read_bytes()
        text = raw.decode('utf-8')
        self.evidence.setdefault('materials', {})[str(path)] = {'text': text, 'sha256': hashlib.sha256(raw).hexdigest()}
        return text

    def prove_read(self, history, path, sections=()):
        expected = Path(path)
        selected = not sections
        remaining = set()
        headings = set()
        for line in expected.read_text(encoding='utf-8').splitlines():
            if sections and line.startswith('## '):
                selected = line[3:] in sections
                if selected:
                    headings.add(line[3:])
            if selected and line.strip():
                remaining.add(line)
        require(remaining and headings == set(sections), ('Missing required V2 material or contract section', path, sections))
        calls = []
        cwd = Path(self.evidence['nativeHeaders'][history['id']]['cwd'])
        results = {message.get('toolCallId'): message for message in self.messages(history) if message.get('role') == 'toolResult'}
        for call in tool_calls(history):
            result = results.get(call['id'])
            if not result or result.get('isError'):
                continue
            args = call.get('arguments', {})
            tool = call['name'].rsplit('.', 1)[-1]
            if tool in ('read', 'read_file'):
                value = args.get('path', args.get('file_path', ''))
                if not isinstance(value, str):
                    continue
                actual = cwd / re.sub(r':\d+(?:-\d+)?$', '', value)
                if actual != expected:
                    continue
            elif tool == 'bash':
                command = args.get('command', '')
                if not isinstance(command, str) or not re.search(r'\b(?:cat|Get-Content|type)\s', command, re.IGNORECASE):
                    continue
                path_pattern = r"""["'\s=]""" + re.escape(expected.as_posix()) + r"""(?=["'\s;|&]|$)"""
                if not re.search(path_pattern, command.replace('\\', '/'), re.IGNORECASE):
                    continue
            else:
                continue
            text = '\n'.join(part.get('text', '') for part in result.get('content', []) if part.get('type') == 'text')
            found = {line for line in remaining if line in text}
            if found:
                calls.append(call['id'])
                remaining.difference_update(found)
                if not remaining:
                    self.evidence.setdefault('materialReads', []).append({'sessionId': history['id'], 'path': str(expected),
                        'sections': list(sections), 'callIds': calls})
                    return
        raise AssertionError(('No successful material read before the V2 action', history['id'], str(expected), sections))

    def matching_grant(self, run, node, action, matches, *, required=True):
        session = run['nodes'][node]['execution']['chat']['sessionId']
        grants = [grant for grant in self.evidence.get('businessGrants', [])
            if grant['sessionId'] == session and grant['stage'] == node and grant['action'] == action]
        matching = [grant for grant in grants if matches(grant['candidate'])]
        require(matching or (not required and not grants), ('Final business effect does not match the displayed grant', node, action, grants))
        return matching[-1] if matching else None

    def verify_business(self, root, child, outer, inner):
        request = material_json(outer['create_task']['request'][1], REQUEST_FIELDS, {'bot'})
        locator = request.get('target')
        target_id = locator.get('task_id') if isinstance(locator, dict) else None
        require(request_valid(request, target_id), ('Invalid original V2 request', request))
        require(csv_scope('\n'.join([request['requirements'], *request['constraints'], *request['non_goals'], *request['resource_limits']])),
            ('V2 request lost the independent CSV business intent', request))
        target_route = '/api/tasks/' + urllib.parse.quote(target_id, safe='')
        target = self.http(target_route)['task']
        bot_id = 'v2-csv-' + self.host
        require(all(value == {'name': self.target_name, 'id': bot_id}.get(key) for key, value in request.get('bot', {}).items()),
            ('Wrong named Bot in the V2 request', request))
        files = [path for outputs in [outer['create_task'], *inner.values()] for path, _ in outputs.values()]
        require(len(set(map(Path, files))) == len(files), ('Independent V2 outputs reused a file', files))
        stage_handoff = outer['create_task']['handoff'][1]
        requirements = inner['align_requirements']['bot_requirements'][1]
        require(csv_scope(stage_handoff) and target_id in stage_handoff and self.target_name in stage_handoff
            and re.search(r'授权|确认', stage_handoff) and re.search(r'未验证|未运行|未执行', stage_handoff),
            ('Incomplete Task stage Markdown handoff', stage_handoff))
        require(csv_scope(requirements) and target_id in requirements and re.search(r'授权|确认', requirements),
            ('Incomplete aligned Markdown requirements', requirements))
        captured = material_json(inner['align_requirements']['target_materials'][1], TARGET_FIELDS)
        require(target_materials_valid(captured, target), ('V2 W1 did not capture this complete current Task', captured, target))
        saved = material_json(inner['design_and_create']['saved'][1], ['schema_version', 'kind', 'bot', 'revision', 'target'])
        handoff = material_json(inner['design_and_create']['handoff'][1], ['schema_version', 'kind', 'stage', 'bot_id',
            'target', 'confirmation_evidence', 'sources', 'changes', 'checks', 'unverified'])
        require(type(saved['schema_version']) is type(handoff['schema_version']) is int
            and saved['schema_version'] == handoff['schema_version'] == 2 and saved['kind'] == 'saved'
            and handoff['kind'] == 'handoff' and handoff['stage'] == 'saved', ('Replaced V2 output protocol', saved, handoff))
        bot = self.http('/api/scheduler/bots/edit', {'id': bot_id})
        actual_bot = {**bot['content'], 'id': bot['id']}
        require(saved['bot'] == actual_bot and saved['revision'] == bot['revision'] and saved['revision'] is not None
            and saved['target'] == {**locator, 'configuration': target}, ('Saved material is not the complete current Bot and Task', saved, bot))
        require(handoff['bot_id'] == bot_id and handoff['target'] == locator
            and all(strings(handoff[key]) and handoff[key] for key in ('confirmation_evidence', 'changes', 'unverified')),
            ('Invalid V2 final handoff identity or evidence', handoff))
        expected = {('bot_requirements', 'input', 'requirements'), ('target_materials', 'input', 'target_materials'),
            ('saved', 'output', 'saved'), ('handoff', 'output', 'handoff')}
        sources = handoff['sources']
        require(isinstance(sources, list) and len(sources) == len(expected)
            and all(isinstance(item, dict) and item.keys() == {'binding_id', 'direction', 'role'}
                and all(string(value) for value in item.values()) for item in sources)
            and {(item['binding_id'], item['direction'], item['role']) for item in sources} == expected,
            ('V2 final handoff must name the actual W2 input and output bindings', sources))
        checks = {'bot_read_back': True, 'target_read_back': True, 'task_prompt_changed': False,
            'non_target_unchanged': True, 'semantics_preserved': True}
        require(isinstance(handoff['checks'], dict) and handoff['checks'].keys() == checks.keys()
            and all(handoff['checks'][key] is value for key, value in checks.items()), ('Invalid V2 final checks', handoff))
        for key in ('saved', 'handoff'):
            require(outer['create_bot_v2'][key] == inner['design_and_create'][key], ('Parent did not relay the actual W2 file', key))
        task_grant = self.matching_grant(root, 'create_task', 'confirm-task',
            lambda candidate: task_configuration(candidate) == task_configuration(target))
        bot_grant = self.matching_grant(child, 'design_and_create', 'confirm-bot',
            lambda candidate: {key: value for key, value in candidate.items() if value is not None} ==
                {key: value for key, value in actual_bot.items() if value is not None})
        intent_grant = self.matching_grant(child, 'align_requirements', 'confirm-requirements', lambda candidate: csv_scope(candidate['requirements']))
        self.matching_grant(root, 'create_task', 'confirm-request', lambda candidate: candidate == request, required=False)
        self.matching_grant(child, 'align_requirements', 'confirm-target-materials', lambda candidate: candidate == captured, required=False)
        require(target['id'] == target_id and target['prompt'] == PROMPT and target['agent'] == self.host,
            ('Wrong V2 target configuration', target))
        task_saves = [wire for wire in self.records('/api/tasks') if wire['method'] == 'POST' and wire.get('status') == 200]
        bot_saves = [wire for wire in self.records('/api/scheduler/bots/save') if wire['method'] == 'POST' and wire.get('status') == 200]
        require(len(task_saves) == 1 and task_grant['at'] <= task_saves[0]['started']
            and task_configuration(decode(task_saves[0], 'request')) == task_configuration(target)
            and target in decode(task_saves[0], 'response')['tasks'], ('No actual authorized Task creation', task_saves))
        require(len(bot_saves) == 1 and bot_grant['at'] <= bot_saves[0]['started'], ('No actual authorized Bot creation', bot_saves))
        bot_write = decode(bot_saves[0], 'request')
        written_bot = strict_json(bot_write['content'])
        require(bot_write['id'] == bot_id and bot_write.get('revision') is None and isinstance(written_bot, dict)
            and 'id' not in written_bot and {**{key: value for key, value in written_bot.items() if value is not None}, 'id': bot_id} == actual_bot
            and decode(bot_saves[0], 'response') == actual_bot, ('Bot save did not match full confirmed readback', bot_write))
        require(not any(wire['method'] == 'PUT' and wire.get('status') == 200 and wire['path'].startswith('/api/tasks/')
            for wire in self.backend_wire.records), 'The V2 no-prompt-change case modified a Task')
        require({item['id'] for item in self.http('/api/tasks')['tasks']} == {*TASK_IDS, target_id}, 'V2 created or removed an unrelated Task')
        self.evidence['businessEffects'] = {'taskSave': task_saves[0], 'botSave': bot_saves[0]}
        self.evidence['actualTarget'] = target
        self.evidence['actualBot'] = bot
        return {'align_requirements': intent_grant, 'design_and_create': bot_grant}

    def verify_bindings(self, root, child, outer, inner, contract, grants):
        request = outer['create_task']['request'][0]
        require(root['fileInputs'] == [] and child['fileInputs'] == [{'id': 'request', 'path': request}],
            ('Wrong Run input binding or unexpected optional materials', root['fileInputs'], child['fileInputs']))
        bindings = {'create_task': {'material_contract': str(contract)}, 'create_bot_v2': {'request': request},
            'align_requirements': {'material_contract': str(contract), 'request': request},
            'design_and_create': {'material_contract': str(contract),
                **{key: pair[0] for key, pair in inner['align_requirements'].items()}}}
        general = '通用格式与目标定位'
        request_section = 'request 与 materials'
        contract_sections = {'create_task': (general, request_section),
            'align_requirements': (general, request_section, 'target_materials'),
            'design_and_create': (general, 'target_materials', 'saved 与 handoff')}
        for run, deliveries in ((root, outer), (child, inner)):
            evidence = self.evidence['handoffs'][run['id']]
            manager = self.histories[run['manager']['sessionId']]
            for node, state in run['nodes'].items():
                execution = state['execution']
                items = execution['inputs']
                require(isinstance(items, list) and len(items) == len(bindings[node])
                    and {item['id']: Path(item['path']) for item in items} == {key: Path(path) for key, path in bindings[node].items()},
                    ('Wrong frozen V2 input bindings', node, items, bindings[node]))
                handoff = evidence['nodes'][node]
                before_complete = history_before(manager, handoff['complete']['callId'])
                for path, _ in deliveries[node].values():
                    self.prove_read(before_complete, path)
                if node != 'create_bot_v2':
                    before_approval = history_before(self.histories[handoff['sessionId']], handoff['approval']['callId'])
                    self.prove_read(before_approval, contract, contract_sections[node])
                    before_candidate = (history_before(self.histories[handoff['sessionId']], grants[node]['boundary'])
                        if node in grants else before_approval)
                    for key, path in bindings[node].items():
                        if key != 'material_contract':
                            self.prove_read(before_candidate, path)
        child_manager = self.histories[child['manager']['sessionId']]
        dispatch = next((call['id'] for call in tool_calls(child_manager) if call['name'].rsplit('.', 1)[-1] == 'run'
            and call.get('arguments', {}).get('action') == 'dispatch' and 'align_requirements' in call['arguments'].get('nodes', [])), None)
        require(dispatch, 'Child manager did not natively dispatch alignment')
        before_dispatch = history_before(child_manager, dispatch)
        self.prove_read(before_dispatch, contract, (general, request_section))
        self.prove_read(before_dispatch, request)
        self.evidence['materialBindings'] = bindings

    def committed_records(self, action, manager):
        return [(record, body) for record in self.records('/api/workflow/manager/' + action, manager)
            if record.get('status') == 200 and (body := decode(record, 'response')) and body.get('changed') is True]

    def verify_run(self, detail, map_file, confirmations):
        run = detail['run']
        require(run['map'] == map_file and set(run['nodes']) == set(MODES[map_file]), ('Replaced V2 topology', detail))
        manager = run['manager']['sessionId']
        finishes = self.committed_records('finish', manager)
        require(len(finishes) == 1 and run['status'] == 'completed' and run['finished']['actor'] == run['manager']
            and finishes[0][1]['snapshot'] == run, ('No explicit finish of this Run', finishes))
        def seen_finish():
            history = self.native(run['manager'])
            if history:
                self.histories[manager] = history
                return native_result(history, 'run', finishes[0][1], action='finish')
        finish_call = until('native manager finish result', seen_finish, 10)
        evidence = {'finish': {'callId': finish_call, 'wire': finishes[0][0]}, 'nodes': {}}
        self.evidence.setdefault('handoffs', {})[run['id']] = evidence
        delivered = {}
        for node, automatic in MODES[map_file].items():
            state = run['nodes'][node]
            execution = state['execution']
            execution_id = execution['id']
            source = 'auto' if automatic else 'manual'
            require(state['status'] == 'completed' and execution['approved'] is True and execution['autoApproved'] is automatic
                and execution['approvalSource'] == source, ('Approval responsibility/source mismatch', state))
            session = execution['chat']['sessionId']
            approvals = self.records('/api/workflow/node/approve', session)
            require(len(approvals) == 1 and approvals[0].get('status') == 200
                and decode(approvals[0], 'request') == {'sessionId': session, 'executionId': execution_id, 'source': source}, approvals)
            approval = decode(approvals[0], 'response')
            require(approval == {'runId': run['id'], 'nodeId': node, 'executionId': execution_id, 'autoApproved': automatic,
                'approved': True, 'approvalSource': source, 'changed': True}, ('Wrong native approval receipt', approval))
            approval_call = native_result(self.histories[session], 'approve', approval)
            require(approval_call, ('Approval receipt missing in native executor history', session))
            raw = [item for item in confirmations if item['sessionId'] == session and item.get('title') == '成果批准']
            ui = [self.yes[item['id']] for item in raw if item['id'] in self.yes]
            require(len(raw) == len(ui) == (0 if automatic else 1) and (automatic or
                (ui[0]['executionId'] == execution_id and ui[0]['publicBefore']['run']['id'] == run['id'])),
                ('Wrong native confirmation for this execution', node, raw, ui))
            completes = [(wire, receipt) for wire, receipt in self.committed_records('complete', manager)
                if decode(wire, 'request').get('nodeId') == node]
            require(len(completes) == 1, ('Expected exactly one native complete commit', node, completes))
            complete_wire, receipt = completes[0]
            reports = committed_handoff(run, node, receipt, OUTPUT_IDS[node])
            complete_call = native_result(self.histories[manager], 'run', receipt, action='complete', nodeId=node,
                reason=state['completion']['reason'])
            require(complete_call and native_result(history_before(self.histories[manager], finish_call),
                'run', receipt, action='complete', nodeId=node),
                ('Missing native complete result before manager finish', node))
            ready = reports[-1]
            require(ready['from'] == execution['chat'] and ready['to'] == run['manager'], ('Wrong report actors', ready))
            def saved_receipt():
                return next((record for record in self.records('/api/workflow/node/report', session)
                    if record.get('status') == 200 and (body := decode(record, 'response'))
                    and body.get('saved') is True and body.get('report') == ready), None)
            reporting = until('saved native report receipt', saved_receipt, 30)
            report_call = next((call['id'] for call in tool_calls(self.histories[session])
                if call['name'].rsplit('.', 1)[-1] == 'report' and call.get('arguments', {}).get('can_complete') is True
                and call['arguments'].get('body') == ready['body']
                and call['arguments'].get('outputs', []) == ready.get('outputs', [])), None)
            require(report_call and native_result(history_before(self.histories[session], report_call), 'approve', approval),
                ('No native approval result before the saved handoff report', ready))
            paths = report_paths(reports)
            final_reports = execution_reports(detail['reports'], node, execution_id)
            require(final_reports[:len(reports)] == reports and final_reports[-1]['canComplete'] is True
                and report_paths(final_reports) == paths, ('Final handoff differs from the committed one', node, final_reports))
            require({item['id'] for item in execution['outputRule']} == OUTPUT_IDS[node], ('Wrong frozen output contract', node))
            delivered[node] = {key: (path, self.material(path)) for key, path in paths.items()}
            evidence['nodes'][node] = {'executionId': execution_id, 'sessionId': session, 'confirmations': raw,
                'approval': {'callId': approval_call, 'wire': approvals[0]}, 'report': {'callId': report_call, 'wire': reporting},
                'complete': {'callId': complete_call, 'wire': complete_wire}, 'reportIds': [report['id'] for report in reports]}
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

    def verify_session_hosts(self, chats):
        identities = {chat['sessionId'] for chat in chats}
        require(len(chats) == len(identities) == 5, ('Mixed V2 manager/worker sessions', chats))
        entry = Path(self.evidence['installedHosts'][self.host]['entry']).resolve()
        hosts = {process['ProcessId']: process for process in self.evidence['nativeProcesses']
            if any(Path(argument).resolve() == entry for argument in process['argv']
                if argument.lower().endswith(('.js', '.ts')))}
        bindings = {}
        with (self.confirmations / 'native-events.jsonl').open(encoding='utf-8') as stream:
            for line in stream:
                event = json.loads(line)
                session = event.get('sessionId')
                if event.get('type') != 'session_start' or session not in identities:
                    continue
                pid = event.get('pid')
                require(event.get('hasUI') is True and pid in hosts
                    and event == bindings.setdefault(session, event), ('Unverified native V2 host session', event))
        require(set(bindings) == identities and all(self.histories.get(session, {}).get('id') == session for session in identities),
            ('Missing native V2 session identity', sorted(identities - bindings.keys())))
        self.evidence['nativeSessionHosts'] = {session: {'host': self.host, 'entry': str(entry),
            'sessionStart': event, 'process': hosts[event['pid']]} for session, event in bindings.items()}

    def verify_mcp_call(self):
        def projects(value):
            if not isinstance(value, dict) or value.get('isError') or 'error' in value:
                return None
            if isinstance(value.get('projects'), list):
                return value
            return projects(value.get('structuredContent'))

        for identity, history in self.histories.items():
            calls = {call['id']: call for call in tool_calls(history)}
            for result in self.messages(history):
                if result.get('role') != 'toolResult' or result.get('isError'):
                    continue
                call = calls.get(result.get('toolCallId'), {})
                name = call.get('name', '').replace('-', '_').lower()
                code = call.get('arguments', {}).get('code', '')
                direct = 'codebase_memory_mcp' in name and name.endswith('list_projects')
                wrapped = name.rsplit('.', 1)[-1] == 'codemode' and isinstance(code, str) and re.search(
                    r'\btools\.mcp__codebase_memory_mcp__list_projects\s*\(', code)
                if not direct and not wrapped:
                    continue
                values = [result.get('details')]
                for part in result.get('content', []):
                    if isinstance(part, dict) and part.get('type') == 'text':
                        for line in part.get('text', '').splitlines():
                            try:
                                values.append(json.loads(line))
                            except ValueError:
                                continue
                for value in values:
                    payload = projects(value)
                    if payload is not None:
                        self.evidence['nativeMcpCall'] = {'sessionId': identity, 'call': call,
                            'result': result, 'projects': payload['projects']}
                        return
        raise AssertionError('No successful native codebase-memory-mcp list_projects call; an empty list is valid')

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
            + '；另外只读本次 Map 显式绑定的格式契约文件 ' + (contract / 'create-bot-v2-materials.md').as_posix()
            + '（不授权读取所在目录的其他文件）。唯一可写业务材料目录为' + delivery.as_posix() + '，唯一可写临时目录为' + (self.root / 'temp').as_posix()
            + '；禁止共享Temp、/tmp或其他目录中的发现式查找和写入，不读取其他Chat/Run或认证；不安装、刷新凭据或修改Skill/文档。'
            + '当前服务由本次配置提供；不得因猜出的相对路径失败而扫描其他目录。'
            + '新建Task的thinking=default是合法的“沿用Agent默认值”；GET /api/tasks的thinking列表只列显式级别，即使只显示off也不要改参数。'
            + '本Run只需一次真实只读MCP调用证明连通：调用codebase-memory-mcp的list_projects，正常返回空列表也有效。'
            + '若经codemode调用，请直接输出工具原始JSON结果（不要复述或重组）。不需要建索引，不调用index_repository，不扩大allowed-root。'
            + '若当前是manager，请把上述环境事实传给create_task执行者。')
        self.evidence['filesystemAuthority'] = {'readOnlyRepository': self.args.repo.as_posix(),
            'readOnlySkills': [self.args.wayfinder.as_posix(), self.args.wayfinder_skills.as_posix()],
            'readOnlyMaterialContract': (contract / 'create-bot-v2-materials.md').as_posix(), 'writableMaterials': delivery.as_posix(), 'writableTemporary': (self.root / 'temp').as_posix(),
            'temporaryEnvironment': {key: self.env[key] for key in ['TMP', 'TEMP', 'TMPDIR']},
            'scope': 'Explicit owned configuration and business authorization; not an OS filesystem sandbox'}
        business = ('仅在本次隔离服务创建一个normal Task及绑定它的完整Bot，使用发布V2，不调用V1或运行目标。Task名称为' + name + '；agent=' + self.host + '，model=probe，thinking=default，skill=null，suite=null，chatName={chat.id}。Task.prompt必须逐字使用：' + PROMPT + '\n'
            'Bot名称为' + name + '，ID为v2-csv-' + self.host + '。三指南按现有职责设计；禁止优化或改写Task.prompt。'
            '授权只读本次文件、当前隔离配置和文档，通过当前服务新建上述Task/Bot；候选全文在各阶段展示后再确认，初始委托不批准未知正文。'
            '只在' + str(delivery) + '新建交付文件，UTF-8，不覆盖；正式交付遵循V2材料契约。原始CSV不写、不上传、不执行业务，不修改既有对象、Skill、共享文档或运行记录。'
            '请把以下要求传给create_task：用单独一个Ask或文本问题确认创建Task；展示单个完整配置JSON（含逐字prompt）或完整字段表和独立prompt。'
            '不要把创建Task与多项建议合并成一个“是”。建好并读回Task后，仍由create_task用真实Task ID准备、写入并读回本节点的request和handoff，按契约报告两项输出；不能把这两份材料交给Bot阶段生成。')
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
        parent_run, child_run = root['run'], child['run']
        parent_execution = parent_run['nodes']['create_bot_v2']['execution']
        require(parent_run.get('parent') is None and parent_run['id'] == root_id and root_id != child_run['id']
            and parent_execution['childRunId'] == child_run['id'] and parent_execution['chat'] == child_run['manager']
            and child_run['parent'] == {'runId': root_id, 'nodeId': 'create_bot_v2', 'executionId': parent_execution['id']},
            ('Wrong V2 parent / child association', parent_run, child_run))
        chats = [parent_run['manager'], child_run['manager'], parent_run['nodes']['create_task']['execution']['chat'],
            *(state['execution']['chat'] for state in child_run['nodes'].values())]
        self.verify_session_hosts(chats)
        self.verify_mcp_call()
        confirmations = [json.loads(path.read_text(encoding='utf-8')) for path in self.confirmations.glob('*.request.json')]
        inner = self.verify_run(child, 'create-bot-v2.json', confirmations)
        outer = self.verify_run(root, 'create-task-v2.json', confirmations)
        child_finish = self.evidence['handoffs'][child_run['id']]['finish']
        parent_approval = self.evidence['handoffs'][root_id]['nodes']['create_bot_v2']['approval']
        child_history = self.histories[child_run['manager']['sessionId']]
        require(native_result(history_before(child_history, parent_approval['callId']), 'run',
            decode(child_finish['wire'], 'response'), action='finish') == child_finish['callId'],
            'Child manager did not explicitly finish before approving the parent execution')
        self.evidence['finalRuns'] = runs
        grants = self.verify_business(parent_run, child_run, outer, inner)
        self.verify_bindings(parent_run, child_run, outer, inner, contract / 'create-bot-v2-materials.md', grants)
        protected_after = {name: digest(self.root / name) if (self.root / name).is_file() else None for name in protected}
        require(protected_after == protected, ('V2 modified an owned existing V1/V2 definition', protected, protected_after))
        definitions_after = {str(path.relative_to(self.root)) for directory in ['maps', 'bots']
            for path in (self.root / directory).glob('*.json')}
        require(definitions_after == {*protected, str(Path('bots') / self.evidence['actualBot']['revision']['file'])},
            ('V2 produced an unrelated Bot or Map definition', definitions_after))
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
            'P34/V2/S2': ['definitions', 'nativeYes', 'histories', 'finalRuns', 'handoffs'],
            'P34/V2/S3': ['browser', 'definitions', 'finalRuns'],
            'P35/V2/S2': ['materials', 'materialReads', 'materialBindings', 'businessGrants', 'businessEffects', 'histories', 'nativeSessionHosts', 'nativeMcpCall', 'modelIdentities', 'modelConsumedPublishedPrompts', 'actualTarget', 'actualBot', 'finalRuns', 'handoffs', 'v1Preservation', 'ownedDefinitionReadback', 'quietWindow'],
        }[key]} for key in ITEMS}
        self.evidence['passed'] = True

    def cleanup_extra(self):
        try:
            super().cleanup_extra()
        finally:
            if self.model_wire and self.model_wire.records:
                self.check_model_identity(verify=False)
