"""P14/P16/P23/P24/P26/P27 through real Task HTTP -> Herdr -> installed Pi.

Seams: S2 public Task/Chat HTTP, S3 isolated deployment, S4 native model wire,
real tool results and SessionManager history. Only the external model and MCP
endpoint are controlled. No agent monkey patch or alternate tool manager.
"""
import argparse
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

from developer_paths import REPO

support = runpy.run_path(str(REPO / 'scripts/test-real-herdr-close.py'))
require, until = support['require'], support['until']
PROMISES = {
    'P14': 'Declared LSP command is never started; Pi still consumes template, file and native MCP.',
    'P16': 'Template body absent before invocation; quoted positional arguments expand into one native user message.',
    'P23': 'Fixed toolkit entry consumes launch config with no optional extension selection.',
    'P24': 'No-Suite native and fixed-entry launches have identical tools and real MCP behavior in both modes.',
    'P26': 'Global only -> Suite on exposes read/bash/edit/write/codemode; reverse exposes only codemode; settings unchanged.',
    'P27': 'Only-mode codemode reads PI-ONLY-REAL-FILE-731 from an actual independently checked file.',
}
FILE_BODY = 'PI-ONLY-REAL-FILE-731\n'
MCP_BODY = 'PI-NATIVE-MCP-FILE-619\n'
TEMPLATE_BODY = 'PI-TEMPLATE-BODY-827 $1|$2|$@\n'
TEMPLATE_EXPECTED = 'PI-TEMPLATE-BODY-827 red fox|blue|red fox blue'


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def message_text(content):
    if isinstance(content, str):
        return content
    return ''.join(part.get('text', '') for part in content if part.get('type') == 'text')


