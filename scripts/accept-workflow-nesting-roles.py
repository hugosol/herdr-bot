"""Real nested dual-role acceptance via public HTTP, installed Herdr and OMP/Pi.

No observer grants authorization. Native tools and native Session identity own all
workflow actions. Missing dependencies/models fail closed; no business retries.
"""
import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import re
import runpy
import shutil
import subprocess
import sys
import time
import traceback

from developer_paths import REPO, backend_suffix, resolve_arguments

shared = runpy.run_path(str(Path(__file__).with_name('test-workflow-file-handoff.py')))
Acceptance = shared['Acceptance']
RolesAcceptance = shared['RolesAcceptance']
WireProxy = shared['WireProxy']
require, until, decode = shared['require'], shared['until'], shared['decode']


# These are the business fixture's actual host declarations. Declared mode keeps
# each role's host intact; the explicit omp,pi matrix runs homogeneous host cases.
DECLARED_ROLES = [
    {'parent': 'omp', 'child': 'pi', 'worker': 'omp'},
    {'parent': 'pi', 'child': 'omp', 'worker': 'pi'},
]


class DualRoleAcceptance(Acceptance):
    """Approved seams: native role tools, public Run facts, Herdr Tabs and history.

    Independent oracle: fixed sample bytes and ticket lifecycle boundaries. A
    plausible violation routes report to the child, lets finish close its Chat,
    or mistakes business approval for manual-send approval. Gates observe each
    boundary before the next actual user message; final status alone cannot pass.
    """
    def __init__(self, args):
        native_args = argparse.Namespace(**vars(args))
        native_args.evidence_dir = None
        super().__init__(native_args, prefix='nested-' + args.host + '-')
        self.args = args
        self.env = {key: value for key, value in self.env.items() if not any(term in key.upper() for term in ['SECRET', 'PASSWORD', 'CREDENTIAL', 'PROXY'])}
        self.host = args.host
        self.hosts = ['omp', 'pi'] if self.host == 'declared' else [self.host]
        self.chat_hosts = {}
        self.backend_wire = self.model_wire = None
        self.handles = []
        self.native_cache = {}
        self.evidence.update(host=self.host, samples=[], nativeProcesses=[], coverage={},
            scope='Real installed host, real model, native identity, public backend authorization; no controlled business provider')

    def configure_model(self):
        RolesAcceptance.configure_model(self)
        agent = self.root / 'agent'
        if 'omp' in self.hosts:
            shutil.copy2(agent / 'models.json', agent / 'models.yml')
            (agent / 'config.yml').write_text('autoResume: false\nstartup:\n  setupWizard: false\n  showSplash: false\n  checkUpdate: false\nmarketplace:\n  autoUpdate: off\n', encoding='utf-8')

    def role_catalog(self, toolkit):
        catalog = 'skills = []\n' + ''.join(f'[[agents]]\nid = "{host}"\nlabel = "{host}"\n' for host in self.hosts)
        catalog += '[[models]]\nid = "default"\nlabel = "Default"\n[[models]]\nid = "probe"\nlabel = "Real acceptance"\nprovider_model = ' + json.dumps(self.provider_model) + '\nthinking = { ' + ', '.join(f'{host} = ["off"]' for host in self.hosts) + ' }\n'
        for host in self.hosts:
            roles = [('manager', toolkit / f'entrypoints/{host}-auto/manager-chat.ts'),
                     ('node', toolkit / f'entrypoints/{host}-auto/node-chat.ts')]
            roles.append(('herdr-state', self.args.omp_state_extension if host == 'omp' else self.args.pi_state_extension))
            for role, path in roles:
                identity = f'{host}-herdr-agent-state' if role == 'herdr-state' else f'{host}-{role}-chat'
                catalog += f'[[extensions]]\nid = "{identity}"\nlabel = "{role}"\nagent = "{host}"\nkind = "fixed"\nentrypoint = {json.dumps(str(path))}\n'
        return catalog

    def prepare(self):
        require(self.args.backend.is_file(), ('Missing current worktree backend', str(self.args.backend)))
        for name, root, lock in [('herdr-bot', REPO, 'backend/Cargo.lock'), ('toolkit', self.args.toolkit, 'bun.lock')]:
            source = {'root': str(root), 'lockSha256': hashlib.sha256((root / lock).read_bytes()).hexdigest()}
            for label, argv in [('revision', ['rev-parse', 'HEAD']), ('status', ['status', '--porcelain']), ('diff', ['diff', '--binary', 'HEAD'])]:
                source[label] = subprocess.run(['git', *argv], cwd=root, capture_output=True, text=True, check=True, encoding='utf-8').stdout
            self.evidence.setdefault('sources', {})[name] = source
        for host in self.hosts:
            package = self.args.omp_package if host == 'omp' else self.args.pi_package
            entry = package / ('dist/cli.js' if host == 'omp' else 'dist/bundle/cli.js')
            require(entry.is_file(), ('Missing installed native host', str(entry)))
            launcher = shutil.which(host)
            require(launcher, ('Missing installed native launcher on unchanged PATH', host))
            self.evidence.setdefault('installedHosts', {})[host] = {'entry': str(entry), 'entrySha256': hashlib.sha256(entry.read_bytes()).hexdigest(),
                'package': json.loads((package / 'package.json').read_text(encoding='utf-8'))['version'],
                'launcher': launcher, 'launcherSha256': hashlib.sha256(Path(launcher).read_bytes()).hexdigest()}
        self.evidence['runnerSha256'] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
        self.evidence['backend'] = {'path': str(self.args.backend), 'sha256': hashlib.sha256(self.args.backend.read_bytes()).hexdigest()}
        super().prepare()
        self.backend_wire = WireProxy(self.base, self.root / 'backend-wire')
        (self.root / 'toolkit/extensions/workflow-chat/config.yaml').write_text(
            f'endpoint: {self.backend_wire.base}\nrequestTimeoutMs: 30000\ncollectTimeoutMs: 5460000\n', encoding='utf-8')

    def detail(self, run):
        return self.http('/api/workflow/runs/' + run)

    def native(self, chat):
        files = list((self.root / 'agent').rglob('*.jsonl'))
        for file in files:
            try:
                with file.open(encoding='utf-8') as stream:
                    first = json.loads(stream.readline())
                    if first.get('type') == 'title':
                        first = json.loads(stream.readline())
            except (ValueError, OSError):
                continue
            if first.get('type') == 'session' and first.get('id') == chat['sessionId']:
                self.evidence.setdefault('nativeHeaders', {})[chat['sessionId']] = first
                digest = hashlib.sha256(file.read_bytes()).hexdigest()
                cached = self.native_cache.get(chat['sessionId'])
                if cached and cached[0] == digest:
                    return cached[1]
                host = self.chat_hosts[chat['sessionId']]
                package = self.args.omp_package if host == 'omp' else self.args.pi_package
                module = package / ('src/session/session-manager.ts' if host == 'omp' else 'dist/core/session-manager.js')
                code = 'import {SessionManager} from ' + json.dumps(module.as_uri()) + '; const s=await SessionManager.open(' + json.dumps(str(file)) + '); console.log(JSON.stringify({id:s.getSessionId(),branch:s.getBranch()}));'
                history = json.loads(self.command([self.args.bun if host == 'omp' else self.args.node, '--eval', code] if host == 'omp'
                    else [self.args.node, '--input-type=module', '--eval', code]))
                require(history['id'] == chat['sessionId'], 'Native history identity mismatch')
                target = self.root / 'histories'
                target.mkdir(exist_ok=True)
                shutil.copy2(file, target / file.name)
                self.native_cache[chat['sessionId']] = (digest, history)
                return history
        return None

    def await_note(self, chat, marker):
        def probe():
            history = self.native(chat)
            if not history:
                return None
            entries = [entry['message'] for entry in history['branch'] if entry.get('type') == 'message']
            assistant = [message for message in entries if message.get('role') == 'assistant']
            return history if assistant and marker in RolesAcceptance.message_text(assistant[-1]) and not any(
                part.get('type') == 'toolCall' for part in assistant[-1].get('content', []) if isinstance(part, dict)) else None
        return until('native assistant approval boundary ' + marker, probe, self.args.timeout)

    def await_approval(self, chat, marker, output=None):
        history = self.await_note(chat, marker)
        assistant = [entry['message'] for entry in history['branch'] if entry.get('type') == 'message' and entry['message'].get('role') == 'assistant'][-1]
        note = RolesAcceptance.message_text(assistant)
        # A marker is synchronization, not approval evidence. The visible native
        # answer must actually ask about sending, state a limitation and describe
        # the current handoff (including its real location once one exists).
        visible = note.replace(marker, '')
        require(len(visible) > 50 and re.search(r'[?？]', visible) and re.search(r'send|report|发送|报告', visible, re.I), 'Native model did not actually ask for report-send approval')
        require(re.search(r'limitation|not |no |only|diagnostic|before|限制|仅|尚|未|诊断', visible, re.I), 'Approval summary omitted actual limitations')
        if output:
            require(str(output).replace('\\', '/') in note.replace('\\', '/'), 'Approval summary omitted actual result location')
        self.evidence.setdefault('approvalSummaries', []).append({'sessionId': chat['sessionId'], 'marker': marker, 'text': note, 'criteria': 'visible summary, send question, limitation, actual result address when available'})
        return history

    def send(self, chat, message, kind):
        self.evidence.setdefault('userInputs', []).append({'kind': kind, 'sessionId': chat['sessionId'], 'input': message, 'at': time.monotonic()})
        return self.herdr('agent', 'prompt', chat['paneId'], message)

    def tabs(self, workspace):
        return self.herdr('tab', 'list', '--workspace', workspace)['tabs']

    def observe_processes(self):
        owned = str(self.root).replace("'", "''")
        code = "Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -like '*" + owned + "*' } | Select-Object ProcessId,ParentProcessId,ExecutablePath,CommandLine | ConvertTo-Json -Compress"
        value = json.loads(self.command(['powershell.exe', '-NoProfile', '-Command', code]) or '[]')
        processes = value if isinstance(value, list) else [value]
        self.evidence['nativeProcesses'].extend(processes)
        parse = ctypes.windll.shell32.CommandLineToArgvW
        parse.argtypes = [ctypes.c_wchar_p, ctypes.POINTER(ctypes.c_int)]
        parse.restype = ctypes.POINTER(ctypes.c_wchar_p)
        free = ctypes.windll.kernel32.LocalFree
        free.argtypes = [ctypes.c_void_p]
        for process in processes:
            count = ctypes.c_int()
            arguments = parse(process.get('CommandLine') or '', ctypes.byref(count))
            require(bool(arguments), 'Cannot parse observed Windows command line')
            try:
                process['arguments'] = [arguments[index] for index in range(count.value)]
            finally:
                free(arguments)
        for host in self.hosts:
            expected = Path(self.evidence['installedHosts'][host]['entry']).resolve()
            require(any(any(Path(argument).resolve() == expected for argument in process['arguments'] if argument.replace('\\', '/').endswith('/cli.js')) for process in processes), ('No actual installed host OS process under this isolation root', host))
        for process in processes:
            if process.get('ProcessId') == os.getpid() or 'powershell' in str(process.get('ExecutablePath', '')).lower():
                continue
            if process['ProcessId'] in {pid for pid, _ in self.handles}:
                continue
            self.handles.append((process['ProcessId'], shared['shared']['LiveProcess'](process['ProcessId'])))
        return processes

    def task(self, name, kind, prompt, host):
        result = self.http('/api/tasks', {'name': name, 'taskType': kind, 'agent': host,
            'chatName': '{chat.id}', 'model': 'probe', 'thinking': 'default', 'skill': None, 'prompt': prompt})
        return next(task['id'] for task in result['tasks'] if task['name'] == name)

    def capture_failure_terminals(self):
        panes = set()
        def visit(value):
            if isinstance(value, dict):
                if value.get('paneId'):
                    panes.add(value['paneId'])
                for child in value.values():
                    visit(child)
            elif isinstance(value, list):
                for child in value:
                    visit(child)
        for record in self.evidence.get('http', []):
            try:
                visit(json.loads(record['body']))
            except ValueError:
                continue
        for pane in panes:
            try:
                self.evidence.setdefault('failureTerminals', {})[pane] = self.command([self.args.herdr, 'pane', 'read', pane, '--lines', '200', '--format', 'text'])
            except BaseException:
                self.evidence.setdefault('diagnosticCaptureErrors', []).append(traceback.format_exc())

    def exercise(self):
        # Two independent manager sources, locations and raw inputs; each calls the
        # same inner Map twice. The first run remains present during the second.
        for index, literal in enumerate(['ALPHA-371:17\\n', 'BETA-902:29\\n']):
            try:
                self.sample(index, literal.replace('\\n', '\n'))
            except BaseException:
                self.capture_failure_terminals()
                raise
        def quiet():
            records = self.model_wire.records + self.backend_wire.records
            return records and all('ended' in record for record in records) and time.monotonic() - max(record['ended'] for record in records) >= 1.25
        until('all actual model and tool responses settled', quiet, self.args.timeout)
        require(all(record.get('status') == 200 and not record.get('error') for record in self.model_wire.records), 'A model transport failure occurred; subsequent decisions cannot hide it')
        support = {
            'P04': ['samples.managerTaskId', 'samples.zone', 'samples.rawInput', 'nativeHeaders', 'histories/'],
            'P06': ['samples.calls.execution', 'samples.retainedOldExecution', 'samples.final'],
            'P17': ['samples.calls.finalReport', 'samples.calls.file', 'backend-wire/index.json'],
            'P20': ['model-wire/index.json', 'histories/', 'backend-wire/index.json'],
            'P21': ['samples.calls.execution', 'samples.calls.child.run.parent', 'backend-wire/index.json'],
            'P22': ['samples.retainedOldExecution', 'samples.calls.child', 'backend-wire/index.json'],
            'P23': ['samples.calls.prefinish', 'samples.calls.progress', 'backend-wire/index.json'],
            'P24': ['samples.calls.finalReport', 'samples.final', 'backend-wire/index.json'],
            'P25': ['histories/', 'samples.calls.child', 'samples.calls.finalReport'],
            'P34': ['userInputs', 'histories/', 'samples.calls.execution.reportPolicy', 'samples.calls.child'],
            'P35': ['samples.calls.tabsAfterFinish', 'samples.calls.tabsAfterParentComplete', 'nativeProcesses', 'backend-wire/index.json'],
        }
        for promise, evidence in support.items():
            self.evidence['coverage'][promise] = {'verified': True, 'host': self.host, 'samples': [0, 1], 'evidence': evidence}
        self.evidence['passed'] = True

    def sample(self, index, literal):
        roles = DECLARED_ROLES[index] if self.host == 'declared' else dict.fromkeys(['parent', 'child', 'worker'], self.host)
        zone = self.root / f'business-{index}'
        zone.mkdir()
        workspace = self.herdr('workspace', 'create', '--cwd', str(zone), '--label', zone.name, '--no-focus')
        self.evidence.setdefault('businessWorkspaces', []).append(workspace)
        zone_id = f'roles/{self.host}/{index}'
        self.http('/api/zones', {'id': zone_id, 'path': str(zone)})
        marker = f'RAW-{self.host}-{index}-BUSINESS-APPROVED'
        parent_output = ['outer-alpha', 'outer-beta'][index]
        worker = self.task(f'worker-{index}', 'normal',
            'Perform only the original business input. Use real native file tools to write a NEW uniquely named UTF-8 file in your working directory with exactly the assigned bytes. '
            'Report with can_complete=true and outputs=[{id:"result",path:absolute actual file path}]. The internal report_policy is auto: do not ask for send approval. Do not orchestrate, read test drivers, runtime records, credentials or other Chats.', roles['worker'])
        child = self.task(f'child-manager-{index}', 'manager',
            'You have BOTH actual workflow roles. run/collect manage only your internal Run; report sends only to your parent execution. Use ONLY these native tools for workflow operations, never HTTP/bash substitutions. '
            'Business work is approved but manual report sending is NOT approved. Initially inspect run status, read your parent output requirements, explain the result/limitation and ask for send confirmation, then stop with REQUEST_SEND_APPROVAL_1. '
            'After SEND_APPROVAL_1, make exactly one can_complete=true report diagnostic BEFORE dispatch; it must be refused because the child is unfinished. '
            'Dispatch your internal node first, collect its actual report, accept it and explicitly complete your internal node first. Then show its result and ask for a separate diagnostic send confirmation, stopping with REQUEST_SEND_APPROVAL_1B. '
            'After SEND_APPROVAL_1B make exactly one more can_complete=true diagnostic report BEFORE finish; it must again be refused. Then finish the internal Run. Do NOT exit this Chat and do NOT report automatically. Explain the actual result address and ask a NEW send confirmation, stopping with REQUEST_SEND_APPROVAL_2. '
            'After SEND_APPROVAL_2, report actual handoff with can_complete=false first, then show the final summary/address/limitations and ask for a new send confirmation, stopping with PROGRESS_REPORTED. Wait for FINAL_SEND_APPROVAL before reporting can_complete=true with the output ID from your ACTUAL PARENT output requirements (not the internal worker output ID) and the real worker file address. After the final report succeeds, acknowledge its actual receipt and stop with FINAL_REPORTED. Do not invent files or copy detailed contents into reports.', roles['child'])
        parent = self.task(f'parent-manager-{index}', 'manager',
            'Use native run/collect for orchestration only. Dispatch first, then stop with PARENT_DISPATCHED; do not complete or finish automatically. '
            'When explicitly asked RETRY_FIRST, explicitly fail first then retry first with confirmed=true; this replacement is business-approved, and stop with RETRY_DISPATCHED. '
            'When explicitly asked CHECK_PROGRESS for a named node, attempt complete for that node exactly ONCE as an authorized diagnostic while latest report is false; inspect the actual backend refusal, do not retry, and stop with PROGRESS_REFUSED. '
            'When explicitly asked ACCEPT_FIRST, collect the latest report, explicitly accept current latest true report and complete first, repeat complete first once to exercise idempotence, dispatch second, then stop with SECOND_DISPATCHED. '
            'When explicitly asked ACCEPT_SECOND, collect and accept latest report, complete second and finish, then acknowledge the successful finish with ROOT_FINISHED. Without an explicit acceptance message acknowledge notifications only; never advance implicitly. Before each FIRST successful complete, read the actual CURRENT LATEST report and include its report ID in your complete reason, so acceptance is attributable. Inspect current reports through run status or the supplied public workflow record paths; never inspect driver scripts/private persistence or call HTTP directly.', roles['parent'])
        def save_json(path, value):
            path.write_text(json.dumps(value, ensure_ascii=False), encoding='utf-8')
        self.http('/api/scheduler/bots/save', {'id': f'worker-{index}', 'content': json.dumps({'name': 'Worker', 'task_id': worker, 'when': '', 'plan_guide': '', 'node_guide': ''}), 'revision': None})
        save_json(self.root / f'maps/inner-{index}.json', {'title': 'Repeated inner', 'description': '', 'nodes': {'first': {
            'title': 'Business file', 'bot_id': f'worker-{index}', 'zone': zone_id, 'blocked_by': [], 'auto_close': False,
            'report_policy': 'auto', 'input': marker + '\nWrite exactly these UTF-8 bytes: ' + json.dumps(literal), 'output_rule': [{'id': 'result'}]}}})
        self.http('/api/scheduler/bots/save', {'id': f'nested-{index}', 'content': json.dumps({'name': 'Nested', 'map_file': f'inner-{index}.json', 'when': '', 'plan_guide': '', 'node_guide': ''}), 'revision': None})
        save_json(self.root / f'maps/outer-{index}.json', {'title': 'Outer', 'description': '', 'nodes': {}})
        for node in ['first', 'second']:
            content = {'title': node, 'bot_id': f'nested-{index}', 'manager_task_id': child, 'zone': zone_id,
                'blocked_by': [], 'report_policy': 'manual', 'auto_close': True,
                'input': marker + '\nOriginal approved business bytes: ' + json.dumps(literal), 'output_rule': [{'id': parent_output}]}
            draft = self.http(f'/api/scheduler/maps/outer-{index}.json/nodes/new')
            self.http(f'/api/scheduler/maps/outer-{index}.json/nodes/save', {'id': node, 'content': json.dumps(content), 'revision': draft['revision'], 'create': True})
        raw_input = marker + '\nBusiness execution is approved. This is NOT approval to send any manual report.'
        started = self.http(f'/api/scheduler/maps/outer-{index}.json/runs', {'managerTaskId': parent, 'zone': zone_id, 'userInput': raw_input}, 201)
        run_id = started['id']
        parent_chat = started['run']['manager']
        self.chat_hosts[parent_chat['sessionId']] = roles['parent']
        sample = {'declaredRoles': roles, 'rawInput': raw_input, 'runId': run_id, 'managerTaskId': parent, 'zone': zone_id, 'expectedBytes': literal, 'calls': []}
        self.evidence['samples'].append(sample)
        for node in ['first', 'second']:
            def child_started():
                detail = self.detail(run_id)
                execution = detail['run']['nodes'][node].get('execution')
                if execution and execution.get('childRunId') and execution.get('chat'):
                    self.chat_hosts[execution['chat']['sessionId']] = roles['child']
                    return execution
                return None
            execution = until('real nested child dispatched', child_started, self.args.timeout)
            if node == 'first':
                self.await_approval(execution['chat'], 'REQUEST_SEND_APPROVAL_1')
                self.await_note(parent_chat, 'PARENT_DISPATCHED')
                old = execution
                self.send(parent_chat, 'RETRY_FIRST: Explicit business approval to fail first and retry it exactly once with confirmed=true. Preserve the old Chat and execution; this is not manual report-send approval.', 'business-replacement-approval')
                execution = until('new native child with retained old execution', lambda: (e if (e := child_started()) and e['id'] != old['id'] else None), self.args.timeout)
                sample['retainedOldExecution'] = old
                require(any(item['execution']['id'] == old['id'] for item in self.detail(run_id)['run']['nodes'][node]['previousExecutions']), 'Old execution was not retained')
            chat = execution['chat']
            child_id = execution['childRunId']
            before = self.await_approval(chat, 'REQUEST_SEND_APPROVAL_1')
            parent_detail = self.detail(run_id)
            require(not any(r['executionId'] == execution['id'] for r in parent_detail['reports']), 'Business approval incorrectly authorized manual sending')
            child_run = self.detail(child_id)['run']
            require(child_run['parent'] == {'runId': run_id, 'nodeId': node, 'executionId': execution['id']}, 'Child parent link is wrong')
            require(child_run['managerTaskId'] == child and Path(child_run['zone']).resolve() == zone.resolve(), 'Wrong actual child manager source or work location')
            require(child_run['userInput'] == marker + '\nOriginal approved business bytes: ' + json.dumps(literal), 'Raw original Map-call business input changed')
            require(marker in json.dumps(before, ensure_ascii=False), 'Original business input absent from actual child history')
            require(Path(self.evidence['nativeHeaders'][chat['sessionId']]['cwd']).resolve() == zone.resolve(), 'Actual native child working directory differs from specified zone')
            require(execution['taskId'] == child and execution['reportPolicy'] == 'manual', 'Wrong manager source or external policy')
            self.observe_processes()
            self.send(chat, 'SEND_APPROVAL_1: I authorize sending only the initial unfinished-child can_complete=true diagnostic now. Business approval was separate. After this refusal do the internal work, then ask before any second diagnostic or final report.', 'send-approval-initial-diagnostic')
            self.await_note(chat, 'REQUEST_SEND_APPROVAL_1B')
            prefinish = self.detail(child_id)
            require(prefinish['run']['status'] != 'completed' and prefinish['run']['nodes']['first']['status'] == 'completed', 'Scenario did not reach all-complete-but-not-finished boundary')
            actual_file = prefinish['reports'][-1]['outputs'][0]['path']
            self.await_approval(chat, 'REQUEST_SEND_APPROVAL_1B', actual_file)
            self.send(chat, 'SEND_APPROVAL_1B: I authorize only the all-complete-but-not-finished can_complete=true diagnostic now. After its refusal finish internally and ask again; no progress or handoff send is authorized yet.', 'send-approval-prefinish-diagnostic')
            finished_history = self.await_approval(chat, 'REQUEST_SEND_APPROVAL_2', actual_file)
            finished = self.detail(child_id)
            require(finished['run']['status'] == 'completed', 'Native manager did not finish internal Run')
            inside = finished['run']['nodes']['first']
            require(inside['status'] == 'completed' and inside['execution']['reportPolicy'] == 'auto', 'Outer manual changed inner policy or internal work incomplete')
            require(finished['reports'] and finished['reports'][-1]['canComplete'] is True, 'Internal automatic report missing')
            current = self.detail(run_id)
            require(current['run']['nodes'][node]['status'] == 'running', 'finish implicitly completed parent')
            require(not any(r['executionId'] == execution['id'] for r in current['reports']), 'Unfinished diagnostics were saved or finish auto-reported')
            tabs_before = self.tabs(chat['workspaceId'])
            tab_ids = {tab['tab_id'] for tab in tabs_before}
            internal_tab = inside['execution']['chat']['tabId']
            require(chat['tabId'] in tab_ids and internal_tab in tab_ids, 'finish closed manager or retained internal Tab')
            self.observe_processes()
            self.send(chat, 'SEND_APPROVAL_2: I approve sending the actual concise progress report with can_complete=false now. This does NOT authorize completion or the later true report. Then stop with PROGRESS_REPORTED.', 'send-approval-progress')
            self.await_approval(chat, 'PROGRESS_REPORTED', actual_file)
            progress = self.detail(run_id)
            report = [r for r in progress['reports'] if r['executionId'] == execution['id']][-1]
            require(report['canComplete'] is False and progress['run']['nodes'][node]['status'] == 'running', 'Progress granted completion eligibility')
            self.send(parent_chat, f'CHECK_PROGRESS for node {node}: explicitly attempt complete exactly once as the authorized diagnostic now; inspect the latest false-report refusal, do not retry or dispatch, then stop with PROGRESS_REFUSED.', 'parent-progress-gate-diagnostic')
            self.await_note(parent_chat, 'PROGRESS_REFUSED')
            progress_refusals = [record for record in self.backend_wire.records if record['path'].endswith('/manager/complete') and (decode(record, 'request') or {}).get('sessionId') == parent_chat['sessionId'] and (decode(record, 'request') or {}).get('nodeId') == node and (decode(record, 'response') or {}).get('code') == 'report-not-completable']
            require(len(progress_refusals) == 1 and report['id'] in (decode(progress_refusals[0], 'response') or {}).get('message', ''), 'Actual progress false report incorrectly granted parent completion eligibility')
            require(self.detail(run_id)['run']['nodes'][node]['status'] == 'running', 'Progress diagnostic changed parent state')
            self.send(chat, 'FINAL_SEND_APPROVAL: I approve sending the final actual-file handoff report now, with can_complete=true and the output ID required by your actual parent execution. Do not complete the parent or exit.', 'send-approval-final')
            def reported():
                detail = self.detail(run_id)
                reports = [r for r in detail['reports'] if r['executionId'] == execution['id']]
                return detail if reports and reports[-1]['canComplete'] is True else None
            ready = until('real post-finish true report', reported, self.args.timeout)
            final_report = [r for r in ready['reports'] if r['executionId'] == execution['id']][-1]
            require(final_report['runId'] == run_id and final_report['nodeId'] == node and final_report['from']['sessionId'] == chat['sessionId'] and final_report['to']['sessionId'] == parent_chat['sessionId'], 'Actual report was not bound to its unique parent execution')
            require(ready['run']['nodes'][node]['status'] == 'running', 'report implicitly completed parent')
            outputs = final_report.get('outputs', [])
            require(len(outputs) == 1 and outputs[0]['id'] == parent_output, 'Real manager reported the internal output ID instead of its actual parent requirement')
            require(outputs[0]['path'] == finished['reports'][-1]['outputs'][0]['path'], 'Outer report did not register the actual internally produced file')
            output = Path(outputs[0]['path'])
            require(output.is_relative_to(zone) and output.read_bytes() == literal.encode(), 'Actual business file bytes differ from independent original input')
            after_history = self.await_note(chat, 'FINAL_REPORTED')
            require(after_history and len(after_history['branch']) > len(finished_history['branch']), 'Chat cannot independently report after finish')
            self.verify_wire(run_id, child_id, execution, after_history)
            sample['calls'].append({'node': node, 'execution': execution, 'child': finished, 'prefinish': prefinish, 'progress': report, 'finalReport': final_report,
                'file': {'path': str(output), 'bytesHex': output.read_bytes().hex()}, 'tabsAfterFinish': tabs_before})
            self.send(parent_chat, ('ACCEPT_FIRST' if node == 'first' else 'ACCEPT_SECOND') + ': Inspect and collect the current latest true report, explicitly accept its actual registered result and complete the designated node now. Follow your existing stage instructions.', 'parent-business-acceptance')
            def removed():
                tabs = self.tabs(chat['workspaceId'])
                return tabs if chat['tabId'] not in {t['tab_id'] for t in tabs} else None
            after_tabs = until('parent complete closes exact target Tab', removed, self.args.timeout)
            after_ids = {tab['tab_id'] for tab in after_tabs}
            require(internal_tab in after_ids, 'Parent completion cascaded to retained internal Tab')
            require(self.sentinel in {tab['tab_id'] for tab in self.tabs(self.workspace)}, 'Parent completion closed the unrelated sentinel Tab')
            require(tab_ids - after_ids == {chat['tabId']}, 'Parent completion removed unrelated Tabs')
            require(self.detail(run_id)['run']['nodes'][node]['status'] == 'completed', 'No explicit parent completion')
            accepted = [r for r in self.backend_wire.records if r['path'].endswith('/manager/complete') and (decode(r, 'request') or {}).get('sessionId') == parent_chat['sessionId'] and (decode(r, 'request') or {}).get('nodeId') == node and (decode(r, 'response') or {}).get('changed') is True]
            require(accepted and final_report['id'] in (decode(accepted[0], 'request') or {}).get('reason', ''), 'Parent did not explicitly accept the current latest actual report')
            sample['calls'][-1]['tabsAfterParentComplete'] = after_tabs
        final = until('root manager explicitly finished', lambda: (d if (d := self.detail(run_id))['run']['status'] == 'completed' else None), self.args.timeout)
        sample['final'] = final
        self.await_note(parent_chat, 'ROOT_FINISHED')
        require(final['run']['userInput'] == raw_input, 'Root original business input changed')
        parent_completes = [record for record in self.backend_wire.records if record['path'].endswith('/manager/complete')
            and (decode(record, 'request') or {}).get('sessionId') == parent_chat['sessionId'] and (decode(record, 'request') or {}).get('nodeId') == 'first' and not (decode(record, 'response') or {}).get('code')]
        require(len(parent_completes) == 2 and (decode(parent_completes[-1], 'response') or {}).get('changed') is False, 'Actual parent did not perform exactly one idempotent repeat complete')
        require(len({call['execution']['childRunId'] for call in sample['calls']}) == 2, 'Repeated Map calls reused child Run')
        require(all(self.detail(call['execution']['childRunId'])['run']['status'] == 'completed' for call in sample['calls']), 'Parent advancement changed child facts')

    def verify_wire(self, parent_id, child_id, execution, after):
        session = execution['chat']['sessionId']
        calls = [r for r in self.backend_wire.records if (decode(r, 'request') or {}).get('sessionId') == session]
        reports = [r for r in calls if r['path'].endswith('/node/report')]
        refusals = [r for r in reports if (decode(r, 'response') or {}).get('code') == 'child-run-not-finished']
        require(len(refusals) == 2, 'Both unfinished and all-complete-before-finish true reports must be actually refused')
        require(len(reports) == 4 and len([r for r in calls if r['path'].endswith('/dispatch')]) == 1 and len([r for r in calls if r['path'].endswith('/complete')]) == 1, 'Unexpected repeated native report or internal business decision; no decision retries are accepted')
        for record in calls:
            body = decode(record, 'request') or {}
            require(not any(key in body for key in ['runId', 'executionId']) and (record['path'].endswith(('/complete', '/retry', '/fail')) or 'nodeId' not in body), 'Model supplied authorization identity instead of native injection')
            response = decode(record, 'response') or {}
            if '/manager/' in record['path'] and 'snapshot' in response:
                require(response['snapshot']['id'] == child_id, 'run/collect resolved the parent or another Run instead of this child')
            if record in reports and response.get('saved'):
                require(response['report']['runId'] == parent_id and response['report']['executionId'] == execution['id'], 'report resolved a different parent execution')
        paths = {r['path'] for r in calls}
        require('/api/workflow/manager/dispatch' in paths and '/api/workflow/manager/collect' in paths and '/api/workflow/node/report' in paths, 'Same real host Session did not call run/collect/report')
        finish = next(r for r in calls if r['path'].endswith('/finish'))
        completes = [r for r in calls if r['path'].endswith('/complete')]
        require(refusals[0]['started'] < completes[0]['started'] < refusals[1]['started'] < finish['started'], 'Required lifecycle diagnostic ordering not observed')
        guides = [(self.args.toolkit / f'extensions/workflow-chat/{role}-chat/shared-logic/prompts/init.md').read_text(encoding='utf-8').strip() for role in ['manager', 'node']]
        co_loaded = [r for r in self.model_wire.records if all(guide in '\n'.join(RolesAcceptance.message_text(m).replace('\r\n', '\n') for m in shared['messages'](r)) for guide in guides)]
        require(co_loaded, 'Real model never received both role guides in one actual Session request')
        require(any({'run', 'collect', 'report'} <= {tool.get('function', {}).get('name') for tool in (decode(r, 'request') or {}).get('tools', [])} for r in co_loaded), 'Three real tools and both guides did not coexist in an actual provider request')
        user_messages = [RolesAcceptance.message_text(entry['message']) for entry in after['branch'] if entry.get('type') == 'message' and entry['message'].get('role') == 'user']
        confirmations = [entry for entry in self.evidence['userInputs'] if entry['sessionId'] == session and entry['kind'].startswith('send-approval')]
        require(len(confirmations) == 4 and all(any(entry['input'] in actual for actual in user_messages) for entry in confirmations), 'Original send confirmations were not actually delivered as native user messages')
        require(all(report['started'] >= confirmation['at'] for report, confirmation in zip(reports, confirmations)), 'An external report preceded its own explicit user send approval')
        for name in ['run', 'collect', 'report']:
            require(any(part.get('type') == 'toolCall' and part.get('name') == name for e in after['branch'] if e.get('type') == 'message' for part in e['message'].get('content', []) if isinstance(part, dict)), ('No actual native tool call', name))

    def cleanup_extra(self):
        errors = []
        for pid, handle in self.handles:
            try:
                until('owned OS process exits: ' + str(pid), lambda: not handle.alive(), 15)
                self.evidence.setdefault('processExitConfirmations', []).append({'pid': pid, 'exited': True, 'observation': 'held Windows process handle'})
            except BaseException:
                errors.append(traceback.format_exc())
                try:
                    if handle.alive():
                        result = subprocess.run(['taskkill', '/PID', str(pid), '/T', '/F'], capture_output=True, text=True, timeout=45)
                        self.evidence.setdefault('forcedCleanup', []).append({'pid': pid, 'exit': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr})
                        until('forced owned process exit: ' + str(pid), lambda: not handle.alive(), 15)
                except BaseException:
                    errors.append(traceback.format_exc())
            finally:
                handle.close()
        for wire in [self.backend_wire, self.model_wire]:
            if wire:
                try:
                    wire.close()
                except BaseException:
                    errors.append(traceback.format_exc())
        for name in ['models.json', 'models.yml']:
            (self.root / 'agent' / name).unlink(missing_ok=True)
        require(not errors, errors)


