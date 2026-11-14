"""P7/P29/P30: delivered chief entry, independent hosts, exclusive resources.

Run from the explicit Herdr worktree. No installation, provider fallback, business
retry or user credential discovery. Every missing dependency remains nonzero.
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
PROMISES = {'P7': 'No-Suite native launch and new-process resume consume chief tools/guidance',
            'P29': 'Concurrent public/browser entries recognize only one chief',
            'P30': 'Offline retention, restoration, expiry and unregister agree with delivered page'}


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
    # Exclusive creation is outside the reporting block: never touch an earlier run.
    args.evidence_dir.mkdir(parents=True, exist_ok=False)
    hosts = args.hosts.split(',')
    evidence = {'passed': False, 'coverage': {host: {key: {'expected': value, 'verified': False}
        for key, value in PROMISES.items()} for host in ['omp', 'pi']}, 'commands': [], 'cleanupErrors': [],
        'modelPolicy': 'Only gpt-6-luna; actual returned model identity required; no business retries',
        'selectedHosts': hosts}
    try:
        def require(condition, message):
            if not condition:
                raise AssertionError(message)
        require(args.backend.is_file(), 'Missing current worktree backend: ' + str(args.backend))
        require(args.herdr_bot_worktree == REPO == Path.cwd().resolve(), 'Run from the explicitly supplied herdr-bot worktree')
        require(args.toolkit_worktree.joinpath('.git').is_file(), 'Expected explicit toolkit Git worktree')
        require(args.model_config.is_file(), 'Missing authorized isolated GPT-6-luna model config')
        config = json.loads(args.model_config.read_text(encoding='utf-8'))
        require(config.get('model') == 'gpt-6-luna', 'Actual model must be gpt-6-luna; no fallback')
        require(config.get('apiKeyEnv') and os.environ.get(config['apiKeyEnv']), 'Missing explicitly authorized model credential environment')
        args.repo, args.toolkit = args.herdr_bot_worktree, args.toolkit_worktree
        from acceptance.host.provenance import preflight
        preflight(args, evidence)
        roots = evidence['paths']['roots']
        args.path_roots = roots
        for host in hosts:
            name = host + '_state_extension'
            require(name in roots and Path(roots[name]).is_file(), 'Missing explicit readonly roots.' + name)
            setattr(args, name, Path(roots[name]).resolve())
        require('chrome' in roots, 'Missing explicit installed roots.chrome directory')
        require((Path(roots['chrome']) / 'chrome.exe').is_file(), 'Missing installed browser')
        # Browser S1/S2 owns its controlled external CLI, not the native P7 claim.
        environment = dict(os.environ, CHIEF_ENTRY_EVIDENCE_DIR=str(args.evidence_dir / 'browser'),
            CHIEF_ENTRY_HOSTS=args.hosts, CHIEF_ENTRY_CHROME=str(Path(roots['chrome']) / 'chrome.exe'))
        command = ['npm', '--prefix', 'frontend', 'run', 'test:browser', '--', 'chief/chiefEntryAcceptance.spec.ts', '--workers=1']
        result = subprocess.run(command, cwd=REPO, env=environment, capture_output=True, text=True,
            encoding='utf-8', errors='replace', timeout=600, shell=os.name == 'nt')
        record = {'argv': command, 'exit': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr}
        evidence['commands'].append(record)
        for host in hosts:
            for promise in ['P29', 'P30']:
                report = args.evidence_dir / 'browser' / f'{host}-{promise}.json'
                if report.is_file():
                    item = json.loads(report.read_text(encoding='utf-8'))
                    evidence['coverage'][host][promise].update(verified=result.returncode == 0 and item.get('passed') is True,
                        evidence=str(report), observations=item)
        # Do not skip the other host after a failure; each has its own owned root.
        native = runpy.run_path(str(REPO / 'scripts/chief_entry_native_acceptance.py'))['ChiefEntryAcceptance']
        for host in hosts:
            host_args = copy.copy(args)
            host_args.host = host
            run = native(host_args)
            status = run.run()
            evidence['coverage'][host]['P7'].update(verified=status == 0,
                evidence=str(run.root / 'evidence.json'), exit=status)
            evidence['cleanupErrors'].extend(run.evidence['cleanupErrors'])
        evidence['passed'] = all(item['verified'] for host in evidence['coverage'].values() for item in host.values()) and not evidence['cleanupErrors']
    except BaseException:
        evidence['error'] = traceback.format_exc()
    evidence['entrypoint'] = {'path': str(Path(__file__).resolve()), 'sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    report = args.evidence_dir / 'evidence.json'
    report.write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps({'passed': evidence['passed'], 'evidence': str(report)}, indent=2))
    return 0 if evidence['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
