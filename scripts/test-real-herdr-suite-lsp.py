"""P37/P38 at S2/S3/S4: installed OMP via real Herdr, never an SDK loader.

The loopback model requests real native LSP capabilities/status. No fake language
server, installation, PATH mutation, user credentials or user Chat is used.
Default all-language acceptance remains nonzero if any prerequisite is missing.
"""
import argparse
import ctypes
import hashlib
import http.server
import json
import os
from pathlib import Path
import re
import runpy
import shutil
import subprocess
import sys
import threading
import traceback

from developer_paths import REPO, backend_suffix

support = runpy.run_path(str(Path(__file__).with_name('test-real-herdr-close.py')))
require, until = support['require'], support['until']

# Independent literals from the approved four-language declarations.
SERVERS = {
    'rust-analyzer': {'command': 'rust-analyzer', 'args': [], 'fileTypes': ['.rs'],
                     'rootMarkers': ['Cargo.toml', 'rust-analyzer.toml']},
    'typescript-language-server': {'command': 'typescript-language-server', 'args': ['--stdio'],
        'fileTypes': ['.ts', '.tsx', '.js', '.jsx', '.mjs', '.cjs'],
        'rootMarkers': ['package.json', 'tsconfig.json', 'jsconfig.json']},
    'jdtls': {'command': 'jdtls', 'args': [], 'fileTypes': ['.java'],
        'rootMarkers': ['pom.xml', 'build.gradle', 'build.gradle.kts', 'settings.gradle', '.project']},
    'pyright': {'command': 'pyright-langserver', 'args': ['--stdio'], 'fileTypes': ['.py', '.pyi'],
        'rootMarkers': ['pyproject.toml', 'pyrightconfig.json', 'setup.py', 'setup.cfg', 'requirements.txt', 'Pipfile']},
}
PROJECTS = {
    'rust-analyzer': ('Cargo.toml', '[package]\nname="owned_lsp"\nversion="0.1.0"\nedition="2021"\n',
                      'src/main.rs', 'fn main() { let answer: i32 = 42; println!("{answer}"); }\n'),
    'typescript-language-server': ('tsconfig.json', '{"compilerOptions":{"strict":true},"include":["main.ts"]}',
                                    'main.ts', 'const answer: number = 42;\nconsole.log(answer);\n'),
    'jdtls': ('pom.xml', '<project xmlns="http://maven.apache.org/POM/4.0.0"><modelVersion>4.0.0</modelVersion><groupId>owned</groupId><artifactId>lsp</artifactId><version>1</version></project>',
              'src/main/java/Probe.java', 'public class Probe { public static void main(String[] args) { int answer = 42; System.out.println(answer); } }\n'),
    'pyright': ('pyproject.toml', '[project]\nname="owned-lsp"\nversion="0.1.0"\n',
                'main.py', 'answer: int = 42\nprint(answer)\n'),
}