def parser():
    cli = argparse.ArgumentParser(description=__doc__)
    for name in ['backend', 'toolkit', 'paths-file', 'model-config', 'evidence-dir']:
        cli.add_argument('--' + name, type=Path, required=True)
    cli.add_argument('--hosts', choices=['omp,pi', 'declared', 'omp', 'pi'], required=True)
    for name in ['herdr', 'pi-package', 'omp-package', 'pi-state-extension', 'omp-state-extension', 'shell-path']:
        cli.add_argument('--' + name, type=Path)
    cli.add_argument('--node', default='node')
    cli.add_argument('--bun', default='bun')
    cli.add_argument('--timeout', type=int, default=300)
    return cli


def main():
    cli = parser()
    args = cli.parse_args()
    for key in list(os.environ):
        if 'PROXY' in key.upper():
            os.environ.pop(key)
    args.evidence_dir = args.evidence_dir.resolve()
    args.evidence_dir.mkdir(parents=True, exist_ok=False)
    evidence = {'passed': False, 'requestedHosts': [] if args.hosts == 'declared' else args.hosts.split(','),
                'processes': [], 'verified': [], 'cleanupErrors': []}
    try:
        require(args.model_config.is_file(), ('Missing explicit authorized model configuration', str(args.model_config)))
        model = json.loads(args.model_config.read_text(encoding='utf-8'))
        require(model.get('apiKeyEnv') and os.environ.get(model['apiKeyEnv']), 'Missing explicit authorized model credential environment')
        resolve_arguments(cli, args, {'herdr': ('herdr', 'herdr.exe'), 'pi_package': ('pi_package', ''),
            'omp_package': ('omp_package', ''), 'pi_state_extension': ('pi_agent', 'extensions/herdr-agent-state.ts'),
            'omp_state_extension': ('omp_agent', 'extensions/herdr-omp-agent-state.ts')})
        require(args.backend.resolve() == (REPO / backend_suffix()).resolve(), 'Backend must be this worktree build in its independent CARGO_TARGET_DIR')
        args.repo = REPO
        args.node = shutil.which(args.node) or args.node
        args.bun = shutil.which(args.bun) or args.bun
        args.toolkit = args.toolkit.resolve()
        args.backend = args.backend.resolve()
        if args.hosts == 'declared':
            evidence['requestedHosts'] = sorted({host for roles in DECLARED_ROLES for host in roles.values()})
            evidence['declaredRoles'] = DECLARED_ROLES
        for host in (['declared'] if args.hosts == 'declared' else evidence['requestedHosts']):
            args.host = host
            case = DualRoleAcceptance(args)
            code = case.run()
            archived = args.evidence_dir / host
            shared['copy_evidence'](case.root, archived)
            evidence['processes'].extend(case.evidence.get('nativeProcesses', []))
            evidence['cleanupErrors'].extend({'host': host, 'error': error} for error in case.evidence.get('cleanupErrors', []))
            evidence.setdefault('hosts', {})[host] = {'exit': code, 'evidence': str(archived / 'evidence.json'), 'nativeRoot': str(case.root)}
            if code == 0:
                evidence['verified'].extend(case.hosts)
        evidence['passed'] = set(evidence['verified']) == set(evidence['requestedHosts'])
        if not evidence['passed']:
            evidence['verificationError'] = 'One or more real host lifecycles failed; see each host evidence. No host result substitutes for another.'
    except BaseException:
        evidence['verificationError'] = traceback.format_exc()
    finally:
        (args.evidence_dir / 'evidence.json').write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding='utf-8')
        print(json.dumps(evidence, ensure_ascii=False, indent=2))
    return 0 if evidence['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
