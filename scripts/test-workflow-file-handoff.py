"""S2 workflow file handoff: real Herdr, native Pi, actual model, paired source trees.

Approved seams: production HTTP, native Chat/provider traffic, installed Herdr,
external business program and read-only runtime records. No Session is supplied
by the driver and no runtime record is edited. Controlled-host parameter coverage
is separately owned by toolkit's node-report-contract dock (OMP and Pi).

Independent oracle: obsolete execution output must not reach the consumer; a
current false report cannot complete; retained pendingReports cannot repeatedly
wake collect; a new true report wakes it, but only explicit manager actions can
complete and dispatch. Detailed business material remains in external files.
"""
import argparse
import hashlib
import http.server
import json
import os
from pathlib import Path
import runpy
import shutil
import subprocess
import sys
import threading
import time
import traceback
import urllib.error
import urllib.parse
import urllib.request

from developer_paths import REPO, backend_suffix, resolve_arguments

shared = runpy.run_path(str(Path(__file__).with_name('test-real-herdr-close.py')))
Acceptance = shared['Acceptance']
require = shared['require']
until = shared['until']


def copy_evidence(source, destination):
    """Retain the full owned tree, including Windows paths beyond MAX_PATH."""
    def native(path):
        absolute = os.path.abspath(path)
        if os.name != 'nt' or absolute.startswith('\\\\?\\'):
            return absolute
        if absolute.startswith('\\\\'):
            return '\\\\?\\UNC\\' + absolute[2:]
        return '\\\\?\\' + absolute
    # Keep copytree's exclusive destination semantics: an old/partial archive
    # must never be silently merged with a different invocation.
    shutil.copytree(native(source), native(destination))


class WireProxy:
    """Transparent loopback observer. Never chooses/replaces a model response.

    Keep bodies, timing and status, not Authorization headers. Each collect gets
    its own thread, so a blocked collect cannot prevent the report that wakes it.
    """
    def __init__(self, upstream, directory):
        self.upstream = upstream.rstrip('/')
        self.directory = directory
        directory.mkdir()
        self.records = []
        self.lock = threading.Lock()
        owner = self

        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *_):
                return

            def do_POST(self):
                self.forward()

            def do_GET(self):
                self.forward()

            def do_PUT(self):
                self.forward()

            def forward(self):
                raw = self.rfile.read(int(self.headers.get('Content-Length', '0')))
                with owner.lock:
                    index = len(owner.records)
                    record = {'index': index, 'method': self.command, 'path': self.path,
                              'started': time.monotonic(), 'request': raw.decode('utf-8')}
                    owner.records.append(record)
                response_body = bytearray()
                try:
                    request = urllib.request.Request(owner.upstream + self.path, data=raw if raw else None,
                        method=self.command, headers={key: value for key, value in self.headers.items()
                        if key.lower() not in ['host', 'connection', 'content-length', 'accept-encoding']})
                    try:
                        response = urllib.request.urlopen(request, timeout=330)
                    except urllib.error.HTTPError as error:
                        response = error
                    with response:
                        record['status'] = response.status
                        self.send_response(response.status)
                        for key in ['Content-Type', 'Cache-Control']:
                            if response.headers.get(key):
                                self.send_header(key, response.headers[key])
                        self.send_header('Connection', 'close')
                        self.end_headers()
                        while True:
                            chunk = response.read1(65536)
                            if not chunk:
                                break
                            response_body.extend(chunk)
                            self.wfile.write(chunk)
                            self.wfile.flush()
                except BaseException:
                    record['error'] = traceback.format_exc()
                    if 'status' not in record:
                        self.send_error(502, 'Actual upstream request failed; see isolated wire evidence')
                finally:
                    record['response'] = response_body.decode('utf-8', errors='replace')
                    record['ended'] = time.monotonic()
                    (owner.directory / f'{index:04d}.json').write_text(
                        json.dumps(record, ensure_ascii=False, indent=2), encoding='utf-8')
                    self.close_connection = True

        self.server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.base = f'http://127.0.0.1:{self.server.server_port}'

    def close(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=15)
        require(not self.thread.is_alive(), 'Wire observer did not stop')
        (self.directory / 'index.json').write_text(json.dumps(self.records, ensure_ascii=False, indent=2), encoding='utf-8')


def decode(record, field):
    try:
        return json.loads(record.get(field, ''))
    except ValueError:
        return None


def messages(record):
    body = decode(record, 'request') or {}
    return body.get('messages', [])


def text(value):
    return json.dumps(value, ensure_ascii=False)


