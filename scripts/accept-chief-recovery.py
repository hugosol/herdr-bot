"""Dual-host manual report recovery acceptance at approved S2/S3 seams.

Explicit absolute inputs, fresh evidence, current-source artifacts, GPT-6-luna
only. Mechanical enabling cases are labelled, never substituted for model proof.
"""
import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import runpy
import subprocess
import sys
import traceback

REPO = Path(__file__).resolve().parents[1]
PROMISES = {
    'P23': 'No automatic resend after failure/create/resume/register/backend restart/time',
    'P24': 'Explicit sender request and fresh native approve/deny preserve original ID/content',
    'P25': 'Durable receipt deduplicates concurrent/lost-response/replacement/restart resends',
    'P26': 'Terminate native process after reliable receive before presentation; no replay',
    'P27': 'Fresh chief explicitly reads and presents original without new business',
    'P34': 'New-process Task sender and direct manager recovery retain original association',
    'P35': 'Changed content cannot replace original; new report requires new confirmation/ID',
}


def absolute(value):
    path = Path(value)
    if not path.is_absolute():
        raise argparse.ArgumentTypeError('Expected explicit absolute path')
    return path.resolve()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['herdr-bot-worktree', 'toolkit-worktree', 'backend', 'paths-file', 'model-config', 'evidence-dir']:
        parser.add_argument('--' + name, type=absolute, required=True)
    parser.add_argument('--hosts', required=True, choices=['omp,pi', 'omp', 'pi'])
    parser.add_argument('--timeout', type=int, choices=range(1, 601), default=180)
    parser.add_argument('--node', default='node')
    parser.add_argument('--bun', default='bun')
    parser.add_argument('--shell-path', type=absolute)
    args = parser.parse_args()
    args.evidence_dir.mkdir(parents=True, exist_ok=False)
    evidence = {'passed': False, 'coverage': {host: {key: {'verified': False, 'expected': value}
        for key, value in PROMISES.items()} for host in ['omp', 'pi']}, 'commands': [], 'cleanupErrors': [],
        'selectedHosts': args.hosts.split(','), 'modelPolicy': 'GPT-6-luna; actual return identity; no retry/fallback'}
    try:
        def require(condition, message):
            if not condition:
                raise AssertionError(message)
        require(args.backend.is_file(), 'Missing current worktree backend: ' + str(args.backend))
        require(args.herdr_bot_worktree == REPO == Path.cwd().resolve(), 'Run from supplied herdr-bot worktree')
        require((args.toolkit_worktree / '.git').is_file(), 'Expected explicit toolkit Git worktree')
        config = json.loads(args.model_config.read_text(encoding='utf-8'))
        require(config.get('model') == 'gpt-6-luna', 'Actual model must be gpt-6-luna; no fallback')
        require(config.get('apiKeyEnv') and os.environ.get(config['apiKeyEnv']), 'Missing explicitly authorized credential environment')
        args.repo, args.toolkit = args.herdr_bot_worktree, args.toolkit_worktree
        from acceptance.host.provenance import preflight
        preflight(args, evidence)
        args.path_roots = evidence['paths']['roots']
        # The inherited isolated deployment prepares both installed host entries,
        # even when this invocation exercises one host for parallel aggregation.
        for host in ['omp', 'pi']:
            name = host + '_state_extension'
            require(name in args.path_roots and Path(args.path_roots[name]).is_file(), 'Missing readonly roots.' + name)
            setattr(args, name, Path(args.path_roots[name]).resolve())
        shell = args.backend.parent / 'examples/safe_shutdown_shell.exe'
        require(shell.is_file(), 'Missing current worktree safe shutdown shell')
        environment = dict(os.environ, HERDR_BOT_REPO=str(REPO), HERDR_BOT_BINARY=str(shell),
            HERDR_BOT_METADATA=str(REPO / 'meta-data/task-options.toml'), CARGO_TARGET_DIR=str(args.backend.parent.parent),
            WORKFLOW_PROMPT_EVIDENCE_DIR=str(args.evidence_dir / 'mechanical'))
        # Dependency-owned protocol tests are actually executed with these exact
        # sources; they do not own or establish the recovery scenario matrix.
        command = [args.bun, 'test', 'tests/harness-dock/native-chief-resend.test.ts',
            'tests/harness-dock/native-chief-reception.test.ts', 'tests/harness-dock/native-chief-receipt-backend.test.ts',
            'tests/harness-dock/chief-read-report.test.ts']
        if args.hosts != 'omp,pi':
            command.extend(['--test-name-pattern', '^' + args.hosts])
        log = args.evidence_dir / 'mechanical.log'
        with log.open('w', encoding='utf-8') as output:
            result = subprocess.run(command, cwd=args.toolkit, env=environment, stdout=output, stderr=subprocess.STDOUT)
        evidence['commands'].append({'argv': command, 'cwd': str(args.toolkit), 'exit': result.returncode, 'log': str(log),
            'scope': 'Controlled provider/HTTP/Herdr faults; not real-model recovery proof'})
        native = runpy.run_path(str(REPO / 'scripts/chief_recovery_native_acceptance.py'))['ChiefRecoveryAcceptance']
        for host in args.hosts.split(','):
            selected = copy.copy(args)
            selected.host = host
            run = native(selected)
            status = run.run()
            run.phase('host-result', 'passed' if status == 0 else 'failed')
            for key, observed in run.evidence['coverage'].items():
                evidence['coverage'][host][key].update(observed, evidence=str(run.root / 'evidence.json'))
            evidence['cleanupErrors'].extend(run.evidence['cleanupErrors'])
            if status:
                evidence.setdefault('hostFailures', []).append(host)
        evidence['passed'] = result.returncode == 0 and not evidence.get('hostFailures') and not evidence['cleanupErrors'] and all(
            item['verified'] for host in evidence['coverage'].values() for item in host.values())
    except BaseException:
        evidence['error'] = traceback.format_exc()
    evidence['entrypoint'] = {'path': str(Path(__file__).resolve()), 'sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    report = args.evidence_dir / 'evidence.json'
    report.write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps({'passed': evidence['passed'], 'evidence': str(report)}, indent=2))
    return 0 if evidence['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
