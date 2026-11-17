"""P31/P34/P35 published V1 acceptance: actual models, native hosts and page.

No installation, credential discovery, controlled success, business retry or
historical side effects. Partial/filtered coverage and cleanup errors are nonzero.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import traceback

REPO = Path(__file__).resolve().parents[1]
# Independent responsibility oracle from the published P34 decision table, not
# calculated from the implementation's auto_approved values.
RESPONSIBILITIES = {
    'create-bot-wrapper.json': {'create_blank': True, 'write_when': False, 'write_plan_guide': False,
        'write_node_guide': False, 'optimize_when': True, 'optimize_plan_guide': True,
        'optimize_node_guide': True, 'save_bot': True},
    'create-bot-task-workflow--bc-61ce48b80a4c.json': {'w1': False, 'w2': True, 'wrap_bot': True, 'optimize_bot': False},
    'enhance-existing-bot--bc-301b25d9f8d6.json': {'w1': True, 'w2': False, 'w3': False, 'w4': False, 'w5': True},
    'plan_workflow.json': {'outline': False, 'generate_map': True, 'optimize_map': False},
    'tibo-monitor-then-c-drive-inspection.json': {'tibo-monitor': True, 'c-drive-inspection': True},
}


def absolute(value):
    path = Path(value)
    if not path.is_absolute():
        raise argparse.ArgumentTypeError('Expected an explicit absolute path')
    return path.resolve()


def coverage():
    items = {}
    for host in ['omp', 'pi']:
        items['P31/S2/' + host] = {'verified': False, 'expected': 'Actual model produces fresh credential-free V1 request; real Bun consumes directly bound baseline and three texts.'}
        items['P35/S2/' + host] = {'verified': False, 'expected': 'Independent V1 Run creates and saves all fields, exact complete public readback/revision/files; native manager explicitly accepts and completes.'}
    for filename, nodes in RESPONSIBILITIES.items():
        for node, automatic in nodes.items():
            for seam in ['S2', 'S3']:
                items[f'P34/{seam}/{filename}/{node}'] = {'verified': False,
                    'expected': {'autoApproved': automatic, 'approvalSource': 'auto' if automatic else 'manual',
                        'observation': 'Actual native execution' if seam == 'S2' else 'Delivered page, complete public definition and frozen execution agree'}}
    return items


def checks(args, record):
    environment = dict(os.environ, CARGO_TARGET_DIR=str(args.backend.parent.parent),
        HERDR_BOT_REPO=str(args.repo), HERDR_BOT_METADATA=str(args.repo / 'meta-data/task-options.toml'),
        HERDR_BOT_BINARY=str(args.backend.parent / 'examples/safe_shutdown_shell.exe'),
        WORKFLOW_PROMPT_EVIDENCE_DIR=str(args.evidence_dir / 'mechanical-docks'))
    record['checkEnvironment'] = {key: environment.get(key) for key in ['CARGO_TARGET_DIR', 'HERDR_BOT_REPO',
        'HERDR_BOT_METADATA', 'HERDR_BOT_BINARY', 'OMP_BASELINE_RECORD_ROOT']}
    commands = [(args.repo, ['npm', '--prefix', 'frontend', 'run', 'build']),
        (args.repo, ['cargo', 'build', '--manifest-path', 'backend/Cargo.toml', '--bin', 'herdr-bot', '--bin', 'herdr-fixture', '--bin', 'herdr-paths', '--example', 'safe_shutdown_shell']),
        (args.repo, ['node', 'scripts/test-backend.mjs']),
        (args.repo, ['npm', '--prefix', 'frontend', 'test']),
        (args.repo, ['npm', '--prefix', 'frontend', 'run', 'test:browser:build']),
        (args.toolkit, [args.bun, 'run', 'typecheck']), (args.toolkit, [args.bun, 'run', 'test']),
        (args.toolkit, [args.bun, 'run', 'check:integrations']),
        (args.toolkit, [args.bun, 'test', 'tests/harness-dock/native-node-report.test.ts', 'tests/harness-dock/node-report-contract.test.ts', 'tests/harness-dock/nested-role-completion.test.ts'])]
    for cwd, argv in commands:
        argv = [shutil.which(argv[0]) or argv[0], *argv[1:]]
        log = args.evidence_dir / f'check-{len(record["commands"])}.log'
        item = {'argv': argv, 'cwd': str(cwd), 'log': str(log)}
        try:
            if cwd == args.toolkit and 'test' in argv and not Path(environment['HERDR_BOT_BINARY']).is_file():
                raise RuntimeError('Missing this worktree safe shutdown shell; no production fallback')
            with log.open('w', encoding='utf-8') as stream:
                item['exit'] = subprocess.run(argv, cwd=cwd, env=environment, stdout=stream, stderr=subprocess.STDOUT).returncode
        except Exception:
            item.update(exit=None, error=traceback.format_exc())
        record['commands'].append(item)
    return all(item.get('exit') == 0 for item in record['commands'])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['herdr-bot-worktree', 'toolkit-worktree', 'backend', 'paths-file', 'model-config', 'evidence-dir']:
        parser.add_argument('--' + name, required=True, type=absolute)
    parser.add_argument('--hosts', required=True, choices=['omp,pi', 'omp', 'pi'])
    parser.add_argument('--timeout', type=int, choices=range(1, 901), default=300)
    parser.add_argument('--node', default='node')
    parser.add_argument('--bun', default='bun')
    parser.add_argument('--shell-path', type=absolute)
    parser.add_argument('--native-only', action='store_true', help='Collect native/page evidence only; never certifies the complete ticket')
    args = parser.parse_args()
    args.evidence_dir.mkdir(parents=True, exist_ok=False)
    record = {'passed': False, 'coverage': coverage(), 'commands': [], 'runs': [], 'cleanupErrors': [],
        'selectedHosts': args.hosts.split(','), 'controlled': False}
    try:
        from acceptance.host.processes import require
        require(args.model_config.is_file(), 'Missing explicit authorized model configuration; no credential discovery')
        model = json.loads(args.model_config.read_text(encoding='utf-8'))
        for key in ['provider', 'model', 'baseUrl', 'apiKeyEnv']:
            require(isinstance(model.get(key), str) and model[key], 'Authorized model configuration requires ' + key)
        require(os.environ.get(model['apiKeyEnv']), 'Missing explicitly authorized credential environment: ' + model['apiKeyEnv'])
        args.repo, args.toolkit = args.herdr_bot_worktree, args.toolkit_worktree
        require(args.repo == REPO and Path.cwd().resolve() == REPO, 'Run from the explicitly supplied worktree')
        require(args.backend.name == 'herdr-bot.exe' and args.backend.parent.name == 'debug', 'Use this worktree production backend in an independent absolute Cargo target/debug directory')
        require(args.paths_file.is_file(), 'Missing explicit isolated paths configuration')
        record['authorizedModel'] = {key: model[key] for key in ['provider', 'model', 'apiKeyEnv']}
        record['nativeOnly'] = args.native_only
        checks_ok = checks(args, record) if not args.native_only else False
        from acceptance.host.provenance import preflight
        preflight(args, record)
        from node_approval_v1_acceptance import run_scenarios
        run_scenarios(args, record, RESPONSIBILITIES)
        record['passed'] = checks_ok and not record['cleanupErrors'] and all(item['exit'] == 0 for item in record['runs']) and all(item['verified'] for item in record['coverage'].values())
    except BaseException:
        record['error'] = traceback.format_exc()
    record['entrypoint'] = {'path': str(Path(__file__).resolve()), 'sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    report = args.evidence_dir / 'evidence.json'
    report.write_text(json.dumps(record, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps({'passed': record['passed'], 'evidence': str(report)}, indent=2))
    return 0 if record['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
