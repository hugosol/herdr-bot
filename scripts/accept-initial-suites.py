"""P11/P42/P44: initial delivered Suites at S2/S3/S4, never manifest-only success."""
import argparse
import http.server
import os
import re
import shutil
import subprocess
import threading
import tomllib
import json
from pathlib import Path
import runpy
import sys
import traceback

REPO = Path(__file__).resolve().parent.parent
resources = runpy.run_path(str(REPO / 'scripts/accept-suite-pi-resources.py'))
require = resources['require']
COVERAGE = ('P11/exploration-omp', 'P11/exploration-pi', 'P11/implementation-omp', 'P42', 'P44')


support = resources['support']
digest, command, until = resources['digest'], resources['command'], resources['until']
lifecycle = runpy.run_path(str(REPO / 'scripts/accept-suite-lifecycle.py'))
# Independent approved names AND source paths, not derived from suites.json or
# the assembler. In particular expert-tranlate is a real directory spelling,
# expert-translate is its native skill name; to-ticket is never an alias.
EXPLORATION = {
    'grill-with-docs': ('matt_pocock_skills', 'engineering/grill-with-docs'),
    'teach': ('matt_pocock_skills', 'productivity/teach'),
    'code-review': ('wayfinder_skills', 'code-review'),
    'wayfinder': ('wayfinder_skills', 'wayfinder'),
    'writing-for-agents': ('matt_pocock_skills', 'productivity/writing-for-agents'),
    'improve-codebase-architecture': ('matt_pocock_skills', 'engineering/improve-codebase-architecture'),
    'expert-translate': ('expert_translate', ''),
    'grilling': ('matt_pocock_skills', 'productivity/grilling'),
    'domain-modeling': ('matt_pocock_skills', 'engineering/domain-modeling'),
    'codebase-design': ('matt_pocock_skills', 'engineering/codebase-design'),
    'research': ('matt_pocock_skills', 'engineering/research'),
    'prototype': ('wayfinder_skills', 'prototype'),
    'lighthouse': ('wayfinder_skills', 'lighthouse'),
    'backtracer': ('wayfinder_skills', 'backtracer'),
    'setup-matt-pocock-skills': ('wayfinder_skills', 'setup-matt-pocock-skills'),
    'to-contract': ('wayfinder_skills', 'to-contract'),
    'to-spec': ('wayfinder_skills', 'to-spec'),
    'to-tickets': ('wayfinder_skills', 'to-tickets'),
}
IMPLEMENTATION = {
    'tdd': ('wayfinder_skills', 'tdd'),
    'to-tickets': ('wayfinder_skills', 'to-tickets'),
    'retro': ('matt_pocock_skills', 'engineering/retro'),
    'resolving-merge-conflicts': ('wayfinder_skills', 'resolving-merge-conflicts'),
    'writing-for-agents': ('matt_pocock_skills', 'productivity/writing-for-agents'),
    'codebase-design': ('matt_pocock_skills', 'engineering/codebase-design'),
    'code-review': ('wayfinder_skills', 'code-review'),
    'setup-matt-pocock-skills': ('wayfinder_skills', 'setup-matt-pocock-skills'),
    'to-contract': ('wayfinder_skills', 'to-contract'),
    'to-spec': ('wayfinder_skills', 'to-spec'),
    'domain-modeling': ('matt_pocock_skills', 'engineering/domain-modeling'),
}
SCENARIOS = {
    'exploration-omp': ('omp', 'codebase-exploration', EXPLORATION),
    'exploration-pi': ('pi', 'codebase-exploration', EXPLORATION),
    'implementation-omp': ('omp', 'code-implementation', IMPLEMENTATION),
}


class NoTaskModel(http.server.BaseHTTPRequestHandler):
    """A command-only check must never ask a model to execute user work."""
    def log_message(self, *_):
        pass

    def do_POST(self):
        self.server.requests.append(json.loads(self.rfile.read(int(self.headers['Content-Length']))))
        self.send_response(503)
        self.end_headers()
        self.wfile.write(b'Native command acceptance must not run a model task')


