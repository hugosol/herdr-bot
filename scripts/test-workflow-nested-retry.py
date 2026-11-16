"""P29/P30 controlled smoke: public definitions/run HTTP, disk and Herdr CLI.

No native model/role claim (ticket 15 owns that). Uses only owned temporary
files, dynamic numeric loopback and the current worktree recording-safe shell.
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
    parser.add_argument('--backend', type=Path, required=True)
    parser.add_argument('--fixture', type=Path, required=True)
    args = parser.parse_args()
    root = Path(tempfile.mkdtemp(prefix='herdr-nested-retry-'))
    evidence = {'http': [], 'verified': [], 'errors': [], 'cleanupErrors': []}
    backend = None
    try:
        require(os.name == 'nt', 'Windows controlled host required')
        require(args.backend.is_file() and args.backend.stem == 'safe_shutdown_shell', 'Build current worktree safe_shutdown_shell first')
        require(args.fixture.is_file() and (REPO / 'backend/static/index.html').is_file(), 'Build fixture and worktree frontend first')
        for name in ['host', 'zone', 'maps', 'bots', 'data', 'home']:
            (root / name).mkdir()
        host = root / 'host'
        fixture = host / 'herdr-fixture.exe'
        shutil.copy2(args.fixture, fixture)
        def save(path, value):
            path.write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding='utf-8')
        save(host / 'workspaces.json', [{'workspace_id': 'w-retry', 'label': 'zone'}])
        def native(session, **flags):
            save(host / 'behavior.json', {'native_session': {'source': 'herdr:omp', 'agent': 'omp', 'kind': 'id', 'value': session}, 'native_cwd': str(root / 'zone'), **flags})
        native('retry-root')
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
        env = {key: value for key, value in os.environ.items() if not key.startswith('HERDR_')}
        env.update({key: str(root / 'home') for key in ['HOME', 'USERPROFILE', 'APPDATA', 'LOCALAPPDATA', 'XDG_CONFIG_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME']})
        backend = Process([str(args.backend.resolve()), '--config', str(root / 'backend.toml')], root, env, root, 'backend')
        def address():
            require(backend.child.poll() is None, 'Backend exited; see logs')
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
        def action(session, name, status=200, **fields):
            return http('/api/workflow/manager/' + name, {'sessionId': session, **fields}, status)
        def prepare_inputs(session):
            prepared = action(session, 'prepare_inputs', fileInputs=[], confirmed=True,
                              reason='Controlled retry fixture text reviewed; no file inputs')
            require((prepared.get('changed') or prepared.get('stateCommitted')) and prepared['snapshot']['inputsReady']
                    and not prepared.get('error') and not prepared.get('historyError'), prepared)
        def records(name):
            path = host / name
            return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []
        def disk(id):
            return json.loads((root / 'data/runs' / id / 'run.json').read_text())
        def bytes_for(id):
            return [(root / 'data/runs' / id / name).read_bytes() for name in ['run.json', 'history.jsonl', 'messages.jsonl']]
        def task(name, kind):
            result = http('/api/tasks', {'name': name, 'taskType': kind, 'agent': 'omp', 'chatName': '{chat.id}', 'model': 'default', 'thinking': 'default', 'prompt': ''})
            return next(task['id'] for task in result['tasks'] if task['name'] == name)
        manager, worker = task('Retry manager', 'manager'), task('Retry worker', 'normal')
        http('/api/zones', {'id': 'zone', 'path': str(root / 'zone')})
        for id, target in [('worker', {'task_id': worker}), ('nested', {'map_file': 'inner.json'})]:
            http('/api/scheduler/bots/save', {'id': id, 'revision': None, 'content': json.dumps({'name': id, 'when': '', 'plan_guide': '', 'node_guide': '', **target})})
        node = {'title': 'Work', 'bot_id': 'worker', 'input': 'WORK', 'zone': 'zone', 'blocked_by': [], 'output_rule': [{'id': 'result'}, {'id': 'missing'}]}
        save(root / 'maps/inner.json', {'title': 'Inner', 'description': '', 'nodes': {'A': node}})
        save(root / 'maps/outer.json', {'title': 'Outer', 'description': '', 'nodes': {}})
        call = {**node, 'bot_id': 'nested', 'manager_task_id': manager}
        for id, content in [('A', call), ('consumer', {**node, 'blocked_by': ['A'], 'input_rule': [{'id': 'material', 'type': 'node', 'node_id': 'A', 'output_id': 'result'}]}), ('missing', {**node, 'blocked_by': ['A'], 'input_rule': [{'id': 'material', 'type': 'node', 'node_id': 'A', 'output_id': 'missing'}]})]:
            draft = http('/api/scheduler/maps/outer.json/nodes/new')
            http('/api/scheduler/maps/outer.json/nodes/save', {'id': id, 'content': json.dumps(content), 'revision': draft['revision'], 'create': True})
        def start(session):
            native(session)
            started = http('/api/scheduler/maps/outer.json/runs', {'managerTaskId': manager, 'zone': 'zone', 'userInput': 'RETRY-SMOKE'}, 201)
            prepare_inputs(session)
            return started['id']
        def report(session, ready, outputs):
            value = http('/api/workflow/node/report', {'sessionId': session, 'body': session, 'canComplete': ready, 'outputs': outputs})
            require(value['saved'] and value['notification']['status'] == 'submitted', value)
            delivered = records('submitted.jsonl')[-1]
            require(delivered['target'] == value['report']['to']['agentName'] and value['report']['id'] in delivered['text'], 'Report notification reached wrong host')
            return value['report']
        def finish(session, inside):
            prepare_inputs(session)
            native(inside)
            action(session, 'dispatch', nodes=['A'])
            report(inside, True, [])
            action(session, 'complete', nodeId='A', reason='Internal acceptance')
            action(session, 'finish')
        parent = start('retry-root')
        native('retry-old')
        old = action('retry-root', 'dispatch', nodes=['A'])['snapshot']['nodes']['A']['execution']
        finish('retry-old', 'retry-old-inside')
        old_file, new_file = root / 'zone/old.txt', root / 'zone/new.txt'
        old_file.write_text('OLD-EFFECT-731')
        new_file.write_text('NEW-OUTPUT-829')
        old_outputs = [{'id': 'result', 'path': str(old_file)}, {'id': 'missing', 'path': str(old_file)}]
        reports = [report('retry-old', True, old_outputs)]
        action('retry-root', 'fail', nodeId='A', reason='Old effects inspected; cannot continue')
        old_bytes = bytes_for(old['childRunId'])
        parent_bytes, calls = bytes_for(parent), records('calls.jsonl')
        require(action('retry-root', 'retry', 409, nodeId='A', confirmed=False, reason='Not approved')['code'] == 'retry-confirmation-required', 'Unconfirmed retry accepted')
        require(bytes_for(parent) == parent_bytes and records('calls.jsonl') == calls, 'Rejected retry had effects')
        native('retry-new')
        retried = action('retry-root', 'retry', nodeId='A', confirmed=True, reason='Confirmed new attempt after inspection')
        current = retried['snapshot']['nodes']['A']['execution']
        for key in ['id', 'childRunId', 'chat']:
            require(current[key] != old[key], ('Reused identity', key))
        require(bytes_for(old['childRunId']) == old_bytes and old_file.read_text() == 'OLD-EFFECT-731', 'Retry erased old facts/effects')
        require(disk(current['childRunId'])['parent'] == {'runId': parent, 'nodeId': 'A', 'executionId': current['id']}, 'New reverse link differs')
        reports.append(report('retry-old', True, old_outputs))
        require(action('retry-root', 'status')['pendingReports'] == {}, 'Old report became pending')
        require(action('retry-root', 'complete', 409, nodeId='A', reason='Reject old readiness')['code'] == 'report-not-completable', 'Old readiness accepted')
        reports.append(report('retry-new', False, []))
        reports.append(report('retry-old', True, old_outputs))
        require(action('retry-root', 'collect')['pendingReports']['A'] == reports[2]['id'], 'Late old report replaced progress')
        require(action('retry-root', 'complete', 409, nodeId='A', reason='Current false')['code'] == 'report-not-completable', 'Old true overrode current false')
        finish('retry-new', 'retry-new-inside')
        reports.append(report('retry-new', True, [{'id': 'result', 'path': str(new_file)}]))
        reports.append(report('retry-old', True, old_outputs))
        require(action('retry-root', 'collect')['pendingReports']['A'] == reports[4]['id'], 'Old report replaced ready report')
        action('retry-root', 'complete', nodeId='A', reason='Accept new only')
        calls = records('calls.jsonl')
        refused = action('retry-root', 'dispatch', nodes=['missing'])
        require(refused['results'][0]['failure']['stage'] == 'resolve' and records('calls.jsonl') == calls, 'Missing output fell back or started a host')
        native('retry-consumer')
        consumed = action('retry-root', 'dispatch', nodes=['consumer'])
        inputs = consumed['snapshot']['nodes']['consumer']['execution']['inputs']
        require(inputs == [{'id': 'material', 'path': str(new_file)}], inputs)
        require(Path(inputs[0]['path']).read_text() == 'NEW-OUTPUT-829', 'Wrong actual material')
        require(f'material: {new_file}\n' in records('submitted.jsonl')[-1]['text'], 'Host initialization did not receive new address')
        require(disk(parent) == consumed['snapshot'], 'Disk/current HTTP differ')
        require([json.loads(line) for line in (root / 'data/runs' / parent / 'messages.jsonl').read_text().splitlines()] == reports, 'Persisted report history differs')
        evidence['verified'].extend(['P29-new-identities-confirmation-preserved-effects', 'P30-interleaved-history-pending-completion-consumer-missing-output'])
        # Use ticket 07 facts: a registered child without a Chat is not absent;
        # an actually accepted input with malformed ack remains uncertain.
        for name, flags in [('no-chat', {'fail_agent_start': True}), ('unknown', {'invalid_prompt_response': True})]:
            session = 'partial-' + name
            id = start(session)
            native('child-' + name, **flags)
            before = len(records('submitted.jsonl'))
            failed = action(session, 'dispatch', nodes=['A'])
            execution = failed['snapshot']['nodes']['A']['execution']
            require(disk(execution['childRunId'])['parent']['runId'] == id, 'Registered child lost')
            calls = records('calls.jsonl')
            if name == 'no-chat':
                require(execution['chat'] is None, 'Expected no Chat')
                require(action(session, 'reconcile_dispatch', 409, nodeId='A', finding='not_created', reason='No Chat, but child exists')['code'] == 'dispatch-child-already-known', 'Erased known child')
            else:
                require(failed['results'][0]['effects']['inputOutcome'] == 'unknown' and len(records('submitted.jsonl')) == before + 1, 'Unknown input effect not real')
                action(session, 'reconcile_dispatch', nodeId='A', finding='uncertain', reason='Accepted input, unknown acknowledgement')
            require(action(session, 'retry', 409, nodeId='A', confirmed=True, reason='Must not replay uncertainty')['code'] == 'dispatch-reconciliation-required', 'Blind retry accepted')
            require(records('calls.jsonl') == calls, 'Reconciliation/retry replayed host effects')
            require(disk(id) == failed['snapshot'], 'Partial facts changed')
            evidence['verified'].append('partial-' + name)
    except BaseException:
        evidence['errors'].append(traceback.format_exc())
    finally:
        if backend is not None:
            try:
                backend.stop()
            except BaseException:
                evidence['cleanupErrors'].append(traceback.format_exc())
        save_path = root / 'evidence.json'
        save_path.write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding='utf-8')
        print(save_path)
    if evidence['errors'] or evidence['cleanupErrors']:
        raise RuntimeError(json.dumps(evidence['errors'] + evidence['cleanupErrors'], ensure_ascii=False))


if __name__ == '__main__':
    main()
