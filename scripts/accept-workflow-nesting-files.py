"""Real native file-consumption acceptance; not a controlled-provider smoke.

Approved seams: delivered browser form, production HTTP, installed Herdr/Pi,
actual model traffic/tools/native history, owned file bytes and read-only Runs.
The isolated business definition declares Pi. OMP is never inferred from Pi.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import traceback

from developer_paths import REPO, backend_suffix, resolve_arguments

from acceptance.workflow.close import Acceptance
from acceptance.workflow.handoff import RolesAcceptance
from acceptance.host.processes import require, until, LiveProcess
from acceptance.host.wire import WireProxy, decode, messages
from acceptance.host.evidence import copy_evidence
from acceptance.workflow.handoff import FanInAcceptance

PROMISES = ['P05', 'P09', 'P10', 'P12', 'P14', 'P16', 'P18']


class FilesAcceptance(RolesAcceptance):
    def __init__(self, args):
        super().__init__(args)
        self.evidence.update(scenario='workflow-nesting-files', promise=None, promises=PROMISES,
            coverage={key: {'verified': False} for key in PROMISES}, verifiedHosts=[],
            declaredHosts=['pi'], requestedHosts=args.hosts, runs=[], interventions=[],
            boundary='Real model via declared transport; Pi only, no OMP or dual-host claim')
        self.native_paths = {}
        self.observed_processes = []
        self.env.update(XDG_DATA_HOME=str(self.root / 'state/data'), XDG_CACHE_HOME=str(self.root / 'state/cache'),
                        OMP_CODING_AGENT_DIR=str(self.root / 'omp-agent'))
        self.evidence['isolation'].update({key: self.env[key] for key in ['XDG_DATA_HOME', 'XDG_CACHE_HOME', 'OMP_CODING_AGENT_DIR']})

    def prepare(self):
        require(self.args.backend == (REPO / backend_suffix()).resolve(),
                'Build/use this worktree backend with the same independent CARGO_TARGET_DIR')
        require(self.args.backend.is_file(), ('Missing current worktree backend', str(self.args.backend)))
        for name, root, lock in [('herdr-bot', REPO, 'backend/Cargo.lock'), ('toolkit', self.args.toolkit, 'bun.lock')]:
            commands = {}
            for label, argv in [('revision', ['git', 'rev-parse', 'HEAD']), ('status', ['git', 'status', '--porcelain']),
                                ('diff', ['git', 'diff', '--binary', 'HEAD'])]:
                result = subprocess.run(argv, cwd=root, capture_output=True, text=True, encoding='utf-8', errors='replace', check=True)
                commands[label] = result.stdout
            self.evidence.setdefault('sources', {})[name] = {'root': str(root), **commands,
                'lockSha256': hashlib.sha256((root / lock).read_bytes()).hexdigest()}
        self.evidence['runnerSources'] = {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in [
            Path(__file__), REPO / 'scripts/workflow-nesting-files-browser.mjs', REPO / 'scripts/test-workflow-file-handoff.py',
            REPO / 'scripts/test-real-herdr-close.py',
            *[REPO / 'scripts/acceptance' / name for name in ['workflow/handoff.py', 'workflow/close.py',
                'host/processes.py', 'host/isolation.py', 'host/model.py', 'host/wire.py']],
            self.args.toolkit / 'entrypoints/pi-auto/manager-chat.ts',
            self.args.toolkit / 'entrypoints/pi-auto/node-chat.ts']}
        self.evidence['backend'] = {'path': str(self.args.backend), 'sha256': hashlib.sha256(self.args.backend.read_bytes()).hexdigest()}
        artifact_paths = [REPO / 'frontend/bun.lock', *sorted((REPO / 'backend/static').rglob('*'))]
        self.evidence['pageAndLockIdentity'] = {str(path): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in artifact_paths if path.is_file()}
        require((REPO / 'backend/static/index.html').is_file(), 'Build this worktree delivered static page first')
        require(self.args.hosts in ['declared', 'pi'],
                'OMP native file-consumption capability is not implemented by the installed Pi acceptance facility; omp,pi remains unverified')
        resolve_arguments(self.parser, self.args, {'herdr': ('herdr', 'herdr.exe'), 'pi_package': ('pi_package', ''),
            'pi_state_extension': ('pi_agent', 'extensions/herdr-agent-state.ts'), 'chrome': ('chrome', 'chrome.exe')})
        self.evidence['dependencies'] = {key: str(getattr(self.args, key)) for key in ['herdr', 'pi_package', 'pi_state_extension', 'chrome']}
        self.evidence['pathsFile'] = {'path': str(self.args.paths_file), 'sha256': hashlib.sha256(self.args.paths_file.read_bytes()).hexdigest()}
        require(self.args.chrome.is_file(), ('Missing declared browser', str(self.args.chrome)))
        for name, argv in [('node', [self.args.node, '--version']), ('bun', [self.args.bun, '--version']),
                           ('pi', [self.args.node, self.args.pi_package / 'dist/bundle/cli.js', '--version'])]:
            self.evidence.setdefault('versions', {})[name] = self.command(argv)
        require(self.evidence['versions']['bun'].strip() == '1.4.4', 'toolkit checks require Bun 1.4.4')
        Acceptance.prepare(self)
        self.backend_wire = WireProxy(self.base, self.root / 'backend-wire')
        (self.root / 'toolkit/extensions/workflow-chat/config.yaml').write_text(
            f'endpoint: {self.backend_wire.base}\nrequestTimeoutMs: 30000\nreportConfirmationTimeoutMs: 20000\ncollectTimeoutMs: 5460000\n', encoding='utf-8')
        self.provision()

    @staticmethod
    def save(path, value):
        path.write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding='utf-8')

    def provision(self):
        self.zones = {}
        for name in ['outer', 'middle', 'leaf', 'ordinary', 'second', 'empty', 'deleted']:
            directory = self.root / ('business-' + name)
            directory.mkdir()
            self.zones[name] = directory
            self.http('/api/zones', {'id': 'files/' + name, 'path': str(directory)})
            workspace = self.herdr('workspace', 'create', '--cwd', str(directory), '--label', directory.name, '--no-focus')
            self.evidence.setdefault('ownedBusinessWorkspaces', []).append(workspace)
        manager = ('This is an authorized isolated file handoff rehearsal. Use actual workflow run/collect tools. '
            'Never use HTTP, scripts, guessed paths or read runtime records. Dispatch ready nodes, collect reports, '
            'explicitly complete each node only after a true report. Do not retry or repair any failed work. '
            'On false report stop and explain the blocker. After every node completed explicitly finish this Run. '
            'If this Chat also executes a parent node, after finishing report true to the parent using output ID result '
            'and the exact real file address registered as result by internal node make or child. Do not read material bodies '
            'by default; transfer addresses from actual reports/snapshot, never invent a file. '
            'Do not finish or report before all internal work is accepted. No human approval is needed for the declared isolated work.')
        worker = ('Perform only your assigned business work. Use native file read/write tools for actual material content. '
            'Read only explicitly bound named inputs, never search a directory or infer a path from prose. '
            'An absent optional input is absent, not a placeholder. Report concise real outcomes using the report tool. '
            'Never repair/retry missing material, never read other Chats, scripts, credentials or runtime records.')
        self.tasks = {}
        # This is the explicit isolated business definition, not a rewritten host
        # selection from the driver. Every Task's declaration is kept as evidence.
        for role, prompt in [('manager', manager), ('worker', worker)]:
            result = self.http('/api/tasks', {'name': 'Files ' + role, 'taskType': 'manager' if role == 'manager' else 'normal',
                'agent': 'pi', 'chatName': '{chat.id}', 'model': 'probe', 'thinking': 'default', 'skill': None, 'prompt': prompt})
            task = next(task for task in result['tasks'] if task['name'] == 'Files ' + role)
            self.tasks[role] = task['id']
            self.evidence.setdefault('taskDeclarations', []).append(task)
        for id, target in [('worker', {'task_id': self.tasks['worker']}), ('middle', {'map_file': 'middle.json'}),
                           ('leaf', {'map_file': 'leaf.json'}), ('second', {'map_file': 'second.json'})]:
            self.save(self.root / f'bots/{id}.json', {'id': id, 'name': id, 'when': '', 'plan_guide': '', 'node_guide': '', **target})
        def node(name, zone, rules=None, dependencies=None, bot='worker', business=None):
            value = {'title': name, 'bot_id': bot, 'zone': 'files/' + zone, 'blocked_by': dependencies or [],
                'input': business or ('Read the sole actual named input with native read, then write exactly its UTF-8 text '
                    'without changing any byte to delivered.txt in your actual working directory. '
                    'Report true with output ID result pointing to that real file. Include OPTIONAL-APPENDIX-ABSENT in your report; do not seek appendix.'),
                'input_rule': rules or [], 'output_rule': [{'id': 'result'}]}
            if bot != 'worker':
                value['manager_task_id'] = self.tasks['manager']
                value['input'] = ('Manage the referenced child workflow using the actual bound file interface. '
                    'The internal worker performs the file reading/writing; do not read material bodies yourself. '
                    'After explicitly finishing the child Run, report true to your parent with output ID result '
                    'mapped to the exact registered result file from child/make. Do not substitute another address.')
            return value
        run_rule = lambda local, upstream: {'id': local, 'type': 'run', 'input_id': upstream}
        node_rule = lambda local: {'id': local, 'type': 'node', 'node_id': 'call', 'output_id': 'result'}
        declaration = lambda id: [{'id': id, 'required': True}, {'id': 'appendix', 'required': False}]
        empty_input = ('There are zero actual named inputs. Do not read, list or search any file, nor invent an input path. '
                       'Report true without outputs, explicitly saying NO-FILE-INPUTS and OPTIONAL-APPENDIX-ABSENT.')
        leaf_worker = node('make', 'leaf', [run_rule('leaf_local', 'deep_source')])
        leaf_worker['input'] = ('Before any material tool call, reply WAITING-FOR-FILE-RELEASE and stop this turn. '
            'Only after a subsequent explicit user release, read the bound leaf_local input, then write its exact UTF-8 bytes '
            'unchanged to delivered.txt in your actual working directory. Report true with output ID result and that real address. '
            'Include OPTIONAL-APPENDIX-ABSENT in your report; do not seek appendix. Never use previous Run material.')
        self.save(self.root / 'maps/leaf.json', {'title': 'Files leaf', 'description': '', 'input_files': declaration('deep_source'),
            'nodes': {'make': leaf_worker, 'none': {**node('none', 'empty', business=empty_input), 'output_rule': []}}})
        self.save(self.root / 'maps/middle.json', {'title': 'Files middle', 'description': '', 'input_files': declaration('middle_source'),
            'nodes': {'child': node('child', 'leaf', [run_rule('deep_source', 'middle_source')], bot='leaf')}})
        self.save(self.root / 'maps/second.json', {'title': 'Files second consumer', 'description': '', 'input_files': declaration('consumer_source'),
            'nodes': {'make': node('make', 'second', [run_rule('second_local', 'consumer_source')])}})
        self.save(self.root / 'maps/empty.json', {'title': 'Files empty', 'description': '', 'input_files': [{'id': 'appendix', 'required': False}],
            'nodes': {'none': {**node('none', 'empty', business=empty_input), 'output_rule': []}}})
        self.save(self.root / 'maps/root.json', {'title': 'Nesting files actual consumption', 'description': '', 'input_files': declaration('root_source'),
            'nodes': {'call': node('call', 'middle', [run_rule('middle_source', 'root_source')], bot='middle'),
                'ordinary': node('ordinary', 'ordinary', [node_rule('ordinary_local')], ['call']),
                'second': node('second', 'second', [node_rule('consumer_source')], ['call'], bot='second')}})
        self.save(self.root / 'maps/deleted.json', {'title': 'Files deleted after dispatch', 'description': '', 'input_files': declaration('vanishing'),
            'nodes': {'make': node('make', 'deleted', [run_rule('vanishing_local', 'vanishing')], business=(
                'Before any material tool call reply WAITING-FOR-FILE-RELEASE and stop this turn. '
                'After explicit user release, try native read of the sole bound vanishing_local input exactly once. '
                'If it is unavailable, report false describing the real error and DELETED-INPUT; do not create, repair, search or retry. '
                'Otherwise report true with the real observation.'))}})
        self.evidence['definitions'] = {str(path.relative_to(self.root)): json.loads(path.read_text(encoding='utf-8'))
            for path in [*sorted((self.root / 'maps').glob('*.json')), *sorted((self.root / 'bots').glob('*.json'))]}

    def native(self, chat):
        session = chat['sessionId']
        if session not in self.native_paths:
            for path in (self.root / 'agent/sessions').rglob('*.jsonl'):
                with path.open(encoding='utf-8') as source:
                    first = source.readline()
                if first and json.loads(first).get('id') == session:
                    self.native_paths[session] = path
                    break
        if session not in self.native_paths:
            return None
        return self.history(str(self.native_paths[session]))

    @staticmethod
    def native_messages(history):
        return [entry['message'] for entry in history['branch'] if entry.get('type') == 'message']

    def wait_chat(self, chat, marker):
        def probe():
            history = self.native(chat)
            if not history:
                return None
            messages = self.native_messages(history)
            if messages and messages[-1].get('role') == 'assistant' and marker in json.dumps(messages[-1], ensure_ascii=False):
                return history
        return until('actual native Chat ' + marker, probe, self.args.timeout)

    def detail_id(self, id):
        return self.http('/api/workflow/runs/' + id)

    def page_start(self, map_file, business, files):
        request = {'base': self.base, 'mapTitle': json.loads((self.root / 'maps' / map_file).read_text())['title'],
            'manager': self.tasks['manager'], 'zone': 'files/outer', 'business': business, 'files': files,
            'chrome': str(self.args.chrome), 'evidence': str(self.root / ('page-' + str(len(self.evidence['runs'])) + '.json'))}
        config = self.root / 'browser-request.json'
        self.save(config, request)
        process = self.start([self.args.node, REPO / 'scripts/workflow-nesting-files-browser.mjs', config], 'browser-' + str(len(self.evidence['runs'])))
        try:
            code = process.child.wait(timeout=120)
            require(code == 0, ('delivered page launch failed; see raw browser streams and evidence', request['evidence']))
        finally:
            process.stop()
        observation = json.loads(Path(request['evidence']).read_text(encoding='utf-8'))
        self.evidence.setdefault('browserStarts', []).append(observation)
        detail = self.detail_id(observation['runId'])
        require(detail['run']['userInput'] == business and detail['run']['fileInputs'] == files, detail)
        self.evidence['runs'].append({'id': detail['run']['id'], 'business': business, 'fileInputs': files})
        return detail['run']

    def find_execution(self, root_id, path):
        detail = self.detail_id(root_id)
        for index, node_id in enumerate(path):
            execution = detail['run']['nodes'][node_id].get('execution')
            if not execution:
                return None
            if index == len(path) - 1:
                return detail, execution
            if not execution.get('childRunId'):
                return None
            detail = self.detail_id(execution['childRunId'])

    def release(self, chat, text):
        self.evidence['interventions'].append({'chat': chat, 'text': text, 'at': time.monotonic()})
        self.evidence['interventions'][-1]['result'] = self.herdr('agent', 'prompt', chat['paneId'], text)

    def snapshot_file(self, path, label):
        body = path.read_bytes()
        self.evidence.setdefault('files', []).append({'label': label, 'path': str(path), 'bytesHex': body.hex(),
            'sha256': hashlib.sha256(body).hexdigest(), 'at': time.monotonic()})
        return body

    def exercise(self):
        try:
            self.exercise_scenarios()
        finally:
            if self.herdr_started and not self.observed_processes:
                try:
                    self.observe_processes(require_pi=False)
                except BaseException:
                    self.evidence['cleanupErrors'].append(traceback.format_exc())

    def exercise_scenarios(self):
        for index, original, expected in [(0, b'FILE-BYTES-ORIGINAL-ONE\n', b'FILE-BYTES-MUTATED-ONE\n'),
                                           (1, b'FILE-BYTES-INDEPENDENT-TWO\n', b'FILE-BYTES-INDEPENDENT-TWO\n')]:
            material = self.zones['outer'] / f'input-{index}.txt'
            material.write_bytes(original)
            self.snapshot_file(material, f'run-{index}-before')
            root = self.page_start('root.json', f'INDEPENDENT-BUSINESS-TEXT-{index}: only the explicit named binding is material.',
                                   [{'id': 'root_source', 'path': str(material)}])
            detail, execution = until('three-level native dispatch', lambda: self.find_execution(root['id'], ['call', 'child', 'make']), self.args.timeout)
            self.wait_chat(execution['chat'], 'WAITING-FOR-FILE-RELEASE')
            require(execution['inputs'] == [{'id': 'leaf_local', 'path': str(material)}], execution)
            require(detail['run']['fileInputs'] == [{'id': 'deep_source', 'path': str(material)}], detail)
            if index == 0:
                material.write_bytes(expected)
            self.snapshot_file(material, f'run-{index}-after-dispatch')
            self.release(execution['chat'], 'Explicit isolated business release: consume the current bound file now; do not use earlier content.')
            final = until('root and native downstream consumers completed', lambda: (value if (value := self.detail_id(root['id']))['run']['status'] == 'completed' else None), self.args.timeout)
            self.verify_tree(final, material, expected, index)
        # A root Run with no bindings and a nested no-input executor are both real.
        empty = self.page_start('empty.json', 'EMPTY-ROOT-BUSINESS-TEXT: no files have been supplied.', [])
        final_empty = until('empty root accepted', lambda: (value if (value := self.detail_id(empty['id']))['run']['status'] == 'completed' else None), self.args.timeout)
        self.verify_empty(final_empty['run']['nodes']['none']['execution'])
        # Delete only after actual dispatch/check and the real executor's pause.
        vanishing = self.zones['outer'] / 'vanishing.txt'
        vanishing.write_bytes(b'DELETE-AFTER-DISPATCH-519\n')
        self.snapshot_file(vanishing, 'before-delete')
        deleted = self.page_start('deleted.json', 'DELETE-CASE-BUSINESS-TEXT: no repair is authorized.', [{'id': 'vanishing', 'path': str(vanishing)}])
        detail, execution = until('delete-case native dispatch', lambda: self.find_execution(deleted['id'], ['make']), self.args.timeout)
        self.wait_chat(execution['chat'], 'WAITING-FOR-FILE-RELEASE')
        require(execution['inputs'] == [{'id': 'vanishing_local', 'path': str(vanishing)}], execution)
        vanishing.unlink()
        self.evidence['deletion'] = {'path': str(vanishing), 'executionId': execution['id'], 'at': time.monotonic()}
        self.release(execution['chat'], 'Explicit isolated business release: attempt the bound file now exactly once; no repair or retry authorized.')
        failure = until('actual missing-file report', lambda: next((report for report in self.detail_id(deleted['id'])['reports'] if report.get('canComplete') is False and 'DELETED-INPUT' in report['body']), None), self.args.timeout)
        history = self.preserve_native(execution['chat'], 'deleted')
        self.verify_read(history, vanishing, None, failure=True)
        before = self.detail_id(deleted['id'])
        time.sleep(1.25)
        after = self.detail_id(deleted['id'])
        require(not vanishing.exists(), 'system repaired deleted input')
        require(before['run']['nodes']['make']['execution']['id'] == after['run']['nodes']['make']['execution']['id'] and not after['run']['nodes']['make']['previousExecutions'], after)
        require(after['run']['nodes']['make']['status'] != 'completed', after)
        self.evidence['deletion'].update(report=failure, observationSeconds=1.25, final=after)
        require(not any(record['path'].endswith('/retry') for record in self.backend_wire.records), 'unexpected business retry observed')
        for role in ['manager', 'node']:
            guide = (self.args.toolkit / f'extensions/workflow-chat/{role}-chat/shared-logic/prompts/init.md').read_text(encoding='utf-8').strip()
            require(any(guide in '\n'.join(self.message_text(message).replace('\r\n', '\n')
                for message in messages(record)) for record in self.model_wire.records), ('actual model did not receive declared role guide', role))
        self.observe_processes()
        for promise in PROMISES:
            self.evidence['coverage'][promise] = {'verified': True, 'host': 'pi',
                'evidence': 'browserStarts, runs, trees, nativeHistories, readTraces, files, deletion'}
        self.evidence['verifiedHosts'] = ['pi']
        self.evidence['passed'] = True

    def preserve_native(self, chat, label):
        history = until('native history ' + label, lambda: self.native(chat), self.args.timeout)
        require(history['id'] == chat['sessionId'], ('native identity mismatch', chat))
        path = self.native_paths[chat['sessionId']]
        with path.open(encoding='utf-8') as source:
            header = json.loads(source.readline())
        self.evidence.setdefault('nativeHistories', []).append({'label': label, 'chat': chat,
            'path': str(path), 'header': header, 'history': history, 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()})
        return history

    def verify_read(self, history, path, expected, failure=False):
        messages = self.native_messages(history)
        calls = [part for message in messages if message.get('role') == 'assistant'
                 for part in message.get('content', []) if isinstance(part, dict) and part.get('type') == 'toolCall']
        reads = [call for call in calls if call.get('name') == 'read' and
                 Path(call.get('arguments', {}).get('path', '')).resolve() == path.resolve()]
        require(len(reads) == 1, ('expected exactly one real read of bound material', str(path), calls))
        result = next((message for message in messages if message.get('role') == 'toolResult' and message.get('toolCallId') == reads[0]['id']), None)
        require(result, ('missing native tool result', reads))
        if failure:
            require(result.get('isError') is True and ('ENOENT' in json.dumps(result) or 'not found' in json.dumps(result).lower()), result)
        else:
            require(not result.get('isError') and expected.decode('utf-8').strip() in json.dumps(result, ensure_ascii=False), result)
        self.evidence.setdefault('readTraces', []).append({'sessionId': history['id'], 'path': str(path), 'call': reads[0], 'result': result})

    def verify_empty(self, execution):
        require(execution['inputs'] == [], execution)
        history = self.preserve_native(execution['chat'], 'empty')
        messages = self.native_messages(history)
        calls = [part for message in messages if message.get('role') == 'assistant'
                 for part in message.get('content', []) if isinstance(part, dict) and part.get('type') == 'toolCall']
        require(calls and all(call.get('name') == 'report' for call in calls), ('empty executor guessed/read a path', calls))
        require(any('NO-FILE-INPUTS' in call.get('arguments', {}).get('body', '') and
                    'OPTIONAL-APPENDIX-ABSENT' in call.get('arguments', {}).get('body', '') for call in calls),
                ('actual empty-input report did not acknowledge absence', calls))

    @staticmethod
    def reported_outputs(detail, node):
        execution = detail['run']['nodes'][node]['execution']['id']
        reports = [report for report in detail['reports'] if report['executionId'] == execution and report.get('outputs')]
        require(len(reports) == 1 and reports[0]['canComplete'] is True, ('expected one actual output registration', node, reports))
        return reports[0]['outputs']

    def verify_tree(self, final, material, expected, index):
        root = final['run']
        middle = self.detail_id(root['nodes']['call']['execution']['childRunId'])
        leaf = self.detail_id(middle['run']['nodes']['child']['execution']['childRunId'])
        second = self.detail_id(root['nodes']['second']['execution']['childRunId'])
        require(middle['run']['fileInputs'] == [{'id': 'middle_source', 'path': str(material)}], middle)
        require(leaf['run']['fileInputs'] == [{'id': 'deep_source', 'path': str(material)}], leaf)
        leaf_outputs = self.reported_outputs(leaf, 'make')
        require(len(leaf_outputs) == 1 and leaf_outputs[0]['id'] == 'result', leaf_outputs)
        leaf_output = leaf_outputs[0]['path']
        require(self.reported_outputs(middle, 'child') == leaf_outputs and self.reported_outputs(final, 'call') == leaf_outputs, 'nested managers changed the actual output mapping')
        require(second['run']['fileInputs'] == [{'id': 'consumer_source', 'path': leaf_output}], second)
        for record, node, local, source, zone in [(leaf, 'make', 'leaf_local', str(material), 'leaf'),
                (final, 'ordinary', 'ordinary_local', leaf_output, 'ordinary'),
                (second, 'make', 'second_local', leaf_output, 'second')]:
            execution = record['run']['nodes'][node]['execution']
            require(execution['inputs'] == [{'id': local, 'path': source}], execution)
            outputs = self.reported_outputs(record, node)
            require(len(outputs) == 1 and outputs[0]['id'] == 'result' and
                    Path(outputs[0]['path']).resolve() == (self.zones[zone] / 'delivered.txt').resolve(), outputs)
            history = self.preserve_native(execution['chat'], f'run-{index}-{zone}')
            require(Path(self.evidence['nativeHistories'][-1]['header']['cwd']).resolve() == self.zones[zone].resolve(), 'actual native executor cwd differs from declared zone')
            self.verify_read(history, Path(source), expected)
            calls = FanInAcceptance.native_calls(history['branch'])
            require(all(call['name'] in ['read', 'write', 'report'] for call in calls), ('executor bypassed material read boundary', calls))
            allowed = {Path(source).resolve(), (self.zones[zone] / 'delivered.txt').resolve()}
            require(all(Path(call.get('arguments', {}).get('path', '')).resolve() in allowed for call in calls if call['name'] == 'read'), ('executor sought unbound/optional files', calls))
            require(any(call['name'] == 'report' and 'OPTIONAL-APPENDIX-ABSENT' in call.get('arguments', {}).get('body', '') for call in calls), 'actual executor did not acknowledge absent optional input')
            require(self.snapshot_file(self.zones[zone] / 'delivered.txt', f'run-{index}-{zone}-output') == expected, 'wrong actual consumer bytes')
        require(root['zone'] == str(self.zones['outer']) and middle['run']['zone'] == str(self.zones['middle']) and leaf['run']['zone'] == str(self.zones['leaf']), 'inner/outer actual startup zone mismatch')
        for record, zone in [(final, 'outer'), (middle, 'middle'), (leaf, 'leaf'), (second, 'second')]:
            self.preserve_native(record['run']['manager'], f'run-{index}-manager-{zone}')
            require(Path(self.evidence['nativeHistories'][-1]['header']['cwd']).resolve() == self.zones[zone].resolve(), 'actual native manager cwd differs from declared zone')
        self.verify_empty(leaf['run']['nodes']['none']['execution'])
        # Text and bytes are independent: initial real provider requests contain
        # the actual business text/address, but not the material body.
        for record, text_marker in [(final, f'INDEPENDENT-BUSINESS-TEXT-{index}'), (leaf, 'Before any material tool call')]:
            chat = record['run']['manager'] if record is final else record['run']['nodes']['make']['execution']['chat']
            history = self.preserve_native(chat, f'run-{index}-initial-{text_marker[:8]}')
            initial = next(message for message in self.native_messages(history) if message.get('role') == 'user')
            require(text_marker in self.message_text(initial), initial)
            # P09 requires the independently edited binding to reach Run/native
            # consumption, not a second manager-prompt injection convention.
            # Root managers receive fileInputs through actual public status;
            # node executors receive their named inputs in initialization.
            if record is final:
                require(any(message.get('role') == 'toolResult' and 'root_source' in self.message_text(message) and
                    str(material) in self.message_text(message).replace('\\\\', '\\') for message in self.native_messages(history)),
                    'root native manager never received actual named binding from public status')
            else:
                require(str(material) in self.message_text(initial), initial)
            require(not any(marker in json.dumps(initial) for marker in ['FILE-BYTES-ORIGINAL-ONE', 'FILE-BYTES-MUTATED-ONE', 'FILE-BYTES-INDEPENDENT-TWO']), 'file body was snapshotted into initial Chat')
            require(any(text_marker in (body := '\n'.join(self.message_text(message) for message in messages(request))) and
                str(material) in body.replace('\\\\', '\\') for request in self.model_wire.records), 'actual model provider never received independent text and binding')
        self.evidence.setdefault('trees', []).append({'index': index, 'root': final, 'middle': middle, 'leaf': leaf, 'second': second})
        for detail in [final, middle, leaf, second]:
            run_id = detail['run']['id']
            target = self.root / f'run-facts-{run_id}.json'
            self.save(target, detail)

    def observe_processes(self, require_pi=True):
        script = ("Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -and $_.CommandLine.Contains(" +
            "'" + str(self.root).replace("'", "''") + "') } | Select-Object ProcessId,ParentProcessId,ExecutablePath,CommandLine | ConvertTo-Json -Compress")
        records = json.loads(self.command(['powershell.exe', '-NoProfile', '-Command', script]) or '[]')
        if isinstance(records, dict):
            records = [records]
        self.evidence['osProcesses'] = records
        for record in records:
            if record['ProcessId'] != os.getpid() and 'powershell' not in (record.get('ExecutablePath') or '').lower():
                self.observed_processes.append(LiveProcess(record['ProcessId']))
        if require_pi:
            require(any('pi-coding-agent' in (record.get('CommandLine') or '') for record in records), 'No actual installed Pi OS process observed')

    def cleanup_extra(self):
        errors = []
        try:
            super().cleanup_extra()
        except BaseException:
            errors.append(traceback.format_exc())
        for process in self.observed_processes:
            try:
                until('owned native process exit', lambda: not process.alive(), 15)
                self.evidence.setdefault('exitedNativePids', []).append(process.pid)
            except BaseException:
                errors.append(traceback.format_exc())
            finally:
                process.close()
        require(not errors, errors)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ['backend', 'toolkit', 'paths-file', 'model-config', 'evidence-dir']:
        parser.add_argument('--' + flag, type=lambda value: Path(value).resolve(), required=True)
    parser.add_argument('--hosts', choices=['declared', 'pi', 'omp', 'omp,pi'], required=True)
    for flag in ['herdr', 'pi-package', 'pi-state-extension', 'chrome', 'shell-path']:
        parser.add_argument('--' + flag, type=Path)
    parser.add_argument('--node', default='node')
    parser.add_argument('--bun', default='bun')
    parser.add_argument('--timeout', type=int, default=300)
    args = parser.parse_args()
    require(1 <= args.timeout <= 900, '--timeout must be 1..900')
    args.repo = REPO
    acceptance = FilesAcceptance(args)
    acceptance.parser = parser
    return acceptance.run()


if __name__ == '__main__':
    sys.exit(main())
