"""Repeatable owned Herdr TUI/App × OMP/Pi chief and Run business acceptance."""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import traceback
from urllib.request import ProxyHandler, build_opener, install_opener

from acceptance.host.processes import require
from acceptance.host.provenance import digest, source_identity
from acceptance.paired_chief_run import ChiefRun, save
from developer_paths import REPO


MATRIX = {f'{host}-{agent}': (host, agent) for host in ('tui', 'app') for agent in ('omp', 'pi')}
DOCKS = ('connect-bot-startup', 'connect-bot-lifecycle', 'session-ready', 'chief-delegation',
         'chief-report-backend-faults', 'native-chief-reception',
         'native-chief-report', 'native-chief-receipt-backend', 'native-node-report',
         'node-report-contract', 'nested-role-completion', 'role-dual-load', 'role-identity-integration')
DEPENDENCIES = ('herdr', 'app', 'core', 'node', 'bun', 'omp_package', 'pi_package',
                'omp_state_extension', 'pi_state_extension', 'toolkit_baseline_root')


def absolute(value):
    path = Path(value)
    require(path.is_absolute(), ('explicit absolute path required', value))
    return path.resolve()


def parse():
    parser = argparse.ArgumentParser(description=__doc__)
    for option in ('herdr-bot-worktree', 'toolkit-worktree', 'backend', 'isolated-config', 'evidence-dir'):
        parser.add_argument('--' + option, required=True, type=str)
    parser.add_argument('--scenarios', required=True, help='all or comma-separated ' + ','.join(MATRIX))
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--checks-only', action='store_true', help='repository checks only; never passes complete scope')
    mode.add_argument('--native-only', action='store_true', help='reuse built artifacts; never passes complete scope')
    parser.add_argument('--timeout', type=int, default=45, help='per-native-observation deadline (at least 15 seconds)')
    return parser.parse_args()


def configure(args, evidence):
    require(sys.platform == 'win32' and args.timeout >= 15, 'Windows and at least 15 seconds required')
    args.repo, args.toolkit = absolute(args.herdr_bot_worktree), absolute(args.toolkit_worktree)
    require(args.repo == REPO == Path.cwd().resolve() and args.repo != args.toolkit, 'run at the specified herdr-bot worktree root')
    require((args.repo / '.git').is_file() and (args.toolkit / '.git').is_file(), 'two linked Git worktrees required')
    args.target = absolute(os.environ.get('CARGO_TARGET_DIR', ''))
    args.backend = absolute(args.backend)
    require(args.backend == args.target / 'debug/herdr-bot.exe' and not any(args.target.is_relative_to(root)
            for root in (args.repo, args.toolkit)), 'use an exclusive absolute build directory outside both worktrees')
    args.safe = args.target / 'debug/examples/safe_shutdown_shell.exe'
    args.recorder = args.target / 'debug/examples/paired_native_receipt.exe'
    config = absolute(args.isolated_config)
    values = json.loads(config.read_text(encoding='utf-8'))
    require(set(values) == set(DEPENDENCIES), ('expected only installation paths, never credentials', sorted(DEPENDENCIES)))
    for name in DEPENDENCIES:
        path = absolute(values[name])
        require(path.is_dir() if name.endswith(('_package', '_root')) else path.is_file(), (name, path))
        setattr(args, name, path)
    evidence.update(isolatedConfig={'path': str(config), 'sha256': digest(config)}, targetDir=str(args.target))
    selected = list(MATRIX) if args.scenarios == 'all' else args.scenarios.split(',')
    require(len(set(selected)) == len(selected) and all(key in MATRIX for key in selected), ('invalid scenarios', selected))
    return selected


def capture(argv, cwd, env, log):
    argv = list(map(str, argv))
    print(log.stem, flush=True)
    with log.open('xb') as stream:
        try:
            code = subprocess.run(argv, cwd=cwd, env=env, stdout=stream, stderr=subprocess.STDOUT).returncode
        except OSError:
            stream.write(traceback.format_exc().encode('utf-8'))
            code = None
    return {'argv': argv, 'cwd': str(cwd), 'exit': code, 'log': str(log)}


