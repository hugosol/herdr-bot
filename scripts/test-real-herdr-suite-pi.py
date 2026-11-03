"""Ticket 07 S2/S3/S4 integration smoke, not the 09/10 contract matrix.

Real Herdr launches installed Pi through the production Task HTTP interface.
A loopback model calls a selected package tool which reads a sibling asset,
and native read consumes a selected skill. Missing resources/tools distinguish
an ignored Suite from a real successful launch; no SDK or replacement CLI.
"""
import argparse
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
            if not request.get('tools'):
                delta, finish = {'role': 'assistant', 'content': '<title>Pi Suite smoke</title>'}, 'stop'
            elif not any(m['role'] == 'tool' for m in messages):
                names = [t['function']['name'] for t in request['tools']]
                require('suite_asset_probe' in names, ('Selected package tool missing', names))
                system = '\n'.join(str(m.get('content', '')) for m in messages if m['role'] in ['system', 'developer'])
                skill = re.search(r'<name>smoke-selected</name>\s*<description>.*?</description>\s*<location>(.*?)</location>', system, re.S)
                require(skill is not None, ('Selected skill was not advertised by native Pi', system))
                selected_path = html.unescape(skill[1])
                require('generated resources' in selected_path, ('Pi must consume the published resource path', selected_path))
                case['advertisedSkill'] = selected_path
                calls = [('read', {'path': selected_path}), ('suite_asset_probe', {})]
                delta = {'role': 'assistant', 'tool_calls': [dict(index=i, id=f'probe-{i}', type='function',
                    function=dict(name=name, arguments=json.dumps(args))) for i, (name, args) in enumerate(calls)]}
                finish = 'tool_calls'
            else:
                outputs = '\n'.join(str(m.get('content', '')) for m in messages if m['role'] == 'tool')
                require('PI-SKILL-BODY-731' in outputs, ('Selected skill was not consumed', outputs))
                require('PI-PACKAGE-ASSET-482:64' in outputs, ('Selected package asset was not consumed', outputs))
                case['passed'] = True
                delta, finish = {'role': 'assistant', 'content': 'PI-SUITE-SMOKE-VERIFIED-951'}, 'stop'
        except BaseException:
            case.setdefault('error', traceback.format_exc())
            delta, finish = {'role': 'assistant', 'content': 'PI-SUITE-SMOKE-FAILED'}, 'stop'
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
        super().__init__(args, prefix='herdr-suite-pi-smoke-')
        self.evidence.update(promise='07 integration development smoke', cases=[])

    def prepare(self):
        require(sys.platform == 'win32', 'Real Herdr smoke requires Windows')
        require(os.environ.get('HERDR_BOT_MANUAL_SYMLINKS') == '1',
                'Set HERDR_BOT_MANUAL_SYMLINKS=1 explicitly; no privilege escalation or fallback')
        cli = self.args.pi_package / 'dist/bundle/cli.js'
        fixed = self.args.toolkit / 'entrypoints/pi/pi-suite.ts'
        for path in [self.args.herdr, self.args.backend, cli, fixed]:
            require(path.is_file(), ('Missing real dependency', str(path)))
        launcher = Path(shutil.which('pi') or '')
        require(launcher.is_file(), 'Installed pi must already be on PATH')
        require('@earendil-works/pi-coding-agent' in launcher.read_text().replace('\\', '/'),
                'PATH pi must be the installed native package launcher')
        self.evidence['source'] = {'repo': str(REPO), 'revision': self.command(['git', '-C', REPO, 'rev-parse', 'HEAD']),
            'diff': self.command(['git', '-C', REPO, 'diff', '--', 'backend', 'scripts', 'meta-data', 'docs']),
            'sha256': {name: hashlib.sha256((REPO / name).read_bytes()).hexdigest() for name in [
                'backend/src/tasks/mod.rs', 'backend/src/suites.rs', 'backend/src/chats/mod.rs',
                'scripts/test-real-herdr-suite-pi.py']}}
        self.evidence['executables'] = {str(path): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in [self.args.herdr, self.args.backend, cli, launcher, fixed]}
        self.evidence['piVersion'] = json.loads((self.args.pi_package / 'package.json').read_text())['version']
        self.model = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Model)
        threading.Thread(target=self.model.serve_forever, daemon=True).start()
        agent = self.root / 'agent'
        (agent / 'settings.json').write_text(json.dumps({'autoUpdate': False}))
        (agent / 'models.json').write_text(json.dumps({'providers': {'suite-local': {
            'baseUrl': f'http://127.0.0.1:{self.model.server_port}/v1', 'api': 'openai-completions',
            'apiKey': 'owned-loopback-not-a-credential', 'models': [{'id': 'suite-probe', 'name': 'Suite probe',
            'reasoning': False, 'input': ['text'], 'contextWindow': 32768, 'maxTokens': 2048,
            'cost': {'input': 0, 'output': 0, 'cacheRead': 0, 'cacheWrite': 0}}]}}}))
        authority = self.root / 'declarations with spaces'
        authority.mkdir()
        skill = authority / 'selected skill'
        skill.mkdir()
        (skill / 'SKILL.md').write_text('---\nname: smoke-selected\ndescription: Owned smoke resource\n---\nPI-SKILL-BODY-731\n')
        package = authority / 'selected package'
        (package / 'extension').mkdir(parents=True)
        (package / 'assets').mkdir()
        (package / 'package.json').write_text(json.dumps({'type': 'module', 'pi': {'extensions': ['extension/probe.ts']}}))
        (package / 'assets/payload.json').write_text('{"unit":19,"offset":7}')
        (package / 'extension/probe.ts').write_text('''import { readFileSync } from "node:fs";
export default function(pi) {
  pi.registerTool({name:"suite_asset_probe",label:"Suite asset",description:"Read selected package asset",
    parameters:{type:"object",properties:{}},async execute() {
      const p=JSON.parse(readFileSync(new URL("../assets/payload.json",import.meta.url),"utf8"));
      return {content:[{type:"text",text:"PI-PACKAGE-ASSET-482:"+(p.unit*3+p.offset)}],details:{}};
    }});
}
''')
        (authority / 'suites.json').write_text(json.dumps({'suites': [{'id': 'selected', 'name': 'Selected',
            'agents': ['pi'], 'skills': ['selected skill'], 'extensions': {'pi': ['selected package']},
            'pi': {'codemode': 'on'}}]}))
        (self.root / 'paths.toml').write_text('[roots]\n')
        self.source_bytes = {p: p.read_bytes() for directory in [skill, package] for p in directory.rglob('*') if p.is_file()}
        q = lambda value: json.dumps(str(value))
        (self.root / 'options.toml').write_text('skills = []\n[[agents]]\nid = "pi"\nlabel = "Pi"\n'
            '[[models]]\nid = "default"\nlabel = "Default"\n[[models]]\nid = "probe"\nlabel = "Probe"\n'
            'provider_model = "suite-local/suite-probe"\nthinking = { pi = ["off"] }\n'
            '[[extensions]]\nid = "pi-suite"\nlabel = "pi-suite"\nagent = "pi"\nkind = "fixed"\nentrypoint = ' + q(fixed) + '\n')
        herdr_config = self.root / 'herdr.toml'
        herdr_config.write_text('onboarding = false\n[terminal]\ndefault_shell = "cmd.exe"\n[update]\nversion_check = false\nmanifest_check = false\n')
        self.env['HERDR_CONFIG_PATH'] = str(herdr_config)
        self.evidence['herdrVersion'] = self.command([self.args.herdr, '--version'])
        self.start([self.args.herdr, 'server'], 'herdr')
        self.herdr_started = True
        def ready():
            return subprocess.run([str(self.args.herdr), 'workspace', 'list'], cwd=self.root,
                env=self.env, capture_output=True, timeout=5).returncode == 0
        until('owned Herdr server', ready)
        self.herdr('workspace', 'create', '--cwd', str(self.root / 'zone'), '--label', 'suite-pi-smoke', '--no-focus')
        static = self.root / 'static'
        static.mkdir()
        (static / 'index.html').write_text('<!doctype html><title>S2/S4 smoke only</title>')
        config = f'listen_addr = "127.0.0.1:0"\nstatic_directory = {q(static)}\ndata_directory = {q(self.root / "data")}\npaths_file = {q(self.root / "paths.toml")}\n[mdns]\nenabled = false\n'
        for name in ['start', 'stop']:
            config += f'[operations.{name}]\nscript = {q(self.root / "unused.bat")}\nworking_directory = {q(self.root)}\n'
        config += f'[herdr]\nworkspace_name = "suite-pi-smoke"\nexecutable = {q(self.args.herdr)}\n[tasks]\nstorage_file = {q(self.root / "data/tasks.json")}\nmetadata_file = {q(self.root / "options.toml")}\n'
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
        self.base = until('owned backend dynamic loopback', address)

    def exercise(self):
        task = self.http('/api/tasks', {'name': 'Pi Suite smoke', 'chatName': '{task.name}-{chat.id}',
            'agent': 'pi', 'taskType': 'normal', 'model': 'probe', 'thinking': 'off', 'skill': None,
            'prompt': 'Run the owned smoke probes.', 'suite': 'selected'})['tasks'][0]
        self.http('/api/zones', {'id': 'owned', 'path': str(self.root / 'zone')})
        preview = self.http('/api/chats/preview', {'zone': 'owned', 'taskId': task['id']})
        case = {'requests': [], 'passed': False}
        self.evidence['cases'].append(case)
        self.model.case = case
        launch = self.http('/api/chats', {'zone': 'owned', 'taskId': task['id'], 'chatId': preview['chatId'],
            'input': 'Read the selected skill and call suite_asset_probe.'}, status=202)
        def result():
            current = self.http('/api/chats/' + launch['id'])
            case['latestCreation'] = current
            require(current['stage'] != 'failed', current)
            require(not case.get('error'), case.get('error'))
            return current if case['passed'] and current['stage'] == 'succeeded' else None
        try:
            case['creation'] = until('actual Pi selected resource consumption', result, seconds=self.args.timeout)
        finally:
            pane = case.get('latestCreation', {}).get('paneId')
            if pane:
                case['terminal'] = self.command([self.args.herdr, 'pane', 'read', pane, '--lines', '150', '--format', 'text'])
        for path, content in self.source_bytes.items():
            require(path.read_bytes() == content, ('Source resource modified', str(path)))
        self.evidence['passed'] = True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--herdr', type=Path, required=True)
    parser.add_argument('--pi-package', type=Path, required=True)
    parser.add_argument('--toolkit', type=Path, required=True)
    parser.add_argument('--backend', type=Path, default=REPO / backend_suffix())
    parser.add_argument('--timeout', type=int, choices=range(1, 301), default=90)
    args = parser.parse_args()
    args.repo = REPO
    return Acceptance(args).run()


if __name__ == '__main__':
    sys.exit(main())
