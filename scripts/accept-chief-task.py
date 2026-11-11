"""P8/P9/P10(Task)/P11/P13: real chief routing and independent Task Chat acceptance.

Explicit worktrees, installed hosts, authorized GPT-6-luna, exclusive evidence.
No installation, fallback, business retry or credential discovery.
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
ITEMS = ['P8/known', 'P8/ambiguous', 'P8/missing', 'P9/corrections',
         'P10/approved', 'P10/denied', 'P10/cancelled', 'P10/timeout',
         'P10/unavailable', 'P10/target-change', 'P10/zone-change', 'P11', 'P13']


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
    evidence = {'passed': False, 'coverage': {host: {key: {'verified': False} for key in ITEMS}
        for host in ['omp', 'pi']}, 'commands': [], 'cleanupErrors': [], 'selectedHosts': args.hosts.split(','),
        'modelPolicy': 'Only GPT-6-luna; actual returned identity required; no business retries'}
    try:
        def require(condition, message):
            if not condition:
                raise AssertionError(message)
        require(args.backend.is_file(), 'Missing current worktree backend: ' + str(args.backend))
        require(args.herdr_bot_worktree == REPO == Path.cwd().resolve(), 'Run from the explicitly supplied herdr-bot worktree')
        require(args.model_config.is_file(), 'Missing authorized isolated GPT-6-luna model config')
        config = json.loads(args.model_config.read_text(encoding='utf-8'))
        require(config.get('model') == 'gpt-6-luna', 'Actual model must be gpt-6-luna; no fallback')
        require(config.get('apiKeyEnv') and os.environ.get(config['apiKeyEnv']), 'Missing explicitly authorized model credential environment')
        args.repo, args.toolkit = args.herdr_bot_worktree, args.toolkit_worktree
        resources = runpy.run_path(str(REPO / 'scripts/accept-suite-pi-resources.py'))
        resources['preflight'](args, evidence)
        args.path_roots = evidence['paths']['roots']
        for host in args.hosts.split(','):
            name = host + '_state_extension'
            require(name in args.path_roots and Path(args.path_roots[name]).is_file(), 'Missing explicit readonly roots.' + name)
            setattr(args, name, Path(args.path_roots[name]).resolve())
        # Base isolation also checks Pi's official file when only OMP is selected.
        if not hasattr(args, 'pi_state_extension'):
            args.pi_state_extension = Path(args.path_roots['pi_state_extension'])
        # Reuse ticket 10's permanent real no-UI host cases, not duplicate them.
        # Controlled provider is explicitly mechanical, never P8/P9 model evidence.
        baseline = args.toolkit / '.cache/omp-baseline/18.5.0/node_modules/@oh-my-pi/pi-coding-agent/package.json'
        require(baseline.is_file(), 'Missing preinstalled isolated dock baseline; acceptance does not install hosts')
        evidence['protocolIdentity'] = {'baselinePackage': str(baseline),
            'baselineVersion': json.loads(baseline.read_text(encoding='utf-8'))['version'],
            'baselinePackageSha256': hashlib.sha256(baseline.read_bytes()).hexdigest(),
            'test': 'tests/harness-dock/chief-delegation.test.ts',
            'testSha256': hashlib.sha256((args.toolkit / 'tests/harness-dock/chief-delegation.test.ts').read_bytes()).hexdigest()}
        require(evidence['protocolIdentity']['baselineVersion'] == '18.5.0' and
            os.environ.get('OMP_TEST_VERSION', '18.5.0') == '18.5.0',
            'Use the existing fixed dock baseline; acceptance never installs a candidate host')
        for host in args.hosts.split(','):
            command = [args.bun, 'test', 'tests/harness-dock/chief-delegation.test.ts',
                '--test-name-pattern', host + ' task chief cannot approve in actual headless mode']
            environment = dict(os.environ, WORKFLOW_PROMPT_EVIDENCE_DIR=str(args.evidence_dir / 'protocol' / host),
                PI_NODE=args.node, PI_CLI=str(args.pi_package / 'dist/bundle/cli.js'))
            result = subprocess.run(command, cwd=args.toolkit, env=environment, capture_output=True,
                text=True, encoding='utf-8', errors='replace', timeout=300)
            record = {'argv': command, 'exit': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr,
                'scope': 'Existing native headless protocol case; controlled provider, not model behavior'}
            evidence['commands'].append(record)
            passed = result.returncode == 0 and '1 pass' in result.stderr and '0 fail' in result.stderr
            evidence['coverage'][host]['P10/unavailable'] = {'verified': passed, 'command': record}
        native = runpy.run_path(str(REPO / 'scripts/chief_task_native_acceptance.py'))['ChiefTaskAcceptance']
        for host in args.hosts.split(','):
            host_args = copy.copy(args)
            host_args.host = host
            run = native(host_args)
            status = run.run()
            for key in ITEMS:
                evidence['coverage'][host][key].update(run.evidence['coverage'].get(key, {}),
                    evidence=str(run.root / 'evidence.json'), exit=status)
            evidence['cleanupErrors'].extend(run.evidence['cleanupErrors'])
        evidence['passed'] = all(item['verified'] for host in evidence['coverage'].values()
            for item in host.values()) and not evidence['cleanupErrors']
    except BaseException:
        evidence['error'] = traceback.format_exc()
    evidence['entrypoint'] = {'path': str(Path(__file__).resolve()), 'sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    report = args.evidence_dir / 'evidence.json'
    report.write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps({'passed': evidence['passed'], 'evidence': str(report)}, indent=2))
    return 0 if evidence['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
