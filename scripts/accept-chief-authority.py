"""P31: live old-chief revocation and reliable-report handoff on installed OMP/Pi.

Explicit absolute dependencies, exclusive evidence, GPT-6-luna only. No installs,
credential discovery, model fallback, business retries or claims of physical undo.
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
    evidence = {'passed': False, 'coverage': {host: {'P31': {'verified': False}} for host in ['omp', 'pi']},
                'cleanupErrors': [], 'selectedHosts': args.hosts.split(','),
                'scope': 'P31 only; not same-ID recovery/deduplication owned by report recovery acceptance',
                'modelPolicy': 'gpt-6-luna only; actual returned identity required; no retries'}
    try:
        def require(condition, detail):
            if not condition:
                raise AssertionError(detail)
        require(args.backend.is_file(), 'Missing current worktree backend: ' + str(args.backend))
        require(args.herdr_bot_worktree == REPO == Path.cwd().resolve(), 'Run from supplied herdr-bot worktree')
        require((args.toolkit_worktree / '.git').is_file(), 'Expected explicit toolkit Git worktree')
        require(args.model_config.is_file(), 'Missing authorized isolated model config')
        model = json.loads(args.model_config.read_text(encoding='utf-8'))
        require(model.get('model') == 'gpt-6-luna', 'Actual model must be gpt-6-luna; no fallback')
        require(model.get('apiKeyEnv') and os.environ.get(model['apiKeyEnv']), 'Missing explicitly authorized credential environment')
        args.repo, args.toolkit = args.herdr_bot_worktree, args.toolkit_worktree
        resources = runpy.run_path(str(REPO / 'scripts/accept-suite-pi-resources.py'))
        resources['preflight'](args, evidence)
        args.path_roots = evidence['paths']['roots']
        # The shared installed-host fixture validates both official state files,
        # even for a deliberately incomplete single-host diagnostic invocation.
        for host in ['omp', 'pi']:
            key = host + '_state_extension'
            require(key in args.path_roots and Path(args.path_roots[key]).is_file(), 'Missing readonly roots.' + key)
            setattr(args, key, Path(args.path_roots[key]).resolve())
        scenario = runpy.run_path(str(REPO / 'scripts/chief_authority_native_acceptance.py'))['ChiefAuthorityAcceptance']
        for host in args.hosts.split(','):
            selected = copy.copy(args)
            selected.host = host
            run = scenario(selected)
            status = run.run()
            evidence['coverage'][host]['P31'].update(run.evidence['coverage']['P31'], evidence=str(run.root / 'evidence.json'))
            evidence['cleanupErrors'].extend(run.evidence['cleanupErrors'])
            if status:
                evidence.setdefault('hostFailures', []).append(host)
                break  # Preserve the first failure; never retry a model/business decision.
        evidence['passed'] = all(item['P31']['verified'] for item in evidence['coverage'].values()) and not evidence['cleanupErrors'] and not evidence.get('hostFailures')
    except BaseException:
        evidence['error'] = traceback.format_exc()
    evidence['entrypoint'] = {'path': str(Path(__file__).resolve()), 'sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    report = args.evidence_dir / 'evidence.json'
    report.write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps({'passed': evidence['passed'], 'evidence': str(report)}, indent=2))
    return 0 if evidence['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
