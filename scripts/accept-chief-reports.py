"""Dual-host report acceptance through delivered native roles and public backend.

All explicit paths must be absolute. No dependency installation, user credential
discovery, model fallback or business retry. Incomplete evidence is nonzero.
"""
import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import runpy
import sys
import traceback

REPO = Path(__file__).resolve().parents[1]
PROMISES = {
    'P18': 'Nested and ordinary node reports retain their manager recipient',
    'P19': 'New chief accurately presents independent success/failure/blocked reports',
    'P20': 'Native approval/denial/cancel/timeout/unavailable with exact approved content',
    'P22': 'Real save failure, definite notification failure and lost post-commit response',
    'P28': 'Four result kinds ask satisfaction and wait without next work',
    'P36': 'Existing node report confirmation, persistence and completion gates',
    'P37': 'Backend owns shared facts; native consumers agree and architecture checks pass',
}


def absolute(value):
    path = Path(value)
    if not path.is_absolute():
        raise argparse.ArgumentTypeError('Expected an explicit absolute path')
    return path.resolve()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['herdr-bot-worktree', 'toolkit-worktree', 'backend', 'paths-file', 'model-config', 'evidence-dir']:
        parser.add_argument('--' + name, required=True, type=absolute)
    parser.add_argument('--hosts', required=True, choices=['omp,pi', 'omp', 'pi'])
    parser.add_argument('--timeout', type=int, choices=range(1, 601), default=180)
    parser.add_argument('--node', default='node')
    parser.add_argument('--bun', default='bun')
    parser.add_argument('--shell-path', type=absolute)
    args = parser.parse_args()
    args.evidence_dir.mkdir(parents=True, exist_ok=False)
    evidence = {'passed': False, 'coverage': {host: {key: {'verified': False, 'expected': value}
        for key, value in PROMISES.items()} for host in ['omp', 'pi']}, 'commands': [], 'cleanupErrors': [],
        'selectedHosts': args.hosts.split(','), 'modelPolicy': 'gpt-6-luna only; actual returned identity required; no retries'}
    try:
        def require(condition, message):
            if not condition:
                raise AssertionError(message)
        require(args.backend.is_file(), 'Missing current worktree backend: ' + str(args.backend))
        require(args.herdr_bot_worktree == REPO == Path.cwd().resolve(), 'Run from supplied herdr-bot worktree')
        require((args.toolkit_worktree / '.git').is_file(), 'Expected explicit toolkit Git worktree')
        require(args.model_config.is_file(), 'Missing authorized isolated model config')
        config = json.loads(args.model_config.read_text(encoding='utf-8'))
        require(config.get('model') == 'gpt-6-luna', 'Actual model must be gpt-6-luna; no fallback')
        require(config.get('apiKeyEnv') and os.environ.get(config['apiKeyEnv']), 'Missing explicitly authorized credential environment')
        args.repo, args.toolkit = args.herdr_bot_worktree, args.toolkit_worktree
        resources = runpy.run_path(str(REPO / 'scripts/accept-suite-pi-resources.py'))
        resources['preflight'](args, evidence)
        args.path_roots = evidence['paths']['roots']
        for host in args.hosts.split(','):
            name = host + '_state_extension'
            require(name in args.path_roots and Path(args.path_roots[name]).is_file(), 'Missing readonly roots.' + name)
            setattr(args, name, Path(args.path_roots[name]).resolve())
        # Existing enabling cases remain their owners' tests. Execute them against
        # these exact participating sources; never reuse another ticket's conclusion.
        import subprocess
        environment = dict(os.environ, HERDR_BOT_REPO=str(REPO), HERDR_BOT_METADATA=str(REPO / 'meta-data/task-options.toml'),
            HERDR_BOT_BINARY=str(args.backend.parent / 'examples/safe_shutdown_shell.exe'),
            CARGO_TARGET_DIR=str(args.backend.parent.parent),
            WORKFLOW_PROMPT_EVIDENCE_DIR=str(args.evidence_dir / 'native-protocol'),
            WORKFLOW_DUAL_EVIDENCE_DIR=str(args.evidence_dir / 'native-protocol'),
            CHIEF_REPORTS_EVIDENCE_DIR=str(args.evidence_dir / 'faults'))
        partitions = [
            ('P18', ['tests/harness-dock/nested-role-completion.test.ts']),
            ('P20', ['tests/harness-dock/native-chief-report.test.ts']),
            ('P22', ['tests/harness-dock/chief-report-backend-faults.test.ts']),
            ('P36', ['tests/harness-dock/node-report-contract.test.ts', 'tests/harness-dock/native-report-confirmation.test.ts']),
        ]
        mechanical = {}
        for promise, files in partitions:
            command = [args.bun, 'test', *files]
            if args.hosts != 'omp,pi':
                command.extend(['--test-name-pattern', '^' + args.hosts])
            log = args.evidence_dir / (promise + '-native.log')
            with log.open('w', encoding='utf-8') as output:
                result = subprocess.run(command, cwd=args.toolkit, env=environment, stdout=output, stderr=subprocess.STDOUT)
            evidence['commands'].append({'argv': command, 'cwd': str(args.toolkit), 'exit': result.returncode, 'log': str(log)})
            mechanical[promise] = {'verified': result.returncode == 0, 'command': len(evidence['commands']) - 1}
        architectural = True
        for command in [[args.bun, 'run', 'check:integrations'], [args.bun, 'test', 'tests/framework/import-boundary.test.ts']]:
            log = args.evidence_dir / ('architecture-' + str(len(evidence['commands'])) + '.log')
            with log.open('w', encoding='utf-8') as output:
                result = subprocess.run(command, cwd=args.toolkit, env=environment, stdout=output, stderr=subprocess.STDOUT)
            evidence['commands'].append({'argv': command, 'cwd': str(args.toolkit), 'exit': result.returncode, 'log': str(log)})
            architectural = architectural and result.returncode == 0
        native = runpy.run_path(str(REPO / 'scripts/chief_reports_native_acceptance.py'))['ChiefReportsAcceptance']
        for host in args.hosts.split(','):
            selected = copy.copy(args)
            selected.host = host
            run = native(selected)
            status = run.run()
            for key, result in run.evidence['coverage'].items():
                if key in mechanical:
                    result = mechanical[key]
                elif key == 'P37':
                    result = {**result, 'verified': result['verified'] and architectural and mechanical['P22']['verified'],
                              'sameFactConsumers': mechanical['P22']}
                evidence['coverage'][host][key].update(result, evidence=str(run.root / 'evidence.json'))
            evidence['cleanupErrors'].extend(run.evidence['cleanupErrors'])
            if status:
                evidence.setdefault('hostFailures', []).append(host)
        evidence['passed'] = all(item['verified'] for host in evidence['coverage'].values() for item in host.values()) and not evidence['cleanupErrors']
    except BaseException:
        evidence['error'] = traceback.format_exc()
    evidence['entrypoint'] = {'path': str(Path(__file__).resolve()), 'sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    report = args.evidence_dir / 'evidence.json'
    report.write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps({'passed': evidence['passed'], 'evidence': str(report)}, indent=2))
    return 0 if evidence['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