class Model(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_POST(self):
        request = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        case = self.server.case
        case['requests'].append(request)
        delta, finish = {'role': 'assistant', 'content': 'LSP-ASSERTION-FAILED'}, 'stop'
        try:
            messages = request['messages']
            if not request.get('tools'):
                delta = {'role': 'assistant', 'content': '<title>Owned native LSP</title>'}
            else:
                tools = [item['function']['name'] for item in request['tools']]
                instructions = '\n'.join(str(message.get('content', '')) for message in messages
                                         if message['role'] in ['system', 'developer'])
                require('write' in tools and 'xd://lsp' in instructions, ('Native LSP device unavailable', tools))
                outputs = [str(message.get('content', '')) for message in messages if message['role'] == 'tool']
                if len(outputs) == 0:
                    # P37 requires actual native server startup, not a stable
                    # semantic hover while the project's first index is changing.
                    params = {'action': 'capabilities', 'file': case['file'], 'timeout': 90}
                elif len(outputs) == 1:
                    case['capabilities'] = outputs[-1]
                    if case['missing']:
                        require('No language server found for this file' in outputs[-1], ('Missing executable must not initialize', outputs[-1]))
                    else:
                        require(case['server'] + ':' in outputs[-1] and 'capabilities:' in outputs[-1]
                                and 'failed to start' not in outputs[-1], ('Real language server did not initialize', outputs[-1]))
                    params = {'action': 'status'}
                else:
                    case['status'] = outputs[-1]
                    if case['missing']:
                        require(case['server'] + ' (ready)' not in outputs[-1], outputs[-1])
                        require('No language servers configured for this project' in outputs[-1], outputs[-1])
                    else:
                        require(case['server'] + ' (ready)' in outputs[-1], ('Declaration is not a live service', outputs[-1]))
                    case['passed'] = True
                    params = None
                    delta = {'role': 'assistant', 'content': 'NATIVE-LSP-OBSERVED-519'}
                if params:
                    delta = {'role': 'assistant', 'tool_calls': [{'index': 0, 'id': 'lsp-' + str(len(outputs)),
                        'type': 'function', 'function': {'name': 'write', 'arguments': json.dumps({
                            'i': 'Observing native LSP', 'path': 'xd://lsp', 'content': json.dumps(params)})}}]}
                    finish = 'tool_calls'
        except BaseException:
            case.setdefault('error', traceback.format_exc())
        self.send_response(200)
        self.send_header('Content-Type', 'text/event-stream')
        self.end_headers()
        for part, reason in [(delta, None), ({}, finish)]:
            chunk = {'id': 'owned-lsp', 'object': 'chat.completion.chunk', 'created': 1, 'model': 'lsp-probe',
                     'choices': [{'index': 0, 'delta': part, 'finish_reason': reason}]}
            self.wfile.write(('data: ' + json.dumps(chunk) + '\n\n').encode())
        self.wfile.write(b'data: [DONE]\n\n')


class Acceptance(support['Acceptance']):
    def __init__(self, args):
        super().__init__(args, prefix='herdr-suite-lsp-')
        self.env = {key: value for key, value in self.env.items() if not any(
            term in key.upper() for term in ['SECRET', 'PASSWORD', 'PROXY', 'CREDENTIAL'])}
        self.evidence.update(promise=['P37', 'P38'], cases=[], blockers=[], language=args.language)
        self.handles = {}

    def prepare(self):
        require(sys.platform == 'win32', 'Real Herdr acceptance requires Windows process handles')
        for path in [self.args.herdr, self.args.backend, self.args.omp_package / 'dist/cli.js']:
            require(path.is_file(), ('Missing real dependency', str(path)))
        launcher = shutil.which('omp')
        require(launcher, 'Installed omp must already be on PATH; runner never changes PATH')
        launcher = Path(launcher)
        require('@oh-my-pi/pi-coding-agent/dist/cli.js' in launcher.read_text().replace('\\', '/'),
                'PATH omp must launch installed dist CLI, not source CLI or a replacement')
        self.evidence['source'] = {'repo': str(REPO), 'revision': self.command(['git', '-C', REPO, 'rev-parse', 'HEAD']),
            'diff': self.command(['git', '-C', REPO, 'diff', '--', 'backend', 'scripts', 'frontend', 'docs'])}
        files = [self.args.backend, self.args.herdr, launcher, self.args.omp_package / 'dist/cli.js',
                 Path(__file__), REPO / 'backend/src/suites/skills.rs', REPO / 'backend/Cargo.lock']
        self.evidence['sha256'] = {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in files}
        self.evidence['ompVersion'] = json.loads((self.args.omp_package / 'package.json').read_text())['version']
        self.evidence['executables'] = {name: shutil.which(config['command']) for name, config in SERVERS.items()}
        if self.args.rust_toolchain:
            require((self.args.rust_toolchain / 'bin/rust-analyzer.exe').is_file(), 'Missing preinstalled Rust toolchain component')
            # Pin an already-installed toolchain without exposing user Cargo
            # credentials or changing PATH. Rustup otherwise sees the owned HOME.
            self.env['RUSTUP_TOOLCHAIN'] = str(self.args.rust_toolchain)
            self.env['CARGO_HOME'] = str(self.root / 'cargo')
            self.evidence['rustToolchain'] = str(self.args.rust_toolchain)
        self.model = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Model)
        threading.Thread(target=self.model.serve_forever, daemon=True).start()
        agent = self.root / 'agent'
        (agent / 'config.yml').write_text('autoResume: false\nstartup:\n  setupWizard: false\n  showSplash: false\n  checkUpdate: false\nmarketplace:\n  autoUpdate: off\n')
        (agent / 'models.yml').write_text(json.dumps({'providers': {'lsp-local': {
            'baseUrl': f'http://127.0.0.1:{self.model.server_port}/v1', 'api': 'openai-completions',
            'apiKey': 'owned-loopback-not-a-credential', 'models': [{'id': 'lsp-probe', 'name': 'LSP probe',
            'reasoning': False, 'input': ['text'], 'contextWindow': 32768, 'maxTokens': 2048,
            'cost': {'input': 0, 'output': 0, 'cacheRead': 0, 'cacheWrite': 0}}]}}}))
        # Explicitly owned user config. A project override in the positive Rust
        # case must win over this missing executable; no user's config is read.
        self.missing = str(self.root / 'never-installed-language-server.exe')
        self.user_lsp = self.root / 'home/.omp/agent/lsp.json'
        self.user_lsp.parent.mkdir(parents=True)
        self.user_lsp.write_text(json.dumps({'servers': {'rust-analyzer': {'command': self.missing}}}))
        authority = self.root / 'declarations with spaces'
        authority.mkdir()
        (authority / 'suites.json').write_text(json.dumps({'suites': [{'id': 'four-languages', 'name': 'Four languages',
            'agents': ['omp'], 'lsp': SERVERS}]}))
        (self.root / 'paths.toml').write_text('[roots]\n')
        observer = self.root / 'owned-process-observer.ts'
        observer.write_text('export default function(pi) { pi.on("session_start", async () => { '
            'await Bun.write(".owned-omp-process.json", JSON.stringify({pid:process.pid,cwd:process.cwd()})); }); }')
        (self.root / 'options.toml').write_text('skills = []\n[[agents]]\nid = "omp"\nlabel = "OMP"\n'
            '[[models]]\nid = "default"\nlabel = "Default"\n[[models]]\nid = "probe"\nlabel = "Probe"\n'
            'provider_model = "lsp-local/lsp-probe"\nthinking = { omp = ["off"] }\n'
            '[[extensions]]\nid = "owned-state-observer"\nlabel = "Owned process observer"\nagent = "omp"\nkind = "fixed"\nentrypoint = '
            + json.dumps(str(observer)) + '\n')
        herdr_config = self.root / 'herdr.toml'
        herdr_config.write_text('onboarding = false\n[terminal]\ndefault_shell = "cmd.exe"\n[update]\nversion_check = false\nmanifest_check = false\n')
        self.env['HERDR_CONFIG_PATH'] = str(herdr_config)
        self.evidence['herdrVersion'] = self.command([self.args.herdr, '--version'])
        self.herdr_process = self.start([self.args.herdr, 'server'], 'herdr')
        self.herdr_started = True
        def ready():
            return subprocess.run([str(self.args.herdr), 'workspace', 'list'], cwd=self.root,
                env=self.env, capture_output=True, timeout=5).returncode == 0
        until('owned Herdr server', ready)
        self.evidence['workspace'] = self.herdr('workspace', 'create', '--cwd', str(self.root / 'zone'), '--label', 'suite-lsp', '--no-focus')
        q = lambda value: json.dumps(str(value))
        static = self.root / 'static'
        static.mkdir()
        (static / 'index.html').write_text('<!doctype html><title>S2/S4 only; S1 uses browser tests</title>')
        config = f'listen_addr = "127.0.0.1:0"\nstatic_directory = {q(static)}\ndata_directory = {q(self.root / "data")}\npaths_file = {q(self.root / "paths.toml")}\n[mdns]\nenabled = false\n'
        for name in ['start', 'stop']:
            config += f'[operations.{name}]\nscript = {q(self.root / "unused.bat")}\nworking_directory = {q(self.root)}\n'
        config += f'[herdr]\nworkspace_name = "suite-lsp"\nexecutable = {q(self.args.herdr)}\n[tasks]\nstorage_file = {q(self.root / "data/tasks.json")}\nmetadata_file = {q(self.root / "options.toml")}\n'
        config += f'[suites]\ndefinition_file = {q(authority / "suites.json")}\nmcp_file = {q(authority / "mcp.json")}\ngenerated_directory = {q(self.root / "generated resources with spaces")}\n'
        config += f'[sessions]\nhistory_file = {q(self.root / "data/history.json")}\n[zones]\nregistry_file = {q(self.root / "data/zones.json")}\n'
        (self.root / 'backend.toml').write_text(config)
        backend = self.start([self.args.backend, '--config', self.root / 'backend.toml'], 'backend')
        def address():
            require(backend.child.poll() is None, 'Backend exited; see retained logs')
            while not backend.lines.empty():
                match = re.search(r'http://(127\.0\.0\.1:\d+)/', backend.lines.get_nowait())
                if match:
                    return 'http://' + match[1]
        self.base = until('owned backend dynamic loopback', address)

    def snapshot(self):
        # Independent OS observation, scoped to descendants of the owned server.
        query = ('$all = @(Get-CimInstance Win32_Process); $owned = @(' + str(self.herdr_process.child.pid) + '); '
            'do { $previous = $owned.Count; $owned = @($owned + @($all | Where-Object { $_.ParentProcessId -in $owned } | '
            'Select-Object -ExpandProperty ProcessId) | Select-Object -Unique) } while ($owned.Count -gt $previous); '
            'ConvertTo-Json -Compress -InputObject @($all | Where-Object { $_.ProcessId -in $owned } | '
            'Select-Object ProcessId,ParentProcessId,Name,CommandLine)')
        text = self.command(['powershell.exe', '-NoProfile', '-NonInteractive', '-Command', query])
        rows = json.loads(text)
        for row in rows:
            pid = row['ProcessId']
            if pid not in self.handles:
                try:
                    self.handles[pid] = support['LiveProcess'](pid)
                except AssertionError:
                    # ERROR_INVALID_PARAMETER: a short-lived compiler/helper
                    # exited between CIM enumeration and OpenProcess. Record
                    # that fact; never count it as independent live evidence.
                    if ctypes.get_last_error() != 87:
                        raise
                    row['alive'] = False
                    continue
            row['alive'] = self.handles[pid].alive()
        return rows

    def scenario(self, name, missing=False):
        zone = self.root / ('missing-executable' if missing else name)
        zone.mkdir()
        marker, content, file, code = PROJECTS[name]
        (zone / marker).write_text(content)
        (zone / file).parent.mkdir(parents=True, exist_ok=True)
        (zone / file).write_text(code)
        # Negative retains the owned user override. Positive demonstrates
        # native project > user > Suite priority without touching global PATH.
        if name == 'rust-analyzer' and not missing:
            (zone / '.omp').mkdir()
            (zone / '.omp/lsp.json').write_text(json.dumps({'servers': {'rust-analyzer': {'command': 'rust-analyzer'}}}))
        case = {'server': name, 'missing': missing, 'cwd': str(zone), 'file': file,
                'requests': [], 'passed': False, 'processesBefore': self.snapshot()}
        self.evidence['cases'].append(case)
        self.model.case = case
        saved = self.http('/api/tasks', {'name': 'Native LSP ' + zone.name, 'chatName': '{task.name}',
            'agent': 'omp', 'taskType': 'normal', 'model': 'probe', 'thinking': 'off', 'skill': None,
            'prompt': 'Inspect native LSP initialization and status.', 'suite': 'four-languages'})
        task = next(task for task in saved['tasks'] if task['name'] == 'Native LSP ' + zone.name)
        self.http('/api/zones', {'id': zone.name, 'path': str(zone)})
        preview = self.http('/api/chats/preview', {'zone': zone.name, 'taskId': task['id']})
        launch = self.http('/api/chats', {'zone': zone.name, 'taskId': task['id'], 'chatId': preview['chatId'],
                                        'input': 'Observe native LSP.'}, status=202)
        def result():
            current = self.http('/api/chats/' + launch['id'])
            case['creation'] = current
            require(current['stage'] != 'failed', current)
            require(not case.get('error'), case.get('error'))
            return current if case['passed'] and current['stage'] == 'succeeded' else None
        try:
            until('real native LSP capabilities/status', result, seconds=self.args.timeout)
        except BaseException:
            case['verificationError'] = traceback.format_exc()
            raise
        finally:
            try:
                case['processesAfter'] = self.snapshot()
                pane = case.get('creation', {}).get('paneId')
                if pane:
                    case['terminal'] = self.command([self.args.herdr, 'pane', 'read', pane, '--lines', '150', '--format', 'text'])
            except BaseException:
                case['observationError'] = traceback.format_exc()
                if not case.get('verificationError'):
                    raise
        identity = json.loads((zone / '.owned-omp-process.json').read_text())
        case['ompIdentity'] = identity
        require(identity['cwd'] == str(zone) and any(row['ProcessId'] == identity['pid']
                for row in case['processesAfter']), 'Native OMP observer must identify a live owned process')
        if missing:
            require(not any(row['Name'].lower() == 'rust-analyzer.exe' for row in case['processesAfter']),
                    'Missing native Rust server must have no owned rust-analyzer process')
        else:
            before = {row['ProcessId'] for row in case['processesBefore']}
            tokens = {'rust-analyzer': 'rust-analyzer', 'typescript-language-server': 'typescript-language-server',
                      'jdtls': 'equinox', 'pyright': 'pyright'}
            require(any(row['alive'] and row['ProcessId'] not in before and tokens[name] in (row['CommandLine'] or '').lower()
                        for row in case['processesAfter']), ('No independent live language-server process', case['processesAfter']))
        case['verified'] = True

    def exercise(self):
        names = list(SERVERS) if self.args.language == 'all' else [self.args.language]
        self.evidence['blockers'] = [{'server': name, 'reason': 'Executable not discoverable on unchanged PATH; startup unverified'}
                                     for name in names if not self.evidence['executables'][name]]
        # Missing executable is always exercised, even on an unprovisioned host.
        self.scenario('rust-analyzer', missing=True)
        for name in names:
            if self.evidence['executables'][name]:
                self.scenario(name)
        require(not self.evidence['blockers'], self.evidence['blockers'])
        self.evidence['passed'] = True

    def cleanup_extra(self):
        errors = []
        for pid, handle in self.handles.items():
            try:
                until('owned process exit ' + str(pid), lambda: not handle.alive(), seconds=15)
            except BaseException:
                errors.append(traceback.format_exc())
            finally:
                handle.close()
        require(not errors, errors)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--herdr', type=Path, required=True)
    parser.add_argument('--omp-package', type=Path, required=True)
    parser.add_argument('--backend', type=Path, default=REPO / backend_suffix())
    parser.add_argument('--rust-toolchain', type=Path, help='Existing Rust toolchain directory; pins Rustup under isolated HOME, never installs')
    parser.add_argument('--language', choices=['all', *SERVERS], default='all',
                        help='A single language is narrower evidence, never full P37 acceptance')
    parser.add_argument('--timeout', type=int, choices=range(1, 301), default=120)
    args = parser.parse_args()
    args.repo = REPO
    return Acceptance(args).run()


if __name__ == '__main__':
    sys.exit(main())
