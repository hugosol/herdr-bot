"""Run existing workflow acceptance with installed Pi's real Codex completion.

The transport preserves real host history/tool execution. It is not evidence of
OMP/Pi using the Codex provider directly, nor a replacement for business checks.
OAuth stays inside the installed-provider subprocess, never in business config.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

from workflow_completion_transport import NativeCompletionTransport

REPO = Path(__file__).resolve().parents[1]
MODEL = 'gpt-6-luna'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--scenario', choices=['roles', 'revise', 'chief-task', 'reports'], required=True)
    parser.add_argument('--hosts', choices=['declared', 'omp,pi', 'omp', 'pi'], required=True)
    for name in ['backend', 'toolkit', 'paths-file', 'evidence-dir', 'pi-package', 'auth-file']:
        parser.add_argument('--' + name, type=lambda value: Path(value).resolve(), required=True)
    for name in ['herdr', 'omp-package', 'pi-state-extension', 'omp-state-extension', 'shell-path']:
        parser.add_argument('--' + name, type=Path)
    parser.add_argument('--node', default='node')
    parser.add_argument('--bun', default='bun')
    parser.add_argument('--timeout', type=int, default=300)
    parser.add_argument('--smoke', action='store_true', help='One real model request only; no business host')
    parser.add_argument('--cases', default='all', help='Revision case selection; a subset does not establish full acceptance')
    args = parser.parse_args()
    if args.scenario == 'chief-task' and args.hosts == 'declared':
        parser.error('chief-task requires explicit omp,pi or one host')
    if args.scenario == 'reports' and args.hosts == 'declared':
        parser.error('Reports requires explicit omp,pi or a diagnostic single host')
    if args.scenario == 'revise' and args.hosts in ['omp', 'pi']:
        parser.error('Single-host selection is only supported for the roles scenario')
    if args.cases != 'all' and (args.scenario != 'revise' or args.smoke):
        parser.error('--cases is only supported for revision business acceptance')
    if not 1 <= args.timeout <= 900:
        parser.error('--timeout must be 1..900')
    # Fail before creating any resources when a caller supplied missing inputs.
    for path in [args.pi_package / 'package.json', args.auth_file, args.backend, args.paths_file,
                 args.toolkit / 'package.json']:
        if not path.is_file():
            parser.error('Missing explicit dependency: ' + str(path))
    args.evidence_dir.mkdir(parents=True, exist_ok=False)
    adapter = REPO / 'scripts/workflow-codex-completion.mjs'
    result = {'passed': False, 'scenario': args.scenario, 'hosts': args.hosts,
              'model': {'provider': 'openai-codex', 'id': MODEL, 'api': 'openai-codex-responses'},
              'scope': 'Native Codex conversation and tool calling; business hosts execute the returned tools. Not direct Codex configuration inside those hosts.',
              'modelRequests': 0, 'started': time.time(), 'smoke': args.smoke,
              'caseSelection': args.cases, 'filtered': args.cases != 'all', 'subsetPassed': False}
    records = args.evidence_dir / 'codex-completions'
    records.mkdir()

    def complete(request):
        index = result['modelRequests']
        result['modelRequests'] += 1
        command = [args.node, str(adapter), '--pi-package', str(args.pi_package),
                   '--auth-file', str(args.auth_file), '--model', MODEL, '--timeout', str(args.timeout)]
        # Prompts and credentials are never command-line arguments. The adapter's
        # stdout protocol excludes tokens/headers/account identifiers.
        try:
            process = subprocess.run(command, input=json.dumps({'request': request}, ensure_ascii=False),
                                     capture_output=True, text=True, encoding='utf-8', errors='strict',
                                     timeout=args.timeout + 15, cwd=REPO)
        except subprocess.TimeoutExpired:
            raise RuntimeError('Codex completion timed out; no retry') from None
        if process.returncode:
            # Only accept a bounded code, never dependency stderr or arbitrary text.
            try:
                code = json.loads(process.stdout).get('error')
            except (ValueError, AttributeError):
                code = None
            allowed = {'invalid-arguments', 'invalid-input', 'auth-missing', 'oauth-expired',
                       'dependency-missing', 'model-unavailable', 'provider-error',
                       'model-identity', 'content-invalid', 'timeout'}
            reason = code if code in allowed else 'unclassified-native-error'
            diagnostic = {'error': reason}
            try:
                failed = json.loads(process.stdout)
                for key in ['httpStatus', 'requests']:
                    value = failed.get(key)
                    if type(value) is int and 0 <= value <= 999:
                        diagnostic[key] = value
                if failed.get('network') in ['dns', 'connect', 'timeout', 'tls', 'other']:
                    diagnostic['network'] = failed['network']
            except (ValueError, AttributeError):
                pass
            (records / f'{index:04d}-error.json').write_text(json.dumps(diagnostic), encoding='utf-8')
            raise RuntimeError('Native Codex completion failed: ' + reason + '; no retry')
        native = json.loads(process.stdout)
        if (native.get('provider'), native.get('model'), native.get('serverModel'), native.get('api')) != (
                'openai-codex', MODEL, MODEL, 'openai-codex-responses'):
            raise RuntimeError('Native completion model identity mismatch; no fallback')
        if not isinstance(native.get('content'), str) or not isinstance(native.get('tool_calls'), list):
            raise RuntimeError('Native completion returned an invalid decision; no retry')
        if not native['content'].strip() and not native['tool_calls']:
            raise RuntimeError('Native completion returned no text or tool calls; no retry')
        (records / f'{index:04d}.json').write_text(json.dumps(native, ensure_ascii=False, indent=2), encoding='utf-8')
        usage = native['usage']
        return {'content': native['content'], 'tool_calls': native['tool_calls'],
                'usage': {'prompt_tokens': usage['input'] + usage['cacheRead'] + usage['cacheWrite'],
                          'completion_tokens': usage['output'], 'total_tokens': usage['totalTokens'],
                          'prompt_tokens_details': {'cached_tokens': usage['cacheRead'],
                                                    'cache_write_tokens': usage['cacheWrite']}}}

    try:
        result['adapterSha256'] = hashlib.sha256(adapter.read_bytes()).hexdigest()
        result['wrapperSha256'] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
        result['transportSha256'] = hashlib.sha256((REPO / 'scripts/workflow_completion_transport.py').read_bytes()).hexdigest()
        result['installedPiPackage'] = str(args.pi_package)
        if args.smoke:
            prompt = 'Reply with exactly LUNA-ACCEPTANCE-READY and no other text.'
            (args.evidence_dir / 'smoke-prompt.txt').write_text(prompt, encoding='utf-8')
            decision = complete({'model': MODEL, 'messages': [{'role': 'user', 'content': prompt}], 'tools': []})
            result['response'] = decision['content']
            result['passed'] = result['response'].strip() == 'LUNA-ACCEPTANCE-READY' and not decision['tool_calls']
        else:
            with NativeCompletionTransport(complete, args.evidence_dir / 'completion-wire') as transport:
                with tempfile.TemporaryDirectory(prefix='workflow-codex-config-') as directory:
                    config = Path(directory) / 'model.json'
                    config.write_text(json.dumps({'provider': 'codex-completion-transport', 'model': MODEL,
                        'baseUrl': transport.base, 'apiKeyEnv': 'WORKFLOW_CODEX_TRANSPORT_KEY'}), encoding='utf-8')
                    # This is a loopback transport marker, not an OAuth/API credential.
                    env = dict(os.environ, WORKFLOW_CODEX_TRANSPORT_KEY='isolated-loopback-transport')
                    env['CARGO_TARGET_DIR'] = str(args.backend.parent.parent)
                    if args.scenario in ['chief-task', 'reports']:
                        script = 'accept-chief-task.py' if args.scenario == 'chief-task' else 'accept-chief-reports.py'
                        command = [sys.executable, str(REPO / 'scripts' / script),
                            '--herdr-bot-worktree', str(REPO), '--toolkit-worktree', str(args.toolkit),
                            '--backend', str(args.backend), '--hosts', args.hosts,
                            '--paths-file', str(args.paths_file), '--model-config', str(config),
                            '--evidence-dir', str(args.evidence_dir / 'business'),
                            '--node', args.node, '--bun', args.bun, '--timeout', str(args.timeout)]
                    else:
                        command = [sys.executable, str(REPO / ('scripts/accept-workflow-nesting-' + args.scenario + '.py')),
                            '--backend', str(args.backend), '--toolkit', str(args.toolkit), '--hosts', args.hosts,
                            '--paths-file', str(args.paths_file), '--model-config', str(config),
                            '--evidence-dir', str(args.evidence_dir / 'business'), '--pi-package', str(args.pi_package),
                            '--node', args.node, '--bun', args.bun, '--timeout', str(args.timeout)]
                    if args.scenario == 'revise':
                        command.extend(['--cases', args.cases])
                    for name in (['shell_path'] if args.scenario == 'chief-task' else
                                 ['herdr', 'omp_package', 'pi_state_extension', 'omp_state_extension', 'shell_path']):
                        value = getattr(args, name)
                        if value and (args.scenario != 'reports' or name == 'shell_path'):
                            command.extend(['--' + name.replace('_', '-'), str(value)])
                    result['command'] = command
                    # Business stdout/stderr are continuously drained to owned files.
                    with (args.evidence_dir / 'business-stdout.log').open('w', encoding='utf-8') as stdout, \
                         (args.evidence_dir / 'business-stderr.log').open('w', encoding='utf-8') as stderr:
                        process = subprocess.run(command, cwd=REPO, env=env, stdout=stdout, stderr=stderr)
                    result['businessExit'] = process.returncode
            result['transportFailed'] = transport.failed
            successful = result['businessExit'] == 0 and not transport.failed
            result['subsetPassed'] = successful and result['filtered']
            result['passed'] = successful and not result['filtered']
    except Exception as error:
        result['passed'] = False
        result['error'] = str(error)
    finally:
        result['ended'] = time.time()
        (args.evidence_dir / 'evidence.json').write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding='utf-8')
        print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0 if result['passed'] or result['subsetPassed'] else 1


if __name__ == '__main__':
    sys.exit(main())
