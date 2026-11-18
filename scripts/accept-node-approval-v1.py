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

from node_approval_v1_matrix import (CASE_MAPS, HOSTS, WORKER_REPORT, cell_keys, cell_passed,
    cell_root, concurrency, dispatch_cells, run_cell_process, scoped_models, selected_cells,
    source_fingerprint, write_json)

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
    for host in HOSTS:
        items['P31/S2/' + host] = {'verified': False, 'expected': 'Actual model produces fresh credential-free V1 request; real Bun consumes directly bound baseline and three texts.'}
        items['P35/S2/' + host] = {'verified': False, 'expected': 'Independent V1 Run creates and saves all fields, exact complete public readback/revision/files; native manager explicitly accepts and completes.'}
        for filename, nodes in RESPONSIBILITIES.items():
            for node, automatic in nodes.items():
                for seam in ['S2', 'S3']:
                    items[f'P34/{seam}/{host}/{filename}/{node}'] = {'verified': False,
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


def worker(args):
    from node_approval_v1_acceptance import run_scenarios

    host, case = args.hosts, args.cases
    keys = cell_keys(host, case, RESPONSIBILITIES)
    record = {'managerHost': host, 'case': case, 'cellPassed': False, 'runs': [], 'cleanupErrors': [],
              'sharedFailures': [], 'cleanupRisk': False,
              'coverage': {key: item for key, item in coverage().items() if key in keys}}
    try:
        # Parent preflight owns validation; workers use only the resolved dependency paths.
        preflight = json.loads((args.matrix_evidence_dir / 'matrix-preflight.json').read_text(encoding='utf-8'))
        record['paths'] = preflight['paths']
        record['host'] = preflight['hostVersions']
        roots = record['paths']['roots']
        args.herdr = Path(roots['herdr']) / 'herdr.exe'
        args.pi_package = Path(roots['pi_package'])
        args.omp_package = Path(roots['omp_package'])
        run_scenarios(args, record, RESPONSIBILITIES)
        record['cellPassed'] = cell_passed(record, keys)
    except BaseException:
        record['cleanupRisk'] = True
        record['error'] = traceback.format_exc()
    write_json(args.evidence_dir / WORKER_REPORT, record)
    return 0 if record['cellPassed'] else 1


def run_matrix(args, record, per_cell_models, source_fingerprints):
    # The only bootstrap data workers need from once-per-invocation preflight.
    write_json(args.evidence_dir / 'matrix-preflight.json', {'paths': record['paths'], 'hostVersions': record['host']})

    def run_cell(host, case):
        directory = cell_root(args.evidence_dir, host, case)
        directory.mkdir(parents=True, exist_ok=False)
        config = directory / 'model.json'
        write_json(config, per_cell_models[host, case])
        flags = {'herdr-bot-worktree': args.repo, 'toolkit-worktree': args.toolkit, 'backend': args.backend,
                 'paths-file': args.paths_file, 'model-config': config, 'evidence-dir': directory / 'native',
                 'hosts': host, 'cases': case, 'timeout': args.timeout, 'node': args.node, 'bun': args.bun,
                 '_matrix-worker-root': args.evidence_dir}
        if args.shell_path is not None:
            flags['shell-path'] = args.shell_path
        argv = [sys.executable, '-B', str(Path(__file__).resolve()),
                *[part for key, value in flags.items() for part in (f'--{key}', str(value))]]
        if args.native_only:
            argv.append('--native-only')
        env = dict(os.environ, GIT_OPTIONAL_LOCKS='0')
        result = run_cell_process(host, case, directory, argv, cell_keys(host, case, RESPONSIBILITIES),
                                  cwd=args.repo, env=env, source_fingerprints=source_fingerprints)
        try:
            config.unlink()
        except OSError as error:
            result.setdefault('cleanupErrors', []).append(f'Cell model configuration cleanup: {error}')
            result.update(state='failed', haltReason='worker-cleanup-risk')
            result.pop('coverage', None)
        return result

    record['cells'] = dispatch_cells(per_cell_models, record['matrix']['effectiveJobs'], run_cell)
    for result in record['cells']:
        record['runs'].extend(result.get('runs', []))
        record['cleanupErrors'].extend(result.get('cleanupErrors', []))
        record['sharedFailures'].extend(result.get('sharedFailures', []))
        if result.get('nativeErrors'):
            record.setdefault('nativeErrors', []).extend(result['nativeErrors'])
        if result.get('haltReason'):
            record.setdefault('matrixHalts', []).append({'host': result['host'], 'case': result['case'],
                                                        'reason': result['haltReason']})
        for key, item in result.pop('coverage', {}).items():
            record['coverage'][key].update(item)


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
    parser.add_argument('--cases', choices=('all', *CASE_MAPS), default='all',
        help='One independent diagnostic case; partial selection never certifies the complete ticket')
    parser.add_argument('--jobs', type=int, choices=(1, 2), default=1,
        help='Maximum parallel cells; an authorized model capacity must explicitly allow concurrency')
    parser.add_argument('--_matrix-worker-root', dest='matrix_evidence_dir', type=absolute, help=argparse.SUPPRESS)
    args = parser.parse_args()
    args.repo, args.toolkit = args.herdr_bot_worktree, args.toolkit_worktree
    if args.matrix_evidence_dir is not None:
        if args.hosts not in HOSTS or args.cases not in CASE_MAPS:
            parser.error('Matrix worker requires exactly one host and one named case')
        directory = cell_root(args.matrix_evidence_dir, args.hosts, args.cases)
        if args.evidence_dir != directory / 'native' or args.model_config != directory / 'model.json':
            parser.error('Matrix worker requires its own cell paths')
        if args.repo != REPO or Path.cwd().resolve() != REPO:
            parser.error('Run from the explicitly supplied worktree')
        args.evidence_dir.mkdir(exist_ok=False)
        return worker(args)

    args.evidence_dir.mkdir(parents=True, exist_ok=False)
    cells = selected_cells(args.hosts, args.cases)
    full_matrix = selected_cells('omp,pi', 'all')
    record = {'passed': False, 'coverage': coverage(), 'commands': [], 'runs': [], 'cleanupErrors': [],
        'selectedHosts': args.hosts.split(','), 'controlled': False, 'sharedFailures': [],
        'caseSelection': args.cases, 'filtered': cells != full_matrix, 'nativeOnly': args.native_only,
        'matrix': concurrency({}, args.jobs, len(cells)),
        'cells': [{'host': host, 'case': case, 'state': 'not-run'} for host, case in cells]}
    try:
        from acceptance.host.processes import require
        require(args.model_config.is_file(), 'Missing explicit authorized model configuration; no credential discovery')
        model = json.loads(args.model_config.read_text(encoding='utf-8'))
        require(isinstance(model, dict), 'Authorized model configuration must be an object')
        for key in ['provider', 'model', 'apiKeyEnv']:
            require(isinstance(model.get(key), str) and model[key].strip(), 'Authorized model configuration requires ' + key)
        record['matrix'] = concurrency(model, args.jobs, len(cells))
        per_cell_models = scoped_models(model, cells)
        require(os.environ.get(model['apiKeyEnv']), 'Missing explicitly authorized credential environment: ' + model['apiKeyEnv'])
        require(args.repo == REPO and Path.cwd().resolve() == REPO, 'Run from the explicitly supplied worktree')
        require(args.backend.name == 'herdr-bot.exe' and args.backend.parent.name == 'debug', 'Use this worktree production backend in an independent absolute Cargo target/debug directory')
        require(args.paths_file.is_file(), 'Missing explicit isolated paths configuration')
        record['authorizedModel'] = {key: model[key] for key in ['provider', 'model', 'apiKeyEnv']}
        record['checksPassed'] = not args.native_only and checks(args, record)
        require(args.native_only or record['checksPassed'], 'Worktree-wide checks failed; native cells were not launched')
        from acceptance.host.provenance import preflight, source_identity
        preflight(args, record)
        sources = {'herdr-bot': (args.repo, 'backend/Cargo.lock'), 'toolkit': (args.toolkit, 'bun.lock')}
        record['sourceBeforeMatrix'] = {name: source_identity(root, lock, args.evidence_dir)
                                        for name, (root, lock) in sources.items()}
        fingerprints = {name: source_fingerprint(identity) for name, identity in record['sourceBeforeMatrix'].items()}
        record['matrix']['sourceFingerprints'] = fingerprints
        run_matrix(args, record, per_cell_models, fingerprints)
        record['sourceAfterMatrix'] = {name: source_identity(root, lock, args.evidence_dir)
                                       for name, (root, lock) in sources.items()}
        require(all(source_fingerprint(record['sourceAfterMatrix'][name]) == fingerprints[name]
                    for name in sources), 'Source changed during matrix acceptance')
        successes = [(item['host'], item['case']) for item in record['cells'] if item['state'] == 'passed']
        record['passed'] = (record['checksPassed'] and not record['filtered'] and successes == full_matrix
            and not record['cleanupErrors'] and not record['sharedFailures'] and not record.get('matrixHalts')
            and all(item['verified'] is True for item in record['coverage'].values()))
    except BaseException:
        record['error'] = traceback.format_exc()
    record['entrypoint'] = {'path': str(Path(__file__).resolve()), 'sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    report = args.evidence_dir / 'evidence.json'
    report.write_text(json.dumps(record, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps({'passed': record['passed'], 'evidence': str(report)}, indent=2))
    return 0 if record['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
