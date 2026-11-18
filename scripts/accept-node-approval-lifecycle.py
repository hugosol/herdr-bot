"""Ticket 12: actual native identity, in-flight approval, recovery and nesting.

Explicit installations/authorization only; never installs, discovers credentials,
substitutes a model decision, edits a Run, or retries a failed business decision.
"""
import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import threading
import traceback
import xml.etree.ElementTree as ET

REPO = Path(__file__).resolve().parents[1]
CASE_NAMES = ('persistence', 'nested-auto', 'nested-manual', 'lifecycle')


def selected_cases(value):
    if value == 'all':
        return list(CASE_NAMES)
    selected = value.split(',')
    if not selected or any(case not in CASE_NAMES for case in selected) or len(set(selected)) != len(selected):
        raise argparse.ArgumentTypeError('Expected all or distinct comma-separated persistence,nested-auto,nested-manual,lifecycle')
    return selected


EXPECTATIONS = {
    'P3': 'Recovered execution consumes frozen manual mode; legal retry consumes changed auto definition.',
    'P10': 'Ordinary node, independent Chat, root manager, unrelated identity, other Run/node, retired execution and nested dual role never exchange approval authority; approve has no identity arguments.',
    'P11': 'Real old native Yes returns after public fail/retry while the actual new worker is deterministically gated before its first business model request; no new approval/report/complete/dispatch/notification occurs during 51s. Only then release real initial model work, retaining its separate provenance.',
    'P14': 'Actual native and backend processes exit; new processes preserve Chat/execution/approval/mode without replaying business input or reclaiming old Run.',
    'P16': 'Real file modification and another report preserve prior approval; no digest matching contract.',
    'P24': 'Queries, report and independent branch finish within bounded budgets while real native confirmation remains pending.',
    'P28': 'Actual child Run finishes; dual-role manager separately approves the parent in both frozen modes; native Yes for manual; parent manager explicitly completes after report gates.',
}


def absolute(value):
    path = Path(value)
    if not path.is_absolute():
        raise argparse.ArgumentTypeError('Expected an explicit absolute path')
    return path.resolve()


def run_check(argv, cwd, environment, log, record, args):
    """Observe the existing runner's ephemeral JUnit without changing its code.

    A private temp root isolates this invocation's reporter files. The observer
    only copies complete XML; it never changes requests, results or teardown.
    """
    is_full_toolkit = cwd == args.toolkit and 'run' in argv and 'test' in argv
    if not is_full_toolkit:
        with log.open('w', encoding='utf-8') as stream:
            return subprocess.run(argv, cwd=cwd, env=environment, stdout=stream, stderr=subprocess.STDOUT).returncode
    expected = []
    for directory, dirs, files in os.walk(cwd):
        dirs[:] = [name for name in dirs if not name.startswith('.') and name != 'node_modules']
        for name in files:
            if re.search(r'[._](test|spec)\.(?:[cm]?[jt]s|[jt]sx)$', name):
                expected.append((Path(directory) / name).relative_to(cwd).as_posix())
    record.update(expectedTestFiles=sorted(expected), selection='Entire discovered repository; appended --parallel only changes scheduling, not selection')
    retained = args.evidence_dir / 'toolkit-full-junit.xml'
    finished = threading.Event()
    observation = {}
    with tempfile.TemporaryDirectory(prefix='l12-junit-') as temp:
        environment = dict(environment, TEMP=temp, TMP=temp, TMPDIR=temp)
        def archive():
            while not finished.is_set():
                for candidate in Path(temp).glob('omp-test-report-*/results.xml'):
                    try:
                        raw = candidate.read_bytes()
                        ET.fromstring(raw)
                        retained.write_bytes(raw)
                        observation['source'] = str(candidate)
                    except (OSError, ET.ParseError):
                        pass  # Reporter may still be writing; no test outcome is inferred.
                finished.wait(0.002)
        observer = threading.Thread(target=archive, name='owned-junit-archive')
        observer.start()
        try:
            with log.open('w', encoding='utf-8') as stream:
                status = subprocess.run(argv, cwd=cwd, env=environment,
                    stdout=stream, stderr=subprocess.STDOUT).returncode
        finally:
            finished.set()
            observer.join()
    record['junit'] = {'path': str(retained), **observation}
    if not retained.is_file():
        record['junit']['error'] = 'Original full-suite JUnit was not retained; no complete coverage claim'
        return status or 1
    xml = ET.fromstring(retained.read_bytes())
    actual = set()
    for element in xml.iter():
        for key in ['file', 'name'] if element.tag in ['testsuite', 'testsuites'] else ['file']:
            value = element.get(key)
            if value:
                candidate = Path(value)
                if candidate.is_absolute():
                    try:
                        candidate = candidate.relative_to(cwd)
                    except ValueError:
                        continue
                actual.add(candidate.as_posix().removeprefix('./'))
    missing = sorted(set(expected) - actual)
    cases = list(xml.iter('testcase'))
    record['junit'].update(sha256=hashlib.sha256(retained.read_bytes()).hexdigest(),
        actualTestFiles=sorted(set(expected) & actual), missingTestFiles=missing,
        testCases=[{'file': case.get('file'), 'name': case.get('name'), 'classname': case.get('classname'),
                    'outcomes': [child.tag for child in case]} for case in cases],
        allExpectedFilesObserved=not missing, testCaseCount=len(cases))
    return status if status or (not missing and cases) else 1


