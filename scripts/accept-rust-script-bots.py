"""Reproducible Rust Bot S1/S2/S3 acceptance; H1/H2 always require the user.

Run from --herdr-bot-worktree. Explicit artifacts only; no credentials, normal
services, business retries, installation, or evidence-directory reuse.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import traceback

ROOT = Path(__file__).resolve().parents[1]


def absolute(value):
    path = Path(value)
    if not path.is_absolute():
        raise argparse.ArgumentTypeError('Expected an explicit absolute path')
    return path.resolve()


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def identity(root):
    paths = subprocess.check_output(['git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z'], cwd=root).decode().split('\0')
    files = {p: digest(root / p) for p in paths if p and not p.startswith('.scratch/') and (root / p).is_file()}
    return {'root': str(root), 'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root).decode().strip(), 'files': files}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['herdr-bot-worktree', 'toolkit-worktree', 'backend', 'static-dir', 'native-entry', 'isolation-config', 'evidence-dir']:
        parser.add_argument('--' + name, type=absolute, required=True)
    args = parser.parse_args()
    args.evidence_dir.mkdir(parents=True, exist_ok=False)
    evidence = {'automatedPassed': False, 'commands': [], 'cleanupErrors': [], 'coverage': {},
                'human': {name: {'status': 'unverified', 'confirmationOwner': 'user'} for name in ['H1', 'H2']},
                'scope': 'Controlled native provider verifies protocol, not manager understanding or real model compliance'}
    try:
        require(args.backend.is_file(), 'Missing current worktree backend: ' + str(args.backend))
        require(ROOT == args.herdr_bot_worktree == Path.cwd().resolve(), 'Run from the explicitly supplied herdr-bot worktree root')
        require((args.toolkit_worktree / '.git').is_file(), 'Expected explicit toolkit worktree')
        require(args.native_entry == args.toolkit_worktree / 'scripts/rust-script-acceptance.ts' and args.native_entry.is_file(), 'Mismatched toolkit native entry')
        require(args.static_dir == ROOT / 'backend/static' and (args.static_dir / 'index.html').is_file(), 'Mismatched or missing current static page')
        depfile = args.backend.with_suffix('.d')
        require(args.backend.name == 'herdr-bot.exe' and depfile.is_file(), 'Production backend and Cargo depfile required; never substitute safe shutdown shell')
        deps = depfile.read_text(encoding='utf-8').replace('\\', '/')
        require(str(ROOT / 'backend/src/main.rs').replace('\\', '/') in deps, 'Backend Cargo identity names another worktree')
        sources = list((ROOT / 'backend/src').rglob('*.rs')) + [ROOT / 'backend/Cargo.lock', ROOT / 'backend/Cargo.toml']
        pages = [p for p in args.static_dir.rglob('*') if p.is_file()]
        # Pages are served from static_directory, not compiled into the Rust binary.
        # Cargo may legitimately reuse that binary after an identical page rebuild.
        require(all(p.stat().st_mtime <= args.backend.stat().st_mtime for p in sources), 'Backend predates current Rust source; rebuild this worktree')
        frontend_sources = [p for p in (ROOT / 'frontend/src').rglob('*') if p.is_file()] + [ROOT / 'frontend/bun.lock', ROOT / 'frontend/package.json']
        require(all(p.stat().st_mtime <= (args.static_dir / 'index.html').stat().st_mtime for p in frontend_sources), 'Static page predates current frontend source/lock')
        target = args.backend.parent.parent
        artifacts = [args.backend, args.backend.parent / 'herdr-fixture.exe', args.backend.parent / 'herdr-paths.exe', args.backend.parent / 'examples/safe_shutdown_shell.exe']
        for file in artifacts:
            require(file.is_file() and file.with_suffix('.d').is_file(), 'Missing paired worktree artifact: ' + str(file))
            require(str(ROOT).replace('\\', '/') in file.with_suffix('.d').read_text(encoding='utf-8').replace('\\', '/'), 'Paired artifact names another worktree: ' + str(file))
        config = json.loads(args.isolation_config.read_text(encoding='utf-8'))
        for key in ['bun', 'node', 'piCli', 'piNode', 'ompBaselineRecordRoot', 'chrome']:
            require(key in config and Path(config[key]).is_absolute() and Path(config[key]).exists(), 'Missing explicit installed isolation setting: ' + key)
        require(config.get('externalMode') == 'controlled-protocol-only', 'Automation must explicitly select controlled protocol; H1/H2 are separate user checks')
        require(subprocess.check_output([config['bun'], '--version'], shell=os.name == 'nt').decode().strip() == '1.4.4', 'Bun 1.4.4 required')
        evidence.update(arguments={k: str(v) for k, v in vars(args).items()}, isolation=config,
                        repositories={'herdr-bot': identity(ROOT), 'toolkit': identity(args.toolkit_worktree)},
                        artifacts={str(p): {'sha256': digest(p), 'depfile': p.with_suffix('.d').read_text(encoding='utf-8')} for p in artifacts},
                        pages={str(p.relative_to(ROOT)): digest(p) for p in pages},
                        environment={key: os.environ.get(key) for key in ['OS', 'PROCESSOR_ARCHITECTURE', 'CARGO_BUILD_JOBS']})
        env = dict(os.environ, CARGO_TARGET_DIR=str(target), CARGO_BUILD_JOBS='1', TOKIO_WORKER_THREADS='2',
                   HERDR_BOT_REPO=str(ROOT), HERDR_BOT_BINARY=str(artifacts[-1]), HERDR_BOT_METADATA=str(ROOT / 'meta-data/task-options.toml'),
                   PI_CLI=config['piCli'], PI_NODE=config['piNode'], OMP_BASELINE_RECORD_ROOT=config['ompBaselineRecordRoot'],
                   WORKFLOW_DUAL_EVIDENCE_DIR=str(args.evidence_dir / 'native-protocol'), WORKFLOW_PROMPT_EVIDENCE_DIR=str(args.evidence_dir / 'native-protocol'),
                   RUST_SCRIPT_ACCEPTANCE_DIR=str(args.evidence_dir / 'browser'), RUST_SCRIPT_CHROME=config['chrome'])
        def run(name, command, cwd):
            stdout, stderr = args.evidence_dir / (name + '.stdout.log'), args.evidence_dir / (name + '.stderr.log')
            record = {'name': name, 'argv': list(map(str, command)), 'cwd': str(cwd), 'stdout': str(stdout), 'stderr': str(stderr)}
            evidence['commands'].append(record)
            with stdout.open('w', encoding='utf-8') as out, stderr.open('w', encoding='utf-8') as err:
                result = subprocess.run(record['argv'], cwd=cwd, env=env, stdout=out, stderr=err, shell=os.name == 'nt')
            record['exit'] = result.returncode
            print(name + ': ' + str(result.returncode), flush=True)
            return result.returncode == 0
        # Existing owner 03's P15 test is executed, not copied or reassigned.
        evidence['coverage']['P15/existing-owner-03'] = run('P15-existing', ['cargo', 'test', '--locked', '--manifest-path', 'backend/Cargo.toml', '--test', 'workflow_dispatch', 'rust_save_material_failure_preserves_saved_effect_and_original_baseline_conflicts_in_another_run', '--', '--nocapture', '--test-threads=1'], ROOT)
        evidence['coverage']['P23/page'] = run('P23-browser', ['npm', '--prefix', 'frontend', 'run', 'test:browser', '--', 'scriptMigrationAcceptance.spec.ts', '--workers=1', '--output', str(args.evidence_dir / 'browser-results')], ROOT)
        evidence['coverage']['native-protocol/enabling-10'] = run('manager-native-protocol', [config['bun'], 'test', 'tests/harness-dock/manager-script-protocol.test.ts'], args.toolkit_worktree)
        evidence['coverage']['P2/P27/P29/H2-mechanics'] = run('cross-repository-business', [config['bun'], str(args.native_entry), '--herdr-bot-worktree', str(ROOT), '--isolation-config', str(args.isolation_config), '--evidence-dir', str(args.evidence_dir / 'business')], args.toolkit_worktree)
        business = args.evidence_dir / 'business/evidence.json'
        if business.is_file():
            evidence['business'] = json.loads(business.read_text(encoding='utf-8'))
            evidence['cleanupErrors'].extend(evidence['business'].get('cleanupErrors', []))
        evidence['automatedPassed'] = all(evidence['coverage'].values()) and not evidence['cleanupErrors']
        evidence['human']['H1']['procedure'] = 'docs/usage.md#rust-script-cross-repository-acceptance'
        evidence['human']['H2']['procedure'] = 'docs/usage.md#rust-script-cross-repository-acceptance'
        human_plan = {
            'status': 'unverified', 'confirmationOwner': 'user',
            'automatedPassed': evidence['automatedPassed'],
            'blockers': ['No explicitly authorized real Herdr Workspace/manager session supplied', 'No explicitly authorized real manager model/paths configuration supplied'],
            'authorizationBoundary': 'Only isolated definitions/materials; never execute the newly bound business target, normal services, user Chat or credential discovery',
            'artifacts': evidence['artifacts'], 'isolation': config,
            'businessEvidence': str(business), 'pageEvidence': str(args.evidence_dir / 'browser-results'),
            'procedure': str(ROOT / 'docs/usage.md') + '#rust-script-cross-repository-acceptance',
            'H1': {'promises': ['P3', 'P5', 'P10', 'P12', 'P29', 'P34'],
                   'expected': 'Both real Rust capabilities: dispatch returns, actual manager receives original backend report, collect/read consumes named files, explicit complete; no script Chat/Tab',
                   'collect': ['real manager/native identity and full configuration', 'original run/collect/read tool calls/results', 'actual notification and report', 'full public Bot/target/revision and complete records', 'screenshots and no-script-Chat/Tab observations', 'process exit confirmations']},
            'H2': {'promises': ['P13', 'P14', 'P35'],
                   'expected': 'Saved Bot survives handoff collision; user repairs collision; real manager explicitly fail/retry; new execution preserves old reports/outputs, original baseline conflicts, old report cannot complete new execution',
                   'collect': ['owned file collision and repair bytes', 'real false report and partial effects presentation', 'explicit fail/confirmed retry tool results', 'old/current execution, reports, output bindings and actual files', 'real manager interpretation and user verdict', 'original errors and separate cleanup errors']}}
        (args.evidence_dir / 'human-checks.json').write_text(json.dumps(human_plan, ensure_ascii=False, indent=2), encoding='utf-8')
    except BaseException:
        evidence['error'] = traceback.format_exc()
    (args.evidence_dir / 'evidence.json').write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps({'automatedPassed': evidence['automatedPassed'], 'human': evidence['human'], 'evidence': str(args.evidence_dir / 'evidence.json')}, ensure_ascii=False))
    return 0 if evidence['automatedPassed'] else 1


if __name__ == '__main__':
    sys.exit(main())
