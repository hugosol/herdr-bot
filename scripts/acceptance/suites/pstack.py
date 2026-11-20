"""S2: public Task/Chat HTTP. S3: installed OMP, its model wire and native history.

P46 tracer: a valid Suite whose declared file vanishes (or is really denied)
must fail both operations before a Tab or agent is started. A successful
fallback after Resume is a failure, even if it produces no model requests.
"""
from contextlib import contextmanager
import errno
import json
import os
from pathlib import Path
import re
import secrets
import shutil
import socket
import subprocess
import sys
import threading
import tomllib
import urllib.error
import urllib.request

from acceptance.host import native_observation
from acceptance.host.completion_transport import NativeCompletionTransport
from acceptance.host.isolation import Acceptance, require, until
from acceptance.host.provenance import command, digest, source_identity
from acceptance.host.smoke_support import deny_read, dump
from developer_paths import REPO


CASES = {'resource-failure': ['P46'], 'assembly': ['P2', 'P3', 'P14'], 'mode-resume-on': ['P17/on'],
         'mode-resume-off': ['P17/off'], 'source-base': ['P20'], 'role-lifecycle': ['P24', 'P28', 'P31']}
ACK = 'PSTACK-OWNED-TURN-COMPLETE-614'
PROJECT = 'PSTACK-OWNED-PROJECT-INSTRUCTIONS-791'
PROVIDER = 'pstack-acceptance'
ALPHA, BETA = 'acceptance-alpha', 'acceptance-beta'
TIMEOUT = 65


def write(path, text):
    path.write_text(text, encoding='utf-8')


