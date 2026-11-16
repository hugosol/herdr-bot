"""Exclusive environment, production HTTP and owned host lifecycle facilities."""
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import traceback
import urllib.error
import urllib.request
import uuid

from .processes import Process, require, until


class Acceptance:
    windows_requirement = 'Real Herdr acceptance requires Windows process handles'

    def __init__(self, args, *, prefix, promise, session_prefix):
        self.args = args
        requested = getattr(args, 'evidence_dir', None)
        if requested is None:
            self.root = Path(tempfile.mkdtemp(prefix=prefix))
        else:
            self.root = requested.resolve()
            self.root.mkdir(parents=True, exist_ok=False)
        self.evidence = {'promise': promise, 'passed': False, 'root': str(self.root),
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
                        HERDR_SESSION=session_prefix + uuid.uuid4().hex)
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
        require(sys.platform == 'win32', self.windows_requirement)
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
        package = json.loads((toolkit / 'package.json').read_text(encoding='utf-8'))
        for dependency in package.get('dependencies', {}):
            source = self.args.toolkit / 'node_modules' / dependency
            require(source.is_dir(), ('Missing frozen Toolkit runtime dependency', str(source)))
            shutil.copytree(source, toolkit / 'node_modules' / dependency, dirs_exist_ok=True)
        role_config = toolkit / 'extensions/workflow-chat/config.yaml'
        role_config.write_text('endpoint: http://127.0.0.1:9\nrequestTimeoutMs: 30000\nreportConfirmationTimeoutMs: 20000\ncollectTimeoutMs: 5460000\n')
        self.configure_model()
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
        catalog = self.role_catalog(toolkit)
        (self.root / 'options.toml').write_text(catalog)
        self.start_backend(role_config)

    def start_backend(self, role_config):
        q = lambda value: json.dumps(str(value))
        config = 'listen_addr = "127.0.0.1:0"\n'
        config += f'static_directory = {q(self.args.repo / "backend/static")}\ndata_directory = {q(self.root / "data")}\n'
        config += '[mdns]\nenabled = false\n'
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
        backend = self.start([self.args.backend, '--config', self.root / 'backend.toml'], 'backend-' + str(len(self.processes)))
        def address():
            require(backend.child.poll() is None, 'Backend exited during startup; see raw logs')
            while not backend.lines.empty():
                match = re.search(r'http://(127\.0\.0\.1:\d+)/', backend.lines.get_nowait())
                if match:
                    return 'http://' + match[1]
        self.base = until('backend bound loopback address', address)
        role_config.write_text(f'endpoint: {self.base}\nrequestTimeoutMs: 30000\nreportConfirmationTimeoutMs: 20000\ncollectTimeoutMs: 5460000\n')

    def history(self, file):
        module = (self.args.pi_package / 'dist/core/session-manager.js').as_uri()
        code = f'import {{SessionManager}} from {json.dumps(module)}; const s = SessionManager.open({json.dumps(file)}); console.log(JSON.stringify({{id:s.getSessionId(),branch:s.getBranch()}}));'
        return json.loads(self.command([self.args.node, '--input-type=module', '-e', code]))

    def cleanup_extra(self):
        """Additional resources owned by a specialized real-host scenario."""

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
                try:
                    self.model.shutdown()
                    self.model.server_close()
                except BaseException:
                    self.evidence['cleanupErrors'].append(traceback.format_exc())
            try:
                self.cleanup_extra()
            except BaseException:
                self.evidence['cleanupErrors'].append(traceback.format_exc())
            self.evidence['passed'] = self.evidence['passed'] and not self.evidence['cleanupErrors']
            (self.root / 'evidence.json').write_text(json.dumps(self.evidence, ensure_ascii=False, indent=2), encoding='utf-8')
            print(json.dumps({'passed': self.evidence['passed'], 'evidence': str(self.root / 'evidence.json'),
                              'verificationError': error, 'cleanupErrors': self.evidence['cleanupErrors']}, ensure_ascii=False, indent=2))
        return 0 if self.evidence['passed'] else 1

