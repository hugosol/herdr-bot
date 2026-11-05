"""P03 native scenarios; independent requirements are fixed before any model call.

The driver supplies user requirements and confirmations, never model decisions,
Bot selections, generated JSON, tool results or workflow runtime transitions.
"""
import hashlib
import json
import os
from pathlib import Path
import re
import runpy
import shutil
import subprocess
import threading
import time
import traceback

from developer_paths import REPO

roles = runpy.run_path(str(REPO / 'scripts/test-workflow-file-handoff.py'))
lifecycle = runpy.run_path(str(REPO / 'scripts/accept-suite-lifecycle.py'))
resources = lifecycle['resources']
Base = roles['Acceptance']
require, until, WireProxy = roles['require'], roles['until'], roles['WireProxy']


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def body_text(value):
    if isinstance(value, str):
        return value
    if isinstance(value, list):
        return '\n'.join(part.get('text', '') for part in value if isinstance(part, dict) and part.get('type') == 'text')
    return ''


class NativePlanningAcceptance(Base):
    # Reuse the established OS handle and native public SessionManager observers,
    # not the lifecycle runner's controlled provider or case decisions.
    capture = lifecycle['Acceptance'].capture
    remember_process = lifecycle['Acceptance'].remember_process
    history = lifecycle['Acceptance'].history

    def __init__(self, args, hosts):
        super().__init__(args, prefix=str(args.evidence_dir / 'native-'))
        self.hosts = hosts
        self.agent = hosts['概要规划']
        self.handles, self.handles_lock = {}, threading.Lock()
        self.backend_wire = self.model_wire = None
        self.evidence.update(promise='P03', passed=False, coverage={}, declaredHosts=hosts,
                             inputs=[], scenarios=[], processes=[])
        self.marker = 'P03-' + self.env['HERDR_SESSION']
        self.model_config = json.loads(args.model_config.read_text(encoding='utf-8'))
        self.chat_count = 0
        # A parent OMP shell marks nested shells; these are new root agents in
        # our owned Herdr panes, not descendants of a user's native Session.
        self.env.pop('OMPCODE', None)
        self.env.update(OMP_CONFIG_DIR=str(self.root / 'agent'), XDG_DATA_HOME=str(self.root / 'state'))
        self.evidence['isolation'].update({key: self.env[key] for key in ['OMP_CONFIG_DIR', 'XDG_DATA_HOME']})

    def configure_model(self):
        # Existing real-provider validation and transparent wire recording. No
        # default model, user authentication or response/decision retry is used.
        roles['RolesAcceptance'].configure_model(self)
        agent = self.root / 'agent'
        model = json.loads((agent / 'models.json').read_text(encoding='utf-8'))
        (agent / 'models.yml').write_text(json.dumps(model), encoding='utf-8')
        settings = json.loads((agent / 'settings.json').read_text(encoding='utf-8'))
        settings.update(autoUpdate=False, retry={'enabled': False, 'maxRetries': 0, 'provider': {'maxRetries': 0}},
                        compaction={'enabled': False})
        (agent / 'settings.json').write_text(json.dumps(settings), encoding='utf-8')
        omp_settings = 'autoResume: false\nretry:\n  enabled: false\n  maxRetries: 0\n  modelFallback: false\n  usageAwareFallback: false\nstartup:\n  setupWizard: false\n  showSplash: false\n  checkUpdate: false\nmarketplace:\n  autoUpdate: off\n'
        if self.args.shell_path:
            omp_settings += 'shellPath: ' + json.dumps(str(self.args.shell_path)) + '\n'
        (agent / 'config.yml').write_text(omp_settings, encoding='utf-8')
        skills = self.args.wayfinder_skills
        for name in ['grill-with-docs', 'grilling', 'domain-modeling']:
            source = skills / name
            require((source / 'SKILL.md').is_file(), ('Missing declared planning skill', str(source)))
            shutil.copytree(source, agent / 'skills' / name)
        self.evidence['skills'] = {name: {'source': str(skills / name), 'sha256': digest(skills / name / 'SKILL.md')}
                                  for name in ['grill-with-docs', 'grilling', 'domain-modeling']}

    def prepare(self):
        require(os.name == 'nt', 'Native acceptance requires Windows process handles')
        self.deployment = self.root / 'toolkit'
        for name in ['adapters', 'framework', 'extensions', 'entrypoints']:
            shutil.copytree(self.args.toolkit / name, self.deployment / name,
                            ignore=shutil.ignore_patterns('.runtime', 'node_modules', '.git'))
        shutil.copy2(self.args.toolkit / 'package.json', self.deployment / 'package.json')
        shutil.copytree(self.args.toolkit / 'node_modules/yaml', self.deployment / 'node_modules/yaml')
        self.evidence['toolkitDeployment'] = {str(path.relative_to(self.deployment)): digest(path)
            for path in self.deployment.rglob('*') if path.is_file()}
        self.configure_model()
        self.observer = self.root / 'native-observer.ts'
        self.observer.write_text('''import {appendFileSync} from "node:fs";
export default function(pi) {
 const record=(event,ctx)=>appendFileSync(process.env.P03_NATIVE_EVENTS,JSON.stringify({
  event:event.type,pid:process.pid,id:ctx.sessionManager.getSessionId(),file:ctx.sessionManager.getSessionFile(),
  cwd:ctx.cwd,model:ctx.model && {id:ctx.model.id,provider:ctx.model.provider}})+"\\n");
 pi.on("session_start",record); pi.on("agent_end",record);
}
''', encoding='utf-8')
        self.env['P03_NATIVE_EVENTS'] = str(self.root / 'native-events.jsonl')
        q = lambda value: json.dumps(str(value))
        options = '[[models]]\nid = "default"\nlabel = "Default"\n[[models]]\nid = "probe"\nlabel = "Authorized P03 model"\nprovider_model = ' + q(self.provider_model) + '\nthinking = { omp = ["medium"], pi = ["medium"] }\n'
        for host in sorted(set(self.hosts.values()) | {'omp', 'pi'}):
            options += f'[[agents]]\nid = "{host}"\nlabel = "{host}"\n'
            entries = [('native-observer', self.observer), ('herdr-state', getattr(self.args, host + '_state_extension')),
                       ('manager-chat', self.deployment / f'entrypoints/{host}-auto/manager-chat.ts'),
                       ('node-chat', self.deployment / f'entrypoints/{host}-auto/node-chat.ts')]
            if host == 'pi':
                entries.append(('pi-suite', self.deployment / 'entrypoints/pi/pi-suite.ts'))
            for name, path in entries:
                require(path.is_file(), ('Missing actual role/fixed entrypoint', str(path)))
                options += f'[[extensions]]\nid = "{host}-{name}"\nlabel = "{name}"\nagent = "{host}"\nkind = "fixed"\nentrypoint = {q(path)}\n'
        options += '[[skills]]\nid = "grill-with-docs"\nlabel = "/grill-with-docs"\nnative_name = "grill-with-docs"\n'
        (self.root / 'options.toml').write_text(options, encoding='utf-8')
        herdr_config = self.root / 'herdr.toml'
        herdr_config.write_text('onboarding = false\n[terminal]\ndefault_shell = "cmd.exe"\n[update]\nversion_check = false\nmanifest_check = false\n', encoding='utf-8')
        self.env['HERDR_CONFIG_PATH'] = str(herdr_config)
        herdr_process = self.start([self.args.herdr, 'server'], 'herdr')
        def herdr_ready():
            require(herdr_process.child.poll() is None, 'Owned Herdr server exited; see raw startup stdout/stderr')
            return subprocess.run([str(self.args.herdr), 'workspace', 'list'],
                cwd=self.root, env=self.env, capture_output=True, timeout=5).returncode == 0
        until('exclusive named Herdr', herdr_ready)
        self.herdr_started = True
        self.evidence['workspace'] = self.herdr('workspace', 'create', '--cwd', str(self.root / 'zone'), '--label', 'zone', '--no-focus')
        (self.root / 'meta-data').mkdir()
        (self.root / 'meta-data/suites.json').write_text('{"suites":[]}', encoding='utf-8')
        (self.root / 'meta-data/mcp.json').write_text('{"mcpServers":{}}', encoding='utf-8')
        config = f'listen_addr = "127.0.0.1:0"\nstatic_directory = {q(REPO / "backend/static")}\ndata_directory = {q(self.root / "data")}\n[mdns]\nenabled = false\n'
        for operation in ['start', 'stop']:
            config += f'[operations.{operation}]\nscript = {q(self.root / "unused.bat")}\nworking_directory = {q(self.root)}\n'
        config += f'[herdr]\nworkspace_name = "zone"\nexecutable = {q(self.args.herdr)}\n[tasks]\nstorage_file = {q(self.root / "data/tasks.json")}\nmetadata_file = {q(self.root / "options.toml")}\n'
        config += f'[suites]\ndefinition_file = {q(self.root / "meta-data/suites.json")}\nmcp_file = {q(self.root / "meta-data/mcp.json")}\ngenerated_directory = {q(self.root / "generated")}\n'
        config += f'[sessions]\nhistory_file = {q(self.root / "data/history.json")}\n[zones]\nregistry_file = {q(self.root / "data/zones.json")}\n[scheduler]\n'
        config += ''.join(f'{key} = {q(value)}\n' for key, value in [('map_directory', self.root / 'maps'), ('bot_directory', self.root / 'bots'),
            ('run_directory', self.root / 'data/runs'), ('run_instructions', REPO / 'docs/workflow-runtime.md')])
        (self.root / 'backend.toml').write_text(config, encoding='utf-8')
        backend = self.start([self.args.backend, '--config', self.root / 'backend.toml'], 'backend')
        def address():
            require(backend.child.poll() is None, 'Owned backend exited; see continuously drained logs')
            while not backend.lines.empty():
                match = re.search(r'http://(127\.0\.0\.1:\d+)/', backend.lines.get_nowait())
                if match:
                    return 'http://' + match[1]
        self.base = until('numeric loopback backend', address)
        self.backend_wire = WireProxy(self.base, self.root / 'backend-wire')
        self.base = self.backend_wire.base
        (self.deployment / 'extensions/workflow-chat/config.yaml').write_text(
            f'endpoint: {self.base}\nrequestTimeoutMs: 30000\ncollectTimeoutMs: 5460000\n', encoding='utf-8')
        (self.root / 'prompts.local.toml').write_text(f'herdr-bot-path = {q(REPO.as_posix())}\nherdr-bot-api-url = {q(self.base)}\n', encoding='utf-8')
        self.provision()

    def save_task(self, value):
        result = self.http('/api/tasks', value)
        return next(task for task in result['tasks'] if task['name'] == value['name'])

    def provision(self):
        self.ids = {}
        for zone in ['work', 'child-manager', 'child-work']:
            folder = self.root / 'zone' / zone
            folder.mkdir()
            self.http('/api/zones', {'id': 'p03/' + zone, 'path': str(folder)})
        self.http('/api/zones', {'id': 'p03/planning', 'path': str(self.root / 'zone')})
        source = self.root / 'zone/work/source.csv'
        source.write_text('order_id,amount_cents\nA,1200\nB,300\n', encoding='utf-8')
        tasks = json.loads((REPO / 'meta-data/tasks.json').read_text(encoding='utf-8'))['tasks']
        suite = {'id': 'p03-planning', 'name': 'Explicit planning skills', 'agents': ['omp', 'pi'],
                 'skills': {'omp': [str(self.root / 'agent/skills')],
                            'pi': [str(self.root / 'agent/skills' / name) for name in ['grill-with-docs', 'grilling', 'domain-modeling']]},
                 'extensions': {}, 'prompts': [], 'lsp': {}, 'mcp': [], 'pi': {'codemode': 'on'}}
        self.http('/api/suites', suite)
        for name in ['概要规划', '概要方案转 Map', 'manager']:
            original = next(task for task in tasks if task['name'] == name)
            deployed = {key: value for key, value in original.items() if key != 'id'}
            deployed.update(model='probe', thinking='medium', suite=suite['id'] if original.get('suite') else None)
            deployed['agent'] = self.hosts[name]
            saved = self.save_task(deployed)
            self.ids[name] = saved['id']
            self.evidence.setdefault('taskDeployment', []).append({'original': original, 'deployed': saved,
                'hostOverride': original['agent'] != deployed['agent']})
        # These business targets retain their independently declared different
        # hosts. They are verified/preflighted, never launched by acceptance.
        for key, prompt, host in [('prepare', 'Prepare a local copy of the named CSV without changing amounts. No review or upload. TARGET-PREPARE', 'pi'),
                                  ('review', 'Review CSV rows locally for duplicate order IDs and negative amounts. Never upload. TARGET-REVIEW', 'omp'),
                                  ('upload', 'Upload approved invoices to a remote production system. Requires separate credentials and authorization. TARGET-UPLOAD', 'pi')]:
            saved = self.save_task({'name': 'P03 ' + key, 'taskType': 'normal', 'agent': host, 'chatName': '{chat.id}',
                'model': 'probe', 'thinking': 'medium', 'skill': None, 'prompt': prompt})
            self.ids[key] = saved['id']
        candidates = [
            ('csv-prepare', 'Order review prepare', {'task_id': self.ids['prepare']},
             'Use for local CSV preparation only; not a duplicate/negative-value review.',
             'Arrange one preparation work. Takes existing CSV; output orders_csv.',
             'State source material, preserve amount_cents exactly, deliver orders_csv. No upload.'),
            ('csv-review', 'Order review', {'map_file': 'local-review.json'},
             'Use for a local duplicate-order/negative-amount review of CSV, without remote calls.',
             'Reuse this Map as ONE node. Supply source and a manager; output review_result. Do not flatten internal work.',
             'State duplicate order IDs and negative amounts checks, read named source, deliver review_result; no upload.'),
            ('csv-review-publish', 'Order review local', {'task_id': self.ids['upload']},
             'Use only to publish invoices remotely; NOT local validation or preparation.',
             'Requires real destination, credentials and production publishing authorization.',
             'Never interpret local-only approval as permission to upload.'),
        ]
        for bot, name, target, when, plan, node in candidates:
            guides = {key: text + ' ' + self.marker + '-' + bot + '-' + key for key, text in
                      [('when', when), ('plan_guide', plan), ('node_guide', node)]}
            value = {'id': bot, 'name': name, **target, **guides}
            (self.root / 'bots' / (bot + '.json')).write_text(json.dumps(value, ensure_ascii=False), encoding='utf-8')
        child = {'title': 'Local two-stage order review', 'description': 'Local only. Internal work is not parent work.',
                 'input_files': [{'id': 'source', 'required': True}], 'nodes': {
                     'scan': {'title': 'Scan', 'bot_id': 'csv-prepare', 'zone': 'p03/child-work', 'input': 'Read source for local scan.',
                              'blocked_by': [], 'input_rule': [{'id': 'source', 'type': 'run', 'input_id': 'source'}], 'output_rule': [{'id': 'scanned'}]},
                     'check': {'title': 'Check', 'bot_id': 'local-check', 'zone': 'p03/child-work', 'input': 'Check duplicates and negative amounts locally.',
                               'blocked_by': ['scan'], 'input_rule': [{'id': 'source', 'type': 'node', 'node_id': 'scan', 'output_id': 'scanned'}],
                               'output_rule': [{'id': 'review'}]}}}
        (self.root / 'bots/local-check.json').write_text(json.dumps({'id': 'local-check', 'name': 'Internal local check',
            'task_id': self.ids['review'], 'when': 'Internal checking step only, not the complete reusable review.',
            'plan_guide': 'Used by local-review.json.', 'node_guide': 'Check duplicates and negatives.'}), encoding='utf-8')
        (self.root / 'maps/local-review.json').write_text(json.dumps(child), encoding='utf-8')
        # Public strict validation also catches invalid test fixtures before a
        # model can accidentally paper over them.
        self.http('/api/scheduler/maps/local-review.json')
        self.baseline = self.definitions()
        self.evidence['independentRequirements'] = {
            'task': {'nodes': ['prepare'], 'bot': 'csv-prepare', 'output': 'orders_csv'},
            'map': {'nodes': ['prepare', 'review'], 'bot': 'csv-review', 'manager': self.ids['manager'],
                    'zone': 'p03/child-manager', 'source': {'id': 'source', 'type': 'node', 'node_id': 'prepare', 'output_id': 'orders_csv'},
                    'output': 'review_result', 'internalZone': 'p03/child-work'},
            'gap': 'No real publication endpoint, credentials, production authorization or target decision. G1 remains blocked.'}
        (self.root / 'independent-requirements.json').write_text(json.dumps(self.evidence['independentRequirements'], ensure_ascii=False, indent=2), encoding='utf-8')

    def definitions(self):
        return {str(path): digest(path) for path in [self.root / 'data/tasks.json', self.root / 'meta-data/suites.json',
            *sorted((self.root / 'bots').glob('*.json')), self.root / 'maps/local-review.json']}

    def events(self):
        path = self.root / 'native-events.jsonl'
        return [json.loads(line) for line in path.read_text(encoding='utf-8').splitlines()] if path.exists() else []

    def native_turn(self, chat, previous_ends=0):
        def ended():
            events = [event for event in self.events() if event['id'] == chat['sessionId'] and event['event'] == 'agent_end']
            return events[-1] if len(events) > previous_ends else None
        event = until('real native planning turn ends', ended, seconds=self.args.timeout)
        self.capture()
        path = Path(event['file'])
        require(path.is_relative_to(self.root) and path.is_file(), ('Native storage escaped isolation', str(path)))
        self.agent = chat['agent']
        history = self.history(str(path))
        require(history['id'] == chat['sessionId'], ('Wrong native identity', event, chat))
        for entry in history['branch']:
            message = entry.get('message', {})
            for call in message.get('content', []) if isinstance(message.get('content'), list) and message.get('role') == 'assistant' else []:
                if isinstance(call, dict) and call.get('type') == 'toolCall':
                    # An OS observation or a peer message can legitimately
                    # quote the driver command line without reading its source.
                    path_argument = call.get('arguments', {}).get('path', '') if call.get('name') == 'read' else ''
                    require(not any(name in path_argument.lower() for name in ['workflow_nesting_planning_acceptance.py',
                        'accept-workflow-nesting-planning.py', 'independent-requirements.json']),
                        ('Native planner read the acceptance oracle instead of business evidence', call))
        require(event['model'] == {'provider': self.model_config['provider'], 'id': self.model_config['model']}, ('Wrong actual model', event))
        destination = self.root / 'native-evidence'
        destination.mkdir(exist_ok=True)
        copied = destination / (chat['sessionId'] + '-' + str(len([e for e in self.events() if e['id'] == chat['sessionId']])) + '.jsonl')
        shutil.copy2(path, copied)
        self.evidence.setdefault('nativeHistories', []).append({'chat': chat, 'native': history, 'raw': str(copied),
            'sha256': digest(copied), 'observedAt': time.monotonic()})
        return history

    def launch(self, task_name, user):
        self.chat_count += 1
        started_at = time.monotonic()
        prior = {event['id'] for event in self.events() if event['event'] == 'session_start'}
        preview = self.http('/api/chats/preview', {'zone': 'p03/planning', 'taskId': self.ids[task_name]})
        launched = self.http('/api/chats', {'zone': 'p03/planning', 'taskId': self.ids[task_name], 'chatId': preview['chatId'], 'input': user}, 202)
        self.evidence['inputs'].append({'task': task_name, 'text': user, 'launch': launched, 'startedAt': started_at})
        def created():
            value = self.http('/api/chats/' + launched['id'])
            require(value['stage'] != 'failed', value)
            return value if value['stage'] == 'succeeded' else None
        creation = until('public native Chat creation', created, seconds=self.args.timeout)
        def observed():
            starts = [event for event in self.events() if event['event'] == 'session_start' and event['id'] not in prior]
            # The owned Chat's session_start precedes its model and any legal
            # read-only subagent research; subagents are not new Herdr Chats.
            return starts[0] if starts else None
        native = until('actual native Session identity', observed, seconds=self.args.timeout)
        self.remember_process(native['pid'])
        chat = {'sessionId': native['id'], 'paneId': creation['paneId'], 'agent': self.hosts[task_name]}
        self.evidence['inputs'][-1]['chat'] = chat
        return chat, self.native_turn(chat)

    def prompt(self, chat, user):
        count = len([event for event in self.events() if event['id'] == chat['sessionId'] and event['event'] == 'agent_end'])
        self.evidence['inputs'].append({'chat': chat, 'text': user})
        self.herdr('agent', 'prompt', chat['paneId'], user)
        return self.native_turn(chat, count)

    def tool_evidence(self, histories, bot, target_marker):
        # Only native tool results count. Assistant assertions and directory
        # summaries cannot supply the three-guide or target evidence.
        results = [entry['message'] for history in histories for entry in history['branch']
                   if entry.get('message', {}).get('role') in ['toolResult', 'tool']]
        def native_text(result):
            content = result.get('content', '')
            return content if isinstance(content, str) else '\n'.join(part.get('text', '') for part in content if isinstance(part, dict))
        guide_reads = {}
        for key in ['when', 'plan_guide', 'node_guide']:
            marker = self.marker + '-' + bot + '-' + key
            found = [index for index, result in enumerate(results) if marker in native_text(result)
                     and re.search(r'"' + key + r'\\?"\s*:', native_text(result)) and not result.get('isError')]
            require(found, ('Missing actual native full-guide JSON read, not a plan quotation', bot, key))
            guide_reads[key] = found
        target_field = 'input_files' if bot == 'csv-review' else 'prompt'
        require(any(target_marker in native_text(result) and re.search(r'"' + target_field + r'\\?"\s*:', native_text(result))
                    and not result.get('isError') for result in results),
                ('Native planner/converter did not verify actual target JSON', bot, target_marker))
        # Re-reading an outline that quotes guide markers is not re-reading the
        # actual Bot/target. Require their real public responses in this Chat's
        # own launch window, separately from the native tool-result observation.
        original_input = next(item for item in self.evidence['inputs']
                              if item.get('chat', {}).get('sessionId') == histories[0]['id'] and 'task' in item)
        end = next(item['observedAt'] for item in self.evidence['nativeHistories'] if item['native'] is histories[-1])
        wire = [item for item in self.backend_wire.records if original_input['startedAt'] <= item['started'] <= end and item.get('status') == 200]
        guides_wire = [item['index'] for item in wire if item['path'].split('?', 1)[0] in ['/api/scheduler/bots', '/api/scheduler/bots/edit']
                       and self.marker + '-' + bot + '-when' in item.get('response', '')]
        target_path = '/api/scheduler/maps/local-review.json' if bot == 'csv-review' else '/api/tasks'
        targets_wire = [item['index'] for item in wire if item['path'] == target_path and target_marker in item.get('response', '')]
        require(guides_wire and targets_wire, ('Missing actual Bot/target HTTP read in this native Chat', bot))
        guide_reads['http'] = {'guides': guides_wire, 'target': targets_wire}
        # No guide is a premise of initialization. It must enter via the native
        # reads above, not an automatic product injection at first model request.
        for history in histories:
            original = next(item['text'] for item in self.evidence['inputs']
                            if item.get('chat', {}).get('sessionId') == history['id'] and 'task' in item)
            initial = next((json.loads(record['request']) for record in self.model_wire.records
                            if any(item.get('role') == 'user' and original in body_text(item.get('content', ''))
                                   for item in json.loads(record['request']).get('messages', []))), None)
            require(initial is not None, ('No attributable actual business initialization request', history['id']))
            require(not any(self.marker in body_text(message.get('content', '')) for message in initial['messages']
                            if message.get('role') in ['system', 'developer', 'user']),
                    ('Bot guides were automatically injected', history['id']))
        return guide_reads

    def no_start(self):
        records = self.backend_wire.records
        forbidden = [record for record in records if record['method'] == 'POST' and
                     (record['path'].endswith('/runs') or '/api/workflow/manager/' in record['path'] or
                      record['path'] in ['/api/workflow/node/report', '/api/tasks', '/api/suites',
                                         '/api/scheduler/bots/save', '/api/scheduler/bots/delete']
                      and record['started'] >= self.observation_start)]
        require(not forbidden, ('Planning launched work or built capabilities', forbidden))
        launches = [record for record in records if record['method'] == 'POST' and record['path'] == '/api/chats']
        require(len(launches) == self.chat_count, ('Planning automatically requested an extra Chat', launches))
        runs = self.http('/api/workflow/runs')['runs']
        require(runs == [], ('Planning created a Run', runs))
        # Preserve all Session observations, including allowed read-only native
        # subagents. Actual new Herdr Chat requests are counted above; a native
        # subagent Session within a Chat is not another Chat or business Run.
        created = {event['id'] for event in self.events() if event['event'] == 'session_start'}
        require(self.definitions() == self.baseline, 'Planner modified existing Task/Bot/child Map definitions')
        self.capture()
        return {'runs': runs, 'nativeSessions': sorted(created), 'observedHttpRequests': len(records), 'at': time.monotonic()}

    def verify_map(self, scenario, raw):
        value = json.loads(raw)
        expected = {'prepare'} if scenario == 'task' else {'prepare', 'review'}
        require(set(value['nodes']) == expected, ('Wrong business work partition or expanded child graph', value))
        prepare = value['nodes']['prepare']
        require(prepare['bot_id'] == 'csv-prepare' and prepare['zone'] == 'p03/work' and prepare['blocked_by'] == [], prepare)
        require(not prepare.get('manager_task_id'), 'Task node has a Map manager')
        require(prepare['input_rule'] == [{'id': 'source', 'type': 'file', 'zone_id': 'p03/work', 'path': 'source.csv'}], prepare)
        require(prepare['output_rule'] == [{'id': 'orders_csv'}], prepare)
        require('amount_cents' in prepare['input'] and 'P03-LOCAL-ONLY' in prepare['input'], ('Business-specific instructions lost', prepare))
        if scenario == 'map':
            review = value['nodes']['review']
            require(review['bot_id'] == 'csv-review' and review['manager_task_id'] == self.ids['manager'], review)
            require(review['zone'] == 'p03/child-manager' and review['blocked_by'] == ['prepare'], review)
            require(review['input_rule'] == [{'id': 'source', 'type': 'node', 'node_id': 'prepare', 'output_id': 'orders_csv'}], review)
            require(review['output_rule'] == [{'id': 'review_result'}], review)
            require(all(text in review['input'] for text in ['order_id', 'amount_cents', 'P03-LOCAL-ONLY']), ('Map business input lost', review))
        return value

    def scenario(self, kind):
        filename = 'p03-' + kind + '.json'
        outline = self.root / 'zone' / ('p03-' + kind + '.md')
        context = (f'隔离业务规划 {kind}，方案 ID P03-{kind}，修订 R1，概要方案保存 {outline}。'
            '业务仅处理已存在 p03/work 的 source.csv；不得运行任何业务、联网发布、补建 Task/Bot 或启动 Run/Chat。'
            '不得读取认证、模型配置、原生存储、其他 Chat 或验收 driver；仅调查公开定义、产品契约和业务材料。'
            '用户要求本次指示保留 P03-LOCAL-ONLY 标记。W1/node prepare：本地准备 CSV，amount_cents 原值保留，'
            '输入 id=source 来自直接文件 zone_id=p03/work,path=source.csv；对外交付 orders_csv，工作 zone=p03/work。'
            '请按真实职责调查候选 Bot，名称相似不能代替三份指南和真实目标；没有让你使用特定 Bot ID。')
        if kind == 'map':
            context += (f'W2/node review：复用完整本地核对流程，核对 order_id 重复和 amount_cents 负数，不上传。'
                f'本次子管理者 Task ID={self.ids["manager"]}，zone=p03/child-manager；子接口 source 取 W1 的 orders_csv，'
                '对外交付 review_result。W2 依赖 W1，保留为一个流程调用，不展开内部工作，不改变子工作 zone。')
        context += (f'外层业务根启动配置已明确：managerTaskId={self.ids["manager"]},zone=p03/planning,userInput=""。'
            f'最终生成文件名 {filename}，但你仅交付概要方案，不生成 JSON。当前未确认 R1，先交付完整九章节草案，'
            '在本轮停止等待用户确认；所有业务取舍已在上述要求确定，不把指南或配置当成业务已执行证据。')
        before = len(self.backend_wire.records)
        chat, draft_history = self.launch('概要规划', context)
        require(outline.is_file(), ('No complete draft at requested path', str(outline)))
        draft = outline.read_bytes()
        require(not (self.root / 'maps' / filename).exists(), 'Map generated before user confirmation')
        self.no_start()
        confirmation = f'P03-CONFIRM-{kind}-R1：用户明确确认刚才 {outline} 的当前 R1 业务方案（sha256={digest(outline)}），范围不变。仅更新确认依据，完整读回并交付；不生成 Map、不启动业务。'
        confirmed_history = self.prompt(chat, confirmation)
        confirmed = outline.read_bytes()
        confirmed_text = confirmed.decode('utf-8')
        require(confirmation.split('：')[0] in confirmed_text, 'Confirmed plan lacks actual confirmation reference')
        require(any(word in confirmed_text for word in ['未验证', '未执行', '尚未运行', '未实际']),
                'Confirmed plan treats capability configuration as actual business execution evidence')
        generator_input = (f'用户已确认的独立概要方案为 {outline}，确认 {confirmation.split("：")[0]}。请完整读取并忠实转换，'
            f'本次隔离后端已显式用 --config {self.root / "backend.toml"} 启动，允许读取该业务部署配置核实真实目录；'
            f'仅新增 {filename}；按实际公开接口严格校验、完整读回及只读 preflight，使用方案内明确的根配置，'
            '不启动 Run/Chat 或业务，不修改被调用 Map/Bot/Task。不得读取认证、模型配置、原生存储、其他 Chat 或验收 driver。'
            '失败保留明确阻塞，不以声明代替执行证据。')
        generator, generated_history = self.launch('概要方案转 Map', generator_input)
        path = self.root / 'maps' / filename
        require(path.is_file(), ('No generated Map', str(path)))
        raw = path.read_bytes()
        value = self.verify_map(kind, raw)
        visible = self.http('/api/scheduler/maps/' + filename)
        self.verify_map(kind, json.dumps(visible))
        traffic = self.backend_wire.records[before:]
        require(any(item['method'] == 'GET' and item['path'] == '/api/scheduler/maps/' + filename and item.get('status') == 200 for item in traffic[:-1]),
                'Generator did not use public strict readback')
        preflights = [item for item in traffic if item['path'] == '/api/scheduler/maps/' + filename + '/preflight']
        require(len(preflights) == 1 and preflights[0].get('status') == 200, ('Missing/failed/retried native preflight', preflights))
        request = json.loads(preflights[0]['request'])
        require({key: request.get(key) for key in ['managerTaskId', 'zone', 'userInput']} ==
                {'managerTaskId': self.ids['manager'], 'zone': 'p03/planning', 'userInput': ''}
                and request.get('fileInputs', []) == [], request)
        require('id' not in json.loads(preflights[0]['response']), 'Preflight created a Run')
        reads = {'csv-prepare': self.tool_evidence([draft_history, confirmed_history], 'csv-prepare', 'TARGET-PREPARE'),
                 'converterPrepare': self.tool_evidence([generated_history], 'csv-prepare', 'TARGET-PREPARE')}
        if kind == 'map':
            reads['csv-review'] = self.tool_evidence([draft_history, confirmed_history], 'csv-review', 'Local two-stage order review')
            reads['converterReview'] = self.tool_evidence([generated_history], 'csv-review', 'Local two-stage order review')
        snapshots = self.no_start()
        self.evidence['scenarios'].append({'kind': kind, 'verified': True, 'planner': chat, 'generator': generator,
            'draftBytes': draft.decode('utf-8'), 'confirmedBytes': confirmed.decode('utf-8'), 'confirmation': confirmation,
            'map': str(path), 'mapBytes': raw.decode('utf-8'), 'sha256': digest(path), 'publicMap': visible,
            'guideReads': reads, 'preflightWire': preflights[0]['index'], 'noStartup': snapshots})

    def gap(self):
        before_maps = {path.name: digest(path) for path in (self.root / 'maps').glob('*.json')}
        path = self.root / 'zone/p03-gap.md'
        user = ('独立新场景 P03-GAP：要求把发票提交到真实生产系统，但具体系统和 endpoint 未决定，无实现代码、凭据或生产授权。'
                '既有本地准备和核对能力不能替代发布。目标、必要能力、授权和业务决定都缺失，必须保持 G1 阻塞。'
                f'只保存自包含能力建设需求到 {path}，不确认可翻译方案、不生成 Map、不创建或改动 Task/Bot、不调度创建者、不启动业务。'
                '不得读取认证、模型配置、原生存储、其他 Chat 或验收 driver。'
                '明确区分已知事实、未验证条件和未授权操作；本轮即可交付上述真实缺口，不猜测缺项、不要求真实用户补答。')
        chat, history = self.launch('概要规划', user)
        require(path.is_file(), ('No independent capability-blocker document', str(path)))
        body = path.read_text(encoding='utf-8')
        require('G1' in body and any(word in body for word in ['阻塞', 'blocked']) and
                all(any(word in body.lower() for word in alternatives) for alternatives in
                    [('授权', 'authorization'), ('凭据', 'credential'), ('未决定', '待定', '未指定', '未确定', 'unknown')]),
                ('Missing explicit unresolved capability/authorization/business decision', body))
        require(before_maps == {file.name: digest(file) for file in (self.root / 'maps').glob('*.json')}, 'Gap generated or modified a Map')
        self.evidence['scenarios'].append({'kind': 'gap', 'verified': True, 'planner': chat,
            'document': str(path), 'bytes': body, 'sha256': digest(path), 'noStartup': self.no_start()})

    def exercise(self):
        # Design: contract P03 selects by independent local-only requirements.
        # Violation: choosing the similar-name upload Task, or flattening the Map.
        # Native tool/target reads plus exact delivered nodes/mappings distinguish
        # the decisions; no controlled provider supplies the responsible choice.
        self.observation_start = time.monotonic()
        try:
            self.scenario('task')
            self.scenario('map')
            self.gap()
            self.evidence['coverage']['P03'] = {'verified': True, 'scenarios': ['task', 'map', 'gap'],
                'boundary': 'Only observed native model/HTTP/file/OS facts; does not assert future business execution quality.'}
            self.evidence['passed'] = True
        finally:
            # Capture descendants even when native creation/first turn fails;
            # the base lifecycle then stops and waits for every owned service.
            try:
                self.capture()
            except BaseException:
                self.evidence['cleanupErrors'].append(traceback.format_exc())

    def cleanup_extra(self):
        errors = []
        # Base cleanup has already stopped/waited for the owned Herdr and backend.
        for pid, handle in self.handles.items():
            try:
                if handle.alive():
                    argv = ['taskkill', '/PID', str(pid), '/T', '/F']
                    stopped = subprocess.run(argv, capture_output=True, text=True, encoding='utf-8', errors='replace', timeout=30)
                    self.evidence.setdefault('cleanupCommands', []).append({'argv': argv, 'exit': stopped.returncode,
                        'stdout': stopped.stdout, 'stderr': stopped.stderr})
                    # Wait for taskkill itself before the exit observation;
                    # detached native worker brokers remain owned by this run.
                    until('owned native process exits after cleanup', lambda: not handle.alive(), seconds=20)
                require(not handle.alive(), ('Owned native process survived cleanup', pid))
                self.evidence['processes'].append({'pid': pid, 'exited': True})
            except BaseException:
                errors.append(traceback.format_exc())
            finally:
                handle.close()
        for proxy in [self.backend_wire, self.model_wire]:
            if proxy:
                try:
                    proxy.close()
                except BaseException:
                    errors.append(traceback.format_exc())
        for name in ['models.json', 'models.yml']:
            try:
                (self.root / 'agent' / name).unlink(missing_ok=True)
            except BaseException:
                errors.append(traceback.format_exc())
        require(not errors, errors)


