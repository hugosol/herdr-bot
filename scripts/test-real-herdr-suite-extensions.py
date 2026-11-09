"""P7/P9/P46: existing extension packages consumed through real Herdr/OMP.

S2/S3/S4, isolated loopback model only. Real factories import sibling TS and a
package dependency; actual tools read assets and compute literal known results.
No SDK loader, source CLI, user credentials, configuration-dir product redirect,
or replacement manager. Shared real-host lifecycle comes from the skills runner.
"""
import argparse
import hashlib
import http.server
import json
from pathlib import Path
import runpy
import shutil
import sys
import traceback

from developer_paths import REPO, backend_suffix

skills = runpy.run_path(str(Path(__file__).with_name('test-real-herdr-suite-skills.py')))
support = skills['support']
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
                delta, finish = {'role':'assistant', 'content':'<title>Extension package acceptance</title>'}, 'stop'
            elif not any(m['role'] == 'tool' for m in messages):
                tools = [tool['function']['name'] for tool in request['tools']]
                system = '\n'.join(str(m.get('content', '')) for m in messages if m['role'] in ['system', 'developer'])
                require('write' in tools and all('xd://' + name in system for name in ['suite_relative_total', 'suite_root_total']),
                        ('Real extension factories did not mount both tools', tools, system))
                calls = [dict(index=i, id=f'compute-{i}', type='function', function=dict(name='write',
                    arguments=json.dumps({'i':'Computing package total', 'path':'xd://' + name,
                                          'content':json.dumps({'quantity':3})})))
                         for i, name in enumerate(['suite_relative_total', 'suite_root_total'])]
                delta, finish = {'role':'assistant', 'tool_calls':calls}, 'tool_calls'
            else:
                outputs = '\n'.join(str(m.get('content', '')) for m in messages if m['role'] == 'tool')
                # Independently worked examples: (19*3)+7=64; (23*3)+11=80.
                for expected in ['RELATIVE-ASSET-412:64', 'ROOT-ASSET-905:80']:
                    require(expected in outputs, ('Asset/dependency computation missing', expected, outputs))
                case['passed'] = True
                delta, finish = {'role':'assistant', 'content':'SUITE-EXTENSIONS-VERIFIED-621'}, 'stop'
        except BaseException:
            case.setdefault('error', traceback.format_exc())
            delta, finish = {'role':'assistant', 'content':'SUITE-EXTENSIONS-ASSERTION-FAILED'}, 'stop'
        self.send_response(200)
        self.send_header('Content-Type', 'text/event-stream')
        self.end_headers()
        for item, reason in [(delta, None), ({}, finish)]:
            chunk = {'id':'owned', 'object':'chat.completion.chunk', 'created':1, 'model':'suite-probe',
                     'choices':[{'index':0, 'delta':item, 'finish_reason':reason}]}
            self.wfile.write(('data: ' + json.dumps(chunk) + '\n\n').encode())
        self.wfile.write(b'data: [DONE]\n\n')