def checks(args, evidence):
    target = args.backend.parent.parent
    safe = args.backend.parent / 'examples/safe_shutdown_shell.exe'
    capacity = json.loads(subprocess.run([args.node, '--input-type=module', '-e',
        'import os from "node:os"; console.log(JSON.stringify({cpu:os.availableParallelism(),freeBytes:os.freemem()}))'],
        capture_output=True, text=True, check=True).stdout)
    # Installed-host files can own a pair of hosts/backend. Reserve 2 GiB per
    # concurrently running native test file, rather than a fixed worker count.
    workers = max(1, min(capacity['cpu'], capacity['freeBytes'] // (2 * 1024 ** 3)))
    evidence['verificationCapacity'] = {**capacity, 'workers': workers, 'bytesPerNativeWorker': 2 * 1024 ** 3,
        'scopePublication': 'Full tests run with resource-derived parallelism; no claim of canonical OMP scope promotion.'}
    environment = dict(os.environ, CARGO_TARGET_DIR=str(target), HERDR_BOT_REPO=str(args.repo),
        HERDR_BOT_METADATA=str(args.repo / 'meta-data/task-options.toml'), HERDR_BOT_BINARY=str(safe),
        OMP_BASELINE_RECORD_ROOT=os.environ.get('OMP_BASELINE_RECORD_ROOT', 'E:/Developer/myhub/my-agent-toolkit'),
        PI_NODE=args.node, PI_CLI=str(args.pi_package / 'dist/bundle/cli.js'),
        OMP_CLI=str(args.omp_package / 'dist/cli.js'),
        WORKFLOW_DUAL_EVIDENCE_DIR=str(args.evidence_dir / 'nested-dock'),
        WORKFLOW_PROMPT_EVIDENCE_DIR=str(args.evidence_dir / 'node-dock'),
        NODE_APPROVAL_LIFECYCLE_FIXTURE_TOOLKIT=str(args.toolkit),
        NODE_APPROVAL_LIFECYCLE_FIXTURE_BACKEND=str(args.backend),
        NODE_APPROVAL_LIFECYCLE_FIXTURE_PATHS=str(args.paths_file),
        NODE_APPROVAL_LIFECYCLE_FIXTURE_EVIDENCE=str(args.evidence_dir / 'fixture-native-collect'))
    commands = [(args.repo, ['npm', '--prefix', 'frontend', 'run', 'build']),
        (args.repo, ['cargo', 'build', '--manifest-path', 'backend/Cargo.toml', '--bin', 'herdr-bot',
                    '--bin', 'herdr-fixture', '--bin', 'herdr-paths', '--example', 'safe_shutdown_shell', '--example', 'recovery_herdr_proxy']),
        (args.repo, ['node', 'scripts/test-backend.mjs', '--binary-jobs', str(workers)]),
        (args.repo, ['npm', '--prefix', 'frontend', 'test', '--', '--maxWorkers=' + str(workers)]),
        (args.repo, ['npm', '--prefix', 'frontend', 'run', 'test:browser:build', '--', '--workers=' + str(workers)]),
        (args.toolkit, [args.bun, 'run', 'typecheck']), (args.toolkit, [args.bun, 'run', 'test', '--parallel=' + str(workers)]),
        (args.toolkit, [args.bun, 'run', 'check:integrations']),
        (args.toolkit, [args.bun, 'test', '--parallel=' + str(workers), 'tests/harness-dock/native-node-report.test.ts',
                        'tests/harness-dock/node-report-contract.test.ts', 'tests/harness-dock/nested-role-completion.test.ts']),
        (args.repo, [sys.executable, 'scripts/accept-node-approval-lifecycle.test.py'])]
    previous = None
    if args.checks_from:
        from acceptance.host.provenance import source_identity
        from acceptance.host.processes import require
        previous = json.loads(args.checks_from.read_text(encoding='utf-8'))
        require(len(previous.get('checks', [])) == len(commands)
            and all(record.get('exit') == 0 for record in previous['checks']),
            'Only a complete successful repository-check phase can be reused')
        require(previous.get('binary', {}).get('sha256') == evidence['binary']['sha256']
            and previous.get('frontend') == evidence['frontend']
            and previous.get('launchers') == evidence['launchers']
            and previous.get('host') == evidence['host']
            and previous.get('herdrVersion') == evidence['herdrVersion']
            and previous.get('paths') == evidence['paths'],
            'Repository-check artifacts, installations or isolated paths changed')
        # These files only drive this ticket's Python acceptance. Its final
        # check is always rerun; the other nine commands do not execute them.
        acceptance_only = {'scripts/accept-node-approval-lifecycle.py',
            'scripts/accept-node-approval-lifecycle.test.py', 'scripts/node_approval_lifecycle_acceptance.py'}
        reporting_only = {'docs/agents/testing.md', 'framework/docs/pi-acceptance.json'}
        for repository, root, lock in [('herdr-bot', args.repo, 'backend/Cargo.lock'),
                                       ('toolkit', args.toolkit, 'bun.lock')]:
            before = previous['nativeSources'][repository]['fileSha256']
            after = previous['sourcesAfter'][repository]['fileSha256']
            require(before == after, 'Source changed during the original acceptance')
            current = source_identity(root, lock, args.evidence_dir)['fileSha256']
            changed = {name for name in current.keys() | after.keys() if current.get(name) != after.get(name)}
            allowed = reporting_only | (acceptance_only if repository == 'herdr-bot' else set())
            require(not changed - allowed, ('Repository-check inputs changed', repository, sorted(changed - allowed)))
        evidence['checksReuse'] = {'source': str(args.checks_from),
            'sha256': hashlib.sha256(args.checks_from.read_bytes()).hexdigest(),
            'originalPassed': previous.get('passed'), 'rerun': str(args.repo / 'scripts/accept-node-approval-lifecycle.test.py')}

    def check_selection(argv):
        selected = []
        skip_count = False
        for value in argv:
            if skip_count:
                skip_count = False
            elif value == '--binary-jobs':
                skip_count = True
            elif not value.startswith(('--maxWorkers=', '--workers=', '--parallel=')):
                selected.append(value)
        return selected

    for index, (cwd, argv) in enumerate(commands):
        argv = [shutil.which(argv[0]) or argv[0], *argv[1:]]
        if previous is not None and index < len(commands) - 1:
            original = previous['checks'][index]
            require(Path(original['cwd']).resolve() == cwd
                and check_selection(original['argv']) == check_selection(argv),
                ('Repository check selection changed', index))
            evidence['checks'].append({**original, 'reusedFrom': str(args.checks_from)})
            continue
        log = args.evidence_dir / f'check-{index:02d}.log'
        record = {'cwd': str(cwd), 'argv': argv, 'log': str(log), 'CARGO_TARGET_DIR': str(target),
                  'HERDR_BOT_BINARY': str(safe), 'OMP_BASELINE_RECORD_ROOT': environment['OMP_BASELINE_RECORD_ROOT']}
        try:
            if cwd == args.toolkit and 'test' in argv and not safe.is_file():
                raise RuntimeError('Missing current safe shutdown shell; never use production for shutdown dock')
            record['exit'] = run_check(argv, cwd, environment, log, record, args)
        except (OSError, RuntimeError):
            record.update(exit=None, error=traceback.format_exc())
        evidence['checks'].append(record)
    return all(record.get('exit') == 0 for record in evidence['checks'])


def combine_native(args, evidence):
    """Compose completed case evidence, never turn an old overall failure green."""
    from acceptance.host.processes import require
    from node_approval_lifecycle_acceptance import COVERAGE_PARTS
    current = evidence['nativeSources']
    covered = set()
    evidence['combinedInputs'] = []
    evidence['coverageParts'] = {host: {key: {} for key in COVERAGE_PARTS} for host in ['omp', 'pi']}
    for path in args.combine:
        original = json.loads(path.read_text(encoding='utf-8'))
        boundary = original.get('modelTransportBoundary')
        require(not original.get('cleanupErrors'), ('Cannot compose failed cleanup', str(path)))
        require(not boundary or boundary.get('host') in ['omp', 'pi'],
                ('Cannot attribute original model transport failure', str(path)))
        require(original.get('nativeSources') and original.get('sourcesAfter'),
                ('Missing original before/after source provenance', str(path)))
        for repository in ['herdr-bot', 'toolkit']:
            before = original['nativeSources'][repository]['fileSha256']
            after = original['sourcesAfter'][repository]['fileSha256']
            require(before == after, ('Source changed during original native run', str(path), repository))
            # These two reporting files are not loaded as role/prompt material.
            # Other Markdown, including runtime instructions and skills, is code input.
            applicable = lambda hashes: {name: sha for name, sha in hashes.items()
                if name not in {'docs/agents/testing.md', 'framework/docs/pi-acceptance.json'}}
            actual = applicable(current[repository]['fileSha256'])
            expected = applicable(after)
            differences = {name for name in actual.keys() | expected.keys() if actual.get(name) != expected.get(name)}
            # Reuse only with a hash-bound original source companion and AST
            # equality outside methods not exercised by the selected cases.
            # Defaults, annotations and decorators remain compared. No allowlist of
            # arbitrary edited lines or user-supplied "verified" flags.
            import ast
            original_cases = {name for result in original.get('caseResults', {}).values()
                              for name in result.get('cases', {})}
            exclusions = {'scripts/accept-node-approval-lifecycle.py': ('entrypoint-source.py', ('combine_native',))}
            unexecuted = []
            if not original_cases.intersection({'nested-auto', 'nested-manual'}):
                unexecuted.append('nested')
            if 'persistence' not in original_cases:
                unexecuted.extend(('persistence', 'material_change'))
            if unexecuted:
                exclusions['scripts/node_approval_lifecycle_acceptance.py'] = ('scenario-source.py', tuple(unexecuted))
            for name in list(differences):
                if repository != 'herdr-bot' or name not in exclusions:
                    continue
                companion, excluded = exclusions[name]
                snapshot = path.parent / companion
                if not snapshot.is_file() or hashlib.sha256(snapshot.read_bytes()).hexdigest() != expected.get(name):
                    continue
                def executable_without(text):
                    tree = ast.parse(text)
                    if name == 'scripts/node_approval_lifecycle_acceptance.py':
                        owner = next(node for node in tree.body if isinstance(node, ast.ClassDef)
                                     and node.name == 'ApprovalLifecycleAcceptance')
                    else:
                        owner = tree
                    for method in excluded:
                        functions = [node for node in owner.body
                            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and node.name == method]
                        require(len(functions) == 1, ('Missing unique unexecuted function', method))
                        # Decorators, defaults and annotations still execute at definition time.
                        functions[0].body = [ast.Pass()]
                    return ast.dump(tree, include_attributes=False)
                require(executable_without(snapshot.read_text(encoding='utf-8')) ==
                        executable_without((args.repo / name).read_text(encoding='utf-8')),
                        ('Previously exercised code changed outside the unrelated partition', name))
                evidence.setdefault('equivalentSources', []).append({'originalEvidence': str(path), 'file': name,
                    'originalSha256': expected[name], 'currentSha256': actual[name],
                    'snapshot': str(snapshot), 'excludedUnexecutedFunction': excluded})
                differences.remove(name)
            require(not differences, ('Native evidence belongs to different executable sources',
                                      str(path), repository, sorted(differences)))
        require(original.get('binary', {}).get('sha256') == evidence['binary']['sha256']
                and original.get('frontend') == evidence['frontend'],
                ('Native evidence artifacts differ from current worktree', str(path)))
        input_record = {'path': str(path), 'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
                        'originalPassed': original.get('passed'), 'acceptedCases': []}
        evidence['combinedInputs'].append(input_record)
        for host, host_result in original.get('caseResults', {}).items():
            require(host in ['omp', 'pi'], ('Unknown native host', host))
            if boundary and host == boundary['host']:
                input_record.setdefault('excludedHosts', {})[host] = boundary
                continue  # Never reuse the failed host; earlier completed hosts retain their own proof.
            raw_path = Path(host_result['evidence'])
            raw = json.loads(raw_path.read_text(encoding='utf-8'))
            require(raw.get('scenario') == 'node-approval-lifecycle' and raw.get('host') == host
                    and not raw.get('cleanupErrors') and raw.get('processExitConfirmations'),
                    ('Missing actual native scenario or process cleanup', str(raw_path)))
            installed = raw['installedHosts'][host]
            package = getattr(args, host + '_package')
            entry = package / ('dist/cli.js' if host == 'omp' else 'dist/bundle/cli.js')
            require(installed['package'] == evidence['launchers'][host]['version']
                    and installed['entrySha256'] == hashlib.sha256(entry.read_bytes()).hexdigest(),
                    ('Installed host changed since the native evidence', host, str(raw_path)))
            require(raw.get('modelIdentities') and all(item.get('returned')
                    and set(item['returned']) == {item['requested']} for item in raw['modelIdentities']),
                    ('Missing actual matching model identities', str(raw_path)))
            require(host_result.get('cases') == raw.get('caseResults'),
                    ('Case index differs from original raw evidence', str(raw_path)))
            reference = {'entrypointEvidence': str(path), 'rawEvidence': str(raw_path),
                         'rawSha256': hashlib.sha256(raw_path.read_bytes()).hexdigest()}
            for name, case in raw['caseResults'].items():
                require(name in CASE_NAMES, ('Unknown lifecycle case', name))
                if not case.get('verified'):
                    continue
                key = host + '/' + name
                require(key not in covered, ('Ambiguous duplicate successful case', key))
                covered.add(key)
                input_record['acceptedCases'].append(key)
                evidence['caseResults'].setdefault(host, {})[name] = {**case, **reference}
                for promise, fact in case['coverage'].items():
                    if promise not in COVERAGE_PARTS and fact.get('verified'):
                        evidence['coverage'][host][promise] = {**fact, **reference}
                for promise, parts in case['coverageParts'].items():
                    for part, fact in parts.items():
                        if fact.get('verified'):
                            evidence['coverageParts'][host][promise][part] = {**fact, **reference}
    for host, promises in evidence['coverageParts'].items():
        for promise, parts in promises.items():
            evidence['coverage'][host][promise] = {'verified': all(parts.get(name, {}).get('verified')
                for name in COVERAGE_PARTS[promise]), 'parts': parts}
    evidence['selectedCases'] = sorted(covered)
    evidence['missingCases'] = [host + '/' + name for host in ['omp', 'pi'] for name in CASE_NAMES
                                if host + '/' + name not in covered]
    evidence['selectedCasesPassed'] = bool(covered)
    return not evidence['missingCases'] and all(item['verified']
        for host in evidence['coverage'].values() for item in host.values())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['herdr-bot-worktree', 'toolkit-worktree', 'backend', 'paths-file', 'evidence-dir']:
        parser.add_argument('--' + name, type=absolute, required=True)
    parser.add_argument('--model-config', type=absolute)
    parser.add_argument('--combine', type=absolute, action='append', default=[],
                        help='Compose original entrypoint evidence; repeat per source. Never sends model requests.')
    parser.add_argument('--hosts', choices=['omp,pi', 'omp', 'pi'], required=True)
    parser.add_argument('--timeout', type=int, choices=range(1, 601), default=180)
    parser.add_argument('--node', default='node')
    parser.add_argument('--bun', default='bun')
    parser.add_argument('--shell-path', type=absolute)
    parser.add_argument('--native-only', action='store_true', help='Diagnostic only; cannot certify ticket checks')
    parser.add_argument('--checks-from', type=absolute,
        help='With --combine only: reuse a complete successful repository-check phase when product inputs are unchanged; rerun lifecycle facility tests')
    parser.add_argument('--cases', type=selected_cases, default=list(CASE_NAMES),
                        help='all or distinct independent case names; a partial matrix always exits nonzero')
    args = parser.parse_args()
    args.evidence_dir.mkdir(parents=True, exist_ok=False)
    evidence = {'passed': False, 'coverage': {host: {key: {'verified': False, 'expected': value}
        for key, value in EXPECTATIONS.items()} for host in ['omp', 'pi']},
        'selectedHosts': args.hosts.split(','), 'nativeExits': {}, 'checks': [], 'cleanupErrors': [],
        'diagnosticNativeOnly': args.native_only, 'caseResults': {}, 'selectedCasesPassed': False,
        'selectedCases': [host + '/' + case for host in args.hosts.split(',') for case in args.cases],
        'missingCases': [host + '/' + case for host in ['omp', 'pi'] for case in CASE_NAMES
                         if host not in args.hosts.split(',') or case not in args.cases]}
    try:
        from acceptance.host.processes import require
        require(not args.checks_from or (args.combine and not args.native_only and args.checks_from.is_file()),
                '--checks-from requires an existing original report, --combine and full certification mode')
        if args.combine:
            require(args.hosts == 'omp,pi' and args.cases == list(CASE_NAMES),
                    'Composition retains the full dual-host matrix; do not filter hosts/cases')
            require(len(set(args.combine)) == len(args.combine) and all(path.is_file() for path in args.combine),
                    'Composition requires distinct existing original evidence files')
        else:
            require(args.model_config and args.model_config.is_file(), 'Missing explicit authorized model configuration; no credential discovery')
            config = json.loads(args.model_config.read_text(encoding='utf-8'))
            for key in ['provider', 'model', 'baseUrl', 'apiKeyEnv']:
                require(isinstance(config.get(key), str) and config[key], 'Authorized real model configuration requires ' + key)
            require(os.environ.get(config['apiKeyEnv']), 'Missing explicitly authorized credential environment: ' + config['apiKeyEnv'])
        args.repo, args.toolkit = args.herdr_bot_worktree, args.toolkit_worktree
        from acceptance.host.provenance import preflight, source_identity
        preflight(args, evidence)
        args.path_roots = evidence['paths']['roots']
        for host in ['omp', 'pi']:
            key = host + '_state_extension'
            require(key in args.path_roots, 'Missing readonly installation root ' + key)
            setattr(args, key, Path(args.path_roots[key]).resolve())
        require(subprocess.run([args.bun, '--version'], capture_output=True, text=True, check=True).stdout.strip() == '1.4.4',
                'Use existing Bun 1.4.4; no installation or upgrade')
        check_passed = checks(args, evidence) if not args.native_only else False
        evidence['nativeSources'] = {name: source_identity(root, lock, args.evidence_dir)
            for name, root, lock in [('herdr-bot', args.repo, 'backend/Cargo.lock'), ('toolkit', args.toolkit, 'bun.lock')]}
        from node_approval_lifecycle_acceptance import ApprovalLifecycleAcceptance
        # Canonical Codex adapter has one actual completion slot. All resources
        # remain exclusive; sequential hosts do not imply a product concurrency limit.
        combined = combine_native(args, evidence) if args.combine else False
        for host in ([] if args.combine else args.hosts.split(',')):
            scenario_args = copy.copy(args)
            scenario_args.host, scenario_args.condition = host, 'lifecycle'
            run = ApprovalLifecycleAcceptance(scenario_args)
            status = run.run()
            evidence['nativeExits'][host] = status
            evidence['caseResults'][host] = {'selectedCasesPassed': run.evidence['selectedCasesPassed'],
                'cases': run.evidence['caseResults'], 'nestedModes': run.evidence['nestedModes'],
                'evidence': str(run.root / 'evidence.json')}
            for key, item in run.evidence['coverage'].items():
                evidence['coverage'][host][key].update(item, rawEvidence=str(run.root / 'evidence.json'))
            evidence['cleanupErrors'].extend(run.evidence['cleanupErrors'])
            completions = [record for record in (run.model_wire.records if run.model_wire else [])
                if record.get('method') == 'POST' and record['path'].split('?', 1)[0].rstrip('/').endswith('/chat/completions')]
            failed = [record for record in completions if record.get('ended')
                and (record.get('status') != 200 or record.get('error'))]
            pending = [record for record in completions if not record.get('ended')]
            if failed or pending:
                evidence['modelTransportBoundary'] = {'host': host, 'failedRequests': [record['index'] for record in failed],
                    'unfinishedRequests': [record['index'] for record in pending],
                    'reason': 'Remaining hosts unverified; completed failures are terminal, unfinished original requests are not classified as provider failures. No decision retry.'}
                break
        evidence['sourcesAfter'] = {name: source_identity(root, lock, args.evidence_dir)
            for name, root, lock in [('herdr-bot', args.repo, 'backend/Cargo.lock'), ('toolkit', args.toolkit, 'bun.lock')]}
        require(all(evidence['sourcesAfter'][name]['fileSha256'] == evidence['nativeSources'][name]['fileSha256']
            for name in ['herdr-bot', 'toolkit']), 'Source changed during native acceptance')
        if args.combine:
            evidence['passed'] = check_passed and combined and not evidence['cleanupErrors']
        else:
            evidence['selectedCasesPassed'] = len(evidence['caseResults']) == len(args.hosts.split(',')) \
                and all(item['selectedCasesPassed'] for item in evidence['caseResults'].values()) and not evidence['cleanupErrors']
            evidence['passed'] = check_passed and args.hosts == 'omp,pi' and not evidence['missingCases'] and not evidence['cleanupErrors'] \
                and all(status == 0 for status in evidence['nativeExits'].values()) \
                and all(item['verified'] for host in evidence['coverage'].values() for item in host.values())
    except BaseException:
        evidence['error'] = traceback.format_exc()
    evidence['entrypoint'] = {'path': str(Path(__file__).resolve()),
        'sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    report = args.evidence_dir / 'evidence.json'
    report.write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps({'passed': evidence['passed'], 'evidence': str(report)}, indent=2))
    return 0 if evidence['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