def configure(args, report):
    args.repo, args.toolkit = args.herdr_bot_worktree, args.toolkit_worktree
    require(sys.platform == 'win32', 'Installed Herdr TUI acceptance requires Windows')
    require(args.repo == REPO == Path.cwd().resolve(), 'Run from the explicitly supplied herdr-bot worktree root')
    require(args.repo != args.toolkit and all((repo / '.git').is_file() for repo in (args.repo, args.toolkit)),
            'Two explicit Git worktrees are required')
    cfg = json.loads(args.isolated_config.read_text(encoding='utf-8'))
    require(cfg['authorization'] == 'owned-local-mechanics-only', 'Owned-local mechanics authorization required')
    require(set(cfg) == {'authorization', 'versions', 'herdr', 'core', 'node', 'bun', 'omp_cli', 'omp_package',
                         'omp_state_extension'}, 'Installation JSON contains missing or unexpected keys')
    for key in ('herdr', 'core', 'node', 'bun', 'omp_cli', 'omp_package', 'omp_state_extension'):
        path = Path(cfg[key])
        require(path.is_absolute(), ('Installation path must be absolute', key))
        require(path.is_dir() if key == 'omp_package' else path.is_file(), ('Missing installation', key, str(path)))
        setattr(args, key, path.resolve())
    package_json = args.omp_package / 'package.json'
    pkg = json.loads(package_json.read_text(encoding='utf-8'))
    args.omp_entry = args.omp_package / 'dist/cli.js'
    require(args.omp_cli.name.casefold() == 'omp.cmd' and pkg['name'] == '@oh-my-pi/pi-coding-agent'
            and (args.omp_cli.parent / 'node_modules' / pkg['name']).resolve() == args.omp_package
            and f"{pkg['name']}/dist/cli.js" in args.omp_cli.read_text(encoding='utf-8').replace('\\', '/'),
            'OMP shim must address the selected installed package')
    bundled_bun = args.omp_cli.parent / 'bun.exe'
    require(not bundled_bun.is_file() or bundled_bun.samefile(args.bun), 'OMP shim would select a different Bun')
    git = shutil.which('git.exe')
    require(git, 'git.exe required from the installed PATH')
    args.git = Path(git).resolve()
    args.recorder = args.backend.parent / 'examples/recovery_herdr_proxy.exe'
    report['artifacts'] = {}
    for executable, source in ((args.backend, 'backend/src/main.rs'), (args.recorder, 'backend/examples/recovery_herdr_proxy.rs'),
                               (args.backend.parent / 'examples/safe_shutdown_shell.exe', 'backend/examples/safe_shutdown_shell.rs')):
        depfile = executable.with_suffix('.d').read_text(encoding='utf-8').replace('\\', '/')
        relative = source.removeprefix('backend/') if '/examples/' in source else None
        require(executable.is_file() and ((REPO / source).as_posix().casefold() in depfile.casefold()
                or (relative and relative + ':' in depfile)), ('Wrong build worktree', str(executable)))
        require((REPO / source).stat().st_mtime <= executable.stat().st_mtime, ('Artifact predates source', str(executable)))
        report['artifacts'][str(executable)] = {'sha256': digest(executable), 'depfile': depfile}
    require(args.backend.name == 'herdr-bot.exe', 'Production backend is required, not the safe shutdown shell')
    require(all(path.stat().st_mtime <= args.backend.stat().st_mtime for path in
                [*REPO.glob('backend/src/**/*.rs'), REPO / 'backend/Cargo.lock', REPO / 'backend/Cargo.toml']),
            'Backend predates source/manifest/lock')
    page = REPO / 'backend/static/index.html'
    require(all(p.stat().st_mtime <= page.stat().st_mtime for p in REPO.glob('frontend/src/**/*') if p.is_file()),
            'Frontend predates source')
    report['page'] = {'path': str(page), 'sha256': digest(page)}
    report['installationConfig'] = {'path': str(args.isolated_config), 'sha256': digest(args.isolated_config)}
    report['installed'] = {}
    for key, argv in {'herdr': [args.herdr], 'core': [args.core], 'node': [args.node], 'bun': [args.bun],
                      'omp': [args.bun, args.omp_entry]}.items():
        actual = command([*argv, '--version'], REPO).strip()
        report['installed'][key] = {'command': list(map(str, [*argv, '--version'])), 'stdout': actual, 'exit': 0,
                                    'sha256': digest(argv[-1])}
        require(actual.split()[-1].split('/')[-1] == cfg['versions'][key], ('Installed version mismatch', key, actual))
    require(cfg['versions']['bun'] == '1.4.4' and cfg['versions']['omp'] == pkg['version'], 'Bun/package version mismatch')
    report['installedFiles'] = {str(path): digest(path) for path in (args.omp_cli, package_json, args.omp_state_extension)}
    report['sources'] = {name: source_identity(repo, lock, args.evidence_dir) for name, repo, lock in
                         [('herdr-bot', args.repo, 'backend/Cargo.lock'), ('toolkit', args.toolkit, 'bun.lock')]}


