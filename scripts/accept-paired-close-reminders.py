"""Real Herdr completion and native reminder acceptance for two explicit worktrees."""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import traceback

from acceptance.host.processes import require
from acceptance.host.provenance import digest, source_identity
from acceptance.paired_close_reminders import Scenario, save
from developer_paths import REPO

CLOSE = [('tui', 'pi', 'extra-tab'), ('app', 'pi', 'extra-tab'),
         ('tui', 'pi', 'last-tab'), ('app', 'pi', 'last-tab'), ('tui', 'pi', 'group-refused')]
REMINDERS = [(host, agent, mode) for host in ('tui', 'app') for agent in ('omp', 'pi')
             for mode in ('custom', 'default', 'disabled')]
DOCKS = ['tests/harness-dock/session-ready.test.ts', 'tests/harness-dock/binding-lifecycle.test.ts',
         'tests/harness-dock/native-node-report.test.ts', 'tests/harness-dock/node-report-contract.test.ts']


def absolute(value):
    path = Path(value)
    if not path.is_absolute():
        raise ValueError(f'expected an explicit absolute path: {value}')
    return path.resolve()


def select(value, matrix):
    names = {'-'.join(case): case for case in matrix}
    if value == 'all':
        return matrix
    if value == 'none':
        return []
    selected = value.split(',')
    require(len(selected) == len(set(selected)) and all(key in names for key in selected), ('unknown or repeated cases', value, sorted(names)))
    return [case for name, case in names.items() if name in selected]


def parse():
    parser = argparse.ArgumentParser(description=__doc__)
    for option in ('herdr-bot-worktree', 'toolkit-worktree', 'backend', 'isolated-config', 'evidence-dir'):
        parser.add_argument('--' + option, required=True)
    parser.add_argument('--close-cases', required=True, help='all, none, or comma-separated host-agent-case names')
    parser.add_argument('--reminder-cases', required=True, help='all, none, or comma-separated host-agent-mode names')
    parser.add_argument('--native-only', action='store_true', help='diagnostic only; cannot certify full automatic scope')
    parser.add_argument('--timeout', type=int, default=45, help='native observation deadline in seconds')
    return parser.parse_args()


def configure(args, evidence):
    require(sys.platform == 'win32' and args.timeout >= 15, 'Windows and at least 15s native observation budget required')
    args.repo, args.toolkit = absolute(args.herdr_bot_worktree), absolute(args.toolkit_worktree)
    require(args.repo == REPO and Path.cwd().resolve() == REPO, 'run from the explicit herdr-bot worktree root')
    require((args.repo / '.git').is_file() and (args.toolkit / '.git').is_file(), 'two explicit Git worktrees required')
    args.backend = absolute(args.backend)
    args.target = absolute(os.environ.get('CARGO_TARGET_DIR', ''))
    require(args.backend == args.target / 'debug/herdr-bot.exe' and args.target not in (args.repo, args.toolkit),
        'backend must be <exclusive absolute CARGO_TARGET_DIR>/debug/herdr-bot.exe')
    args.safe = args.backend.parent / 'examples/safe_shutdown_shell.exe'
    args.recorder = args.backend.parent / 'examples/paired_native_receipt.exe'
    args.paths = absolute(args.isolated_config)
    config = json.loads(args.paths.read_text(encoding='utf-8'))
    for name in ('herdr', 'app', 'core', 'node', 'bun', 'omp_package', 'pi_package', 'omp_state_extension', 'pi_state_extension', 'toolkit_baseline_root'):
        path = absolute(config[name])
        require(path.is_dir() if name.endswith(('_package', '_root')) else path.is_file(), ('isolated dependency missing', name, path))
        setattr(args, name, path)
    args.sound = {}
    if config.get('sound'):
        for key in ('ffmpeg', 'done', 'request', 'tui_done', 'tui_request', 'app_done', 'app_request'):
            args.sound[key] = absolute(config['sound'][key])
            require(args.sound[key].is_file(), ('isolated sound dependency missing', key, args.sound[key]))
        require(digest(args.sound['done']) != digest(args.sound['request']), 'custom sounds must be distinct')
    evidence['isolatedConfig'] = {'path': str(args.paths), 'sha256': digest(args.paths)}
    evidence['targetDir'] = str(args.target)


