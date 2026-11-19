"""Owned Windows hosts; independent native observations of public Chat Resume."""
from contextlib import contextmanager
import errno
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import re
import secrets
import shutil
import subprocess
import threading
import time
import traceback
import urllib.error
import urllib.request
import uuid

from .host.isolation import Acceptance
from .host.processes import LiveProcess, require, until

INITIAL = 'PAIRED-INITIAL-628'
SKILL = 'PAIRED-SKILL-170'
ANSWER = 'PAIRED-OWNED-REPLY-315'
NAMES = ('bot-playground', 'paired-recovery-new-name')
ENV_REASONS = {'no-source': '缺少来源 Task', 'deleted-source': '来源 Task 已删除',
               'env-unreadable': 'env config', 'suite-invalid': 'Suite 配置不可用'}
CONTROLS = {'same-value': 'Artificial equality in owned persisted originalWorkspaceId; installed host answers normally',
            'no-source': 'Remove sourceTaskId from the owned record before Resume; real native identity unchanged',
            'env-unreadable': 'Replace the owned Task storage file with a directory after launch',
            'suite-invalid': 'Corrupt the owned Suite definition only after launch',
            'file-conflict': 'Change the header ID of the exact owned native file after shutdown; restore fixture bytes after observation',
            'task-agent-conflict': 'Change the agent of the owned source Task after launch'}


def q(value):
    return json.dumps(str(value), ensure_ascii=False)


def dump(file, value):
    file.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')


def same_path(actual, expected):
    return bool(actual) and Path(actual).resolve() == Path(expected).resolve()


def dynamic(factory):
    for attempt in range(32):
        port = 49152 + secrets.randbelow(16384)
        try:
            return factory(port)
        except OSError as error:
            if (error.errno != errno.EADDRINUSE and getattr(error, 'winerror', None) != 10048) or attempt == 31:
                raise


class LocalText(BaseHTTPRequestHandler):
    """Mechanical text provider. No tool choices, credentials or external HTTP."""
    def log_message(self, *_):
        pass

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        request = {'route': self.path, 'body': body, 'at': time.monotonic()}
        self.server.requests.append(request)
        if self.path != '/v1/chat/completions' or body.get('model') not in ('launch-only', 'default-only', 'future-only'):
            request['status'] = 400
            self.send_error(400, 'Unexpected local provider request')
            return
        request.update(status=200, response=ANSWER)
        self.send_response(200)
        self.send_header('Content-Type', 'text/event-stream')
        self.end_headers()
        for delta, finish in (({'role': 'assistant', 'content': ANSWER}, None), ({}, 'stop')):
            packet = {'id': 'paired-owned-text', 'object': 'chat.completion.chunk', 'created': 1,
                'model': body['model'], 'choices': [{'index': 0, 'delta': delta, 'finish_reason': finish}]}
            self.wfile.write(('data: ' + json.dumps(packet) + '\n\n').encode())
        self.wfile.write(b'data: [DONE]\n\n')


