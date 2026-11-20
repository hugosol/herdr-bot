"""Run installed OMP/Pi × Herdr TUI/App Task launch acceptance from the bot worktree."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time
import traceback

from acceptance.host.processes import require
from acceptance.host.provenance import digest, source_identity

REPO = Path(__file__).resolve().parents[1]
MATRIX = {f'{agent}-{host}': (agent, host) for agent in ('omp', 'pi') for host in ('tui', 'app')}
ADDITIONAL = {
    'legacy-tui': 'P2', 'backend-switch': 'P4', 'alternate-tui': 'P8', 'alternate-app': 'P8', 'foreign-core': 'P5',
    'tui-modal-release': 'P18/P20/P24/P25/P50', 'app-modal-release': 'P18/P20/P24/P25/P50',
    'tui-modal-twice': 'P24/P26/P50', 'app-modal-twice': 'P24/P26/P50',
    'tui-modal-exit': 'P28', 'app-modal-replace': 'P19/P28', 'app-ambiguous-start': 'P27/P50',
}
DOCKS = ('session-ready', 'binding-lifecycle', 'connect-bot-startup', 'connect-bot-lifecycle')


def dump(path, value):
    temporary = path.with_suffix('.tmp')
    temporary.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    temporary.replace(path)


def absolute(value):
    path = Path(value)
    if not path.is_absolute():
        raise argparse.ArgumentTypeError('expected explicit absolute paths')
    return path.resolve()


def selected(value, available):
    if value == 'all':
        return tuple(available)
    fields = value.split(',')
    if len(set(fields)) != len(fields) or not set(fields) <= available.keys():
        raise argparse.ArgumentTypeError('choose from: ' + ','.join(available))
    return tuple(fields)


def parse():
    parser = argparse.ArgumentParser(description=__doc__)
    for option in ('herdr-bot-worktree', 'toolkit-worktree', 'backend', 'isolated-config', 'evidence-dir'):
        parser.add_argument('--' + option, type=absolute, required=True)
    parser.add_argument('--combinations', required=True, type=lambda value: selected(value, MATRIX))
    parser.add_argument('--cases', default=tuple(ADDITIONAL), type=lambda value: selected(value, ADDITIONAL))
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument('--native-only', action='store_true', help='use the already-built backend; this run cannot certify repository checks')
    modes.add_argument('--checks-only', action='store_true', help='build and run repository checks but do not certify native cells')
    args = parser.parse_args()
    if args.evidence_dir.exists():
        parser.error('evidence directory must not exist')
    return args


def run(argv, cwd, env, log=None, timeout=30):
    argv = list(map(str, argv))
    # npm is an installed Windows .cmd shim, resolved explicitly, not shell source.
    command = [os.environ['COMSPEC'], '/d', '/c', subprocess.list2cmdline(argv)] if argv[0].endswith('.cmd') else argv
    record = {'argv': argv, 'cwd': str(cwd), 'exit': None}
    if log is None:
        result = subprocess.run(command, cwd=cwd, env=env, capture_output=True, encoding='utf-8', errors='replace', timeout=timeout)
        return dict(record, exit=result.returncode, stdout=result.stdout, stderr=result.stderr)
    record['log'] = str(log)
    with log.open('wb') as output:
        try:
            record['exit'] = subprocess.run(command, cwd=cwd, env=env, stdout=output, stderr=subprocess.STDOUT, timeout=timeout).returncode
        except (OSError, subprocess.TimeoutExpired):
            record['error'] = traceback.format_exc()
            output.write(record['error'].encode())
    return record


def configure(args, report):
    require(sys.platform == 'win32', 'Installed Herdr pairing requires Windows')
    require(args.herdr_bot_worktree == Path.cwd().resolve() == REPO, 'run from the supplied bot worktree root')
    require(all((path / '.git').is_file() for path in (REPO, args.toolkit_worktree)) and REPO != args.toolkit_worktree,
            'two explicit Git worktrees required')
    args.target = absolute(os.getenv('CARGO_TARGET_DIR', ''))
    require(args.backend == args.target / 'debug/herdr-bot.exe', 'backend must be <exclusive CARGO_TARGET_DIR>/debug/herdr-bot.exe')
    config = json.loads(args.isolated_config.read_text(encoding='utf-8'))
    require(config['authorization'] == 'owned-local-mechanics-only', 'explicit isolated native run authorization required')
    require(type(config['versions']) is dict, 'expected installed versions must be declared')
    args.roots = {}
    for name in ('herdr', 'app', 'core', 'node', 'bun', 'chrome', 'omp_cli', 'pi_cli', 'omp_package', 'pi_package',
                 'toolkit_baseline', 'omp_state_extension', 'pi_state_extension'):
        path = absolute(config[name])
        require(path.is_dir() if name.endswith(('_package', '_baseline')) else path.is_file(), ('missing explicit path', name, path))
        args.roots[name] = path
    for name in ('herdr', 'app', 'core'):
        setattr(args, name, args.roots[name])
    args.recorder = args.backend.parent / 'examples/recovery_herdr_proxy.exe'
    args.safe = args.backend.parent / 'examples/safe_shutdown_shell.exe'
    args.entries = {agent: args.roots[f'{agent}_package'] / entry for agent, entry in (
        ('omp', 'dist/cli.js'), ('pi', 'dist/bundle/cli.js'))}
    for agent, entry in args.entries.items():
        launcher = args.roots[f'{agent}_cli']
        package = args.roots[f'{agent}_package']
        metadata = json.loads((package / 'package.json').read_text(encoding='utf-8'))
        require(entry.is_file() and launcher.suffix.casefold() == '.cmd' and launcher.stem.casefold() == agent and
                (launcher.parent / 'node_modules' / metadata['name']).resolve() == package and
                metadata['name'] in launcher.read_text(encoding='utf-8').replace('\\', '/'),
                ('declared launcher does not load the declared installed package', launcher, entry))
        require(launcher.parent == args.roots['omp_cli'].parent, 'agent shims must resolve from the same explicit PATH directory')
    for name in ('npm', 'cargo', 'git'):
        resolved = shutil.which(f'{name}.cmd' if name == 'npm' else f'{name}.exe')
        require(resolved, f'missing {name} from PATH')
        setattr(args, name, Path(resolved))
    env = {key: value for key, value in os.environ.items() if not key.upper().startswith(('HERDR', 'HC_', 'PI_', 'OMP_'))
           and not any(term in key.upper() for term in ('API_KEY', 'TOKEN', 'PASSWORD', 'CREDENTIAL', 'SECRET'))}
    env.update(CARGO_TARGET_DIR=str(args.target), HERDR_BOT_REPO=str(REPO), HERDR_BOT_BINARY=str(args.safe),
               HERDR_BOT_METADATA=str(REPO / 'meta-data/task-options.toml'), OMP_BASELINE_RECORD_ROOT=str(args.roots['toolkit_baseline']),
               OMP_CLI=str(args.roots['omp_cli']), PI_CLI=str(args.entries['pi']), PI_NODE=str(args.roots['node']),
               HARNESS_ACCEPTANCE_EVIDENCE_DIR=str(args.evidence_dir / 'toolkit-harness-evidence'))
    args.env = env
    report['configuration'] = {'path': str(args.isolated_config), 'sha256': digest(args.isolated_config),
                               'target': str(args.target), 'authorization': config['authorization']}
    report['installed'] = {}
    version_commands = {name: [args.roots[name]] for name in ('herdr', 'app', 'core', 'node', 'bun')}
    version_commands.update({agent: [args.roots[f'{agent}_cli']] for agent in args.entries})
    for name, prefix in version_commands.items():
        result = run([*prefix, '--version'], REPO, env)
        version = result.get('stdout', '').strip().split('/')[-1].split()[-1:]
        report['installed'][name] = dict(result, path=str(prefix[0]), sha256=digest(prefix[0]))
        require(result['exit'] == 0 and version == [config['versions'][name]], ('installed version mismatch', name, result))
    require(config['versions']['bun'] == '1.4.4', 'both lockfiles require Bun 1.4.4')
    report['installedFiles'] = {str(path): digest(path) for path in (
        *args.entries.values(), args.roots['chrome'], args.roots['omp_state_extension'], args.roots['pi_state_extension'])}
    report['sources'] = {
        'bot': source_identity(REPO, 'backend/Cargo.lock', args.evidence_dir),
        'toolkit': source_identity(args.toolkit_worktree, 'bun.lock', args.evidence_dir),
    }


def artifacts(args, report):
    report['artifacts'] = {}
    for executable, source in ((args.backend, 'backend/src/main.rs'), (args.safe, 'backend/examples/safe_shutdown_shell.rs'),
                               (args.recorder, 'backend/examples/recovery_herdr_proxy.rs')):
        depfile = executable.with_suffix('.d').read_text(encoding='utf-8').replace('\\', '/')
        absolute_source = (REPO / source).as_posix().casefold()
        relative_example = source.removeprefix('backend/') if source.startswith('backend/examples/') else None
        require(absolute_source in depfile.casefold() or
                (relative_example and relative_example + ':' in depfile), ('non-worktree artifact dependency', executable, depfile))
        require((REPO / source).stat().st_mtime <= executable.stat().st_mtime, ('artifact predates source', executable))
        report['artifacts'][str(executable)] = {'sha256': digest(executable), 'depfile': depfile}
    sources = [p for p in (REPO / 'backend/src').rglob('*.rs')] + [REPO / 'backend/Cargo.lock', REPO / 'backend/Cargo.toml']
    require(all(p.stat().st_mtime <= args.backend.stat().st_mtime for p in sources), 'backend predates Rust source or lock')
    frontend = REPO / 'backend/static/index.html'
    require(all(p.stat().st_mtime <= frontend.stat().st_mtime for p in (REPO / 'frontend/src').rglob('*') if p.is_file()),
            'packaged page predates frontend source')
    report['assets'] = {str(p.relative_to(REPO)): digest(p) for p in frontend.parent.rglob('*') if p.is_file()}


def checks(args, report, save):
    args.target.mkdir(parents=True, exist_ok=True)
    require(not any(args.target.iterdir()), 'full checks require a new exclusive CARGO_TARGET_DIR')
    (args.target / '.paired-task-launch-owner').write_text(str(REPO), encoding='utf-8')
    bun, npm, node, cargo, toolkit = args.roots['bun'], args.npm, args.roots['node'], args.cargo, args.toolkit_worktree
    steps = [
        ('frontend-install', REPO / 'frontend', [bun, 'install', '--frozen-lockfile', '--linker', 'hoisted', '--force']),
        ('toolkit-install', toolkit, [bun, 'install', '--frozen-lockfile', '--linker', 'hoisted', '--force']),
        ('frontend-build', REPO, [npm, '--prefix', 'frontend', 'run', 'build']),
        ('backend-build', REPO, [cargo, 'build', '--locked', '--manifest-path', 'backend/Cargo.toml', '--bins',
                                '--example', 'safe_shutdown_shell', '--example', 'recovery_herdr_proxy']),
        ('backend', REPO, [node, 'scripts/test-backend.mjs', '--binary-jobs', '2', '--output-dir', args.evidence_dir / 'backend-targets']),
        ('frontend', REPO, [npm, '--prefix', 'frontend', 'test']),
        ('browser-build', REPO, [npm, '--prefix', 'frontend', 'run', 'test:browser:build', '--', '--workers=1']),
        ('controlled-input-faults', REPO, [cargo, 'test', '--locked', '--manifest-path', 'backend/Cargo.toml', '--test', 'chat_creation', 'initial_input_retry', '--', '--nocapture']),
        ('controlled-timeout', REPO, [cargo, 'test', '--locked', '--manifest-path', 'backend/Cargo.toml', '--test', 'chat_creation', 'a_timed_out_submission', '--', '--nocapture']),
        *[(f'toolkit-{name.replace(":", "-")}', toolkit, [bun, 'run', name]) for name in ('typecheck', 'test:framework', 'check:integrations', 'test')],
        ('toolkit-native-docks', toolkit, [bun, 'test', *[f'tests/harness-dock/{name}.test.ts' for name in DOCKS]]),
    ]
    for label, cwd, argv in steps:
        log = args.evidence_dir / f'{label}.log'
        if label in ('toolkit-test', 'toolkit-native-docks') and not args.safe.is_file():
            log.write_text('Current safe shutdown shell missing; no production fallback.\n')
            result = {'argv': list(map(str, argv)), 'cwd': str(cwd), 'exit': None, 'log': str(log)}
        else:
            # Repository suites own their internal native-process deadlines. Do not abandon their process trees.
            result = run(argv, cwd, args.env, log, timeout=None)
        report['checks'][label] = result
        print(f'{label}: {result["exit"]}', flush=True)
        save()


def main():
    args = parse()
    args.evidence_dir.mkdir(parents=True, exist_ok=False)
    report = {'passed': False, 'checks': {}, 'provider': 'mechanical loopback text only, not real model decisions',
              'combinations': {key: {'status': 'unverified', 'selected': key in args.combinations} for key in MATRIX},
              'additional': {key: {'status': 'unverified', 'selected': key in args.cases, 'promises': promise}
                             for key, promise in ADDITIONAL.items()}}
    def save():
        dump(args.evidence_dir / 'summary.json', report)
    try:
        configure(args, report)
        if not args.native_only:
            checks(args, report, save)
        artifacts(args, report)
        if not args.checks_only:
            from acceptance.paired_task_launch import Launch
            for group, cells in (('combinations', args.combinations), ('additional', args.cases)):
                for label in cells:
                    cell = report[group][label]
                    destination = args.evidence_dir / label
                    try:
                        case = Launch(args, label, MATRIX.get(label))
                        outcome = case.run(destination)
                        cell.update(status='passed' if outcome['passed'] else 'failed', evidence=str(destination / 'evidence.json'),
                                    error=outcome.get('verificationError'), cleanupErrors=outcome['cleanupErrors'])
                    except Exception:
                        cell.update(status='failed', error=traceback.format_exc())
                    print(f'{label}: {cell["status"]}', flush=True)
                    save()
        report['passed'] = (not args.native_only and not args.checks_only and
                            all(result['exit'] == 0 for result in report['checks'].values()) and
                            all(cell['status'] == 'passed' for group in ('combinations', 'additional') for cell in report[group].values()))
    except Exception:
        report['preflightError'] = traceback.format_exc()
    finally:
        report['finishedAtUnix'] = time.time()
        save()
        print(args.evidence_dir / 'summary.json', flush=True)
    return 0 if report['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