class RolesAcceptance(Acceptance):
    def __init__(self, args):
        super().__init__(args, prefix='workflow-file-handoff-roles-')
        self.evidence.update(scenario='roles', promise=None, host='Pi',
            promises=['P4', 'P5', 'P6', 'P7', 'P8', 'P17', 'P20', 'P21', 'P22', 'P26'],
            observationWindowSeconds=args.timeout, retryPolicy='No business/model retries by the runner')
        self.backend_wire = None
        self.model_wire = None
        self.run_id = None
        self.marker = 'FILE-HANDOFF-' + self.env['HERDR_SESSION']
        self.evidence['marker'] = self.marker

    def configure_model(self):
        # Explicit acceptance-only configuration, not the user's agent state.
        require(self.args.model_config.is_file(), ('Missing isolated actual-model configuration', str(self.args.model_config)))
        config = json.loads(self.args.model_config.read_text(encoding='utf-8'))
        for key in ['provider', 'model', 'baseUrl', 'apiKeyEnv']:
            require(isinstance(config.get(key), str) and bool(config[key]), f'model config requires {key}')
        require(config.get('api', 'openai-completions') == 'openai-completions',
                'This recorder currently supports actual OpenAI-compatible chat-completions models only')
        parsed = urllib.parse.urlparse(config['baseUrl'])
        require(parsed.scheme in ['http', 'https'] and parsed.hostname and not parsed.username and not parsed.password and not parsed.query,
                'baseUrl must be an HTTP(S) service URL without inline credentials or query')
        credential = os.environ.get(config['apiKeyEnv'])
        require(credential, f"Missing actual model credential environment variable {config['apiKeyEnv']}")
        self.model_wire = WireProxy(config['baseUrl'], self.root / 'model-wire')
        self.provider_model = config['provider'] + '/' + config['model']
        self.evidence['model'] = {key: config[key] for key in ['provider', 'model', 'baseUrl']}
        self.evidence['model']['credentialEnvironmentName'] = config['apiKeyEnv']
        self.evidence['model']['credentialFingerprint'] = hashlib.sha256(credential.encode()).hexdigest()[:12]
        # Remove even a nonstandard credential variable from the inherited child environment.
        self.env.pop(config['apiKeyEnv'], None)
        agent = self.root / 'agent'
        settings = {'sessionDir': str(agent / 'sessions')}
        if self.args.shell_path:
            shell = self.args.shell_path.resolve()
            require(shell.is_file(), ('Missing explicit native shell', str(shell)))
            settings['shellPath'] = str(shell)
            self.evidence['nativeShell'] = str(shell)
        (agent / 'settings.json').write_text(json.dumps(settings), encoding='utf-8')
        (agent / 'models.json').write_text(json.dumps({'providers': {config['provider']: {
            'baseUrl': self.model_wire.base, 'api': 'openai-completions', 'apiKey': credential,
            'models': [{'id': config['model'], 'name': 'Actual role acceptance model',
                'reasoning': config.get('reasoning', False), 'input': ['text'],
                'contextWindow': config.get('contextWindow', 128000), 'maxTokens': config.get('maxTokens', 8192),
                'cost': {'input': 0, 'output': 0, 'cacheRead': 0, 'cacheWrite': 0}}]}}}), encoding='utf-8')

    def prepare(self):
        require(self.args.backend == (REPO / backend_suffix()).resolve(),
                'Build/use this worktree backend with the same independent CARGO_TARGET_DIR; an unrelated executable is not accepted')
        for name, root, lock in [('herdr-bot', REPO, 'backend/Cargo.lock'), ('toolkit', self.args.toolkit, 'bun.lock')]:
            require((root / lock).is_file(), ('Missing participating source dependency lock', str(root)))
            commands = {}
            for label, argv in [('revision', ['git', 'rev-parse', 'HEAD']), ('status', ['git', 'status', '--porcelain']),
                                ('diff', ['git', 'diff', '--binary', 'HEAD'])]:
                result = subprocess.run(argv, cwd=root, capture_output=True, text=True, encoding='utf-8', errors='replace', check=True)
                commands[label] = result.stdout
            self.evidence.setdefault('sources', {})[name] = {'root': str(root), **commands,
                'lockSha256': hashlib.sha256((root / lock).read_bytes()).hexdigest()}
        require(self.args.backend.is_file(), ('Missing current worktree backend', str(self.args.backend)))
        self.evidence['backend'] = {'path': str(self.args.backend), 'sha256': hashlib.sha256(self.args.backend.read_bytes()).hexdigest()}
        for name, argv in [('python', [sys.executable, '--version']), ('node', [self.args.node, '--version']),
                           ('bun', [self.args.bun, '--version']),
                           ('pi', [self.args.node, self.args.pi_package / 'dist/bundle/cli.js', '--version'])]:
            self.evidence.setdefault('versions', {})[name] = self.command(argv)
        # rustup's toolchain selection belongs to the source build environment,
        # not the deliberately empty native Chat HOME. This version query starts no service.
        cargo = subprocess.run(['cargo', '--version'], cwd=REPO, capture_output=True,
                               text=True, encoding='utf-8', errors='replace', check=True)
        self.evidence['versions']['cargo'] = cargo.stdout
        super().prepare()
        self.backend_wire = WireProxy(self.base, self.root / 'backend-wire')
        (self.root / 'toolkit/extensions/workflow-chat/config.yaml').write_text(
            f'endpoint: {self.backend_wire.base}\nrequestTimeoutMs: 30000\nreportConfirmationTimeoutMs: 20000\ncollectTimeoutMs: 5460000\n', encoding='utf-8')
        self.run_contract_dock()
        self.write_business_program()

    def run_contract_dock(self):
        argv = [shutil.which(self.args.bun) or self.args.bun, 'test', 'tests/harness-dock/node-report-contract.test.ts']
        environment = dict(self.env, HERDR_BOT_REPO=str(REPO),
            CARGO_TARGET_DIR=str(self.args.backend.parent.parent),
            WORKFLOW_PROMPT_EVIDENCE_DIR=str(self.root / 'dock'))
        if os.environ.get('OMP_BASELINE_RECORD_ROOT'):
            environment['OMP_BASELINE_RECORD_ROOT'] = os.environ['OMP_BASELINE_RECORD_ROOT']
        with (self.root / 'dock.log').open('w', encoding='utf-8') as log:
            result = subprocess.run(argv, cwd=self.args.toolkit, env=environment, stdout=log,
                                    stderr=subprocess.STDOUT, timeout=660)
        self.evidence['contractDock'] = {'argv': argv, 'cwd': str(self.args.toolkit),
            'exit': result.returncode, 'log': str(self.root / 'dock.log'), 'scope': 'S1 real OMP/Pi; controlled provider/Herdr; not S2 model obedience'}
        require(result.returncode == 0, ('OMP/Pi report parameter/persistence dock failed', self.evidence['contractDock']))

    def write_business_program(self):
        # This external program owns business data, not Run/execution records.
        source = '''import json, pathlib, sys, time
root = pathlib.Path(__file__).resolve().parent
mode = sys.argv[1]
if mode == "produce":
    old = not (root / "attempt.json").exists()
    (root / "attempt.json").write_text(json.dumps({"started": True}))
    output = root / ("obsolete.csv" if old else "current.csv")
    output.write_text("DETAIL-SENSITIVE-MARKER\\n" + "\\n".join(f"row-{i},business-value-{i * 17}" for i in range(200)), encoding="utf-8")
    print(json.dumps({"phase": "obsolete" if old else "partial", "output": str(output), "effect": "200 business rows materialized", "blocker": None if old else "release approval not yet present"}))
elif mode == "wait":
    deadline = time.monotonic() + 240
    while not (root / "release-approved.json").exists():
        if time.monotonic() >= deadline:
            raise TimeoutError("No isolated business approval in bounded observation window")
        time.sleep(0.1)
    print(json.dumps({"phase": "ready", "output": str(root / "current.csv"), "effect": "business approval observed", "blocker": None}))
elif mode == "consume":
    output = root / "consumer-receipt.txt"
    output.write_text("Independent consumer receipt; upstream body was not copied.\\n", encoding="utf-8")
    print(json.dumps({"output": str(output), "effect": "independent consumer receipt generated"}))
else:
    raise ValueError(mode)
'''
        (self.root / 'zone/business.py').write_text(source, encoding='utf-8')

    def detail(self):
        return self.http('/api/workflow/runs/' + self.run_id)

    def report_calls(self):
        return [record for record in self.backend_wire.records if record['path'] == '/api/workflow/node/report']

    def exercise(self):
        zone = str(self.root / 'zone')
        background = self.marker + '-BACKGROUND'
        business = self.marker + '-BUSINESS-INPUT'
        report_secret = self.marker + '-REPORT-BODY-NOT-INPUT'
        script = subprocess.list2cmdline([sys.executable, str(self.root / 'zone/business.py')])
        self.http('/api/zones', {'id': 'acceptance/roles', 'path': zone})
        manager_prompt = (
            'This is an isolated, approved automated business rehearsal. Use your actual workflow tools, not HTTP or shell substitutes. '
            'Dispatch A. The FIRST execution produces an obsolete specimen and reports true: explicitly fail A, '
            'then retry A with confirmed=true. This replacement is explicitly business-approved; preserve all partial effects. '
            'For the replacement execution, wait for its partial false report, then independently attempt complete A ONCE '
            'to exercise the backend completion refusal (this one diagnostic attempt is explicitly requested). '
            'Read the refusal and follow the public waiting instructions: use collect and never repeatedly complete the unchanged report. '
            'When a new true report arrives, explicitly complete A and dispatch B. After B reports true, complete B and finish. '
            'Make orchestration decisions from reports; do not read the output files for default acceptance. '
            'Do not inspect driver scripts, other Chats, credentials or runtime storage. Do not use read/bash tools for orchestration.')
        node_prompt = (
            'Perform only your assigned business input using the external business program. Never read the program source, '
            'output file contents, other Chats or runtime records. Detailed results belong only in external files. '
            'Use the actual report tool; public role instructions still apply. Report concise conclusions, blockers, partial effects '
            'and approval requirements, not rows or known identity metadata. The business fixture supplies all approval decisions; do not ask a human.')
        ids = {}
        for role, prompt in [('manager', manager_prompt), ('node', node_prompt)]:
            result = self.http('/api/tasks', {'name': role, 'taskType': 'manager' if role == 'manager' else 'normal',
                'agent': 'pi', 'chatName': '{chat.id}', 'model': 'probe', 'thinking': 'default',
                'skill': None, 'prompt': prompt})
            ids[role] = next(task['id'] for task in result['tasks'] if task['name'] == role)
        (self.root / 'bots/worker.json').write_text(json.dumps({'id': 'worker', 'name': 'Worker',
            'when': '', 'plan_guide': '', 'node_guide': '', 'task_id': ids['node']}), encoding='utf-8')
        producer = (
            f'Before work, write one brief assistant note containing {self.marker}-CHAT-PRIVATE-NOT-INPUT. '
            f'Run external command {script} produce. If phase=obsolete, report true with outputs=[{{"id":"handoff","path":output}}] and stop. '
            f'If phase=partial, report false with that same output ID, explain actual blocker and partial effects; '
            f'then run {script} wait (timeout at least 250 seconds) to await already-approved external business release. '
            f'After it returns phase=ready, report true, omitting outputs (retain the current registration). '
            f'In each A report include the orchestration diagnostic {report_secret} and the decoy address {zone}/decoy-not-an-output.csv. '
            'Do not include the detailed business material.')
        consumer = (f'{business}. Produce a separate receipt using {script} consume. '
                    'Do not read upstream file bodies to perform default acceptance. Report true, omitting outputs.')
        (self.root / 'maps/roles.json').write_text(json.dumps({'title': 'Isolated real role acceptance', 'description': '', 'nodes': {
            'A': {'title': 'External producer', 'bot_id': 'worker', 'input': producer, 'zone': 'acceptance/roles', 'blocked_by': [],
                  'output_rule': [{'id': 'handoff'}]},
            'B': {'title': 'Address consumer', 'bot_id': 'worker', 'input': consumer, 'zone': 'acceptance/roles', 'blocked_by': ['A'],
                  'input_rule': [{'id': 'primary', 'type': 'node', 'node_id': 'A', 'output_id': 'handoff'}]}}}), encoding='utf-8')
        started = self.http('/api/scheduler/maps/roles.json/runs',
            {'managerTaskId': ids['manager'], 'zone': 'acceptance/roles', 'userInput': background}, 201)
        self.run_id = started['id']
        self.evidence['started'] = started
        # All subsequent workflow actions are chosen by the actual model in native Chats.
        def gate_wait():
            records = self.backend_wire.records
            refusals = [record for record in records if record['path'] == '/api/workflow/manager/complete'
                        and (decode(record, 'response') or {}).get('changed') is False]
            waiting = [record for record in records if record['path'] == '/api/workflow/manager/collect'
                       and 'ended' not in record and time.monotonic() - record['started'] >= 1.25]
            if refusals and waiting:
                return refusals[-1], waiting[-1]
        refused, waiting = until('actual manager gate refusal followed by an in-flight collect', gate_wait, self.args.timeout)
        baseline = self.detail()
        reports = baseline['reports']
        current = baseline['run']['nodes']['A']['execution']
        latest = next(report for report in reversed(reports) if report['executionId'] == current['id'])
        require(latest['canComplete'] is False, ('Expected actual current partial false report', latest))
        refusal = decode(refused, 'response')
        require(refusal.get('code') == 'report-not-completable' and latest['id'] in refusal.get('message', ''),
                ('Manager must see the actual latest-report gate refusal and explicit reason', refusal))
        require(baseline['run']['nodes']['A']['status'] == 'running', baseline)
        require(baseline['pendingReports']['A'] == latest['id'], baseline)
        require(waiting['started'] > refused['started'], ('collect must follow refusal', refused, waiting))
        # An explicit bounded absence window; no retry or time extension on failure.
        time.sleep(1.25)
        require('ended' not in waiting, ('Retained report immediately woke collect again', waiting))
        require(sum(record['path'] == '/api/workflow/manager/complete' for record in self.backend_wire.records) == 1,
                'Manager repeatedly attempted complete before a new report')
        self.evidence['waiting'] = {'reportId': latest['id'], 'collectIndex': waiting['index'],
                                    'observedPendingSeconds': time.monotonic() - waiting['started'], 'snapshot': baseline}
        (self.root / 'zone/release-approved.json').write_text(json.dumps({'approved': True, 'scope': 'isolated business rehearsal'}), encoding='utf-8')
        def finished():
            detail = self.detail()
            return detail if detail['run']['status'] == 'completed' else None
        final = until('actual manager completes and dispatches after new report', finished, self.args.timeout)
        require('ended' in waiting, ('New report did not wake existing collect', waiting))
        wake = decode(waiting, 'response')
        require(wake and wake.get('reason') == 'notification', ('collect did not return new notification', waiting))
        new_id = wake['pendingReports']['A']
        require(new_id != latest['id'], ('collect returned same retained report', wake))
        new_report = next(report for report in final['reports'] if report['id'] == new_id)
        require(new_report['executionId'] == current['id'] and new_report['canComplete'] is True, new_report)
        require(all(final['run']['nodes'][node]['status'] == 'completed' for node in ['A', 'B']), final)
        self.evidence['final'] = final
        def wire_quiet():
            observed = self.backend_wire.records + self.model_wire.records
            return all('ended' in record for record in observed) and time.monotonic() - max(record['ended'] for record in observed) >= 1.25
        until('actual wire responses and bounded final conversation quiet window', wire_quiet, self.args.timeout)
        self.evidence['finalQuietWindowSeconds'] = 1.25
        self.verify_observations(final, baseline, latest, new_report, background, business, report_secret)
        self.evidence['passed'] = True

    def verify_observations(self, final, baseline, blocked, ready, background, business, report_secret):
        records = self.backend_wire.records
        node_a = final['run']['nodes']['A']
        node_b = final['run']['nodes']['B']
        require(len(node_a['previousExecutions']) == 1, ('Native retry did not create an independent execution', node_a))
        old = node_a['previousExecutions'][0]['execution']
        current = node_a['execution']
        require(current['id'] != old['id'], node_a)
        identities = [final['run']['manager'], old['chat'], current['chat'], node_b['execution']['chat']]
        require(len({chat['sessionId'] for chat in identities}) == 4, ('Native Session identities are not independent', identities))
        self.evidence['identities'] = identities
        for report in final['reports']:
            execution = next(execution for execution in [old, current, node_b['execution']] if execution['id'] == report['executionId'])
            require(report['runId'] == self.run_id and report['from']['sessionId'] == execution['chat']['sessionId'], report)
            require('DETAIL-SENSITIVE-MARKER' not in report['body'] and 'row-199,business-value-3383' not in report['body'], report)
            require(0 < len(report['body']) < 2000, ('Report is not a concise orchestration conclusion', report))
        for record in self.report_calls():
            request = decode(record, 'request') or {}
            response = decode(record, 'response') or {}
            report = response.get('report')
            require(report and request.get('sessionId') == report['from']['sessionId'],
                    ('Public Session/actual HTTP/saved report identity disagree', record))
        report_requests = [decode(record, 'request') for record in self.report_calls()]
        require(any(request.get('canComplete') is False for request in report_requests), report_requests)
        require(any(request.get('canComplete') is True and 'outputs' not in request for request in report_requests), report_requests)
        # Current true report intentionally omits outputs; current registration must survive.
        current_address = str(self.root / 'zone/current.csv')
        old_address = str(self.root / 'zone/obsolete.csv')
        model_records = self.model_wire.records
        consumer_records = [record for record in model_records if any(
            business in self.message_text(message) and '[Named file inputs]' in self.message_text(message)
            for message in messages(record))]
        require(consumer_records, 'No actual consumer provider request contains the independent business input')
        initial = messages(consumer_records[0])
        require(background in text(initial) and business in text(initial), 'Actual Chat lacks independent background/business input')
        serialized = text(initial).replace('\\\\', '\\')
        require(current_address in serialized, ('Actual Chat lacks current registered output address', initial))
        for forbidden in [old_address, report_secret, self.marker + '-CHAT-PRIVATE-NOT-INPUT', 'DETAIL-SENSITIVE-MARKER', 'decoy-not-an-output.csv']:
            require(forbidden not in serialized, ('Upstream body/chat/old execution/detail leaked into native initialization', forbidden))
        require(node_b['execution']['inputs'] == [{'id': 'primary', 'path': current_address}],
                ('Actual resolved input differs from the current execution address', node_b['execution']))
        require(node_b['execution']['chat']['sessionId'], 'Consumer node was not actually started')
        # Prove real public guides reached the model, not merely that prompt files exist.
        for role in ['manager', 'node']:
            guide = (self.args.toolkit / f'extensions/workflow-chat/{role}-chat/shared-logic/prompts/init.md').read_text(encoding='utf-8').strip()
            require(any(guide in '\n'.join(self.message_text(message).replace('\r\n', '\n')
                        for message in messages(record)) for record in model_records),
                    ('Public role guide did not reach actual model', role))
        histories = []
        module = (self.args.pi_package / 'dist/core/session-manager.js').as_uri()
        code = f'import {{SessionManager}} from {json.dumps(module)}; console.log(JSON.stringify(await SessionManager.list({json.dumps(str(self.root / "zone"))}, {json.dumps(str(self.root / "agent/sessions"))})));'
        native_files = json.loads(self.command([self.args.node, '--input-type=module', '-e', code]))
        for chat in identities:
            native = next(item for item in native_files if item['id'] == chat['sessionId'])
            file = Path(native['path'])
            require(file.is_relative_to(self.root), ('Native history escaped isolated storage', str(file)))
            histories.append({'chat': chat, 'path': str(file), 'native': self.history(str(file)),
                              'sha256': hashlib.sha256(file.read_bytes()).hexdigest()})
        self.evidence['nativeHistories'] = histories
        for history in histories[1:3]:
            require(any(entry.get('type') == 'message' and entry.get('message', {}).get('role') == 'assistant'
                        and self.marker + '-CHAT-PRIVATE-NOT-INPUT' in text(entry['message'].get('content'))
                        for entry in history['native']['branch']),
                    ('Independent private native chat sentinel was not actually produced', history['chat']))
        for history in histories[1:]:
            native_reports = [part for entry in history['native']['branch'] if entry.get('type') == 'message'
                              for part in entry.get('message', {}).get('content', [])
                              if isinstance(part, dict) and part.get('type') == 'toolCall' and part.get('name') == 'report']
            require(native_reports, ('Native report was never actually called', history['chat']))
            require(all(not any(key in call.get('arguments', {}) for key in ['sessionId', 'runId', 'nodeId', 'executionId'])
                        for call in native_reports), 'Model supplied identity instead of public Session injection')
        manager_history = histories[0]['native']['branch']
        calls = [part for entry in manager_history if entry.get('type') == 'message'
                 for part in entry.get('message', {}).get('content', []) if isinstance(part, dict) and part.get('type') == 'toolCall']
        completions = [call for call in calls if call.get('name') == 'run' and call.get('arguments', {}).get('action') == 'complete']
        require(len(completions) == 3, ('Expected one refusal, A complete, B complete; no repeated gate attempts', completions))
        for call in calls:
            require(call.get('name') in ['run', 'collect'], ('Manager default-read output or bypassed role tools', call))
        action_order = [(record['path'].rsplit('/', 1)[-1], decode(record, 'request'), decode(record, 'response'))
                        for record in records if record['path'].startswith('/api/workflow/manager/') and 'ended' in record]
        a_complete = next(record for record in records if record['path'].endswith('/manager/complete')
                          and (decode(record, 'request') or {}).get('nodeId') == 'A'
                          and (decode(record, 'response') or {}).get('changed') is True)
        b_dispatch = next(record for record in records if record['path'].endswith('/manager/dispatch')
                          and 'B' in (decode(record, 'request') or {}).get('nodes', []))
        require(a_complete['started'] < b_dispatch['started'], ('B dispatched without explicit A completion first', action_order))
        require((decode(a_complete, 'response') or {})['snapshot']['nodes']['B']['status'] == 'pending',
                'complete automatically dispatched downstream')
        require(ready['id'] in text(records), 'True report ID missing from actual wire evidence')
        for path in [self.root / 'zone/obsolete.csv', self.root / 'zone/current.csv', self.root / 'zone/consumer-receipt.txt']:
            require(path.is_file(), ('External business file was not produced', str(path)))
            self.evidence.setdefault('files', []).append({'path': str(path), 'size': path.stat().st_size,
                'sha256': hashlib.sha256(path.read_bytes()).hexdigest()})
        self.evidence['coverage'] = {
            'P4/P5/P6': {'initialProviderRequest': consumer_records[0]['index'], 'nativeSession': node_b['execution']['chat']['sessionId']},
            'P7/P8/P26': {'actualReportRequests': [record['index'] for record in self.report_calls()],
                          'malformedHostCalls': 'Separate required OMP/Pi node-report-contract dock; never inferred from real-model valid calls'},
            'P17': {'completeRequest': a_complete['index'], 'dispatchRequest': b_dispatch['index']},
            'P20': self.evidence['waiting'],
            'P21/P22': {'nativeHistories': 'nativeHistories', 'managerActions': action_order,
                        'blockedReport': blocked['id'], 'readyReport': ready['id'],
                        'rubric': 'External files exist; detailed sentinels absent from concise reports; actual injected public guides; manager uses only run/collect, one refusal then wait then explicit complete/dispatch'},
        }

    @staticmethod
    def message_text(message):
        content = message.get('content', '')
        if isinstance(content, str):
            return content
        return '\n'.join(part.get('text', '') for part in content if isinstance(part, dict)) if isinstance(content, list) else ''

    def cleanup_extra(self):
        errors = []
        for proxy in [self.backend_wire, self.model_wire]:
            if proxy:
                try:
                    proxy.close()
                except BaseException:
                    errors.append(traceback.format_exc())
        if self.run_id:
            records = self.root / 'data/runs' / self.run_id
            if records.is_dir():
                shutil.copytree(records, self.root / 'read-only-record-copy')
        # Model credentials are not evidence. Never retain the credential-bearing config.
        model_file = self.root / 'agent/models.json'
        if model_file.exists():
            model_file.unlink()
        require(not errors, errors)


