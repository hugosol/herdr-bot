"""Owned safe-backend workflow smoke setup and public HTTP operations."""
import json
import os
from pathlib import Path
import queue
import re
import shutil
import subprocess
import tempfile
import threading
import urllib.error
import urllib.request

from acceptance.host.smoke_support import dump, require

REPO = Path(__file__).resolve().parents[3]


class Smoke:
    def __init__(self, target):
        self.root = Path(tempfile.mkdtemp(prefix='workflow-file-consumption-'))
        self.child = None
        self.readers = []
        self.evidence = {'root': str(self.root), 'scenarios': [], 'errors': [],
                         'boundary': 'safe backend process / HTTP / real files / controlled host, not native Chat consumption'}
        self.target = target.resolve()
        self.opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        self.wire = []

    def calls(self):
        path = self.root / 'host/calls.jsonl'
        return [json.loads(line) for line in path.read_text(encoding='utf-8').splitlines()] if path.exists() else []

    def http(self, route, body=None, status=200):
        request = urllib.request.Request(self.base + route, data=None if body is None else json.dumps(body).encode(),
                                         headers={'Content-Type': 'application/json'})
        try:
            response = self.opener.open(request, timeout=30)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            raw = response.read().decode('utf-8')
            self.wire.append({'route': route, 'request': body, 'status': response.status, 'body': raw})
            require(response.status == status, self.wire[-1])
            return json.loads(raw) if raw else None

    def identity(self, name, node=False):
        dump(self.root / 'host/behavior.json', {'native_session': {'source': 'herdr:omp', 'agent': 'omp', 'kind': 'id', 'value': name},
                                               'native_cwd': str(self.root / ('node' if node else 'manager'))})

    def prepare(self):
        for directory in ['host', 'manager', 'node', 'maps', 'bots', 'business']:
            (self.root / directory).mkdir()
        for name in ['herdr-fixture.exe', 'examples/safe_shutdown_shell.exe']:
            require((self.target / 'debug' / name).is_file(), ('build current worktree binary first', name))
        require((REPO / 'backend/static/index.html').is_file(), 'build this worktree frontend first')
        shutil.copy2(self.target / 'debug/herdr-fixture.exe', self.root / 'host/herdr-fixture.exe')
        dump(self.root / 'host/workspaces.json', [{'workspace_id': 'owned', 'label': 'manager'}])
        self.identity('initial')
        index = 0
        def entry(_match):
            nonlocal index
            path = self.root / f'extension-{index}.ts'
            index += 1
            path.write_text('export default function () {}\n', encoding='utf-8')
            return "entrypoint = '" + path.as_posix() + "'"
        catalog = re.sub(r'^entrypoint\s*=.*$', entry, (REPO / 'meta-data/task-options.toml').read_text(encoding='utf-8'), flags=re.M)
        (self.root / 'options.toml').write_text(catalog, encoding='utf-8')
        (self.root / 'panel.toml').write_text(f"""listen_addr = '127.0.0.1:0'
data_directory = 'data'
static_directory = '{(REPO / 'backend/static').as_posix()}'
[operations.start]
script = 'unused.bat'
working_directory = '.'
[operations.stop]
script = 'unused.bat'
working_directory = '.'
[mdns]
enabled = false
[herdr]
workspace_name = 'manager'
executable = '{(self.root / 'host/herdr-fixture.exe').as_posix()}'
[zones]
registry_file = 'zones.json'
[tasks]
storage_file = 'tasks.json'
metadata_file = 'options.toml'
[sessions]
history_file = 'history.json'
[scheduler]
map_directory = 'maps'
bot_directory = 'bots'
run_directory = 'runs'
run_instructions = '{(REPO / 'docs/workflow-runtime.md').as_posix()}'
""", encoding='utf-8')
        env = os.environ.copy()
        env['HERDR_FIXTURE_DIR'] = str(self.root / 'host')
        env['HERDR_BOT_FIXTURE_RECORD_DIR'] = str(self.root / 'host')
        self.child = subprocess.Popen([str(self.target / 'debug/examples/safe_shutdown_shell.exe'), '--config', 'panel.toml'],
                                      cwd=self.root, env=env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        addresses = queue.Queue()
        def drain(stream, name):
            with stream, (self.root / name).open('wb') as log:
                for line in iter(stream.readline, b''):
                    log.write(line)
                    log.flush()
                    text = line.decode('utf-8', errors='replace').strip()
                    match = re.search(r'监听：http://(127\.0\.0\.1:\d+)/', text)
                    if match:
                        addresses.put('http://' + match.group(1))
        for stream, name in [(self.child.stdout, 'stdout.log'), (self.child.stderr, 'stderr.log')]:
            thread = threading.Thread(target=drain, args=(stream, name))
            thread.start()
            self.readers.append(thread)
        self.base = addresses.get(timeout=20)
        self.evidence.update(base=self.base, pid=self.child.pid, target=str(self.target))
        for zone in ['manager', 'node']:
            self.http('/api/zones', {'id': zone, 'path': str(self.root / zone)})
        self.tasks = {}
        for role in ['manager', 'normal']:
            result = self.http('/api/tasks', {'name': role, 'taskType': role, 'agent': 'omp', 'chatName': '{chat.id}',
                                             'model': 'default', 'thinking': 'default', 'skill': None, 'prompt': ''})
            self.tasks[role] = next(task['id'] for task in result['tasks'] if task['name'] == role)
        self.http('/api/scheduler/bots/save', {'id': 'worker', 'content': json.dumps({'name': 'worker', 'when': '', 'plan_guide': '', 'node_guide': '', 'task_id': self.tasks['normal']}), 'revision': None})
        # Maps are file-backed definitions; provision only the empty container.
        # All material declarations and nodes are authored through public HTTP.
        self.map = 'work.json'
        dump(self.root / 'maps' / self.map, {'title': 'File consumption', 'description': '', 'nodes': {}})
        route = f'/api/scheduler/maps/{self.map}'
        self.route = route
        self.http(route + '/inputs', {'input_requirements': '', 'input_files': [{'id': 'brief', 'required': False}],
                                    'revision': self.http(route + '/inputs')['revision']})
        for node in ['A', 'B', 'C']:
            draft = self.http(route + '/nodes/new')
            content = {'title': node, 'bot_id': 'worker', 'input': 'Read only the declared material', 'zone': 'node', 'blocked_by': [],
                       'output_rule': [{'id': 'result'}]}
            self.http(route + '/nodes/save', {'id': node, 'content': json.dumps(content),
                                             'revision': draft['revision'], 'create': True})

    def rule(self, node, rule):
        draft = self.http(self.route + '/nodes/edit', {'id': node})
        content = draft['content']
        content['input_rule'] = rule
        self.http(self.route + '/nodes/save', {'id': node, 'content': json.dumps(content), 'revision': draft['revision'], 'create': False})

    def start(self, name, files):
        self.identity(name)
        run = self.http(self.route + '/runs', {'managerTaskId': self.tasks['manager'], 'zone': 'manager',
                                             'userInput': 'Public process smoke', 'fileInputs': files}, status=201)['run']
        prepared = self.http('/api/workflow/manager/prepare_inputs', {'sessionId': name, 'fileInputs': files,
                            'confirmed': True, 'reason': 'Mechanism fixture: known text and owned readable files reviewed'})
        require((prepared.get('changed') or prepared.get('stateCommitted')) and prepared['snapshot']['inputsReady']
                    and not prepared.get('error') and not prepared.get('historyError'), prepared)
        return run

    def dispatch(self, manager, node, name):
        self.identity(name, node=True)
        return self.http('/api/workflow/manager/dispatch', {'sessionId': manager, 'nodes': [node]})

    def report(self, session, path):
        return self.http('/api/workflow/node/report', {'sessionId': session, 'body': 'Actual file address', 'canComplete': True,
                                                      'outputs': [{'id': 'result', 'path': str(path)}]})

    def close(self):
        if self.child is not None:
            if self.child.poll() is None:
                self.child.terminate()
            self.child.wait(timeout=30)
        for reader in self.readers:
            reader.join(timeout=10)
            require(not reader.is_alive(), 'process output reader did not finish')
        dump(self.root / 'http.json', self.wire)
        dump(self.root / 'evidence.json', self.evidence)

