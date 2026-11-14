"""Real-model Bot wrapping acceptance at public Chat/Run/HTTP/native-history seams.

The shipped Tasks retain their declared hosts. Explicit omp,pi runs both complete
homogeneous matrices. No controlled provider chooses business steps, and neither
an observer nor a node report supplies user authorization."""
import argparse
import json
from pathlib import Path
import sys

from developer_paths import REPO
from acceptance.workflow.create import CreateAcceptance, PROMISES
from acceptance.host.processes import require

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['backend', 'toolkit', 'paths-file', 'model-config', 'evidence-dir']:
        parser.add_argument('--' + name, type=lambda value: Path(value).resolve(), required=True)
    parser.add_argument('--hosts', choices=['declared', 'omp,pi'], required=True)
    for name in ['herdr', 'pi-package', 'omp-package', 'pi-state-extension', 'omp-state-extension',
                 'wayfinder', 'wayfinder-skills', 'pi-extensions', 'codebase-memory-mcp', 'shell-path']:
        parser.add_argument('--' + name, type=Path)
    parser.add_argument('--node', default='node')
    parser.add_argument('--bun', default='bun')
    parser.add_argument('--timeout', type=int, default=300)
    args = parser.parse_args()
    require(1 <= args.timeout <= 900, '--timeout must be 1..900 seconds')
    args.repo = REPO
    if args.hosts == 'declared':
        case = CreateAcceptance(args)
        case.parser = parser
        return case.run()
    args.evidence_dir.mkdir(parents=True, exist_ok=False)
    results = {'passed': False, 'requestedHosts': ['omp', 'pi'], 'verifiedHosts': [],
        'coverage': {key: {'verified': False} for key in PROMISES}, 'hosts': {}, 'cleanupErrors': []}
    for host in ['omp', 'pi']:
        native_args = argparse.Namespace(**vars(args))
        native_args.execution_host = host
        native_args.evidence_dir = args.evidence_dir / host
        case = CreateAcceptance(native_args)
        case.parser = parser
        code = case.run()
        results['hosts'][host] = {'exit': code, 'evidence': str(case.root / 'evidence.json')}
        results['cleanupErrors'].extend(case.evidence['cleanupErrors'])
        if code == 0:
            results['verifiedHosts'].append(host)
    results['passed'] = results['verifiedHosts'] == ['omp', 'pi'] and not results['cleanupErrors']
    for key in PROMISES:
        results['coverage'][key]['verified'] = results['passed']
    (args.evidence_dir / 'evidence.json').write_text(json.dumps(results, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps(results, ensure_ascii=False, indent=2))
    return 0 if results['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
