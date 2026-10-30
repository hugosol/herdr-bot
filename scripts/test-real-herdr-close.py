"""P22: real Herdr effects via workflow dispatch and manager complete (Windows).

The approved seams are S2 (production HTTP) and S3 (installed Herdr CLI).
Independent outcome: the exact execution Tab disappears, a live child in that
Tab exits, the sentinel Tab survives, and native Pi history still contains both
our unique input and the controlled model's literal reply. A no-op/wrong-target
close can leave workflow completed but must fail these external observations.
No test adapter or direct tab-close command participates in the assertion path.
"""
import argparse
import ctypes
import hashlib
import http.server
import json
import os
from pathlib import Path
import queue
import re
import runpy
import shutil
import subprocess
import sys
import tempfile
import threading
import time
import traceback
import urllib.error
import urllib.request
import uuid


def require(condition, detail):
    if not condition:
        raise AssertionError(detail)


def until(description, probe, seconds=45):
    deadline = time.monotonic() + seconds
    while True:
        result = probe()
        if result:
            return result
        if time.monotonic() >= deadline:
            raise AssertionError('Timed out: ' + description)
        time.sleep(0.1)


class Process:
    """Continuously drain both streams, including startup and cleanup failures."""
    def __init__(self, argv, cwd, env, root, label):
        self.lines = queue.Queue()
        self.child = subprocess.Popen(argv, cwd=cwd, env=env, stdout=subprocess.PIPE,
                                      stderr=subprocess.PIPE, text=True, encoding='utf-8', errors='replace')
        self.threads = []
        for name, stream in [('stdout', self.child.stdout), ('stderr', self.child.stderr)]:
            def drain(stream=stream, name=name):
                with (root / f'{label}-{name}.log').open('w', encoding='utf-8') as log:
                    for line in stream:
                        log.write(line)
                        log.flush()
                        self.lines.put(line)
            thread = threading.Thread(target=drain, daemon=True)
            thread.start()
            self.threads.append(thread)

    def stop(self):
        if self.child.poll() is None:
            # Wait for taskkill itself before starting the exit-observation window.
            result = subprocess.run(['taskkill', '/PID', str(self.child.pid), '/T', '/F'],
                                    capture_output=True, encoding='utf-8', errors='replace', timeout=60)
            self.child.wait(timeout=15)
            require(result.returncode == 0 or self.child.poll() is not None,
                    ('taskkill', result.returncode, result.stdout, result.stderr))
        for thread in self.threads:
            thread.join(timeout=15)
            require(not thread.is_alive(), 'Process output drain did not finish')


class LiveProcess:
    """Hold the actual process handle: PID reuse cannot produce false evidence."""
    def __init__(self, pid):
        self.kernel = ctypes.WinDLL('kernel32', use_last_error=True)
        self.kernel.OpenProcess.argtypes = [ctypes.c_ulong, ctypes.c_int, ctypes.c_ulong]
        self.kernel.OpenProcess.restype = ctypes.c_void_p
        self.kernel.WaitForSingleObject.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
        self.kernel.CloseHandle.argtypes = [ctypes.c_void_p]
        self.handle = self.kernel.OpenProcess(0x00100000, False, pid)
        require(self.handle, ('Cannot observe controlled PID', pid, ctypes.get_last_error()))
        self.pid = pid

    def alive(self):
        status = self.kernel.WaitForSingleObject(self.handle, 0)
        require(status in [0, 258], ('WaitForSingleObject', status))
        return status == 258

    def close(self):
        self.kernel.CloseHandle(self.handle)


