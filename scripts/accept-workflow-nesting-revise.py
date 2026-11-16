"""Real-model existing Task/Map Bot guide revision acceptance.

Uses the shipped five-node revision Map and declared native hosts. Shared
creation infrastructure provides isolation/transport, not business decisions or
revision coverage. Missing dependencies and cleanup failures are never skipped.
"""
import argparse
import json
from pathlib import Path
import sys

from developer_paths import REPO
from workflow_revise_acceptance import CASE_NAMES, select_cases

from acceptance.workflow.create import CreateAcceptance, digest
from acceptance.host.model import message_text
from acceptance.host.processes import require, until
import subprocess
import traceback
PROMISES = ['P40', 'P42', 'P43', 'P44', 'P45']
DEDICATED = 'task-revise-bot-baseline-v1'


class ReviseAcceptance(CreateAcceptance):
    def __init__(self, args):
        super().__init__(args)
        self.evidence.update(scenario='workflow-nesting-revise', promises=PROMISES,
            coverage={key: {'verified': False} for key in PROMISES},
            taskPairs={DEDICATED: {'standalone': False, 'workflow': False}},
            revisionSpecificTasks=[DEDICATED], requestedCases=list(args.cases),
            filtered=set(args.cases) != set(CASE_NAMES),
            subsetPassed=False)

    def run(self):
        code = super().run()
        if not self.evidence['filtered']:
            return code
        self.evidence['subsetPassed'] = bool(self.evidence['subsetPassed']
            and not self.evidence.get('verificationError') and not self.evidence['cleanupErrors'])
        self.evidence['passed'] = False
        self.evidence['verifiedHosts'] = []
        (self.root / 'evidence.json').write_text(json.dumps(self.evidence, ensure_ascii=False, indent=2), encoding='utf-8')
        return 0 if self.evidence['subsetPassed'] else 1

    def prepare(self):
        # Even an unavailable real dependency must leave attributable evidence.
        for name, root, lock in [('herdr-bot', REPO, 'backend/Cargo.lock'), ('toolkit', self.args.toolkit, 'bun.lock')]:
            identity = {'root': str(root), 'lockSha256': digest(root / lock)}
            for label, argv in [('revision', ['rev-parse', 'HEAD']), ('status', ['status', '--porcelain']), ('diff', ['diff', '--binary', 'HEAD'])]:
                identity[label] = subprocess.run(['git', *argv], cwd=root, capture_output=True,
                    text=True, encoding='utf-8', check=True).stdout
            self.evidence.setdefault('sources', {})[name] = identity
        super().prepare()
        for name in ['accept-workflow-nesting-revise.py', 'workflow_revise_acceptance.py', 'workflow_revise_safety.py']:
            path = REPO / 'scripts' / name
            self.evidence['runnerSources'][str(path)] = digest(path)
        for relative in ['workflow-maps/enhance-existing-bot--bc-301b25d9f8d6.json', 'bots/prepare-bot-revision.json']:
            path = REPO / 'meta-data' / relative
            self.evidence['businessSourceIdentity'][str(path)] = digest(path)
        self.evidence.pop('creationSpecificTasks', None)

    def await_chat(self, chat, after=None):
        def ready():
            history = self.native(chat)
            if not history:
                return None
            messages = self.messages(history)
            if after and not any(m.get('role') == 'user' and after in message_text(m) for m in messages):
                return None
            terminal = self.command([self.args.herdr, 'pane', 'read', chat['paneId']])
            if '发送报告？（仅批准本次报告发送）' in terminal and 'Yes' in terminal and 'No' in terminal:
                self.evidence.setdefault('reportConfirmationPreviews', {})[chat['sessionId']] = terminal
                return history
            if not messages or messages[-1].get('role') != 'assistant':
                return None
            last = messages[-1]
            require(last.get('stopReason') not in ['error', 'aborted'], ('Native model error', last))
            return history if not any(p.get('type') == 'toolCall' for p in last.get('content', []) if isinstance(p, dict)) else None
        history = until('real Chat reaches user or report UI boundary', ready, self.args.timeout)
        require(not any(message.get('stopReason') in ['error', 'aborted']
            for message in self.messages(history)), 'A real native model failure cannot be hidden by continuation')
        require(all(record.get('status') == 200 and not record.get('error') for record in self.model_wire.records if 'ended' in record),
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
                error = traceback.format_exc()
                self.evidence.setdefault('processObservationErrors', []).append(error)
                self.evidence['cleanupErrors'].append(error)
                self.evidence['passed'] = False


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['backend', 'toolkit', 'paths-file', 'model-config', 'evidence-dir']:
        parser.add_argument('--' + name, type=lambda value: Path(value).resolve(), required=True)
    parser.add_argument('--hosts', choices=['declared', 'omp,pi'], required=True)
    parser.add_argument('--cases', type=select_cases, default='all',
        help='all (default), or unique comma-separated names: ' + ', '.join(CASE_NAMES))
    for name in ['herdr', 'pi-package', 'omp-package', 'pi-state-extension', 'omp-state-extension',
                 'wayfinder', 'wayfinder-skills', 'pi-extensions', 'codebase-memory-mcp', 'shell-path']:
        parser.add_argument('--' + name, type=Path)
    parser.add_argument('--node', default='node')
    parser.add_argument('--bun', default='bun')
    parser.add_argument('--timeout', type=int, default=300)
    args = parser.parse_args()
    require(1 <= args.timeout <= 900, '--timeout must be 1..900 seconds')
    args.repo = REPO
    if args.hosts == 'declared':
        case = ReviseAcceptance(args)
        case.parser = parser
        return case.run()
    args.evidence_dir.mkdir(parents=True, exist_ok=False)
    results = {'passed': False, 'scenario': 'workflow-nesting-revise', 'requestedHosts': ['omp', 'pi'],
        'verifiedHosts': [], 'coverage': {key: {'verified': False} for key in PROMISES}, 'hosts': {}, 'cleanupErrors': [],
        'requestedCases': list(args.cases), 'filtered': set(args.cases) != set(CASE_NAMES), 'subsetPassed': False}
    successful_hosts = []
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
            successful_hosts.append(host)
            if not results['filtered']:
                results['verifiedHosts'].append(host)
    results['subsetPassed'] = results['filtered'] and successful_hosts == ['omp', 'pi'] and not results['cleanupErrors']
    results['passed'] = not results['filtered'] and results['verifiedHosts'] == ['omp', 'pi'] and not results['cleanupErrors']
    for key in PROMISES:
        results['coverage'][key]['verified'] = results['passed']
    (args.evidence_dir / 'evidence.json').write_text(json.dumps(results, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps(results, ensure_ascii=False, indent=2))
    return 0 if results['passed'] or results['subsetPassed'] else 1


if __name__ == '__main__':
    sys.exit(main())