def agent_cli(args, agent):
    package = getattr(args, agent + '_package')
    return package / ('dist/cli.js' if agent == 'omp' else 'dist/bundle/cli.js')


def checks(args, evidence):
    args.target.mkdir(parents=True, exist_ok=True)
    require(not any(args.target.iterdir()), 'full checks require a new exclusive empty CARGO_TARGET_DIR')
    (args.target / '.paired-chief-run-owner').write_text(str(args.repo), encoding='utf-8')
    env = {key: value for key, value in os.environ.items() if not key.upper().startswith(('HERDR', 'HC_', 'PI_', 'OMP_'))
           and not any(term in key.upper() for term in ('API_KEY', 'TOKEN', 'PASSWORD', 'CREDENTIAL', 'SECRET', 'PROXY'))}
    env.update(CARGO_TARGET_DIR=str(args.target), HERDR_BOT_REPO=str(args.repo), HERDR_BOT_BINARY=str(args.safe),
               HERDR_BOT_METADATA=str(args.repo / 'meta-data/task-options.toml'), OMP_BASELINE_RECORD_ROOT=str(args.toolkit_baseline_root),
               PI_NODE=str(args.node), PI_CLI=str(agent_cli(args, 'pi')), OMP_CLI=str(agent_cli(args, 'omp')),
               HARNESS_ACCEPTANCE_EVIDENCE_DIR=str(args.evidence_dir / 'toolkit-harness-evidence'))
    npm, cargo = [shutil.which(command) or command for command in ('npm.cmd', 'cargo')]
    frozen = [args.bun, 'install', '--frozen-lockfile', '--linker', 'hoisted']
    phases = [
        ('frontend-install', args.repo / 'frontend', frozen),
        ('toolkit-install', args.toolkit, frozen),
        ('frontend-build', args.repo, [npm, '--prefix', 'frontend', 'run', 'build']),
        ('cargo-build', args.repo, [cargo, 'build', '--locked', '--manifest-path', 'backend/Cargo.toml',
            '--bin', 'herdr-bot', '--bin', 'herdr-paths', '--bin', 'herdr-fixture',
            '--example', 'safe_shutdown_shell', '--example', 'paired_native_receipt']),
        ('backend', args.repo, [args.node, 'scripts/test-backend.mjs', '--binary-jobs', '2']),
        ('frontend', args.repo, [npm, '--prefix', 'frontend', 'test']),
        ('browser', args.repo, [npm, '--prefix', 'frontend', 'run', 'test:browser:build', '--', '--workers=1']),
    ]
    phases += [('toolkit-' + name, args.toolkit, [args.bun, 'run', name])
               for name in ('typecheck', 'test:framework', 'check:integrations', 'test')]
    paths = [f'tests/harness-dock/{name}.test.ts' for name in DOCKS]
    phases.append(('toolkit-docks', args.toolkit, [args.bun, 'test', *paths]))
    for name, cwd, argv in phases:
        log = args.evidence_dir / (name.replace(':', '-') + '.log')
        if name in ('toolkit-test', 'toolkit-docks') and not args.safe.is_file():
            log.write_text('The current safe shell is absent. No production-backend fallback.\n', encoding='utf-8')
            record = {'argv': list(map(str, argv)), 'exit': None, 'log': str(log)}
        else:
            record = capture(argv, cwd, env, log)
        evidence['checks'][name] = record
        if name == 'backend':
            match = re.search(r'^Summary: (.+summary\.json)\s*$', log.read_text(encoding='utf-8', errors='replace'), re.M)
            if match and Path(match[1]).is_file():
                shutil.copytree(Path(match[1]).parent, args.evidence_dir / 'backend-original-logs')
        elif name == 'browser' and (args.repo / 'frontend/test-results').is_dir():
            shutil.copytree(args.repo / 'frontend/test-results', args.evidence_dir / 'browser-original-results')
        save(args.evidence_dir / 'summary.json', evidence)