class Acceptance:
    def __init__(self, args):
        self.args = args
        self.root = Path(tempfile.mkdtemp(prefix='herdr-close-p22-'))
        self.evidence = {'promise': 'P22', 'passed': False, 'root': str(self.root),
                         'commands': [], 'cleanupErrors': []}
        self.processes = []
        self.controlled = None
        self.model = None
        self.herdr_started = False
        self.base = None
        self.env = {k: v for k, v in os.environ.items() if not any(term in k.upper() for term in
                    ['API_KEY', 'TOKEN', 'HERDR', 'PI_', 'OMP_', 'ANTHROPIC', 'OPENAI', 'AZURE', 'XDG_'])}
        for name in ['home', 'config', 'state', 'agent', 'zone', 'maps', 'bots', 'data', 'appdata', 'localappdata']:
            (self.root / name).mkdir()
        self.env.update(HOME=str(self.root / 'home'), USERPROFILE=str(self.root / 'home'),
                        APPDATA=str(self.root / 'appdata'), LOCALAPPDATA=str(self.root / 'localappdata'),
                        XDG_CONFIG_HOME=str(self.root / 'config'), XDG_STATE_HOME=str(self.root / 'state'),
                        PI_CODING_AGENT_DIR=str(self.root / 'agent'), NO_COLOR='1',
                        HERDR_SESSION='p22-' + uuid.uuid4().hex)
        self.evidence['isolation'] = {key: self.env[key] for key in [
            'HOME', 'USERPROFILE', 'APPDATA', 'LOCALAPPDATA', 'XDG_CONFIG_HOME',
            'XDG_STATE_HOME', 'PI_CODING_AGENT_DIR', 'HERDR_SESSION']}

    def command(self, argv, seconds=60):
        result = subprocess.run([str(x) for x in argv], cwd=self.root, env=self.env,
                                capture_output=True, encoding='utf-8', errors='replace', timeout=seconds)
        record = {'argv': [str(x) for x in argv], 'exit': result.returncode,
                  'stdout': result.stdout, 'stderr': result.stderr}
        self.evidence['commands'].append(record)
        require(result.returncode == 0, record)
        return result.stdout

    def herdr(self, *argv):
        return json.loads(self.command([self.args.herdr, *argv]))['result']

    def http(self, path, body=None, status=200):
        request = urllib.request.Request(self.base + path, data=None if body is None else
                                         json.dumps(body).encode(), headers={'Content-Type': 'application/json'})
        try:
            response = urllib.request.urlopen(request, timeout=100)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            text = response.read().decode()
            record = {'path': path, 'status': response.status, 'body': text}
            self.evidence.setdefault('http', []).append(record)
            require(response.status == status, record)
            return json.loads(text)

    def start(self, argv, label):
        process = Process([str(x) for x in argv], self.root, self.env, self.root, label)
        self.processes.append(process)
        return process

    def prepare(self):
        require(sys.platform == 'win32', 'P22 runner currently requires Windows process handles')
        for path in [self.args.herdr, self.args.backend, self.args.pi_package / 'dist/bundle/cli.js',
                     self.args.pi_state_extension, self.args.toolkit / 'extensions/workflow-chat/config.yaml']:
            require(path.is_file(), ('Missing real dependency', str(path)))
        # Copy the real role extensions/config, never redirect their endpoint in a user's checkout.
        toolkit = self.root / 'toolkit'
        toolkit.mkdir()
        for name in ['adapters', 'framework', 'extensions', 'entrypoints']:
            shutil.copytree(self.args.toolkit / name, toolkit / name,
                            ignore=shutil.ignore_patterns('.runtime', 'node_modules', '.git'))
        shutil.copy2(self.args.toolkit / 'package.json', toolkit / 'package.json')
        shutil.copytree(self.args.toolkit / 'node_modules/yaml', toolkit / 'node_modules/yaml')
        role_config = toolkit / 'extensions/workflow-chat/config.yaml'
        role_config.write_text('endpoint: http://127.0.0.1:9\nrequestTimeoutMs: 30000\ncollectTimeoutMs: 5460000\n')
        native = runpy.run_path(str(Path(__file__).with_name('test-native-session-storage.py')))
        self.model = http.server.ThreadingHTTPServer(('127.0.0.1', 0), native['Model'])
        threading.Thread(target=self.model.serve_forever, daemon=True).start()
        agent = self.root / 'agent'
        (agent / 'settings.json').write_text(json.dumps({'sessionDir': str(agent / 'sessions')}))
        (agent / 'models.json').write_text(json.dumps({'providers': {'storage-local': {
            'baseUrl': f'http://127.0.0.1:{self.model.server_port}/v1', 'api': 'openai-completions',
            'apiKey': 'isolated-not-a-real-key', 'models': [{'id': 'storage-probe', 'name': 'P22 probe',
            'reasoning': False, 'input': ['text'], 'contextWindow': 32768, 'maxTokens': 1000,
            'cost': {'input': 0, 'output': 0, 'cacheRead': 0, 'cacheWrite': 0}}]}}}))
        herdr_config = self.root / 'herdr.toml'
        herdr_config.write_text('onboarding = false\n[terminal]\ndefault_shell = "cmd.exe"\n'
                                '[update]\nversion_check = false\nmanifest_check = false\n')
        self.env['HERDR_CONFIG_PATH'] = str(herdr_config)
        self.evidence['herdrVersion'] = self.command([self.args.herdr, '--version'])
        self.start([self.args.herdr, 'server'], 'herdr')
        self.herdr_started = True
        def ready():
            result = subprocess.run([str(self.args.herdr), 'workspace', 'list'], cwd=self.root,
                                    env=self.env, capture_output=True, timeout=5)
            return result.returncode == 0
        until('isolated Herdr server', ready)
        workspace = self.herdr('workspace', 'create', '--cwd', str(self.root / 'zone'),
                               '--label', 'zone', '--no-focus')
        self.workspace = workspace['workspace']['workspace_id']
        self.sentinel = workspace['tab']['tab_id']
        self.evidence['ownedWorkspace'] = workspace
        catalog = 'skills = []\n[[agents]]\nid = "pi"\nlabel = "Pi"\n'
        catalog += '[[models]]\nid = "default"\nlabel = "Default"\n'
        catalog += '[[models]]\nid = "probe"\nlabel = "Probe"\nprovider_model = "storage-local/storage-probe"\nthinking = { pi = ["off"] }\n'
        for id, path in [('pi-herdr-agent-state', self.args.pi_state_extension),
                         ('pi-manager-chat', toolkit / 'entrypoints/pi-auto/manager-chat.ts'),
                         ('pi-node-chat', toolkit / 'entrypoints/pi-auto/node-chat.ts')]:
            catalog += f'[[extensions]]\nid = "{id}"\nlabel = "{id}"\nagent = "pi"\nkind = "fixed"\nentrypoint = {json.dumps(str(path))}\n'
        (self.root / 'options.toml').write_text(catalog)
        q = lambda value: json.dumps(str(value))
        config = 'listen_addr = "127.0.0.1:0"\n[mdns]\nenabled = false\n'
        for name in ['start', 'stop']:
            config += f'[operations.{name}]\nscript = {q(self.root / "unused.bat")}\nworking_directory = {q(self.root)}\n'
        config += f'[herdr]\nworkspace_name = "zone"\nexecutable = {q(self.args.herdr)}\n'
        config += f'[tasks]\nstorage_file = {q(self.root / "data/tasks.json")}\nmetadata_file = {q(self.root / "options.toml")}\n'
        config += f'[sessions]\nhistory_file = {q(self.root / "data/history.json")}\n'
        config += f'[zones]\nregistry_file = {q(self.root / "data/zones.json")}\n'
        config += '[scheduler]\n' + ''.join(f'{key} = {q(value)}\n' for key, value in [
            ('map_directory', self.root / 'maps'), ('bot_directory', self.root / 'bots'),
            ('run_directory', self.root / 'data/runs'), ('run_instructions', self.args.repo / 'docs/workflow-runtime.md')])
        (self.root / 'backend.toml').write_text(config)
        backend = self.start([self.args.backend, '--config', self.root / 'backend.toml'], 'backend')
        def address():
            require(backend.child.poll() is None, 'Backend exited during startup; see raw logs')
            while not backend.lines.empty():
                match = re.search(r'http://(127\.0\.0\.1:\d+)/', backend.lines.get_nowait())
                if match:
                    return 'http://' + match[1]
        self.base = until('backend bound loopback address', address)
        role_config.write_text(f'endpoint: {self.base}\nrequestTimeoutMs: 30000\ncollectTimeoutMs: 5460000\n')

    def history(self, file):
        module = (self.args.pi_package / 'dist/core/session-manager.js').as_uri()
        code = f'import {{SessionManager}} from {json.dumps(module)}; const s = SessionManager.open({json.dumps(file)}); console.log(JSON.stringify({{id:s.getSessionId(),branch:s.getBranch()}}));'
        return json.loads(self.command([self.args.node, '--input-type=module', '-e', code]))

    def exercise(self):
        zone = str(self.root / 'zone')
        self.http('/api/zones', {'path': zone})
        ids = {}
        for role in ['manager', 'node']:
            result = self.http('/api/tasks', {'name': role, 'taskType': 'manager' if role == 'manager' else 'normal',
                'agent': 'pi', 'chatName': '{chat.id}', 'model': 'probe', 'thinking': 'default',
                'skill': None, 'prompt': 'Acknowledge only. Do not call tools.', 'extensions': []})
            ids[role] = next(task['id'] for task in result['tasks'] if task['name'] == role)
        marker = 'P22-HISTORY-' + uuid.uuid4().hex
        (self.root / 'bots/worker.json').write_text(json.dumps({'id': 'worker', 'name': 'Worker',
            'when': '', 'plan_guide': '', 'node_guide': '', 'task_id': ids['node']}))
        (self.root / 'maps/close.json').write_text(json.dumps({'title': 'P22', 'description': '', 'nodes': {
            'A': {'title': 'Owned target', 'bot_id': 'worker', 'input': marker, 'zone': zone,
                  'blocked_by': [], 'auto_close': True}}}))
        started = self.http('/api/scheduler/maps/close.json/runs',
                            {'managerTaskId': ids['manager'], 'zone': zone, 'userInput': 'Acknowledge only.'}, 201)
        manager = started['run']['manager']['sessionId']
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
        command = subprocess.list2cmdline([sys.executable, str(Path(__file__).resolve()), '--controlled-process', str(pidfile)])
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

    def run(self):
        error = None
        try:
            self.prepare()
            self.exercise()
        except BaseException:
            error = traceback.format_exc()
            self.evidence['verificationError'] = error
        finally:
            if self.herdr_started:
                try:
                    self.command([self.args.herdr, 'server', 'stop'])
                except BaseException:
                    self.evidence['cleanupErrors'].append(traceback.format_exc())
            for process in reversed(self.processes):
                try:
                    process.stop()
                except BaseException:
                    self.evidence['cleanupErrors'].append(traceback.format_exc())
            if self.controlled:
                try:
                    require(not self.controlled.alive(), 'Controlled process survived cleanup')
                except BaseException:
                    self.evidence['cleanupErrors'].append(traceback.format_exc())
                finally:
                    self.controlled.close()
            if self.model:
                self.model.shutdown()
                self.model.server_close()
            self.evidence['passed'] = self.evidence['passed'] and not self.evidence['cleanupErrors']
            (self.root / 'evidence.json').write_text(json.dumps(self.evidence, ensure_ascii=False, indent=2), encoding='utf-8')
            print(json.dumps({'passed': self.evidence['passed'], 'evidence': str(self.root / 'evidence.json'),
                              'verificationError': error, 'cleanupErrors': self.evidence['cleanupErrors']}, ensure_ascii=False, indent=2))
        return 0 if self.evidence['passed'] else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--controlled-process', type=Path, help=argparse.SUPPRESS)
    parser.add_argument('--herdr', type=Path)
    parser.add_argument('--backend', type=Path)
    parser.add_argument('--pi-package', type=Path)
    parser.add_argument('--pi-state-extension', type=Path)
    parser.add_argument('--toolkit', type=Path)
    parser.add_argument('--node', default='node')
    args = parser.parse_args()
    if args.controlled_process:
        pending = args.controlled_process.with_suffix('.pending')
        pending.write_text(json.dumps({'pid': os.getpid()}))
        pending.replace(args.controlled_process)
        while True:
            time.sleep(1)
    for name in ['herdr', 'backend', 'pi_package', 'pi_state_extension', 'toolkit']:
        if getattr(args, name) is None:
            parser.error('--' + name.replace('_', '-') + ' is required')
        setattr(args, name, getattr(args, name).resolve())
    args.repo = Path(__file__).resolve().parent.parent
    return Acceptance(args).run()


if __name__ == '__main__':
    sys.exit(main())
