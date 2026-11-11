"""Controlled nested completion smoke: public HTTP -> safe backend -> Herdr CLI.

Not native agent/Tab lifetime evidence. No credentials, user daemon or shutdown.
"""
import argparse
import json
import os
from pathlib import Path
import queue
import re
import shutil
import subprocess
import tempfile
import threading
import time
import traceback
import urllib.error
import urllib.request

REPO = Path(__file__).resolve().parents[1]


def require(condition, detail):
    if not condition:
        raise AssertionError(detail)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--backend', type=Path, required=True, help='Current worktree safe_shutdown_shell executable')
    parser.add_argument('--herdr-fixture', type=Path, required=True)
    args = parser.parse_args()
    root = Path(tempfile.mkdtemp(prefix='herdr-nested-completion-'))
    evidence = {'root': str(root), 'verified': False, 'http': [], 'cleanupErrors': []}
    process = None
    readers = []
    try:
        require(args.backend.is_file() and args.backend.stem == 'safe_shutdown_shell', 'Build and supply the recording-only safe_shutdown_shell')
        require(args.herdr_fixture.is_file(), 'Build and supply current herdr-fixture')
        require((REPO / 'backend/static/index.html').is_file(), 'Build this worktree frontend first')
        for name in ['host', 'maps', 'bots', 'data', 'zone']:
            (root / name).mkdir()
        host = root / 'host'
        shutil.copy2(args.herdr_fixture, host / 'herdr-fixture.exe')
        def save(path, value):
            path.write_text(json.dumps(value, ensure_ascii=False), encoding='utf-8')
        save(host / 'workspaces.json', [{'workspace_id': 'owned-workspace', 'label': 'zone'}])
        def native(identity):
            save(host / 'behavior.json', {'native_session': {'source': 'herdr:omp', 'agent': 'omp', 'kind': 'id', 'value': identity}, 'native_cwd': str(root / 'zone')})
        native('smoke-manager')
        extension = root / 'controlled-role.ts'
        extension.write_text('export default function () {}\n', encoding='utf-8')
        metadata = (REPO / 'meta-data/task-options.toml').read_text(encoding='utf-8')
        metadata = re.sub(r'(?m)^entrypoint\s*=.*$', lambda _: 'entrypoint = ' + json.dumps(str(extension)), metadata)
        (root / 'options.toml').write_text(metadata, encoding='utf-8')
        q = lambda path: json.dumps(str(path))
        config = f'listen_addr = "127.0.0.1:0"\nstatic_directory = {q(REPO / "backend/static")}\ndata_directory = {q(root / "data")}\n[mdns]\nenabled = false\n'
        for name in ['start', 'stop']:
            config += f'[operations.{name}]\nscript = {q(root / "unused.bat")}\nworking_directory = {q(root)}\n'
        config += f'[herdr]\nworkspace_name = "nested-smoke"\nexecutable = {q(host / "herdr-fixture.exe")}\n[tasks]\nstorage_file = {q(root / "data/tasks.json")}\nmetadata_file = {q(root / "options.toml")}\n[sessions]\nhistory_file = {q(root / "data/history.json")}\n[zones]\nregistry_file = {q(root / "data/zones.json")}\n[scheduler]\n'
        for key, value in [('map_directory', root / 'maps'), ('bot_directory', root / 'bots'), ('run_directory', root / 'data/runs'), ('run_instructions', REPO / 'docs/workflow-runtime.md')]:
            config += f'{key} = {q(value)}\n'
        (root / 'backend.toml').write_text(config, encoding='utf-8')
        env = dict(os.environ)
        for key in list(env):
            if key.startswith('HERDR_'):
                del env[key]
        env['HERDR_FIXTURE_DIR'] = str(host)
        lines = queue.Queue()
        process = subprocess.Popen([str(args.backend.resolve()), '--config', str(root / 'backend.toml')], cwd=root, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        def drain(stream, name):
            with (root / name).open('wb') as log:
                for line in iter(stream.readline, b''):
                    log.write(line)
                    log.flush()
                    lines.put(line.decode('utf-8', errors='replace'))
        for stream, name in [(process.stdout, 'stdout.log'), (process.stderr, 'stderr.log')]:
            thread = threading.Thread(target=drain, args=(stream, name))
            thread.start()
            readers.append(thread)
        deadline = time.monotonic() + 20
        base = None
        while time.monotonic() < deadline and not base:
            require(process.poll() is None, 'Safe backend exited; see stdout/stderr')
            try:
                match = re.search(r'http://(127\.0\.0\.1:\d+)/', lines.get(timeout=.1))
                if match:
                    base = 'http://' + match[1]
            except queue.Empty:
                pass
        require(base, 'Safe backend failed to bind dynamic loopback port')
        def http(path, body=None, expected=200):
            request = urllib.request.Request(base + path, data=None if body is None else json.dumps(body).encode(), headers={'Content-Type': 'application/json'})
            try:
                response = urllib.request.urlopen(request, timeout=30)
            except urllib.error.HTTPError as error:
                response = error
            with response:
                raw = response.read().decode('utf-8')
                evidence['http'].append({'path': path, 'request': body, 'status': response.status, 'response': raw})
                require(response.status == expected, evidence['http'][-1])
                return json.loads(raw)
        def action(session, name, **fields):
            return http('/api/workflow/manager/' + name, {'sessionId': session, **fields})
        http('/api/zones', {'id': 'owned', 'path': str(root / 'zone')})
        tasks = {}
        for role in ['manager', 'worker']:
            result = http('/api/tasks', {'name': role, 'taskType': 'manager' if role == 'manager' else 'normal', 'agent': 'omp', 'chatName': '{chat.id}', 'model': 'default', 'thinking': 'default', 'prompt': 'Controlled smoke'})
            tasks[role] = next(task['id'] for task in result['tasks'] if task['name'] == role)
        http('/api/scheduler/bots/save', {'id': 'worker', 'content': json.dumps({'name': 'Worker', 'task_id': tasks['worker'], 'when': '', 'plan_guide': '', 'node_guide': ''}), 'revision': None})
        node = lambda title: {'title': title, 'bot_id': 'worker', 'input': 'Owned work', 'zone': 'owned', 'blocked_by': []}
        save(root / 'maps/inner.json', {'title': 'Inner', 'description': '', 'nodes': {'inside': node('Inside')}})
        http('/api/scheduler/bots/save', {'id': 'nested', 'content': json.dumps({'name': 'Nested', 'map_file': 'inner.json', 'when': '', 'plan_guide': '', 'node_guide': ''}), 'revision': None})
        save(root / 'maps/outer.json', {'title': 'Outer', 'description': '', 'nodes': {}})
        call = {**node('Call'), 'bot_id': 'nested', 'manager_task_id': tasks['manager'], 'output_rule': [{'id': 'result'}, {'id': 'optional'}], 'auto_close': True}
        consumer = {**node('Consumer'), 'blocked_by': ['call'], 'input_rule': [{'type': 'node', 'id': 'material', 'node_id': 'call', 'output_id': 'result'}]}
        for identity, content in [('call', call), ('consumer', consumer)]:
            draft = http('/api/scheduler/maps/outer.json/nodes/new')
            http('/api/scheduler/maps/outer.json/nodes/save', {'id': identity, 'content': json.dumps(content), 'revision': draft['revision'], 'create': True})
        started = http('/api/scheduler/maps/outer.json/runs', {'managerTaskId': tasks['manager'], 'zone': 'owned', 'userInput': 'Owned nested smoke'}, 201)
        root_id = started['id']
        prepared = action('smoke-manager', 'prepare_inputs', fileInputs=[], confirmed=True,
                          reason='Controlled outer fixture text reviewed; no file inputs')
        require((prepared.get('changed') or prepared.get('stateCommitted')) and prepared['snapshot']['inputsReady']
                    and not prepared.get('error') and not prepared.get('historyError'), prepared)
        native('smoke-child')
        launched = action('smoke-manager', 'dispatch', nodes=['call'])
        execution = launched['snapshot']['nodes']['call']['execution']
        child_id = execution['childRunId']
        report = {'sessionId': 'smoke-child', 'body': 'Not yet finished', 'canComplete': True}
        require(http('/api/workflow/node/report', report, 400)['code'] == 'child-run-not-finished', 'unfinished child accepted')
        prepared = action('smoke-child', 'prepare_inputs', fileInputs=[], confirmed=True,
                          reason='Controlled inner fixture text reviewed; no file inputs')
        require((prepared.get('changed') or prepared.get('stateCommitted')) and prepared['snapshot']['inputsReady']
                    and not prepared.get('error') and not prepared.get('historyError'), prepared)
        native('smoke-inside')
        action('smoke-child', 'dispatch', nodes=['inside'])
        http('/api/workflow/node/report', {'sessionId': 'smoke-inside', 'body': 'Internal done', 'canComplete': True})
        action('smoke-child', 'complete', nodeId='inside', reason='Accepted internal result')
        http('/api/workflow/node/report', report, 400)
        action('smoke-child', 'finish')
        require(not (host / 'closed.jsonl').exists(), 'child finish closed a Tab')
        output = root / 'zone/result.txt'
        output.write_text('NESTED-SMOKE-749', encoding='utf-8')
        save(host / 'behavior.json', {'fail_prompt': True})
        report.update(body='Finished with retained notification failure', outputs=[{'id': 'result', 'path': str(output)}])
        saved = http('/api/workflow/node/report', report)
        require(saved['saved'] and saved['notification']['status'] == 'failed', saved)
        parent = http('/api/workflow/runs/' + root_id)
        child = http('/api/workflow/runs/' + child_id)
        require(parent['reports'] == [saved['report']] and parent['run']['nodes']['call']['status'] == 'running', parent)
        require(child['run']['status'] == 'completed', child)
        persisted = [json.loads(line) for line in (root / 'data/runs' / root_id / 'messages.jsonl').read_text(encoding='utf-8').splitlines()]
        require(persisted == [saved['report']], persisted)
        save(host / 'behavior.json', {})
        action('smoke-manager', 'complete', nodeId='call', reason='Explicit acceptance after saved notification failure')
        deadline = time.monotonic() + 10
        while not (host / 'closed.jsonl').exists() and time.monotonic() < deadline:
            time.sleep(.01)
        require((host / 'closed.jsonl').exists(), 'No close effect')
        action('smoke-manager', 'complete', nodeId='call', reason='Idempotent repeat')
        native('smoke-consumer')
        consumed = action('smoke-manager', 'dispatch', nodes=['consumer'])
        require(consumed['snapshot']['nodes']['consumer']['execution']['inputs'] == [{'id': 'material', 'path': str(output)}], consumed)
        time.sleep(.15)
        closed = [json.loads(line) for line in (host / 'closed.jsonl').read_text().splitlines()]
        require(closed == [{'target': execution['chat']['tabId']}], closed)
        require(output.read_text(encoding='utf-8') == 'NESTED-SMOKE-749', 'Output changed')
        require(http('/api/workflow/runs/' + child_id) == child, 'Child changed after parent acceptance')
        evidence['verified'] = True
    except BaseException:
        evidence['error'] = traceback.format_exc()
    finally:
        if process is not None:
            try:
                if process.poll() is None:
                    process.kill()
                process.wait(timeout=15)
            except BaseException:
                evidence['cleanupErrors'].append(traceback.format_exc())
        for thread in readers:
            thread.join(timeout=15)
            if thread.is_alive():
                evidence['cleanupErrors'].append('Log consumer did not exit')
        (root / 'evidence.json').write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps({'evidence': str(root / 'evidence.json'), 'verified': evidence['verified'], 'error': evidence.get('error'), 'cleanupErrors': evidence['cleanupErrors']}))
    return 0 if evidence['verified'] and not evidence['cleanupErrors'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