def capture_command(argv, cwd, env, log):
    argv = [str(x) for x in argv]
    with log.open('wb') as output:
        try:
            exit_code = subprocess.run(argv, cwd=cwd, env=env, stdout=output, stderr=subprocess.STDOUT).returncode
        except OSError:
            output.write(traceback.format_exc().encode('utf-8'))
            exit_code = None
    return {'argv': argv, 'cwd': str(cwd), 'exit': exit_code, 'log': str(log)}


def repository_checks(args, evidence):
    owner = args.target / '.paired-close-reminders-owner'
    args.target.mkdir(parents=True, exist_ok=True)
    require(not any(args.target.iterdir()), 'use a new exclusive CARGO_TARGET_DIR for a full acceptance run')
    with owner.open('x', encoding='utf-8') as stream:
        stream.write(str(args.repo))
    env = {key: value for key, value in os.environ.items() if not key.upper().startswith(('HERDR', 'HC_', 'PI_', 'OMP_'))
        and not any(term in key.upper() for term in ('API_KEY', 'TOKEN', 'PASSWORD', 'CREDENTIAL', 'SECRET'))}
    env.update(CARGO_TARGET_DIR=str(args.target), HERDR_BOT_REPO=str(args.repo),
        HERDR_BOT_BINARY=str(args.safe), HERDR_BOT_METADATA=str(args.repo / 'meta-data/task-options.toml'),
        OMP_BASELINE_RECORD_ROOT=str(args.toolkit_baseline_root), PI_NODE=str(args.node),
        PI_CLI=str(args.pi_package / 'dist/bundle/cli.js'), OMP_CLI=str(args.omp_package / 'dist/cli.js'))
    npm, cargo = [shutil.which(name) or name for name in ('npm', 'cargo')]
    steps = [
        ('frontend-install', args.repo / 'frontend', [args.bun, 'install', '--frozen-lockfile']),
        ('toolkit-install', args.toolkit, [args.bun, 'install', '--frozen-lockfile']),
        ('frontend-build', args.repo, [npm, '--prefix', 'frontend', 'run', 'build']),
        ('cargo-build', args.repo, [cargo, 'build', '--locked', '--manifest-path', 'backend/Cargo.toml',
            '--bin', 'herdr-bot', '--bin', 'herdr-paths', '--bin', 'herdr-fixture',
            '--example', 'safe_shutdown_shell', '--example', 'paired_native_receipt']),
        ('backend', args.repo, [args.node, 'scripts/test-backend.mjs']),
        ('frontend', args.repo, [npm, '--prefix', 'frontend', 'test']),
        ('browser', args.repo, [npm, '--prefix', 'frontend', 'run', 'test:browser:build']),
        ('toolkit-typecheck', args.toolkit, [args.bun, 'run', 'typecheck']),
        ('toolkit-framework', args.toolkit, [args.bun, 'run', 'test:framework']),
        ('toolkit-integrations', args.toolkit, [args.bun, 'run', 'check:integrations']),
        ('toolkit-full', args.toolkit, [args.bun, 'run', 'test']),
        ('toolkit-native-docks', args.toolkit, [args.bun, 'test', *DOCKS]),
    ]
    for name, cwd, command in steps:
        log = args.evidence_dir / f'{name}.log'
        if name in ('toolkit-full', 'toolkit-native-docks') and not args.safe.is_file():
            log.write_text('Current safe shutdown shell absent; no production fallback.\n')
            record = {'argv': list(map(str, command)), 'cwd': str(cwd), 'exit': None, 'log': str(log)}
        else:
            record = capture_command(command, cwd, env, log)
        evidence['repositoryChecks'][name] = record
        print(f'{name}: {record["exit"]}', flush=True)
        if name == 'backend':
            match = re.search(r'^Summary: (.+summary\.json)\s*$', log.read_text(encoding='utf-8', errors='replace'), re.M)
            if match and Path(match[1]).is_file():
                shutil.copytree(Path(match[1]).parent, args.evidence_dir / 'backend-original-logs')
        if name == 'browser':
            results = args.repo / 'frontend/test-results'
            if results.is_dir():
                shutil.copytree(results, args.evidence_dir / 'browser-original-results')