def run_scenarios(args, record):
    args.repo, args.node, args.bun = REPO, 'node', 'bun'
    resources['preflight'](args, record)
    roots = record['paths']['roots']
    for key in ['pi_state_extension', 'omp_state_extension', 'wayfinder_skills']:
        require(key in roots, 'Explicit isolated dependencies must name roots.' + key)
        setattr(args, key, Path(roots[key]))
        if key.endswith('_state_extension'):
            require(getattr(args, key).is_file(), ('Missing declared read-only state integration', key, roots[key]))
            record.setdefault('stateIntegrations', {})[key] = {'path': roots[key], 'sha256': digest(getattr(args, key))}
    args.shell_path = Path(roots['native_shell']) if roots.get('native_shell') else None
    require(args.model_config.is_file(), ('Missing explicitly authorized model configuration', str(args.model_config)))
    tasks = json.loads((REPO / 'meta-data/tasks.json').read_text(encoding='utf-8'))['tasks']
    declared = {name: next(task for task in tasks if task['name'] == name)['agent'] for name in ['概要规划', '概要方案转 Map', 'manager']}
    mappings = [declared] if args.hosts == 'declared' else [{name: host for name in declared} for host in ['omp', 'pi']]
    record['declaredHosts'] = declared
    record['executedHostMappings'] = mappings
    for mapping in mappings:
        scenario = NativePlanningAcceptance(args, mapping)
        code = scenario.run()
        record['scenarios'].append({'hosts': mapping, 'exit': code, 'evidence': str(scenario.root / 'evidence.json')})
        record['processes'].extend(scenario.evidence['processes'])
        record['cleanupErrors'].extend(scenario.evidence['cleanupErrors'])
    require(all(item['exit'] == 0 for item in record['scenarios']), ('Native P03 scenarios did not all pass', record['scenarios']))
