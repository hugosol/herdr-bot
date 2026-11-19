"""S1 + S3: reopen the exact installed OMP/Pi Chat with one Herdr host at a time."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tomllib
import traceback

from acceptance.host.processes import require
from acceptance.host.provenance import digest, source_identity

REPO = Path(__file__).resolve().parents[1]
AGENTS = ('omp', 'pi')
DIRECTIONS = ('tui-to-app', 'app-to-tui')
SCENARIOS = {
    'cross-end': 'P8/P10/P12/P13/P29/P31/P32/P41',
    'same-end': 'P10/P12/P13/P41',
    'same-value': 'P10/P12/P13/P41',
    'git-unique': 'P14',
    'git-ambiguous': 'P14/P15',
    'cwd-missing': 'P15',
    'no-source': 'P33/P35',
    'deleted-source': 'P33/P35',
    'env-unreadable': 'P33/P35',
    'suite-invalid': 'P33/P35',
    'file-conflict': 'P30',
    'task-agent-conflict': 'P30',
}
CONTROLLED = ('chat_resume', 'chat_resume_environment', 'chat_resume_omp', 'chat_resume_pi', 'chat_resume_timeout')
DOCKS = ('session-ready', 'session-recovery', 'connect-bot-startup', 'connect-bot-lifecycle')


def dump(file, value):
    file.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')


def selection(value, allowed):
    if value == 'all':
        return tuple(allowed)
    words = value.split(',')
    if len(set(words)) != len(words) or not set(words) <= set(allowed):
        raise argparse.ArgumentTypeError('expected a comma-separated selection from ' + ','.join(allowed))
    return tuple(words)


def parse():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('herdr-bot-worktree', 'toolkit-worktree', 'backend', 'paths-file', 'evidence-dir'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--directions', required=True, type=lambda value: selection(value, DIRECTIONS))
    parser.add_argument('--agents', default=AGENTS, type=lambda value: selection(value, AGENTS))
    parser.add_argument('--scenarios', default=tuple(SCENARIOS), type=lambda value: selection(value, SCENARIOS))
    parser.add_argument('--native-only', action='store_true', help='skip repository checks; summary remains incomplete')
    parser.add_argument('--controlled-only', action='store_true', help='execute named controlled Resume targets only; summary remains incomplete')
    args = parser.parse_args()
    if args.native_only and args.controlled_only:
        parser.error('--native-only and --controlled-only are mutually exclusive')
    for name in ('herdr_bot_worktree', 'toolkit_worktree', 'backend', 'paths_file', 'evidence_dir'):
        path = getattr(args, name)
        if not path.is_absolute():
            parser.error(f'{name.replace("_", "-")} requires an explicit absolute path')
        setattr(args, name, path.resolve())
    if args.evidence_dir.exists():
        parser.error('evidence directory must not exist')
    return args


def preflight(args, report):
    require(sys.platform == 'win32', 'Installed paired Herdr check requires Windows')
    require(Path.cwd().resolve() == args.herdr_bot_worktree == REPO, 'run from the supplied herdr-bot worktree root')
    require((args.toolkit_worktree / '.git').is_file(), 'an explicit Toolkit git worktree is required')
    require(args.backend.is_file() and args.backend.name == 'herdr-bot.exe' and args.backend.parent.name == 'debug',
            'supply this worktree’s debug/herdr-bot.exe')
    target = os.getenv('CARGO_TARGET_DIR')
    require(target and Path(target).is_absolute() and Path(target).resolve() == args.backend.parent.parent,
            'CARGO_TARGET_DIR must be absolute, exclusive to this worktree and match the backend')
    depfile = args.backend.with_suffix('.d').read_text(encoding='utf-8').replace('\\', '/').casefold()
    require((REPO / 'backend/src/main.rs').as_posix().casefold() in depfile, 'backend depfile is not from the supplied worktree')
    for file in list((REPO / 'backend/src').rglob('*.rs')) + [REPO / 'backend/Cargo.toml', REPO / 'backend/Cargo.lock']:
        require(file.stat().st_mtime <= args.backend.stat().st_mtime, ('backend predates source', str(file)))
    raw = tomllib.loads(args.paths_file.read_text(encoding='utf-8'))['roots']
    keys = ('herdr', 'herdr_app', 'herdr_core', 'omp_package', 'pi_package', 'omp_state_extension',
            'pi_state_extension', 'node', 'bun', 'omp_cli', 'toolkit_baseline')
    args.roots = {}
    for key in keys:
        path = Path(raw[key])
        require(path.is_absolute() and path.exists(), ('missing explicit installation/config path', key, str(path)))
        args.roots[key] = path.resolve()
    args.herdr = args.roots['herdr'] / 'herdr.exe'
    args.app = args.roots['herdr_app'] / 'herdr-app.exe'
    args.core = args.roots['herdr_core'] / 'hc.exe'
    args.safe = args.backend.parent / 'examples/safe_shutdown_shell.exe'
    args.recorder = args.backend.parent / 'examples/recovery_herdr_proxy.exe'
    args.entries = {'omp': args.roots['omp_package'] / 'dist/cli.js',
                    'pi': args.roots['pi_package'] / 'dist/bundle/cli.js'}
    require((REPO / 'frontend/node_modules').is_dir() and (args.toolkit_worktree / 'node_modules/yaml').is_dir(),
            'restore both lockfile-pinned dependency sets before acceptance')
    for exe in (args.herdr, args.app, args.core, args.roots['node'], args.roots['bun']):
        result = subprocess.run([str(exe), '--version'], capture_output=True, text=True,
                                encoding='utf-8', errors='replace', timeout=20)
        report.setdefault('executables', {})[exe.name] = {'path': str(exe), 'sha256': digest(exe),
            'stdout': result.stdout, 'stderr': result.stderr, 'exit': result.returncode}
        require(result.returncode == 0, report['executables'][exe.name])
    require(report['executables'][args.roots['bun'].name]['stdout'].strip() == '1.4.4', 'both lockfiles require Bun 1.4.4')
    for agent, entry in args.entries.items():
        package = args.roots[f'{agent}_package'] / 'package.json'
        report.setdefault('agents', {})[agent] = {'package': json.loads(package.read_text(encoding='utf-8'))['version'],
            'entry': str(entry), 'entrySha256': digest(entry), 'stateExtension': str(args.roots[f'{agent}_state_extension']),
            'stateExtensionSha256': digest(args.roots[f'{agent}_state_extension'])}
    omp_native_entry = args.roots['omp_package'] / 'src/cli.ts'
    if omp_native_entry.is_file():
        report['agents']['omp']['appResolvedEntry'] = {'path': str(omp_native_entry), 'sha256': digest(omp_native_entry)}
    files = [args.backend, args.safe, args.recorder, REPO / 'backend/static/index.html', REPO / 'frontend/bun.lock',
             REPO / 'backend/Cargo.lock', args.roots['omp_cli'], args.paths_file, Path(__file__),
             REPO / 'scripts/acceptance/paired_history.py']
    report['artifacts'] = {str(path): digest(path) for path in files}
    report['sources'] = {key: source_identity(root, lock, args.evidence_dir) for key, root, lock in (
        ('herdr-bot', REPO, 'backend/Cargo.lock'), ('toolkit', args.toolkit_worktree, 'bun.lock'))}


def checks(args, report):
    env = dict(os.environ, HERDR_BOT_BINARY=str(args.safe), HERDR_BOT_REPO=str(REPO),
               HERDR_BOT_METADATA=str(REPO / 'meta-data/task-options.toml'),
               OMP_BASELINE_RECORD_ROOT=str(args.roots['toolkit_baseline']), OMP_CLI=str(args.roots['omp_cli']),
               PI_CLI=str(args.entries['pi']), PI_NODE=str(args.roots['node']))
    commands = [
        ('backend', REPO, ['node', 'scripts/test-backend.mjs', '--output-dir', args.evidence_dir / 'backend-targets']),
        ('frontend', REPO, ['npm', '--prefix', 'frontend', 'test']),
        ('browser-build', REPO, ['npm', '--prefix', 'frontend', 'run', 'test:browser:build']),
        *[(name.replace(':', '-'), args.toolkit_worktree, [args.roots['bun'], 'run', name])
          for name in ('typecheck', 'test:framework', 'check:integrations', 'test')],
        ('native-docks', args.toolkit_worktree, [args.roots['bun'], 'test',
            *[f'tests/harness-dock/{name}.test.ts' for name in DOCKS]]),
    ]
    controlled = [('controlled-placement-environment-conflicts', REPO,
        ['cargo', 'test', '--manifest-path', 'backend/Cargo.toml', '--locked',
         *[arg for name in CONTROLLED for arg in ('--test', name)], '--', '--nocapture'])]
    queued = controlled if args.controlled_only else commands + controlled
    for label, cwd, command in queued:
        argv = [str(shutil.which(str(command[0])) or command[0]), *map(str, command[1:])]
        log = args.evidence_dir / f'{label}.log'
        entry = {'label': label, 'argv': argv, 'cwd': str(cwd), 'log': str(log), 'exit': None}
        report['checks'].append(entry)
        try:
            with log.open('w', encoding='utf-8') as stream:
                entry['exit'] = subprocess.run(argv, cwd=cwd, env=env, stdout=stream, stderr=subprocess.STDOUT).returncode
        except Exception:
            entry['error'] = traceback.format_exc()
        dump(args.evidence_dir / 'evidence.json', report)


def main():
    args = parse()
    args.evidence_dir.mkdir(parents=True, exist_ok=False)
    report = {'passed': False, 'checks': [], 'cells': [
        dict(agent=agent, direction=direction, scenario=scenario, promises=promises, status='unverified')
        for scenario, promises in SCENARIOS.items() for direction in DIRECTIONS for agent in AGENTS
        if not (scenario.startswith('git-') and direction == 'tui-to-app')]}
    try:
        preflight(args, report)
        if not args.controlled_only:
            from acceptance.paired_history import Recovery
            cleanup_safe = True
            for cell in report['cells']:
                if not cleanup_safe or not (cell['agent'] in args.agents and cell['direction'] in args.directions
                                             and cell['scenario'] in args.scenarios):
                    continue
                path = args.evidence_dir / f"{cell['direction']}-{cell['agent']}-{cell['scenario']}"
                runner = Recovery(args, cell)
                outcome = runner.run(path)
                receipt = outcome.get('resume', {}).get('body', {})
                cell.update(status='passed' if outcome['passed'] else outcome['status'],
                            publicOutcome=receipt.get('outcome'), publicStage=receipt.get('stage'),
                            evidence=str(path / 'evidence.json'))
                cleanup_safe = not outcome['cleanupErrors']
                dump(args.evidence_dir / 'evidence.json', report)
        if not args.native_only:
            checks(args, report)
        report['passed'] = not (args.native_only or args.controlled_only) and all(
            cell['status'] == 'passed' for cell in report['cells']) and all(item['exit'] == 0 for item in report['checks'])
    except Exception:
        report['preflightError' if 'sources' not in report else 'runnerError'] = traceback.format_exc()
    finally:
        dump(args.evidence_dir / 'evidence.json', report)
    print(str(args.evidence_dir / 'evidence.json'))
    return 0 if report['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