def artifacts(args, evidence):
    for name, path, source in [('backend', args.backend, 'src/main.rs'),
                               ('safeShutdownShell', args.safe, 'examples/safe_shutdown_shell.rs'),
                               ('terminalRecorder', args.recorder, 'examples/paired_native_receipt.rs')]:
        deps = path.with_suffix('.d').read_text(encoding='utf-8').replace('\\', '/')
        path_source = args.repo / 'backend' / source
        tokens = deps.split(': ', 1)[1].split()
        require((path_source.as_posix() in tokens or (source.startswith('examples/') and source in tokens)) and
                path.stat().st_mtime >= path_source.stat().st_mtime, ('not built for this worktree/source', path, path_source))
        evidence['artifacts'][name] = {'path': str(path), 'sha256': digest(path), 'depfile': deps}
    page = args.repo / 'backend/static'
    require((page / 'index.html').is_file(), 'missing built frontend in specified worktree')
    evidence['artifacts']['page'] = {file.relative_to(page).as_posix(): digest(file) for file in page.rglob('*') if file.is_file()}
    for name in ('herdr', 'app', 'core', 'node', 'bun'):
        path = getattr(args, name)
        log = args.evidence_dir / ('installed-' + name + '.log')
        record = capture([path, '--version'], args.repo, os.environ.copy(), log)
        require(record['exit'] == 0, record)
        evidence['installations'][name] = {'path': str(path), 'sha256': digest(path),
                                           'version': log.read_text(encoding='utf-8', errors='replace').strip()}
    for agent in ('omp', 'pi'):
        package = getattr(args, agent + '_package')
        metadata = json.loads((package / 'package.json').read_text(encoding='utf-8'))
        launcher = Path(shutil.which(agent + '.cmd') or '')
        require(launcher.is_file() and (launcher.parent / 'node_modules' / metadata['name']).resolve() == package,
                ('PATH launcher does not resolve to declared package', agent, launcher, package))
        entry = agent_cli(args, agent)
        evidence['installations'][agent] = {'path': str(package), 'version': metadata['version'], 'entrySha256': digest(entry),
            'launcher': str(launcher), 'launcherSha256': digest(launcher),
            'officialTuiExtensionSha256': digest(getattr(args, agent + '_state_extension'))}


def main():
    args = parse()
    try:
        args.evidence_dir = absolute(args.evidence_dir)
        args.evidence_dir.mkdir(parents=True, exist_ok=False)
    except (OSError, AssertionError) as error:
        print(error, file=sys.stderr)
        return 1
    summary = {'passed': False, 'realNetwork': 'not used or verified; no user credentials accessed',
               'provider': 'controlled numeric loopback; no claim of real model judgment or business quality',
               'checks': {}, 'artifacts': {}, 'installations': {},
               'scenarios': {name: {'passed': False, 'status': 'not run'} for name in MATRIX}}
    try:
        install_opener(build_opener(ProxyHandler({})))
        selected = configure(args, summary)
        summary['sources'] = {name: source_identity(root, lock, args.evidence_dir) for name, root, lock in
            [('herdr-bot', args.repo, 'backend/Cargo.lock'), ('toolkit', args.toolkit, 'bun.lock')]}
        if not args.native_only:
            checks(args, summary)
        artifacts(args, summary)
        if not args.checks_only:
            for name in selected:
                result = ChiefRun(args, *MATRIX[name], args.evidence_dir / name).run()
                summary['scenarios'][name] = {key: result[key] for key in ('passed', 'archive', 'nativeRoot', 'cleanupErrors', 'verified')}
                summary['scenarios'][name]['error'] = result.get('verificationError')
                save(args.evidence_dir / 'summary.json', summary)
                print(name, 'passed' if result['passed'] else 'failed', flush=True)
                if result['cleanupErrors']:
                    break
        summary['passed'] = (not args.native_only and not args.checks_only and all(row['passed'] for row in summary['scenarios'].values())
                             and bool(summary['checks']) and all(row['exit'] == 0 for row in summary['checks'].values()))
    except Exception:
        summary['error'] = traceback.format_exc()
        print(summary['error'], file=sys.stderr)
    finally:
        save(args.evidence_dir / 'summary.json', summary)
    return 0 if summary['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
