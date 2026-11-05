"""P38 process smoke: public definitions/run HTTP, real records, controlled host.

Build this worktree's frontend, herdr-fixture and safe_shutdown_shell first.
No credentials, native agents, user data, daemon, LAN/mDNS or real shutdown.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import json
from pathlib import Path
import runpy
import time

support = runpy.run_path(str(Path(__file__).with_name('check-workflow-file-consumption.py')))
Smoke, require, dump, deny_read = (support[name] for name in ['Smoke', 'require', 'dump', 'deny_read'])


class HistorySmoke(Smoke):
    def records(self, identity):
        root = self.root / 'runs' / identity
        return {name: (root / name).read_bytes() for name in ['run.json', 'history.jsonl', 'messages.jsonl']}

    def node(self, map_name, identity, content, create):
        route = '/api/scheduler/maps/' + map_name
        draft = self.http(route + ('/nodes/new' if create else '/nodes/edit'), None if create else {'id': identity})
        return self.http(route + '/nodes/save', {'id': identity, 'content': json.dumps(content),
                                                'revision': draft['revision'], 'create': create})

    def run(self):
        self.prepare()
        # Only empty Map containers are provisioned directly. Actual node/Bot
        # definitions, revisions, startup and deletion use production HTTP.
        dump(self.root / 'maps/plain.json', {'title': 'Same display name', 'description': '', 'nodes': {}})
        self.node('plain.json', 'A', {'title': 'A', 'bot_id': 'worker', 'input': 'ORDINARY', 'zone': 'node', 'blocked_by': []}, True)
        self.identity('smoke-unassociated')
        plain = self.http('/api/scheduler/maps/plain.json/runs', {'managerTaskId': self.tasks['manager'], 'zone': 'manager', 'userInput': ''}, status=201)['run']
        before = self.calls()
        self.http('/api/scheduler/maps/plain.json/delete', {'deleteHistory': True})
        require(not (self.root / 'runs' / plain['id']).exists(), 'ordinary history was not removed')
        require(self.calls() == before, 'ordinary deletion called host')
        self.evidence['scenarios'].append({'unassociatedDeletion': True, 'run': plain['id']})

        dump(self.root / 'maps/inner.json', {'title': 'Same display name', 'description': '', 'nodes': {}})
        self.node('inner.json', 'A', {'title': 'A', 'bot_id': 'worker', 'input': 'INNER', 'zone': 'manager', 'blocked_by': []}, True)
        self.http('/api/scheduler/bots/save', {'id': 'nested', 'content': json.dumps({'name': 'Not an execution identity', 'when': '', 'plan_guide': '', 'node_guide': '', 'map_file': 'inner.json'}), 'revision': None})
        self.node('work.json', 'A', {'title': 'A', 'bot_id': 'nested', 'manager_task_id': self.tasks['manager'], 'input': 'CHILD', 'zone': 'node', 'blocked_by': []}, False)
        parent = self.start('smoke-parent', [])
        parent_id = parent['id']
        release = self.root / 'release-child-identity'
        dump(self.root / 'host/behavior.json', {'native_session': {'source': 'herdr:omp', 'agent': 'omp', 'kind': 'id', 'value': 'smoke-child'},
                                               'native_cwd': str(self.root / 'node'),
                                               'native_session_gate': {'wait_for_file': str(release), 'timeout_ms': 30000}})
        with ThreadPoolExecutor(max_workers=1) as executor:
            pending = executor.submit(self.http, '/api/workflow/manager/dispatch', {'sessionId': 'smoke-parent', 'nodes': ['A']})
            original = None
            cleanup = []
            try:
                deadline = time.monotonic() + 15
                while sum(call['kind'] == 'agent_get' for call in self.calls()) < 3:
                    require(time.monotonic() < deadline, 'child did not reach actual identity gate')
                    time.sleep(0.01)
                stored = json.loads(self.records(parent_id)['run.json'])
                execution = stored['nodes']['A']['execution']
                child_id = execution['childRunId']
                child = json.loads(self.records(child_id)['run.json'])
                require(child['parent'] == {'runId': parent_id, 'nodeId': 'A', 'executionId': execution['id']}, child)
                require(child['manager'] is None, 'identity gate was not before child association')
                # Explicitly remove the live definition reference, not the saved
                # execution link, to reach the independent history guard.
                self.node('work.json', 'A', {'title': 'A', 'bot_id': 'worker', 'input': 'NEW DEFINITION ONLY', 'zone': 'node', 'blocked_by': []}, False)
                target = self.http('/api/scheduler/bots/check-delete', {'id': 'nested'})
                self.http('/api/scheduler/bots/delete', target)
                before_records = {identity: self.records(identity) for identity in [parent_id, child_id]}
                before_calls = self.calls()
                for map_name in ['work.json', 'inner.json']:
                    rejected = self.http(f'/api/scheduler/maps/{map_name}/delete', {'deleteHistory': True}, status=500)
                    require(rejected['code'] == 'map-history-delete-failed', rejected)
                    require(all(locator in rejected['message'] for locator in [parent_id, child_id, 'A', execution['id']]), rejected)
                    require((self.root / 'maps' / map_name).is_file(), 'rejection removed Map')
                    for identity in [parent_id, child_id]:
                        require(self.records(identity) == before_records[identity], 'rejection changed saved references')
                        self.http('/api/workflow/runs/' + identity)
                require(self.calls() == before_calls, 'rejection performed external rollback or retry')
                self.evidence['scenarios'].append({'childIdentityInterleaving': True, 'parent': parent_id, 'child': child_id,
                                                   'execution': execution['id'], 'bothDirectionsRejected': True})
                path = self.root / 'runs' / child_id / 'run.json'
                with deny_read(path):
                    rejected = self.http('/api/scheduler/maps/work.json/delete', {'deleteHistory': True}, status=500)
                    require('run.json' in rejected['message'], rejected)
                require(self.records(child_id) == before_records[child_id], 'unreadable scan changed data')
                self.evidence['scenarios'].append({'actualUnreadableAssociationRejected': True})
            except BaseException as error:
                original = error
            finally:
                try:
                    release.write_text('release owned child', encoding='utf-8')
                except BaseException as error:
                    cleanup.append('release: ' + repr(error))
                try:
                    result = pending.result(timeout=40)
                except BaseException as error:
                    cleanup.append('await child: ' + repr(error))
            require(original is None and not cleanup, {'original': repr(original) if original else None, 'cleanup': cleanup})
        parent = json.loads(self.records(parent_id)['run.json'])
        child = json.loads(self.records(child_id)['run.json'])
        require(parent['nodes']['A']['execution']['childRunId'] == child_id, parent)
        require(child['parent']['runId'] == parent_id, child)
        require(child['manager'] == parent['nodes']['A']['execution']['chat'] and child['manager'] is not None, (parent, child))
        require(result['results'][0]['status'] == 'running', result)
        self.evidence['scenarios'].append({'lateAssociationSavedWithoutDanglingReference': True, 'parent': parent_id, 'child': child_id})
        self.evidence['boundary'] = 'public HTTP / safe backend process / real persistent files and unreadability / controlled Herdr; not native agent or model quality'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target-dir', type=Path, required=True)
    smoke = HistorySmoke(parser.parse_args().target_dir)
    try:
        smoke.run()
    except BaseException as error:
        smoke.evidence['errors'].append(repr(error))
    finally:
        try:
            smoke.close()
        except BaseException as error:
            smoke.evidence['errors'].append('cleanup: ' + repr(error))
            dump(smoke.root / 'evidence.json', smoke.evidence)
    print(json.dumps(smoke.evidence, ensure_ascii=False, indent=2))
    return 1 if smoke.evidence['errors'] else 0


if __name__ == '__main__':
    raise SystemExit(main())
