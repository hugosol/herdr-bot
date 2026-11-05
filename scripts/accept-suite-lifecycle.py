"""P12/P13/P28-P32/P35 at S2 HTTP, S3 deployment and S4 real Herdr hosts.

Only the external model is controlled. Native tools, extension factories, auth
stores, session managers, launch/resume and workflow association remain real.
The observation extension uses documented native events/read-only APIs; it does
not replace a product extension, modify host methods, or create a product API.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
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
import time
import traceback
import urllib.error
import urllib.request
import uuid

REPO = Path(__file__).resolve().parent.parent
resources = runpy.run_path(str(REPO / 'scripts/accept-suite-pi-resources.py'))
support = resources['support']
require, until, digest = support['require'], support['until'], resources['digest']
PROMISES = {
    'P12': 'Both native hosts retain actual fixed model-cost and workflow behavior with/without Suite.',
    'P13': 'Standalone has no node role; manager/node and resumed execution retain their original association.',
    'P28': 'Overlapping native launches authenticate from one test-owned native store; concurrent native writes survive.',
    'P29': 'Native model configuration, native history directory and conversation identity are independently read unchanged.',
    'P30': 'Literal model/thinking/initial Skill/fixed message/input/name survive Suite selection.',
    'P31': 'After Task/Suite edits and backend restart, resume loads latest environment without replay or identity replacement; absent/deleted source uses native discovery.',
    'P32': 'Saving Suite does not actively prompt/restart/reconfigure an existing Chat; later launch consumes new definition.',
    'P35': 'Same/different Suite real concurrent launches consume complete independent sentinels while a first native start is held.',
}
SCENARIOS = {
    'baseline': ['P12', 'P13', 'P29', 'P30'],
    'suite-lifecycle': ['P12', 'P28', 'P29', 'P30', 'P31', 'P32'],
    'roles-native': ['P12', 'P13', 'P31'],
    'roles-suite': ['P12', 'P13', 'P31'],
    'native-resume': ['P13', 'P29', 'P31'],
    'auth-native': ['P28'],
    'concurrency': ['P28', 'P35'],
}
FIXED = 'LIFECYCLE-FIXED-172'
INITIAL = 'LIFECYCLE-INITIAL-SKILL-463'
INPUT = 'LIFECYCLE-INPUT-298'
ACK = 'LIFECYCLE-NATIVE-ACK-851'
SENTINELS = {'a': ('LIFECYCLE-SKILL-A-314', 'LIFECYCLE-ASSET-A-817'),
             'b': ('LIFECYCLE-SKILL-B-926', 'LIFECYCLE-ASSET-B-532'),
             'c': ('LIFECYCLE-SKILL-C-675', 'LIFECYCLE-ASSET-C-249')}


def text(content):
    if isinstance(content, str):
        return content
    return ''.join(p.get('text', '') for p in content if p.get('type') == 'text')


def user_entries(branch):
    # Native OMP persists an explicitly invoked Skill as a user-attributed
    # custom_message; Pi uses an ordinary user message. Both are user input.
    return [entry for entry in branch if entry.get('message', {}).get('role') == 'user'
            or (entry['type'] == 'custom_message' and entry.get('attribution') == 'user')]


class Model(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        owner = self.server.owner
        if self.path == '/gate':
            case = owner.cases[body['case']]
            case['gateObservations'].append(body)
            # The native process awaits this response: acquire its real handle
            # before it can exit, including later validation/startup failures.
            owner.remember_process(body['pid'])
            case['entered'].set()
            if case.get('hold') and not case['release'].wait(owner.args.timeout):
                case['error'] = 'Held native start was not released within the observation deadline'
            self.send_response(500 if case.get('error') else 200)
            self.end_headers()
            self.wfile.write(b'{}')
            return
        routing = '\n'.join(text(m.get('content', '')) for m in body.get('messages', []) if m['role'] == 'user')
        # Only submitted user markers route a turn. A real run-status result may
        # contain another Chat's input marker and must not change its recipient.
        positions = [(routing.rfind('CASE:' + key), value) for key, value in owner.cases.items()]
        position, case = max(positions, key=lambda item: item[0])
        delta, finish = {'role': 'assistant', 'content': '<title>Lifecycle probe</title>'}, 'stop'
        if body.get('tools') and position >= 0:
            case['requests'].append(body)
            try:
                require(self.headers.get('Authorization') == 'Bearer ' + owner.key,
                        'Native request did not authenticate from the single test-owned native store')
                case['authMatched'] = True  # Never retain headers or credential bytes.
                if case.get('authOverlap') and not case.get('authSynchronized'):
                    case['authSynchronized'] = True
                    case['authArrivedAt'] = time.monotonic()
                    case['authEntered'].set()
                    owner.auth_barrier.wait(timeout=owner.args.timeout)
                    case['authReleasedAt'] = time.monotonic()
                require(body['model'] == 'lifecycle-probe', ('Native model changed', body['model']))
                messages = body['messages']
                users = '\n'.join(text(m.get('content', '')) for m in messages if m['role'] == 'user')
                system = '\n'.join(str(m.get('content', '')) for m in messages if m['role'] in ['system', 'developer'])
                # Restrict results to this explicit turn, not successful tools in older history.
                latest = max(i for i, m in enumerate(messages) if m['role'] == 'user')
                outputs = '\n'.join(str(m.get('content', '')) for m in messages[latest + 1:] if m['role'] == 'tool')
                if not outputs:
                    if case['trigger']:
                        require(FIXED in users and INPUT in users and INITIAL in users,
                                ('Initial Skill/message not consumed', users))
                    calls = []
                    if case.get('nativeFallback'):
                        calls.append(owner.tool(case['agent'], 'native_discovery_sentinel', {}))
                    if case['suite']:
                        letter = case['suite']
                        if case['agent'] == 'pi':
                            advertised = {name: html.unescape(path) for name, path in re.findall(
                                r'<name>(.*?)</name>\s*<description>.*?</description>\s*<location>(.*?)</location>', system, re.S)}
                            require(sorted(advertised) == sorted(['lifecycle-initial', 'lifecycle-' + letter]),
                                    ('Incomplete or mixed advertised native Skills', advertised))
                            skill = advertised['lifecycle-' + letter]
                            expected = (owner.authority / ('skills-' + letter) / ('skill-' + letter) / 'SKILL.md').resolve()
                            require(Path(skill).resolve() == expected, ('Native Skill must use its original source', skill, str(expected)))
                            case['resourcePaths'] = advertised
                        else:
                            tables = re.findall(r'<skills>\s*(.*?)</skills>', system, re.S)
                            names = re.findall(r'^- ([^:]+):', '\n'.join(tables), re.M)
                            require(sorted(names) == sorted(['lifecycle-initial', 'lifecycle-' + letter]),
                                    ('Incomplete or mixed advertised native Skills', names))
                            skill = 'skill://lifecycle-' + letter
                            case['resourcePaths'] = [skill]
                        calls.append(('read', {'path': skill, **({'i': 'Reading lifecycle sentinel'} if case['agent'] == 'omp' else {})}))
                        calls.append(owner.tool(case['agent'], 'lifecycle_asset', {}))
                    if case['role'] == 'manager':
                        calls.append(owner.tool(case['agent'], 'run', {'action': 'status'}))
                    if case['role'] == 'node':
                        calls.append(owner.tool(case['agent'], 'report', {'body': 'LIFECYCLE-REPORT-637', 'can_complete': False}))
                    if calls:
                        delta = {'role': 'assistant', 'tool_calls': [dict(index=i, id='lifecycle-' + str(i), type='function',
                            function={'name': name, 'arguments': json.dumps(args)}) for i, (name, args) in enumerate(calls)]}
                        finish = 'tool_calls'
                    else:
                        case['consumed'] = True
                        delta = {'role': 'assistant', 'content': ACK}
                else:
                    if case.get('nativeFallback'):
                        require('NATIVE-DISCOVERY-ONLY-784' in outputs, ('Native default discovery was not consumed', outputs))
                    if case['suite']:
                        for literal in SENTINELS[case['suite']]:
                            require(literal in outputs, ('Incomplete or mixed native resource set', literal, outputs))
                        require(not any(literal in outputs for letter, literals in SENTINELS.items()
                                        if letter != case['suite'] for literal in literals), ('Mixed native resource output', outputs))
                    if case.get('readOnlyExpected'):
                        code = 'manager-unassociated' if case['role'] == 'manager' else 'node-unassociated'
                        require(code in outputs, ('Restarted Run unexpectedly reacquired authority', code, outputs))
                        case['readOnlyRejection'] = code
                    elif case['role'] == 'manager':
                        require('snapshot' in outputs and 'nodes' in outputs, ('Native run status did not execute', outputs))
                    elif case['role'] == 'node':
                        require('LIFECYCLE-REPORT-637' in outputs and 'executionId' in outputs,
                                ('Native report did not reach its associated execution', outputs))
                    case['toolOutput'] = outputs
                    case['consumed'] = True
                    delta = {'role': 'assistant', 'content': ACK}
            except BaseException:
                case.setdefault('error', traceback.format_exc())
                delta, finish = {'role': 'assistant', 'content': 'LIFECYCLE-ASSERTION-FAILED'}, 'stop'
            case['responses'].append({'delta': delta, 'finish': finish})
        self.send_response(200)
        self.send_header('Content-Type', 'text/event-stream')
        self.end_headers()
        for item, reason in [(delta, None), ({}, finish)]:
            chunk = {'id': 'lifecycle', 'object': 'chat.completion.chunk', 'created': 1, 'model': 'lifecycle-probe',
                     'choices': [{'index': 0, 'delta': item, 'finish_reason': reason}]}
            self.wfile.write(('data: ' + json.dumps(chunk) + '\n\n').encode())
        self.wfile.write(b'data: [DONE]\n\n')


class Acceptance(support['Acceptance']):
    def __init__(self, args, agent, scenario):
        super().__init__(args, prefix=str(args.evidence_dir / (agent + '-' + scenario + '-')))
        self.agent, self.scenario = agent, scenario
        self.state_extension = getattr(args, agent + '_state_extension')
        self.cases, self.handles = {}, {}
        self.handles_lock = threading.Lock()
        self.key = uuid.uuid4().hex + uuid.uuid4().hex
        self.env = {k: v for k, v in self.env.items() if not any(term in k.upper() for term in ['KEY', 'AUTH', 'SECRET', 'PASSWORD', 'PROXY', 'CREDENTIAL', 'AWS_', 'GOOGLE', 'GEMINI', 'CODEX'])}
        self.evidence.update(promise=SCENARIOS[scenario], host=agent, scenario=scenario, cases=[], expectations=PROMISES)
        self.env['LIFECYCLE_AUTH_SEED'] = self.key

    @staticmethod
    def tool(agent, name, args):
        return (name, args) if agent == 'pi' else ('write', {'i': 'Observing lifecycle behavior', 'path': 'xd://' + name, 'content': json.dumps(args)})

    def request(self, method, path, body=None, status=200):
        request = urllib.request.Request(self.base + path, method=method,
            data=None if body is None else json.dumps(body).encode(), headers={'Content-Type': 'application/json'})
        try:
            response = urllib.request.urlopen(request, timeout=self.args.timeout + 10)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            raw = response.read().decode()
            self.evidence.setdefault('http', []).append({'method': method, 'path': path, 'request': body, 'status': response.status, 'result': raw})
            require(response.status == status, self.evidence['http'][-1])
            return json.loads(raw)

    def http(self, path, body=None, status=200):
        return self.request('GET' if body is None else 'POST', path, body, status)

    def prepare(self):
        require(sys.platform == 'win32', 'Real lifecycle acceptance requires Windows process handles')
        self.model = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Model)
        self.model.owner = self
        threading.Thread(target=self.model.serve_forever, daemon=True).start()
        self.model_url = f'http://127.0.0.1:{self.model.server_port}'
        agent = self.root / 'agent'
        (agent / 'settings.json').write_text(json.dumps({'autoUpdate': False, 'sessionDir': str(agent / 'sessions'),
            'defaultTools': ['+codemode'], 'codemode': {'mode': 'on'}}), encoding='utf-8')
        (agent / 'config.yml').write_text('autoResume: false\nstartup:\n  setupWizard: false\n  showSplash: false\n  checkUpdate: false\nmarketplace:\n  autoUpdate: off\n', encoding='utf-8')
        # No model apiKey: actual requests must resolve the native auth store.
        model = {'providers': {'lifecycle-owned': {'baseUrl': self.model_url + '/v1', 'api': 'openai-completions',
            'models': [{'id': 'lifecycle-probe', 'name': 'Lifecycle native model', 'reasoning': True,
                'input': ['text'], 'contextWindow': 32768, 'maxTokens': 2048,
                'cost': {'input': 0, 'output': 0, 'cacheRead': 0, 'cacheWrite': 0}}]}}}
        (agent / 'models.json').write_text(json.dumps(model), encoding='utf-8')
        # OMP requires an explicit auth mode for custom providers without a
        # config apiKey. Its native resolver still prefers stored credentials
        # over the anonymous sentinel; the endpoint independently requires our
        # exact native-store key. This never supplies a key through models.yml.
        omp_model = json.loads(json.dumps(model))
        omp_model['providers']['lifecycle-owned']['auth'] = 'none'
        (agent / 'models.yml').write_text(json.dumps(omp_model), encoding='utf-8')
        self.model_files = {str(agent / name): digest(agent / name) for name in ['models.json', 'models.yml']}
        self.write_auth_helper()
        self.command(['bun', self.root / 'native-auth.ts', 'seed'])
        del self.env['LIFECYCLE_AUTH_SEED']
        self.authority = self.root / 'declarations with spaces'
        self.authority.mkdir()
        initial = agent / 'skills/lifecycle-initial'
        initial.mkdir(parents=True)
        (initial / 'SKILL.md').write_text('---\nname: lifecycle-initial\ndescription: Owned initial instruction\n---\n' + INITIAL + '\n', encoding='utf-8')
        self.suites = {}
        for letter in SENTINELS:
            skill = self.authority / ('skills-' + letter) / ('skill-' + letter)
            skill.mkdir(parents=True)
            (skill / 'SKILL.md').write_text(f'---\nname: lifecycle-{letter}\ndescription: Owned complete sentinel\n---\n{SENTINELS[letter][0]}\n', encoding='utf-8')
            package = self.authority / ('package-' + letter)
            package.mkdir()
            (package / 'asset.txt').write_text(SENTINELS[letter][1], encoding='utf-8')
            (package / 'package.json').write_text(json.dumps({'type': 'module'}), encoding='utf-8')
            (package / 'entry.ts').write_text('''import {readFileSync} from "node:fs";
export default function(pi) { pi.registerTool({name:"lifecycle_asset",label:"Lifecycle asset",description:"Read complete native package asset",
 parameters:pi.zod ? pi.zod.object({}) : {type:"object",properties:{}},async execute(){return {content:[{type:"text",text:readFileSync(new URL("asset.txt",import.meta.url),"utf8")}],details:{}};}}); }
''', encoding='utf-8')
            self.suites[letter] = {'id': letter, 'name': 'Lifecycle ' + letter, 'agents': ['pi', 'omp'],
                'skills': {'omp': [str(initial.parent), 'skills-' + letter], 'pi': [str(initial), 'skills-' + letter + '/skill-' + letter]}, 'extensions': {'pi': ['package-' + letter + '/entry.ts'], 'omp': ['package-' + letter + '/entry.ts']},
                'prompts': [], 'lsp': {}, 'mcp': [], 'pi': {'codemode': 'on'}}
        (self.authority / 'suites.json').write_text(json.dumps({'suites': list(self.suites.values())}), encoding='utf-8')
        (self.authority / 'mcp.json').write_text('{"mcpServers":{}}', encoding='utf-8')
        # A byte-identical deployment of the supplied worktree, except its explicit
        # test-owned endpoint configuration. Never copy runtime access records/auth.
        self.deployment = self.root / 'toolkit'
        for name in ['adapters', 'framework', 'extensions', 'entrypoints']:
            shutil.copytree(self.args.toolkit / name, self.deployment / name,
                            ignore=shutil.ignore_patterns('.runtime', 'node_modules', '.git'))
        shutil.copy2(self.args.toolkit / 'package.json', self.deployment / 'package.json')
        shutil.copytree(self.args.toolkit / 'node_modules/yaml', self.deployment / 'node_modules/yaml')
        self.evidence['toolkitDeployment'] = {'source': str(self.args.toolkit), 'root': str(self.deployment),
            'stage': 'source-copy-before-owned-endpoint-override',
            'sha256': {str(p.relative_to(self.deployment)): digest(p) for folder in ['adapters', 'framework', 'extensions', 'entrypoints']
                for p in (self.deployment / folder).rglob('*') if p.is_file()}}
        self.write_observer()
        self.write_options()
        q = lambda value: json.dumps(str(value))
        herdr_config = self.root / 'herdr.toml'
        herdr_config.write_text('onboarding = false\n[terminal]\ndefault_shell = "cmd.exe"\n[update]\nversion_check = false\nmanifest_check = false\n', encoding='utf-8')
        self.env['HERDR_CONFIG_PATH'] = str(herdr_config)
        self.start([self.args.herdr, 'server'], 'herdr')
        self.herdr_started = True
        until('exclusive named Herdr', lambda: subprocess.run([str(self.args.herdr), 'workspace', 'list'], cwd=self.root,
            env=self.env, capture_output=True, timeout=5).returncode == 0)
        self.evidence['workspace'] = self.herdr('workspace', 'create', '--cwd', str(self.root / 'zone'), '--label', 'lifecycle', '--no-focus')
        config = f'listen_addr = "127.0.0.1:0"\nstatic_directory = {q(REPO / "backend/static")}\ndata_directory = {q(self.root / "data")}\n[mdns]\nenabled = false\n'
        for name in ['start', 'stop']:
            config += f'[operations.{name}]\nscript = {q(self.root / "unused.bat")}\nworking_directory = {q(self.root)}\n'
        config += f'[herdr]\nworkspace_name = "lifecycle"\nexecutable = {q(self.args.herdr)}\n[tasks]\nstorage_file = {q(self.root / "data/tasks.json")}\nmetadata_file = {q(self.root / "options.toml")}\n'
        config += f'[suites]\ndefinition_file = {q(self.authority / "suites.json")}\nmcp_file = {q(self.authority / "mcp.json")}\ngenerated_directory = {q(self.root / "generated resources")}\n'
        config += f'[sessions]\nhistory_file = {q(self.root / "data/history.json")}\n[zones]\nregistry_file = {q(self.root / "data/zones.json")}\n[scheduler]\n'
        config += ''.join(f'{key} = {q(value)}\n' for key, value in [('map_directory', self.root / 'maps'), ('bot_directory', self.root / 'bots'),
            ('run_directory', self.root / 'data/runs'), ('run_instructions', REPO / 'docs/workflow-runtime.md')])
        (self.root / 'backend.toml').write_text(config, encoding='utf-8')
        self.start_backend()

    def write_auth_helper(self):
        pi_module = (self.args.pi_package / 'dist/core/auth-storage.js').as_uri()
        omp_module = (self.args.omp_package / 'node_modules/@oh-my-pi/pi-ai/src/auth-storage.ts').as_uri()
        dirs_module = (self.args.omp_package / 'node_modules/@oh-my-pi/pi-utils/src/dirs.ts').as_uri()
        # Native public storage APIs, not raw SQLite edits or copied credentials.
        source = f'''import {{writeFileSync,existsSync}} from "node:fs";
import {{AuthStorage}} from {json.dumps(pi_module if self.agent == 'pi' else omp_module)};
'''
        source += ('const path=process.env.PI_CODING_AGENT_DIR+"/auth.json"; const store=AuthStorage.create(path);\n' if self.agent == 'pi' else
            f'import {{getAgentDbPath}} from {json.dumps(dirs_module)};\nconst path=getAgentDbPath(); const store=await AuthStorage.create(path); await store.credentials.reload();\n')
        source += '''const action=process.argv[2];
const provider=action==="seed" ? "lifecycle-owned" : "lifecycle-writer-"+action;
const key=process.env.LIFECYCLE_AUTH_SEED;
if(action==="left" || action==="right") {
 const root=process.env.PI_CODING_AGENT_DIR;
 writeFileSync(root+"/writer-"+action+".ready",String(process.pid));
 const deadline=Date.now()+15000;
 while(!existsSync(root+"/writers.release")) {if(Date.now()>deadline) throw new Error("Owned writer barrier timed out"); await new Promise(resolve=>setTimeout(resolve,10));}
}
'''
        source += ('if(action!=="list") await store.modify(provider,async()=>({type:"api_key",key:key??"test-owned-writer-"+action}));\nconsole.log(JSON.stringify({path,metadata:await store.list()}));\n' if self.agent == 'pi' else
            'if(action!=="list") await store.credentials.set(provider,{type:"api_key",key:key??"test-owned-writer-"+action});\nawait store.credentials.reload();\nconsole.log(JSON.stringify({path,providers:Object.keys(store.credentials.all())})); store.close();\n')
        (self.root / 'native-auth.ts').write_text(source, encoding='utf-8')

    def write_observer(self):
        # Observer-only fixed extension; real model-cost/roles remain in exercised path.
        source = '''import {appendFileSync,readFileSync} from "node:fs";
export default function(pi) {
 const observe=(event,ctx)=> { const c=JSON.parse(readFileSync(ctx.cwd+"/.lifecycle-case.json","utf8"));
 const record={event:event.type,case:c.case,pid:process.pid,cwd:ctx.cwd,id:ctx.sessionManager.getSessionId(),file:ctx.sessionManager.getSessionFile(),
  model:ctx.model && {id:ctx.model.id,provider:ctx.model.provider},thinking:pi.getThinkingLevel(),tools:pi.getAllTools().map(t=>t.name),commands:pi.getCommands().map(c=>c.name),prompt:event.prompt};
 appendFileSync(c.events,JSON.stringify(record)+"\\n"); return c; };
 pi.on("session_start",async(event,ctx)=> {const c=observe(event,ctx); const response=await fetch(c.gate,{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({case:c.case,id:ctx.sessionManager.getSessionId(),pid:process.pid})}); if(!response.ok) throw new Error("Owned gate failed");});
 pi.on("before_agent_start",(event,ctx)=>{observe(event,ctx);});
 pi.on("agent_end",(event,ctx)=>{observe(event,ctx);});
}
'''
        self.observer = self.root / 'native-observer.ts'
        self.observer.write_text(source, encoding='utf-8')
        # Native fallback discovery sees an independent ambient observer, not Suite.
        ambient = self.root / 'agent/extensions'
        ambient.mkdir()
        ambient_source = source.replace('export default function(pi) {', '''export default function(pi) {
 pi.registerTool({name:"native_discovery_sentinel",label:"Native discovery",description:"Owned ambient-only native tool",
 parameters:pi.zod ? pi.zod.object({}) : {type:"object",properties:{}},async execute(){return {content:[{type:"text",text:"NATIVE-DISCOVERY-ONLY-784"}],details:{}};}});
''')
        (ambient / 'observer.ts').write_text(ambient_source, encoding='utf-8')
        shutil.copy2(self.state_extension, ambient / 'herdr-state.ts')

    def write_options(self):
        q = lambda path: json.dumps(str(path))
        options = f'[[agents]]\nid = "{self.agent}"\nlabel = "{self.agent}"\n[[models]]\nid = "default"\nlabel = "Default"\n[[models]]\nid = "probe"\nlabel = "Probe"\nprovider_model = "lifecycle-owned/lifecycle-probe"\nthinking = {{ {self.agent} = ["high"] }}\n[[skills]]\nid = "initial"\nlabel = "Initial"\nnative_name = "lifecycle-initial"\n'
        entries = [('acceptance-observer', self.observer),
                   (self.agent + '-herdr-agent-state', self.state_extension),
                   (self.agent + '-model-cost', self.deployment / f'entrypoints/{self.agent}/model-cost.ts'),
                   (self.agent + '-manager-chat', self.deployment / f'entrypoints/{self.agent}-auto/manager-chat.ts'),
                   (self.agent + '-node-chat', self.deployment / f'entrypoints/{self.agent}-auto/node-chat.ts')]
        if self.agent == 'pi':
            entries.append(('pi-suite', self.deployment / 'entrypoints/pi/pi-suite.ts'))
        for name, path in entries:
            options += f'[[extensions]]\nid = "{name}"\nlabel = "{name}"\nagent = "{self.agent}"\nkind = "fixed"\nentrypoint = {q(path)}\n'
        (self.root / 'options.toml').write_text(options, encoding='utf-8')

    def start_backend(self):
        self.backend = self.start([self.args.backend, '--config', self.root / 'backend.toml'], 'backend-' + str(len(self.processes)))
        def address():
            require(self.backend.child.poll() is None, 'Owned backend exited; see drained logs')
            while not self.backend.lines.empty():
                match = re.search(r'http://(127\.0\.0\.1:\d+)/', self.backend.lines.get_nowait())
                if match:
                    return 'http://' + match[1]
        self.base = until('backend dynamic loopback', address)
        (self.deployment / 'extensions/workflow-chat/config.yaml').write_text(
            f'endpoint: {self.base}\nrequestTimeoutMs: 30000\ncollectTimeoutMs: 5460000\n', encoding='utf-8')
        config = self.deployment / 'extensions/workflow-chat/config.yaml'
        self.evidence.setdefault('deploymentOverrides', []).append({'path': str(config), 'sha256': digest(config),
            'content': config.read_text(encoding='utf-8')})

    def new_case(self, name, suite=None, role='normal', trigger=False, hold=False, cwd=None):
        key = uuid.uuid4().hex
        case = {'case': key, 'name': name, 'agent': self.agent, 'suite': suite, 'role': role, 'trigger': trigger,
            'hold': hold, 'requests': [], 'responses': [], 'gateObservations': [], 'entered': threading.Event(),
            'release': threading.Event(), 'authEntered': threading.Event(), 'consumed': False}
        self.cases[key] = case
        self.evidence['cases'].append(case)
        zone = cwd or self.root / ('zone-' + key)
        zone.mkdir(exist_ok=True)
        case['zone'] = str(zone)
        (zone / '.lifecycle-case.json').write_text(json.dumps({'case': key, 'events': str(self.root / 'events.jsonl'), 'gate': self.model_url + '/gate'}), encoding='utf-8')
        # A resumed/continued turn keeps its already registered stable zone.
        # Registering this cwd under a second ID makes persisted Run discovery
        # ambiguous and would test an invalid fixture instead of Chat recovery.
        if cwd is None:
            self.http('/api/zones', {'id': key, 'path': str(zone)})
        return case

    def save_task(self, case):
        body = {'name': case['name'], 'agent': self.agent, 'taskType': 'manager' if case['role'] == 'manager' else 'normal',
            'chatName': 'Lifecycle {chat.id}', 'model': 'probe', 'thinking': 'high',
            'skill': 'initial' if case['trigger'] else None, 'prompt': FIXED if case['trigger'] else '', 'suite': case['suite']}
        saved = self.http('/api/tasks', body)
        case['task'] = next(t for t in saved['tasks'] if t['name'] == case['name'])
        return case['task']

    def launch(self, case, task=None):
        task = task or self.save_task(case)
        case['task'] = task
        preview = self.http('/api/chats/preview', {'zone': case['case'], 'taskId': task['id']})
        expected = 'Lifecycle ' + preview['chatId']
        require(preview['name'] == expected, ('Literal Chat name changed', preview))
        case['expectedName'] = expected
        case['launch'] = self.http('/api/chats', {'zone': case['case'], 'taskId': task['id'], 'chatId': preview['chatId'],
            'input': (INPUT + '\n' if case['trigger'] else '') + 'CASE:' + case['case']}, 202)
        return case

    def events(self, case):
        file = self.root / 'events.jsonl'
        return [item for line in file.read_text(encoding='utf-8').splitlines() if (item := json.loads(line))['case'] == case['case']] if file.exists() else []

    def remember_process(self, pid):
        with self.handles_lock:
            old = self.handles.get(pid)
            if old and not old.alive():
                self.evidence.setdefault('retiredProcessExits', []).append({'pid': pid, 'exited': True})
                old.close()
                del self.handles[pid]
            if pid not in self.handles:
                self.handles[pid] = support['LiveProcess'](pid)

    def capture(self):
        # Only OS descendants of this invocation's named Herdr server; no user cmdlines.
        pid = self.processes[0].child.pid
        script = (f'$owned=[System.Collections.Generic.HashSet[int]]::new();[void]$owned.Add({pid});'
            '$tree=@(Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,CreationDate);'
            '$by=@{};foreach($p in $tree){$by[[int]$p.ProcessId]=$p};'
            'do {$added=$false;foreach($p in $tree){if($owned.Contains([int]$p.ParentProcessId) '
            '-and $by.ContainsKey([int]$p.ParentProcessId) '
            '-and $p.CreationDate -ge ($by[[int]$p.ParentProcessId]).CreationDate '
            '-and $owned.Add([int]$p.ProcessId)){$added=$true}}}while($added);'
            # Only runtimes this deterministic fixture starts are ours. OS input
            # method/desktop services are not owned even when a console triggers them.
            '$names=@("herdr.exe","cmd.exe","powershell.exe","node.exe","bun.exe","OpenConsole.exe","conhost.exe");'
            '$r=@(Get-CimInstance Win32_Process | Where-Object {$owned.Contains([int]$_.ProcessId) '
            '-and $_.Name -in $names -and $_.CreationDate -eq ($by[[int]$_.ProcessId]).CreationDate} '
            '| Select-Object ProcessId,ParentProcessId,CreationDate,CommandLine);ConvertTo-Json -InputObject $r -Compress')
        snapshot = json.loads(self.command(['powershell.exe', '-NoProfile', '-Command', script]))
        self.evidence.setdefault('ownedSnapshots', []).append(snapshot)
        for item in snapshot:
            self.remember_process(item['ProcessId'])

    def history(self, file):
        package = self.args.pi_package if self.agent == 'pi' else self.args.omp_package
        module = package / ('dist/core/session-manager.js' if self.agent == 'pi' else 'src/session/session-manager.ts')
        code = f'import {{SessionManager}} from {json.dumps(module.as_uri())};const s=await SessionManager.open({json.dumps(str(file))});console.log(JSON.stringify({{id:s.getSessionId(),name:s.getSessionName(),branch:s.getBranch()}}));'
        return json.loads(self.command(['bun', '-e', code]))

    def wait_case(self, case, creation=True):
        def ready():
            require(not case.get('error'), case.get('error'))
            if creation:
                case['creation'] = self.http('/api/chats/' + case['launch']['id'])
                require(case['creation']['stage'] != 'failed', case['creation'])
            events = self.events(case)
            ends = [e for e in events if e['event'] == 'agent_end']
            return ends[-1] if case['consumed'] and ends and (not creation or case['creation']['stage'] == 'succeeded') else None
        observation = until(case['name'] + ' real native turn', ready, seconds=self.args.timeout)
        case['observation'] = observation
        self.capture()
        require(observation['model'] == {'provider': 'lifecycle-owned', 'id': 'lifecycle-probe'}, ('Independent native model read', observation))
        require(observation['thinking'] == 'high', ('Independent native thinking read', observation))
        tools = set(observation['tools'])
        expected_role_tools = {'run', 'collect'} if case['role'] == 'manager' else {'report'} if case['role'] == 'node' else set()
        require(tools & {'run', 'collect', 'report'} == expected_role_tools,
                ('Wrong native execution role', expected_role_tools, tools))
        require(('native_discovery_sentinel' in tools) == bool(case.get('nativeFallback')),
                'Native-discovery and fixed-only boundaries were conflated')
        if case.get('nativeFallback'):
            require('lifecycle_asset' not in tools, 'Native fallback guessed an unrelated Suite')
        else:
            require('budget' in observation['commands'], ('Real fixed model-cost did not register its public command', observation))
        file = Path(observation['file'])
        require(file.is_relative_to(self.root / 'agent/sessions') and file.is_file(), ('History moved outside native store', str(file)))
        native = self.history(file)
        require(native['id'] == observation['id'], ('Native SessionManager identity mismatch', native, observation))
        require(any(ACK in json.dumps(e, ensure_ascii=False) for e in native['branch']), 'Native assistant response not persisted')
        case['history'] = native
        case['historyFile'] = str(file)
        case['historySha256'] = digest(file)
        for path, expected in self.model_files.items():
            require(digest(Path(path)) == expected, ('Suite rewrote native models', path))
        case['modelConfiguration'] = {path: json.loads(Path(path).read_text(encoding='utf-8')) for path in self.model_files}
        return case

    def close_case(self, case):
        pane = case.get('creation', {}).get('paneId') or case.get('chat', {}).get('paneId')
        require(pane, ('Missing owned pane', case['name']))
        self.capture()
        pid = case['observation']['pid']
        self.herdr('pane', 'close', pane)
        until('owned native process exits before resume', lambda: not self.handles[pid].alive(), seconds=20)
        for observed in self.cases.values():
            if observed.get('observation', {}).get('pid') == pid:
                observed['nativeExitConfirmed'] = True

    def resume(self, old, suite, role=None, fallback=False, readonly=False):
        case = self.new_case(old['name'] + '-resumed', suite, role or old['role'], cwd=Path(old['zone']))
        case['nativeFallback'] = fallback
        case['readOnlyExpected'] = readonly
        before = self.history(old['historyFile'])
        record = self.http('/api/chats/resume', {'sessionId': before['id']})
        case['resume'] = record
        require(record['outcome'] == 'restored', record)
        case['chat'] = record['target']
        require(case['entered'].wait(self.args.timeout), 'Resumed observer did not run')
        starts = self.events(case)
        require(starts and starts[-1]['id'] == before['id'], 'Resume changed original identity')
        require(self.history(old['historyFile'])['branch'] == before['branch'], 'Resume replayed trigger or submitted a message')
        if fallback:
            require(any(item['element'] == 'extensions' for item in record['degradations']), 'Missing native-discovery degradation')
        self.command([self.args.herdr, 'agent', 'prompt', case['chat']['paneId'], 'CASE:' + case['case']], seconds=self.args.timeout + 10)
        self.wait_case(case, creation=False)
        require(case['history']['id'] == before['id'] and case['historyFile'] == old['historyFile'], 'Native history identity/location changed')
        require(case['history'].get('name') == before.get('name'), 'Resume reapplied changed Task naming')
        before_users = user_entries(before['branch'])
        after_users = user_entries(case['history']['branch'])
        require(len(after_users) == len(before_users) + 1, 'Trigger replay during resume')
        require(after_users[:len(before_users)] == before_users, 'Resume rewrote existing user history')
        return case

    def budget(self, case):
        pane = case.get('creation', {}).get('paneId') or case['chat']['paneId']
        requests = len(case['requests'])
        self.command([self.args.herdr, 'agent', 'prompt', pane, '/budget help'], seconds=self.args.timeout + 10)
        def rendered():
            terminal = self.command([self.args.herdr, 'pane', 'read', pane, '--lines', '200', '--format', 'text'])
            return terminal if all(word in terminal for word in ['detail', 'holiday', 'clear', '归档并重置统计']) else None
        case['budgetHelpTerminal'] = until('real fixed budget help rendering', rendered, seconds=self.args.timeout)
        require(len(case['requests']) == requests, 'Fixed budget help became an ordinary model prompt')

    def baseline(self):
        case = self.wait_case(self.launch(self.new_case('standalone-no-suite', trigger=True)))
        self.check_trigger(case)
        self.budget(case)

    def check_trigger(self, case):
        users = '\n'.join(text(e.get('message', e).get('content', '')) for e in user_entries(case['history']['branch']))
        if self.agent == 'omp':
            invoked = [e for e in user_entries(case['history']['branch']) if e.get('customType') == 'skill-prompt']
            require(len(invoked) == 1 and invoked[0]['details']['name'] == 'lifecycle-initial', 'Initial native Skill invocation changed')
            require(invoked[0]['details']['prompt'] == '/skill:lifecycle-initial ' + FIXED + '\n\n' + INPUT + '\nCASE:' + case['case'],
                    'Native initial Skill arguments differ from the literal submission')
        require(users.count(FIXED) == 1 and users.count(INPUT) == 1 and INITIAL in users, ('Literal trigger/history mismatch', users))
        require(case['creation']['name'] == case['expectedName'], 'Suite replaced Chat naming')

    def suite_lifecycle(self):
        # Independent trigger/role oracles: Suite never supplies these values.
        normal = self.wait_case(self.launch(self.new_case('standalone-no-suite', trigger=True)))
        selected = self.wait_case(self.launch(self.new_case('standalone-suite', 'a', trigger=True)))
        for case in [normal, selected]:
            self.check_trigger(case)
            self.budget(case)
        require(normal['authMatched'] and selected['authMatched'], 'Suite changed shared native authentication')
        # Save a new immutable fixture selection through the real Suite API while
        # a Chat remains alive. No file-content freeze/native-reload promise.
        before_events = self.events(selected)
        before_requests = len(selected['requests'])
        changed = dict(self.suites['a'], skills=self.suites['b']['skills'], extensions=self.suites['b']['extensions'])
        self.request('PUT', '/api/suites/a', changed)
        saved = self.http('/api/suites/a')['suite']
        require(saved['skills'] == self.suites['b']['skills'], 'Suite save did not persist')
        # Read barriers and a bounded quiet interval bracket product save;
        # native host reload is not disabled and resource bytes are not frozen.
        quiet_started = time.monotonic()
        self.http('/api/tasks')
        time.sleep(1.25)
        require(len(selected['requests']) == before_requests, 'Suite save actively prompted existing Chat')
        after_events = self.events(selected)
        quiet_seconds = time.monotonic() - quiet_started
        require([e for e in after_events[len(before_events):] if e['event'] == 'before_agent_start'] == [], 'Suite save actively restarted a turn')
        require(self.handles[selected['observation']['pid']].alive(), 'Suite save replaced the native process')
        live = self.new_case('existing-after-suite-save', 'a', cwd=Path(selected['zone']))
        live['chat'] = selected['creation']
        self.command([self.args.herdr, 'agent', 'prompt', selected['creation']['paneId'], 'CASE:' + live['case']], seconds=self.args.timeout + 10)
        self.wait_case(live, creation=False)
        require(live['observation']['pid'] == selected['observation']['pid'], 'Save actively replaced existing process')
        later = self.new_case('after-suite-save', 'b')
        self.save_task(later)
        body = dict(later['task'], suite='a')
        body.pop('id', None)
        self.request('PUT', '/api/tasks/' + later['task']['id'], body)
        # Explicit Suite a now references the independent B sentinels.
        preview = self.http('/api/chats/preview', {'zone': later['case'], 'taskId': later['task']['id']})
        later['launch'] = self.http('/api/chats', {'zone': later['case'], 'taskId': later['task']['id'], 'chatId': preview['chatId'], 'input': 'CASE:' + later['case']}, 202)
        self.wait_case(later)
        self.evidence['saveObservation'] = {'before': before_events, 'after': after_events, 'saved': saved,
                                           'oldPid': selected['observation']['pid'], 'laterCase': later['case'],
                                           'quietObservedSeconds': quiet_seconds}
        # Change both the source Task and Suite. Trigger changes are deliberately
        # incompatible with original history and must not affect resume.
        edited = dict(selected['task'], suite='b', prompt='MUST-NOT-REPLAY-TRIGGER-955', skill=None, model='default', thinking='default', chatName='MUST-NOT-RENAME')
        edited.pop('id', None)
        self.request('PUT', '/api/tasks/' + selected['task']['id'], edited)
        latest_b = dict(self.suites['b'], skills=self.suites['c']['skills'], extensions=self.suites['c']['extensions'])
        self.request('PUT', '/api/suites/b', latest_b)
        self.close_case(selected)
        self.backend.stop()
        self.start_backend()
        resumed = self.resume(selected, 'c')
        require('MUST-NOT-REPLAY' not in json.dumps(resumed['history']), 'Latest trigger replayed')
        
    def native_resume(self):
        normal = self.wait_case(self.launch(self.new_case('legacy-source', trigger=True)))
        # A same-name Task now selects Suite A; source-less recovery must not
        # infer it from name/cwd/current selection.
        decoy_source = dict(normal['task'], suite='a')
        decoy_source.pop('id', None)
        self.request('PUT', '/api/tasks/' + normal['task']['id'], decoy_source)
        # No source legacy catalog: retain genuine native history, remove only its
        # test-owned source metadata while backend is stopped. No guessed Suite.
        self.close_case(normal)
        self.backend.stop()
        history_file = self.root / 'data/history.json'
        rows = json.loads(history_file.read_text(encoding='utf-8'))
        require(isinstance(rows, list), 'Unexpected public history storage format')
        row = next(row for row in rows if row['sessionId'] == normal['history']['id'])
        row.pop('sourceTaskId', None)
        history_file.write_text(json.dumps(rows), encoding='utf-8')
        self.evidence['legacySourceFixture'] = {'sessionId': row['sessionId'], 'removedField': 'sourceTaskId', 'nativeHistoryUnchanged': digest(Path(normal['historyFile'])) == normal['historySha256']}
        self.start_backend()
        self.resume(normal, None, fallback=True)
        # Actual source deletion, plus a same-name decoy, must not infer Suite.
        deleted = self.wait_case(self.launch(self.new_case('deleted-source')))
        self.close_case(deleted)
        self.request('DELETE', '/api/tasks/' + deleted['task']['id'])
        decoy = self.new_case('deleted-source', 'a')
        self.save_task(decoy)
        self.backend.stop()
        self.start_backend()
        self.resume(deleted, None, fallback=True)

    def roles_native(self):
        self.roles([None])

    def roles_suite(self):
        self.roles(['b'])

    def roles(self, suites):
        for suite in suites:
            manager = self.new_case('manager-' + str(suite), suite, 'manager')
            worker = self.new_case('node-' + str(suite), suite, 'node')
            self.save_task(manager)
            self.save_task(worker)
            for case in [manager, worker]:
                # Managed Runs use the actual zone directory's Workspace name,
                # not the standalone Chat deployment's fallback workspace.
                case['workspace'] = self.herdr('workspace', 'create', '--cwd', case['zone'],
                    '--label', Path(case['zone']).name, '--no-focus')
            standalone = self.new_case('same-task-standalone-' + str(suite), suite)
            self.wait_case(self.launch(standalone, worker['task']))
            require(standalone['task']['id'] == worker['task']['id'], 'Role comparison did not use the same ordinary Task')
            (self.root / 'bots/worker.json').write_text(json.dumps({'id': 'worker', 'name': 'Worker', 'when': '', 'plan_guide': '', 'node_guide': '', 'task_id': worker['task']['id']}), encoding='utf-8')
            (self.root / 'maps/lifecycle.json').write_text(json.dumps({'title': 'Lifecycle', 'description': '', 'nodes': {'A': {
                'title': 'Owned node', 'bot_id': 'worker', 'input': 'CASE:' + worker['case'], 'zone': worker['case'], 'blocked_by': []}}}), encoding='utf-8')
            started = self.http('/api/scheduler/maps/lifecycle.json/runs', {'managerTaskId': manager['task']['id'], 'zone': manager['case'], 'userInput': 'CASE:' + manager['case']}, 201)
            manager['chat'] = started['run']['manager']
            self.wait_case(manager, creation=False)
            dispatched = self.http('/api/workflow/manager/dispatch', {'sessionId': manager['history']['id'], 'nodes': ['A']})
            execution = dispatched['snapshot']['nodes']['A']['execution']
            worker['chat'] = execution['chat']
            self.wait_case(worker, creation=False)
            before = self.http('/api/workflow/manager/status', {'sessionId': manager['history']['id']})
            require(before['snapshot']['nodes']['A']['execution']['id'] == execution['id'], 'Node report changed execution association')
            require(any(report['executionId'] == execution['id'] and report['from']['sessionId'] == worker['history']['id']
                        and report['body'] == 'LIFECYCLE-REPORT-637' for report in before['reports']),
                    'Native node report did not belong to its dispatched execution')
            before_report_ids = {report['id'] for report in before['reports']}
            for case in [manager, worker]:
                self.close_case(case)
            active_manager = self.resume(manager, suite)
            active_worker = self.resume(worker, suite)
            after = self.http('/api/workflow/manager/status', {'sessionId': manager['history']['id']})
            require(after['snapshot']['nodes']['A']['execution']['id'] == execution['id'], 'Resume created a new execution')
            require(after['snapshot']['nodes']['A']['execution']['chat']['sessionId'] == worker['history']['id'], 'Resume lost node native association')
            require(any(report['executionId'] == execution['id'] and report['from']['sessionId'] == worker['history']['id']
                        and report['body'] == 'LIFECYCLE-REPORT-637'
                        and report['id'] not in before_report_ids for report in after['reports']),
                    'Resumed native report lost the original execution association')
            # Existing workflow contract: backend restart retains execution facts
            # read-only; ordinary Chat Resume MUST NOT take over the old Run.
            retained = self.http('/api/workflow/runs/' + started['id'])
            self.close_case(active_manager)
            self.close_case(active_worker)
            self.backend.stop()
            self.start_backend()
            self.resume(active_manager, suite, readonly=True)
            self.resume(active_worker, suite, readonly=True)
            retired = self.http('/api/workflow/runs/' + started['id'])
            require(retired['managed'] is False and retired['run'] == retained['run'],
                    'Backend restart/Chat recovery changed or reclaimed original Run facts')
            require(retired['reports'] == retained['reports'], 'Read-only resumed report was incorrectly persisted')
            self.evidence.setdefault('roleAssociations', []).append({'suite': suite, 'before': before, 'after': after,
                'beforeRestart': retained, 'readOnlyAfterRestart': retired})

    def auth_native(self):
        # Authentication concurrency is observed at the external model boundary,
        # after both real hosts resolve their credentials. Unlike P35 this does
        # not need to hold a startup callback or alter native TUI readiness.
        self.auth_barrier = threading.Barrier(2)
        first = self.new_case('auth-first')
        second = self.new_case('auth-second')
        for case in [first, second]:
            case['authOverlap'] = True
        self.launch(first)
        require(first['authEntered'].wait(self.args.timeout), 'First native authenticated request did not arrive')
        self.launch(second)
        self.wait_case(first)
        self.wait_case(second)
        require(max(case['authArrivedAt'] for case in [first, second]) <= min(case['authReleasedAt'] for case in [first, second]),
                'Native authenticated requests did not actually overlap')
        self.evidence['authenticatedOverlap'] = [{key: case[key] for key in ['case', 'authArrivedAt', 'authReleasedAt', 'authMatched']} for case in [first, second]]
        self.auth_writers()

    def concurrency(self):
        # Keep the first real session_start callback waiting across Herdr's
        # external start operation; the second must reach native consumption.
        for same in [True, False]:
            first = self.new_case('same-first' if same else 'different-first', 'a', hold=True)
            second = self.new_case('same-second' if same else 'different-second', 'a' if same else 'b')
            try:
                self.launch(first)
                require(first['entered'].wait(self.args.timeout), 'First native startup did not reach held public event')
                first['heldCreation'] = self.http('/api/chats/' + first['launch']['id'])
                require(first['heldCreation']['stage'] in ['starting', 'submitting'],
                        'Gate did not hold an external Herdr startup effect; cannot establish no-lock-across-start')
                self.launch(second)
                self.wait_case(second)
                require(not first['release'].is_set(), 'First startup released before second consumption')
                self.evidence.setdefault('overlap', []).append({'sameSuite': same, 'first': first['case'], 'second': second['case'],
                    'firstHeldStage': first['heldCreation']['stage'], 'secondNativeConsumedWhileFirstHeld': True})
            finally:
                first['release'].set()
            self.wait_case(first)
            require(first['authMatched'] and second['authMatched'], 'Concurrent native hosts did not use common test-owned authentication')
        self.auth_writers()

    def auth_writers(self):
        # Concurrent public native credential writers share the same default store
        # used by the launched agents. Separate provider rows expose lost updates.
        with ThreadPoolExecutor(max_workers=2) as pool:
            jobs = [pool.submit(self.command, ['bun', self.root / 'native-auth.ts', name]) for name in ['left', 'right']]
            try:
                until('both native credential writers reach their shared-store barrier',
                      lambda: all((self.root / 'agent' / ('writer-' + name + '.ready')).is_file() for name in ['left', 'right']), seconds=15)
            finally:
                (self.root / 'agent/writers.release').write_text('owned concurrent native writes', encoding='utf-8')
            writes = [json.loads(job.result()) for job in jobs]
        listed = json.loads(self.command(['bun', self.root / 'native-auth.ts', 'list']))
        require(writes[0]['path'] == writes[1]['path'] == listed['path'], 'Native auth coordination split across directories')
        require(Path(listed['path']).is_relative_to(self.root), 'Auth escaped test-owned storage')
        require(not Path(listed['path']).is_symlink(), 'Native auth storage was linked')
        duplicated = [str(p) for p in (self.root / 'generated resources').rglob('*')
                      if p.name == 'auth.json' or p.name.startswith('agent.db')]
        require(duplicated == [], ('Suite generated a separate or copied credential store', duplicated))
        providers = listed['providers'] if 'providers' in listed else [item['providerId'] for item in listed['metadata']]
        require(all(name in providers for name in ['lifecycle-owned', 'lifecycle-writer-left', 'lifecycle-writer-right']), ('Native concurrent credential update lost', providers))
        self.evidence['authCoordination'] = {'writes': writes, 'readback': listed, 'nativeRequestsAuthenticated': True,
            'testOwnedDataOnly': True, 'noCredentialCopyOrLink': True}

    def exercise(self):
        try:
            getattr(self, self.scenario.replace('-', '_'))()
            self.evidence['passed'] = True
        except BaseException:
            self.evidence['scenarioError'] = traceback.format_exc()
            raise
        finally:
            if hasattr(self, 'auth_barrier'):
                self.auth_barrier.abort()
            for case in self.cases.values():
                case['release'].set()
            # Failure paths also capture live native handles before server stop.
            try:
                self.capture()
            except BaseException:
                self.evidence['cleanupErrors'].append(traceback.format_exc())
            for case in self.cases.values():
                pane = case.get('creation', {}).get('paneId') or case.get('chat', {}).get('paneId')
                if pane and not case.get('nativeExitConfirmed'):
                    try:
                        case['terminal'] = self.command([self.args.herdr, 'pane', 'read', pane, '--lines', '200', '--format', 'text'])
                    except BaseException:
                        self.evidence['cleanupErrors'].append(traceback.format_exc())

    def cleanup_extra(self):
        exits = []
        for pid, handle in self.handles.items():
            try:
                until('owned native process exit', lambda: not handle.alive(), seconds=20)
                exits.append({'pid': pid, 'exited': True})
            except BaseException:
                self.evidence['cleanupErrors'].append(traceback.format_exc())
                exits.append({'pid': pid, 'exited': False})
            finally:
                handle.close()
        self.evidence['ownedProcessExits'] = exits
        self.evidence['serviceExits'] = [{'pid': p.child.pid, 'exitCode': p.child.poll()} for p in self.processes]
        for case in self.cases.values():
            case['events'] = self.events(case)
            case.pop('entered')
            case.pop('release')
            case.pop('authEntered')
        # Test credentials never appear in commands, headers, model wire or logs.
        # Remove native credential-bearing storage only after every host exited.
        if all(item['exited'] for item in exits) and all(p.child.poll() is not None for p in self.processes):
            for path in (self.root / 'agent').rglob('*'):
                if path.is_file() and (path.name == 'auth.json' or path.name.startswith('agent.db')):
                    path.unlink()
            self.evidence['testCredentialsRemovedAfterExit'] = True
        for path in self.root.glob('*.log'):
            raw = path.read_text(encoding='utf-8', errors='replace')
            if self.key in raw:
                path.write_text(raw.replace(self.key, '[REDACTED-TEST-CREDENTIAL]'), encoding='utf-8')
                self.evidence['cleanupErrors'].append('Native log contained test credential; redacted, acceptance fails')
        serialized = json.dumps(self.evidence, ensure_ascii=False)
        if self.key in serialized:
            self.evidence = json.loads(serialized.replace(self.key, '[REDACTED-TEST-CREDENTIAL]'))
            self.evidence['cleanupErrors'].append('Credential reached evidence; redacted, acceptance fails')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ['herdr-bot-worktree', 'toolkit-worktree', 'backend', 'paths-file', 'evidence-dir']:
        parser.add_argument('--' + flag, type=lambda value: Path(value).resolve(), required=True)
    parser.add_argument('--scenario', choices=['all', *SCENARIOS], default='all')
    parser.add_argument('--host', choices=['all', 'pi', 'omp'], default='all')
    parser.add_argument('--timeout', type=int, choices=range(1, 301), default=90)
    args = parser.parse_args()
    args.repo, args.toolkit, args.node = args.herdr_bot_worktree, args.toolkit_worktree, 'node'
    args.evidence_dir.mkdir(parents=True, exist_ok=False)
    scenarios = list(SCENARIOS) if args.scenario == 'all' else [args.scenario]
    hosts = ['pi', 'omp'] if args.host == 'all' else [args.host]
    evidence = {'passed': False, 'argv': sys.argv[1:], 'coverage': {p: {'expected': expected, 'verified': False, 'items': []} for p, expected in PROMISES.items()},
        'startedProcesses': [], 'cleanupErrors': [], 'runs': []}
    try:
        evidence['entrypoint'] = {'path': str(Path(__file__).resolve()), 'sha256': digest(Path(__file__))}
        require(args.backend.is_file(), 'Missing current worktree backend: ' + str(args.backend))
        require('CARGO_TARGET_DIR' in os.environ and Path(os.environ['CARGO_TARGET_DIR']).resolve().is_relative_to(REPO),
                'Use a private CARGO_TARGET_DIR within the explicit herdr-bot worktree')
        require(args.backend.is_relative_to(Path(os.environ['CARGO_TARGET_DIR']).resolve()), 'Backend is outside worktree-private CARGO_TARGET_DIR')
        resources['preflight'](args, evidence)
        evidence['stateIntegrations'] = {}
        for host in ['pi', 'omp']:
            key = host + '_state_extension'
            require(key in evidence['paths']['roots'], 'Isolated paths must name readonly roots.' + key)
            path = Path(evidence['paths']['roots'][key])
            require(path.is_file(), ('Missing installed Herdr state extension', str(path)))
            setattr(args, key, path)
            evidence['stateIntegrations'][host] = {'path': str(path), 'sha256': digest(path)}
        for host in hosts:
            for scenario in scenarios:
                run = Acceptance(args, host, scenario)
                result = run.run()
                evidence['runs'].append({'host': host, 'scenario': scenario, 'exit': result, 'evidence': str(run.root / 'evidence.json')})
                evidence['startedProcesses'].extend({'pid': p.child.pid, 'exit': p.child.poll()} for p in run.processes)
                evidence['cleanupErrors'].extend(run.evidence['cleanupErrors'])
                for promise in SCENARIOS[scenario]:
                    evidence['coverage'][promise]['items'].append({'host': host, 'scenario': scenario, 'verified': result == 0, 'evidence': str(run.root / 'evidence.json')})
        for promise, item in evidence['coverage'].items():
            expected_scenarios = [s for s, promises in SCENARIOS.items() if promise in promises]
            item['verified'] = all(any(i['host'] == h and i['scenario'] == s and i['verified'] for i in item['items']) for h in ['pi', 'omp'] for s in expected_scenarios)
        evidence['selectedScenarioSucceeded'] = all(r['exit'] == 0 for r in evidence['runs']) and not evidence['cleanupErrors']
        evidence['passed'] = all(item['verified'] for item in evidence['coverage'].values()) and not evidence['cleanupErrors']
    except BaseException:
        evidence['error'] = traceback.format_exc()
    (args.evidence_dir / 'evidence.json').write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps({'passed': evidence['passed'], 'evidence': str(args.evidence_dir / 'evidence.json')}, indent=2))
    return 0 if evidence['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
