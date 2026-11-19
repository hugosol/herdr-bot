"""S1 HTTP + S3 installed hosts: selection, placement, and fixed script receipts.

No model or agent launch is authorized by this check. The reference outcomes
are the independently created native containers, not IDs returned by the bot.
"""
import argparse
import contextlib
import errno
import json
import os
from pathlib import Path
import re
import secrets
import socket
import subprocess
import sys
import urllib.request
import uuid

from acceptance.host.isolation import Acceptance
from acceptance.host.processes import LiveProcess, require, until

REPO = Path(__file__).resolve().parents[1]


class Selection(Acceptance):
    def __init__(self, args):
        super().__init__(args, prefix='hb-select-', promise='P1/P2/P3/P4/P5/P8/P9/P51',
                         session_prefix='hb-select-')
        self.env = {k: v for k, v in self.env.items() if not k.upper().startswith('HC_')}
        app_home = self.root / 'app'
        app_home.mkdir()
        (app_home / 'settings.json').write_text('{"telemetry":false,"check_updates":false}', encoding='utf-8')
        self.env.update(HERDR_APP_HOME=str(app_home), HERDR_CORE_DIRS=str(args.core.parent),
                        HC_SESSION='hbs-' + uuid.uuid4().hex[:12], PATH=os.path.join(os.environ['SystemRoot'], 'System32'))
        self.cores = []
        self.handles = {}

    def native(self, executable, *argv):
        return json.loads(self.command([executable, *argv]))

    def core_status(self, env=None):
        return self.in_env(env, lambda: self.native(self.args.core, 'server', 'status', '--json'))

    def in_env(self, env, action):
        old = self.env
        try:
            if env is not None:
                self.env = env
            return action()
        finally:
            self.env = old

    def start_core(self, env):
        status = self.in_env(env, lambda: self.native(self.args.core, 'server', 'start', '--json'))
        self.cores.append(env)
        self.record_core(status)
        return status['core']['id']

    def record_core(self, status):
        for pid in [status['pid'], *(holder['pid'] for holder in status['holders'])]:
            if pid not in self.handles:
                self.handles[pid] = LiveProcess(pid)

    def probe(self, executable, *args, env=None):
        argv = [str(executable), *args]
        result = subprocess.run(argv, cwd=self.root, env=env or self.env,
                                capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=15)
        self.evidence['commands'].append({'argv': argv, 'exit': result.returncode,
                                          'stdout': result.stdout, 'stderr': result.stderr})
        return result.returncode == 0

    def wait_for_endpoint(self, executable, *argv):
        until(f'{executable.name} endpoint', lambda: self.probe(executable, *argv), seconds=15)

    def prepare(self):
        require(sys.platform == 'win32', self.windows_requirement)
        for name in ['backend', 'herdr', 'app', 'core']:
            path = getattr(self.args, name)
            require(path.is_absolute() and path.is_file(), ('explicit installed binary required', name, path))
        self.evidence['versions'] = {name: self.command([getattr(self.args, name), '--version']).strip()
                                     for name in ['herdr', 'app', 'core']}
        (self.root / 'herdr.toml').write_text('onboarding = false\n[terminal]\ndefault_shell = "cmd.exe"\n'
                                            '[update]\nversion_check = false\nmanifest_check = false\n')
        self.env['HERDR_CONFIG_PATH'] = str(self.root / 'herdr.toml')
        self.start([self.args.herdr, 'server'], 'tui')
        self.herdr_started = True
        self.wait_for_endpoint(self.args.herdr, 'workspace', 'list')
        self.evidence['selectedCore'] = self.start_core(self.env)
        self.start([self.args.app], 'app')
        self.wait_for_endpoint(self.args.app, 'ls')
        self.evidence['instanceSelectors'] = {key: self.env[key] for key in ['HERDR_SESSION', 'HERDR_APP_HOME', 'HC_SESSION']}
        # Use the real production catalog parser but no installed agent extensions.
        # The fixture paths satisfy launch preflight, never supply host responses.
        catalog = (REPO / 'meta-data/task-options.toml').read_text(encoding='utf-8')
        entry = self.root / 'unused.ts'
        entry.write_text('export default function () {}\n', encoding='utf-8')
        (self.root / 'options.toml').write_text(re.sub(r'^entrypoint\s*=.*$', lambda _: f'entrypoint = {json.dumps(str(entry))}',
                                                    catalog, flags=re.M), encoding='utf-8')

    def tui_membership(self):
        entries = self.herdr('workspace', 'list')['workspaces']
        return [(w['workspace_id'], w['label'], w['tab_count'], w['pane_count']) for w in entries]

    def app_membership(self):
        spaces = self.native(self.args.app, 'ls')['spaces']
        return [(s['id'], s['name'], [(t['id'], t['name'], [(p['pane'], p['terminal']) for p in t['terminals']])
                                     for t in s['tabs']]) for s in spaces]

    def snapshot(self):
        return self.tui_membership(), self.app_membership()

    def create_space(self, name):
        return self.native(self.args.app, 'do', 'space.new', json.dumps({'name': name, 'folder': str(self.root / 'zone'), 'focus': False}))

    @contextlib.contextmanager
    def backend(self, label, backend, name, env=None):
        directory = self.root / label
        directory.mkdir()
        q = lambda value: json.dumps(str(value))
        config = directory / 'backend.toml'
        for attempt in range(32):
            port = 49152 + secrets.randbelow(16384)
            try:
                with socket.socket() as reservation:
                    reservation.bind(('127.0.0.1', port))
                break
            except OSError as error:
                if error.errno != errno.EADDRINUSE or attempt == 31:
                    raise
        config.write_text(f'''listen_addr = '127.0.0.1:{port}'
static_directory = {q(REPO / 'backend/static')}
[mdns]
enabled = false
[tasks]
metadata_file = {q(self.root / 'options.toml')}
[zones]
registry_file = 'zones.json'
[herdr]
workspace_name = {q(name)}
executable = {q(self.args.herdr)}
app_executable = {q(self.args.app)}
core_executable = {q(self.args.core)}
''' + (f'backend = {q(backend)}\n' if backend else '') + ''.join(
            f'[operations{table}.{op}]\nscript = {q(self.root / f"{host}-{op}.bat")}\nworking_directory = {q(directory)}\n'
            for host, table in [('tui', ''), ('app', '.app')] for op in ['start', 'stop']), encoding='utf-8')
        process = self.in_env(env, lambda: self.start([self.args.backend, '--config', config], label))
        try:
            def started():
                while not process.lines.empty():
                    match = re.search(r'http://127\.0\.0\.1:\d+', process.lines.get_nowait())
                    if match:
                        return match[0]
                require(process.child.poll() is None, (label, 'unexpected backend exit'))
            if backend == 'invalid-host':
                require(process.child.wait(timeout=10) != 0, 'invalid backend was accepted')
                process.stop()
                stderr = (self.root / f'{label}-stderr.log').read_text(encoding='utf-8')
                require('invalid-host' in stderr and 'backend' in stderr, stderr)
            else:
                self.base = until('backend loopback listener', started, seconds=10)
            yield directory
        finally:
            process.stop()
            self.processes.remove(process)

    def launch(self):
        self.http('/api/zones', {'id': 'host-test', 'path': str(self.root / 'zone')})
        tasks = self.http('/api/tasks', {'name': 'placement-only', 'chatName': 'Owned {chat.id}', 'agent': 'omp',
                                         'model': 'default', 'thinking': 'default', 'skill': None, 'prompt': ''})
        task_id = tasks['tasks'][0]['id']
        preview = self.http('/api/chats/preview', {'zone': 'host-test', 'taskId': task_id})
        created = self.http('/api/chats', {'zone': 'host-test', 'taskId': task_id, 'chatId': preview['chatId'],
                                          'workspaceTarget': 'default', 'input': ''}, status=202)
        def observed():
            view = self.http('/api/chats/' + created['id'])
            return view if view.get('tabId') or view['stage'] == 'failed' else None
        return preview['name'], until('native placement or explicit rejection', observed, seconds=15)

    def exercise(self):
        tui = {}
        app = {}
        for name in ['bot-playground', 'ticket-alternate', 'BOT-PLAYGROUND']:
            tui[name] = self.herdr('workspace', 'create', '--label', name, '--cwd', str(self.root / 'zone'), '--no-focus')['workspace']['workspace_id']
            app[name] = self.create_space(name)
        self.record_core(self.core_status())
        # A fallback or case-insensitive match would change the wrong native collection.
        for label, backend, name in [('implicit', None, 'bot-playground'), ('explicit-tui', 'tui', 'bot-playground'),
                                     ('explicit-app', 'app', 'bot-playground'), ('alternate-tui', 'tui', 'ticket-alternate'),
                                     ('alternate-app', 'app', 'ticket-alternate')]:
            before_tui, before_app = self.snapshot()
            with self.backend(label, backend, name):
                title, view = self.launch()
                require(view.get('tabId'), view)
                if backend == 'app':
                    require(self.tui_membership() == before_tui, 'App routed to TUI')
                    space = next(s for s in self.native(self.args.app, 'ls')['spaces'] if s['id'] == app[name]['space'])
                    tab = next((t for t in space['tabs'] if t['id'] == view['tabId']), None)
                    require(tab and tab['name'] == title, (space, view))
                    term = next((t for t in tab['terminals'] if t['pane'] == view['paneId']), None)
                    require(term is not None, (tab, view))
                    core_term = self.native(self.args.core, 'get', term['terminal'], '--json')
                    require(core_term['labels']['herdr.space'] == app[name]['space'] and
                            core_term['labels']['herdr.tab'] == tab['id'], core_term)
                else:
                    require(self.app_membership() == before_app, 'TUI routed to App')
                    tabs = self.herdr('tab', 'list', '--workspace', tui[name])['tabs']
                    require(any(t['tab_id'] == view['tabId'] and t['label'] == title for t in tabs), (tabs, view))
        for backend in ['tui', 'app']:
            self.herdr('workspace', 'create', '--label', 'ambiguous', '--cwd', str(self.root / 'zone'), '--no-focus')
            self.create_space('ambiguous')
        for backend in ['tui', 'app']:
            for name, code in [('Ambiguous', 'workspace-not-found'), ('ambiguous', 'workspace-ambiguous')]:
                before = self.snapshot()
                with self.backend(f'{backend}-{code}', backend, name):
                    _, view = self.launch()
                    require(view['failure']['code'] == code and 'tabId' not in view, view)
                require(self.snapshot() == before, (backend, name, 'changed the native containers'))
        before = self.snapshot()
        with self.backend('invalid-backend', 'invalid-host', 'bot-playground'):
            pass
        require(self.snapshot() == before, 'rejected startup changed a native container')
        self.mismatched_core(app['bot-playground'])
        self.fixed_operations()
        self.evidence['passed'] = True

    def mismatched_core(self, selected):
        # A core-local address is deliberately identical to the real App terminal.
        # Matching on t1 alone would report success while operating on a foreign terminal.
        foreign = dict(self.env, HC_SESSION='hbs-' + uuid.uuid4().hex[:12])
        foreign_id = self.start_core(foreign)
        require(foreign_id != self.evidence['selectedCore'], 'need two independent core identities')
        terminal = self.in_env(foreign, lambda: self.native(self.args.core, 'run', '--bg', '--json',
                                                            '--label', 'herdr.space=foreign-space', '--label', 'herdr.tab=foreign-tab'))
        require(terminal['address'] == selected['terminal'], (terminal, selected))
        self.record_core(self.core_status(foreign))
        before = self.snapshot()
        def foreign_inventory():
            return self.in_env(foreign, lambda: self.native(self.args.core, 'ls', '--all', '--json'))
        foreign_before = foreign_inventory()
        with self.backend('foreign-core', 'app', 'bot-playground', foreign):
            _, view = self.launch()
            require(view['failure']['stage'] == 'tab' and 'App/core 实例不一致' in view['failure']['message']
                    and not view.get('tabId'), view)
        require(self.snapshot() == before and foreign_inventory() == foreign_before,
                'mismatch changed a native container or foreign terminal')

    def fixed_operations(self):
        for host in ['tui', 'app']:
            for operation in ['start', 'stop']:
                (self.root / f'{host}-{operation}.bat').write_text(
                    f'@echo off\n>>invoked.txt echo {host}-{operation}\nexit /b 0\n')
        inactive_home = self.root / 'inactive-app'
        inactive_home.mkdir()
        inactive = dict(self.env, HERDR_APP_HOME=str(inactive_home), HC_SESSION='hbs-' + uuid.uuid4().hex[:12],
                        HERDR_SESSION='hbs-' + uuid.uuid4().hex[:12])
        def running(backend, env):
            if backend == 'tui':
                return (self.probe(self.args.herdr, 'workspace', 'list', env=env),)
            return (self.probe(self.args.app, 'ls', env=env),
                    self.probe(self.args.core, 'server', 'status', '--json', env=env))
        for backend in ['tui', 'app']:
            for operation, env, expected_host in [('start', inactive, False), ('stop', self.env, True)]:
                expected = (expected_host,) * (1 if backend == 'tui' else 2)
                require(running(backend, env) == expected, (backend, operation, 'wrong starting state'))
                with self.backend(f'{backend}-{operation}', backend, 'bot-playground', env) as working_dir:
                    path = '/api/operations/' + operation
                    request = urllib.request.Request(self.base + path, method='POST')
                    with urllib.request.urlopen(request, timeout=15) as response:
                        receipt = json.load(response)
                        self.evidence.setdefault('http', []).append({'path': path, 'status': response.status, 'body': receipt})
                    require(receipt == {'operation': operation, 'started': True, 'ended': True,
                                        'exitCode': 0, 'outcome': 'completed'}, receipt)
                    require((working_dir / 'invoked.txt').read_text().splitlines() == [f'{backend}-{operation}'],
                            'wrong fixed script or working directory')
                require(running(backend, env) == expected, (backend, operation, 'script receipt changed host state'))

    def cleanup_extra(self):
        try:
            for env in self.cores:
                self.record_core(self.core_status(env))
                self.in_env(env, lambda: self.native(self.args.core, 'server', 'stop'))
            for handle in self.handles.values():
                until(f'owned core/holder {handle.pid} exited', lambda: not handle.alive(), seconds=10)
        finally:
            for handle in self.handles.values():
                handle.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['backend', 'herdr', 'app', 'core', 'evidence-dir']:
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    require(args.evidence_dir.is_absolute(), 'evidence directory must be an explicit absolute path')
    return Selection(args).run()


if __name__ == '__main__':
    sys.exit(main())
