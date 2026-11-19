"""S1 public node operations and S3 installed Herdr/agent/OS observations."""
from contextlib import ExitStack
from datetime import datetime, timezone
import errno
import json
import os
from pathlib import Path
import re
import secrets
import shutil
import socket
import stat
import subprocess
import sys
import threading
import time
import traceback
from types import SimpleNamespace
import uuid

from .host.completion_transport import NativeCompletionTransport
from .host.isolation import Acceptance
from .host.processes import LiveProcess, require, until
from .host.provenance import digest
from .host.wire import WireProxy
from .paired_reminder_observation import Loopback, foreground_window, identify_audio, launch_tui_client

CLOSE_CASES = ('extra-tab', 'last-tab', 'group-refused')
APPROVAL_QUESTION = '当前会话的成果是否已达到预期，可以交接并推进后续流程？'


def check_close(before, after, space, tab, target_exited, others_alive, refused):
    require(tab in before[space], 'target missing before complete')
    expected = {key: set(tabs) for key, tabs in before.items()}
    if not refused:
        expected[space].remove(tab)
        if not expected[space]:
            del expected[space]
    require(after == expected, ('native layout differs', expected, after))
    require(others_alive, 'unrelated process did not survive')
    require(target_exited != refused, 'target process exit does not match close disposition')


def check_sound(expected, observed):
    require(observed is not None, 'playback observation unavailable')
    require(observed == ([] if expected is None else [expected]), ('unexpected playback', expected, observed))


def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding='utf-8')


def timestamp():
    return {'utc': datetime.now(timezone.utc).isoformat(), 'monotonic': time.monotonic()}


def remove_owned_root(root):
    def remove_readonly(action, path, exception):
        error = exception[1]
        if not isinstance(error, PermissionError):
            raise error
        info = os.lstat(path)
        # os.access reports read-only Windows directories as writable. Use
        # the actual mode, and never chmod through a symlink or junction.
        if (info.st_mode & stat.S_IWRITE or getattr(info, 'st_reparse_tag', 0)
            or not (stat.S_ISREG(info.st_mode) or stat.S_ISDIR(info.st_mode))):
            raise error
        os.chmod(path, info.st_mode | stat.S_IWRITE)
        action(path)

    shutil.rmtree(root, onerror=remove_readonly)