class Recovery(Acceptance):
    def __init__(self, args, cell):
        # Only the retained evidence may live at a long path; App IPC lives here.
        super().__init__(type('LocalArgs', (), {})(), prefix='hb8-', promise=cell['promises'], session_prefix='hb8-')
        self.args = args
        self.agent = cell['agent']
        self.source, self.destination = cell['direction'].split('-to-')
        self.scenario = cell['scenario']
        if self.scenario == 'same-end':
            self.destination = self.source
        self.evidence.update(cell=cell.copy(), controlledInputs=CONTROLS.get(self.scenario),
                             provider='owned mechanical text only; no real model or user auth', status='unverified')
        # Only OS/console bootstrap keys; no inherited provider/config/auth overrides.
        os_keys = {'SYSTEMROOT', 'WINDIR', 'SYSTEMDRIVE', 'COMSPEC', 'PATHEXT', 'NUMBER_OF_PROCESSORS',
                   'PROCESSOR_ARCHITECTURE', 'COMPUTERNAME', 'USERNAME', 'OS'}
        self.env = {k: v for k, v in self.env.items() if k.upper() in os_keys or k in self.evidence['isolation']}
        runtime = self.args.roots['node'].parent
        self.git_executable = Path(shutil.which('git.exe')).resolve()
        path = [self.args.roots['omp_cli'].parent, runtime, self.args.roots['bun'].parent,
                self.git_executable.parent, Path(os.environ['SystemRoot']) / 'System32']
        self.env.update(PATH=os.pathsep.join(map(str, dict.fromkeys(path))), GIT_CONFIG_NOSYSTEM='1',
                        GIT_CONFIG_GLOBAL=str(self.root / 'empty-git-config'), NO_COLOR='1')
        (self.root / 'empty-git-config').touch()
        for key in ('TEMP', 'TMP', 'XDG_CACHE_HOME'):
            self.env[key] = str(self.root / 'state')
        self.agent_dir = self.root / 'agent'
        self.env.update(PI_CODING_AGENT_DIR=str(self.agent_dir), OMP_CODING_AGENT_DIR=str(self.agent_dir))
        self.history_path = self.root / 'data/history.json'
        self.tasks_path = self.root / 'data/tasks.json'
        self.suites_path = self.root / 'suites.json'
        self.events_path = self.root / 'native.jsonl'
        self.deployment = self.root / 'toolkit'
        self.origin = self.root / 'zone'
        self.default_cwd = self.root / 'default-directory'
        self.default_cwd.mkdir()
        self.default_cwds = {}
        self.handles = {}
        self.reports = []

    def native(self, exe, *argv):
        return json.loads(self.command([exe, *argv]))

    def request(self, method, path, body=None, expected=200):
        request = urllib.request.Request(self.base + path, method=method, data=None if body is None else json.dumps(body).encode(),
                                         headers={'Content-Type': 'application/json'})
        try:
            response = urllib.request.build_opener(urllib.request.ProxyHandler({})).open(request, timeout=105)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            raw = response.read().decode('utf-8')
            call = {'method': method, 'path': path, 'request': body, 'status': response.status, 'body': raw}
            self.reports.append(call)
            require(expected is None or response.status == expected, call)
            return response.status, json.loads(raw)

    def http(self, path, body=None, status=200):
        return self.request('GET' if body is None else 'POST', path, body, status)[1]

    def probe(self, *argv):
        result = subprocess.run(list(map(str, argv)), cwd=self.root, env=self.env, capture_output=True,
                                encoding='utf-8', errors='replace', timeout=10)
        self.evidence['commands'].append(dict(argv=list(map(str, argv)), exit=result.returncode,
                                               stdout=result.stdout, stderr=result.stderr))
        return result.returncode == 0

    def remember(self, phase, pid, kind):
        require(isinstance(pid, int) and pid > 0, ('Invalid observed process', phase, pid, kind))
        if (phase, pid) not in self.handles:
            self.handles[phase, pid] = (kind, LiveProcess(pid))

    def exit_confirmation(self, phase):
        errors = []
        for (owner, pid), (kind, handle) in list(self.handles.items()):
            if owner != phase:
                continue
            try:
                until(f'{owner} {kind} PID {pid} exit', lambda: not handle.alive(), seconds=15)
                self.evidence.setdefault('exits', []).append(dict(phase=owner, pid=pid, kind=kind,
                                                                  exited=True, observedAt=time.monotonic()))
            except Exception:
                errors.append(traceback.format_exc())
            finally:
                handle.close()
                del self.handles[owner, pid]
        return errors

    def core_status(self, phase):
        value = self.native(self.args.core, 'server', 'status', '--json')
        self.remember(phase, value['pid'], 'core')
        for holder in value['holders']:
            self.remember(phase, holder['pid'], 'core-holder')
        return value['core']['id']

    def native_endpoint(self, host):
        return (self.args.herdr, 'workspace', 'list') if host == 'tui' else (self.args.app, 'ls')

    @contextmanager
    def host(self, host, phase):
        self.env['HERDR_SESSION'] = 'hb8-' + uuid.uuid4().hex
        self.env['HC_SESSION'] = 'hb8-' + uuid.uuid4().hex
        # Same-end tests retain the owned TUI instance's persisted Workspace ID.
        configuration_name = 'origin' if self.scenario == 'same-end' else phase
        config = self.root / f'{configuration_name}.toml'
        config.write_text('onboarding = false\n[terminal]\ndefault_shell = "cmd.exe"\n'
                          '[update]\nversion_check = false\nmanifest_check = false\n', encoding='utf-8')
        if self.scenario == 'same-end' and phase == 'destination':
            self.env.update(self.evidence['instances'][0]['selectors'])
        else:
            home = self.root / ('a' + phase[0])
            home.mkdir()
            dump(home / 'settings.json', {'telemetry': False, 'check_updates': False})
            self.env.update(HERDR_CONFIG_PATH=str(config), HERDR_APP_HOME=str(home),
                            HERDR_CORE_DIRS=str(self.args.core.parent))
        selectors = {key: self.env[key] for key in ('HERDR_SESSION', 'HERDR_CONFIG_PATH', 'HERDR_APP_HOME',
                                                   'HC_SESSION', 'HERDR_CORE_DIRS')}
        self.evidence.setdefault('instances', []).append(dict(host=host, phase=phase, selectors=selectors))
        prior_events = len(self.events())
        service = client = None
        core_started = False
        cleanup = []
        try:
            if host == 'tui':
                service = self.start([self.args.herdr, 'server'], f'{phase}-tui')
                self.remember(phase, service.child.pid, 'tui-server')
                until('owned TUI endpoint', lambda: self.probe(*self.native_endpoint(host)), seconds=20)
                client = subprocess.Popen([str(self.args.herdr), 'session', 'attach', self.env['HERDR_SESSION']],
                                          cwd=self.root, env=self.env, creationflags=subprocess.CREATE_NEW_CONSOLE)
                self.remember(phase, client.pid, 'tui-client')
            else:
                core = self.native(self.args.core, 'server', 'start', '--json')
                core_started = True
                self.remember(phase, core['pid'], 'core')
                for holder in core['holders']:
                    self.remember(phase, holder['pid'], 'core-holder')
                service = self.start([self.args.app], f'{phase}-app')
                self.remember(phase, service.child.pid, 'app-window')
                until('owned App endpoint', lambda: self.probe(*self.native_endpoint(host)), seconds=20)
                require(self.core_status(phase) == core['core']['id'], 'App startup selected a different core')
            yield phase
        finally:
            def clean(action):
                try:
                    action()
                except Exception:
                    cleanup.append(traceback.format_exc())
            for pid in {row['pid'] for row in self.events()[prior_events:] if row['event'] == 'session_start'}:
                clean(lambda: self.remember(phase, pid, 'installed-' + self.agent))
            if service and host == 'tui' and service.child.poll() is None:
                clean(lambda: self.command([self.args.herdr, 'server', 'stop']))
            if service:
                clean(service.stop)
                self.evidence.setdefault('ownedExitCodes', []).append(dict(phase=phase, host=host, exit=service.child.poll()))
                self.processes.remove(service)
            if client:
                try:
                    client.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    client.terminate()
                    clean(lambda: client.wait(timeout=10))
                self.evidence['ownedExitCodes'].append(dict(phase=phase, host='tui-client', exit=client.poll()))
            if core_started:
                clean(lambda: self.core_status(phase))
                clean(lambda: self.native(self.args.core, 'server', 'stop', '--json'))
            cleanup.extend(self.exit_confirmation(phase))
            self.evidence['cleanupErrors'].extend(cleanup)
            if not cleanup and service:
                # Probe only non-starting read-only endpoints after handles have signaled.
                if self.probe(*self.native_endpoint(host)) or (host == 'app' and self.probe(self.args.core, 'server', 'status', '--json')):
                    self.evidence['cleanupErrors'].append(f'{phase}: owned {host} endpoint still responding')

    def containers(self, host):
        if host == 'tui':
            return {x['workspace_id']: x['label'] for x in self.herdr('workspace', 'list')['workspaces']}
        return {x['id']: x['name'] for x in self.native(self.args.app, 'ls')['spaces']}

    def new_container(self, host, label, cwd):
        if host == 'tui':
            return self.herdr('workspace', 'create', '--label', label, '--cwd', str(cwd), '--no-focus')['workspace']['workspace_id']
        return self.native(self.args.app, 'do', 'space.new', json.dumps({'name': label, 'folder': str(cwd), 'focus': False}))['space']

    def inventory(self, host):
        if host == 'tui':
            workspaces = self.herdr('workspace', 'list')['workspaces']
            return {w['workspace_id']: {t['tab_id'] for t in self.herdr('tab', 'list', '--workspace', w['workspace_id'])['tabs']}
                    for w in workspaces}
        return {s['id']: {t['id'] for t in s['tabs']} for s in self.native(self.args.app, 'ls')['spaces']}

    def observe_host(self, host, phase, target, native, *, focused):
        ws, tab, pane = (target[k] for k in ('workspaceId', 'tabId', 'paneId'))
        if host == 'app':
            layout = self.native(self.args.app, 'ls')
            space = next(s for s in layout['spaces'] if s['id'] == ws)
            actual_tab = next(t for t in space['tabs'] if t['id'] == tab)
            member = next(p for p in actual_tab['terminals'] if p['pane'] == pane)
            core_id = self.core_status(phase)
            actual = self.native(self.args.core, 'get', member['terminal'], '--json')
            require(actual['state'] == 'live' and actual['labels']['herdr.space'] == ws and
                    actual['labels']['herdr.tab'] == tab and actual['address'] == member['terminal'], actual)
            focus = self.native(self.args.app, 'status')
            if focused:
                require((focus.get('space'), focus.get('tab'), focus.get('focused_pane')) == (ws, tab, pane), focus)
            current = actual.get('foreground') or {}
            require(same_path(current.get('cwd') or actual.get('cwd'), native['cwd']), (actual, native))
            require(actual.get('pid') in native['lineage'], ('Observed agent is not in App core terminal process tree', actual, native))
            self.evidence.setdefault('observations', []).append(dict(phase=phase, target=target,
                app=layout, core=actual, coreId=core_id, focus=focus, native=native))
        else:
            require(tab in self.inventory(host)[ws], ('TUI tab not in selected workspace', target))
            pane_data = self.herdr('pane', 'get', pane)
            agent_data = self.herdr('agent', 'get', pane)
            info = self.herdr('pane', 'process-info', '--pane', pane)['process_info']
            require(info['pane_id'] == pane and any(p['pid'] == native['pid'] and
                    same_path(p.get('cwd') or agent_data['agent'].get('foreground_cwd'), native['cwd'])
                    for p in info['foreground_processes']), ('Observed agent is not the TUI Pane process', info, native))
            focus = self.herdr('api', 'snapshot')
            if focused:
                require(tui_focus(focus) == (ws, tab, pane), focus)
                require(self.handles[phase, next(pid for (owner, pid), (kind, _) in self.handles.items()
                        if owner == phase and kind == 'tui-client')][1].alive(), 'owned TUI client is not attached/alive')
            self.evidence.setdefault('observations', []).append(dict(phase=phase, target=target, pane=pane_data,
                                                                       agent=agent_data, focus=focus, native=native))

    def configure(self):
        self.recorders = {}
        for kind, installed in [('tui', self.args.herdr), ('app', self.args.app), ('core', self.args.core)]:
            directory = self.root / f'wire-{kind}'
            directory.mkdir()
            dump(directory / 'recovery-proxy.json', {'herdr': str(installed)})
            self.recorders[kind] = directory / installed.name
            shutil.copy2(self.args.recorder, self.recorders[kind])
        self.model = dynamic(lambda port: ThreadingHTTPServer(('127.0.0.1', port), LocalText))
        self.model.requests = []
        self.model_thread = threading.Thread(target=self.model.serve_forever, daemon=True)
        self.model_thread.start()
        for folder in ('adapters', 'framework', 'extensions', 'entrypoints'):
            shutil.copytree(self.args.toolkit_worktree / folder, self.deployment / folder,
                            ignore=shutil.ignore_patterns('.runtime', '.git', 'node_modules'))
        shutil.copy2(self.args.toolkit_worktree / 'package.json', self.deployment / 'package.json')
        for dep in ('yaml', 'pi-subagents'):
            shutil.copytree(self.args.toolkit_worktree / 'node_modules' / dep, self.deployment / 'node_modules' / dep)
        model = {'providers': {'paired-owned': {'baseUrl': f'http://127.0.0.1:{self.model.server_port}/v1',
            'api': 'openai-completions', 'apiKey': 'owned-loopback-not-a-credential', 'models': [
                {'id': name, 'name': name, 'reasoning': True, 'input': ['text'], 'contextWindow': 32768,
                 'maxTokens': 2048, 'cost': {'input': 0, 'output': 0, 'cacheRead': 0, 'cacheWrite': 0}}
                for name in ('launch-only', 'default-only', 'future-only')]}}}
        dump(self.agent_dir / 'models.json', model)
        dump(self.agent_dir / 'models.yml', model)
        dump(self.agent_dir / 'settings.json', {'autoUpdate': False, 'sessionDir': str(self.agent_dir / 'sessions'),
                                                'defaultProvider': 'paired-owned', 'defaultModel': 'default-only'})
        (self.agent_dir / 'config.yml').write_text('autoResume: false\nstartup:\n  setupWizard: false\n  showSplash: false\n'
            '  checkUpdate: false\nmarketplace:\n  autoUpdate: off\nterminal:\n  programStatus: true\n', encoding='utf-8')
        observer = self.root / 'observer.ts'
        observer.write_text(observer_source(self.events_path, 'configured'), encoding='utf-8')
        ambient = self.agent_dir / 'extensions'
        ambient.mkdir()
        (ambient / 'paired-native-observer.ts').write_text(observer_source(self.events_path, 'native-default'), encoding='utf-8')
        # Only TUI's fallback has the official reference reporter through ambient discovery.
        # App fallback intentionally has no connect-bot or any substitute report.
        if self.destination == 'tui':
            shutil.copy2(self.args.roots[f'{self.agent}_state_extension'], ambient / 'herdr-state.ts')
        self.options = self.root / 'options.toml'
        self.options.write_text(self.catalog(observer, 'fixed-old'), encoding='utf-8')
        suites = []
        for variant in ('before', 'after'):
            entry = self.root / f'suite-{variant}.ts'
            entry.write_text(observer_source(self.events_path, f'suite-{variant}', observe_input=False), encoding='utf-8')
            directory = self.root / f'skills-{variant}'
            skill = directory / 'paired-initial'
            skill.mkdir(parents=True)
            (skill / 'SKILL.md').write_text(f'---\nname: paired-initial\ndescription: Owned initial marker\n---\n{SKILL}\n', encoding='utf-8')
            suite = {'id': variant, 'name': variant, 'agents': [self.agent],
                     'skills': {self.agent: [str(directory if self.agent == 'omp' else skill)]},
                     'extensions': {self.agent: [str(entry)]}}
            if self.agent == 'pi':
                suite['pi'] = {'codemode': 'on'}
            suites.append(suite)
        dump(self.suites_path, {'suites': suites})
        dump(self.root / 'mcp.json', {'mcpServers': {}})

    def catalog(self, observer, fixed):
        script = self.root / f'{fixed}.ts'
        script.write_text(observer_source(self.events_path, fixed, observe_input=False), encoding='utf-8')
        entries = [(self.agent, 'paired-observer', observer), (self.agent, 'paired-fixed', script),
                   ('pi', 'pi-suite', self.deployment / 'entrypoints/pi/pi-suite.ts')]
        for agent in ('omp', 'pi'):
            entries.extend([(agent, f'{agent}-herdr-agent-state', self.args.roots[f'{agent}_state_extension']),
                            (agent, f'{agent}-connect-bot', self.deployment / f'entrypoints/{agent}-auto/connect-bot.ts')])
        text = ''.join(f'[[agents]]\nid = "{agent}"\nlabel = "{agent}"\n' for agent in ('omp', 'pi'))
        text += '[[models]]\nid = "default"\nlabel = "Default"\n'
        for model in ('launch-only', 'future-only'):
            text += f'[[models]]\nid = "{model}"\nlabel = "{model}"\nprovider_model = "paired-owned/{model}"\nthinking = {{ {self.agent} = ["high"] }}\n'
        text += '[[skills]]\nid = "paired-init"\nlabel = "Initial"\nnative_name = "paired-initial"\n'
        for agent, key, path in entries:
            text += f'[[extensions]]\nid = "{key}"\nlabel = "{key}"\nagent = "{agent}"\nkind = "fixed"\nentrypoint = {q(path)}\n'
        return text

    @contextmanager
    def backend(self, host, name, phase):
        import socket
        def reserve(port):
            with socket.socket() as listener:
                listener.bind(('127.0.0.1', port))
            return port
        port = dynamic(reserve)
        filename = self.root / f'backend-{phase}.toml'
        denied = self.root / 'operation-not-authorized.cmd'
        denied.write_text('@exit /b 79\n', encoding='ascii')
        filename.write_text(f'''listen_addr = "127.0.0.1:{port}"
static_directory = {q(self.args.herdr_bot_worktree / 'backend/static')}
data_directory = {q(self.root / 'data')}
[mdns]
enabled = false
[operations.start]
script = {q(denied)}
working_directory = {q(self.root)}
[operations.stop]
script = {q(denied)}
working_directory = {q(self.root)}
[herdr]
backend = {q(host)}
workspace_name = {q(name)}
executable = {q(self.recorders['tui'])}
app_executable = {q(self.recorders['app'])}
core_executable = {q(self.recorders['core'])}
[tasks]
metadata_file = {q(self.options)}
storage_file = {q(self.tasks_path)}
[zones]
registry_file = {q(self.root / 'data/zones.json')}
[sessions]
history_file = {q(self.history_path)}
[suites]
definition_file = {q(self.suites_path)}
mcp_file = {q(self.root / 'mcp.json')}
generated_directory = {q(self.root / 'generated')}
''', encoding='utf-8')
        self.base = f'http://127.0.0.1:{port}'
        (self.deployment / 'extensions/connect-bot/config.yaml').write_text(
            f'endpoint: "{self.base}"\nrequestTimeoutMs: 5000\n', encoding='utf-8')
        panel = self.start([self.args.backend, '--config', filename], f'backend-{phase}')
        try:
            def listening():
                require(panel.child.poll() is None, ('owned backend exited; see logs', phase, panel.child.poll()))
                while not panel.lines.empty():
                    if self.base in panel.lines.get_nowait():
                        return True
            until('owned backend listening', listening, seconds=25)
            yield
        finally:
            try:
                panel.stop()
                self.evidence.setdefault('exits', []).append(dict(phase=phase, pid=panel.child.pid, kind='backend',
                                                                   exited=True, code=panel.child.poll()))
            except Exception:
                self.evidence['cleanupErrors'].append(traceback.format_exc())
            finally:
                self.processes.remove(panel)

    def events(self):
        if not self.events_path.exists():
            return []
        content = self.events_path.read_text(encoding='utf-8')
        return [json.loads(line) for line in content[:content.rfind('\n') + 1].splitlines()]

    def start_event(self, source, phase, after=0):
        def observation():
            return next((e for e in self.events()[after:] if e['source'] == source and e['event'] == 'session_start'), None)
        facts = until('actual native session_start ' + source, observation, seconds=20)
        self.remember(phase, facts['pid'], 'installed-' + self.agent)
        # Runtime-reported facts alone could be produced by an unrelated process.
        command = (f'$id0={facts["pid"]}; $all=@{{}}; '
                   'foreach($p in Get-CimInstance Win32_Process -Property ProcessId,ParentProcessId,CreationDate){$all[[int]$p.ProcessId]=$p}; '
                   '$rows=@(); $p=$all[$id0]; while($null -ne $p -and $p.ProcessId -notin $rows){'
                   '$rows+= $p.ProcessId; $parent=$all[[int]$p.ParentProcessId]; '
                   'if($null -eq $parent -or $parent.CreationDate -gt $p.CreationDate){break}; $p=$parent}; '
                   '$p=Get-CimInstance Win32_Process -Filter "ProcessId=$id0" -Property ProcessId,ParentProcessId,ExecutablePath,CommandLine; '
                   '@{lineage=$rows; process=($p | Select-Object ProcessId,ParentProcessId,ExecutablePath,CommandLine)} '
                   '| ConvertTo-Json -Compress')
        os_observation = self.native('powershell.exe', '-NoProfile', '-Command', command)
        os_process = os_observation['process']
        from .host.native import process_arguments
        argv = process_arguments(os_process['CommandLine'])
        installed = [self.args.entries[self.agent]]
        if self.agent == 'omp':
            # The installed App core may resolve the Bun script rather than the
            # npm launcher. Prove the exact file in that same declared package.
            entry = self.args.roots['omp_package'] / 'src/cli.ts'
            if entry.is_file():
                installed.append(entry)
        require(any(same_path(arg, item) for arg in argv if arg.lower().endswith(('.js', '.ts')) for item in installed),
                ('Observed PID not loading this installed agent', os_process))
        require(same_path(os_process['ExecutablePath'], self.args.roots['bun' if self.agent == 'omp' else 'node']), os_process)
        facts.update(os=os_process, lineage=os_observation['lineage'])
        require(same_path(facts['nativeCwd'], facts['cwd']), ('native Session cwd disagrees with process cwd', facts))
        return facts

    def manager(self, file):
        require(Path(file).resolve().is_relative_to(self.agent_dir.resolve()), ('Native file escaped isolation', file))
        relative = 'src/session/session-manager.ts' if self.agent == 'omp' else 'dist/core/session-manager.js'
        module = (self.args.roots[f'{self.agent}_package'] / relative).as_uri()
        js = f'import {{SessionManager}} from {q(module)};const s=await SessionManager.open({q(file)});console.log(JSON.stringify({{id:s.getSessionId(),branch:s.getBranch()}}));'
        argv = [self.args.roots['bun'], '-e', js] if self.agent == 'omp' else [self.args.roots['node'], '--input-type=module', '-e', js]
        return self.native(*argv)

    def preset(self, variant):
        return dict(name='paired-source', chatName='Owned paired {chat.id}', agent=self.agent,
                    model='launch-only' if variant == 'before' else 'future-only', thinking='high',
                    skill='paired-init', prompt=INITIAL, suite=variant)

    def launch(self, container):
        self.http('/api/zones', {'id': 'paired-owned', 'path': str(self.origin)})
        source = self.http('/api/tasks', self.preset('before'))['tasks'][0]['id']
        preview = self.http('/api/chats/preview', {'zone': 'paired-owned', 'taskId': source})
        accepted = self.http('/api/chats', {'zone': 'paired-owned', 'taskId': source, 'chatId': preview['chatId'],
                                            'workspaceTarget': 'default', 'input': 'Owned initial input'}, status=202)
        def complete():
            state = self.http('/api/chats/' + accepted['id'])
            require(state['stage'] != 'failed', state)
            return state if state['stage'] == 'succeeded' else None
        state = until('public Task Chat launch and initial submission', complete, seconds=100)
        start = self.start_event('configured', 'origin')
        require(same_path(start['cwd'], self.origin), start)
        require(start['model'] == 'launch-only', start)
        target = dict(workspaceId=container, tabId=state['tabId'], paneId=state['paneId'])
        self.observe_host(self.source, 'origin', target, start, focused=False)
        file = Path(start['file'])
        def answered():
            if not file.is_file() or ANSWER not in file.read_text(encoding='utf-8'):
                return None
            history = self.manager(file)
            return history if ANSWER in json.dumps(history) else None
        original = until('initial marker persisted by installed SessionManager', answered, seconds=40)
        require(original['id'] == start['id'] and INITIAL in json.dumps(original) and SKILL in json.dumps(original), original)
        self.assert_sources(start, {'configured', 'suite-before', 'fixed-old'})
        row = self.public_history(start['id'])
        require(row['originalWorkspaceId'] == container and row['agentType'] == self.agent, row)
        disk = json.loads(self.history_path.read_text(encoding='utf-8'))
        saved = next(r for r in disk if r['sessionId'] == start['id'])
        require(saved['sourceTaskId'] == source and same_path(saved['sessionFile'], file), saved)
        self.evidence['created'] = dict(task=source, progress=state, public=row, native=start, branch=original)
        return source, start, file

    def assert_sources(self, start, expected):
        events = self.events()
        actual = {e['source'] for e in events if e['pid'] == start['pid'] and e['event'] == 'session_start'}
        require(actual == expected, ('actual executed extension set', actual, expected, start))

    def public_history(self, id):
        rows = self.http('/api/sessions/history')['sessions']
        require(sum(r['sessionId'] == id for r in rows) == 1 and len(rows) == 1, ('original single public Chat', rows, id))
        return {key: value for key, value in rows[0].items() if key not in ('originalWorkspaceName', 'sessionDirectory')}

    def arrange(self, source, file, dest, copied_id):
        if self.scenario in ('same-value', 'no-source'):
            rows = json.loads(self.history_path.read_text(encoding='utf-8'))
            require(len(rows) == 1, rows)
            if self.scenario == 'same-value':
                rows[0]['originalWorkspaceId'] = dest
            else:
                del rows[0]['sourceTaskId']
            dump(self.history_path, rows)
        if self.scenario == 'deleted-source':
            self.request('DELETE', '/api/tasks/' + source)
        elif self.scenario == 'env-unreadable':
            self.tasks_path.rename(self.root / 'original-tasks.json')
            self.tasks_path.mkdir()
        elif self.scenario == 'suite-invalid':
            self.suites_path.write_text('{ independently invalid Suite', encoding='utf-8')
        elif self.scenario == 'file-conflict':
            lines = file.read_bytes().splitlines(keepends=True)
            for position, line in enumerate(lines[:2]):
                record = json.loads(line)
                if record['type'] == 'session':
                    require(record['id'] == copied_id, record)
                    lines[position] = (json.dumps({**record, 'id': str(uuid.uuid4())}) + '\n').encode('utf-8')
                    break
            else:
                raise AssertionError('Cannot locate actual session header to inject conflict')
            file.write_bytes(b''.join(lines))
            shutil.copy2(file, self.root / 'native-conflicting.jsonl')
        elif self.scenario == 'task-agent-conflict':
            rows = json.loads(self.tasks_path.read_text(encoding='utf-8'))
            for row in rows if isinstance(rows, list) else rows['tasks']:
                if row['id'] == source:
                    row['agent'] = 'pi' if row['agent'] == 'omp' else 'omp'
                    dump(self.tasks_path, rows)
                    break
            else:
                raise AssertionError('Source Task record not found')
        else:
            self.request('PUT', '/api/tasks/' + source, self.preset('after'))
            self.options.write_text(self.catalog(self.root / 'observer.ts', 'fixed-new'), encoding='utf-8')

    def resume(self, old, file, dest, cwd):
        history_before = self.public_history(old['id'])
        persisted_before = self.history_path.read_bytes()
        (self.root / 'history-before.json').write_bytes(persisted_before)
        before_native = self.manager(file) if self.scenario != 'file-conflict' else None
        before_tabs = self.inventory(self.destination)
        before_events = len(self.events())
        before_requests = len(self.model.requests)
        before_calls = {kind: set(exe.parent.joinpath('calls').glob('*.json')) for kind, exe in self.recorders.items()}
        self.evidence['status'] = 'failed'
        status, public = self.request('POST', '/api/chats/resume', {'sessionId': old['id']}, expected=None)
        self.evidence['resume'] = {'status': status, 'body': public, 'priorTabs': json_inventory(before_tabs),
                                   'expectedContainer': dest, 'expectedCwd': str(cwd)}
        if self.scenario in ('file-conflict', 'task-agent-conflict'):
            expected = {('session', 'native-session-mismatch'), ('lookup', 'identity-verification-failed')} if self.scenario == 'file-conflict' else {
                ('settings', 'task-agent-mismatch')}
            require(status >= 400 and public.get('outcome') == 'failed' and
                    (public.get('stage'), public.get('code')) in expected, public)
            require(self.inventory(self.destination) == before_tabs and len(self.events()) == before_events,
                    'known conflict launched a replacement or focused a candidate')
        else:
            environment = self.scenario in ENV_REASONS
            expected = 'unconfirmed' if environment and self.destination == 'app' else 'restored'
            require(status == 200 and public['outcome'] == expected and public['stage'] == 'focused', public)
            if expected == 'unconfirmed':
                require(public['progress'] == {'tabCreated': True, 'startSubmitted': True,
                        'conversationConfirmed': False, 'focused': True}, public)
            target = public['target']
            require(target['workspaceId'] == dest and same_path(public['actualCwd'], cwd), (target, public, dest, cwd))
            if environment:
                reasons = [d for d in public['degradations'] if d['element'] == 'extensions']
                require(len(reasons) == 1 and ENV_REASONS[self.scenario] in reasons[0]['reason'] and
                        '原生默认扩展发现' in reasons[0]['strategy'] and '不强制接入 connect-bot' in reasons[0]['strategy'], public)
            if self.scenario in ('cwd-missing', 'git-ambiguous'):
                reasons = [d for d in public['degradations'] if d['element'] == 'cwd']
                require(len(reasons) == 1 and '不指定工作目录' in reasons[0]['strategy'] and
                        (self.destination != 'app' or 'App 不提供' in reasons[0]['reason']), public)
            if self.scenario == 'git-unique':
                require(any(d['element'] == 'cwd' and '准确工作树归属' in d['strategy'] for d in public['degradations']), public)
            observed = self.start_event('native-default' if environment else 'configured', 'destination', before_events)
            require(observed['pid'] != old['pid'] and observed['id'] == old['id'] and same_path(observed['file'], file)
                    and same_path(observed['cwd'], cwd), (old, observed, file, cwd))
            self.observe_host(self.destination, 'destination', target, observed, focused=True)
            self.assert_sources(observed, {'native-default'} if environment else {'configured', 'fixed-new', 'suite-after'})
            require(not any(name in ('connect-bot',) for name in observed['commands']) if environment else
                    'connect-bot' in observed['commands'], ('actual command discovery', observed))
            from .host.native import process_arguments
            argv = process_arguments(observed['os']['CommandLine'])
            require(not any(arg in ('--model', '--thinking') for arg in argv), ('launch overrides replayed', observed))
            if environment:
                require(not any(arg in ('--extension', '--no-extensions') for arg in argv), ('explicit environment leaked', observed))
            require(not any(e['event'] == 'before_agent_start' for e in self.events()[before_events:]), 'trigger input replayed')
            after_tabs = self.inventory(self.destination)
            require({ws: after_tabs[ws] - tabs for ws, tabs in before_tabs.items()} ==
                    {ws: {target['tabId']} if ws == dest else set() for ws in before_tabs}
                    and before_tabs.keys() == after_tabs.keys() and all(tabs <= after_tabs[ws] for ws, tabs in before_tabs.items()),
                    ('unexpected retry/rebind/close/extra tab', before_tabs, after_tabs))
            require(self.manager(file) == before_native, 'original conversation was changed or trigger was replayed')
        if self.scenario == 'cross-end' or self.scenario in ENV_REASONS:
            # A delayed initial-input retry would otherwise outlive a fast Resume
            # assertion and be hidden by teardown. Leave the target alive beyond 5 s.
            began = time.monotonic()
            while time.monotonic() - began < 5.5:
                require(len(self.model.requests) == before_requests and not any(
                    event['event'] == 'before_agent_start' for event in self.events()[before_events:]),
                    'unexpected native round or provider invocation after Resume')
                time.sleep(0.2)
            self.evidence['resume']['quietSeconds'] = time.monotonic() - began
        calls = self.calls_since(before_calls)
        self.evidence['resume']['forwardedHostCalls'] = calls
        self.assert_effects(calls, self.scenario in ('file-conflict', 'task-agent-conflict'))
        require(len(self.model.requests) == before_requests, 'Resume caused an unexpected provider invocation')
        history_after = self.public_history(old['id'])
        persisted_after = self.history_path.read_bytes()
        (self.root / 'history-after.json').write_bytes(persisted_after)
        self.evidence['resume']['originalWorkspaceIdBefore'] = history_before.get('originalWorkspaceId')
        self.evidence['resume']['originalWorkspaceIdAfter'] = history_after.get('originalWorkspaceId')
        # A real connect-bot register can spell the same Windows cwd without
        # the trailing slash from Herdr. Keep the raw bytes, but require the
        # same field set, row and directory; a fallback cwd must not pass this.
        require(history_record(history_after) == history_record(history_before) and
                [history_record(row) for row in json.loads(persisted_after)] ==
                [history_record(row) for row in json.loads(persisted_before)],
                ('Resume changed original public history or existing persistent format/content', history_before, history_after))

    def calls_since(self, baseline):
        return {kind: [json.loads(path.read_text(encoding='utf-8')) for path in
                       sorted(set(exe.parent.joinpath('calls').glob('*.json')) - baseline.get(kind, set()))]
                for kind, exe in self.recorders.items()}

    def assert_effects(self, calls, conflict):
        require(not any(call['rejected'] for records in calls.values() for call in records), 'recorders must not inject errors')
        tui, app, core = ([call['argv'] for call in calls[kind]] for kind in ('tui', 'app', 'core'))
        require(not any(args[:2] in (['agent', 'prompt'], ['agent', 'input']) for args in tui) and
                not any(args[0] in ('send', 'keys') for args in core), ('Resume replayed terminal input', calls))
        creates = [args for args in tui if args[:2] == ['tab', 'create']]
        app_creates = [json.loads(args[2]) for args in app if args[:2] == ['do', 'tab.new']]
        starts = [args for args in tui if args[:2] == ['agent', 'start']]
        focuses = [args for args in tui if args[:2] == ['agent', 'focus']] + [args for args in app if args[:2] == ['do', 'pane.focus']]
        if conflict:
            require(not (creates or app_creates or starts or focuses), ('conflict mutated original target', calls))
        else:
            expect_tui = self.destination == 'tui'
            require((len(creates), len(starts), len(app_creates), len(focuses)) ==
                    (int(expect_tui), int(expect_tui), int(not expect_tui), 1), ('unexpected launch/focus attempts', calls))
            if self.scenario in ('cwd-missing', 'git-ambiguous'):
                require(all('--cwd' not in args for args in creates) and all('folder' not in args for args in app_creates),
                        ('default cwd must be selected natively, not supplied', calls))

    def git(self, directory, *argv):
        return self.command([self.git_executable, '-C', directory, *argv])

    def git_root(self, name, checkout):
        root = self.root / name
        root.mkdir()
        self.git(root, 'init', '-b', 'main')
        self.git(root, '-c', 'user.name=Owned fixture', '-c', 'user.email=paired@example.invalid', 'commit', '--allow-empty', '-m', 'owned')
        self.git(root, 'worktree', 'add', '-b', 'owned-linked', str(checkout))
        return root

    def exercise(self):
        self.configure()
        if self.scenario.startswith('git-'):
            self.origin = self.root / 'git-historical-checkout'
            main = self.git_root('git-main', self.origin)
        with self.host(self.source, 'origin'):
            original = self.new_container(self.source, NAMES[0], self.default_cwd)
            with self.backend(self.source, NAMES[0], 'origin'):
                source_id, old, file = self.launch(original)
                if self.source == 'tui' and self.scenario in ('same-end', 'cwd-missing'):
                    # Do not let native TUI restart its own parked Chat or hold
                    # the old cwd. Leave the Workspace with its unrelated blank tab.
                    tab = self.evidence['created']['progress']['tabId']
                    self.herdr('tab', 'close', tab)
                    handle = self.handles['origin', old['pid']][1]
                    until('explicitly closed old TUI Chat process', lambda: not handle.alive(), seconds=15)
        require(not self.evidence['cleanupErrors'], 'old host or agent exit unconfirmed; no switch admitted')
        shutil.copy2(file, self.root / 'native-before.jsonl')
        original_bytes = file.read_bytes()
        expected_cwd = self.origin
        if self.scenario == 'cwd-missing':
            self.origin.rmdir()
            expected_cwd = self.default_cwd
        with self.host(self.destination, 'destination'):
            default = self.new_container(self.destination, NAMES[1], self.default_cwd)
            containers = self.containers(self.destination)
            if NAMES[0] not in containers.values():
                self.new_container(self.destination, NAMES[0], self.root / 'home')
                containers = self.containers(self.destination)
            require(containers[default] == NAMES[1] and list(containers.values()).count(NAMES[1]) == 1, containers)
            chosen = default
            if self.scenario == 'cross-end':
                require(original not in containers, ('cross-end expected no matching opaque ID', original, containers))
            elif self.scenario == 'same-end':
                require(containers.get(original) == NAMES[0], ('same-end original ID not retained by installed host', original, containers))
                chosen = original
            elif self.scenario == 'same-value':
                chosen = self.new_container(self.destination, 'different name, same ID by fixture', self.root / 'home')
            elif self.scenario.startswith('git-'):
                self.prepare_git_placement(main)
                expected_cwd = main if self.scenario == 'git-unique' else self.default_cwd
            if self.scenario in ('cwd-missing', 'git-ambiguous'):
                self.default_directory_probe(default, self.destination)
                expected_cwd = self.default_cwds[self.destination]
            self.evidence['placement'] = dict(originalContainer=original, inventory=containers, expected=chosen,
                                              configuredName=NAMES[1], expectedCwd=str(expected_cwd))
            try:
                with self.backend(self.destination, NAMES[1], 'destination'):
                    self.arrange(source_id, file, chosen, old['id'])
                    self.resume(old, file, chosen, expected_cwd)
            finally:
                if self.scenario == 'file-conflict':
                    file.write_bytes(original_bytes)
                shutil.copy2(file, self.root / 'native-after.jsonl')

    def prepare_git_placement(self, main):
        self.herdr('worktree', 'open', '--path', str(self.origin), '--label', 'owned Git checkout', '--no-focus')
        if self.scenario == 'git-ambiguous':
            shutil.rmtree(self.origin)
            other = self.git_root('git-other', self.origin)
            self.herdr('worktree', 'open', '--path', str(self.origin), '--label', 'another Git checkout', '--no-focus')
        else:
            other = None
        workspaces = self.herdr('workspace', 'list')['workspaces']
        exact = [w['worktree']['repo_key'] for w in workspaces if w.get('worktree') and
                 same_path(w['worktree']['checkout_path'], self.origin)]
        known = {Path(x).resolve() for x in exact}
        expected = {main / '.git'} | ({other / '.git'} if other else set())
        require(known == expected, ('Installed TUI must demonstrate exact Git metadata, not matching names', workspaces, expected))
        self.evidence['git'] = {str(p): self.git(p, 'worktree', 'list', '--porcelain') for p in expected}
        shutil.rmtree(self.origin)
        require(not self.origin.exists(), 'historical checkout still exists')

    def default_directory_probe(self, container, host):
        # An independent native blank Tab supplies the host's own default cwd;
        # App may not use the historical repo or even the Space's creation folder.
        if host == 'tui':
            tab = self.herdr('tab', 'create', '--workspace', container, '--label', 'native default cwd control', '--no-focus')
            pane = self.herdr('pane', 'get', tab['root_pane']['pane_id'])['pane']
            cwd = pane.get('cwd')
        else:
            tab = self.native(self.args.app, 'do', 'tab.new', json.dumps({'space': container,
                                               'name': 'native default cwd control', 'focus': False}))
            term = self.native(self.args.core, 'get', tab['terminal'], '--json')
            cwd = (term.get('foreground') or {}).get('cwd') or term.get('cwd')
        require(cwd and Path(cwd).is_dir() and Path(cwd).resolve().is_relative_to(self.root.resolve()),
                ('Native default cwd not independently observed in isolation', tab, cwd))
        self.default_cwds[host] = Path(cwd)

    def run(self, target):
        target.mkdir(exist_ok=False)
        try:
            self.exercise()
            self.evidence['passed'] = not self.evidence['cleanupErrors']
        except Exception:
            self.evidence['verificationError'] = traceback.format_exc()
        finally:
            if self.model:
                self.model.shutdown()
                self.model.server_close()
                self.model_thread.join(timeout=10)
                if self.model_thread.is_alive():
                    self.evidence['cleanupErrors'].append('Owned model server thread still alive')
                dump(self.root / 'controlled-provider.json', self.model.requests)
            self.evidence['http'] = self.reports
            self.evidence['passed'] = self.evidence['passed'] and not self.evidence['cleanupErrors']
            try:
                for path in self.root.iterdir():
                    if path.is_file() and path.suffix in ('.log', '.json', '.jsonl', '.toml', '.ts'):
                        shutil.copy2(path, target / path.name)
                if hasattr(self, 'recorders'):
                    dump(target / 'forwarded-host-calls.json', self.calls_since({}))
                if self.history_path.is_file():
                    shutil.copy2(self.history_path, target / 'history-final.json')
                # The isolated state is deliberately retained for independent
                # inspection. Evidence uses exit handles, not folder removal.
            except Exception:
                self.evidence['cleanupErrors'].append(traceback.format_exc())
                self.evidence['passed'] = False
            dump(target / 'evidence.json', self.evidence)
        return self.evidence


