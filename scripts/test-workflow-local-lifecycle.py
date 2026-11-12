"""P27/P28/P47: isolated public HTTP, durable facts and controlled Herdr effects.

No native-model, real Tab lifetime, automatic recovery or scale claim. Never
connects a user daemon, reads credentials, enables LAN/mDNS or invokes shutdown.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import json
import os
from pathlib import Path
import re
import runpy
import shutil
import tempfile
import traceback
import urllib.error
import urllib.request

shared = runpy.run_path(str(Path(__file__).with_name('test-real-herdr-close.py')))
Process, require, until = (shared[name] for name in ['Process', 'require', 'until'])
REPO = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--backend', type=Path, required=True)
    parser.add_argument('--fixture', type=Path, required=True)
    args = parser.parse_args()
    root = Path(tempfile.mkdtemp(prefix='herdr-local-lifecycle-'))
    evidence = {'http': [], 'verified': [], 'errors': [], 'cleanupErrors': []}
    backend = None
    pool = ThreadPoolExecutor(max_workers=4)
    accepted = []
    gates = [root / name for name in ['launch.release', 'inner.release', 'outer.release']]
    try:
        require(os.name == 'nt', 'Windows controlled-host smoke required')
        require(args.backend.is_file() and args.backend.stem == 'safe_shutdown_shell', 'Build current worktree recording-only safe shell')
        require(args.fixture.is_file(), 'Build current worktree herdr-fixture')
        require((REPO / 'backend/static/index.html').is_file(), 'Build worktree frontend first')
        for directory in ['host', 'zone', 'maps', 'bots', 'data', 'home']:
            (root / directory).mkdir()
        fixture = root / 'host/herdr-fixture.exe'
        shutil.copy2(args.fixture, fixture)
        def save(path, value):
            path.write_text(json.dumps(value, ensure_ascii=False), encoding='utf-8')
        save(root / 'host/workspaces.json', [{'workspace_id': 'owned', 'label': 'zone'}])
        def native(session, **flags):
            save(root / 'host/behavior.json', {'native_session': {'source': 'herdr:omp', 'agent': 'omp', 'kind': 'id', 'value': session}, 'native_cwd': str(root / 'zone'), **flags})
        native('local-root')
        extension = root / 'role.ts'
        extension.write_text('export default function () {}\n', encoding='utf-8')
        metadata = (REPO / 'meta-data/task-options.toml').read_text(encoding='utf-8')
        metadata = re.sub(r'(?m)^entrypoint\s*=.*$', lambda _: 'entrypoint = ' + json.dumps(str(extension)), metadata)
        (root / 'options.toml').write_text(metadata, encoding='utf-8')
        q = lambda value: json.dumps(str(value))
        config = f'listen_addr = "127.0.0.1:0"\nstatic_directory = {q(REPO / "backend/static")}\ndata_directory = {q(root / "data")}\nprompts_file = {q(root / "prompts.toml")}\n[mdns]\nenabled = false\n'
        for name in ['start', 'stop']:
            config += f'[operations.{name}]\nscript = {q(root / "unused.bat")}\nworking_directory = {q(root)}\n'
        config += f'[herdr]\nworkspace_name = "zone"\nexecutable = {q(fixture)}\n[tasks]\nstorage_file = {q(root / "data/tasks.json")}\nmetadata_file = {q(root / "options.toml")}\n[sessions]\nhistory_file = {q(root / "data/history.json")}\n[zones]\nregistry_file = {q(root / "data/zones.json")}\n[scheduler]\n'
        config += ''.join(f'{key} = {q(value)}\n' for key, value in [('map_directory', root / 'maps'), ('bot_directory', root / 'bots'), ('run_directory', root / 'data/runs'), ('run_instructions', REPO / 'docs/workflow-runtime.md')])
        (root / 'backend.toml').write_text(config, encoding='utf-8')
        environment = dict(os.environ)
        for key in list(environment):
            if key.startswith('HERDR_'):
                del environment[key]
        environment.update(HOME=str(root / 'home'), USERPROFILE=str(root / 'home'), APPDATA=str(root / 'home'), LOCALAPPDATA=str(root / 'home'), XDG_CONFIG_HOME=str(root / 'home'), XDG_DATA_HOME=str(root / 'home'))
        backend = Process([str(args.backend.resolve()), '--config', str(root / 'backend.toml')], root, environment, root, 'backend')
        def address():
            require(backend.child.poll() is None, 'Backend exited; see raw logs')
            while not backend.lines.empty():
                match = re.search(r'http://(127\.0\.0\.1:\d+)/', backend.lines.get_nowait())
                if match:
                    return 'http://' + match[1]
        base = until('dynamic numeric loopback bind', address, 20)
        def http(path, body=None, status=200):
            request = urllib.request.Request(base + path, data=None if body is None else json.dumps(body).encode(), headers={'Content-Type': 'application/json'})
            try:
                response = urllib.request.urlopen(request, timeout=15)
            except urllib.error.HTTPError as error:
                response = error
            with response:
                value = json.loads(response.read())
                receipt = {'path': path, 'request': body, 'status': response.status, 'response': value}
                evidence['http'].append(receipt)
                require(response.status == status, receipt)
                return value
        def action(session, name, status=200, **fields):
            return http('/api/workflow/manager/' + name, {'sessionId': session, **fields}, status)
        def prepare_inputs(session):
            prepared = action(session, 'prepare_inputs', fileInputs=[], confirmed=True,
                              reason='Controlled lifecycle fixture text reviewed; no file inputs')
            require((prepared.get('changed') or prepared.get('stateCommitted')) and prepared['snapshot']['inputsReady']
                    and not prepared.get('error') and not prepared.get('historyError'), prepared)
        def report(session, body):
            return http('/api/workflow/node/report', {'sessionId': session, 'body': body, 'canComplete': True})
        def records(name):
            path = root / 'host' / name
            return [json.loads(line) for line in path.read_text(encoding='utf-8').splitlines()] if path.exists() else []
        def detail(identity):
            value = http('/api/workflow/runs/' + identity)
            disk = json.loads((root / 'data/runs' / identity / 'run.json').read_text(encoding='utf-8'))
            require(value['run'] == disk, 'Public/durable Run mismatch')
            return value
        def task(name, kind):
            result = http('/api/tasks', {'name': name, 'taskType': kind, 'agent': 'omp', 'chatName': '{chat.id}', 'model': 'default', 'thinking': 'default', 'prompt': ''})
            return next(task['id'] for task in result['tasks'] if task['name'] == name)
        manager, worker = task('Manager', 'manager'), task('Worker', 'normal')
        http('/api/zones', {'id': 'zone', 'path': str(root / 'zone')})
        for identity, target in [('worker', {'task_id': worker}), ('nested', {'map_file': 'inner.json'})]:
            http('/api/scheduler/bots/save', {'id': identity, 'revision': None, 'content': json.dumps({'name': identity, 'when': '', 'plan_guide': '', 'node_guide': '', **target})})
        node = lambda title: {'title': title, 'bot_id': 'worker', 'input': title, 'zone': 'zone', 'blocked_by': []}
        save(root / 'maps/inner.json', {'title': 'Inner', 'description': '', 'nodes': {'inside': node('INSIDE')}})
        save(root / 'maps/outer.json', {'title': 'Outer', 'description': '', 'nodes': {}})
        for identity, content in [('call', {**node('CALL'), 'bot_id': 'nested', 'manager_task_id': manager}), ('branch', node('BRANCH')), ('later', node('LATER'))]:
            draft = http('/api/scheduler/maps/outer.json/nodes/new')
            http('/api/scheduler/maps/outer.json/nodes/save', {'id': identity, 'content': json.dumps(content), 'revision': draft['revision'], 'create': True})
        def start(session):
            native(session)
            started = http('/api/scheduler/maps/outer.json/runs', {'managerTaskId': manager, 'zone': 'zone', 'userInput': 'Owned lifecycle smoke'}, 201)
            prepare_inputs(session)
            return started['id']
        root_id = start('local-root')
        independent_id = start('local-independent')
        native('local-child')
        child_id = action('local-root', 'dispatch', nodes=['call'])['snapshot']['nodes']['call']['execution']['childRunId']
        native('local-branch')
        action('local-root', 'dispatch', nodes=['branch'])
        action('local-root', 'fail', nodeId='branch', reason='Controlled failure')
        action('local-root', 'pause')
        before = detail(root_id)
        calls = records('calls.jsonl')
        rejected = action('local-root', 'dispatch', nodes=['later'])
        require(rejected['results'] == [{'nodeId': 'later', 'status': 'rejected', 'reasons': ['Run 状态为 paused']}], rejected)
        require(action('local-root', 'retry', status=409, nodeId='branch', confirmed=True, reason='Paused retry')['code'] == 'run-paused', 'Paused retry accepted')
        require(detail(root_id) == before and records('calls.jsonl') == calls, 'Paused rejection changed facts or host')
        prepare_inputs('local-child')
        native('local-inside')
        require(action('local-child', 'dispatch', nodes=['inside'])['results'][0]['status'] == 'running', 'Child blocked by parent pause')
        report('local-inside', 'Inside completed while parent paused')
        action('local-child', 'complete', nodeId='inside', reason='Accepted')
        action('local-child', 'finish')
        report('local-child', 'Finished while parent paused')
        require(detail(child_id)['run']['status'] == 'completed', 'Child failed to finish')
        require(detail(root_id)['run']['status'] == 'paused', 'Child changed parent eligibility')
        action('local-root', 'complete', nodeId='call', reason='Explicit acceptance')
        calls = records('calls.jsonl')
        action('local-root', 'resume')
        require(records('calls.jsonl') == calls, 'Resume automatically dispatched')
        evidence['verified'].append('P27')

        # Real refused host submission; independent Run must still launch/submit.
        failure_root = start('failure-root')
        native('failure-child')
        failed_child = action('failure-root', 'dispatch', nodes=['call'])['snapshot']['nodes']['call']['execution']['childRunId']
        prepare_inputs('failure-child')
        before = detail(failure_root)
        native('failure-inside', fail_prompt=True)
        failed = action('failure-child', 'dispatch', nodes=['inside'])
        require(failed['results'][0]['failure']['stage'] == 'submit' and failed['results'][0]['effects']['inputOutcome'] == 'rejected', failed)
        require(detail(failed_child)['run']['nodes']['inside']['status'] == 'failed', 'Failure not durable')
        require(detail(failure_root) == before, 'Child failure cascaded')
        native('independent-worker')
        advanced = action('local-independent', 'dispatch', nodes=['branch'])
        require(advanced['results'][0]['status'] == 'running', 'Independent Run stopped')
        independent_chat = advanced['snapshot']['nodes']['branch']['execution']['chat']
        require(independent_chat['sessionId'] == 'independent-worker', independent_chat)
        require(detail(independent_id)['run']['status'] == 'running', 'Independent Run paused')
        require(any(item['target'] == independent_chat['agentName'] and 'BRANCH' in item['text'] for item in records('submitted.jsonl')), 'No actual independent host submission')
        evidence['verified'].append('P28')

        # Parent dispatch is held at actual initial input. Child work, finish,
        # independent dispatch and parent acceptance overlap slow notifications.
        concurrent_id = start('concurrent-root')
        native('concurrent-child', prompt_gate={'wait_for_file': str(gates[0]), 'timeout_ms': 30000})
        count = len(records('calls.jsonl'))
        parent = pool.submit(action, 'concurrent-root', 'dispatch', nodes=['call'])
        accepted.append(parent)
        until('parent initial-input gate', lambda: any(item['kind'] == 'agent_prompt' for item in records('calls.jsonl')[count:]), 10)
        concurrent_child = detail(concurrent_id)['run']['nodes']['call']['execution']['childRunId']
        prepare_inputs('concurrent-child')
        native('concurrent-inside')
        require(action('concurrent-child', 'dispatch', nodes=['inside'])['results'][0]['status'] == 'running', 'Child operation waits for parent')
        native('concurrent-inside', prompt_gate={'match': 'INNER-SLOW', 'wait_for_file': str(gates[1]), 'timeout_ms': 30000})
        inner = pool.submit(report, 'concurrent-inside', 'INNER-SLOW')
        accepted.append(inner)
        until('saved internal report', lambda: detail(concurrent_child)['reports'], 10)
        until('slow internal notification', lambda: any('INNER-SLOW' in item.get('text', '') for item in records('calls.jsonl')), 10)
        action('concurrent-child', 'complete', nodeId='inside', reason='Saved report accepted before notification returns')
        action('concurrent-child', 'finish')
        native('concurrent-child', prompt_gate={'match': 'OUTER-SLOW', 'wait_for_file': str(gates[2]), 'timeout_ms': 30000})
        outer = pool.submit(report, 'concurrent-child', 'OUTER-SLOW')
        accepted.append(outer)
        native('concurrent-independent', prompt_gate={'match': 'OUTER-SLOW', 'wait_for_file': str(gates[2]), 'timeout_ms': 30000})
        require(action('local-independent', 'dispatch', nodes=['later'])['results'][0]['status'] == 'running', 'Independent launch blocked by parent/child locks')
        gates[0].write_text('release')
        require(parent.result(timeout=10)['results'][0]['status'] == 'running', 'Parent failed to advance')
        until('saved outward report', lambda: detail(concurrent_id)['reports'], 10)
        until('slow outward notification', lambda: any('OUTER-SLOW' in item.get('text', '') for item in records('calls.jsonl')), 10)
        action('concurrent-root', 'complete', nodeId='call', reason='Explicit acceptance during slow notifications')
        require(not inner.done() and not outer.done(), 'Notifications must still be held by their external gates')
        require(detail(concurrent_id)['run']['nodes']['call']['status'] == 'completed', 'Parent acceptance not durable')
        require(detail(concurrent_child)['run']['status'] == 'completed', 'Child finish not durable')
        evidence['verified'].append('P47')
    except BaseException:
        evidence['errors'].append(traceback.format_exc())
    finally:
        for gate in gates:
            try:
                gate.write_text('release', encoding='utf-8')
            except BaseException:
                evidence['cleanupErrors'].append(traceback.format_exc())
        for future in accepted:
            try:
                result = future.result(timeout=35)
                if result.get('saved') is True:
                    require(result['notification']['status'] == 'submitted', result)
                else:
                    require(result['results'][0]['status'] == 'running', result)
            except BaseException:
                evidence['cleanupErrors'].append(traceback.format_exc())
        pool.shutdown(wait=True)
        if backend is not None:
            try:
                backend.stop()
            except BaseException:
                evidence['cleanupErrors'].append(traceback.format_exc())
        save_evidence = root / 'evidence.json'
        save_evidence.write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding='utf-8')
        print(save_evidence)
    return 0 if evidence['verified'] == ['P27', 'P28', 'P47'] and not evidence['errors'] and not evidence['cleanupErrors'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
