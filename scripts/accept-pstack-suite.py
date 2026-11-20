"""Accept pstack Suite through production Task/Chat HTTP and installed OMP in owned Herdr."""
import argparse
from pathlib import Path
import sys
import traceback

from acceptance.host.smoke_support import dump
from acceptance.suites.pstack import CASES, PstackAcceptance, configure


def absolute(value):
    path = Path(value)
    if not path.is_absolute():
        raise argparse.ArgumentTypeError(f'absolute path required: {value}')
    return path.resolve()


def main():
    parser = argparse.ArgumentParser(description=__doc__, epilog=(
        'See docs/pstack-suite-native-acceptance.md for the owned installation JSON, '
        'build and independent repository checks. Filtered diagnostics always exit nonzero.'))
    for flag in ('herdr-bot-worktree', 'toolkit-worktree', 'backend', 'isolated-config', 'evidence-dir'):
        parser.add_argument('--' + flag, type=absolute, required=True)
    parser.add_argument('--cases', default='all', help='all or comma-separated: ' + ', '.join(CASES))
    args = parser.parse_args()
    selected = list(CASES) if args.cases == 'all' else args.cases.split(',')
    if len(set(selected)) != len(selected) or not set(selected) <= CASES.keys():
        parser.error('invalid --cases selection; see --help')
    try:
        args.evidence_dir.mkdir(parents=True, exist_ok=False)
    except OSError as error:
        parser.error(f'new evidence directory required: {error}')
    cells = {case: {'status': 'unverified', 'selected': case in selected, 'coverage': coverage}
             for case, coverage in CASES.items()}
    report = {'passed': False, 'scope': 'native mechanics only; repository checks separate', 'cases': cells}
    try:
        configure(args, report)
        for case in selected:
            cell = cells[case]
            try:
                run = PstackAcceptance(args, case)
                cell['evidence'] = str(run.root / 'evidence.json')
                code = run.run()
                cell['status'] = ('unverified' if not run.prepared else 'failed') if code else 'passed'
            except Exception:
                cell['error'] = traceback.format_exc()
        report['passed'] = args.cases == 'all' and all(cell['status'] == 'passed' for cell in cells.values())
    except Exception:
        report['preflightError'] = traceback.format_exc()
    finally:
        summary = args.evidence_dir / 'summary.json'
        dump(summary, report)
        print(summary)
    return 0 if report['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
