"""Published V2 only: P34/S2+S3 and P35/S2, actual OMP and Pi.

Explicit authorization and isolated installations are required. No installation,
credential discovery, controlled success, V1 substitution or model retry.
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
    'P34/V2/S2': 'Published create_task, align_requirements and design_and_create are manual native Yes; the finished child manager independently approves the automatic create_bot_v2 parent.',
    'P34/V2/S3': 'The delivered browser shows each published V2 responsibility and the actual Run approval/report facts, cross-checked with independent public HTTP reads.',
    'P35/V2/S2': 'Both installed hosts consume unchanged V2 business protocols and real named files; native managers explicitly complete/finish separate parent and child Runs, with V1 definitions and identities untouched.',
}


def absolute(value):
    path = Path(value)
    if not path.is_absolute():
        raise argparse.ArgumentTypeError('Expected an explicit absolute path')
    return path.resolve()


def checks(args, evidence):
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
        (args.toolkit, [args.bun, 'test', 'tests/harness-dock/native-node-report.test.ts',
            'tests/harness-dock/node-report-contract.test.ts', 'tests/harness-dock/nested-role-completion.test.ts']),
    ]
    for cwd, argv in commands:
        argv = [shutil.which(argv[0]) or argv[0], *argv[1:]]
        log = args.evidence_dir / ('check-' + str(len(evidence['commands'])) + '.log')
        record = {'argv': argv, 'cwd': str(cwd), 'log': str(log)}
        try:
            if cwd == args.toolkit and 'test' in argv and not Path(environment['HERDR_BOT_BINARY']).is_file():
                raise RuntimeError('Missing this worktree safe shutdown shell; no production backend fallback')
            with log.open('w', encoding='utf-8') as stream:
                result = subprocess.run(argv, cwd=cwd, env=environment, stdout=stream, stderr=subprocess.STDOUT)
            record['exit'] = result.returncode
        except (OSError, RuntimeError):
            record.update(exit=None, error=traceback.format_exc())
        evidence['commands'].append(record)
        (args.evidence_dir / 'checks.json').write_text(json.dumps(evidence['commands'], indent=2), encoding='utf-8')
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
    parser.add_argument('--native-only', action='store_true', help='Run isolated native/page coverage only; never certifies all required repository checks')
    args = parser.parse_args()
    args.evidence_dir.mkdir(parents=True, exist_ok=False)
    evidence = {'passed': False, 'nativeOnly': args.native_only, 'nativeCoveragePassed': False, 'selectedHosts': args.hosts.split(','), 'commands': [], 'cleanupErrors': [],
        'coverage': {host: {key: {'verified': False, 'expected': value} for key, value in EXPECTATIONS.items()}
            for host in ['omp', 'pi']},
        'modelPolicy': 'Actual explicitly authorized model only; no credential discovery, fallback or business retries.'}
    try:
        from acceptance.host.processes import require
        require(args.model_config.is_file(), 'Missing explicit authorized model configuration; no credential discovery')
        config = json.loads(args.model_config.read_text(encoding='utf-8'))
        for key in ['provider', 'model', 'baseUrl', 'apiKeyEnv']:
            require(isinstance(config.get(key), str) and config[key], 'Authorized model configuration requires ' + key)
        require(os.environ.get(config['apiKeyEnv']), 'Missing explicitly authorized credential environment: ' + config['apiKeyEnv'])
        evidence['authorizedModel'] = {key: config[key] for key in ['provider', 'model', 'apiKeyEnv']}
        evidence['scheduling'] = {'hosts': 'sequential', 'declaredModelCapacity': config.get('maxConcurrentRequests'),
            'reason': 'Each actual nested V2 Run retains concurrent native managers; do not queue two independent businesses behind a single-request authorized transport.'}
        args.repo, args.toolkit = args.herdr_bot_worktree, args.toolkit_worktree
        require(args.repo == REPO and Path.cwd().resolve() == REPO, 'Run from the explicitly supplied herdr-bot worktree')
        require((args.toolkit / '.git').is_file(), 'Use the explicitly assigned toolkit worktree')
        require(args.backend.name == 'herdr-bot.exe' and args.backend.parent.name == 'debug', 'Use this worktree independent Cargo target/debug/herdr-bot.exe')
        require(args.backend.parent.parent != args.repo / 'backend/target', 'An independent absolute CARGO_TARGET_DIR is required')
        require(subprocess.check_output([args.bun, '--version'], text=True).strip() == '1.4.4', 'Bun 1.4.4 is required')
        passed_checks = False if args.native_only else checks(args, evidence)
        from acceptance.host.provenance import preflight, source_identity
        preflight(args, evidence)
        args.path_roots = evidence['paths']['roots']
        for name in ['omp_state_extension', 'pi_state_extension', 'wayfinder', 'wayfinder_skills', 'pi_extensions', 'codebase_memory_mcp']:
            require(name in args.path_roots and Path(args.path_roots[name]).exists(), 'Missing explicitly declared isolated dependency roots.' + name)
            setattr(args, name, Path(args.path_roots[name]).resolve())
        require('chrome' in args.path_roots and (Path(args.path_roots['chrome']) / 'chrome.exe').is_file(), 'Declare the installed Chrome directory in roots.chrome')
        args.chrome = Path(args.path_roots['chrome']) / 'chrome.exe'
        if args.shell_path is None:
            require('native_shell' in args.path_roots, 'Declare roots.native_shell or --shell-path; do not inherit a WSL shell for Windows files and loopback')
            args.shell_path = Path(args.path_roots['native_shell']).resolve()
        require(args.shell_path.is_file(), 'Missing explicitly selected native shell')
        evidence['nativeShell'] = {'path': str(args.shell_path), 'sha256': hashlib.sha256(args.shell_path.read_bytes()).hexdigest()}
        for skill in ['grill-with-docs', 'grilling', 'domain-modeling', 'writing-for-agents']:
            path = args.wayfinder / 'dependent-skills/codebase-exploration' / skill / 'SKILL.md'
            require(path.is_file(), ('Missing original published Suite skill', str(path)))
        before = {name: source_identity(root, lock, args.evidence_dir)['fileSha256'] for name, root, lock in
            [('herdr-bot', args.repo, 'backend/Cargo.lock'), ('toolkit', args.toolkit, 'bun.lock')]}
        from node_approval_v2_acceptance import PublishedV2Acceptance
        for host in args.hosts.split(','):
            selected = copy.copy(args)
            selected.host = host
            run = PublishedV2Acceptance(selected)
            status = run.run()
            for key, result in run.evidence['coverage'].items():
                evidence['coverage'][host][key].update(result, evidence=str(run.root / 'evidence.json'))
            evidence['cleanupErrors'].extend(run.evidence['cleanupErrors'])
            if status:
                evidence.setdefault('failedHosts', []).append(host)
        after = {name: source_identity(root, lock, args.evidence_dir)['fileSha256'] for name, root, lock in
            [('herdr-bot', args.repo, 'backend/Cargo.lock'), ('toolkit', args.toolkit, 'bun.lock')]}
        evidence['sourceStability'] = {'before': before, 'after': after, 'unchanged': before == after}
        evidence['nativeCoveragePassed'] = before == after and not evidence['cleanupErrors'] and not evidence.get('failedHosts') and all(
            item['verified'] for host in evidence['coverage'].values() for item in host.values())
        evidence['passed'] = passed_checks and evidence['nativeCoveragePassed']
    except BaseException:
        evidence['error'] = traceback.format_exc()
    evidence['entrypoint'] = {'path': str(Path(__file__).resolve()), 'sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    report = args.evidence_dir / 'evidence.json'
    report.write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps({'passed': evidence['passed'], 'coverage': evidence['coverage'], 'evidence': str(report)}, ensure_ascii=False, indent=2))
    return 0 if evidence['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