class Acceptance(skills['Acceptance']):
    def __init__(self, args):
        super().__init__(args)
        self.evidence.update(promise=['P7:OMP-extensions', 'P9:OMP'], scenario='extension-packages', cases=[])

    def prepare(self):
        super().prepare()
        self.model.RequestHandlerClass = Model
        relative = self.authority / 'relative existing package'
        rooted = self.root / 'central extension package'
        self.make_package(relative, 'suite_relative_total', 19, 7, 'RELATIVE-ASSET-412')
        self.make_package(rooted, 'suite_root_total', 23, 11, 'ROOT-ASSET-905')
        # Non-index entries load directly from their original package directories.
        self.entrypoints = [(relative / 'extension/entry.ts').resolve(), (rooted / 'extension/entry.ts').resolve()]
        self.expected_targets = {relative.resolve(), rooted.resolve()}
        self.source_bytes = {path:path.read_bytes() for source in self.expected_targets for path in source.rglob('*') if path.is_file()}
        (self.root / 'paths.toml').write_text('[roots]\nowned_extensions = ' + json.dumps(str(rooted)) + '\n')
        # Roots are resolved on startup; restart our backend only, never Herdr or user processes.
        backend = self.processes[-1]
        backend.stop()
        (self.authority / 'suites.json').write_text(json.dumps({'suites':[{'id':'selected', 'name':'Selected', 'agents':['omp'],
            'skills':{'omp': [], 'pi': []}, 'extensions':{'omp':['relative existing package/extension/entry.ts', '${roots.owned_extensions}/extension/entry.ts']}}]}))
        for zone in self.zones:
            # Wrong-cwd resource must not be selected; literal wrong result is distinguishable.
            wrong = zone / 'relative existing package/extension'
            wrong.mkdir(parents=True)
            (wrong / 'entry.ts').write_text('throw new Error("WRONG-CHAT-CWD-ENTRY-831");')
        child = self.start([self.args.backend, '--config', self.root / 'backend.toml'], 'extension-backend')
        def address():
            require(child.child.poll() is None, 'Backend exited; see retained logs')
            while not child.lines.empty():
                match = __import__('re').search(r'http://(127\.0\.0\.1:\d+)/', child.lines.get_nowait())
                if match:
                    return 'http://' + match[1]
        self.base = until('extension backend dynamic loopback', address)
        for name in ['backend/src/suites/extensions.rs', 'scripts/test-real-herdr-suite-extensions.py']:
            self.evidence['source']['sha256'][name] = hashlib.sha256((REPO / name).read_bytes()).hexdigest()
        self.evidence['source']['diff'] = self.command(['git', '-C', REPO, 'diff', '--', 'backend', 'scripts', 'docs'])

    @staticmethod
    def make_package(root, tool, price, offset, marker):
        for folder in ['extension', 'shared', 'assets', 'node_modules/owned-offset']:
            (root / folder).mkdir(parents=True, exist_ok=True)
        (root / 'package.json').write_text('{"type":"module"}')
        (root / 'node_modules/owned-offset/package.json').write_text('{"name":"owned-offset","type":"module","main":"index.js"}')
        (root / 'node_modules/owned-offset/index.js').write_text(f'export const offset = {offset};\n')
        (root / 'shared/total.ts').write_text('export const total = (unit, quantity, offset) => unit * quantity + offset;\n')
        (root / 'assets/price.json').write_text(json.dumps({'unit':price, 'marker':marker}))
        (root / 'extension/entry.ts').write_text('''import { readFileSync } from 'node:fs';
import { total } from '../shared/total.ts';
import { offset } from 'owned-offset';
export default function(pi) {
 const z = pi.zod;
 pi.registerTool({name: TOOL, label: 'Owned package total', description: 'Read bundled unit price and compute a total',
  parameters: z.object({quantity: z.number()}),
  async execute(_id, params) {
   const asset = JSON.parse(readFileSync(new URL('../assets/price.json', import.meta.url), 'utf8'));
   return {content:[{type:'text', text:asset.marker+':'+total(asset.unit, params.quantity, offset)}], details:{}};
  }});
}
'''.replace('TOOL', json.dumps(tool)))

    def exercise(self):
        saved = self.http('/api/tasks', {'name':'Suite extension packages', 'chatName':'{task.name}-{chat.id}',
            'agent':'omp', 'taskType':'normal', 'model':'probe', 'thinking':'off', 'skill':None,
            'prompt':'Use the selected extension tools.', 'suite':'selected'})
        task_id = saved['tasks'][0]['id']
        for index, zone in enumerate(self.zones):
            case = {'cwd':str(zone), 'requests':[], 'passed':False}
            self.evidence['cases'].append(case)
            self.model.case = case
            self.http('/api/zones', {'id':f'owned-{index}', 'path':str(zone)})
            preview = self.http('/api/chats/preview', {'zone':f'owned-{index}', 'taskId':task_id})
            launch = self.http('/api/chats', {'zone':f'owned-{index}', 'taskId':task_id,
                'chatId':preview['chatId'], 'input':'Compute quantity three using both selected package tools.'}, status=202)
            def result():
                self.observe_mcp()
                current = self.http('/api/chats/' + launch['id'])
                case['latestCreation'] = current
                require(current['stage'] != 'failed', current)
                require(not case.get('error'), case.get('error'))
                return current if case['passed'] and current['stage'] == 'succeeded' else None
            try:
                case['creation'] = until('real extension tool asset/dependency calculations', result, seconds=self.args.timeout)
            except BaseException:
                pane = case.get('latestCreation', {}).get('paneId')
                if pane:
                    try:
                        case['terminal'] = self.command([self.args.herdr, 'pane', 'read', pane, '--lines', '150', '--format', 'text'])
                    except BaseException:
                        case['diagnosticError'] = traceback.format_exc()
                raise
            generated = self.root / 'generated resources with spaces'
            links = [path for path in generated.rglob('*') if path.is_symlink()]
            require(not links, ('Direct extension entries must not create package links', [str(x) for x in links]))
            require(not list(generated.rglob('omp-extensions.json')), 'Direct extension entries must not create a loader manifest')
            case['sourceEntrypoints'] = [str(path) for path in self.entrypoints]
            require((self.root / 'fixed-loaded.txt').read_text() == 'FIXED-LOADED-628', 'Fixed extension was lost')
            for path, original in self.source_bytes.items():
                require(path.read_bytes() == original, ('Source package was modified', str(path)))
        self.evidence['passed'] = True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--herdr', type=Path, required=True)
    parser.add_argument('--omp-package', type=Path, required=True)
    parser.add_argument('--backend', type=Path, default=REPO / backend_suffix())
    parser.add_argument('--node', type=Path, default=Path(shutil.which('node') or 'node'))
    parser.add_argument('--timeout', type=int, choices=range(1, 301), default=120)
    args = parser.parse_args()
    args.scenario, args.repo = 'empty', REPO
    return Acceptance(args).run()


if __name__ == '__main__':
    sys.exit(main())
