"""Ticket 11: real installed OMP/Pi result-approval failures and continued work.

An explicitly configured controlled provider can drive these mechanical cases;
no controlled output is claimed as a real model's business decision. Native UI,
sessions, tools, material edits, reports and backend executions remain real.
"""
import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import traceback

REPO = Path(__file__).resolve().parents[1]
CONDITIONS = ('no', 'cancel', 'timeout', 'unavailable')
PROMISES = ['P5/S2/non-Yes', 'P7', 'P8', 'P38/native-failures']
EXPECTED = {'approved': False, 'nodeStatus': 'running', 'automaticConfirmations': 0,
            'continueMaterial': 'CONTINUED-MATERIAL-731\n', 'reportBody': 'P7-CONTINUED-731',
            'reportCanComplete': False, 'newRequest': 'explicit-user-only'}


def absolute(value):
    path = Path(value)
    if not path.is_absolute():
        raise argparse.ArgumentTypeError('Expected an explicit absolute path')
    return path.resolve()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['herdr-bot-worktree', 'toolkit-worktree', 'backend', 'paths-file', 'model-config', 'evidence-dir']:
        parser.add_argument('--' + name, type=absolute, required=True)
    parser.add_argument('--hosts', choices=['omp,pi', 'omp', 'pi'], required=True)
    parser.add_argument('--timeout', type=int, choices=range(1, 601), default=180)
    parser.add_argument('--node', default='node')
    parser.add_argument('--bun', default='bun')
    parser.add_argument('--shell-path', type=absolute)
    parser.add_argument('--native-only', action='store_true',
                        help='Diagnostic native matrix only; never certifies complete ticket checks')
    args = parser.parse_args()
    args.evidence_dir.mkdir(parents=True, exist_ok=False)
    evidence = {'passed': False, 'selectedHosts': args.hosts.split(','), 'nativeExits': {},
        'coverage': {host: {condition: {'verified': False, 'promises': PROMISES, 'expected': EXPECTED}
            for condition in CONDITIONS} for host in ['omp', 'pi']}, 'checks': [], 'cleanupErrors': [], 'diagnosticNativeOnly': args.native_only}
    try:
        from acceptance.host.processes import require
        require(args.backend.is_file(), 'Missing current worktree backend: ' + str(args.backend))
        require(args.model_config.is_file(), 'Missing explicitly authorized isolated model configuration')
        config = json.loads(args.model_config.read_text(encoding='utf-8'))
        if config.get('mode') == 'controlled-native-failures':
            evidence['providerScope'] = 'Controlled provider; mechanical native confirmation/continuation only, not actual model business selection'
        else:
            require(config.get('model') == 'gpt-6-luna', 'Actual model must be gpt-6-luna; no fallback')
            require(config.get('model') and config.get('apiKeyEnv') and os.environ.get(config['apiKeyEnv']),
                    'Missing explicitly authorized model identity or credential environment')
            evidence['providerScope'] = 'Explicitly authorized actual model; requested and returned identity must match'
        args.repo, args.toolkit = args.herdr_bot_worktree, args.toolkit_worktree
        from acceptance.host.provenance import preflight, source_identity
        preflight(args, evidence)
        args.path_roots = evidence['paths']['roots']
        for host in ['omp', 'pi']:
            key = host + '_state_extension'
            require(key in args.path_roots and Path(args.path_roots[key]).is_file(), 'Missing readonly roots.' + key)
            setattr(args, key, Path(args.path_roots[key]).resolve())
        # The production binary is ONLY for the isolated business scenario. The
        # paired dock must use the separately built safe shell, never this binary.
        target = args.backend.parent.parent
        safe = args.backend.parent / 'examples/safe_shutdown_shell.exe'
        require(safe.is_file(), 'Build this worktree safe_shutdown_shell for paired shutdown regression')
        environment = dict(os.environ, CARGO_TARGET_DIR=str(target), HERDR_BOT_REPO=str(args.repo),
            HERDR_BOT_BINARY=str(safe), HERDR_BOT_METADATA=str(args.repo / 'meta-data'),
            PI_NODE=args.node, PI_CLI=str(args.pi_package / 'dist/bundle/cli.js'),
            OMP_CLI=str(args.omp_package / 'dist/cli.js'))
        commands = [(args.repo, ['npm', '--prefix', 'frontend', 'run', 'build']),
            (args.repo, ['cargo', 'build', '--manifest-path', 'backend/Cargo.toml', '--bin', 'herdr-bot',
                        '--bin', 'herdr-fixture', '--bin', 'herdr-paths', '--example', 'safe_shutdown_shell']),
            (args.repo, ['node', 'scripts/test-backend.mjs']),
            (args.repo, ['npm', '--prefix', 'frontend', 'test']),
            (args.repo, ['npm', '--prefix', 'frontend', 'run', 'test:browser:build', '--', '--workers=1']),
            (args.toolkit, [args.bun, 'run', 'typecheck']), (args.toolkit, [args.bun, 'run', 'test']),
            (args.toolkit, [args.bun, 'run', 'check:integrations']),
            (args.toolkit, [args.bun, 'test', 'tests/harness-dock/native-node-report.test.ts',
                            'tests/harness-dock/node-report-contract.test.ts'])]
        require(subprocess.run([args.bun, '--version'], capture_output=True, text=True, check=True).stdout.strip() == '1.4.4',
                'Use the existing Bun 1.4.4 runtime; no host installation')
        for index, (cwd, argv) in enumerate([] if args.native_only else commands):
            log = args.evidence_dir / f'check-{index:02d}.log'
            with log.open('w', encoding='utf-8') as stream:
                result = subprocess.run(argv, cwd=cwd, env=environment, stdout=stream, stderr=subprocess.STDOUT)
            evidence['checks'].append({'cwd': str(cwd), 'argv': argv, 'exit': result.returncode, 'log': str(log),
                'CARGO_TARGET_DIR': str(target), 'HERDR_BOT_BINARY': str(safe)})
        # Preserve all check failures. Native scenarios still provide diagnostic
        # evidence; a failed regression can never certify this ticket.
        from node_approval_failures_acceptance import ApprovalFailuresAcceptance
        evidence['nativeSources'] = {name: source_identity(root, lock, args.evidence_dir)
            for name, root, lock in [('herdr-bot', args.repo, 'backend/Cargo.lock'), ('toolkit', args.toolkit, 'bun.lock')]}
        for host in args.hosts.split(','):
            for condition in CONDITIONS:
                scenario_args = copy.copy(args)
                scenario_args.host, scenario_args.condition = host, condition
                scenario_args.evidence_dir = args.evidence_dir / condition
                run = ApprovalFailuresAcceptance(scenario_args)
                status = run.run()
                evidence['nativeExits'][host + '/' + condition] = status
                evidence['coverage'][host][condition].update(run.evidence['coverage'], exit=status,
                    evidence=str(run.root / 'evidence.json'))
                evidence['cleanupErrors'].extend(run.evidence['cleanupErrors'])
        evidence['sourcesAfter'] = {name: source_identity(root, lock, args.evidence_dir)
            for name, root, lock in [('herdr-bot', args.repo, 'backend/Cargo.lock'), ('toolkit', args.toolkit, 'bun.lock')]}
        require(all(evidence['sourcesAfter'][name]['fileSha256'] == evidence['nativeSources'][name]['fileSha256']
                    for name in ['herdr-bot', 'toolkit']), 'Source changed during native acceptance; evidence is not final')
        # New acceptance entry/module files are included even before first commit.
        evidence['passed'] = all(case['verified'] for host in evidence['coverage'].values() for case in host.values()) \
            and all(check['exit'] == 0 for check in evidence['checks']) and len(evidence['checks']) == len(commands) \
            and len(evidence['nativeExits']) == 8 and all(code == 0 for code in evidence['nativeExits'].values()) \
            and not evidence['cleanupErrors']
    except BaseException:
        evidence['error'] = traceback.format_exc()
    evidence['entrypoint'] = {'path': str(Path(__file__).resolve()),
        'sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    report = args.evidence_dir / 'evidence.json'
    report.write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps({'passed': evidence['passed'], 'evidence': str(report)}, indent=2))
    return 0 if evidence['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
