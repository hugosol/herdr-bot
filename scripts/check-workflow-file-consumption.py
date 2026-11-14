"""Public HTTP/process smoke for ordinary workflow file consumption (P10-P16).

Uses only this worktree's built safe backend shell and controlled Herdr fixture.
It proves file checks, persisted bindings and host effects, not native agent work.
Every run owns its directories and OS-assigned numeric loopback port. No daemon,
credentials, LAN, mDNS, real shutdown, model or business-operation retries.
"""
import argparse
import contextlib
import json
from pathlib import Path
import time

from acceptance.host.smoke_support import deny_read, dump, require
from acceptance.workflow.smoke import Smoke


class FileConsumptionSmoke(Smoke):
    def run(self):
        self.prepare()
        path = self.root / 'business/material.txt'
        path.write_bytes(b'ORIGINAL')
        for condition in ['missing', 'locked']:
            candidate = path if condition == 'locked' else self.root / 'business/missing.txt'
            with deny_read(candidate) if condition == 'locked' else contextlib.nullcontext():
                before = self.calls()
                for entry in ['preflight', 'runs']:
                    self.http(self.route + '/' + entry, {'managerTaskId': self.tasks['manager'], 'zone': 'manager',
                              'userInput': '', 'fileInputs': [{'id': 'brief', 'path': str(candidate)}]}, status=400)
                require(self.calls() == before, 'invalid root input started host work')
        for source in ['file', 'node', 'run']:
            for condition in ['readable', 'missing', 'locked']:
                path = self.root / ('manager/direct.txt' if source == 'file' else 'business/material.txt')
                path.write_bytes(b'ORIGINAL')
                rule = {'id': 'local', 'type': source}
                rule.update({'path': 'direct.txt'} if source == 'file' else {'node_id': 'A', 'output_id': 'result'} if source == 'node' else {'input_id': 'brief'})
                self.rule('B', [rule])
                manager = f'{source}-{condition}-manager'
                run = self.start(manager, [{'id': 'brief', 'path': str(path)}])
                if source == 'node':
                    producer = f'{source}-{condition}-producer'
                    self.dispatch(manager, 'A', producer)
                    self.report(producer, path)
                if condition == 'missing':
                    path.unlink()
                with deny_read(path) if condition == 'locked' else contextlib.nullcontext():
                    before = self.calls()
                    result = self.dispatch(manager, 'B', f'{source}-{condition}-consumer')
                    expected = 'running' if condition == 'readable' else 'failed'
                    require(result['results'][0]['status'] == expected, result)
                    stored = json.loads((self.root / 'runs' / run['id'] / 'run.json').read_text(encoding='utf-8'))
                    require(stored['nodes']['B']['status'] == expected, stored)
                    if condition != 'readable':
                        require(self.calls() == before, 'unreadable file started host work')
                        require(stored['nodes']['B']['failure']['stage'] == 'resolve', stored)
                    else:
                        require(stored['nodes']['B']['execution']['inputs'] == [{'id': 'local', 'path': str(path)}], stored)
                        prompt = next(call['text'] for call in reversed(self.calls()) if call['kind'] == 'agent_prompt')
                        require(f'local: {path}\n' in prompt, prompt)
                self.evidence['scenarios'].append({'source': source, 'condition': condition, 'run': run['id'], 'status': expected})
        # Address update after dispatch, then mutation/deletion at the old bound
        # path. The observer reads the binding, not a recomputed source address.
        self.rule('B', [{'id': 'renamed', 'type': 'node', 'node_id': 'A', 'output_id': 'result'}])
        self.rule('C', [{'id': 'renamed', 'type': 'node', 'node_id': 'A', 'output_id': 'result'}])
        old, new = self.root / 'business/old.txt', self.root / 'business/new.txt'
        old.write_bytes(b'OLD')
        new.write_bytes(b'NEW')
        run = self.start('fixed-manager', [])
        self.dispatch('fixed-manager', 'A', 'fixed-producer')
        self.report('fixed-producer', old)
        self.dispatch('fixed-manager', 'B', 'fixed-old')
        self.report('fixed-producer', new)
        self.dispatch('fixed-manager', 'C', 'fixed-new')
        root = self.root / 'runs' / run['id']
        stored = json.loads((root / 'run.json').read_text(encoding='utf-8'))
        require(stored['nodes']['B']['execution']['inputs'] == [{'id': 'renamed', 'path': str(old)}], stored)
        require(stored['nodes']['C']['execution']['inputs'] == [{'id': 'renamed', 'path': str(new)}], stored)
        old.write_bytes(b'CHANGED')
        bound = Path(stored['nodes']['B']['execution']['inputs'][0]['path'])
        require(bound.read_bytes() == b'CHANGED', 'content snapshot or lock')
        old.unlink()
        before = self.calls()
        records = {name: (root / name).read_bytes() for name in ['run.json', 'history.jsonl', 'messages.jsonl']}
        time.sleep(0.3)
        require(not old.exists() and self.calls() == before, 'automatic repair or retry')
        require(all((root / name).read_bytes() == data for name, data in records.items()), 'autonomous record changes')
        self.evidence['scenarios'].append({'bindingMutationDeletion': True, 'run': run['id'], 'observationMs': 300})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--target-dir', type=Path, required=True, help='Independent CARGO_TARGET_DIR built from this worktree')
    args = parser.parse_args()
    smoke = FileConsumptionSmoke(args.target_dir)
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