class FanInAcceptance(RolesAcceptance):
    """P23/P24 at S2/S3: existing production handoff, never a mock provider.

    Independent source: literals fixed here before either Run. A plausible
    violation is a consumer guessing/copying the answer without reading, or a
    framework relocating a registered external file into a zone. Exact native
    read calls AND successful results at the original addresses distinguish it;
    the final bytes alone cannot. The driver never performs manager actions.
    """
    SPECIMENS = (
        {'source.txt': b'item=amber-widget\nunits=7\nunit_price=6\nfee_per_unit=5\n',
         'left.txt': b'item=amber-widget\nsubtotal=42\n',
         'right.txt': b'item=amber-widget\nfee=35\n',
         'summary.txt': b'item=amber-widget\nsubtotal=42\nfee=35\ntotal=77\n'},
        {'source.txt': b'item=violet-gear\nunits=9\nunit_price=4\nfee_per_unit=3\n',
         'left.txt': b'item=violet-gear\nsubtotal=36\n',
         'right.txt': b'item=violet-gear\nfee=27\n',
         'summary.txt': b'item=violet-gear\nsubtotal=36\nfee=27\ntotal=63\n'},
    )
    NODES = ('source', 'left', 'right', 'summary')

    def __init__(self, args):
        super().__init__(args)
        self.evidence.update(scenario='fan-in', promises=['P23', 'P24'], runs=[],
            coordinationInputs=[], oracle='Literal UTF-8/LF bytes fixed before launch; never computed from actual outputs')

    def write_business_program(self):
        # Business-only fixtures, not prepared handoff outputs: the native
        # source Chat must actually read its seed and write the registered file.
        # Native read/write avoids assuming Pi's Bash is a Windows shell.
        # No answers/expected bytes reach consumers or the manager.
        for name in self.NODES:
            (self.root / ('node-' + name)).mkdir()
        (self.root / 'business').mkdir()
        (self.root / 'oracle').mkdir()
        for index, expected in enumerate(self.SPECIMENS):
            business = self.root / 'business' / str(index)
            business.mkdir()
            seed = business / 'seed.txt'
            seed.write_bytes(expected['source.txt'])
            oracle = self.root / 'oracle' / str(index)
            oracle.mkdir()
            for name, content in expected.items():
                (oracle / name).write_bytes(content)
        self.evidence['coordinationInputs'].append({'kind': 'prelaunch business seeds and independent oracle',
            'source': 'SPECIMENS literals in acceptance runner', 'modelAnswersInjected': False})

    def exercise(self):
        manager_zone = self.root / 'zone'
        zones = {'manager': manager_zone, **{name: self.root / ('node-' + name) for name in self.NODES}}
        for name, path in zones.items():
            self.http('/api/zones', {'id': 'acceptance/' + name, 'path': str(path)})
        self.http('/api/zones', {'id': 'acceptance/business', 'path': str(self.root / 'business')})
        manager_prompt = (
            'This isolated business Run uses a saved source -> left/right -> summary Map. '
            'Use actual run/collect workflow tools only, never HTTP/bash/read substitutes. '
            'Explicitly dispatch source, wait for its true report and explicitly complete source. '
            'Then explicitly dispatch left and right. Wait for their true reports and explicitly complete both. '
            'Only then explicitly dispatch summary, wait for its true report, explicitly complete summary and finish. '
            'Use collect for new reports when needed. Reporting never completes or dispatches automatically. '
            'Decide orchestration from concise reports; do not read business files to do default acceptance. '
            'The summary node owns business merging/checks. No retries, fail/retry, Map edits or human approvals. '
            'Do not inspect scripts, other Chats, runtime records, credentials or oracle files.')
        node_prompt = (
            'Perform ONLY this node input. Named file inputs carry original registered addresses. '
            'For each required input use the native read tool at that exact address, then use native write '
            'to create your own result at the specified business address (UTF-8 without BOM, LF, trailing LF). '
            'Do not use bash to read/transform files: computations and merging are your assigned business work. '
            'Only the source node may read its explicitly named seed input, then write the handoff file itself. '
            'Never read other seeds, oracle files, runtime records or other Chats. '
            'Report using the actual report tool with can_complete=true, outputs=[{"id":"result","path":absolute output address}]. '
            'Keep the report a short orchestration conclusion without file bodies, numeric answers or identity metadata. '
            'All business permissions are preapproved; do not request human confirmation.')
        ids = {}
        for role, prompt in [('manager', manager_prompt), ('node', node_prompt)]:
            result = self.http('/api/tasks', {'name': role, 'taskType': 'manager' if role == 'manager' else 'normal',
                'agent': 'pi', 'chatName': '{chat.id}', 'model': 'probe', 'thinking': 'default',
                'skill': None, 'prompt': prompt})
            ids[role] = next(task['id'] for task in result['tasks'] if task['name'] == role)
        (self.root / 'bots/worker.json').write_text(json.dumps({'id': 'worker', 'name': 'Fan-in worker',
            'when': '', 'plan_guide': '', 'node_guide': '', 'task_id': ids['node']}), encoding='utf-8')
        definitions = []
        for index in range(len(self.SPECIMENS)):
            business = self.root / 'business' / str(index)
            inputs = {
                'source': f'Read your named seed input with native read. Write the exact same bytes to {business / "source.txt"} with native write, then register that new file as result. Do not include its body in a report.',
                'left': f'Read named input shared. From its units and unit_price compute subtotal=units*unit_price. Write {business / "left.txt"} with exactly item=VALUE then subtotal=VALUE, one line each.',
                'right': f'Read named input shared. From its units and fee_per_unit compute fee=units*fee_per_unit. Write {business / "right.txt"} with exactly item=VALUE then fee=VALUE, one line each.',
                'summary': f'Read BOTH named inputs left_result and right_result. Check their item values agree. Write {business / "summary.txt"} with exactly item=VALUE, subtotal=VALUE, fee=VALUE, total=VALUE, one line each; total=subtotal+fee. This node owns the business merge and content checks.',
            }
            nodes = {}
            for name in self.NODES:
                node = {'title': name, 'bot_id': 'worker', 'input': inputs[name], 'zone': 'acceptance/' + name,
                    'blocked_by': [] if name == 'source' else ['left', 'right'] if name == 'summary' else ['source'],
                    'output_rule': [{'id': 'result'}]}
                if name == 'source':
                    node['input_rule'] = [{'id': 'seed', 'type': 'file', 'zone_id': 'acceptance/business', 'path': f'{index}/seed.txt'}]
                elif name in ['left', 'right']:
                    node['input_rule'] = [{'id': 'shared', 'type': 'node', 'node_id': 'source', 'output_id': 'result'}]
                elif name == 'summary':
                    node['input_rule'] = [{'id': upstream + '_result', 'type': 'node', 'node_id': upstream, 'output_id': 'result'} for upstream in ['left', 'right']]
                nodes[name] = node
            definition = {'title': 'Isolated shared-file fan-in ' + str(index), 'description': '', 'nodes': nodes}
            path = self.root / 'maps' / f'fan-in-{index}.json'
            path.write_text(json.dumps(definition, ensure_ascii=False, indent=2), encoding='utf-8')
            definitions.append({'path': str(path), 'sha256': hashlib.sha256(path.read_bytes()).hexdigest(), 'definition': definition})
        self.evidence['savedMapsBeforeRuns'] = definitions
        self.evidence['coordinationInputs'].append({'kind': 'prelaunch Task prompts', 'manager': manager_prompt, 'node': node_prompt})
        for index, definition in enumerate(definitions):
            started = self.http(f'/api/scheduler/maps/fan-in-{index}.json/runs',
                {'managerTaskId': ids['manager'], 'zone': 'acceptance/manager',
                 'userInput': f'Isolated business fan-in Run {index}; follow the saved Map and explicit orchestration sequence.'}, 201)
            self.run_id = started['id']
            run_evidence = {'started': started, 'map': definition, 'zones': {name: str(path) for name, path in zones.items()}}
            self.evidence['runs'].append(run_evidence)
            def completed():
                detail = self.detail()
                return detail if detail['run']['status'] == 'completed' else None
            final = until('native manager explicitly finishes fan-in Run ' + str(index), completed, self.args.timeout)
            def quiet():
                records = self.backend_wire.records + self.model_wire.records
                return records and all('ended' in record for record in records) and time.monotonic() - max(record['ended'] for record in records) >= 1.25
            until('actual fan-in wire conversation quiet window', quiet, self.args.timeout)
            run_evidence['final'] = final
            self.verify_fan_in(index, final, run_evidence, zones)
            require(hashlib.sha256(Path(definition['path']).read_bytes()).hexdigest() == definition['sha256'], 'Saved Map changed during Run')
            shutil.copytree(self.root / 'data/runs' / self.run_id, self.root / ('read-only-record-copy-' + str(index)))
        locations = [run['materials'] for run in self.evidence['runs']]
        require(len(set(locations)) == 2, ('Different Runs share intermediate-material location', locations))
        self.evidence['coverage'] = {
            'P23': {'runs': [run['started']['id'] for run in self.evidence['runs']],
                'evidence': 'runs[].nativeHistories, reads, files, orchestration; fixed prelaunch oracle bytes; both consumers read shared source and summary reads both outputs'},
            'P24': {'materials': locations, 'evidence': 'runs[].zones, nativeHistories cwd, original external registrations, reads and files; no zone relocation'},
        }
        self.evidence['passed'] = True

    def verify_fan_in(self, index, final, evidence, zones):
        run = final['run']
        require(all(run['nodes'][name]['status'] == 'completed' for name in self.NODES), final)
        require(run['zone'] == str(zones['manager']), ('Actual manager zone differs', run))
        chats = {'manager': run['manager'], **{name: run['nodes'][name]['execution']['chat'] for name in self.NODES}}
        require(len({chat['sessionId'] for chat in chats.values()}) == 5, ('Real Chats are not independent', chats))
        business = self.root / 'business' / str(index)
        expected = self.SPECIMENS[index]
        evidence['files'] = []
        for name, content in expected.items():
            path = business / name
            require(path.is_file() and path.read_bytes() == content, ('Business bytes differ from fixed independent oracle', str(path)))
            evidence['files'].append({'path': str(path), 'hex': path.read_bytes().hex(), 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()})
        for name in self.NODES:
            execution = run['nodes'][name]['execution']
            require(not run['nodes'][name]['previousExecutions'], ('Unexpected node retry', name))
            reports = [report for report in final['reports'] if report['executionId'] == execution['id']]
            require(reports and reports[-1]['canComplete'] is True, ('Missing current execution true report', name))
            calls = [record for record in self.report_calls() if (decode(record, 'request') or {}).get('sessionId') == chats[name]['sessionId']]
            require(any((decode(call, 'request') or {}).get('outputs') == [{'id': 'result', 'path': str(business / (name + '.txt'))}] for call in calls),
                ('Original zone-external address was not registered by native report', name, calls))
            require(all(report['from']['sessionId'] == chats[name]['sessionId'] and report['runId'] == run['id'] for report in reports), reports)
            require(all(0 < len(report['body']) < 1500 and not any(line in report['body'] for content in expected.values()
                for line in content.decode().splitlines()) for report in reports), ('Report leaked business rows', reports))
        histories = {}
        module = (self.args.pi_package / 'dist/core/session-manager.js').as_uri()
        for name, chat in chats.items():
            code = f'import {{SessionManager}} from {json.dumps(module)}; console.log(JSON.stringify(await SessionManager.list({json.dumps(str(zones[name]))}, {json.dumps(str(self.root / "agent/sessions"))})));'
            native_files = json.loads(self.command([self.args.node, '--input-type=module', '-e', code]))
            native = next(item for item in native_files if item['id'] == chat['sessionId'])
            path = Path(native['path'])
            require(path.is_relative_to(self.root), ('Native history escaped isolated storage', native))
            header = json.loads(path.read_text(encoding='utf-8').splitlines()[0])
            require(Path(header['cwd']).resolve() == zones[name].resolve(), ('Actual native cwd does not match independent zone', header, zones[name]))
            histories[name] = {'chat': chat, 'path': str(path), 'header': header, 'native': self.history(str(path)),
                'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}
        evidence['nativeHistories'] = histories
        materials = zones['manager'] / '.workflow' / run['id']
        for name, history in histories.items():
            require(any(str(materials) in self.message_text(entry.get('message', {}))
                for entry in history['native']['branch'] if entry.get('type') == 'message'),
                ('Run-specific intermediate guidance absent from actual Chat', name, str(materials)))
        evidence['materials'] = str(materials)
        require(len({path.resolve() for path in zones.values()} | {business.resolve()}) == 6, 'Manager/node/business directories are not separate')
        for zone in zones.values():
            require(not business.is_relative_to(zone) and not zone.is_relative_to(business), ('Business files overlap zone', business, zone))
            require(not any(path.name in expected for path in zone.rglob('*') if path.is_file()), ('Business artifact copied into zone', zone))
        evidence['reads'] = {}
        for name, filenames in [('source', ['seed.txt']), ('left', ['source.txt']), ('right', ['source.txt']), ('summary', ['left.txt', 'right.txt'])]:
            branch = histories[name]['native']['branch']
            initial = next(entry['message'] for entry in branch if entry.get('type') == 'message' and entry.get('message', {}).get('role') == 'user')
            initialization = self.message_text(initial)
            require('[Named file inputs]' in initialization, ('Missing actual named-input initialization', name))
            for filename in filenames:
                require(str(business / filename) in initialization, ('Registered original path absent from initialization', name, filename))
                content = expected['source.txt' if filename == 'seed.txt' else filename]
                require(not any(line in initialization for line in content.decode().splitlines()), ('Input body leaked into initialization', name))
                evidence['reads'].setdefault(name, []).append(self.native_read(branch, business / filename, content))
            calls = self.native_calls(branch)
            require(all(call['name'] in ['read', 'write', 'report'] for call in calls), ('Consumer bypassed native read/write/report boundary', name, calls))
            allowed_reads = {(business / filename).resolve() for filename in filenames + [name + '.txt']}
            require(all(Path(call.get('arguments', {}).get('path', '')).resolve() in allowed_reads
                for call in calls if call['name'] == 'read'), ('Node read oracle/another seed/unassigned business material', name, calls))
            require(any(call['name'] == 'write' and Path(call.get('arguments', {}).get('path', '')).resolve() == (business / (name + '.txt')).resolve() for call in calls),
                ('Independent business output not written by native node', name))
        manager_calls = self.native_calls(histories['manager']['native']['branch'])
        require(all(call['name'] in ['run', 'collect'] for call in manager_calls), ('Manager read/merged business content or bypassed role tools', manager_calls))
        for name, role in [('manager', 'manager'), ('left', 'node')]:
            guide = (self.args.toolkit / f'extensions/workflow-chat/{role}-chat/shared-logic/prompts/init.md').read_text(encoding='utf-8').strip()
            require(any(guide in '\n'.join(self.message_text(message).replace('\r\n', '\n')
                for message in messages(record)) for record in self.model_wire.records),
                ('Shipped role guide never reached actual model', role))
        session = chats['manager']['sessionId']
        actions = [record for record in self.backend_wire.records if record['path'].startswith('/api/workflow/manager/')
            and (decode(record, 'request') or {}).get('sessionId') == session]
        evidence['orchestration'] = actions
        def complete(node):
            return next(record for record in actions if record['path'].endswith('/complete')
                and (decode(record, 'request') or {}).get('nodeId') == node and (decode(record, 'response') or {}).get('changed') is True)
        def dispatch(node):
            return next(record for record in actions if record['path'].endswith('/dispatch')
                and node in (decode(record, 'request') or {}).get('nodes', []))
        for downstream, upstream in [('left', 'source'), ('right', 'source'), ('summary', 'left'), ('summary', 'right')]:
            before, after = complete(upstream), dispatch(downstream)
            require(before['ended'] <= after['started'], ('No explicit completed-before-dispatch ordering', upstream, downstream))
            require((decode(before, 'response') or {})['snapshot']['nodes'][downstream]['status'] == 'pending', 'Framework automatically dispatched downstream')
        require(len([action for action in actions if action['path'].endswith('/complete')]) == 4, 'Unexpected completion attempts')
        require(any(action['path'].endswith('/finish') for action in actions), 'Native manager never explicitly finished')

    @staticmethod
    def native_calls(branch):
        return [part for entry in branch if entry.get('type') == 'message'
            for part in entry.get('message', {}).get('content', []) if isinstance(part, dict) and part.get('type') == 'toolCall']

    def native_read(self, branch, path, expected):
        calls = [call for call in self.native_calls(branch) if call['name'] == 'read'
            and Path(call.get('arguments', {}).get('path', '')).resolve() == path.resolve()]
        for call in calls:
            results = [entry['message'] for entry in branch if entry.get('type') == 'message'
                and entry.get('message', {}).get('role') == 'toolResult' and entry['message'].get('toolCallId') == call['id']]
            for result in results:
                if not result.get('isError') and all(line in self.message_text(result) for line in expected.decode().splitlines()):
                    return {'path': str(path), 'call': call, 'result': result}
        raise AssertionError(('No successful native read of exact registered address and actual business bytes', str(path), calls))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--scenario', choices=['roles', 'fan-in', 'plan-workflow'], required=True)
    parser.add_argument('--paths-file', type=Path, required=True, help='Explicit isolated dependency roots, never user business/session configuration')
    parser.add_argument('--backend', type=Path, required=True)
    parser.add_argument('--toolkit', type=Path, required=True)
    parser.add_argument('--herdr', type=Path)
    parser.add_argument('--pi-package', type=Path)
    parser.add_argument('--pi-state-extension', type=Path)
    parser.add_argument('--model-config', type=Path, help='Acceptance-only compatible-model JSON; otherwise roots.workflow_model/model.json')
    parser.add_argument('--shell-path', type=Path, help='Explicit native Pi shell (for example Git Bash on Windows), written only to isolated settings')
    parser.add_argument('--node', default='node')
    parser.add_argument('--bun', default='bun')
    parser.add_argument('--timeout', type=int, default=300, help='Bounded per-phase observation window; no scenario retries')
    args = parser.parse_args()
    require(1 <= args.timeout <= 900, '--timeout must be between 1 and 900 seconds')
    resolve_arguments(parser, args, {'herdr': ('herdr', 'herdr.exe'), 'pi_package': ('pi_package', ''),
        'pi_state_extension': ('pi_agent', 'extensions/herdr-agent-state.ts'), 'model_config': ('workflow_model', 'model.json')})
    args.backend = args.backend.resolve()
    args.toolkit = args.toolkit.resolve()
    args.repo = REPO
    if args.scenario == 'plan-workflow':
        from workflow_plan_acceptance import PlanWorkflowScenario

        class PlanWorkflowAcceptance(PlanWorkflowScenario, RolesAcceptance):
            """Use the same isolated real-host lifecycle and wire observers."""

        return PlanWorkflowAcceptance(args).run()
    return (FanInAcceptance if args.scenario == 'fan-in' else RolesAcceptance)(args).run()


if __name__ == '__main__':
    sys.exit(main())
