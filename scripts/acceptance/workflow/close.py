"""P22: real Herdr effects via workflow dispatch and manager complete (Windows).

The approved seams are S2 (production HTTP) and S3 (installed Herdr CLI).
Independent outcome: the exact execution Tab disappears, a live child in that
Tab exits, the sentinel Tab survives, and native Pi history still contains both
our unique input and the controlled model's literal reply. A no-op/wrong-target
close can leave workflow completed but must fail these external observations.
No test adapter or direct tab-close command participates in the assertion path.
"""
import hashlib
import http.server
import json
from pathlib import Path
import subprocess
import sys
import threading
import uuid

from developer_paths import REPO
from acceptance.host.isolation import Acceptance as HostAcceptance
from acceptance.host.processes import LiveProcess, require, until
from acceptance.native.session_storage import Model


class Acceptance(HostAcceptance):
    windows_requirement = 'P22 runner currently requires Windows process handles'

    def __init__(self, args, *, prefix='herdr-close-p22-'):
        super().__init__(args, prefix=prefix, promise='P22', session_prefix='p22-')

    def configure_model(self):
        """P22's controlled model; role acceptance overrides this with a real provider."""
        self.model = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Model)
        threading.Thread(target=self.model.serve_forever, daemon=True).start()
        agent = self.root / 'agent'
        (agent / 'settings.json').write_text(json.dumps({'sessionDir': str(agent / 'sessions')}))
        (agent / 'models.json').write_text(json.dumps({'providers': {'storage-local': {
            'baseUrl': f'http://127.0.0.1:{self.model.server_port}/v1', 'api': 'openai-completions',
            'apiKey': 'isolated-not-a-real-key', 'models': [{'id': 'storage-probe', 'name': 'P22 probe',
            'reasoning': False, 'input': ['text'], 'contextWindow': 32768, 'maxTokens': 1000,
            'cost': {'input': 0, 'output': 0, 'cacheRead': 0, 'cacheWrite': 0}}]}}}))
        self.provider_model = 'storage-local/storage-probe'

    def role_catalog(self, toolkit):
        catalog = 'skills = []\n[[agents]]\nid = "pi"\nlabel = "Pi"\n'
        catalog += '[[models]]\nid = "default"\nlabel = "Default"\n'
        catalog += '[[models]]\nid = "probe"\nlabel = "Probe"\nprovider_model = ' + json.dumps(self.provider_model) + '\nthinking = { pi = ["off"] }\n'
        for id, path in [('pi-herdr-agent-state', self.args.pi_state_extension),
                         ('pi-manager-chat', toolkit / 'entrypoints/pi-auto/manager-chat.ts'),
                         ('pi-node-chat', toolkit / 'entrypoints/pi-auto/node-chat.ts')]:
            catalog += f'[[extensions]]\nid = "{id}"\nlabel = "{id}"\nagent = "pi"\nkind = "fixed"\nentrypoint = {json.dumps(str(path))}\n'
        return catalog

    def exercise(self):
        zone = str(self.root / 'zone')
        self.http('/api/zones', {'id': 'acceptance/close', 'path': zone})
        ids = {}
        for role in ['manager', 'node']:
            result = self.http('/api/tasks', {'name': role, 'taskType': 'manager' if role == 'manager' else 'normal',
                'agent': 'pi', 'chatName': '{chat.id}', 'model': 'probe', 'thinking': 'default',
                'skill': None, 'prompt': 'Acknowledge only. Do not call tools.'})
            ids[role] = next(task['id'] for task in result['tasks'] if task['name'] == role)
        marker = 'P22-HISTORY-' + uuid.uuid4().hex
        (self.root / 'bots/worker.json').write_text(json.dumps({'id': 'worker', 'name': 'Worker',
            'when': '', 'plan_guide': '', 'node_guide': '', 'task_id': ids['node']}))
        (self.root / 'maps/close.json').write_text(json.dumps({'title': 'P22', 'description': '', 'nodes': {
            'A': {'title': 'Owned target', 'bot_id': 'worker', 'input': marker, 'zone': 'acceptance/close',
                  'blocked_by': [], 'auto_close': True}}}))
        started = self.http('/api/scheduler/maps/close.json/runs',
                            {'managerTaskId': ids['manager'], 'zone': 'acceptance/close', 'userInput': 'Acknowledge only.'}, 201)
        manager = started['run']['manager']['sessionId']
        prepared = self.http('/api/workflow/manager/prepare_inputs', {'sessionId': manager, 'fileInputs': [],
                             'confirmed': True, 'reason': 'Controlled close fixture text reviewed; no file inputs'})
        require((prepared.get('changed') or prepared.get('stateCommitted')) and prepared['snapshot']['inputsReady']
                    and not prepared.get('error') and not prepared.get('historyError'), prepared)
        dispatched = self.http('/api/workflow/manager/dispatch', {'sessionId': manager, 'nodes': ['A']})
        node = dispatched['snapshot']['nodes']['A']
        require(node['status'] == 'running', node)
        execution = node['execution']
        require(execution['autoClose'] is True, execution)
        chat = execution['chat']
        require(chat['tabId'] != self.sentinel and chat['workspaceId'] == self.workspace, chat)
        self.http('/api/sessions/history')
        module = (self.args.pi_package / 'dist/core/session-manager.js').as_uri()
        code = f'import {{SessionManager}} from {json.dumps(module)}; console.log(JSON.stringify(await SessionManager.list({json.dumps(zone)}, {json.dumps(str(self.root / "agent/sessions"))})));'
        records = json.loads(self.command([self.args.node, '--input-type=module', '-e', code]))
        file = next(record['path'] for record in records if record['id'] == chat['sessionId'])
        require(Path(file).is_relative_to(self.root), ('Native history outside owned root', file))
        def history_ready():
            if not Path(file).is_file():
                return None
            text = Path(file).read_text(encoding='utf-8')
            return marker in text and 'isolated storage acknowledgement' in text
        until('native user and assistant history persisted', history_ready)
        before_history = self.history(file)
        require(before_history['id'] == chat['sessionId'], before_history)
        messages = [entry['message'] for entry in before_history['branch'] if entry['type'] == 'message']
        require(any(message['role'] == 'user' and marker in json.dumps(message['content']) for message in messages), before_history)
        require(any(message['role'] == 'assistant' and 'isolated storage acknowledgement' in json.dumps(message['content']) for message in messages), before_history)
        split = self.herdr('pane', 'split', chat['paneId'], '--direction', 'down', '--cwd', zone, '--no-focus')
        require(split['pane']['tab_id'] == chat['tabId'], ('Controlled pane in wrong Tab', split))
        controlled_pane = split['pane']['pane_id']
        pidfile = self.root / 'controlled-pid.json'
        command = subprocess.list2cmdline([sys.executable, str(REPO / 'scripts/test-real-herdr-close.py'), '--controlled-process', str(pidfile)])
        self.command([self.args.herdr, 'pane', 'run', controlled_pane, command])
        until('controlled child started in target Tab', pidfile.is_file)
        pid = json.loads(pidfile.read_text())['pid']
        self.controlled = LiveProcess(pid)
        require(self.controlled.alive(), 'Controlled process must be alive before complete')
        before_tabs = self.herdr('tab', 'list', '--workspace', self.workspace)
        before_ids = {tab['tab_id'] for tab in before_tabs['tabs']}
        require(chat['tabId'] in before_ids and self.sentinel in before_ids, before_tabs)
        self.evidence['before'] = {'runId': started['id'], 'execution': execution, 'tabs': before_tabs,
            'controlledPane': controlled_pane, 'controlledPid': pid, 'controlledAlive': True,
            'nativeHistory': before_history, 'marker': marker,
            'nativeSha256': hashlib.sha256(Path(file).read_bytes()).hexdigest()}
        declaration = self.http('/api/workflow/node/report', {'sessionId': chat['sessionId'],
            'body': 'P22 controlled acceptance: native history persisted; target terminal independently observed alive.',
            'canComplete': True})
        require(declaration['saved'] is True, declaration)
        self.evidence['completionDeclaration'] = declaration
        completed = self.http('/api/workflow/manager/complete', {'sessionId': manager, 'nodeId': 'A',
            'reason': 'P22: independently observed native history and live controlled terminal.'})
        require(completed['snapshot']['nodes']['A']['status'] == 'completed', completed)
        def removed():
            tabs = self.herdr('tab', 'list', '--workspace', self.workspace)
            return tabs if chat['tabId'] not in {tab['tab_id'] for tab in tabs['tabs']} else None
        after_tabs = until('execution Tab removed by business complete', removed)
        after_ids = {tab['tab_id'] for tab in after_tabs['tabs']}
        require(after_ids == before_ids - {chat['tabId']}, ('Unrelated Tab identity changed', before_tabs, after_tabs))
        until('controlled target terminal process terminated', lambda: not self.controlled.alive())
        after_history = self.history(file)
        require(after_history == before_history, 'Native SessionManager history changed or was deleted on close')
        self.evidence['after'] = {'tabs': after_tabs, 'controlledAlive': self.controlled.alive(),
            'nativeHistory': after_history, 'nativeSha256': hashlib.sha256(Path(file).read_bytes()).hexdigest()}
        self.evidence['passed'] = True