class Acceptance(support['Acceptance']):
    remember_process = lifecycle['Acceptance'].remember_process
    capture = lifecycle['Acceptance'].capture

    def __init__(self, args, scenario, roots):
        super().__init__(args, prefix=str(args.evidence_dir / (scenario + '-')))
        self.scenario, self.roots = scenario, roots
        self.agent, self.suite_id, self.expected = SCENARIOS[scenario]
        self.handles, self.handles_lock = {}, threading.Lock()
        self.mcp = None
        self.env = {k: v for k, v in self.env.items() if not any(x in k.upper() for x in
            ['KEY', 'AUTH', 'SECRET', 'PASSWORD', 'PROXY', 'CREDENTIAL', 'AWS_', 'GOOGLE', 'GEMINI', 'CODEX'])}
        self.evidence.update(scenario=scenario, host=self.agent, expectedSkills=sorted(self.expected),
            discoveryVerified=False, commandVerified=False, ownedProcessExits=[])

    def prepare(self):
        require(sys.platform == 'win32', 'Installed Herdr acceptance requires Windows process handles')
        require(os.environ.get('HERDR_BOT_MANUAL_SYMLINKS') == '1',
                'Set HERDR_BOT_MANUAL_SYMLINKS=1 to attempt true links; no elevation or copied-resource fallback')
        self.model = http.server.ThreadingHTTPServer(('127.0.0.1', 0), NoTaskModel)
        self.model.requests = []
        threading.Thread(target=self.model.serve_forever, daemon=True).start()
        # Only the remote MCP service is controlled. The delivered definition
        # still selects codebase-memory-mcp, and native loading remains real.
        self.mcp = http.server.ThreadingHTTPServer(('127.0.0.1', 0), resources['Mcp'])
        self.mcp.wire, self.mcp.files = [], self.root
        (self.root / 'native.txt').write_text('INITIAL-SUITE-MCP-OWNED-417', encoding='utf-8')
        threading.Thread(target=self.mcp.serve_forever, daemon=True).start()
        agent = self.root / 'agent'
        (agent / 'settings.json').write_text(json.dumps({'autoUpdate': False, 'sessionDir': str(agent / 'sessions')}), encoding='utf-8')
        (agent / 'config.yml').write_text('autoResume: false\nstartup:\n  setupWizard: false\n  showSplash: false\n  checkUpdate: false\nmarketplace:\n  autoUpdate: off\n', encoding='utf-8')
        model = {'providers': {'initial-owned': {'baseUrl': f'http://127.0.0.1:{self.model.server_port}/v1',
            'api': 'openai-completions', 'apiKey': 'owned-unused-local-key', 'models': [{'id': 'initial-probe',
            'name': 'Initial suite command probe', 'reasoning': False, 'input': ['text'],
            'contextWindow': 32768, 'maxTokens': 1024, 'cost': {'input': 0, 'output': 0, 'cacheRead': 0, 'cacheWrite': 0}}]}}}
        for name in ['models.json', 'models.yml']:
            (agent / name).write_text(json.dumps(model), encoding='utf-8')
        self.authority = self.root / 'declarations with spaces'
        self.authority.mkdir()
        source = self.args.repo / 'meta-data/suites.json'
        shutil.copy2(source, self.authority / 'suites.json')
        self.evidence['deliveredDefinition'] = {'source': str(source), 'sha256': digest(source),
            'copySha256': digest(self.authority / 'suites.json')}
        (self.authority / 'mcp.json').write_text(json.dumps({'mcpServers': {'codebase-memory-mcp': {
            'url': f'http://127.0.0.1:{self.mcp.server_port}/native', 'exposure': 'direct'}}}), encoding='utf-8')
        self.deployment = self.root / 'toolkit'
        for folder in ['adapters', 'framework', 'extensions', 'entrypoints']:
            shutil.copytree(self.args.toolkit / folder, self.deployment / folder,
                ignore=shutil.ignore_patterns('.runtime', 'node_modules', '.git'))
        shutil.copy2(self.args.toolkit / 'package.json', self.deployment / 'package.json')
        shutil.copytree(self.args.toolkit / 'node_modules/yaml', self.deployment / 'node_modules/yaml')
        self.evidence['toolkitDeployment'] = {'source': str(self.args.toolkit), 'root': str(self.deployment),
            'sha256': {str(p.relative_to(self.deployment)): digest(p) for folder in ['adapters', 'framework', 'extensions', 'entrypoints']
                       for p in (self.deployment / folder).rglob('*') if p.is_file()}}
        roots = {**self.roots, 'toolkit': str(self.deployment)}
        (self.root / 'paths.toml').write_text('[roots]\n' + ''.join(f'{key} = {json.dumps(value)}\n' for key, value in roots.items()), encoding='utf-8')
        observer = self.root / 'native-discovery.ts'
        # Public extension APIs only; getCommands observes the host's final
        # loaded skills including manual-only expert-translate. A prompt-only
        # oracle would miss that skill and weaken the promised 18-item set.
        observer.write_text('''import {writeFileSync} from "node:fs";
export default function(pi) {
 pi.registerCommand("initial-suite-discovery", {description:"Observe loaded native resources", handler:async (_args,ctx)=>{
  writeFileSync(''' + json.dumps(str(self.root / 'discovery.json')) + ''',JSON.stringify({pid:process.pid,commands:pi.getCommands()}));
  ctx.ui.notify("INITIAL-SUITE-DISCOVERY-READY-739","info");
 }});
}
''', encoding='utf-8')
        self.evidence['observer'] = {'path': str(observer), 'sha256': digest(observer), 'publicApi': 'pi.registerCommand + pi.getCommands'}
        options = 'skills = []\n'
        for host in ['omp', 'pi']:
            options += f'[[agents]]\nid = "{host}"\nlabel = "{host}"\n'
        options += '[[models]]\nid = "default"\nlabel = "Default"\n[[models]]\nid = "probe"\nlabel = "Probe"\nprovider_model = "initial-owned/initial-probe"\nthinking = { pi = ["off"], omp = ["off"] }\n'
        for host in ['omp', 'pi']:
            entries = [(host + '-native-discovery', observer),
                (host + '-herdr-agent-state', Path(self.roots[host + '_state_extension'])),
                (host + '-model-cost', self.deployment / f'entrypoints/{host}/model-cost.ts'),
                (host + '-manager-chat', self.deployment / f'entrypoints/{host}-auto/manager-chat.ts'),
                (host + '-node-chat', self.deployment / f'entrypoints/{host}-auto/node-chat.ts')]
            if host == 'pi':
                entries.append(('pi-suite', self.deployment / 'entrypoints/pi/pi-suite.ts'))
            for name, path in entries:
                require(path.is_file(), ('Missing fixed extension', str(path)))
                options += f'[[extensions]]\nid = "{name}"\nlabel = "{name}"\nagent = "{host}"\nkind = "fixed"\nentrypoint = {json.dumps(str(path))}\n'
        metadata = self.args.repo / 'meta-data/task-options.toml'
        declared = tomllib.loads(metadata.read_text(encoding='utf-8'))
        for extension in declared.get('extensions', []):
            if extension.get('kind') != 'optional':
                continue
            entrypoint = extension['entrypoint']
            # Keep root references for the product resolver, including toolkit's
            # isolated replacement; only metadata-relative paths need rebasing.
            if not entrypoint.startswith('${roots.') and not Path(entrypoint).is_absolute():
                entrypoint = str(metadata.parent / entrypoint)
            values = {**extension, 'entrypoint': entrypoint}
            options += '[[extensions]]\n' + ''.join(
                f'{key} = {json.dumps(values[key])}\n'
                for key in ['id', 'label', 'agent', 'kind', 'entrypoint'])
        (self.root / 'options.toml').write_text(options, encoding='utf-8')
        q = lambda value: json.dumps(str(value))
        (self.root / 'herdr.toml').write_text('onboarding = false\n[terminal]\ndefault_shell = "cmd.exe"\n[update]\nversion_check = false\nmanifest_check = false\n', encoding='utf-8')
        self.env['HERDR_CONFIG_PATH'] = str(self.root / 'herdr.toml')
        self.start([self.args.herdr, 'server'], 'herdr')
        self.herdr_started = True
        until('owned named Herdr server', lambda: subprocess.run([str(self.args.herdr), 'workspace', 'list'],
            cwd=self.root, env=self.env, capture_output=True, timeout=5).returncode == 0)
        self.herdr('workspace', 'create', '--cwd', str(self.root / 'zone'), '--label', 'initial-suites', '--no-focus')
        config = f'listen_addr = "127.0.0.1:0"\nstatic_directory = {q(self.args.repo / "backend/static")}\ndata_directory = {q(self.root / "data")}\npaths_file = {q(self.root / "paths.toml")}\n[mdns]\nenabled = false\n'
        for name in ['start', 'stop']:
            config += f'[operations.{name}]\nscript = {q(self.root / "unused.bat")}\nworking_directory = {q(self.root)}\n'
        config += f'[herdr]\nworkspace_name = "initial-suites"\nexecutable = {q(self.args.herdr)}\n[tasks]\nstorage_file = {q(self.root / "data/tasks.json")}\nmetadata_file = {q(self.root / "options.toml")}\n'
        config += f'[suites]\ndefinition_file = {q(self.authority / "suites.json")}\nmcp_file = {q(self.authority / "mcp.json")}\ngenerated_directory = {q(self.root / "generated resources")}\n'
        config += f'[sessions]\nhistory_file = {q(self.root / "data/history.json")}\n[zones]\nregistry_file = {q(self.root / "data/zones.json")}\n'
        (self.root / 'backend.toml').write_text(config, encoding='utf-8')
        backend = self.start([self.args.backend, '--config', self.root / 'backend.toml'], 'backend')
        def address():
            require(backend.child.poll() is None, 'Owned backend exited; see logs')
            while not backend.lines.empty():
                match = re.search(r'http://(127\.0\.0\.1:\d+)/', backend.lines.get_nowait())
                if match:
                    return 'http://' + match[1]
        self.base = until('owned backend dynamic loopback', address)
        for folder, content in [('connect-bot', f'endpoint: {self.base}\nrequestTimeoutMs: 1000\n'),
                                ('workflow-chat', f'endpoint: {self.base}\nrequestTimeoutMs: 30000\ncollectTimeoutMs: 5460000\n')]:
            path = self.deployment / f'extensions/{folder}/config.yaml'
            path.write_text(content, encoding='utf-8')
            self.evidence.setdefault('deploymentOverrides', []).append({'path': str(path), 'sha256': digest(path), 'content': content})
        self.http('/api/suites')

    def exercise(self):
        pane = None
        try:
            self.http('/api/zones', {'id': 'initial-owned', 'path': str(self.root / 'zone')})
            tasks = self.http('/api/tasks', {'name': self.scenario, 'chatName': '{task.name}-{chat.id}',
                'agent': self.agent, 'taskType': 'normal', 'model': 'probe', 'thinking': 'off',
                'skill': None, 'prompt': '', 'suite': self.suite_id})['tasks']
            task = next(task for task in tasks if task['name'] == self.scenario)
            preview = self.http('/api/chats/preview', {'zone': 'initial-owned', 'taskId': task['id']})
            launch = self.http('/api/chats', {'zone': 'initial-owned', 'taskId': task['id'],
                'chatId': preview['chatId'], 'input': '/initial-suite-discovery'}, status=202)
            def discovered():
                state = self.http('/api/chats/' + launch['id'])
                self.evidence['creation'] = state
                require(state['stage'] != 'failed', state)
                return state if state['stage'] == 'succeeded' and (self.root / 'discovery.json').is_file() else None
            state = until('actual native discovery command', discovered, seconds=self.args.timeout)
            pane = state['paneId']
            observed = json.loads((self.root / 'discovery.json').read_text(encoding='utf-8'))
            self.evidence['nativeDiscovery'] = observed
            self.remember_process(observed['pid'])
            skills = [c for c in observed['commands'] if c['source'] == 'skill']
            names = [c['name'].removeprefix('skill:') for c in skills]
            require(sorted(names) == sorted(self.expected), ('Independent exact native skill set', sorted(self.expected), names))
            origins = {}
            for skill in skills:
                name = skill['name'].removeprefix('skill:')
                source = skill.get('path') or skill.get('sourceInfo', {}).get('path')
                require(source, ('Native command omitted skill origin', skill))
                actual = Path(source).resolve()
                if actual.is_file():
                    actual = actual.parent
                root, suffix = self.expected[name]
                expected = (Path(self.roots[root]) / suffix).resolve()
                require(actual == expected, ('Wrong skill source priority/origin', name, str(expected), str(actual)))
                origins[name] = {'path': str(actual), 'skillSha256': digest(actual / 'SKILL.md')}
            self.evidence['nativeSkillOrigins'] = origins
            self.evidence['discoveryVerified'] = True
            self.capture()
            package = self.args.omp_package if self.agent == 'omp' else self.args.pi_package
            entry = str(package / ('dist/cli.js' if self.agent == 'omp' else 'dist/bundle/cli.js')).replace('\\', '/').lower()
            native = [row for snapshot in self.evidence['ownedSnapshots'] for row in snapshot
                      if entry in (row.get('CommandLine') or '').replace('\\', '/').lower()]
            require(native, ('Installed delivered CLI was not observed under Herdr', entry))
            self.evidence['publicNativeEntrypoint'] = native
            if self.suite_id == 'code-implementation':
                registered = [c for c in observed['commands'] if c['source'] == 'extension' and c['name'] == 'hyper-auto-coding']
                require(len(registered) == 1, ('Actual oh-my-wayfinder command registration missing', registered))
                # No slug means the real handler's usage branch, before reading
                # any spec, starting Phase 2, worktrees, TDD or retro.
                self.command([self.args.herdr, 'pane', 'send-text', pane, '/hyper-auto-coding'])
                self.command([self.args.herdr, 'pane', 'send-keys', pane, 'enter'])
                def usage():
                    terminal = self.command([self.args.herdr, 'pane', 'read', pane, '--lines', '250', '--format', 'text'])
                    self.evidence['commandTerminal'] = terminal
                    return terminal if re.search(r'用法:\s*/hyper-auto-coding\s+<slug>', terminal) else None
                until('real registered hyper-auto-coding usage response', usage, seconds=self.args.timeout)
                self.evidence['commandVerified'] = True
            require(self.model.requests == [], ('Command check executed a model task', self.model.requests))
            require(not (self.root / 'zone/.scratch').exists(), 'Command check created user task artifacts')
            self.evidence['passed'] = True
        finally:
            if self.herdr_started:
                try:
                    self.capture()
                except BaseException:
                    self.evidence['cleanupErrors'].append(traceback.format_exc())
            pane = pane or self.evidence.get('creation', {}).get('paneId')
            if pane:
                try:
                    self.evidence['terminal'] = self.command([self.args.herdr, 'pane', 'read', pane, '--lines', '250', '--format', 'text'])
                except BaseException:
                    self.evidence['terminalError'] = traceback.format_exc()

    def cleanup_extra(self):
        for pid, handle in self.handles.items():
            try:
                until('owned native process exit', lambda: not handle.alive(), seconds=20)
                self.evidence['ownedProcessExits'].append({'pid': pid, 'exited': True})
            except BaseException:
                self.evidence['cleanupErrors'].append(traceback.format_exc())
                self.evidence['ownedProcessExits'].append({'pid': pid, 'exited': False})
            finally:
                handle.close()
        self.evidence['serviceExits'] = [{'pid': p.child.pid, 'exit': p.child.poll()} for p in self.processes]
        if self.model:
            self.evidence['modelRequests'] = self.model.requests
        if self.mcp:
            self.evidence['mcpWire'] = self.mcp.wire
            self.mcp.shutdown()
            self.mcp.server_close()