class Model(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_POST(self):
        request = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        case = self.server.case
        case['requests'].append(request)
        try:
            if not request.get('tools'):
                delta, finish = {'role': 'assistant', 'content': '<title>Owned mode probe</title>'}, 'stop'
            else:
                names = [item['function']['name'] for item in request['tools']]
                expected = ['codemode'] if case['effectiveMode'] == 'only' else ['read', 'bash', 'edit', 'write', 'codemode']
                require(names == expected, ('Actual model declarations', expected, names))
                messages = request['messages']
                system = '\n'.join(str(m.get('content', '')) for m in messages if m['role'] in ['system', 'developer'])
                require('PI-TEMPLATE-BODY-827' not in system, 'Template body became a system instruction')
                if case.get('invokingTemplate'):
                    users = [message_text(m['content']) for m in messages if m['role'] == 'user']
                    require(any(text.strip() == TEMPLATE_EXPECTED for text in users),
                            ('Native quoted template expansion missing', users))
                    case['templateVerified'] = True
                    delta, finish = {'role': 'assistant', 'content': 'PI-TEMPLATE-CALL-VERIFIED'}, 'stop'
                elif not any(m['role'] == 'tool' for m in messages):
                    require('PI-TEMPLATE-BODY-827' not in json.dumps(messages), 'Template expanded before explicit invocation')
                    case['initialDeclarations'] = request['tools']
                    code = 'text(await tools.read({path:"sentinel.txt"}));text(await tools.mcp__native_file__read_sentinel({}));'
                    delta = {'role': 'assistant', 'tool_calls': [{'index': 0, 'id': 'owned-file-probe', 'type': 'function',
                        'function': {'name': 'codemode', 'arguments': json.dumps({'code': code})}}]}
                    finish = 'tool_calls'
                else:
                    output = '\n'.join(str(m.get('content', '')) for m in messages if m['role'] == 'tool')
                    require(FILE_BODY.strip() in output, ('Underlying file tool lost', output))
                    require(MCP_BODY.strip() in output, ('Native MCP source lost', output))
                    require('PI-TEMPLATE-BODY-827' not in json.dumps(messages), 'Template expanded before invocation')
                    case['toolOutput'] = output
                    case['passed'] = True
                    delta, finish = {'role': 'assistant', 'content': 'PI-MODE-TOOLS-VERIFIED'}, 'stop'
        except BaseException:
            case.setdefault('error', traceback.format_exc())
            delta, finish = {'role': 'assistant', 'content': 'PI-MODE-ACCEPTANCE-FAILED'}, 'stop'
        case.setdefault('responses', []).append({'delta': delta, 'finish': finish})
        self.send_response(200)
        self.send_header('Content-Type', 'text/event-stream')
        self.end_headers()
        for part, reason in [(delta, None), ({}, finish)]:
            chunk = {'id': 'owned-mode', 'object': 'chat.completion.chunk', 'created': 1, 'model': 'mode-probe',
                     'choices': [{'index': 0, 'delta': part, 'finish_reason': reason}]}
            self.wfile.write(('data: ' + json.dumps(chunk) + '\n\n').encode())
        self.wfile.write(b'data: [DONE]\n\n')


class Mcp(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_GET(self):
        self.send_error(405)

    def do_DELETE(self):
        self.send_response(200)
        self.end_headers()

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        self.server.wire.append(body)
        if 'id' not in body:
            self.send_response(202)
            self.end_headers()
            return
        method = body['method']
        if method == 'initialize':
            result = {'protocolVersion': '2025-03-26', 'capabilities': {'tools': {}},
                      'serverInfo': {'name': 'owned-native-file', 'version': '1'}}
        elif method == 'tools/list':
            result = {'tools': [{'name': 'read_sentinel', 'description': 'Read an owned native file',
                                'inputSchema': {'type': 'object', 'properties': {}}}]}
        elif method == 'tools/call':
            require(body['params']['name'] == 'read_sentinel', body)
            result = {'content': [{'type': 'text', 'text': self.server.file.read_text(encoding='utf-8')}]}
        else:
            result = {}
        payload = json.dumps({'jsonrpc': '2.0', 'id': body['id'], 'result': result}).encode()
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)


class Acceptance(support['Acceptance']):
    def __init__(self, args):
        super().__init__(args, prefix='pi-mode-owned-')
        # The caller owns the evidence directory; never reuse another invocation's data.
        old = self.root
        require(not args.evidence_dir.exists(), ('Evidence directory must not exist', str(args.evidence_dir)))
        args.evidence_dir.parent.mkdir(parents=True, exist_ok=True)
        shutil.move(str(old), str(args.evidence_dir))
        self.root = args.evidence_dir
        self.env = {key: value.replace(str(old), str(self.root)) for key, value in self.env.items()}
        self.evidence.update(root=str(self.root), promise=list(PROMISES), cases=[],
            coverage={key: {'expected': value, 'verified': False, 'cases': []} for key, value in PROMISES.items()})
        self.evidence['isolation'] = {key: self.env[key] for key in self.evidence['isolation']}
        self.mcp = None
        self.handles = []

    def prepare(self):
        require(self.args.backend.is_file(), ('Missing current worktree backend', str(self.args.backend)))
        require(self.args.herdr_bot_worktree == REPO and Path.cwd().resolve() == REPO,
                'Run the delivered script from its explicitly supplied herdr-bot worktree root')
        require(self.args.paths_file.is_file(), 'Missing isolated deployment paths file')
        require(sys.platform == 'win32', 'Real Herdr acceptance requires Windows process handles')
        target = Path(os.environ.get('CARGO_TARGET_DIR', '')).resolve()
        require('CARGO_TARGET_DIR' in os.environ and target.is_relative_to(REPO), 'Set independent CARGO_TARGET_DIR inside this worktree')
        require(self.args.backend.is_relative_to(target), 'Backend must belong to this worktree CARGO_TARGET_DIR')
        require((self.args.toolkit / 'bun.lock').is_file(), 'Missing participating toolkit worktree')
        self.evidence['invocation'] = {'argv': sys.argv, 'cwd': str(Path.cwd()), 'cargoTargetDir': str(target),
                                      'pathsFileSha256': digest(self.args.paths_file)}
        # A fresh Cargo artifact for these exact sources is the binary identity authority.
        native_env = self.env
        try:
            # Compilation uses the existing read-only toolchain/cache, offline; only
            # real agent/server processes use the isolated native HOME below.
            self.env = {key: value for key, value in os.environ.items() if not any(
                term in key.upper() for term in ['API_KEY', 'TOKEN', 'ANTHROPIC', 'OPENAI', 'AZURE'])}
            built = self.command(['cargo', 'build', '--offline', '--manifest-path', REPO / 'backend/Cargo.toml',
                                  '--bin', 'herdr-bot', '--bin', 'herdr-paths', '--message-format=json'], seconds=600)
        finally:
            self.env = native_env
        artifacts = [json.loads(line) for line in built.splitlines() if line.startswith('{')]
        current = next(Path(item['executable']).resolve() for item in artifacts
                       if item.get('reason') == 'compiler-artifact' and item['target']['name'] == 'herdr-bot' and item.get('executable'))
        require(digest(current) == digest(self.args.backend), 'Supplied backend differs from freshly built current-source binary')
        roots_cli = target / 'debug/herdr-paths.exe'
        roots = json.loads(self.command([roots_cli, '--paths', self.args.paths_file]))
        require('herdr' in roots and 'pi_package' in roots, 'Isolated paths must declare read-only roots.herdr and roots.pi_package')
        self.args.herdr = Path(roots['herdr']) / 'herdr.exe'
        self.args.pi_package = Path(roots['pi_package'])
        cli = self.args.pi_package / 'dist/bundle/cli.js'
        self.fixed = self.args.toolkit / 'entrypoints/pi/pi-suite.ts'
        for path in [self.args.herdr, cli, self.fixed, REPO / 'backend/static/index.html']:
            require(path.is_file(), ('Missing real worktree/dependency artifact', str(path)))
        launcher = Path(shutil.which('pi') or '')
        require(launcher.is_file(), 'Installed native pi launcher must already be on PATH')
        launcher_text = launcher.read_text(encoding='utf-8').replace('\\', '/')
        require('@earendil-works/pi-coding-agent' in launcher_text, 'Unexpected native Pi launcher')
        # Resolve the package beside the launcher, rather than accepting any similarly named install.
        launched_cli = launcher.parent / 'node_modules/@earendil-works/pi-coding-agent/dist/bundle/cli.js'
        require(launched_cli.is_file() and digest(launched_cli) == digest(cli), 'PATH Pi does not match declared read-only package')
        self.evidence['sources'] = {}
        for name, root in [('herdr-bot', REPO), ('toolkit', self.args.toolkit)]:
            self.evidence['sources'][name] = {'root': str(root),
                'revision': self.command(['git', '-C', root, 'rev-parse', 'HEAD']).strip(),
                'diff': self.command(['git', '-C', root, 'diff', '--', 'backend', 'scripts', 'meta-data', 'entrypoints', 'extensions/pi-suite']),
                'files': {str(path.relative_to(root)): digest(path) for path in
                          ([REPO / 'scripts/accept-suite-pi-mode.py', REPO / 'backend/Cargo.lock'] if name == 'herdr-bot' else
                           [self.fixed, root / 'extensions/pi-suite/harnesses/pi.ts', root / 'bun.lock'])}}
        self.evidence['executables'] = {str(path): digest(path) for path in [self.args.backend, current, self.args.herdr, cli, launcher]}
        self.evidence['versions'] = {'pi': json.loads((self.args.pi_package / 'package.json').read_text())['version'],
            'herdr': self.command([self.args.herdr, '--version']), 'node': self.command(['node', '--version']),
            'bun': self.command(['bun', '--version']), 'platform': sys.version}
        self.model = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Model)
        self.mcp = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Mcp)
        self.mcp.wire = []
        self.mcp.file = self.root / 'native-mcp.txt'
        self.mcp.file.write_text(MCP_BODY, encoding='utf-8')
        for server in [self.model, self.mcp]:
            threading.Thread(target=server.serve_forever, daemon=True).start()
        agent = self.root / 'agent'
        (agent / 'models.json').write_text(json.dumps({'providers': {'owned-mode': {
            'baseUrl': f'http://127.0.0.1:{self.model.server_port}/v1', 'api': 'openai-completions',
            'apiKey': 'owned-loopback-not-a-credential', 'models': [{'id': 'mode-probe', 'name': 'Owned mode',
            'reasoning': False, 'input': ['text'], 'contextWindow': 32768, 'maxTokens': 2048,
            'cost': {'input': 0, 'output': 0, 'cacheRead': 0, 'cacheWrite': 0}}]}}}), encoding='utf-8')
        (agent / 'mcp.json').write_text(json.dumps({'mcpServers': {'native_file': {
            'url': f'http://127.0.0.1:{self.mcp.server_port}/mcp', 'exposure': 'codemode'}}}), encoding='utf-8')
        self.authority = self.root / 'declarations with spaces'
        self.authority.mkdir()
        (self.authority / 'suite-mode.md').write_text('---\ndescription: Owned invocation probe\n---\n' + TEMPLATE_BODY, encoding='utf-8')
        self.lsp_marker = self.root / 'lsp-started.json'
        lsp = self.authority / 'owned-lsp.py'
        lsp.write_text('import pathlib,time\npathlib.Path(' + repr(str(self.lsp_marker)) + ').write_text("LSP-WAS-STARTED")\ntime.sleep(300)\n', encoding='utf-8')
        suites = []
        for mode in ['on', 'only']:
            suites.append({'id': 'mode-' + mode, 'name': 'Mode ' + mode, 'agents': ['pi', 'omp'],
                'skills': [], 'prompts': ['suite-mode.md'], 'mcp': [],
                'lsp': {'owned-lsp': {'command': sys.executable, 'args': [str(lsp)], 'fileTypes': ['.py']}},
                'pi': {'codemode': mode}})
        # Current authority schema is recorded; no array/ID compatibility is accepted.
        (self.authority / 'suites.json').write_text(json.dumps({'suites': suites}), encoding='utf-8')
        (self.authority / 'mcp.json').write_text('{"mcpServers":{}}', encoding='utf-8')
        (self.root / 'zone/sentinel.txt').write_text(FILE_BODY, encoding='utf-8')
        (self.root / 'zone/probe.py').write_text('owned_lsp_probe = 1\n', encoding='utf-8')
        q = lambda value: json.dumps(str(value))
        herdr_config = self.root / 'herdr.toml'
        herdr_config.write_text('onboarding = false\n[terminal]\ndefault_shell = "cmd.exe"\n[update]\nversion_check = false\nmanifest_check = false\n', encoding='utf-8')
        self.env['HERDR_CONFIG_PATH'] = str(herdr_config)
        self.start([self.args.herdr, 'server'], 'herdr')
        self.herdr_started = True
        until('owned named Herdr server', lambda: subprocess.run([str(self.args.herdr), 'workspace', 'list'],
            cwd=self.root, env=self.env, capture_output=True, timeout=5).returncode == 0)
        self.evidence['workspace'] = self.herdr('workspace', 'create', '--cwd', str(self.root / 'zone'), '--label', 'pi-mode-owned', '--no-focus')
        self.write_options(True)
        config = f'listen_addr = "127.0.0.1:0"\nstatic_directory = {q(REPO / "backend/static")}\ndata_directory = {q(self.root / "data")}\npaths_file = {q(self.args.paths_file)}\n[mdns]\nenabled = false\n'
        for operation in ['start', 'stop']:
            config += f'[operations.{operation}]\nscript = {q(self.root / "unused.bat")}\nworking_directory = {q(self.root)}\n'
        config += f'[herdr]\nworkspace_name = "pi-mode-owned"\nexecutable = {q(self.args.herdr)}\n[tasks]\nstorage_file = {q(self.root / "data/tasks.json")}\nmetadata_file = {q(self.root / "options.toml")}\n'
        config += f'[suites]\ndefinition_file = {q(self.authority / "suites.json")}\nmcp_file = {q(self.authority / "mcp.json")}\ngenerated_directory = {q(self.root / "generated resources")}\n'
        config += f'[sessions]\nhistory_file = {q(self.root / "data/history.json")}\n[zones]\nregistry_file = {q(self.root / "data/zones.json")}\n'
        (self.root / 'backend.toml').write_text(config, encoding='utf-8')
        backend = self.start([self.args.backend, '--config', self.root / 'backend.toml'], 'backend')
        def address():
            require(backend.child.poll() is None, 'Backend exited; inspect retained logs')
            while not backend.lines.empty():
                match = re.search(r'http://(127\.0\.0\.1:\d+)/', backend.lines.get_nowait())
                if match:
                    return 'http://' + match[1]
        self.base = until('owned dynamic backend', address)
        self.http('/api/zones', {'id': 'owned', 'path': str(self.root / 'zone')})

    def write_options(self, fixed):
        text = 'skills = []\n[[agents]]\nid = "pi"\nlabel = "Pi"\n[[models]]\nid = "default"\nlabel = "Default"\n[[models]]\nid = "probe"\nlabel = "Probe"\nprovider_model = "owned-mode/mode-probe"\nthinking = { pi = ["off"] }\n'
        # Both arms retain the same real, shipped model-cost fixed extension.
        # Metadata requires a fixed extension for each agent; never fake one.
        text += '[[extensions]]\nid = "pi-model-cost"\nlabel = "Model cost"\nagent = "pi"\nkind = "fixed"\nentrypoint = ' + json.dumps(str(self.args.toolkit / 'entrypoints/pi/model-cost.ts')) + '\n'
        if fixed:
            text += '[[extensions]]\nid = "pi-suite"\nlabel = "pi-suite"\nagent = "pi"\nkind = "fixed"\nentrypoint = ' + json.dumps(str(self.fixed)) + '\n'
        (self.root / 'options.toml').write_text(text, encoding='utf-8')

    def processes_snapshot(self):
        # Never export unrelated/user process command lines to acceptance evidence.
        server_pid = self.processes[0].child.pid
        script = (f'$owned = [System.Collections.Generic.HashSet[int]]::new(); [void]$owned.Add({server_pid}); '
            '$tree = @(Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId); '
            'do { $added = $false; foreach ($p in $tree) { '
            'if ($owned.Contains([int]$p.ParentProcessId) -and $owned.Add([int]$p.ProcessId)) { $added = $true } '
            '} } while ($added); '
            '$result = @(Get-CimInstance Win32_Process | Where-Object { $owned.Contains([int]$_.ProcessId) } '
            '| Select-Object ProcessId,ParentProcessId,CommandLine); ConvertTo-Json -InputObject $result -Compress')
        return json.loads(self.command(['powershell.exe', '-NoProfile', '-Command', script]))

    def case(self, name, native_mode, suite_mode=None, fixed=True, template=False):
        case = {'name': name, 'nativeMode': native_mode, 'suiteMode': suite_mode, 'fixed': fixed,
                'effectiveMode': suite_mode or native_mode, 'requests': [], 'passed': False}
        self.evidence['cases'].append(case)
        self.model.case = case
        self.write_options(fixed)
        settings = self.root / 'agent/settings.json'
        settings.write_text(json.dumps({'autoUpdate': False, 'sessionDir': str(self.root / 'agent/sessions'),
            'defaultTools': ['+codemode'], 'codemode': {'mode': native_mode}}), encoding='utf-8')
        watched = [settings, self.root / 'agent/mcp.json', self.authority / 'suite-mode.md', self.root / 'zone/sentinel.txt', self.mcp.file]
        before = {str(path): digest(path) for path in watched}
        case['configurationBefore'] = before
        case['settingsBefore'] = json.loads(settings.read_text(encoding='utf-8'))
        wire_mark = len(self.mcp.wire)
        body = {'name': name, 'chatName': '{chat.id}', 'agent': 'pi', 'taskType': 'normal', 'model': 'probe',
                'thinking': 'off', 'skill': None, 'prompt': '', 'extensions': [],
                'suite': None if suite_mode is None else 'mode-' + suite_mode}
        response = self.http('/api/tasks', body)
        task = next(task for task in response['tasks'] if task['name'] == name)
        preview = self.http('/api/chats/preview', {'zone': 'owned', 'taskId': task['id']})
        launch = self.http('/api/chats', {'zone': 'owned', 'taskId': task['id'], 'chatId': preview['chatId'],
            'input': 'Read the owned file and native MCP probe via codemode.'}, status=202)
        pane = None
        try:
            def result():
                current = self.http('/api/chats/' + launch['id'])
                case['creation'] = current
                require(current['stage'] != 'failed', current)
                require(not case.get('error'), case.get('error'))
                return current if case['passed'] and current['stage'] == 'succeeded' else None
            current = until(name + ' actual native calls', result, seconds=self.args.timeout)
            pane = current['paneId']
            info = self.herdr('pane', 'process-info', '--pane', pane)
            case['processInfo'] = info
            snapshot = self.processes_snapshot()
            by_pid = {p['ProcessId']: p for p in snapshot}
            server_pid = self.processes[0].child.pid
            def descendant(proc):
                seen = set()
                parent = proc['ParentProcessId']
                while parent in by_pid and parent not in seen:
                    if parent == server_pid:
                        return True
                    seen.add(parent)
                    parent = by_pid[parent]['ParentProcessId']
                return False
            owned = [p for p in snapshot if descendant(p)]
            require(any('pi-coding-agent' in (p.get('CommandLine') or '') for p in owned),
                    ('No observable native Pi descendant of owned Herdr server', owned))
            case['ownedProcesses'] = owned
            case_handles = []
            for proc in owned:
                handle = support['LiveProcess'](proc['ProcessId'])
                case_handles.append(handle)
                self.handles.append(handle)
            require(not any('owned-lsp.py' in (p.get('CommandLine') or '') for p in snapshot), 'Pi started Suite LSP')
            require(not self.lsp_marker.exists(), 'Declared LSP command executed')
            if template:
                case['invokingTemplate'] = True
                self.command([self.args.herdr, 'agent', 'prompt', pane, '/suite-mode "red fox" blue'], seconds=self.args.timeout + 10)
                until('native explicit template expansion', lambda: (require(not case.get('error'), case.get('error')) or case.get('templateVerified')), seconds=self.args.timeout)
            case['terminal'] = self.command([self.args.herdr, 'pane', 'read', pane, '--lines', '200', '--format', 'text'])
            # Public native history API, not the backend's registration side channel.
            histories = list((self.root / 'agent/sessions').rglob('*.jsonl'))
            require(histories, 'Native session history missing')
            case['histories'] = [{'path': str(file), 'sha256': digest(file), 'native': self.history(str(file))} for file in histories]
            if template:
                history = json.dumps(case['histories'], ensure_ascii=False)
                require(TEMPLATE_EXPECTED in history and '/suite-mode \\"red fox\\" blue' not in history, 'Native history lacks expanded template text')
            case['mcpWire'] = self.mcp.wire[wire_mark:]
            require(sum(item.get('method') == 'tools/call' for item in case['mcpWire']) == 1, 'Native MCP call not observed exactly once')
            case['configurationAfter'] = {str(path): digest(path) for path in watched}
            case['settingsAfter'] = json.loads(settings.read_text(encoding='utf-8'))
            require(case['settingsAfter']['codemode'] == {'mode': native_mode}, 'Suite wrote back original global mode')
            require(all(case['configurationAfter'][str(path)] == before[str(path)] for path in watched if path != settings),
                    'Suite modified native MCP or resource sources')
            require((self.root / 'zone/sentinel.txt').read_text(encoding='utf-8') == FILE_BODY, 'Controlled file changed')
            case['verified'] = True
        finally:
            pane = pane or case.get('creation', {}).get('paneId')
            if pane:
                try:
                    if 'terminal' not in case:
                        case['terminal'] = self.command([self.args.herdr, 'pane', 'read', pane, '--lines', '200', '--format', 'text'])
                    if 'case_handles' not in locals():
                        failed_snapshot = self.processes_snapshot()
                        case['failureProcessSnapshot'] = failed_snapshot
                        for proc in failed_snapshot:
                            if proc['ProcessId'] != self.processes[0].child.pid:
                                self.handles.append(support['LiveProcess'](proc['ProcessId']))
                except BaseException:
                    self.evidence['cleanupErrors'].append(traceback.format_exc())
                try:
                    self.herdr('pane', 'close', pane)
                    if 'case_handles' in locals():
                        native_handles = [handle for handle in case_handles if any(
                            proc['ProcessId'] == handle.pid and 'pi-coding-agent' in (proc.get('CommandLine') or '')
                            for proc in owned)]
                        for handle in native_handles:
                            until('closed pane native Pi exits', lambda handle=handle: not handle.alive(), seconds=15)
                        case['nativeExitConfirmed'] = True
                except BaseException:
                    self.evidence['cleanupErrors'].append(traceback.format_exc())
        return case

    def mark(self, promise, cases):
        self.evidence['coverage'][promise].update(verified=True, cases=[case['name'] for case in cases])

    def exercise(self):
        if self.args.scenario in ['all', 'baseline']:
            pairs = []
            for mode in ['on', 'only']:
                native = self.case('native-no-suite-' + mode, mode, fixed=False)
                fixed = self.case('fixed-no-suite-' + mode, mode)
                require(native['initialDeclarations'] == fixed['initialDeclarations'], 'Inert fixed entry changed native declarations')
                pairs.extend([native, fixed])
            self.mark('P24', pairs)
        if self.args.scenario in ['all', 'modes', 'template']:
            on = self.case('global-only-suite-on', 'only', 'on', template=self.args.scenario != 'modes')
            only = self.case('global-on-suite-only', 'on', 'only')
            for promise in ['P23', 'P26', 'P27']:
                self.mark(promise, [on, only])
            if self.args.scenario != 'modes':
                self.mark('P16', [on])
                self.mark('P14', [on, only])
        self.evidence['passed'] = self.args.scenario == 'all' and all(item['verified'] for item in self.evidence['coverage'].values())
        self.evidence['selectedScenarioSucceeded'] = True

    def cleanup_extra(self):
        if self.mcp:
            self.evidence['mcpWire'] = self.mcp.wire
            self.mcp.shutdown()
            self.mcp.server_close()
        exits = []
        for handle in self.handles:
            try:
                until('owned native PID exits', lambda: not handle.alive(), seconds=15)
                exits.append({'pid': handle.pid, 'exited': True})
            except BaseException:
                self.evidence['cleanupErrors'].append(traceback.format_exc())
                exits.append({'pid': handle.pid, 'exited': False})
            finally:
                handle.close()
        self.evidence['ownedProcessExits'] = exits
        self.evidence['serviceExits'] = [{'pid': p.child.pid, 'exitCode': p.child.poll()} for p in self.processes]
        for item in self.evidence['coverage'].values():
            item['processCleanupConfirmed'] = not self.evidence['cleanupErrors']
            item['verified'] = item['verified'] and not self.evidence['cleanupErrors']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ['herdr-bot-worktree', 'toolkit-worktree', 'backend', 'paths-file', 'evidence-dir']:
        parser.add_argument('--' + flag, type=lambda path: Path(path).resolve(), required=True)
    parser.add_argument('--scenario', choices=['all', 'baseline', 'modes', 'template'], default='all')
    parser.add_argument('--timeout', type=int, choices=range(1, 301), default=90)
    args = parser.parse_args()
    args.repo, args.toolkit, args.node = args.herdr_bot_worktree, args.toolkit_worktree, 'node'
    return Acceptance(args).run()


if __name__ == '__main__':
    sys.exit(main())
