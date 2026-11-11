"""P9(Map)/P10(Map)/P12/P14/P15/P16: installed native hosts and GPT-6-luna.

All dependency paths are explicit. No installs, credential discovery, model fallback,
operation retries or reuse of another acceptance run's business conclusions.
"""
import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import runpy
import sys
import traceback

REPO = Path(__file__).resolve().parents[1]
PROMISES = {
    'P9': 'Map parameters and conversational corrections match actual preview and dispatch',
    'P10': 'Map native approve/deny/cancel/timeout/unavailable and stale target/location',
    'P12': 'Actual Run/manager/node retain distinct zones and internal management',
    'P14': 'Single-executor/multi-step gaps choose shipped complete Task/Map creation Bots',
    'P15': 'Reuse rejection alone waits; explicit Task/Map creation and abandonment remain distinct',
    'P16': 'Creation stage approvals remain independent; returned creation outcome starts no business',
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
    evidence = {'passed': False, 'coverage': {host: {key: {'expected': value, 'verified': False}
        for key, value in PROMISES.items()} for host in ['omp', 'pi']}, 'commands': [], 'cleanupErrors': [],
        'modelPolicy': 'Only gpt-6-luna; actual returned identity required; no business retries',
        'selectedHosts': args.hosts.split(',')}
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
        resources = runpy.run_path(str(REPO / 'scripts/accept-suite-pi-resources.py'))
        resources['preflight'](args, evidence)
        args.path_roots = evidence['paths']['roots']
        # Complete creation graphs retain their delivered target hosts even when
        # the chief scenario is filtered to one host.
        for host in ['omp', 'pi']:
            name = host + '_state_extension'
            require(name in args.path_roots and Path(args.path_roots[name]).is_file(), 'Missing explicit readonly roots.' + name)
            setattr(args, name, Path(args.path_roots[name]).resolve())
        native = runpy.run_path(str(REPO / 'scripts/chief_map_native_acceptance.py'))
        evidence['scenarios'] = []
        for host in args.hosts.split(','):
            for scenario, owned in native['SCENARIOS'].items():
                selected = copy.copy(args)
                selected.host, selected.scenario = host, scenario
                selected.evidence_dir = args.evidence_dir / scenario
                selected.evidence_dir.mkdir(exist_ok=True)
                run = native['ChiefMapAcceptance'](selected)
                status = run.run()
                report = {'host': host, 'scenario': scenario, 'exit': status,
                          'evidence': str(run.root / 'evidence.json'), 'coverage': run.evidence['coverage']}
                evidence['scenarios'].append(report)
                evidence['cleanupErrors'].extend(run.evidence['cleanupErrors'])
            for promise in PROMISES:
                cases = [case for case, owned in native['SCENARIOS'].items() if promise in owned]
                observations = [item for item in evidence['scenarios'] if item['host'] == host and item['scenario'] in cases]
                evidence['coverage'][host][promise].update(verified=len(observations) == len(cases) and all(
                    item['exit'] == 0 and item['coverage'].get(promise, {}).get('verified') is True for item in observations),
                    scenarios=observations)
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
