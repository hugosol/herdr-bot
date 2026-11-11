"""Ticket 18 public publication smoke, not owner 22 real-model acceptance.

Build this worktree frontend, fixture and safe_shutdown_shell with an isolated
CARGO_TARGET_DIR. Uses numeric loopback, owned files and no real shutdown/daemon.
"""
import argparse
import json
from pathlib import Path
import runpy

REPO = Path(__file__).resolve().parents[1]
shared = runpy.run_path(str(REPO / 'scripts/check-workflow-file-consumption.py'))
Smoke, require, dump, deny_read = [shared[key] for key in ['Smoke', 'require', 'dump', 'deny_read']]


class RevisionSmoke(Smoke):
    def run(self):
        self.prepare()
        self.evidence['boundary'] = 'public safe backend definitions/preflight/real files; no autonomous model or full business run'
        file = 'enhance-existing-bot--bc-301b25d9f8d6.json'
        definition = json.loads((REPO / 'meta-data/workflow-maps' / file).read_text(encoding='utf-8'))
        tasks = json.loads((REPO / 'meta-data/tasks.json').read_text(encoding='utf-8'))['tasks']
        dedicated = next(task for task in tasks if task['id'] == 'task-revise-bot-baseline-v1')
        require(dedicated['taskType'] == 'normal', dedicated)
        # Public Task creation exercises the dedicated shipped prompt; deployment
        # Suite and native role loading are deliberately not inferred here.
        content = {key: value for key, value in dedicated.items() if key not in ['id', 'suite']}
        created = self.http('/api/tasks', content)
        task = next(task for task in created['tasks'] if task['name'] == dedicated['name'])
        read = self.http('/api/tasks/' + task['id'])
        require(read['task']['prompt'] == dedicated['prompt'], read)
        for id in ['prepare-bot-revision', 'write-bot-when', 'write-bot-plan-guide', 'write-bot-node-guide', 'replace-text']:
            bot = {'name': id, 'when': '', 'plan_guide': '', 'node_guide': '', 'task_id': task['id'] if id == 'prepare-bot-revision' else self.tasks['normal']}
            self.http('/api/scheduler/bots/save', {'id': id, 'content': json.dumps(bot), 'revision': None})
            require(self.http('/api/scheduler/bots/edit', {'id': id})['content'] == bot, id)
        dump(self.root / 'maps' / file, {'title': 'Revision', 'description': '', 'nodes': {}})
        route = '/api/scheduler/maps/' + file
        self.http(route + '/inputs', {'input_requirements': definition.get('input_requirements', ''), 'input_files': definition['input_files'], 'revision': self.http(route + '/inputs')['revision']})
        for id, source in definition['nodes'].items():
            node = dict(source, zone='node')
            draft = self.http(route + '/nodes/new')
            self.http(route + '/nodes/save', {'id': id, 'content': json.dumps(node), 'revision': draft['revision'], 'create': True})
            require(self.http(route)['nodes'][id] == node, id)
        wrapper = json.loads((REPO / 'meta-data/bots/revise-bot-guides.json').read_text(encoding='utf-8'))
        wrapper.pop('id')
        self.http('/api/scheduler/bots/save', {'id': 'revise-bot-guides', 'content': json.dumps(wrapper), 'revision': None})
        require(self.http('/api/scheduler/bots/edit', {'id': 'revise-bot-guides'})['content'] == wrapper, wrapper)
        request, reference = self.root / 'request.md', self.root / 'reference.md'
        request.write_text('Revise worker Bot guides only; preserve its existing target and all other fields. Save authorized after three full-text confirmations.', encoding='utf-8')
        reference.write_text('Preserve authorization boundaries.', encoding='utf-8')
        def preflight(inputs, status=200):
            return self.http(route + '/preflight', {'managerTaskId': self.tasks['manager'], 'zone': 'manager', 'userInput': 'Revise guides only', 'fileInputs': inputs}, status=status)
        bound = [{'id': 'request', 'path': str(request)}]
        preflight([])
        preflight(bound)
        preflight(bound + [{'id': 'materials', 'path': str(reference)}])
        preflight(bound + [{'id': 'materials', 'path': str(self.root / 'missing.md')}], 400)
        before = self.http(route)
        with deny_read(reference):
            preflight(bound + [{'id': 'materials', 'path': str(reference)}], 400)
        require(self.http(route) == before, 'rejected preflight mutated definition')
        require(not self.calls(), 'publication or preflight started host')
        require(not list((self.root / 'runs').glob('*/run.json')), 'publication created Run')
        self.evidence['scenarios'].append({'definitionRoundtrip': True, 'dedicatedTaskRoundtrip': True, 'optionalAbsent': True, 'optionalPresent': True, 'missingRequiredDeferredToPreparation': True, 'missingOptionalFileRejected': True, 'realUnreadableRejected': True, 'noHostOrRun': True})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target-dir', type=Path, required=True)
    smoke = RevisionSmoke(parser.parse_args().target_dir)
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