class PstackAcceptance(Acceptance):
    agent = 'omp'  # The shared native observer uses this to open installed OMP history.
    history = native_observation.history
    remember_process = native_observation.remember_process
    capture = native_observation.capture

    def __init__(self, args, case):
        # Keep Herdr, OMP, and child worktree paths short on Windows. Retain the
        # owned root even after cleanup as evidence; never remove user resources.
        from copy import copy
        native_args = copy(args)
        native_args.evidence_dir = None
        super().__init__(native_args, prefix='ps13-', promise=CASES[case], session_prefix='ps13-')
        self.case = case
        self.prepared = False
        self.handles = {}
        self.handles_lock = threading.Lock()
        self.sessions = []
        self.turns = {}
        self.transport = None
        self.zone = self.root / 'zone'
        self.agent_dir = self.root / 'agent'
        self.deployment = self.root / 'owned toolkit'
        self.recorder = self.root / 'recorder'
        self.native_log = self.root / 'native-events.jsonl'
        self.history_file = self.root / 'data/history.json'
        self.public_log = args.evidence_dir / case
        self.public_log.mkdir()
        self.env = {k: v for k, v in self.env.items() if not any(word in k.upper() for word in
                    ('PROXY', 'CREDENTIAL', 'PASSWORD', 'SECRET', 'API_KEY', 'AUTH', 'CODEX', 'GEMINI', 'GOOGLE', 'AWS_', 'HC_', 'GIT_'))}
        # Use the explicit installed shim (and its real package) first. Git is
        # already installed; no ambient extensions, configs, or credential files.
        self.env['PATH'] = os.pathsep.join(dict.fromkeys(map(str, [args.bun.parent, args.omp_cli.parent, args.node.parent,
                args.git.parent, args.herdr.parent, Path(os.environ['SystemRoot']) / 'System32',
                Path(os.environ['SystemRoot']) / 'System32/WindowsPowerShell/v1.0'])))
        # Installed OMP uses the historical PI_CODING_AGENT_DIR selector.
        # Bind both to one owned location; neither selects an existing Pi home.
        self.env.update(PI_CODING_AGENT_DIR=str(self.agent_dir), OMP_CODING_AGENT_DIR=str(self.agent_dir),
                        XDG_CACHE_HOME=str(self.root / 'cache'),
                        XDG_DATA_HOME=str(self.root / 'state'), GIT_CONFIG_NOSYSTEM='1', GIT_TERMINAL_PROMPT='0',
                        GIT_CONFIG_GLOBAL=str(self.root / 'gitconfig'), GIT_TRACE2_EVENT=str(self.root / 'git-trace.jsonl'),
                        GCM_INTERACTIVE='Never')
        self.evidence['case'] = case
        self.evidence['isolation'].update({k: self.env[k] for k in ('OMP_CODING_AGENT_DIR', 'PATH', 'XDG_CACHE_HOME', 'GIT_CONFIG_GLOBAL')})

    def command(self, argv, seconds=60):
        if str(argv[0]) == 'bun':
            argv = [self.args.bun, *argv[1:]]
        return super().command(argv, seconds)

    def deploy(self):
        for name in ('adapters', 'framework', 'extensions', 'entrypoints', 'pstack-suite'):
            shutil.copytree(self.args.toolkit / name, self.deployment / name,
                            ignore=shutil.ignore_patterns('.runtime', 'node_modules', '.git', '*.local.json', 'roles.local.json'))
        shutil.copy2(self.args.toolkit / 'package.json', self.deployment / 'package.json')
        package = json.loads((self.deployment / 'package.json').read_text(encoding='utf-8'))
        for dependency in package.get('dependencies', {}):
            shutil.copytree(self.args.toolkit / 'node_modules' / dependency, self.deployment / 'node_modules' / dependency)
        self.suite = self.deployment / 'pstack-suite'
        self.generated = self.suite / 'generated'
        self.skill = self.generated / 'skills/setup-pstack/SKILL.md'
        require(self.skill.is_file(), 'Missing shipped pstack setup resource; no implicit build')
        require(not (self.generated / 'agents').exists(), 'Use a worktree with unconfigured shipped pstack outputs')
        require(not (self.args.toolkit / 'pstack-suite/models.local.json').exists(), 'Do not use a developer-configured package')
        self.evidence['deployment'] = {'source': str(self.args.toolkit), 'root': str(self.deployment),
                                       'generatedAtCopy': self.tree(self.generated)}

    @staticmethod
    def tree(root):
        return {path.relative_to(root).as_posix(): ({'sha256': digest(path), 'mtimeNs': path.stat().st_mtime_ns}
                if path.is_file() else {'kind': 'directory'}) for path in sorted(root.rglob('*'))}

    def prepare(self):
        self.deploy()
        # Observe short-lived Bun subprocess calls too, not just the long-lived
        # OMP PID and Herdr's CLI requests. No model/tool/config interception.
        if self.case == 'assembly':
            trace = self.public_log / 'bun-processes.jsonl'
            observer = self.root / 'bun-process-observer.mjs'
            write(observer, 'import {appendFileSync} from "node:fs";\n'
                  'appendFileSync(' + json.dumps(str(trace)) + ', JSON.stringify({pid:process.pid, '
                  'ppid:process.ppid, cwd:process.cwd(), argv:process.argv, execArgv:process.execArgv}) + "\\n");\n')
            require(not any(character.isspace() for character in str(observer)),
                    'Bun process observer requires an owned Windows TEMP path without whitespace')
            self.env['BUN_OPTIONS'] = f'--preload={observer.as_posix()}'
            self.command([self.args.bun, '-e', 'console.log("owned preload calibration")'])
            require(trace.is_file(), 'Installed Bun did not run the owned process observer')
        self.transport = NativeCompletionTransport(self.decide, self.public_log / 'model')
        self.transport.__enter__()
        self.prepare_native()
        self.install_observer()
        self.prepare_catalogs()
        self.recorder.mkdir()
        shutil.copy2(self.args.recorder, self.recorder / self.args.recorder.name)
        dump(self.recorder / 'recovery-proxy.json', {'herdr': str(self.args.herdr), 'recordSubmission': True})
        write(self.root / 'herdr.toml', 'onboarding = false\n[terminal]\ndefault_shell = "cmd.exe"\n'
              '[update]\nversion_check = false\nmanifest_check = false\n')
        self.env['HERDR_CONFIG_PATH'] = str(self.root / 'herdr.toml')
        self.start([self.args.herdr, 'server'], 'herdr')
        self.herdr_started = True
        until('owned Herdr server', lambda: subprocess.run([str(self.args.herdr), 'workspace', 'list'], cwd=self.root,
              env=self.env, capture_output=True, timeout=5).returncode == 0)
        workspace = self.herdr('workspace', 'create', '--cwd', str(self.zone), '--label', 'pstack-owned', '--no-focus')
        self.workspace = workspace['workspace']['workspace_id']
        self.evidence['workspace'] = workspace
        self.start_backend()
        self.http('/api/zones', {'id': 'pstack-owned-zone', 'path': str(self.zone)})
        self.prepared = True

    def prepare_native(self):
        write(self.zone / 'AGENTS.md', PROJECT + '\n')
        for argv in (['init', '-q'], ['add', 'AGENTS.md'], ['-c', 'user.name=Owned suite fixture', '-c',
                      'user.email=fixture@example.invalid', '-c', 'commit.gpgsign=false', 'commit', '-qm', 'Owned project']):
            self.command([self.args.git, '-C', self.zone, *argv])
        write(self.agent_dir / 'config.yml', f'''autoResume: false
startup:
  setupWizard: false
  showSplash: false
  checkUpdate: false
marketplace:
  autoUpdate: off
completion:
  notify: "off"
async:
  enabled: false
modelRoles:
  default: {PROVIDER}/{ALPHA}
task:
  isolation:
    enabled: true
    apply: false
isolation:
  backend: rcopy
''')
        dump(self.agent_dir / 'models.yml', {'providers': {PROVIDER: {'baseUrl': self.transport.base,
            'api': 'openai-completions', 'apiKey': 'owned-loopback-not-a-credential', 'models': [
                {'id': m, 'name': m, 'reasoning': True, 'input': ['text'], 'contextWindow': 65536,
                 'maxTokens': 2048, 'cost': {'input': 0, 'output': 0, 'cacheRead': 0, 'cacheWrite': 0}}
                for m in (ALPHA, BETA)]}}})
        ambient = self.agent_dir / 'skills'
        for name, body in [('setup-pstack', 'WRONG-SAME-NAME-SOURCE-321'), ('ambient-pstack-decoy', 'UNSELECTED-SET-732')]:
            path = ambient / name / 'SKILL.md'
            path.parent.mkdir(parents=True)
            write(path, f'---\nname: {name}\ndescription: Owned alternate source\n---\n{body}\n')
        state = self.agent_dir / 'extensions/herdr-omp-agent-state.ts'
        state.parent.mkdir()
        shutil.copy2(self.args.omp_state_extension, state)

    def install_observer(self):
        self.observer = self.root / 'observer.ts'
        source = '''import {appendFileSync} from "node:fs";
export default function (omp) {
  for (const event of ["session_start", "before_agent_start", "agent_end"]) {
    omp.on(event, (_, ctx) => appendFileSync(EVENTS, JSON.stringify({event, pid: process.pid,
      cwd: process.cwd(), id: ctx.sessionManager.getSessionId(), file: ctx.sessionManager.getSessionFile(),
      model: ctx.model && {provider: ctx.model.provider, id: ctx.model.id},
      thinking: omp.getThinkingLevel(), commands: omp.getCommands().map(command => command.name)}) + "\\n"));
  }
}
'''
        write(self.observer, source.replace('EVENTS', json.dumps(str(self.native_log))))

    def prepare_catalogs(self):
        production = json.loads((REPO / 'meta-data/suites.json').read_text(encoding='utf-8'))
        chosen = next(suite for suite in production['suites'] if suite['id'] == 'pstack')
        self.suites = self.root / 'suites.json'
        dump(self.suites, {'suites': [chosen, {'id': 'owned-other', 'name': 'Owned other Suite', 'agents': ['omp'],
                                                       'skills': {'omp': [str(self.agent_dir / 'skills')]}}]})
        dump(self.root / 'mcp.json', {'mcpServers': {}})
        write(self.root / 'paths.toml', '[roots]\ntoolkit = ' + json.dumps(str(self.deployment)) +
              '\nomp_agent = ' + json.dumps(str(self.agent_dir)) + '\n')
        shipped = tomllib.loads((REPO / 'meta-data/task-options.toml').read_text(encoding='utf-8'))
        extensions = [entry for entry in shipped['extensions'] if entry['agent'] == 'omp' and entry['kind'] == 'fixed']
        extensions.append({'id': 'pstack-owned-observer', 'label': 'Observing installed OMP', 'agent': 'omp',
                           'kind': 'fixed', 'entrypoint': str(self.observer)})
        options = f'''skills = []
[[agents]]
id = "omp"
label = "OMP"
[[models]]
id = "default"
label = "Default"
[[models]]
id = "probe"
label = "Controlled alpha"
provider_model = "{PROVIDER}/{ALPHA}"
thinking = {{ omp = ["high"] }}
'''
        for entry in extensions:
            options += '\n[[extensions]]\n' + ''.join(f'{key} = {json.dumps(value)}\n' for key, value in entry.items())
        write(self.root / 'options.toml', options)

    def start_backend(self):
        for _ in range(32):
            port = 49152 + secrets.randbelow(16384)
            try:
                with socket.socket() as reserved:
                    reserved.bind(('127.0.0.1', port))
                break
            except OSError as error:
                if error.errno != errno.EADDRINUSE:
                    raise
        else:
            raise RuntimeError('No available loopback port')
        self.base = f'http://127.0.0.1:{port}'
        config = f'listen_addr = "127.0.0.1:{port}"\n[mdns]\nenabled = false\n'
        header, _, sections = config.partition('[mdns]')
        paths = lambda **items: ''.join(f'{k} = {json.dumps(str(v))}\n' for k, v in items.items())
        config = header + paths(paths_file=self.root / 'paths.toml', static_directory=REPO / 'backend/static',
                                data_directory=self.root / 'data') + '[mdns]' + sections
        for name in ('start', 'stop'):
            config += f'[operations.{name}]\n' + paths(script=self.root / 'not-authorized.bat', working_directory=self.root)
        config += '[herdr]\nbackend = "tui"\nworkspace_name = "pstack-owned"\n' + paths(
            executable=self.recorder / self.args.recorder.name, core_executable=self.args.core)
        config += '[tasks]\n' + paths(storage_file=self.root / 'data/tasks.json', metadata_file=self.root / 'options.toml')
        config += '[suites]\n' + paths(definition_file=self.suites, mcp_file=self.root / 'mcp.json',
                                      generated_directory=self.root / 'per launch config')
        config += '[sessions]\n' + paths(history_file=self.history_file)
        config += '[zones]\n' + paths(registry_file=self.root / 'data/zones.json')
        config += '[scheduler]\n' + paths(map_directory=self.root / 'maps', bot_directory=self.root / 'bots',
                                          run_directory=self.root / 'data/runs', run_instructions=REPO / 'docs/workflow-runtime.md')
        write(self.root / 'backend.toml', config)
        for name in ('connect-bot', 'workflow-chat'):
            path = self.deployment / 'extensions' / name / 'config.yaml'
            text, count = re.subn(r'(?m)^endpoint:.*$', 'endpoint: ' + self.base, path.read_text(encoding='utf-8'))
            require(count == 1, ('Missing shipped extension endpoint', str(path)))
            write(path, text)
        backend = self.start([self.args.backend, '--config', self.root / 'backend.toml'], 'backend')
        def listening():
            require(backend.child.poll() is None, 'Production backend exited; see raw logs')
            while not backend.lines.empty():
                if self.base + '/' in backend.lines.get_nowait():
                    return True
        until('production backend listening', listening)
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        with opener.open(self.base + '/', timeout=10) as response:
            page = response.read()
            require(response.status == 200 and page == (REPO / 'backend/static/index.html').read_bytes(),
                    'Production HTTP did not serve the packaged worktree page')
            (self.public_log / 'page.html').write_bytes(page)

    def request(self, method, path, body=None):
        url = self.base + path
        req = urllib.request.Request(url, method=method, data=None if body is None else json.dumps(body).encode(),
                                     headers={'Content-Type': 'application/json'})
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        try:
            response = opener.open(req, timeout=TIMEOUT + 10)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            raw = response.read().decode('utf-8')
            self.evidence.setdefault('http', []).append({'method': method, 'url': url, 'request': body,
                                                        'status': response.status, 'response': raw})
            return response.status, json.loads(raw)

    def http(self, path, body=None, status=200):
        code, result = self.request('GET' if body is None else 'POST', path, body)
        require(code == status, (code, result))
        return result

    def task(self, name, suite='pstack'):
        body = {'name': name, 'chatName': 'Owned {chat.id}', 'agent': 'omp', 'taskType': 'normal',
                'model': 'probe' if suite == 'pstack' else 'default', 'thinking': 'high' if suite == 'pstack' else 'default',
                'skill': None, 'prompt': '', 'suite': suite}
        result = self.http('/api/tasks', body)
        return next(t for t in result['tasks'] if t['name'] == name)['id']

    def launch_request(self, task, marker):
        info = {'zone': 'pstack-owned-zone', 'taskId': task}
        preview = self.http('/api/chats/preview', info)
        return self.request('POST', '/api/chats', dict(info, chatId=preview['chatId'], input=marker))

    def events(self):
        if not self.native_log.is_file():
            return []
        return [json.loads(line) for line in self.native_log.read_text(encoding='utf-8').splitlines() if line]

    def launch(self, task, marker):
        offset = len(self.events())
        status, launch = self.launch_request(task, marker)
        require(status == 202, (status, launch))
        def ready():
            current = self.http('/api/chats/' + launch['id'])
            if current['stage'] == 'failed' and current.get('paneId'):
                self.evidence['startupTerminal'] = self.terminal({'chat': current})
            require(current['stage'] != 'failed', current)
            return current if current['stage'] == 'succeeded' else None
        chat = until('public Chat launch', ready, TIMEOUT)
        return self.native(chat, offset)

    def native(self, chat, offset, completed=True):
        # OMP dispatches child events to the same installed observer and may
        # run a child in the same PID. The first Session start belongs to the
        # Chat; correlate round completion by native Session ID, not by PID.
        started = until('main native Session start', lambda: next((e for e in self.events()[offset:]
                        if e['event'] == 'session_start'), None), TIMEOUT)
        event = started
        if completed:
            event = until('main native turn completed', lambda: next((e for e in self.events()[offset:]
                          if e['id'] == started['id'] and e['event'] == 'agent_end'), None), TIMEOUT)
        self.remember_process(event['pid'])
        file = Path(event['file'])
        require(file.is_relative_to(self.agent_dir), ('Native file not owned', file))
        observation = {'chat': chat, 'native': event, 'file': str(file)}
        self.sessions.append(observation)
        self.evidence['sessions'] = self.sessions
        if completed:
            self.wait_history(observation)
        return observation

    def wait_history(self, session):
        file = Path(session['file'])
        until('actual installed OMP turn persisted', lambda: file.is_file() and ACK in file.read_text(encoding='utf-8'), TIMEOUT)
        history = self.history(file)
        require(history['id'] == session['native']['id'] and ACK in json.dumps(history['branch']), history)
        session['history'] = history
        return history

    def start_native_resume(self, previous):
        before = self.history(previous['file'])
        offset = len(self.events())
        request_count = len(self.transport.records)
        result = self.http('/api/chats/resume', {'sessionId': previous['native']['id']})
        require(result['outcome'] == 'restored' and not result.get('degradations'), result)
        session = self.native(result['target'], offset, completed=False)
        require(session['native']['id'] == before['id'] and Path(session['file']).samefile(previous['file'])
                and session['native']['pid'] != previous['native']['pid'], ('Resume did not open the same history in a new process', session))
        require(self.history(session['file'])['branch'] == before['branch'] and len(self.transport.records) == request_count,
                'Resume replayed input or changed the original history before the first explicit turn')
        session['resume'] = result
        return session

    def terminal(self, session):
        return self.command([self.args.herdr, 'pane', 'read', session['chat']['paneId'], '--lines', '200', '--format', 'text'])

    def send(self, session, prompt):
        pane = session['chat']['paneId']
        try:
            self.herdr('agent', 'prompt', pane, prompt)
        except AssertionError:
            session['terminalAtSendFailure'] = self.terminal(session)
            raise

    def turn(self, session, prompt):
        offset = len(self.events())
        file = Path(session['file'])
        acknowledged = lambda: file.read_text(encoding='utf-8').count(ACK)
        prior = acknowledged()
        self.send(session, prompt)
        until('new native turn completed', lambda: any(e['event'] == 'agent_end' and e['id'] == session['native']['id']
              for e in self.events()[offset:]) and acknowledged() > prior, TIMEOUT)
        history = self.wait_history(session)
        require(json.dumps(prompt, ensure_ascii=False)[1:-1] in json.dumps(history['branch'], ensure_ascii=False),
                ('New input missing from native history', prompt))
        return history

    def close(self, session):
        tab, pid = session['chat']['tabId'], session['native']['pid']
        session['terminal'] = self.terminal(session)
        self.capture()
        self.herdr('tab', 'close', tab)
        until('owned native PID exit before next launch or publication', lambda: not self.handles[pid].alive(), 20)
        session['exited'] = True
        shutil.copy2(session['file'], self.public_log / f'history-{pid}.jsonl')

    def decide(self, request):
        from acceptance.suites.pstack_cases import decision
        return decision(self, request)

    def calls(self):
        return [json.loads(path.read_text(encoding='utf-8')) for path in sorted((self.recorder / 'calls').glob('*.json'))]

    def startup_effects(self):
        return {'tabs': self.herdr('tab', 'list', '--workspace', self.workspace), 'events': self.events(),
                'historyIndex': digest(self.history_file),
                'nativeFiles': self.tree(self.agent_dir / 'sessions'),
                'launchConfigs': self.tree(self.root / 'per launch config'), 'requests': len(self.transport.records)}

    @contextmanager
    def missing(self, path):
        moved = path.with_suffix('.acceptance-moved')
        path.rename(moved)
        try:
            require(not path.exists(), 'Missing-resource fixture did not take effect')
            yield
        finally:
            moved.rename(path)

    def resource_failure(self):
        task = self.task('Resource failure source')
        original = self.launch(task, 'PSTACK-NEGATIVE-REAL-HISTORY-124')
        self.close(original)
        failures = self.evidence['negative'] = []
        entry = self.suite / 'omp.ts'  # A declared file, not an optional member of a selected collection.
        for mechanism, unavailable in [('missing', self.missing(entry)), ('unreadable', deny_read(entry))]:
            with unavailable:
                before = self.startup_effects()
                offset = len(self.calls())
                status, creation = self.launch_request(task, 'Do not start if a declared resource is unavailable')
                created_negative = self.calls()[offset:]
                resumed, response = self.request('POST', '/api/chats/resume', {'sessionId': original['native']['id']})
                after = self.startup_effects()
                calls = self.calls()[offset:]
                failures.append({'fault': mechanism, 'file': str(entry), 'launchStatus': status, 'launch': creation,
                                 'resumeStatus': resumed, 'resume': response, 'before': before, 'after': after, 'hostCalls': calls})
                mutating = lambda call: call['argv'][:2] in (['tab', 'create'], ['agent', 'start'], ['agent', 'prompt'])
                require(status >= 400 and not any(map(mutating, created_negative)), ('Creation must reject before start', status, creation))
                require(resumed >= 400 and response.get('outcome') == 'failed' and response.get('stage') == 'settings'
                        and not response.get('target'), ('Resume must reject the resource, not fall back', resumed, response))
                for failure in (creation, response):
                    require(entry.name in json.dumps(failure, ensure_ascii=False), ('Missing resource-specific diagnostic', failure))
                require(not any(c['argv'][:2] in (['tab', 'create'], ['agent', 'start'], ['agent', 'prompt']) for c in calls), calls)
                require(before == after, ('Observed startup or history effects after rejection', before, after))

    def exercise(self):
        from acceptance.suites import pstack_cases
        try:
            if self.case == 'resource-failure':
                self.resource_failure()
            else:
                getattr(pstack_cases, self.case.replace('-', '_'))(self)
            require(not self.transport.failed, 'Controlled provider observed a transport/decision failure')
            self.evidence['passed'] = True
        finally:
            # Record live descendants on failing paths too, before the
            # inherited stop touches only the uniquely named owned server.
            self.capture()
            for session in self.sessions:
                if not session.get('exited'):
                    session['terminalBeforeCleanup'] = self.terminal(session)
                    path = Path(session['file'])
                    if path.is_file():
                        shutil.copy2(path, self.public_log / f"history-{session['native']['pid']}-before-cleanup.jsonl")

    def cleanup_extra(self):
        if self.transport:
            self.transport.__exit__(None, None, None)
        exits = self.evidence['ownedProcessExits'] = []
        # The installed worker can outlive both the Tab and the named Herdr
        # server. Only terminate a daemon from our recorded process tree with
        # its original kernel handle still held; never search or clean by name.
        detached_workers = {proc['ProcessId'] for snapshot in self.evidence.get('ownedSnapshots', []) for proc in snapshot
                            if (proc.get('CommandLine') or '').endswith(' __omp_worker_daemon')}
        for pid, handle in self.handles.items():
            try:
                if pid in detached_workers and handle.alive():
                    try:
                        self.command(['taskkill.exe', '/PID', str(pid), '/F'], seconds=20)
                    except AssertionError:
                        if handle.alive():
                            raise
                until(f'owned native PID {pid} exit', lambda: not handle.alive(), 20)
                exits.append({'pid': pid, 'exited': True, 'detachedWorker': pid in detached_workers})
            except Exception as error:
                self.evidence['cleanupErrors'].append(str(error))
                exits.append({'pid': pid, 'exited': False})
            finally:
                handle.close()
        self.evidence['serviceExits'] = [{'pid': p.child.pid, 'exitCode': p.child.poll()} for p in self.processes]
        for name in ('native-events.jsonl', 'backend.toml', 'options.toml', 'paths.toml', 'suites.json', 'herdr.toml'):
            path = self.root / name
            if path.is_file():
                shutil.copy2(path, self.public_log / name)
        for path in self.root.glob('*.log'):
            shutil.copy2(path, self.public_log / path.name)
        for name in ('calls', 'submissions'):
            directory = self.recorder / name
            if directory.is_dir():
                shutil.copytree(directory, self.public_log / name)
