"""S2/S3/S4 Pi resources and native MCP acceptance, using two explicit worktrees.

The controlled model and file-backed MCP endpoints supply external conditions only.
Actual Task HTTP -> Herdr -> installed agent -> native tools/resources remains real.
Each scenario owns its deployment, processes, dynamic ports and retained evidence.
"""
import argparse
from collections import Counter
import ctypes
import hashlib
import html
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

REPO = Path(__file__).resolve().parent.parent
support = runpy.run_path(str(REPO / 'scripts/test-real-herdr-close.py'))
require, until = support['require'], support['until']
SCENARIOS = {
    'selected': ['P6/pi-paths', 'P7/pi-consumption', 'P9/pi-dependencies', 'P10/pi-filter-and-mcp'],
    'append': ['P17/pi-central-empty-project', 'P19/pi-builtin-and-suite'],
    'same-name': ['P20/pi-file-name-wins'],
    'equivalent': ['P20/pi-equivalent-other-name'],
    'disabled': ['P20/pi-disabled'],
    'empty': ['P21/pi-empty-list'],
    'unreadable-skill': ['P46/pi-unreadable-skill'],
    'unreadable-extension': ['P46/pi-unreadable-extension'],
    'generation-failure': ['P46/pi-generation-failure'],
    'incompatible': ['P46/pi-incompatible-suite'],
    'unreachable': ['P46/pi-native-runtime-error'],
}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def command(argv, cwd):
    result = subprocess.run([str(x) for x in argv], cwd=cwd, capture_output=True,
                            encoding='utf-8', errors='replace', timeout=60)
    require(result.returncode == 0, {'argv': list(map(str, argv)), 'exit': result.returncode,
                                   'stdout': result.stdout, 'stderr': result.stderr})
    return result.stdout


class Mcp(http.server.BaseHTTPRequestHandler):
    """Real wire endpoint: independent on-disk sentinels, no native manager replacement."""
    def log_message(self, *_):
        pass

    def do_GET(self):
        self.send_error(405)

    def do_DELETE(self):
        self.send_response(200)
        self.end_headers()

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        route = self.path.strip('/')
        record = {'route': route, 'request': body}
        self.server.wire.append(record)
        if 'id' not in body:
            self.send_response(202)
            self.end_headers()
            return
        if body['method'] == 'initialize':
            result = {'protocolVersion': '2025-03-26', 'capabilities': {'tools': {}},
                      'serverInfo': {'name': 'owned-file-probe', 'version': '1'}}
        elif body['method'] == 'ping':
            result = {}
        elif body['method'] == 'tools/list':
            result = {'tools': [{'name': 'read_sentinel', 'description': 'Read this server owned file',
                                'inputSchema': {'type': 'object', 'properties': {}}}]}
        elif body['method'] == 'tools/call':
            require(body['params']['name'] == 'read_sentinel', body)
            result = {'content': [{'type': 'text', 'text': (self.server.files / f'{route}.txt').read_text()}]}
        else:
            self.send_error(400)
            return
        record['result'] = result
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.end_headers()
        self.wfile.write(json.dumps({'jsonrpc': '2.0', 'id': body['id'], 'result': result}).encode())