class Scenario(Acceptance):
    def __init__(self, args, host, agent, case, archive):
        # Keep native Unix/Windows socket names short, independently of the archive path.
        super().__init__(SimpleNamespace(**dict(vars(args), evidence_dir=None)),
            prefix='hb15-', promise='P43–P45/P47–P48/P54–P55', session_prefix='hb15-')
        self.args = args
        self.host, self.agent, self.case = host, agent, case
        self.archive = archive
        self.handles = {}
        self.agent_processes = {}
        self.recorders = {}
        self.stack = ExitStack()
        self.cores = False
        self.gates = {}
        self.called = set()
        self.env = {k: v for k, v in self.env.items() if not any(token in k.upper() for token in
            ['SECRET', 'PASSWORD', 'CREDENTIAL', 'PROXY', 'OMPCODE', 'NEXTEST']) and not k.upper().startswith('HC_')}
        for name in ('a', 'tmp', 'cache', 'runtime'):
            (self.root / name).mkdir()
        self.env.update(HERDR_APP_HOME=str(self.root / 'a'), HERDR_CORE_DIRS=str(args.core.parent),
            HC_SESSION='hb15-' + uuid.uuid4().hex[:12], OMP_CODING_AGENT_DIR=str(self.root / 'agent'),
            XDG_DATA_HOME=str(self.root / 'state'), XDG_CACHE_HOME=str(self.root / 'cache'),
            XDG_RUNTIME_DIR=str(self.root / 'runtime'), TEMP=str(self.root / 'tmp'), TMP=str(self.root / 'tmp'))
        self.evidence.update(host=host, agent=agent, case=case, timeline=[], processExits=[],
            scope='Installed agents/hosts and production HTTP; controlled loopback model only, no model judgment or listening claim')
        self.evidence['isolation'].update({key: self.env[key] for key in (
            'HERDR_APP_HOME', 'HC_SESSION', 'OMP_CODING_AGENT_DIR', 'XDG_DATA_HOME', 'XDG_CACHE_HOME', 'TEMP')})

    def note(self, event, **details):
        value = dict(timestamp(), event=event, **details)
        self.evidence['timeline'].append(value)
        return value

    def run(self):
        try:
            self.prepare()
            if self.case in ('extra-tab', 'last-tab', 'group-refused'):
                self.exercise_close()
            else:
                self.exercise_reminders()
            self.evidence['passed'] = True
        except Exception:
            self.evidence['verificationError'] = traceback.format_exc()
        finally:
            self.cleanup()
            errors = self.evidence['cleanupErrors']
            self.evidence['passed'] = self.evidence['passed'] and not errors
            self.evidence['nativeRoot'] = str(self.root)
            self.evidence['archive'] = str(self.archive)
            save(self.root / 'evidence.json', self.evidence)
            try:
                # Copy original observations (not the installed dependency deployment).
                shutil.copytree(self.root, self.archive, ignore=lambda path, names: [name for name in names
                    if name.endswith('.sock') or (Path(path) == self.root and name == 'toolkit')])
                if not errors:
                    remove_owned_root(self.root)
            except (OSError, shutil.Error):
                errors.append(traceback.format_exc())
                self.evidence['passed'] = False
                if self.archive.is_dir():
                    save(self.archive / 'evidence.json', self.evidence)
                if self.root.is_dir():
                    save(self.root / 'evidence.json', self.evidence)
        return self.evidence

    def native_command(self, path, *args, check=True):
        argv = [str(path), *map(str, args)]
        result = subprocess.run(argv, cwd=self.root, env=self.env, capture_output=True,
            encoding='utf-8', errors='replace', timeout=20)
        record = dict(timestamp(), argv=argv, exit=result.returncode, stdout=result.stdout, stderr=result.stderr)
        self.evidence['commands'].append(record)
        if not check:
            return record
        require(result.returncode == 0, record)
        return json.loads(result.stdout)

    def app(self, *args, check=True):
        return self.native_command(self.args.app, *args, check=check)

    def app_do(self, command, **args):
        return self.app('do', command, json.dumps(args))

    def core(self, *args, check=True):
        return self.native_command(self.args.core, *args, '--json', check=check)

    def launch_recorder(self, name, installed):
        directory = self.root / 'wire' / name
        directory.mkdir(parents=True)
        executable = directory / installed.name
        shutil.copy2(self.args.recorder, executable)
        save(directory / 'installed.json', str(installed))
        self.recorders[name] = directory
        return executable

    def configure_provider(self):
        def decision(request):
            labels = [m for message in request.get('messages', []) if message.get('role') == 'user'
                for m in re.findall(r'HB15-([a-f0-9]{12})', json.dumps(message.get('content', '')))]
            label = labels[-1] if labels else None
            if label in self.gates and label not in self.called:
                self.called.add(label)
                entered, release, blocked = self.gates[label]
                self.note('provider-entered', marker=label)
                entered.set()
                require(release.wait(self.args.timeout), ('controlled provider gate was never released', label))
                self.note('provider-released', marker=label)
                if blocked:
                    require(any(t.get('function', {}).get('name') == 'approve' for t in request.get('tools', [])),
                        'native workflow approval tool unavailable')
                    return {'content': '', 'tool_calls': [{'name': 'approve', 'arguments': {}}]}
            return {'content': 'HB15-ACK' + (f'-{label}' if label else ''), 'tool_calls': []}

        self.provider = self.stack.enter_context(NativeCompletionTransport(decision, self.root / 'provider'))
        agent = self.root / 'agent'
        model = {'providers': {'owned': {'baseUrl': self.provider.base, 'api': 'openai-completions',
            'apiKey': 'owned-noncredential', 'models': [{'id': 'mechanical', 'name': 'Owned mechanical fixture',
                'reasoning': False, 'input': ['text'], 'contextWindow': 128000, 'maxTokens': 4096,
                'cost': {'input': 0, 'output': 0, 'cacheRead': 0, 'cacheWrite': 0}}]}}}
        for name in ('models.json', 'models.yml'):
            save(agent / name, model)
        save(agent / 'settings.json', {'sessionDir': str(agent / 'sessions')})
        (agent / 'config.yml').write_text('autoResume: false\nstartup:\n  setupWizard: false\n  showSplash: false\n'
            '  checkUpdate: false\nmarketplace:\n  autoUpdate: off\nterminal:\n  programStatus: true\n', encoding='utf-8')

    def deploy_toolkit(self):
        toolkit = self.root / 'toolkit'
        toolkit.mkdir()
        for name in ('adapters', 'framework', 'extensions', 'entrypoints'):
            shutil.copytree(self.args.toolkit / name, toolkit / name,
                ignore=shutil.ignore_patterns('.git', '.runtime', 'node_modules'))
        shutil.copy2(self.args.toolkit / 'package.json', toolkit / 'package.json')
        for dependency in json.loads((toolkit / 'package.json').read_text(encoding='utf-8')).get('dependencies', {}):
            shutil.copytree(self.args.toolkit / 'node_modules' / dependency, toolkit / 'node_modules' / dependency)
        return toolkit

    def prepare(self):
        require(sys.platform == 'win32', self.windows_requirement)
        self.configure_provider()
        toolkit = self.deploy_toolkit()
        q = lambda value: json.dumps(str(value))
        catalog = f'skills = []\n[[agents]]\nid = {q(self.agent)}\nlabel = {q(self.agent)}\n'
        catalog += '[[models]]\nid = "default"\nlabel = "Default"\n[[models]]\nid = "probe"\nlabel = "Local fixture"\n'
        catalog += f'provider_model = "owned/mechanical"\nthinking = {{ {self.agent} = ["off"] }}\n'
        extensions = {role: toolkit / f'entrypoints/{self.agent}-auto/{role}.ts'
                      for role in ('manager-chat', 'node-chat', 'connect-bot')}
        if self.host == 'tui':
            extensions['herdr-agent-state'] = getattr(self.args, self.agent + '_state_extension')
        if self.case not in CLOSE_CASES:
            self.confirmations = self.root / 'confirmations'
            self.env['CHIEF_MAP_CONFIRMATION_EVIDENCE_DIR'] = str(self.confirmations)
            observer = self.root / 'native-dialog-observer.mjs'
            shutil.copy2(self.args.repo / 'scripts/chief-ui-observer.mjs', observer)
            extensions['acceptance-dialog-observer'] = observer
        for role, path in extensions.items():
            require(path.is_file(), ('fixed extension missing', role, path))
            catalog += f'[[extensions]]\nid = {q(self.agent + "-" + role)}\nlabel = {q(role)}\nagent = {q(self.agent)}\nkind = "fixed"\nentrypoint = {q(path)}\n'
        (self.root / 'options.toml').write_text(catalog, encoding='utf-8')
        self.evidence['extensions'] = {key: {'path': str(path), 'sha256': digest(path)} for key, path in extensions.items()}
        self.prepare_host()
        self.start_backend_owned(toolkit)
        self.http('/api/zones', {'id': 'hb15/zone', 'path': str(self.root / 'zone')})

    def prepare_host(self):
        enabled = self.case in ('custom', 'default')
        tui_sound = f'[ui.sound]\nenabled = {str(enabled).lower()}\n'
        app_sound = {'enabled': enabled}
        if self.case not in CLOSE_CASES:
            require(self.args.sound, 'isolated custom and bundled sound references are required')
            refs = self.root / 'sound-references'
            refs.mkdir()
            self.sound_references = {}
            self.evidence['soundResources'] = {}
            for kind in ('done', 'request'):
                for role, key in [('custom', kind), ('bundled', f'{self.host}_{kind}')]:
                    source = self.args.sound[key]
                    name = f'{role}-{kind}'
                    path = refs / (name + '.mp3')
                    shutil.copy2(source, path)
                    self.sound_references[name] = path
                    self.evidence['soundResources'][name] = {'source': str(source), 'archive': str(path.relative_to(self.root)), 'sha256': digest(path)}
                    if role == 'custom' and self.case != 'default':
                        tui_sound += f'{kind}_path = {json.dumps(str(path))}\n'
                        app_sound['blocked' if kind == 'request' else kind] = str(path)
        if self.host == 'tui':
            config = self.root / 'herdr.toml'
            config.write_text('onboarding = false\n[terminal]\ndefault_shell = "cmd.exe"\n'
                '[update]\nversion_check = false\nmanifest_check = false\n[server]\nheadless_cols = 160\nheadless_rows = 60\n'
                + tui_sound, encoding='utf-8')
            self.env['HERDR_CONFIG_PATH'] = str(config)
            self.start([self.args.herdr, 'server'], 'tui-server')
            self.herdr_started = True
            until('owned Herdr TUI endpoint', lambda: self.native_command(self.args.herdr, 'workspace', 'list', check=False)['exit'] == 0)
            if self.case == 'group-refused':
                for git_args in [('init', '-b', 'main'), ('-c', 'user.name=HB15', '-c', 'user.email=hb15@example.invalid',
                        'commit', '--allow-empty', '-m', 'owned worktree fixture')]:
                    self.command(['git', '-C', self.root / 'zone', *git_args])
            primary = self.herdr('workspace', 'create', '--cwd', str(self.root / 'zone'), '--label', 'zone', '--no-focus')
            self.space, self.sentinel = primary['workspace']['workspace_id'], primary['tab']['tab_id']
            if self.case == 'group-refused':
                self.evidence['groupCreation'] = self.herdr('worktree', 'create', '--workspace', self.space,
                    '--branch', 'hb15-control', '--path', str(self.root / 'group-control'), '--no-focus', '--trust-repository')
            else:
                self.herdr('workspace', 'create', '--cwd', str(self.root), '--label', 'outside', '--no-focus')
        else:
            config = self.root / 'a/settings.json'
            save(config, {'telemetry': False, 'check_updates': False, 'sounds': app_sound})
            status = self.core('server', 'start')
            self.cores = True
            self.core_identity = status['core']['id']
            self.observe_core(status)
            self.window_process = self.start([self.args.app], 'app')
            until('owned Herdr App endpoint', lambda: self.app('ls', check=False)['exit'] == 0)
            primary = self.app_do('space.new', name='zone', folder=str(self.root / 'zone'), focus=False)
            self.space, self.sentinel = primary['space'], primary['tab']
            self.app_do('space.new', name='outside', folder=str(self.root), focus=False)
        self.evidence['hostConfiguration'] = {'sha256': digest(config), 'path': str(config), 'enabled': enabled}
        self.evidence['primary'] = primary

    def start_backend_owned(self, toolkit):
        for attempt in range(32):
            port = 49152 + secrets.randbelow(16384)
            try:
                with socket.socket() as reservation:
                    reservation.bind(('127.0.0.1', port))
                break
            except OSError as error:
                if error.errno != errno.EADDRINUSE or attempt == 31:
                    raise
        base = self.base = f'http://127.0.0.1:{port}'
        self.wire = WireProxy(base, self.root / 'public-wire')
        self.stack.callback(self.wire.close)
        for role in ('connect-bot', 'workflow-chat'):
            config = toolkit / f'extensions/{role}/config.yaml'
            configuration = re.sub(r'^endpoint:.*$', lambda _: 'endpoint: ' + self.wire.base,
                config.read_text(encoding='utf-8'), flags=re.M)
            require(self.wire.base in configuration, ('extension endpoint not configured', role))
            config.write_text(configuration, encoding='utf-8')
        q = lambda value: json.dumps(str(value))
        file = self.root / 'backend.toml'
        unused = self.root / 'disabled-operation.bat'
        unused.write_text('@exit /b 1\n', encoding='ascii')
        operations = ''.join(f'[operations.{name}]\nscript = {q(unused)}\nworking_directory = {q(self.root)}\n'
            for name in ('start', 'stop'))
        file.write_text(f'listen_addr = "127.0.0.1:{port}"\nstatic_directory = {q(self.args.repo / "backend/static")}\n'
            f'data_directory = {q(self.root / "data")}\n[mdns]\nenabled = false\n'
            + operations + '[herdr]\nworkspace_name = "zone"\n' + f'backend = "{self.host}"\n'
            f'executable = {q(self.launch_recorder("tui", self.args.herdr))}\n'
            f'app_executable = {q(self.launch_recorder("app", self.args.app))}\n'
            f'core_executable = {q(self.launch_recorder("core", self.args.core))}\n'
            f'[tasks]\nstorage_file = {q(self.root / "data/tasks.json")}\nmetadata_file = {q(self.root / "options.toml")}\n'
            f'[sessions]\nhistory_file = {q(self.root / "data/history.json")}\n'
            f'[zones]\nregistry_file = {q(self.root / "data/zones.json")}\n'
            f'[scheduler]\nmap_directory = {q(self.root / "maps")}\nbot_directory = {q(self.root / "bots")}\n'
            f'run_directory = {q(self.root / "data/runs")}\nrun_instructions = {q(self.args.repo / "docs/workflow-runtime.md")}\n', encoding='utf-8')
        self.backend_process = self.start([self.args.backend, '--config', file], 'backend')
        def ready():
            require(self.backend_process.child.poll() is None, 'backend startup failed; see stderr')
            while not self.backend_process.lines.empty():
                if base + '/' in self.backend_process.lines.get_nowait():
                    return True
            return False
        until('owned public backend listener', ready)

    def task(self, name, kind):
        result = self.http('/api/tasks', {'name': name, 'taskType': kind, 'agent': self.agent,
            'chatName': '{chat.id}', 'model': 'probe', 'thinking': 'default', 'skill': None, 'prompt': 'Reply without tools.'})
        return next(row['id'] for row in result['tasks'] if row['name'] == name)

    def dispatch(self, *, auto):
        manager_task, node_task = self.task('owned-manager', 'manager'), self.task('owned-node', 'normal')
        save(self.root / 'bots/work.json', {'id': 'work', 'name': 'Work', 'when': '', 'plan_guide': '',
            'node_guide': '', 'task_id': node_task})
        save(self.root / 'maps/case.json', {'title': 'Paired observation', 'description': '', 'nodes': {
            'work': {'title': 'Owned target', 'bot_id': 'work', 'input': 'Respond with the local acknowledgement.',
                'zone': 'hb15/zone', 'blocked_by': [], 'output_rule': [], 'auto_close': auto, 'auto_approved': auto}}})
        started = self.http('/api/scheduler/maps/case.json/runs', {'managerTaskId': manager_task, 'zone': 'hb15/zone',
            'userInput': 'Acknowledge; no autonomous operations.'}, 201)
        manager = started['run']['manager']
        state = self.http('/api/workflow/manager/prepare_inputs', {'sessionId': manager['sessionId'], 'fileInputs': [],
            'confirmed': True, 'reason': 'Fixture has no file inputs'})
        require(state['snapshot']['inputsReady'] and not state.get('error') and not state.get('historyError'), state)
        state = self.http('/api/workflow/manager/dispatch', {'sessionId': manager['sessionId'], 'nodes': ['work']})
        node = state['snapshot']['nodes']['work']
        require(node['status'] == 'running' and node['execution']['autoClose'] is auto, state)
        target = node['execution']['chat']
        require(target['workspaceId'] == self.space and target['tabId'] not in (manager['tabId'], self.sentinel), state)
        self.evidence['association'] = {'run': started['id'], 'execution': node['execution']['id'], 'target': target,
            'manager': manager}
        for chat in (manager, target):
            identity = chat['sessionId']
            def ready():
                return next((event for event in self.accepted_events(identity) if event['kind'] == 'launch_ready'), None)
            ready_event = until('owned native SDK ready process', ready, self.args.timeout)
            pid = ready_event['data']['pid']
            handle = self.remember(pid)
            require(handle.alive(), ('native SDK ready process already exited', ready_event))
            self.agent_processes[chat['tabId']] = handle
            self.evidence.setdefault('ownedAgentProcesses', {})[chat['tabId']] = ready_event
        return started['id'], manager, target

    def remember(self, pid):
        if pid not in self.handles:
            self.handles[pid] = LiveProcess(pid)
        return self.handles[pid]

    def observe_core(self, status):
        require(status['core']['id'] == self.core_identity, status)
        for pid in [status['pid'], *(holder['pid'] for holder in status['holders'])]:
            self.remember(pid)

    def layout(self):
        if self.host == 'app':
            return {s['id']: {t['id']: t['terminals'] for t in s['tabs']} for s in self.app('ls')['spaces']}
        return {w['workspace_id']: {t['tab_id']: None for t in self.herdr('tab', 'list', '--workspace', w['workspace_id'])['tabs']}
            for w in self.herdr('workspace', 'list')['workspaces']}

    def members(self, space, tab, layout, pane=None):
        agent = self.agent_processes.get(tab)
        known = [agent] if agent is not None else []
        if self.host == 'app':
            entries = layout[space][tab]
            if pane:
                entries = [entry for entry in entries if entry['pane'] == pane]
            self.observe_core(self.core('server', 'status'))
            records = [self.core('get', entry['terminal']) for entry in entries]
            require(records and all(record['labels']['herdr.space'] == space
                and record['labels']['herdr.tab'] == tab for record in records), records)
            terminal_handles = [self.remember(record['pid']) for record in records if record.get('pid')]
            return records, list(set(known + terminal_handles))
        entries = [p['pane_id'] for p in self.herdr('pane', 'list', '--workspace', space)['panes']
            if p['tab_id'] == tab] if pane is None else [pane]
        records = [self.herdr('pane', 'process-info', '--pane', member)['process_info'] for member in entries]
        require([record['pane_id'] for record in records] == entries, ('TUI pane/process mismatch', entries, records))
        terminal_pids = {pid for record in records for pid in
            [record.get('shell_pid'), *(p['pid'] for p in record.get('foreground_processes', []))] if pid}
        return records, list(set(known + [self.remember(pid) for pid in terminal_pids]))

    def discard_initial_tab(self, tab):
        if self.host == 'app':
            require(self.app_do('tab.close', id=tab)['ok'] is True, 'could not remove setup tab')
        else:
            self.herdr('tab', 'close', tab)

    def receipts(self, *, finished=True):
        pattern = '*.json' if finished else '*.request'
        return [(name, json.loads(path.read_text(encoding='utf-8'))) for name, root in self.recorders.items()
            for path in sorted(root.glob(pattern)) if path.name != 'installed.json']

    def history(self, chat, marker):
        def ready():
            for path in (self.root / 'agent').rglob('*.jsonl'):
                try:
                    rows = [json.loads(line) for line in path.read_text(encoding='utf-8').splitlines()]
                except (OSError, ValueError):
                    continue
                if not any(row.get('type') == 'session' and row.get('id') == chat['sessionId'] for row in rows):
                    continue
                text = json.dumps(rows, ensure_ascii=False)
                if marker in text:
                    self.evidence.setdefault('nativeHistories', {})[chat['sessionId']] = {'path': str(path), 'sha256': digest(path)}
                    return rows
            return None
        return until('persisted installed-agent history ' + marker, ready, self.args.timeout)

    def exercise_close(self):
        run, manager, chat = self.dispatch(auto=True)
        self.history(chat, 'HB15-ACK')
        space, tab, pane = chat['workspaceId'], chat['tabId'], chat['paneId']
        approved = self.http('/api/workflow/node/approve', {'sessionId': chat['sessionId'],
            'executionId': self.evidence['association']['execution'], 'source': 'auto'})
        require(approved['approved'] is True, approved)
        report = self.http('/api/workflow/node/report', {'sessionId': chat['sessionId'], 'body': 'Owned completion candidate', 'canComplete': True})
        require(report['saved'] is True, report)
        if self.case != 'extra-tab':
            for setup_tab in (manager['tabId'], self.sentinel):
                _, handles = self.members(space, setup_tab, self.layout())
                self.discard_initial_tab(setup_tab)
                require(handles, ('setup tab had no observable process', setup_tab))
                until('retired setup tab process exit', lambda: all(not handle.alive() for handle in handles))
        current = self.layout()
        before = {key: set(tabs) for key, tabs in current.items()}
        target_records, target_handles = self.members(space, tab, current, pane)
        other_records, other_handles = [], []
        for key, tabs in before.items():
            for label in sorted(tabs - {tab}):
                records, handles = self.members(key, label, current)
                require(handles and all(handle.alive() for handle in handles), ('no live control process', records))
                other_records.extend(records)
                other_handles.extend(handles)
        require(target_handles and all(handle.alive() for handle in target_handles) and other_handles,
            ('missing target or unrelated OS process', target_records, other_records))
        require(not (set(target_handles) & set(other_handles)), 'target and control process overlap')
        require(len(before[space]) == (3 if self.case == 'extra-tab' else 1), before)
        self.evidence['beforeClose'] = {'layout': current, 'target': target_records, 'controls': other_records}
        if self.case == 'group-refused':
            workspaces = self.herdr('workspace', 'list')['workspaces']
            parent = next(row for row in workspaces if row['workspace_id'] == space)
            repo_key = parent['worktree']['repo_key']
            require(not parent['worktree']['is_linked_worktree'] and any(row['workspace_id'] != space
                and (row.get('worktree') or {}).get('repo_key') == repo_key
                and row['worktree']['is_linked_worktree'] for row in workspaces), workspaces)
            self.evidence['beforeClose']['worktreeGroup'] = workspaces
        if self.host == 'app' and self.case == 'last-tab':
            # Establish the installed host's unconfirmed refusal without performing a close.
            # The public completion below must enter the confirmed Tab path instead.
            no_confirm = self.app('do', 'tab.close', json.dumps({'id': tab}), check=False)
            require(no_confirm['exit'] != 0 and 'confirmed' in (no_confirm['stdout'] + no_confirm['stderr'])
                and self.layout() == current and all(handle.alive() for handle in target_handles), no_confirm)
        completed = self.http('/api/workflow/manager/complete', {'sessionId': manager['sessionId'], 'nodeId': 'work',
            'reason': 'Owned result approved explicitly through public HTTP'})
        require(completed['snapshot']['nodes']['work']['status'] == 'completed', completed)
        self.note('public-node-completed', tab=tab)
        refused = self.case == 'group-refused'
        def receipt():
            for host, record in self.receipts():
                argv = record['argv']
                if host == self.host and argv[:2] == (['tab', 'close'] if host == 'tui' else ['do', 'tab.close']):
                    return record
        if refused:
            def failure():
                detail = self.http('/api/workflow/runs/' + run)['run']['nodes']['work']
                return detail if detail.get('autoCloseFailure') else None
            detail = until('close refusal recorded without rolling back completion', failure)
            require(detail['status'] == 'completed' and detail['autoCloseFailure']['code'] == 'close-scope-denied', detail)
            calls = self.receipts(finished=False)
            require(not any(argv[:2] in (['tab', 'close'], ['workspace', 'close']) for _, argv in calls), calls)
            self.evidence['closeRefusal'] = detail['autoCloseFailure']
        else:
            native_receipt = until('native close command returned', receipt)
            self.evidence['closeReceipt'] = native_receipt
            argv = native_receipt['argv']
            require(argv[2] == tab if self.host == 'tui' else json.loads(argv[2]) == {'id': tab, 'confirmed': True}, native_receipt)
            require(native_receipt['exit'] == 0, native_receipt)
            wire_json = json.loads(native_receipt['stdout'])
            require((wire_json['result'].get('type') == 'ok' if self.host == 'tui' else wire_json.get('ok') is True), native_receipt)
            until('target Tab removed from native layout', lambda:
                tab not in self.layout().get(space, {}))
            until('owned target process exited independently', lambda: all(not handle.alive() for handle in target_handles))
        after = self.layout()
        self.evidence['afterClose'] = {'layout': after, 'targetProcesses': {h.pid: h.alive() for h in target_handles},
            'controlProcesses': {h.pid: h.alive() for h in other_handles}}
        effects = [(host, argv) for host, argv in self.receipts(finished=False) if argv[:2] in
            (['tab', 'close'], ['workspace', 'close'], ['do', 'tab.close'], ['do', 'space.close'],
             ['do', 'tab.settle'], ['do', 'terminal.park']) or argv[:1] == ['kill']]
        require(effects == ([] if refused else [(self.host, native_receipt['argv'])]), ('unexpected close, settle or park requests', effects))
        check_close(before, {key: set(tabs) for key, tabs in after.items()}, space, tab,
            all(not handle.alive() for handle in target_handles), all(handle.alive() for handle in other_handles), refused)

    def target_status(self, chat):
        if self.host == 'tui':
            pane = self.herdr('pane', 'get', chat['paneId'])['pane']
            agent = self.herdr('agent', 'get', chat['paneId'])
            agent = agent.get('agent', agent)
            require((pane['workspace_id'], pane['tab_id'], pane['pane_id']) ==
                (chat['workspaceId'], chat['tabId'], chat['paneId']) and agent['pane_id'] == chat['paneId'], (pane, agent))
            return agent['agent_status'], {'pane': pane, 'agent': agent}
        entries = self.layout()[chat['workspaceId']][chat['tabId']]
        members = [entry for entry in entries if entry['pane'] == chat['paneId']]
        require(len(members) == 1 and not members[0].get('settled'), members)
        entry = members[0]
        record = self.core('get', entry['terminal'])
        require(record['address'] == self.terminal and entry['terminal'] == self.terminal and
            record['labels']['herdr.space'] == chat['workspaceId'] and record['labels']['herdr.tab'] == chat['tabId'], record)
        native = record.get('status') or {}
        displayed = entry['status']
        if native.get('app') != self.agent or (displayed in ('working', 'blocked') and native.get('state') != displayed):
            displayed = None
        return displayed, {'appPane': entry, 'core': record}

    def await_status(self, chat, expected):
        def probe():
            value, record = self.target_status(chat)
            observation = self.note('native-status', state=value, observation=record)
            return observation if value in expected else None
        return until('native state ' + '/'.join(expected), probe, self.args.timeout)

    def focused_tab(self):
        if self.host == 'app':
            status = self.app('status')
            return status['tab'], status
        workspaces = self.herdr('workspace', 'list')['workspaces']
        # tab focus is independently visible in the public session snapshot.
        snapshot = self.herdr('api', 'snapshot')
        tabs = self.herdr('tab', 'list', '--workspace', self.space)['tabs']
        active = [row['tab_id'] for row in tabs if row.get('focused') or row.get('active') or row.get('is_active')]
        if len(active) != 1:
            # Some versions publish one focused Pane instead of a focused Tab.
            panes = self.herdr('pane', 'list', '--workspace', self.space)['panes']
            active = list({row['tab_id'] for row in panes if row.get('focused')})
        require(len(active) == 1, ('no independently focused native Tab', workspaces, snapshot, tabs))
        return active[0], snapshot

    def focus(self, tab):
        if self.host == 'app':
            self.app_do('tab.select', id=tab)
            window = foreground_window(pid=self.window_process.child.pid)
        else:
            self.herdr('tab', 'focus', tab)
            require(self.console_client.poll() is None, 'owned TUI client exited before refocus')
            window = foreground_window(anchor=self.tui_window)
        active, status = self.focused_tab()
        require(active == tab, ('focus affected another tab', tab, active, status))
        self.note('native-focus', tab=active, desktopWindow=window, status=status)

    def begin_round(self, chat, blocked):
        label = uuid.uuid4().hex[:12]
        enter, release = threading.Event(), threading.Event()
        self.gates[label] = (enter, release, blocked)
        text = 'Explicit local native reminder observation. HB15-' + label
        self.note('native-input', marker=label, text=text, target=chat)
        if self.host == 'app':
            self.native_command(self.args.core, 'send', '--json', self.terminal, '--', text)
        else:
            self.herdr('agent', 'prompt', chat['paneId'], text)
        require(enter.wait(self.args.timeout), ('provider never entered from owned native input', label))
        return label, release

    def dialog(self, chat):
        def screen():
            if self.host == 'tui':
                return self.command([self.args.herdr, 'pane', 'read', chat['paneId'], '--source', 'visible', '--format', 'text'])
            return self.command([self.args.core, 'read', self.terminal])
        def visible():
            for path in self.confirmations.glob('*.request.json'):
                request = json.loads(path.read_text(encoding='utf-8'))
                if request['sessionId'] == chat['sessionId'] and request['message'] == APPROVAL_QUESTION:
                    view = screen()
                    if request['title'] in view and all(word in view for word in (APPROVAL_QUESTION, 'Yes', 'No')):
                        return request, view
        request, view = until('owned native approval confirmation is visibly pending', visible, self.args.timeout)
        self.note('native-dialog', request=request, screen=view)
        self.remember(request['pid'])
        return request, screen

    def decline(self, chat, request, screen):
        def keys(key):
            if self.host == 'app':
                self.core('keys', self.terminal, key)
            else:
                self.herdr('agent', 'send-keys', chat['paneId'], key)
        def selected():
            view = screen()
            return view if any(arrow + ' No' in view for arrow in ('→', '❯')) else None
        if not selected():
            keys('down')
        self.note('native-no-selected', screen=until('visibly selected No', selected, seconds=5))
        keys('enter')
        path = self.confirmations / (request['id'] + '.response.json')
        reply = until('actual NO result from native confirmation', lambda:
            json.loads(path.read_text(encoding='utf-8')) if path.is_file() else None, seconds=5)
        require(reply['sessionId'] == chat['sessionId'] and reply['result'] is False, reply)
        self.note('native-dialog-returned', response=reply)

    def accepted_events(self, session_id, start=0):
        for record in self.wire.records[start:]:
            if record['path'] == '/api/sessions/events' and record.get('status') == 200 and record.get('ended'):
                request = json.loads(record['request'])
                if request.get('sessionId') == session_id:
                    yield request

    def panel_states(self, start, chat):
        return [event['data']['value'] for event in self.accepted_events(chat['sessionId'], start)
            if event.get('kind') == 'state']

    def reminder_round(self, chat, phase, references):
        self.focus(chat['tabId'])
        marker, release = self.begin_round(chat, phase == 'blocked')
        self.await_status(chat, ('working',))
        self.focus(self.sentinel if phase == 'background-done' else chat['tabId'])
        wav = self.root / f'{phase}.wav'
        with Loopback(wav) as capture:
            self.note('audio-baseline', phase=phase)
            time.sleep(0.5)
            started = self.note('provider-release-request', phase=phase)['monotonic']
            release.set()
            if phase == 'blocked':
                request, screen = self.dialog(chat)
                self.await_status(chat, ('blocked',))
                # Sample the real pending state across multiple native polls;
                # completing/declining is intentionally outside this window.
                for _ in range(3):
                    time.sleep(0.7)
                    self.await_status(chat, ('blocked',))
            else:
                states = ('done',) if phase == 'background-done' else ('idle',)
                self.await_status(chat, states)
                time.sleep(2.2)
            tab, status = self.focused_tab()
            require(tab == (self.sentinel if phase == 'background-done' else chat['tabId']), status)
            self.note('phase-sampling-ended', phase=phase, activeTab=tab)
        audio = capture.describe()
        sound = 'request' if phase == 'blocked' else 'done'
        source = 'bundled' if self.case == 'default' else 'custom'
        expected = None if self.case == 'disabled' or phase == 'foreground-done' else f'{source}-{sound}'
        audio.update(expected=expected, matches=None)
        self.evidence.setdefault('audio', {})[phase] = audio
        if not capture.error:
            try:
                audio['matches'] = identify_audio(self.args.sound['ffmpeg'], wav, references, capture.origin)
            except Exception:
                audio['error'] = traceback.format_exc()
        if phase == 'blocked':
            next_audio = self.root / 'after-no.wav'
            with Loopback(next_audio) as after_no:
                self.decline(chat, request, screen)
                self.await_status(chat, ('idle',))
                time.sleep(2.2)
            self.evidence['afterNo'] = after_no.describe()
        self.history(chat, 'HB15-ACK-' + marker)
        observations = audio['matches']
        if observations is not None:
            # Early sound is not evidence for this transition. Keep its timestamp
            # even when a later sound also appears.
            require(all(sample['monotonic'] >= started - 0.12 for sample in observations), ('playback before provider release', observations))
        return expected, None if observations is None else [sample['sound'] for sample in observations]

    def exercise_reminders(self):
        require(self.args.sound, 'isolated custom and bundled sound references are required')
        if self.host == 'tui':
            self.console_title = 'HB15 ' + uuid.uuid4().hex
            self.console_client = launch_tui_client(self.args.herdr, self.env['HERDR_SESSION'], self.console_title, self.root, self.env)
            self.tui_window = foreground_window(title=self.console_title)
            self.note('owned-tui-window', desktopWindow=self.tui_window)
        run, manager, chat = self.dispatch(auto=False)
        self.history(manager, 'HB15-ACK')
        self.history(chat, 'HB15-ACK')
        records, handles = self.members(self.space, chat['tabId'], self.layout(), chat['paneId'])
        require(handles and all(handle.alive() for handle in handles), records)
        self.evidence['reminderTarget'] = records
        before = self.layout()
        if self.host == 'app':
            self.terminal = records[0]['address']
            self.start([self.args.core, 'events', '--terminal', self.terminal, '--json'], 'core-events')
        self.focus(chat['tabId'])
        self.await_status(chat, ('idle',))
        self.evidence['reminderControl'] = before[self.space][self.sentinel]
        checks = []
        start = len(self.wire.records)
        receipts_before = len(self.receipts(finished=False))
        for phase in ('background-done', 'foreground-done', 'blocked'):
            checks.append((phase, *self.reminder_round(chat, phase, self.sound_references)))
        values = until('public connect-bot accepted both round edges for owned Session', lambda:
            values if 'working' in (values := self.panel_states(start, chat)) and 'awaiting-input' in values else None)
        self.evidence['acceptedPanelStates'] = values
        calls = self.receipts(finished=False)[receipts_before:]
        self.evidence['backendCallsDuringReminders'] = calls
        require(not calls, ('bot issued an additional host/core command during native reminder transitions', calls))
        detail = self.http('/api/workflow/runs/' + run)['run']['nodes']['work']
        require(detail['status'] == 'running' and detail['execution']['approved'] is False, detail)
        control = self.layout()[self.space][self.sentinel]
        require(control == self.evidence['reminderControl'], ('unrelated control mutated', control))
        for phase, expected, observed in checks:
            self.note('playback-comparison', phase=phase, expected=expected, observed=observed)
        for phase, expected, observed in checks:
            check_sound(expected, observed)

    def cleanup(self):
        def operation(action):
            try:
                action()
            except Exception:
                self.evidence['cleanupErrors'].append(traceback.format_exc())
        for _, release, _ in self.gates.values():
            release.set()
        if hasattr(self, 'space'):
            def observe_remaining():
                layout = self.layout()
                for space, tabs in layout.items():
                    for tab in tabs:
                        self.members(space, tab, layout)
            operation(observe_remaining)
        if self.herdr_started:
            operation(lambda: self.command([self.args.herdr, 'server', 'stop']))
        if self.cores:
            operation(lambda: self.observe_core(self.core('server', 'status')))
            operation(lambda: self.command([self.args.core, 'server', 'stop']))
        if hasattr(self, 'console_client'):
            def observe_client():
                try:
                    exit_code = self.console_client.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    self.command(['taskkill', '/PID', str(self.console_client.pid), '/T', '/F'])
                    exit_code = self.console_client.wait(timeout=10)
                self.evidence['processExits'].append({'pid': self.console_client.pid, 'exit': exit_code,
                    'observation': 'owned interactive console handle'})
            operation(observe_client)
        for process in reversed(self.processes):
            operation(process.stop)
            self.evidence['processExits'].append({'pid': process.child.pid, 'exit': process.child.poll(), 'observation': 'owned child handle'})
        for pid, handle in self.handles.items():
            def observe():
                until(f'owned PID {pid} exited', lambda: not handle.alive(), seconds=10)
                self.evidence['processExits'].append({'pid': pid, 'exited': True, 'observation': 'held Windows process handle'})
            try:
                operation(observe)
            finally:
                handle.close()
        operation(self.stack.close)
