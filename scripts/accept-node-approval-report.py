"""Ticket 10 S2: installed OMP/Pi approval, free reporting and chief boundaries.

Controlled provider choices are mechanical protocol evidence, never evidence of a
real model's business judgment. All tools, native UI and backend effects are real.
"""
import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import traceback

REPO = Path(__file__).resolve().parents[1]
EXPECTED = {
    'P2': 'Both dispatched ordinary nodes initially expose approved=false, then native approve changes that fact.',
    'P4': 'Automatic approval is an actual zero-argument tool call, with approvalSource=auto.',
    'P5': 'Only actual selected native Yes approves manual work; chat/report claims do not.',
    'P6': 'Manual native message is exactly 当前会话的成果是否已达到预期，可以交接并推进后续流程？ without payload.',
    'P9': 'Both modes approve a legal execution with no report, output or artifact and no model-supplied identity.',
    'P12': 'Actual deny-delete storage failure survives native feedback; public false and complete refusal remain.',
    'P13': 'Repeated and truly overlapping native approvals produce one fact and no duplicate external effects.',
    'P17': 'Both modes freely save and notify progress/problem/failure/result reports before and after approval.',
    'P18': 'Report-only never approves; approve-only never creates a report.',
    'P19': 'Approval changes neither manager input nor external calls; ordinary report notifications still occur.',
    'P23': 'Approval does not complete, dispatch or finish; only an explicit manager operation advances.',
    'P37': 'Existing chief regressions pass and both real native chief confirmations remain observable.',
    'P38': 'Native successful reports have no obsolete confirmation expectation.',
}


