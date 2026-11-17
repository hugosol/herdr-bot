"""P20/P30 S2: actual installed OMP/Pi × auto/manual, no scripted model choices.

Run from the explicit Herdr worktree. Missing authorization, partial coverage,
failed repository checks or cleanup errors are nonzero and remain unverified.
No credential discovery, installation, controlled provider or decision retries.
"""
import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import traceback

REPO = Path(__file__).resolve().parents[1]
EXPECTATIONS = {
    'P20/order': 'The real model independently produces the literal quote, approves, then reports the normal result.',
    'P20/progress-before-approval': 'A real missing-discount report is saved while the current execution is unapproved.',
    'P20/approval-source': 'Manual is actual native Yes; auto has source auto and no user confirmation.',
    'P30/end-node-finish': 'After ordinary work completes, native manager dispatches the end Chat and explicitly finishes without its approval/report/business completion.',
}


def absolute(value):
    path = Path(value)
    if not path.is_absolute():
        raise argparse.ArgumentTypeError('Expected an explicit absolute path')
    return path.resolve()


def checks(args, evidence):
    """Required checks on these sources; retain each original result separately."""
    target = args.backend.parent.parent
    environment = dict(os.environ, CARGO_TARGET_DIR=str(target), HERDR_BOT_REPO=str(args.repo),
        HERDR_BOT_METADATA=str(args.repo / 'meta-data/task-options.toml'),
        HERDR_BOT_BINARY=str(args.backend.parent / 'examples/safe_shutdown_shell.exe'),
        WORKFLOW_PROMPT_EVIDENCE_DIR=str(args.evidence_dir / 'mechanical-native'))
    commands = [
        (args.repo, ['npm', '--prefix', 'frontend', 'run', 'build']),
        (args.repo, ['cargo', 'build', '--manifest-path', 'backend/Cargo.toml', '--bin', 'herdr-bot', '--bin', 'herdr-fixture', '--bin', 'herdr-paths', '--example', 'safe_shutdown_shell']),
        (args.repo, ['node', 'scripts/test-backend.mjs']),
        (args.repo, ['npm', '--prefix', 'frontend', 'test']),
        (args.repo, ['npm', '--prefix', 'frontend', 'run', 'test:browser:build']),
        (args.toolkit, [args.bun, 'run', 'typecheck']),
        (args.toolkit, [args.bun, 'run', 'test']),
        (args.toolkit, [args.bun, 'run', 'check:integrations']),
        (args.toolkit, [args.bun, 'test', 'tests/harness-dock/native-node-report.test.ts', 'tests/harness-dock/node-report-contract.test.ts']),
    ]
    for cwd, argv in commands:
        argv = [shutil.which(argv[0]) or argv[0], *argv[1:]]
        log = args.evidence_dir / ('check-' + str(len(evidence['commands'])) + '.log')
        record = {'argv': argv, 'cwd': str(cwd), 'log': str(log)}
        if cwd == args.toolkit and 'test' in argv and not Path(environment['HERDR_BOT_BINARY']).is_file():
            record.update(exit=None, error='Missing this worktree safe shutdown shell; native checks not started, no production fallback.')
            evidence['commands'].append(record)
            continue
        try:
            with log.open('w', encoding='utf-8') as stream:
                result = subprocess.run(argv, cwd=cwd, env=environment, stdout=stream, stderr=subprocess.STDOUT)
            record['exit'] = result.returncode
        except OSError:
            record.update(exit=None, error=traceback.format_exc())
        evidence['commands'].append(record)
    return all(record.get('exit') == 0 for record in evidence['commands'])


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
    evidence = {'passed': False, 'coverage': {host: {mode: {key: {'verified': False, 'expected': value}
        for key, value in EXPECTATIONS.items()} for mode in ['auto', 'manual']} for host in ['omp', 'pi']},
        'selectedHosts': args.hosts.split(','), 'commands': [], 'cleanupErrors': [],
        'modelPolicy': 'Explicitly authorized actual model; record requested/returned identity; no product-specific model restriction, fallback or retry.'}
    try:
        from acceptance.host.processes import require
        require(args.model_config.is_file(), 'Missing explicit authorized model configuration; no credential discovery')
        config = json.loads(args.model_config.read_text(encoding='utf-8'))
        for key in ['provider', 'model', 'baseUrl', 'apiKeyEnv']:
            require(isinstance(config.get(key), str) and config[key], 'Authorized model configuration requires ' + key)
        require(os.environ.get(config['apiKeyEnv']), 'Missing explicitly authorized credential environment: ' + config['apiKeyEnv'])
        evidence['authorizedModel'] = {key: config[key] for key in ['provider', 'model', 'apiKeyEnv']}
        require(args.backend.is_file(), 'Missing current worktree backend: ' + str(args.backend))
        args.repo, args.toolkit = args.herdr_bot_worktree, args.toolkit_worktree
        from acceptance.host.provenance import preflight
        preflight(args, evidence)
        args.path_roots = evidence['paths']['roots']
        for host in ['omp', 'pi']:
            name = host + '_state_extension'
            require(name in args.path_roots and Path(args.path_roots[name]).is_file(), 'Missing isolated readonly roots.' + name)
            setattr(args, name, Path(args.path_roots[name]).resolve())
        from node_approval_handoff_acceptance import NodeApprovalHandoff
        checks_passed = checks(args, evidence)
        for mode in ['auto', 'manual']:
            for host in args.hosts.split(','):
                selected = copy.copy(args)
                selected.host, selected.mode = host, mode
                selected.evidence_dir = args.evidence_dir / mode
                run = NodeApprovalHandoff(selected)
                status = run.run()
                for key, result in run.evidence['coverage'].items():
                    evidence['coverage'][host][mode][key].update(result, evidence=str(run.root / 'evidence.json'))
                evidence['cleanupErrors'].extend(run.evidence['cleanupErrors'])
                if status:
                    evidence.setdefault('failedCells', []).append(host + '/' + mode)
        evidence['passed'] = checks_passed and not evidence['cleanupErrors'] and all(
            item['verified'] for host in evidence['coverage'].values() for mode in host.values() for item in mode.values())
    except BaseException:
        evidence['error'] = traceback.format_exc()
    evidence['entrypoint'] = {'path': str(Path(__file__).resolve()), 'sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    report = args.evidence_dir / 'evidence.json'
    report.write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps({'passed': evidence['passed'], 'evidence': str(report)}, indent=2))
    return 0 if evidence['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
