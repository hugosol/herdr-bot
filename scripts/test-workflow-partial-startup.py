"""P31 process smoke: production definition/run HTTP, disk and controlled Herdr.

This is not native model/role evidence. No credentials, user Chat, daemon,
shutdown, LAN or mDNS is used. The fixture records actual accepted input before
returning an invalid acknowledgement, distinguishing unknown from rejection.
"""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import tempfile
import traceback
import urllib.error
import urllib.request

from acceptance.host.processes import Process, require, until
REPO = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--backend', type=Path, required=True, help='Current worktree safe_shutdown_shell.exe')
    parser.add_argument('--fixture', type=Path, required=True, help='Current worktree herdr-fixture.exe')
    args = parser.parse_args()
    root = Path(tempfile.mkdtemp(prefix='herdr-partial-startup-'))
    evidence = {'http': [], 'scenarios': [], 'errors': [], 'cleanupErrors': []}
    backend = None
    try:
        require(os.name == 'nt', 'Windows controlled host required')
        require(args.backend.is_file() and args.fixture.is_file(), 'Build current backend/safe shell and fixture first')
        for directory in ['host', 'zone', 'maps', 'bots', 'data', 'home']:
            (root / directory).mkdir()
        fixture = root / 'host/herdr-fixture.exe'
        shutil.copy2(args.fixture, fixture)
        def write(path, value):
            path.write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding='utf-8')
        write(root / 'host/workspaces.json', [{'workspace_id': 'w-p31', 'label': 'zone'}])
        def behavior(session, **flags):
            write(root / 'host/behavior.json', {'native_session': {'source': 'herdr:omp', 'agent': 'omp', 'kind': 'id', 'value': session}, 'native_cwd': str(root / 'zone'), **flags})
        behavior('p31-root')
        q = lambda value: json.dumps(str(value))
        catalog = 'skills = []\n[[agents]]\nid = "omp"\nlabel = "OMP"\n[[models]]\nid = "default"\nlabel = "Default"\n'
        for role in ['manager', 'node']:
            entry = root / f'{role}.ts'
            entry.write_text('export default function () {}\n')
            catalog += f'[[extensions]]\nid = "omp-{role}-chat"\nlabel = "{role}"\nagent = "omp"\nkind = "fixed"\nentrypoint = {q(entry)}\n'
        (root / 'options.toml').write_text(catalog)
        config = f'listen_addr = "127.0.0.1:0"\nstatic_directory = {q(REPO / "backend/static")}\ndata_directory = {q(root / "data")}\nprompts_file = {q(root / "prompts.toml")}\n[mdns]\nenabled = false\n'
        for name in ['start', 'stop']:
            config += f'[operations.{name}]\nscript = {q(root / "unused.bat")}\nworking_directory = {q(root)}\n'
        config += f'[herdr]\nworkspace_name = "zone"\nexecutable = {q(fixture)}\n[tasks]\nstorage_file = {q(root / "data/tasks.json")}\nmetadata_file = {q(root / "options.toml")}\n[sessions]\nhistory_file = {q(root / "data/history.json")}\n[zones]\nregistry_file = {q(root / "data/zones.json")}\n[scheduler]\n'
        config += ''.join(f'{key} = {q(value)}\n' for key, value in [('map_directory', root / 'maps'), ('bot_directory', root / 'bots'), ('run_directory', root / 'data/runs'), ('run_instructions', REPO / 'docs/workflow-runtime.md')])
        (root / 'backend.toml').write_text(config)
        environment = dict(os.environ)
        for key in ['HERDR_FIXTURE_DIR', 'HERDR_BOT_FIXTURE_RECORD_DIR', 'HERDR_WORKSPACE', 'HERDR_PANE', 'HERDR_AGENT']:
            environment.pop(key, None)
        environment.update(HOME=str(root / 'home'), USERPROFILE=str(root / 'home'), APPDATA=str(root / 'home'), LOCALAPPDATA=str(root / 'home'), XDG_CONFIG_HOME=str(root / 'home'), XDG_DATA_HOME=str(root / 'home'))
        backend = Process([str(args.backend.resolve()), '--config', str(root / 'backend.toml')], root, environment, root, 'backend')
        def address():
            require(backend.child.poll() is None, 'Backend exited; see raw logs')
            while not backend.lines.empty():
                match = re.search(r'http://(127\.0\.0\.1:\d+)/', backend.lines.get_nowait())
                if match:
                    return 'http://' + match[1]
        base = until('isolated backend loopback address', address)
        def http(path, body=None, status=200):
            request = urllib.request.Request(base + path, data=None if body is None else json.dumps(body).encode(), headers={'Content-Type': 'application/json'})
            try:
                response = urllib.request.urlopen(request, timeout=45)
            except urllib.error.HTTPError as error:
                response = error
            with response:
                value = json.loads(response.read())
                evidence['http'].append({'path': path, 'request': body, 'status': response.status, 'response': value})
                require(response.status == status, evidence['http'][-1])
                return value
        def task(name, kind):
            tasks = http('/api/tasks', {'name': name, 'taskType': kind, 'agent': 'omp', 'chatName': '{chat.id}', 'model': 'default', 'thinking': 'default', 'prompt': ''})
            return next(task['id'] for task in tasks['tasks'] if task['name'] == name)
        manager, worker = task('P31 manager', 'manager'), task('P31 worker', 'normal')
        http('/api/zones', {'id': 'zone', 'path': str(root / 'zone')})
        for id, target in [('worker', {'task_id': worker}), ('nested', {'map_file': 'inner.json'})]:
            http('/api/scheduler/bots/save', {'id': id, 'revision': None, 'content': json.dumps({'name': id, 'when': '', 'plan_guide': '', 'node_guide': '', **target})})
        write(root / 'maps/inner.json', {'title': 'Inner', 'description': '', 'nodes': {'A': {'title': 'Work', 'bot_id': 'worker', 'input': 'WORK', 'zone': 'zone', 'blocked_by': []}}})
        write(root / 'maps/outer.json', {'title': 'Outer', 'description': '', 'nodes': {'A': {'title': 'Call', 'bot_id': 'nested', 'manager_task_id': manager, 'input': 'CHILD-731', 'zone': 'zone', 'blocked_by': []}}})
        http('/api/scheduler/maps/outer.json')
        def records(name):
            path = root / 'host' / name
            return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []
        def disk(id):
            return json.loads((root / 'data/runs' / id / 'run.json').read_text())
        for name, flags, expected, increment in [('success', {}, 'submitted', 1), ('rejected', {'fail_prompt': True}, 'rejected', 0), ('unknown', {'invalid_prompt_response': True}, 'unknown', 1)]:
            session = f'p31-root-{name}'
            behavior(session)
            root_run = http('/api/scheduler/maps/outer.json/runs', {'managerTaskId': manager, 'zone': 'zone', 'userInput': 'ROOT-418'}, 201)
            prepared = http('/api/workflow/manager/prepare_inputs', {'sessionId': session, 'fileInputs': [],
                            'confirmed': True, 'reason': 'Controlled startup fixture text reviewed; no file inputs'})
            require((prepared.get('changed') or prepared.get('stateCommitted')) and prepared['snapshot']['inputsReady']
                    and not prepared.get('error') and not prepared.get('historyError'), prepared)
            behavior(f'p31-child-{name}', **flags)
            before = len(records('submitted.jsonl'))
            dispatched = http('/api/workflow/manager/dispatch', {'sessionId': session, 'nodes': ['A']})
            result = dispatched['results'][0]
            require(result['effects']['inputOutcome'] == expected, result)
            child = dispatched['snapshot']['nodes']['A']['execution']['childRunId']
            started = records('started.jsonl')
            require(len(started) == 2 * (len(evidence['scenarios']) + 1), 'Unexpected actual host startup count')
            require(started[-1]['name'] == result['effects']['agentName'], 'Wrong host agent started')
            require(disk(root_run['id']) == dispatched['snapshot'], 'Parent disk differs')
            child_before = disk(child)
            require(child_before['manager'] == dispatched['snapshot']['nodes']['A']['execution']['chat'], 'Child association differs')
            require(http('/api/workflow/runs/' + child)['run'] == child_before, 'Child HTTP/disk differs')
            require(len(records('submitted.jsonl')) == before + increment, 'Host input effect differs')
            calls = len(records('calls.jsonl'))
            if name != 'success':
                require(dispatched['snapshot']['nodes']['A']['status'] == 'failed', result)
                for finding in ['uncertain', 'created']:
                    observed = http('/api/workflow/manager/reconcile_dispatch', {'sessionId': session, 'nodeId': 'A', 'finding': finding, 'reason': 'Observe existing host and input effect only'})
                    require(observed['snapshot']['nodes']['A']['status'] == 'failed', observed)
                    require(observed['snapshot']['nodes']['A']['failure'] == dispatched['snapshot']['nodes']['A']['failure'], observed)
                refused = http('/api/workflow/manager/retry', {'sessionId': session, 'nodeId': 'A', 'confirmed': True, 'reason': 'No blind replay allowed'}, 409)
                require(refused['code'] == 'dispatch-reconciliation-required', refused)
            require(all(call['kind'] == 'agent_get' and call['target'] == result['effects']['agentName'] for call in records('calls.jsonl')[calls:]), 'Reconciliation replayed startup/input or queried another host target')
            require(disk(child) == child_before, 'Observation changed child')
            require(len(records('submitted.jsonl')) == before + increment, 'Observation replayed input')
            evidence['scenarios'].append({'name': name, 'verified': True, 'parent': root_run['id'], 'child': child, 'inputOutcome': expected})
    except BaseException:
        evidence['errors'].append(traceback.format_exc())
    finally:
        if backend is not None:
            try:
                backend.stop()
            except BaseException:
                evidence['cleanupErrors'].append(traceback.format_exc())
        (root / 'evidence.json').write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding='utf-8')
        print(root / 'evidence.json')
    if evidence['errors'] or evidence['cleanupErrors']:
        raise RuntimeError(json.dumps(evidence['errors'] + evidence['cleanupErrors'], ensure_ascii=False))


if __name__ == '__main__':
    main()