class Model(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_POST(self):
        request = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        case = self.server.case
        case['requests'].append(request)
        try:
            messages = request['messages']
            if not request.get('tools'):
                delta, finish = {'role': 'assistant', 'content': '<title>Owned Suite acceptance</title>'}, 'stop'
            elif len(messages) == 1 and str(messages[0].get('content', '')).startswith('Compress into one routing hint'):
                delta, finish = {'role': 'assistant', 'content': 'Use owned resources.'}, 'stop'
            elif not any(m['role'] == 'tool' for m in messages):
                system = '\n'.join(str(m.get('content', '')) for m in messages if m['role'] in ['system', 'developer'])
                calls = []
                if case['selected']:
                    if case['agent'] == 'pi':
                        advertised = dict((name, html.unescape(location)) for name, location in re.findall(
                            r'<name>(.*?)</name>\s*<description>.*?</description>\s*<location>(.*?)</location>', system, re.S))
                        require(sorted(advertised) == ['selected-relative', 'selected-root'], ('Exact selected skills', advertised, system))
                        require(all('generated resources' in p for p in advertised.values()), advertised)
                        paths = [advertised['selected-root'], str(Path(advertised['selected-root']).parent / 'scripts/probe.py'),
                                 str(Path(advertised['selected-root']).parent / 'assets/payload.txt'), advertised['selected-relative']]
                    else:
                        paths = ['skill://selected-root', 'skill://selected-root/scripts/probe.py',
                                 'skill://selected-root/assets/payload.txt', 'skill://selected-relative']
                    case['resourcePaths'] = paths
                    for path in paths:
                        args = {'path': path}
                        if case['agent'] == 'omp':
                            args['i'] = 'Reading selected resource'
                        calls.append(('read', args))
                    if case['agent'] == 'pi':
                        calls.append(('suite_asset_probe', {}))
                    else:
                        calls.append(('write', {'i': 'Consuming package asset', 'path': 'xd://suite_asset_probe', 'content': '{}'}))
                for name in case['mcpTools']:
                    calls.append(('codemode', {'code': f'text(await tools.mcp__{name}__read_sentinel({{}}))'}))
                if case['scenario'] == 'unreachable':
                    calls.append(('codemode', {'code': 'text(await tools.mcp__unreachable__read_sentinel({}))'}))
                delta = {'role': 'assistant', 'tool_calls': [dict(index=i, id=f'owned-{i}', type='function',
                    function={'name': name, 'arguments': json.dumps(args)}) for i, (name, args) in enumerate(calls)]}
                finish = 'tool_calls'
            else:
                output = '\n'.join(str(m.get('content', '')) for m in messages if m['role'] == 'tool')
                for literal in case['expected']:
                    require(literal in output, ('Native tool output missing', literal, output))
                if case['scenario'] == 'unreachable':
                    require(any(word in output.lower() for word in ['error', 'not found', 'failed', 'unknown']), output)
                case['toolOutput'] = output
                case['consumed'] = True
                delta, finish = {'role': 'assistant', 'content': 'OWNED-SUITE-ACCEPTANCE-419'}, 'stop'
        except BaseException:
            case.setdefault('error', traceback.format_exc())
            delta, finish = {'role': 'assistant', 'content': 'OWNED-SUITE-ACCEPTANCE-FAILED'}, 'stop'
        self.send_response(200)
        self.send_header('Content-Type', 'text/event-stream')
        self.end_headers()
        for item, reason in [(delta, None), ({}, finish)]:
            payload = {'id': 'owned', 'object': 'chat.completion.chunk', 'created': 1, 'model': 'suite-probe',
                       'choices': [{'index': 0, 'delta': item, 'finish_reason': reason}]}
            self.wfile.write(('data: ' + json.dumps(payload) + '\n\n').encode())
        self.wfile.write(b'data: [DONE]\n\n')


class Acceptance(support['Acceptance']):
    def __init__(self, args, scenario):
        super().__init__(args, prefix=str(args.evidence_dir / (scenario + '-')))
        self.scenario = scenario
        self.handles = []
        self.mcp = None
        self.evidence.update(promise=SCENARIOS[scenario], scenario=scenario, cases=[], processExits=[])
        self.env = {k: v for k, v in self.env.items() if not any(x in k.upper() for x in ['SECRET', 'PASSWORD', 'PROXY', 'CREDENTIAL'])}

    def prepare(self):
        require(sys.platform == 'win32', 'Real Herdr acceptance requires Windows process handles')
        self.mcp = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Mcp)
        self.mcp.wire = []
        self.mcp.files = self.root
        for route, literal in [('native', 'NATIVE-FILE-113'), ('suite', 'SUITE-FILE-997'), ('loser', 'WRONG-FILE-266')]:
            (self.root / f'{route}.txt').write_text(literal)
        threading.Thread(target=self.mcp.serve_forever, daemon=True).start()
        self.model = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Model)
        threading.Thread(target=self.model.serve_forever, daemon=True).start()
        agent = self.root / 'agent'
        (agent / 'settings.json').write_text(json.dumps({'autoUpdate': False}))
        (agent / 'models.json').write_text(json.dumps({'providers': {'suite-local': {
            'baseUrl': f'http://127.0.0.1:{self.model.server_port}/v1', 'api': 'openai-completions',
            'apiKey': 'owned-loopback-not-a-credential', 'models': [{'id': 'suite-probe', 'name': 'Suite probe',
            'reasoning': False, 'input': ['text'], 'contextWindow': 32768, 'maxTokens': 4096,
            'cost': {'input': 0, 'output': 0, 'cacheRead': 0, 'cacheWrite': 0}}]}}}))
        # Each host reads its own native format from this test-owned agent directory.
        (agent / 'config.yml').write_text('autoResume: false\nstartup:\n  setupWizard: false\n  showSplash: false\n  checkUpdate: false\nmarketplace:\n  autoUpdate: off\n')
        shutil.copy2(agent / 'models.json', agent / 'models.yml')
        authority = self.root / 'declarations with spaces'
        authority.mkdir()
        self.authority = authority
        root = self.root / 'central resources with spaces'
        root.mkdir()
        self.skill = root / 'root skill'
        self.extension = authority / 'relative package'
        for directory, name, text in [(self.skill, 'selected-root', 'ROOT-SKILL-BODY-119'),
            (authority / 'relative skill', 'selected-relative', 'RELATIVE-SKILL-BODY-508'),
            (root / 'unselected', 'not-selected', 'UNSELECTED-BODY-MUST-NOT-APPEAR')]:
            directory.mkdir()
            (directory / 'SKILL.md').write_text(f'---\nname: {name}\ndescription: Owned resource\n---\n{text}\n')
        (self.skill / 'scripts').mkdir()
        (self.skill / 'scripts/probe.py').write_text('print("SCRIPT-READ-947")\n')
        (self.skill / 'assets').mkdir()
        (self.skill / 'assets/payload.txt').write_text('ASSET-BYTES-276')
        (self.extension / 'extension').mkdir(parents=True)
        (self.extension / 'assets').mkdir()
        (self.extension / 'node_modules/owned-factor').mkdir(parents=True)
        (self.extension / 'node_modules/owned-factor/package.json').write_text('{"type":"module","main":"index.js"}')
        (self.extension / 'node_modules/owned-factor/index.js').write_text('export const factor=3;')
        (self.extension / 'extension/sibling.ts').write_text('export const offset=7;')
        (self.extension / 'assets/payload.json').write_text('{"unit":19}')
        (self.extension / 'package.json').write_text(json.dumps({'type': 'module'}))
        (self.extension / 'extension/probe.ts').write_text('''import {readFileSync} from "node:fs";
import {factor} from "owned-factor";
import {offset} from "./sibling.ts";
export default function(pi) {
 pi.registerTool({name:"suite_asset_probe",label:"Suite asset",description:"Consume relative package dependencies",
 parameters:pi.zod ? pi.zod.object({}) : {type:"object",properties:{}},async execute(){
 const value=JSON.parse(readFileSync(new URL("../assets/payload.json",import.meta.url),"utf8"));
 return {content:[{type:"text",text:"PACKAGE-ASSET-482:"+(value.unit*factor+offset)}],details:{}};
 }});
}
''')
        selected = self.scenario == 'selected'
        negative = self.scenario in ['unreadable-skill', 'unreadable-extension', 'generation-failure', 'incompatible']
        self.suite = {'id': 'shared', 'name': 'Shared declaration', 'agents': ['omp', 'pi'],
                      'skills': ['${roots.resources}/root skill', 'relative skill'] if selected or self.scenario == 'unreadable-skill' else [],
                      'pi': {'codemode': 'on'}, 'mcp': []}
        if selected or self.scenario == 'unreadable-extension':
            self.suite['extensions'] = {'omp': ['relative package/extension/probe.ts'], 'pi': ['relative package/extension/probe.ts']}
        native = {'native_file': self.server_config('native')}
        central = {}
        if selected or self.scenario == 'append':
            central['suite_file'] = self.server_config('suite')
        elif self.scenario == 'same-name':
            central['native_file'] = self.server_config('loser')
        elif self.scenario == 'equivalent':
            central['alias_file'] = self.server_config('native')
        elif self.scenario == 'disabled':
            native['suite_file'] = {**self.server_config('suite'), 'enabled': False}
            central['suite_file'] = self.server_config('suite')
        elif self.scenario == 'unreachable':
            # A valid URL on an owned, bound, non-listening socket: no port race or unrelated service.
            import socket
            self.unreachable = socket.socket()
            self.unreachable.bind(('127.0.0.1', 0))
            central['unreachable'] = {'url': f'http://127.0.0.1:{self.unreachable.getsockname()[1]}/mcp', 'exposure': 'direct'}
        self.suite['mcp'] = list(central)
        (agent / 'mcp.json').write_text(json.dumps({'mcpServers': native}))
        self.native_path = agent / 'mcp.json'
        self.native_bytes = self.native_path.read_bytes()
        (authority / 'mcp.json').write_text(json.dumps({'mcpServers': central}))
        self.publish()
        (self.root / 'paths.toml').write_text('[roots]\nresources = ' + json.dumps(str(root)) + '\n')
        self.original = {str(p): digest(p) for directory in [root, authority] for p in directory.rglob('*') if p.is_file()}
        q = lambda value: json.dumps(str(value))
        options = 'skills = []\n'
        for name in ['pi', 'omp']:
            options += f'[[agents]]\nid = "{name}"\nlabel = "{name}"\n'
        options += '[[models]]\nid = "default"\nlabel = "Default"\n[[models]]\nid = "probe"\nlabel = "Probe"\nprovider_model = "suite-local/suite-probe"\nthinking = { pi = ["off"], omp = ["off"] }\n'
        options += '[[extensions]]\nid = "pi-suite"\nlabel = "pi-suite"\nagent = "pi"\nkind = "fixed"\nentrypoint = ' + q(self.args.toolkit / 'entrypoints/pi/pi-suite.ts') + '\n'
        observer = self.root / 'omp-fixed-observer.ts'
        observer.write_text('import {writeFileSync} from "node:fs"; export default function(pi) { pi.on("session_start", () => writeFileSync('
            + q(self.root / 'omp-fixed-loaded.txt') + ', "FIXED-LOADED-628")); }')
        options += '[[extensions]]\nid = "omp-fixed-observer"\nlabel = "Owned fixed observer"\nagent = "omp"\nkind = "fixed"\nentrypoint = ' + q(observer) + '\n'
        (self.root / 'options.toml').write_text(options)
        herdr_config = self.root / 'herdr.toml'
        herdr_config.write_text('onboarding = false\n[terminal]\ndefault_shell = "cmd.exe"\n[update]\nversion_check = false\nmanifest_check = false\n')
        self.env['HERDR_CONFIG_PATH'] = str(herdr_config)
        self.start([self.args.herdr, 'server'], 'herdr')
        self.herdr_started = True
        until('owned Herdr server', lambda: subprocess.run([str(self.args.herdr), 'workspace', 'list'], cwd=self.root,
            env=self.env, capture_output=True, timeout=5).returncode == 0)
        self.herdr('workspace', 'create', '--cwd', str(self.root / 'zone'), '--label', 'suite-resources', '--no-focus')
        config = f'listen_addr = "127.0.0.1:0"\nstatic_directory = {q(self.args.repo / "backend/static")}\ndata_directory = {q(self.root / "data")}\npaths_file = {q(self.root / "paths.toml")}\n[mdns]\nenabled = false\n'
        for name in ['start', 'stop']:
            config += f'[operations.{name}]\nscript = {q(self.root / "unused.bat")}\nworking_directory = {q(self.root)}\n'
        config += f'[herdr]\nworkspace_name = "suite-resources"\nexecutable = {q(self.args.herdr)}\n[tasks]\nstorage_file = {q(self.root / "data/tasks.json")}\nmetadata_file = {q(self.root / "options.toml")}\n'
        config += f'[suites]\ndefinition_file = {q(authority / "suites.json")}\nmcp_file = {q(authority / "mcp.json")}\ngenerated_directory = {q(self.root / "generated resources")}\n'
        config += f'[sessions]\nhistory_file = {q(self.root / "data/history.json")}\n[zones]\nregistry_file = {q(self.root / "data/zones.json")}\n'
        (self.root / 'backend.toml').write_text(config)
        backend = self.start([self.args.backend, '--config', self.root / 'backend.toml'], 'backend')
        def address():
            require(backend.child.poll() is None, 'Backend exited; see retained logs')
            while not backend.lines.empty():
                match = re.search(r'http://(127\.0\.0\.1:\d+)/', backend.lines.get_nowait())
                if match:
                    return 'http://' + match[1]
        self.base = until('owned backend loopback', address)
        self.http('/api/suites')
        if not negative and (selected):
            require(os.environ.get('HERDR_BOT_MANUAL_SYMLINKS') == '1', 'Selected resource acceptance requires explicit HERDR_BOT_MANUAL_SYMLINKS=1; no link fallback')

    def server_config(self, route):
        return {'url': f'http://127.0.0.1:{self.mcp.server_port}/{route}', 'exposure': 'direct'}

    def publish(self):
        (self.authority / 'suites.json').write_text(json.dumps({'suites': [self.suite]}))

    def exercise(self):
        selected = self.scenario == 'selected'
        count = 3 if selected else 1
        for index in range(count):
            agent = 'omp' if selected and index == 0 else 'pi'
            zone = self.root / ('zone' if index == 0 else f'other cwd with spaces {index}')
            zone.mkdir(exist_ok=True)
            if selected:
                # Unselected defaults and wrong cwd-relative paths must not enter Pi's final set.
                for directory, name in [(zone / '.pi/skills/default', 'project-default'),
                                        (self.root / 'agent/skills/default', 'user-default'),
                                        (self.root / 'agent/skills/shadow', 'selected-root'),
                                        (zone / 'relative skill', 'wrong-cwd')]:
                    directory.mkdir(parents=True, exist_ok=True)
                    (directory / 'SKILL.md').write_text(f'---\nname: {name}\ndescription: Not selected\n---\nWRONG-DEFAULT\n')
                if index == 2:
                    extra = self.skill.parent / 'new source skill'
                    extra.mkdir()
                    (extra / 'SKILL.md').write_text('---\nname: newly-added\ndescription: Not selected\n---\nNEW-MUST-NOT-APPEAR\n')
            self.http('/api/zones', {'id': f'owned-{index}', 'path': str(zone)})
            saved = self.http('/api/tasks', {'name': f'{self.scenario}-{index}', 'chatName': '{task.name}-{chat.id}',
                'agent': agent, 'taskType': 'normal', 'model': 'probe', 'thinking': 'off', 'skill': None,
                'prompt': 'Consume the owned acceptance resources.', 'suite': 'shared'})['tasks']
            task = next(task for task in saved if task['name'] == f'{self.scenario}-{index}')
            preview = self.http('/api/chats/preview', {'zone': f'owned-{index}', 'taskId': task['id']})
            before_tabs = self.herdr('tab', 'list')
            lock = None
            if self.scenario.startswith('unreadable-'):
                lock_path = self.skill / 'SKILL.md' if self.scenario == 'unreadable-skill' else self.extension / 'extension/probe.ts'
                kernel = ctypes.WinDLL('kernel32', use_last_error=True)
                kernel.CreateFileW.argtypes = [ctypes.c_wchar_p, ctypes.c_ulong, ctypes.c_ulong, ctypes.c_void_p, ctypes.c_ulong, ctypes.c_ulong, ctypes.c_void_p]
                kernel.CreateFileW.restype = ctypes.c_void_p
                kernel.CloseHandle.argtypes = [ctypes.c_void_p]
                lock = kernel.CreateFileW(str(lock_path), 0x80000000, 0, None, 3, 0, None)
                require(lock not in [None, ctypes.c_void_p(-1).value], ('Cannot exclusively lock selected resource', ctypes.get_last_error()))
            elif self.scenario == 'generation-failure':
                (self.root / 'generated resources').write_text('OWNED-OUTPUT-BLOCKER')
            elif self.scenario == 'incompatible':
                self.suite['agents'] = ['omp']
                self.suite.pop('pi')
                self.publish()
                self.http('/api/suites')  # Still a valid declaration; only Task compatibility changed.
            case = {'agent': agent, 'scenario': self.scenario, 'selected': selected, 'requests': [],
                    'expected': [], 'mcpTools': [], 'consumed': False, 'cwd': str(zone)}
            if selected:
                case['expected'] = ['ROOT-SKILL-BODY-119', 'SCRIPT-READ-947', 'ASSET-BYTES-276', 'RELATIVE-SKILL-BODY-508', 'PACKAGE-ASSET-482:64']
            if agent == 'pi':
                case['mcpTools'] = ['native_file']
                if selected or self.scenario == 'append':
                    case['mcpTools'].append('suite_file')
                if self.scenario == 'equivalent':
                    case['mcpTools'].append('alias_file')
                    case['equivalenceOracle'] = 'Pi 1.5 docs/mcp.md: same URL under different names retains separately named servers/sign-ins; unlike OMP, URL equality is not name identity.'
                case['expected'] += ['NATIVE-FILE-113'] + (['SUITE-FILE-997'] if selected or self.scenario == 'append' else [])
                case['expectedNativeServerNames'] = list(case['mcpTools'])
            self.evidence['cases'].append(case)
            self.model.case = case
            wire_start = len(self.mcp.wire)
            payload = {'zone': f'owned-{index}', 'taskId': task['id'], 'chatId': preview['chatId'], 'input': 'Execute owned resource and MCP probes.'}
            try:
                negative = self.scenario in ['unreadable-skill', 'unreadable-extension', 'generation-failure', 'incompatible']
                if negative:
                    case['rejection'] = self.http('/api/chats', payload, status=400)
                    rejected = json.dumps(case['rejection'], ensure_ascii=False)
                    expected_cause = {'unreadable-skill': 'root skill', 'unreadable-extension': 'relative package',
                                      'generation-failure': 'shared', 'incompatible': 'shared'}[self.scenario]
                    require(expected_cause in rejected, ('Wrong launch rejection cause', expected_cause, rejected))
                    require(case['requests'] == [], 'Rejected launch reached the model')
                    require(self.herdr('tab', 'list') == before_tabs, 'Rejected launch created a Herdr tab')
                    case['verified'] = True
                    continue
                launch = self.http('/api/chats', payload, status=202)
                def ready():
                    state = self.http('/api/chats/' + launch['id'])
                    case['creation'] = state
                    require(state['stage'] != 'failed', state)
                    require(not case.get('error'), case.get('error'))
                    return state if state['stage'] == 'succeeded' and case['consumed'] else None
                try:
                    until('native resource and MCP results', ready, seconds=self.args.timeout)
                finally:
                    pane = case.get('creation', {}).get('paneId')
                    if pane:
                        case['terminal'] = self.command([self.args.herdr, 'pane', 'read', pane, '--lines', '250', '--format', 'text'])
                pane = case['creation']['paneId']
                if agent == 'omp':
                    require((self.root / 'omp-fixed-loaded.txt').read_text() == 'FIXED-LOADED-628', 'Native fixed extension was lost')
                info = self.herdr('pane', 'process-info', '--pane', pane)
                case['nativeProcess'] = info
                self.capture_handles()
                if agent == 'pi':
                    self.command([self.args.herdr, 'pane', 'send-text', pane, '/mcp'])
                    self.command([self.args.herdr, 'pane', 'send-keys', pane, 'enter'])
                    def status():
                        text = self.command([self.args.herdr, 'pane', 'read', pane, '--lines', '250', '--format', 'text'])
                        case['nativeMcpStatus'] = text
                        if self.scenario == 'unreachable':
                            return text if 'unreachable' in text and any(x in text.lower() for x in ['failed', 'error', 'disconnected']) else None
                        return text if 'native_file' in text and 'connected' in text else None
                    until('native /mcp status', status, seconds=self.args.timeout)
                    require(all(name in case['nativeMcpStatus'] for name in case['expectedNativeServerNames']),
                            ('Native MCP status lost a configured server', case['expectedNativeServerNames'], case['nativeMcpStatus']))
                    if self.scenario == 'disabled':
                        require('suite_file' in case['nativeMcpStatus'] and 'disabled' in case['nativeMcpStatus'].lower(),
                                ('Native disabled status missing', case['nativeMcpStatus']))
                    require(self.native_path.read_bytes() == self.native_bytes, 'Native MCP file was modified')
                    project_mcp = [str(p.relative_to(zone)) for p in zone.rglob('*') if p.is_file() and p.name in ['mcp.json', '.mcp.json']]
                    require(project_mcp == [], ('Suite MCP was copied into the project', project_mcp))
                    case['projectMcpFiles'] = project_mcp
                case['mcpWire'] = self.mcp.wire[wire_start:]
                generated = self.root / 'generated resources'
                case['artifacts'] = [{'path': str(p), 'target': str(p.resolve())} if p.is_symlink()
                    else {'path': str(p), 'sha256': digest(p)} for p in generated.rglob('*') if p.is_symlink() or p.is_file()]
                if selected:
                    targets = {item['target'] for item in case['artifacts'] if 'target' in item}
                    require({str(self.skill.resolve()), str((self.authority / 'relative skill').resolve())} <= targets,
                            ('Final true-link skill artifacts missing', case['artifacts']))
                    require(str(self.extension.resolve()) not in targets,
                            ('Extension package must load from its source entry, not a generated link', case['artifacts']))
                    case['sourceEntrypoint'] = str((self.extension / 'extension/probe.ts').resolve())
                case['verified'] = True
                self.command([self.args.herdr, 'pane', 'close', pane])
            finally:
                if lock:
                    kernel.CloseHandle(lock)
        if self.scenario not in ['unreadable-skill', 'unreadable-extension', 'generation-failure', 'incompatible']:
            pi_wire = [item for case in self.evidence['cases'] if case['agent'] == 'pi' for item in case['mcpWire']]
            init = Counter(item['route'] for item in pi_wire if item['request']['method'] == 'initialize')
            calls = Counter(item['route'] for item in pi_wire if item['request']['method'] == 'tools/call')
            # Approved P20 preserves each host's native rules, not OMP's URL dedup policy in Pi.
            expected = {'native': 1}
            if selected:
                expected = {'native': 2, 'suite': 2}
            elif self.scenario == 'append':
                expected = {'native': 1, 'suite': 1}
            elif self.scenario == 'equivalent':
                expected = {'native': 2}
            require(dict(init) == expected, ('Independent MCP connection counts', expected, dict(init)))
            require(dict(calls) == expected, ('Independent MCP call counts', expected, dict(calls)))
            self.evidence.update(expectedConnections=expected, actualConnections=dict(init), actualCalls=dict(calls))
        for path, sha in self.original.items():
            if self.scenario == 'incompatible' and path.endswith('suites.json'):
                continue
            require(digest(Path(path)) == sha, ('Source changed during assembly', path))
        self.evidence['passed'] = True

    def capture_handles(self):
        # OS observation only. Match this scenario's generated argv, never user Chat state.
        escaped = str(self.root).replace("'", "''")
        script = f"Get-CimInstance Win32_Process | Where-Object {{$_.CommandLine -and $_.CommandLine.Contains('{escaped}') -and $_.Name -in @('node.exe','bun.exe')}} | Select-Object ProcessId,Name,CommandLine | ConvertTo-Json -Compress"
        text = self.command(['powershell.exe', '-NoProfile', '-Command', script])
        rows = json.loads(text) if text.strip() else []
        if isinstance(rows, dict):
            rows = [rows]
        require(rows, 'No owned native agent process observed')
        for row in rows:
            if row['ProcessId'] not in [handle.pid for handle in self.handles]:
                self.handles.append(support['LiveProcess'](row['ProcessId']))
        self.evidence.setdefault('nativeProcesses', []).extend(rows)

    def cleanup_extra(self):
        errors = []
        for handle in self.handles:
            try:
                until('owned native agent exit', lambda: not handle.alive(), seconds=20)
                self.evidence['processExits'].append({'pid': handle.pid, 'exited': True})
            except BaseException:
                errors.append(traceback.format_exc())
            finally:
                handle.close()
        if self.mcp:
            self.evidence['mcpWire'] = self.mcp.wire
            self.mcp.shutdown()
            self.mcp.server_close()
        if hasattr(self, 'unreachable'):
            self.unreachable.close()
        histories = []
        for path in (self.root / 'agent').rglob('*.jsonl'):
            histories.append({'path': str(path), 'sha256': digest(path), 'content': path.read_text(encoding='utf-8')})
        self.evidence['nativeHistory'] = histories
        if self.evidence['passed'] and any(c['agent'] == 'pi' and c.get('consumed') for c in self.evidence['cases']):
            require(histories, 'Native Pi history was not retained')
        require(not errors, errors)


