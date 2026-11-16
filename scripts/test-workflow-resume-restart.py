"""P32/P33 backend acceptance: public HTTP, persisted identities, real restart.

The recording Herdr fixture observes argv/input; it is not native OMP/Pi
extension loading or tool wiring evidence (owned by the native lifecycle suite).
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
    root = Path(tempfile.mkdtemp(prefix='herdr-resume-restart-'))
    evidence = {'http': [], 'verified': [], 'errors': [], 'cleanupErrors': [], 'processes': []}
    backend = None
    try:
        require(os.name == 'nt', 'Windows controlled host required')
        require(args.backend.is_file() and args.backend.stem == 'safe_shutdown_shell', 'Build current worktree safe_shutdown_shell first')
        require(args.fixture.is_file() and (REPO / 'backend/static/index.html').is_file(), 'Build fixture and current worktree frontend first')
        for name in ['host', 'zone', 'maps', 'bots', 'data', 'home']:
            (root / name).mkdir()
        host = root / 'host'
        fixture = host / 'herdr-fixture.exe'
        shutil.copy2(args.fixture, fixture)
        def save(path, value):
            path.write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding='utf-8')
        def native(session):
            save(host / 'behavior.json', {'native_session': {'source': 'herdr:omp', 'agent': 'omp', 'kind': 'id', 'value': session}, 'native_cwd': str(root / 'zone')})
        save(host / 'workspaces.json', [{'workspace_id': 'w-resume', 'label': 'zone'}])
        q = lambda value: json.dumps(str(value))
        def catalog(revision):
            text = 'skills = []\n[[agents]]\nid = "omp"\nlabel = "OMP"\n[[models]]\nid = "default"\nlabel = "Default"\n'
            for role in ['manager', 'node']:
                entry = root / f'{revision}-{role}.ts'
                entry.write_text('export default function () {}\n')
                text += f'[[extensions]]\nid = "omp-{role}-chat"\nlabel = "{role}"\nagent = "omp"\nkind = "fixed"\nentrypoint = {q(entry)}\n'
            (root / 'options.toml').write_text(text)
        catalog('original')
        config = f'listen_addr = "127.0.0.1:0"\nstatic_directory = {q(REPO / "backend/static")}\ndata_directory = {q(root / "data")}\nprompts_file = {q(root / "prompts.toml")}\n[mdns]\nenabled = false\n'
        for name in ['start', 'stop']:
            config += f'[operations.{name}]\nscript = {q(root / "unused.bat")}\nworking_directory = {q(root)}\n'
        config += f'[herdr]\nworkspace_name = "zone"\nexecutable = {q(fixture)}\n[tasks]\nstorage_file = {q(root / "data/tasks.json")}\nmetadata_file = {q(root / "options.toml")}\n[sessions]\nhistory_file = {q(root / "data/history.json")}\n[zones]\nregistry_file = {q(root / "data/zones.json")}\n[scheduler]\n'
        config += ''.join(f'{key} = {q(value)}\n' for key, value in [('map_directory', root / 'maps'), ('bot_directory', root / 'bots'), ('run_directory', root / 'data/runs'), ('run_instructions', REPO / 'docs/workflow-runtime.md')])
        (root / 'backend.toml').write_text(config)
        env = {key: value for key, value in os.environ.items() if not key.startswith('HERDR_')}
        env.update({key: str(root / 'home') for key in ['HOME', 'USERPROFILE', 'APPDATA', 'LOCALAPPDATA', 'XDG_CONFIG_HOME', 'XDG_DATA_HOME', 'XDG_STATE_HOME']})
        def start(generation):
            nonlocal backend
            backend = Process([str(args.backend.resolve()), '--config', str(root / 'backend.toml')], root, env, root, f'backend-{generation}')
            evidence['processes'].append({'generation': generation, 'pid': backend.child.pid})
            def address():
                require(backend.child.poll() is None, 'Backend exited; see logs')
                while not backend.lines.empty():
                    match = re.search(r'http://(127\.0\.0\.1:\d+)/', backend.lines.get_nowait())
                    if match:
                        return 'http://' + match[1]
            return until('isolated numeric loopback address', address)
        base = start(1)
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
        def calls():
            path = host / 'calls.jsonl'
            return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []
        def facts():
            return {str(path.relative_to(root / 'data/runs')): path.read_bytes() for path in (root / 'data/runs').glob('*/*') if path.is_file()}
        def task(name, kind):
            result = http('/api/tasks', {'name': name, 'taskType': kind, 'agent': 'omp', 'chatName': 'Same source and cwd', 'model': 'default', 'thinking': 'default', 'prompt': 'NEVER-REPLAY-TRIGGER-731'})
            return next(task['id'] for task in result['tasks'] if task['name'] == name)
        manager = task('Source manager', 'manager')
        worker = task('Worker', 'normal')
        http('/api/scheduler/bots/save', {'id': 'worker', 'revision': None, 'content': json.dumps({'name': 'Worker', 'when': '', 'plan_guide': '', 'node_guide': '', 'task_id': worker})})
        http('/api/zones', {'id': 'zone', 'path': str(root / 'zone')})
        http('/api/scheduler/bots/save', {'id': 'nested', 'revision': None, 'content': json.dumps({'name': 'Nested', 'when': '', 'plan_guide': '', 'node_guide': '', 'map_file': 'inner.json'})})
        save(root / 'maps/inner.json', {'title': 'Inner', 'description': '', 'nodes': {'internal': {'title': 'Internal work', 'bot_id': 'worker', 'input': 'INNER', 'zone': 'zone', 'blocked_by': []}}})
        save(root / 'maps/outer.json', {'title': 'Outer', 'description': '', 'nodes': {}})
        draft = http('/api/scheduler/maps/outer.json/nodes/new')
        http('/api/scheduler/maps/outer.json/nodes/save', {'id': 'A', 'revision': draft['revision'], 'create': True, 'content': json.dumps({'title': 'Call', 'bot_id': 'nested', 'manager_task_id': manager, 'input': 'BUSINESS-TRIGGER-829', 'zone': 'zone', 'blocked_by': []})})
        native('resume-root')
        parent = http('/api/scheduler/maps/outer.json/runs', {'managerTaskId': manager, 'zone': 'zone', 'userInput': 'ROOT-TRIGGER-905'}, 201)['id']
        prepared = action('resume-root', 'prepare_inputs', fileInputs=[], confirmed=True,
                          reason='Controlled resume fixture text reviewed; no file inputs')
        require((prepared.get('changed') or prepared.get('stateCommitted')) and prepared['snapshot']['inputsReady']
                    and not prepared.get('error') and not prepared.get('historyError'), prepared)
        native('resume-old')
        old = action('resume-root', 'dispatch', nodes=['A'])['snapshot']['nodes']['A']['execution']
        action('resume-root', 'fail', nodeId='A', reason='Original failure independently confirmed')
        native('resume-current')
        current = action('resume-root', 'retry', nodeId='A', confirmed=True, reason='Explicitly authorize independent retry')['snapshot']['nodes']['A']['execution']
        require(old['id'] != current['id'] and old['childRunId'] != current['childRunId'], 'Need genuine old and current executions')
        catalog('latest')
        def resume(session, revision='latest'):
            native(session)
            before, offset = facts(), len(calls())
            result = http('/api/chats/resume', {'sessionId': session})
            require(result['outcome'] == 'restored', result)
            effects = calls()[offset:]
            starts = [call for call in effects if call['kind'] == 'agent_start']
            require(len(starts) == 1, effects)
            argv = starts[0]['args']
            require(all(str(root / f'{revision}-{role}.ts') in argv for role in ['manager', 'node']), argv)
            require(argv[-2:] == ['--resume', session] and '--model' not in argv and '--thinking' not in argv, argv)
            require(not any(call['kind'] in ['agent_prompt', 'agent_input'] for call in effects), 'Resume replayed input')
            require(facts() == before, 'Resume changed parent/child/execution facts or created a Run')
        for session, execution in [('resume-current', current), ('resume-old', old)]:
            resume(session)
            status = action(session, 'status')['snapshot']
            require(status['id'] == execution['childRunId'] and status['parent'] == {'runId': parent, 'nodeId': 'A', 'executionId': execution['id']}, status)
        evidence['verified'].append('P32-current-retired-dual-role-current-environment-no-replay')
        # Same trusted source/name/cwd is not a parent-node identity. A
        # connect-bot event cannot establish source Task provenance; use the
        # actual public Chat launch, which records that trusted association.
        native('unrelated-same-environment')
        preview = http('/api/chats/preview', {'zone': 'zone', 'taskId': manager})
        creation = http('/api/chats', {'zone': 'zone', 'taskId': manager, 'chatId': preview['chatId'], 'input': 'STANDALONE-ONLY'}, 202)
        def created():
            status = http('/api/chats/' + creation['id'])
            require(status['stage'] != 'failed', status)
            return status['stage'] == 'succeeded'
        until('standalone Chat accepted', created)
        history = http('/api/sessions/history')['sessions']
        child_history = next(row for row in history if row['sessionId'] == 'resume-current')
        unrelated = next(row for row in history if row['sessionId'] == 'unrelated-same-environment')
        for key in ['sessionName', 'cwd']:
            require(unrelated[key] == child_history[key], ('Standalone sentinel differs', key))
        persisted_history = json.loads((root / 'data/history.json').read_text())
        sources = {row['sessionId']: row.get('sourceTaskId') for row in persisted_history}
        require(sources['unrelated-same-environment'] == sources['resume-current'] == manager, 'Trusted source associations differ')
        evidence['sourceAssociations'] = sources
        offset = len(calls())
        require(http('/api/chats/resume', {'sessionId': 'unrelated-same-environment'})['outcome'] == 'restored', 'Unrelated Chat restore failed')
        argv = next(call['args'] for call in calls()[offset:] if call['kind'] == 'agent_start')
        require(str(root / 'latest-manager.ts') in argv and str(root / 'latest-node.ts') not in argv, 'Guessed node role from source/name/cwd')
        action('unrelated-same-environment', 'status', 403)
        # Real disk faults at the approved persistence seam, not private code
        # mutation: a broken reciprocal link or duplicate identity must reject
        # before any new Tab, agent or input, rather than grant a partial role.
        child_file = root / 'data/runs' / current['childRunId'] / 'run.json'
        original = child_file.read_bytes()
        duplicate = root / 'data/runs/duplicate-identity'
        try:
            for fault in ['wrong-parent', 'duplicate-identity']:
                if fault == 'wrong-parent':
                    value = json.loads(original)
                    value['parent']['executionId'] = 'NOT-THE-ORIGINAL-EXECUTION'
                    save(child_file, value)
                else:
                    child_file.write_bytes(original)
                    duplicate.mkdir()
                    value = json.loads(original)
                    value['id'] = 'duplicate-identity'
                    save(duplicate / 'run.json', value)
                    for name in ['history.jsonl', 'messages.jsonl']:
                        (duplicate / name).write_text('')
                native('resume-current')
                saved, offset = facts(), len(calls())
                rejected = http('/api/chats/resume', {'sessionId': 'resume-current'}, 502)
                require(rejected['code'] == 'execution-association-invalid', rejected)
                require(not any(call['kind'] in ['tab_create', 'agent_start', 'agent_prompt', 'agent_input'] for call in calls()[offset:]), 'Illegal identity created host effects')
                require(facts() == saved, 'Rejected resume changed saved facts')
        finally:
            child_file.write_bytes(original)
            if duplicate.exists():
                shutil.rmtree(duplicate)
        evidence['verified'].append('P32-no-name-cwd-inference-invalid-parent-and-duplicate-rejected')
        # Independent literal pre-nesting record: deliberately no parent,
        # childRunId, mapInputs, outputs or other new business field aliases.
        legacy = root / 'data/runs/legacy-independent'
        legacy.mkdir()
        save(legacy / 'run.json', {'id': 'legacy-independent', 'map': 'retired.json', 'createdAt': 100, 'managerTaskId': 'old-task', 'zone': 'C:/legacy', 'workspaceName': 'old-workspace', 'userInput': 'OLD INPUT', 'manager': {'sessionId': 'legacy-manager', 'workspaceId': 'w-old', 'tabId': 't-old', 'paneId': 'p-old', 'agentName': 'a-old'}, 'status': 'running', 'finished': None, 'nodes': {'A': {'status': 'pending', 'execution': None, 'failure': None, 'completion': None, 'previousExecutions': []}}})
        for name in ['history.jsonl', 'messages.jsonl']:
            (legacy / name).write_text('')
        before = facts()
        backend.stop()
        backend = None
        base = start(2)
        ids = [parent, old['childRunId'], current['childRunId'], 'legacy-independent']
        before_calls = calls()
        details = {id: http('/api/workflow/runs/' + id) for id in ids}
        require(all(not detail['managed'] for detail in details.values()), 'Restart reclaimed management')
        for execution in [old, current]:
            require(details[execution['childRunId']]['run']['parent'] == {'runId': parent, 'nodeId': 'A', 'executionId': execution['id']}, 'Restart lost reverse link')
        require(details[parent]['run']['nodes']['A']['execution'] == current and details[parent]['run']['nodes']['A']['previousExecutions'][0]['execution'] == old, 'Restart lost attempts')
        require(details['legacy-independent']['run']['nodes']['A']['status'] == 'pending', 'Legacy was treated as corrupt')
        require(calls() == before_calls and facts() == before, 'Startup/read caused side effects')
        # Every public manager operation is rejected, both before and after Chat
        # restore, including manual recovery/reconciliation and explicit resume.
        def denied(session):
            for name, fields in [('status', {}), ('collect', {}), ('pause', {}), ('resume', {}), ('dispatch', {'nodes': ['A']}), ('complete', {'nodeId': 'A', 'reason': 'No takeover'}), ('finish', {}), ('fail', {'nodeId': 'A', 'reason': 'No takeover'}), ('recover', {'nodeId': 'A', 'confirmed': True, 'reason': 'No takeover'}), ('retry', {'nodeId': 'A', 'confirmed': True, 'reason': 'No takeover'}), ('reconcile_dispatch', {'nodeId': 'A', 'finding': 'uncertain', 'reason': 'No takeover'})]:
                action(session, name, 403, **fields)
            http('/api/workflow/node/report', {'sessionId': session, 'body': 'No takeover', 'canComplete': False}, 403)
        for session in ['resume-root', 'resume-old', 'resume-current', 'legacy-manager']:
            denied(session)
        require(calls() == before_calls, 'Denied operations contacted host')
        for session in ['resume-current', 'resume-old']:
            resume(session)
            offset = len(calls())
            denied(session)
            require(len(calls()) == offset, 'Chat restore granted takeover')
        require(facts() == before, 'Restart/read/rejection/resume rewrote historical facts')
        evidence['verified'].extend(['P33-actual-process-restart-parent-child-and-legacy-readable', 'P33-all-role-operations-denied-before-after-restore'])
        evidence['runIds'] = ids
    except BaseException:
        evidence['errors'].append(traceback.format_exc())
    finally:
        if backend is not None:
            try:
                backend.stop()
            except BaseException:
                evidence['cleanupErrors'].append(traceback.format_exc())
        path = root / 'evidence.json'
        path.write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding='utf-8')
        print(path)
    if evidence['errors'] or evidence['cleanupErrors']:
        raise RuntimeError(json.dumps(evidence['errors'] + evidence['cleanupErrors'], ensure_ascii=False))


if __name__ == '__main__':
    main()