def tui_focus(snapshot):
    # Public snapshot focus, not a command exit or Windows foreground claims.
    return tuple(snapshot['snapshot'].get(f'focused_{key}_id') for key in ('workspace', 'tab', 'pane'))


def history_record(row):
    cwd = row.get('cwd')
    return {**row, 'cwd': os.path.normcase(os.path.normpath(cwd))} if cwd else row


def json_inventory(inventory):
    return {key: sorted(tabs) for key, tabs in inventory.items()}


def observer_source(file, source, observe_input=True):
    input_hook = 'pi.on("before_agent_start", (event, ctx) => observe("before_agent_start", ctx, event.prompt));' if observe_input else ''
    return f'''import {{appendFileSync}} from "node:fs";
export default function(pi) {{
  const observe = (event, ctx, prompt) => appendFileSync({q(file)}, JSON.stringify({{
    source: {q(source)}, event, pid: process.pid, cwd: process.cwd(), nativeCwd: ctx.cwd,
    id: ctx.sessionManager.getSessionId(), file: ctx.sessionManager.getSessionFile(),
    model: ctx.model?.id, commands: pi.getCommands().map(c=>c.name), prompt
  }}) + "\\n");
  pi.on("session_start", (_, ctx) => observe("session_start", ctx));
  {input_hook}
}}
'''