def artifacts(args, evidence):
    for name, artifact in [('backend', args.backend), ('safeShutdownShell', args.safe), ('receiptRecorder', args.recorder)]:
        depfile = artifact.with_suffix('.d')
        deps = depfile.read_text(encoding='utf-8').replace('\\', '/')
        source = args.repo / ('backend/src/main.rs' if name == 'backend' else 'backend/examples/' + artifact.stem + '.rs')
        # Cargo emits absolute dependencies for the production backend, but
        # can use manifest-relative dependencies for standalone examples.
        dep_names = deps.split(': ', 1)[-1].split()
        matches = [name for name in dep_names if name == source.as_posix() or
            (name == source.relative_to(args.repo / 'backend').as_posix() and name.startswith('examples/'))]
        require(matches and source.stat().st_mtime <= artifact.stat().st_mtime,
            ('artifact not built from this worktree', artifact, source, deps))
        evidence['artifacts'][name] = {'path': str(artifact), 'sha256': digest(artifact), 'depfile': deps}
    static = args.repo / 'backend/static'
    require((static / 'index.html').is_file(), 'build frontend in the specified worktree')
    evidence['artifacts']['frontend'] = {p.relative_to(static).as_posix(): digest(p) for p in static.rglob('*') if p.is_file()}
    for name in ('herdr', 'app', 'core', 'node', 'bun'):
        path = getattr(args, name)
        result = subprocess.run([str(path), '--version'], cwd=args.repo, capture_output=True, text=True, check=True)
        evidence['installations'][name] = {'path': str(path), 'sha256': digest(path), 'version': result.stdout.strip()}
    for name, cli in [('omp', 'dist/cli.js'), ('pi', 'dist/bundle/cli.js')]:
        package = getattr(args, name + '_package')
        launcher = Path(shutil.which(name) or '')
        metadata = json.loads((package / 'package.json').read_text(encoding='utf-8'))
        require(launcher.is_file() and (launcher.parent / 'node_modules' / metadata['name']).resolve() == package,
            ('product PATH launcher does not identify supplied installed package', name, launcher, package))
        evidence['installations'][name] = {'entry': str(package / cli), 'entrySha256': digest(package / cli),
            'launcher': str(launcher), 'launcherSha256': digest(launcher), 'version': metadata['version']}


def main():
    args = parse()
    try:
        args.evidence_dir = absolute(args.evidence_dir)
        args.evidence_dir.mkdir(parents=True, exist_ok=False)
    except (OSError, ValueError) as error:
        print(error, file=sys.stderr)
        return 1
    matrix = CLOSE + REMINDERS
    coverage = {'-'.join(case): {'passed': False, 'status': 'not run'} for case in matrix}
    evidence = {'automatedPassed': False, 'passed': False, 'P48Listening': 'pending user confirmation', 'cases': coverage,
        'repositoryChecks': {}, 'artifacts': {}, 'installations': {}}
    path = args.evidence_dir / 'summary.json'
    try:
        configure(args, evidence)
        selected = select(args.close_cases, CLOSE) + select(args.reminder_cases, REMINDERS)
        require(selected, 'select at least one case')
        evidence['sources'] = {name: source_identity(root, lock, args.evidence_dir) for name, root, lock in
            [('herdr-bot', args.repo, 'backend/Cargo.lock'), ('toolkit', args.toolkit, 'bun.lock')]}
        if not args.native_only:
            repository_checks(args, evidence)
        artifacts(args, evidence)
        for case in selected:
            key = '-'.join(case)
            result = Scenario(args, *case, args.evidence_dir / key).run()
            coverage[key] = {'passed': result['passed'], 'archive': result['archive'], 'nativeRoot': result['nativeRoot'],
                'error': result.get('verificationError'), 'cleanupErrors': result['cleanupErrors']}
            print(f'{key}: {"passed" if result["passed"] else "failed"}', flush=True)
            save(path, evidence)
            if result['cleanupErrors']:
                break
        evidence['automatedPassed'] = (not args.native_only and all(result['passed'] for result in coverage.values())
            and all(record['exit'] == 0 for record in evidence['repositoryChecks'].values()))
    except Exception:
        evidence['error'] = traceback.format_exc()
        print(evidence['error'], file=sys.stderr)
    finally:
        save(path, evidence)
        print(f'Automated scope: {evidence["automatedPassed"]}; P48 actual listening remains pending; evidence: {path}', flush=True)
    return 0 if evidence['automatedPassed'] else 1


if __name__ == '__main__':
    sys.exit(main())