def preflight(args, evidence):
    require(args.repo == REPO and Path.cwd().resolve() == REPO, 'Run from the explicitly supplied herdr-bot worktree')
    require((args.toolkit / '.git').is_file(), 'toolkit-worktree must be an explicit Git worktree, not an installed fallback')
    depfile = args.backend.with_suffix('.d')
    require(args.backend.name == 'herdr-bot.exe' and depfile.is_file(), 'backend provenance: executable needs its Cargo .d identity')
    deps = depfile.read_text(encoding='utf-8').replace('\\', '/')
    require(str(args.repo / 'backend/src/main.rs').replace('\\', '/') in deps,
            'backend provenance: Cargo depfile does not name this worktree source')
    source = list((args.repo / 'backend/src').rglob('*.rs')) + [args.repo / 'backend/Cargo.toml', args.repo / 'backend/Cargo.lock']
    require(all(p.stat().st_mtime <= args.backend.stat().st_mtime for p in source), 'backend provenance: binary predates current source; build this worktree')
    resolver = args.backend.parent / 'herdr-paths.exe'
    require(resolver.is_file(), 'Build this worktree herdr-paths binary beside --backend; no source/deployment fallback')
    roots = json.loads(command([resolver, '--paths', args.paths_file], args.repo))
    for key in ['herdr', 'pi_package', 'omp_package']:
        require(key in roots, f'Isolated dependency config requires roots.{key}')
    args.herdr = Path(roots['herdr']) / 'herdr.exe'
    args.pi_package = Path(roots['pi_package'])
    args.omp_package = Path(roots['omp_package'])
    required = [args.herdr, args.pi_package / 'dist/bundle/cli.js', args.omp_package / 'dist/cli.js',
                args.toolkit / 'entrypoints/pi/pi-suite.ts', args.repo / 'backend/static/index.html']
    for path in required:
        require(path.is_file(), ('Missing real dependency/artifact', str(path)))
    launchers = {}
    for name, package in [('pi', args.pi_package), ('omp', args.omp_package)]:
        path = Path(shutil.which(name) or '')
        require(path.is_file(), f'Installed {name} must be on PATH; runner does not install or replace it')
        text = path.read_text(encoding='utf-8').replace('\\', '/')
        package_name = json.loads((package / 'package.json').read_text())['name']
        require(package_name in text, ('PATH launcher package differs', name, path))
        # npm launchers resolve node_modules from their own directory. Check actual package bytes.
        installed = path.parent / 'node_modules' / package_name
        require(installed.resolve() == package.resolve(), ('PATH launcher does not load declared package', installed, package))
        launchers[name] = {'path': str(path), 'sha256': digest(path), 'version': json.loads((package / 'package.json').read_text())['version']}
    evidence['launchers'] = launchers
    evidence['host'] = {name: command([name, '--version'], args.repo).strip() for name in ['node', 'bun', 'python']}
    evidence['herdrVersion'] = command([args.herdr, '--version'], args.repo)
    evidence['identity'] = {}
    for name, root in [('herdr-bot', args.repo), ('toolkit', args.toolkit)]:
        tracked = command(['git', '-C', root, 'ls-files'], root).splitlines()
        evidence['identity'][name] = {'root': str(root), 'revision': command(['git', '-C', root, 'rev-parse', 'HEAD'], root).strip(),
            'diff': command(['git', '-C', root, 'diff', '--', '.', ':!.scratch'], root),
            'sha256': {name: digest(root / name) for name in tracked if (root / name).is_file() and not name.startswith('.scratch/')}}
    frontend_sources = [p for p in (args.repo / 'frontend/src').rglob('*') if p.is_file()]
    frontend_sources += [args.repo / 'frontend/package.json', args.repo / 'frontend/package-lock.json', args.repo / 'frontend/vite.config.ts']
    require(all(p.stat().st_mtime <= (args.repo / 'backend/static/index.html').stat().st_mtime for p in frontend_sources),
            'Frontend output predates current source; build the supplied worktree')
    evidence['frontend'] = {str(p.relative_to(args.repo)): digest(p) for p in (args.repo / 'backend/static').rglob('*') if p.is_file()}
    evidence['binary'] = {'path': str(args.backend), 'sha256': digest(args.backend), 'depfile': deps,
                          'sourceHashes': {str(p.relative_to(args.repo)): digest(p) for p in source}}
    evidence['paths'] = {'file': str(args.paths_file), 'sha256': digest(args.paths_file), 'roots': roots}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['herdr-bot-worktree', 'toolkit-worktree', 'backend', 'paths-file', 'evidence-dir']:
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--scenario', choices=['all', *SCENARIOS], default='all')
    parser.add_argument('--timeout', type=int, choices=range(1, 301), default=90)
    args = parser.parse_args()
    args.repo, args.toolkit = args.herdr_bot_worktree.resolve(), args.toolkit_worktree.resolve()
    args.backend, args.paths_file, args.evidence_dir = args.backend.resolve(), args.paths_file.resolve(), args.evidence_dir.resolve()
    args.evidence_dir.mkdir(parents=True, exist_ok=False)
    selected = list(SCENARIOS) if args.scenario == 'all' else [args.scenario]
    evidence = {'passed': False, 'coverage': {item: {'verified': False, 'scenario': scenario} for scenario in selected for item in SCENARIOS[scenario]},
                'startedProcesses': [], 'scenarios': [], 'cleanupErrors': [], 'argv': sys.argv[1:]}
    try:
        preflight(args, evidence)
        for scenario in selected:
            run = Acceptance(args, scenario)
            result = run.run()
            evidence['startedProcesses'].extend({'pid': p.child.pid, 'exit': p.child.poll()} for p in run.processes)
            evidence['scenarios'].append({'scenario': scenario, 'exit': result, 'evidence': str(run.root / 'evidence.json')})
            evidence['cleanupErrors'].extend(run.evidence['cleanupErrors'])
            for item in SCENARIOS[scenario]:
                evidence['coverage'][item].update(verified=result == 0, evidence=str(run.root / 'evidence.json'))
        evidence['passed'] = all(item['verified'] for item in evidence['coverage'].values()) and not evidence['cleanupErrors']
    except BaseException:
        evidence['error'] = traceback.format_exc()
    (args.evidence_dir / 'evidence.json').write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps({'passed': evidence['passed'], 'evidence': str(args.evidence_dir / 'evidence.json')}, indent=2))
    return 0 if evidence['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