def absolute(value):
    path = Path(value)
    if not path.is_absolute():
        raise argparse.ArgumentTypeError('Expected an explicit absolute path')
    return path.resolve()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['herdr-bot-worktree', 'toolkit-worktree', 'backend', 'paths-file', 'model-config', 'evidence-dir']:
        parser.add_argument('--' + name, type=absolute, required=True)
    parser.add_argument('--hosts', choices=['omp,pi', 'omp', 'pi'], required=True)
    parser.add_argument('--timeout', type=int, choices=range(1, 601), default=180)
    parser.add_argument('--node', default='node')
    parser.add_argument('--bun', default='bun')
    parser.add_argument('--shell-path', type=absolute)
    args = parser.parse_args()
    args.evidence_dir.mkdir(parents=True, exist_ok=False)
    evidence = {'passed': False, 'coverage': {host: {key: {'verified': False, 'expected': expected}
        for key, expected in EXPECTED.items()} for host in ['omp', 'pi']}, 'commands': [],
        'cleanupErrors': [], 'selectedHosts': args.hosts.split(','), 'nativeExits': {},
        'scope': 'Controlled provider drives real installed hosts, actual UI and production backend; no real-model judgment claim'}
    try:
        from acceptance.host.processes import require
        require(args.paths_file.is_file(), 'Missing explicit paths file: ' + str(args.paths_file))
        require(args.herdr_bot_worktree == REPO == Path.cwd().resolve(), 'Run from the explicitly supplied herdr-bot worktree')
        require((args.toolkit_worktree / '.git').is_file(), 'Supply the participating toolkit worktree')
        require(args.model_config.is_file(), 'Missing explicit model configuration')
        config = json.loads(args.model_config.read_text(encoding='utf-8'))
        require(config == {'mode': 'controlled', 'scope': 'ticket-10-native-mechanics'},
            'This mechanical runner requires explicit {"mode":"controlled","scope":"ticket-10-native-mechanics"}; it never discovers credentials')
        evidence['providerConfiguration'] = {'path': str(args.model_config), 'sha256': hashlib.sha256(args.model_config.read_bytes()).hexdigest(), **config}
        args.repo, args.toolkit = args.herdr_bot_worktree, args.toolkit_worktree
        target = Path(os.environ.get('CARGO_TARGET_DIR', str(args.evidence_dir / 'cargo-target'))).resolve()
        require(not target.is_relative_to(args.repo / 'backend/target'), 'Use an independent absolute CARGO_TARGET_DIR')
        require(args.backend == target / 'debug/herdr-bot.exe', '--backend must name this invocation CARGO_TARGET_DIR/debug/herdr-bot.exe')
        env = dict(os.environ, CARGO_TARGET_DIR=str(target), HERDR_BOT_REPO=str(args.repo),
            HERDR_BOT_METADATA=str(args.repo / 'meta-data/task-options.toml'),
            HERDR_BOT_BINARY=str(target / 'debug/examples/safe_shutdown_shell.exe'),
            WORKFLOW_PROMPT_EVIDENCE_DIR=str(args.evidence_dir / 'chief-regressions'))
        evidence['checkEnvironment'] = {key: env[key] for key in ['CARGO_TARGET_DIR', 'HERDR_BOT_REPO', 'HERDR_BOT_METADATA', 'HERDR_BOT_BINARY']}
        def check(cwd, argv, label):
            record = {'label': label, 'cwd': str(cwd), 'argv': argv, 'verified': False}
            evidence['commands'].append(record)
            try:
                result = subprocess.run(argv, cwd=cwd, env=env, capture_output=True, text=True,
                    encoding='utf-8', errors='replace', timeout=7200)
                record.update(exit=result.returncode, stdout=result.stdout, stderr=result.stderr, verified=result.returncode == 0)
            except BaseException:
                record['error'] = traceback.format_exc()
            (args.evidence_dir / (label + '.json')).write_text(json.dumps(record, ensure_ascii=False, indent=2), encoding='utf-8')
            print(json.dumps({'check': label, 'verified': record['verified']}, ensure_ascii=False), flush=True)
            return record['verified']
        npm = 'npm.cmd' if os.name == 'nt' else 'npm'
        check(args.repo, [npm, '--prefix', 'frontend', 'run', 'build'], 'frontend-build')
        built = check(args.repo, ['cargo', 'build', '--manifest-path', 'backend/Cargo.toml', '--bin', 'herdr-bot', '--bin', 'herdr-fixture', '--bin', 'herdr-paths', '--example', 'safe_shutdown_shell', '--example', 'recovery_herdr_proxy'], 'backend-build')
        check(args.repo, [args.node, 'scripts/test-backend.mjs'], 'backend-tests')
        check(args.repo, [npm, '--prefix', 'frontend', 'test'], 'frontend-tests')
        check(args.repo, [npm, '--prefix', 'frontend', 'run', 'test:browser:build'], 'browser-tests')
        for name in ['typecheck', 'test', 'check:integrations']:
            check(args.toolkit, [args.bun, 'run', name], 'toolkit-' + name.replace(':', '-'))
        chief = check(args.toolkit, [args.bun, 'test', 'tests/harness-dock/chief-delegation.test.ts',
            'tests/harness-dock/native-chief-report.test.ts', 'tests/harness-dock/native-node-report.test.ts'], 'native-regressions')
        require(built, 'Current worktree backend build failed; raw check failure retained')
        from acceptance.host.provenance import preflight, source_identity
        preflight(args, evidence)
        evidence['participatingSources'] = {name: source_identity(root, lock, args.evidence_dir)
            for name, root, lock in [('herdr-bot', args.repo, 'backend/Cargo.lock'), ('toolkit', args.toolkit, 'bun.lock')]}
        args.path_roots = evidence['paths']['roots']
        for host in ['omp', 'pi']:
            name = host + '_state_extension'
            require(name in args.path_roots and Path(args.path_roots[name]).is_file(), 'Missing explicit readonly roots.' + name)
            setattr(args, name, Path(args.path_roots[name]).resolve())
        from node_approval_report_acceptance import NodeApprovalReportAcceptance
        for host in args.hosts.split(','):
            host_args = copy.copy(args)
            host_args.host = host
            run = NodeApprovalReportAcceptance(host_args)
            status = run.run()
            evidence['nativeExits'][host] = status
            for key in EXPECTED:
                evidence['coverage'][host][key].update(run.evidence['coverage'].get(key, {}),
                    evidence=str(run.root / 'evidence.json'), exit=status)
            evidence['coverage'][host]['P37']['verified'] = evidence['coverage'][host]['P37']['verified'] and chief
            evidence['cleanupErrors'].extend(run.evidence['cleanupErrors'])
        evidence['passed'] = all(item['verified'] for host in evidence['coverage'].values() for item in host.values()) and all(
            evidence['nativeExits'].get(host) == 0 for host in ['omp', 'pi']) and all(
            item['verified'] for item in evidence['commands']) and not evidence['cleanupErrors']
    except BaseException:
        evidence['error'] = traceback.format_exc()
    evidence['entrypoint'] = {'path': str(Path(__file__).resolve()), 'sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    report = args.evidence_dir / 'evidence.json'
    report.write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps({'passed': evidence['passed'], 'evidence': str(report), 'coverage': evidence['coverage']}, ensure_ascii=False, indent=2))
    return 0 if evidence['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