def protected_identity(args):
    """Hash installed delivery bytes, not a source CLI or SDK substitute."""
    result = {str(args.herdr): digest(args.herdr)}
    for package in [args.pi_package, args.omp_package]:
        for path in package.rglob('*'):
            relative = path.relative_to(package)
            if path.is_file() and 'node_modules' not in relative.parts and '.git' not in relative.parts:
                result[str(path)] = digest(path)
    return result


def delivery_manifest(args, evidence):
    """P44 consistency is evidence, not a new product/scheduling gate.

    The explicit baselines delimit all contributed delivery changes. Both
    inventories retain every file and byte identity, plus commit provenance.
    Independent forbidden patterns catch host forks/patches/SDK reconstruction;
    installed byte stability and OS entrypoint observations are separate checks.
    """
    inventories = {}
    for name, root, base in [('herdr-bot', args.repo, args.herdr_bot_base),
                              ('toolkit', args.toolkit, args.toolkit_base)]:
        resolved = command(['git', 'rev-parse', '--verify', base + '^{commit}'], root).strip()
        command(['git', 'merge-base', '--is-ancestor', resolved, 'HEAD'], root)
        names = set(command(['git', 'diff', '--name-only', resolved, '--', '.', ':!.scratch'], root).splitlines())
        names.update(command(['git', 'ls-files', '--others', '--exclude-standard'], root).splitlines())
        names = sorted(p for p in names if not p.startswith('.scratch/'))
        files = []
        for filename in names:
            path = root / filename
            require(not any(part in ['vendor', 'patches', 'node_modules'] for part in Path(filename).parts)
                    and not filename.endswith(('.patch', '.diff')), ('Undeclared host fork/patch delivery', name, filename))
            files.append({'path': filename, 'sha256': digest(path) if path.is_file() else None,
                          'deleted': not path.exists()})
        production = ['backend/src', 'frontend/src'] if name == 'herdr-bot' else ['entrypoints', 'extensions', 'adapters', 'framework']
        patch = command(['git', 'diff', '--unified=0', resolved, '--', *production], root)
        additions = '\n'.join(line[1:] for line in patch.splitlines() if line.startswith('+') and not line.startswith('+++'))
        forbidden = re.findall(r'createAgentSession\s*\(|new\s+AgentSession\s*\(|(?:Object\.defineProperty|Reflect\.set)\s*\(|(?:pi|session|agent)\._(?:resourceLoader|extensionRunner)|(?:pi-coding-agent|oh-my-pi)[^\n\"\']*/src/', additions)
        require(not forbidden, ('Internal patch/SDK reconstruction in delivered product changes', name, forbidden))
        inventories[name] = {'root': str(root), 'base': resolved,
            'revision': command(['git', 'rev-parse', 'HEAD'], root).strip(),
            'changes': files, 'productionDiff': patch,
            'contributions': command(['git', 'log', '--format=%H %s', '--name-status', resolved + '..HEAD', '--', '.', ':!.scratch'], root),
            'uncommittedDiff': command(['git', 'diff', '--', '.', ':!.scratch'], root)}
    evidence['deliveryManifest'] = inventories
    evidence['entrypoint'] = {'path': str(Path(__file__).resolve()), 'sha256': digest(Path(__file__))}
    evidence['protectedInstalledBefore'] = protected_identity(args)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ['herdr-bot-worktree', 'toolkit-worktree', 'backend', 'paths-file', 'evidence-dir']:
        parser.add_argument('--' + flag, type=lambda value: Path(value).resolve(), required=True)
    for flag in ['herdr-bot-base', 'toolkit-base']:
        parser.add_argument('--' + flag, required=True)
    parser.add_argument('--scenario', choices=['all', *SCENARIOS], default='all')
    parser.add_argument('--timeout', type=int, choices=range(1, 301), default=90)
    args = parser.parse_args()
    args.repo, args.toolkit = args.herdr_bot_worktree, args.toolkit_worktree
    args.evidence_dir.mkdir(parents=True, exist_ok=False)
    evidence = {'passed': False, 'coverage': {item: {'verified': False} for item in COVERAGE},
                'startedProcesses': [], 'runs': [], 'cleanupErrors': [], 'argv': sys.argv[1:]}
    try:
        require(args.backend.is_relative_to(args.repo), 'backend provenance: backend must belong to the supplied worktree')
        resources['preflight'](args, evidence)
        delivery_manifest(args, evidence)
        roots = evidence['paths']['roots']
        required = ['wayfinder', 'wayfinder_skills', 'matt_pocock_skills', 'expert_translate',
                    'pi_state_extension', 'omp_state_extension']
        require(all(key in roots for key in required), ('Dependency-only roots missing', required, sorted(roots)))
        evidence['sourceResources'] = {}
        for name, (root, suffix) in {**EXPLORATION, **IMPLEMENTATION}.items():
            path = Path(roots[root]) / suffix / 'SKILL.md'
            evidence['sourceResources'][name] = {'path': str(path), 'sha256': digest(path) if path.is_file() else None,
                                                  'missing': not path.is_file()}
        scenarios = list(SCENARIOS) if args.scenario == 'all' else [args.scenario]
        for scenario in scenarios:
            run = Acceptance(args, scenario, roots)
            result = run.run()
            evidence['runs'].append({'scenario': scenario, 'exit': result, 'evidence': str(run.root / 'evidence.json')})
            evidence['startedProcesses'].extend({'pid': p.child.pid, 'exit': p.child.poll()} for p in run.processes)
            evidence['cleanupErrors'].extend(run.evidence['cleanupErrors'])
            clean = result == 0 and not run.evidence['cleanupErrors']
            evidence['coverage']['P11/' + scenario].update(verified=clean and run.evidence['discoveryVerified'], evidence=str(run.root / 'evidence.json'))
            if scenario == 'implementation-omp':
                evidence['coverage']['P42'].update(verified=clean and run.evidence['commandVerified'], evidence=str(run.root / 'evidence.json'))
        evidence['protectedInstalledAfter'] = protected_identity(args)
        require(evidence['protectedInstalledBefore'] == evidence['protectedInstalledAfter'],
                'Installed Herdr/OMP/Pi bytes changed during acceptance')
        # Static manifests alone cannot satisfy P44: every required delivered
        # host entry must have been observed in a successful native scenario.
        evidence['coverage']['P44']['verified'] = (len(evidence['runs']) == 3
            and all(item['exit'] == 0 for item in evidence['runs']) and not evidence['cleanupErrors'])
        evidence['passed'] = all(item['verified'] for item in evidence['coverage'].values()) and not evidence['cleanupErrors']
    except BaseException:
        evidence['error'] = traceback.format_exc()
    (args.evidence_dir / 'evidence.json').write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps({'passed': evidence['passed'], 'evidence': str(args.evidence_dir / 'evidence.json')}))
    return 0 if evidence['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
