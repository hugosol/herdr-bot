"""Real-model existing Task/Map Bot guide revision acceptance.

Uses the shipped five-node revision Map and declared native hosts. Shared
creation infrastructure provides isolation/transport, not business decisions or
revision coverage. Missing dependencies and cleanup failures are never skipped.
"""
import argparse
import json
from pathlib import Path
import runpy
import sys

from developer_paths import REPO

runtime = runpy.run_path(str(Path(__file__).with_name('accept-workflow-nesting-create.py')))
PROMISES = ['P40', 'P42', 'P43', 'P44', 'P45']
DEDICATED = 'task-revise-bot-baseline-v1'


class ReviseAcceptance(runtime['CreateAcceptance']):
    def __init__(self, args):
        super().__init__(args)
        self.evidence.update(scenario='workflow-nesting-revise', promises=PROMISES,
            coverage={key: {'verified': False} for key in PROMISES},
            taskPairs={DEDICATED: {'standalone': False, 'workflow': False}},
            revisionSpecificTasks=[DEDICATED])

    def prepare(self):
        # Even an unavailable real dependency must leave attributable evidence.
        for name, root, lock in [('herdr-bot', REPO, 'backend/Cargo.lock'), ('toolkit', self.args.toolkit, 'bun.lock')]:
            identity = {'root': str(root), 'lockSha256': runtime['digest'](root / lock)}
            for label, argv in [('revision', ['rev-parse', 'HEAD']), ('status', ['status', '--porcelain']), ('diff', ['diff', '--binary', 'HEAD'])]:
                identity[label] = runtime['subprocess'].run(['git', *argv], cwd=root, capture_output=True,
                    text=True, encoding='utf-8', check=True).stdout
            self.evidence.setdefault('sources', {})[name] = identity
        super().prepare()
        for name in ['accept-workflow-nesting-revise.py', 'workflow_revise_acceptance.py', 'workflow_revise_safety.py']:
            path = REPO / 'scripts' / name
            self.evidence['runnerSources'][str(path)] = runtime['digest'](path)
        for relative in ['workflow-maps/enhance-existing-bot--bc-301b25d9f8d6.json', 'bots/prepare-bot-revision.json']:
            path = REPO / 'meta-data' / relative
            self.evidence['businessSourceIdentity'][str(path)] = runtime['digest'](path)
        self.evidence.pop('creationSpecificTasks', None)

    def await_chat(self, chat, after=None):
        history = super().await_chat(chat, after)
        runtime['require'](not any(message.get('stopReason') in ['error', 'aborted']
            for message in self.messages(history)), 'A real native model failure cannot be hidden by continuation')
        runtime['require'](all(record.get('status') == 200 and not record.get('error') for record in self.model_wire.records if 'ended' in record),
            'Actual model transport failed; no retry or later response can erase it')
        return history

    def exercise(self):
        from workflow_revise_acceptance import exercise
        try:
            exercise(self)
        finally:
            try:
                self.observe_processes()
            except BaseException:
                error = runtime['traceback'].format_exc()
                self.evidence.setdefault('processObservationErrors', []).append(error)
                self.evidence['cleanupErrors'].append(error)
                self.evidence['passed'] = False


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['backend', 'toolkit', 'paths-file', 'model-config', 'evidence-dir']:
        parser.add_argument('--' + name, type=lambda value: Path(value).resolve(), required=True)
    parser.add_argument('--hosts', choices=['declared', 'omp,pi'], required=True)
    for name in ['herdr', 'pi-package', 'omp-package', 'pi-state-extension', 'omp-state-extension',
                 'wayfinder', 'wayfinder-skills', 'pi-extensions', 'codebase-memory-mcp', 'shell-path']:
        parser.add_argument('--' + name, type=Path)
    parser.add_argument('--node', default='node')
    parser.add_argument('--bun', default='bun')
    parser.add_argument('--timeout', type=int, default=300)
    args = parser.parse_args()
    runtime['require'](1 <= args.timeout <= 900, '--timeout must be 1..900 seconds')
    args.repo = REPO
    if args.hosts == 'declared':
        case = ReviseAcceptance(args)
        case.parser = parser
        return case.run()
    args.evidence_dir.mkdir(parents=True, exist_ok=False)
    results = {'passed': False, 'scenario': 'workflow-nesting-revise', 'requestedHosts': ['omp', 'pi'],
        'verifiedHosts': [], 'coverage': {key: {'verified': False} for key in PROMISES}, 'hosts': {}, 'cleanupErrors': []}
    for host in ['omp', 'pi']:
        native_args = argparse.Namespace(**vars(args))
        native_args.execution_host = host
        native_args.evidence_dir = args.evidence_dir / host
        case = ReviseAcceptance(native_args)
        case.parser = parser
        code = case.run()
        results['hosts'][host] = {'exit': code, 'evidence': str(case.root / 'evidence.json')}
        results['cleanupErrors'].extend(case.evidence['cleanupErrors'])
        if code == 0:
            results['verifiedHosts'].append(host)
    results['passed'] = results['verifiedHosts'] == ['omp', 'pi'] and not results['cleanupErrors']
    for key in PROMISES:
        results['coverage'][key]['verified'] = results['passed']
    (args.evidence_dir / 'evidence.json').write_text(json.dumps(results, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps(results, ensure_ascii=False, indent=2))
    return 0 if results['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
