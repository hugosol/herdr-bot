"""P5/P6/P7/P8/P10/P15: selected skills consumed by installed OMP via Herdr.

Approved seams S2/S3/S4. A controlled loopback model asks the REAL read tool
for skill:// resources and calls a REAL native project MCP tool in the same
turn. It checks literal bytes, the exact advertised skill set, native project
instructions and omission of Suite templates. It does not use SDK assembly,
source CLI, user credentials, a replacement manager or a probe-only API.
"""
import argparse
import http.server
import hashlib
import json
import os
from pathlib import Path
import re
import runpy
import shutil
import sys
import threading
import traceback

from developer_paths import REPO, backend_suffix

support = runpy.run_path(str(Path(__file__).with_name('test-real-herdr-close.py')))
require, until = support['require'], support['until']


class Model(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_POST(self):
        request = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
        case = self.server.case
        case['requests'].append(request)
        try:
            messages = request['messages']
            if len(messages) == 1 and messages[0]['role'] == 'user' and str(messages[0].get('content', '')).startswith('Compress into one routing hint'):
                delta, finish = {'role': 'assistant', 'content': 'Use for owned resource checks.'}, 'stop'
            elif not request.get('tools') and any(str(m.get('content', '')).startswith(('You name coding-agent sessions.', 'You head coding-agent session titles')) for m in messages):
                delta, finish = {'role': 'assistant', 'content': '<title>Selected skill resources</title>'}, 'stop'
            elif not any(m['role'] == 'tool' for m in messages):
                system = '\n'.join(str(m.get('content', '')) for m in messages if m['role'] in ['system', 'developer'])
                tables = re.findall(r'<skills>\s*(.*?)</skills>', system, re.S)
                names = re.findall(r'^- ([^:]+):', '\n'.join(tables), re.M)
                expected = case['expectedNames']
                require(sorted(names) == expected, ('Exact native skill set', names, system))
                require('NATIVE-INSTRUCTIONS-KEEP-732' in system, 'Native project instructions disappeared')
                require('SUITE-TEMPLATE-MUST-NOT-INJECT-684' not in json.dumps(messages), 'Suite template was injected')
                tools = [x['function']['name'] for x in request.get('tools', [])]
                require('xd://mcp__native_probe' in system and 'write' in tools,
                        ('Native mounted MCP tool must coexist with skill filtering', tools, system))
                require('read' in tools, ('Native read tool missing', tools))
                paths = [] if case['scenario'] == 'empty' else ['skill://selected-root', 'skill://selected-root/scripts/probe.py',
                            'skill://selected-root/assets/payload.txt', 'skill://selected-relative/references/note.md',
                            'skill://literal*name']
                if case['scenario'] == 'selected':
                    paths += [str(Path(root) / 'SKILL.md') for root in case['originalSkillRoots']]
                    if 'new-source-skill' in expected:
                        paths.append('skill://new-source-skill')
                case['consumedResourcePaths'] = paths
                calls = [dict(id=f'read{i}', type='function', function=dict(name='read', arguments=json.dumps({'i':'Reading selected resource', 'path': path})))
                         for i, path in enumerate(paths)]
                calls.append(dict(id='native-call', type='function', function=dict(name='write', arguments=json.dumps({
                    'i':'Calling native MCP probe', 'path':'xd://mcp__native_probe', 'content':'{}'}))))
                delta = {'role': 'assistant', 'tool_calls': [{'index': i, **call} for i, call in enumerate(calls)]}
                finish = 'tool_calls'
            else:
                outputs = '\n'.join(str(m.get('content', '')) for m in messages if m['role'] == 'tool')
                expected_outputs = ['NATIVE-MCP-CALLED-361']
                if case['scenario'] == 'selected':
                    expected_outputs += ['ROOT-SKILL-BODY-119', 'print("SCRIPT-READ-947")', 'ASSET-BYTES-276', 'REFERENCE-BYTES-508', 'LITERAL-GLOB-SKILL-913']
                    if 'new-source-skill' in case['expectedNames']:
                        expected_outputs.append('new-source-skill')
                for literal in expected_outputs:
                    require(literal in outputs, ('Real resource/tool result missing', literal, outputs))
                case['passed'] = True
                delta, finish = {'role': 'assistant', 'content': 'SUITE-SKILLS-VERIFIED-803'}, 'stop'
        except BaseException:
            case.setdefault('error', traceback.format_exc())
            delta, finish = {'role': 'assistant', 'content': 'SUITE-SKILLS-ASSERTION-FAILED'}, 'stop'
        self.send_response(200)
        self.send_header('Content-Type', 'text/event-stream')
        self.end_headers()
        for item, reason in [(delta, None), ({}, finish)]:
            chunk = {'id': 'owned', 'object': 'chat.completion.chunk', 'created': 1, 'model': 'suite-probe',
                     'choices': [{'index': 0, 'delta': item, 'finish_reason': reason}]}
            self.wfile.write(('data: ' + json.dumps(chunk) + '\n\n').encode())
        self.wfile.write(b'data: [DONE]\n\n')


class Acceptance(support['Acceptance']):
    def __init__(self, args):
        super().__init__(args, prefix='herdr-suite-skills-')
        self.evidence['scenario'] = args.scenario
        self.evidence['promise'] = ['P15'] if args.scenario == 'empty' else ['P5', 'P6', 'P7', 'P8', 'P10', 'P15']
        self.evidence['cases'] = []
        self.mcp_handles = []
        self.mcp_pids = set()
        self.env = {key: value for key, value in self.env.items() if not any(
            term in key.upper() for term in ['SECRET', 'PASSWORD', 'PROXY', 'CREDENTIAL'])}

    def prepare(self):
        require(sys.platform == 'win32', 'This runner requires Windows real Herdr and process handles')
        native_entry = self.args.omp_package / 'dist/cli.js'
        for path in [self.args.herdr, self.args.backend, native_entry]:
            require(path.is_file(), ('Missing real dependency', str(path)))
        require(shutil.which('omp') is not None, 'Installed omp must already be on PATH; runner never changes global PATH')
        launcher = Path(shutil.which('omp'))
        self.evidence['source'] = {'repo': str(REPO), 'revision': self.command(['git', '-C', REPO, 'rev-parse', 'HEAD']),
                                   'diff': self.command(['git', '-C', REPO, 'diff', '--', 'backend', 'scripts', 'docs', 'herdr-bot.toml'])}
        sources = ['backend/src/suites/skills.rs', 'backend/src/suites.rs', 'backend/src/files.rs',
            'backend/src/tasks/mod.rs', 'backend/src/tasks/catalog.rs', 'backend/src/config.rs',
            'backend/src/lib.rs', 'backend/src/chats/mod.rs', 'backend/src/chats/resume.rs',
            'backend/src/runs.rs', 'backend/src/runs/startup.rs', 'backend/Cargo.lock',
            'scripts/test-real-herdr-suite-skills.py']
        self.evidence['source']['sha256'] = {name:hashlib.sha256((REPO / name).read_bytes()).hexdigest() for name in sources}
        launcher_text = launcher.read_text(encoding='utf-8').replace('\\', '/')
        require('@oh-my-pi/pi-coding-agent/dist/cli.js' in launcher_text,
                ('PATH omp must launch the packaged dist CLI, not a source CLI or replacement', str(launcher)))
        self.evidence['executables'] = {str(path):hashlib.sha256(path.read_bytes()).hexdigest()
            for path in [self.args.backend, self.args.herdr, launcher, native_entry]}
        if self.args.omp_package:
            self.evidence['ompPackage'] = json.loads((self.args.omp_package / 'package.json').read_text())['version']
        self.model = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Model)
        threading.Thread(target=self.model.serve_forever, daemon=True).start()
        agent = self.root / 'agent'
        (agent / 'config.yml').write_text('autoResume: false\nstartup:\n  setupWizard: false\n  showSplash: false\n  checkUpdate: false\nmarketplace:\n  autoUpdate: off\n')
        (agent / 'models.yml').write_text(json.dumps({'providers': {'suite-local': {
            'baseUrl': f'http://127.0.0.1:{self.model.server_port}/v1', 'api': 'openai-completions',
            'apiKey': 'owned-loopback-not-a-credential', 'models': [{'id': 'suite-probe', 'name': 'Suite probe',
            'reasoning': False, 'input': ['text'], 'contextWindow': 32768, 'maxTokens': 2048,
            'cost': {'input': 0, 'output': 0, 'cacheRead': 0, 'cacheWrite': 0}}]}}}))
        # Different directory and native name intentionally exercise entity reuse.
        self.authority = self.root / 'declarations with spaces'
        self.authority.mkdir()
        self.collection = self.root / 'central skills with spaces'
        root_skill = self.collection / 'existing directory'
        relative_skill = self.authority / 'relative skill directory'
        for directory in [root_skill / 'scripts', root_skill / 'assets', relative_skill / 'references']:
            directory.mkdir(parents=True, exist_ok=True)
        (root_skill / 'SKILL.md').write_text('---\nname: selected-root\ndescription: Root-selected test skill\n---\nROOT-SKILL-BODY-119\n')
        (root_skill / 'scripts/probe.py').write_text('print("SCRIPT-READ-947")\n')
        (root_skill / 'assets/payload.txt').write_text('ASSET-BYTES-276\n')
        (relative_skill / 'SKILL.md').write_text('---\nname: selected-relative\ndescription: Relative-selected test skill\n---\nRelative body\n')
        (relative_skill / 'references/note.md').write_text('REFERENCE-BYTES-508\n')
        literal_skill = self.collection / 'literal glob skill'
        literal_skill.mkdir()
        (literal_skill / 'SKILL.md').write_text('---\nname: literal*name\ndescription: Literal glob name\n---\nLITERAL-GLOB-SKILL-913\n')
        self.expected_targets = {root_skill.resolve(), relative_skill.resolve(), literal_skill.resolve()}
        self.source_bytes = {path: path.read_bytes() for source in self.expected_targets for path in source.rglob('*') if path.is_file()}
        (self.authority / 'template.md').write_text('SUITE-TEMPLATE-MUST-NOT-INJECT-684\n')
        (self.authority / 'suites.json').write_text(json.dumps({'suites':[{'id':'selected', 'name':'Selected', 'agents':['omp'],
            'skills':{'omp': [] if self.args.scenario == 'empty' else ['${roots.owned_skills}', '.'], 'pi': []},
            'extensions': {'omp': [str(self.root / 'native plugin with spaces' / 'extension.ts')]},
            'prompts':['template.md']}]}))
        (self.root / 'paths.toml').write_text('[roots]\nowned_skills = ' + json.dumps(str(self.collection)) + '\n')
        # A native global default must NOT leak into the Suite skill set.
        self.sentinel(agent / 'skills/global-default', 'global-default')
        self.sentinel(agent / 'skills/shadow-default', 'selected-root')
        self.sentinel(agent / 'managed-skills/managed-default', 'managed-default')
        self.sentinel(agent / 'managed-skills/wildcard-leak', 'literalXname')
        plugin = self.root / 'native plugin with spaces'
        plugin.mkdir()
        (plugin / 'package.json').write_text(json.dumps({'name': 'owned-sentinel-plugin',
            'omp': {'extensions': ['./extension.ts']}}))
        (plugin / 'extension.ts').write_text('export default function extension(pi) { pi.on("session_start", async () => { '
            'await Bun.write(' + json.dumps(str(self.root / 'plugin-loaded.txt')) + ', "PLUGIN-LOADED-851"); }); }\n')
        self.sentinel(plugin / 'skills/plugin-default', 'plugin-default')
        self.sentinel(plugin / 'skills/shadow-selected', 'selected-root')
        configured_skills = self.root / 'unselected custom skills'
        self.sentinel(configured_skills / 'configured-default', 'configured-default')
        self.sentinel(configured_skills / 'shadow-selected', 'selected-root')
        with (agent / 'config.yml').open('a', encoding='utf-8') as config:
            config.write('extensions: ' + json.dumps([str(plugin)]) + '\n')
            config.write('skills: ' + json.dumps({'customDirectories': [str(configured_skills)],
                'includeSkills': ['configured-default'], 'ignoredSkills': ['selected-root']}) + '\n')
        self.evidence['excludedNativeSources'] = {
            'user': str(agent / 'skills'), 'managed': str(agent / 'managed-skills'),
            'plugin': str(plugin / 'skills'), 'pluginConfiguration': str(agent / 'config.yml')}
        mcp = self.root / 'native-mcp.cjs'
        mcp.write_text("""const fs = require('node:fs');
const readline = require('node:readline');
const log = value => fs.appendFileSync(process.argv[2], JSON.stringify(value)+'\\n');
log({event:'start',pid:process.pid});
const reply = (id, result) => process.stdout.write(JSON.stringify({jsonrpc:'2.0',id,result})+'\\n');
readline.createInterface({input:process.stdin}).on('line', line => {
 const m=JSON.parse(line); log(m);
 if (m.method==='initialize') reply(m.id,{protocolVersion:'2024-11-05',capabilities:{tools:{}},serverInfo:{name:'owned-native',version:'1'}});
 else if (m.method==='tools/list') reply(m.id,{tools:[{name:'probe',description:'Owned native coexistence probe',inputSchema:{type:'object',properties:{}}}]});
 else if (m.method==='tools/call') reply(m.id,{content:[{type:'text',text:'NATIVE-MCP-CALLED-361'}]});
 else if (m.id!==undefined) reply(m.id,{});
});
""")
        self.zones = [self.root / 'zone', self.root / 'different Chat cwd with spaces']
        self.zones[1].mkdir()
        for index, zone in enumerate(self.zones):
            self.sentinel(zone / '.omp/skills/project-default', 'project-default')
            (zone / 'AGENTS.md').write_text('NATIVE-INSTRUCTIONS-KEEP-732\n')
            (zone / '.omp/mcp.json').write_text(json.dumps({'mcpServers': {'native': {
                'command': str(self.args.node), 'args': [str(mcp), str(self.root / f'mcp-{index}.jsonl')]}}}))
        herdr_config = self.root / 'herdr.toml'
        herdr_config.write_text('onboarding = false\n[terminal]\ndefault_shell = "cmd.exe"\n[update]\nversion_check = false\nmanifest_check = false\n')
        self.env['HERDR_CONFIG_PATH'] = str(herdr_config)
        self.evidence['herdrVersion'] = self.command([self.args.herdr, '--version'])
        self.start([self.args.herdr, 'server'], 'herdr')
        self.herdr_started = True
        def ready():
            result = __import__('subprocess').run([str(self.args.herdr), 'workspace', 'list'], cwd=self.root,
                env=self.env, capture_output=True, timeout=5)
            return result.returncode == 0
        until('owned Herdr server', ready)
        self.evidence['ownedWorkspace'] = self.herdr('workspace', 'create', '--cwd', str(self.zones[0]), '--label', 'suite-skills', '--no-focus')
        q = lambda value: json.dumps(str(value))
        fixed = self.root / 'fixed.ts'
        fixed.write_text('export default function(pi) { pi.on("session_start", async () => { await Bun.write('
            + json.dumps(str(self.root / 'fixed-loaded.txt')) + ', "FIXED-LOADED-628"); }); }')
        (self.root / 'options.toml').write_text('skills = []\n[[agents]]\nid = "omp"\nlabel = "OMP"\n'
            '[[models]]\nid = "default"\nlabel = "Default"\n[[models]]\nid = "probe"\nlabel = "Probe"\nprovider_model = "suite-local/suite-probe"\nthinking = { omp = ["off"] }\n'
            '[[extensions]]\nid = "owned-fixed"\nlabel = "Owned fixed"\nagent = "omp"\nkind = "fixed"\nentrypoint = ' + q(fixed) + '\n')
        static = self.root / 'static'
        static.mkdir()
        (static / 'index.html').write_text('<!doctype html><title>S2/S4 only; no UI assertion</title>')
        config = 'listen_addr = "127.0.0.1:0"\n'
        config += f'static_directory = {q(static)}\ndata_directory = {q(self.root / "data")}\npaths_file = {q(self.root / "paths.toml")}\n'
        config += '[mdns]\nenabled = false\n'
        for name in ['start', 'stop']:
            config += f'[operations.{name}]\nscript = {q(self.root / "unused.bat")}\nworking_directory = {q(self.root)}\n'
        config += f'[herdr]\nworkspace_name = "suite-skills"\nexecutable = {q(self.args.herdr)}\n'
        config += f'[tasks]\nstorage_file = {q(self.root / "data/tasks.json")}\nmetadata_file = {q(self.root / "options.toml")}\n'
        config += f'[suites]\ndefinition_file = {q(self.authority / "suites.json")}\nmcp_file = {q(self.authority / "mcp.json")}\ngenerated_directory = {q(self.root / "generated resources with spaces")}\n'
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

    @staticmethod
    def sentinel(directory, name):
        directory.mkdir(parents=True, exist_ok=True)
        (directory / 'SKILL.md').write_text(f'---\nname: {name}\ndescription: Undeclared sentinel\n---\nNot selected\n')

    def exercise(self):
        saved = self.http('/api/tasks', {'name':'Suite exact skills', 'chatName':'{task.name}-{chat.id}',
            'agent':'omp', 'taskType':'normal', 'model':'probe', 'thinking':'off', 'skill':None,
            'prompt':'Keep native instructions.', 'suite':'selected'})
        task_id = saved['tasks'][0]['id']
        for index, zone in enumerate(self.zones):
            if index:
                self.sentinel(self.collection / 'new source skill', 'new-source-skill')
            names = [] if self.args.scenario == 'empty' else sorted(
                ['literal*name', 'selected-relative', 'selected-root'] + (['new-source-skill'] if index else []))
            case = {'scenario':self.args.scenario, 'cwd':str(zone), 'requests':[], 'passed':False,
                    'expectedNames': names, 'sourceParents': [str(self.collection.resolve()), str(self.authority.resolve())],
                    'originalSkillRoots': sorted(str(path) for path in self.expected_targets)}
            self.evidence['cases'].append(case)
            self.model.case = case
            self.http('/api/zones', {'id':f'owned-{index}', 'path':str(zone)})
            generated = self.root / 'generated resources with spaces'
            prior_configs = {path: path.read_bytes() for path in generated.rglob('omp-skills.json')}
            preview = self.http('/api/chats/preview', {'zone':f'owned-{index}', 'taskId':task_id})
            launch = self.http('/api/chats', {'zone':f'owned-{index}', 'taskId':task_id,
                'chatId':preview['chatId'], 'input':'Read the selected resources and call the native MCP probe.'}, status=202)
            def result():
                self.observe_mcp()
                current = self.http('/api/chats/' + launch['id'])
                case['latestCreation'] = current
                if (current['stage'] == 'succeeded' or case.get('error')) and 'nativeProcesses' not in case:
                    owned = str(self.root).replace("'", "''")
                    script = ("Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -like '*" + owned +
                        "*' } | Select-Object ProcessId,ParentProcessId,ExecutablePath,CommandLine | ConvertTo-Json -Compress")
                    observed = self.command(['powershell.exe', '-NoProfile', '-Command', script])
                    processes = json.loads(observed or '[]')
                    case['nativeProcesses'] = processes if isinstance(processes, list) else [processes]
                require(current['stage'] != 'failed', current)
                require(not case.get('error'), case.get('error'))
                return current if case['passed'] and current['stage'] == 'succeeded' else None
            try:
                case['creation'] = until('real native resource reads and MCP call', result, seconds=self.args.timeout)
            except BaseException:
                pane = case.get('latestCreation', {}).get('paneId')
                if pane:
                    try:
                        case['terminal'] = self.command([self.args.herdr, 'pane', 'read', pane, '--lines', '150', '--format', 'text'])
                    except BaseException:
                        case['diagnosticError'] = traceback.format_exc()
                raise
            wire = [json.loads(line) for line in (self.root / f'mcp-{index}.jsonl').read_text().splitlines()]
            require(any(x.get('method') == 'tools/call' for x in wire), 'No real MCP tools/call')
            case['mcpWire'] = wire
            require((self.root / 'fixed-loaded.txt').read_text() == 'FIXED-LOADED-628', 'Fixed extension not loaded')
            require((self.root / 'plugin-loaded.txt').read_text() == 'PLUGIN-LOADED-851',
                    'Configured native plugin extension did not load; its excluded skill source was not exercised')
            case['pluginExtensionLoaded'] = True
            generated = self.root / 'generated resources with spaces'
            links = [path for path in generated.rglob('*') if path.is_symlink()]
            require(not links, ('Skills must load from original sources without generated links', [str(x) for x in links]))
            for path, original in prior_configs.items():
                require(path.read_bytes() == original, ('Earlier Chat config was rewritten', str(path)))
            configs = [json.loads(path.read_text(encoding='utf-8')) for path in generated.rglob('omp-skills.json')
                       if path not in prior_configs]
            skill_configs = [config['skills'] for config in configs if isinstance(config, dict) and isinstance(config.get('skills'), dict)]
            require(skill_configs, ('Missing generated native OMP skills config', configs))
            expected_parents = set(case['sourceParents']) if names else set()
            for config in skill_configs:
                require(config.get('enabled') is bool(names), config)
                parents = [Path(path) for path in config.get('customDirectories', [])]
                require(all(any(path.samefile(expected) for expected in expected_parents) for path in parents)
                        and all(any(path.samefile(expected) for path in parents) for expected in expected_parents), config)
            case['nativeSkillConfigs'] = skill_configs
            for path, expected_bytes in self.source_bytes.items():
                require(path.read_bytes() == expected_bytes, ('Source resource changed', str(path)))
        self.evidence['passed'] = True

    def observe_mcp(self):
        for path in self.root.glob('mcp-*.jsonl'):
            for line in path.read_text().splitlines():
                entry = json.loads(line)
                if entry.get('event') == 'start' and entry['pid'] not in self.mcp_pids:
                    self.mcp_handles.append(support['LiveProcess'](entry['pid']))
                    self.mcp_pids.add(entry['pid'])

    def cleanup_extra(self):
        for handle in self.mcp_handles:
            try:
                until('owned native MCP process exit', lambda: not handle.alive(), seconds=15)
            finally:
                handle.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--herdr', type=Path, required=True)
    parser.add_argument('--omp-package', type=Path, required=True)
    parser.add_argument('--backend', type=Path, default=REPO / backend_suffix())
    parser.add_argument('--node', type=Path, default=Path(shutil.which('node') or 'node'))
    parser.add_argument('--timeout', type=int, choices=range(1, 301), default=120)
    parser.add_argument('--scenario', choices=['selected', 'empty'], default='selected',
                        help='empty proves native MCP + exact empty skill set + template omission, NOT selected resource consumption')
    args = parser.parse_args()
    args.repo = REPO
    return Acceptance(args).run()


if __name__ == '__main__':
    sys.exit(main())
