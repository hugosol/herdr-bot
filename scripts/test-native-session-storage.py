"""P23/P24/P25 S4 acceptance with installed, real Pi/OMP CLIs.

The loopback model is controlled; the agent launch, storage, and native discovery
are real. Never reads user auth/config or submits input to an existing session.
Retains the isolated root and prints JSON evidence; does not certify deployment H1.
"""
import argparse
import hashlib
import http.server
import json
import os
from pathlib import Path
import subprocess
import tempfile
import threading

from developer_paths import resolve_arguments


def digest(data):
    return hashlib.sha256(data).hexdigest()


def inventory(root):
    return {str(p.relative_to(root)): digest(p.read_bytes()) if p.is_file() else None
            for p in sorted(root.rglob('*'))}


class Model(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        self.rfile.read(int(self.headers.get('Content-Length', '0')))
        self.send_response(200)
        self.send_header('Content-Type', 'text/event-stream')
        self.end_headers()
        for delta, finish in [({'role': 'assistant', 'content': 'isolated storage acknowledgement'}, None), ({}, 'stop')]:
            chunk = {'id': 'isolated', 'object': 'chat.completion.chunk', 'created': 1,
                     'model': 'storage-probe', 'choices': [{'index': 0, 'delta': delta, 'finish_reason': finish}]}
            self.wfile.write(('data: ' + json.dumps(chunk) + '\n\n').encode())
        self.wfile.write(b'data: [DONE]\n\n')

    def log_message(self, *_):
        pass


def run(command, cwd, env):
    result = subprocess.run(command, cwd=cwd, env=env, capture_output=True, text=True, timeout=90)
    assert result.returncode == 0, (command, result.returncode, result.stdout, result.stderr)
    return {'argv': command, 'exit': result.returncode, 'stdout': result.stdout, 'stderr': result.stderr}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--paths-file', type=Path, help='Machine roots TOML (default: repository paths.local.toml)')
    parser.add_argument('--pi-package', type=Path)
    parser.add_argument('--omp-package', type=Path)
    parser.add_argument('--bun', default='bun')
    parser.add_argument('--node', default='node')
    args = parser.parse_args()
    resolve_arguments(parser, args, {
        'pi_package': ('pi_package', ''),
        'omp_package': ('omp_package', ''),
    })
    root = Path(tempfile.mkdtemp(prefix='herdr-native-storage-'))
    server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Model)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    evidence = {'root': str(root), 'seam': 'S4 isolated real CLI + native SessionManager.list; NOT deployment H1', 'agents': {}}
    try:
        for kind, package in [('pi', args.pi_package), ('omp', args.omp_package)]:
            package = package.resolve()
            base = root / kind
            zone, agent, home = base / 'zone', base / 'configured-agent', base / 'home'
            for directory in [zone, agent, home]:
                directory.mkdir(parents=True)
            sessions = base / 'nondefault-native-sessions' if kind == 'pi' else agent / 'sessions'
            env = {k: v for k, v in os.environ.items() if not any(s in k.upper() for s in
                   ['API_KEY', 'TOKEN', 'PI_', 'OMP_', 'ANTHROPIC', 'OPENAI', 'AZURE', 'XDG_'])}
            env.update(HOME=str(home), USERPROFILE=str(home), PI_CODING_AGENT_DIR=str(agent), NO_COLOR='1')
            config = agent / ('settings.json' if kind == 'pi' else 'config.yml')
            config.write_text(json.dumps({'sessionDir': str(sessions)}) + '\n' if kind == 'pi' else 'autoResume: false\n')
            model = {'providers': {'storage-local': {
                'baseUrl': f'http://127.0.0.1:{server.server_port}/v1', 'api': 'openai-completions',
                'apiKey': 'isolated-not-a-real-key', 'models': [{'id': 'storage-probe', 'name': 'storage probe',
                'reasoning': False, 'input': ['text'], 'contextWindow': 32768, 'maxTokens': 1000,
                'cost': {'input': 0, 'output': 0, 'cacheRead': 0, 'cacheWrite': 0}}]}}}
            # JSON is valid YAML; use OMP's current filename, not its migration input.
            models = agent / ('models.json' if kind == 'pi' else 'models.yml')
            models.write_text(json.dumps(model) + '\n')
            configs_before = {str(p): p.read_bytes() for p in [config, models]}
            cli = package / ('dist/bundle/cli.js' if kind == 'pi' else 'dist/cli.js')
            command = [args.node if kind == 'pi' else args.bun, str(cli), '--print', '--model',
                       'storage-local/storage-probe', '--no-extensions', 'Acknowledge this isolated storage test.']
            first = run(command, zone, env)
            assert 'isolated storage acknowledgement' in first['stdout']
            old_files = list(sessions.rglob('*.jsonl'))
            assert len(old_files) == 1, old_files
            # Seed attached records before the next normal launch. Never resume the old file.
            sidecar = old_files[0].with_suffix('.records')
            (sidecar / 'empty').mkdir(parents=True)
            (sidecar / 'tool.bin').write_bytes(b'\0old-sidecar\xff\r\n')
            old_before = inventory(sessions)
            second = run(command, zone, env)
            assert 'isolated storage acknowledgement' in second['stdout']
            after = inventory(sessions)
            for path, expected in old_before.items():
                assert after.get(path, 'MISSING') == expected, ('old native file changed', path)
            added = sorted(set(after) - set(old_before))
            assert len(added) == 1 and added[0].endswith('.jsonl'), added
            actual = sessions / added[0]
            entries = [json.loads(line) for line in actual.read_text(encoding='utf-8').splitlines()]
            header = next(entry for entry in entries if entry['type'] == 'session')
            assert header['cwd'] == str(zone)
            assert any(e.get('message', {}).get('role') == 'assistant' for e in entries)
            # Same installed native listing implementation used by the session pickers.
            module = package / ('dist/core/session-manager.js' if kind == 'pi' else 'src/session/session-manager.ts')
            imports = f'import {{ SessionManager }} from {json.dumps(module.as_posix())}; '
            lookup_args = json.dumps(str(zone))
            if kind == 'pi':
                settings = package / 'dist/core/settings-manager.js'
                imports += f'import {{ SettingsManager }} from {json.dumps(settings.as_posix())}; '
                lookup_args += ', SettingsManager.create(process.cwd()).getSessionDir()'
            expression = imports + f'console.log(JSON.stringify(await SessionManager.list({lookup_args})));'
            discovery = run([args.bun, '-e', expression], zone, env)
            listed = json.loads(discovery['stdout'])
            assert any(item['id'] == header['id'] and Path(item['path']) == actual for item in listed), listed
            configs_after = {str(p): p.read_bytes() for p in [config, models]}
            assert configs_before == configs_after, 'effective settings/model configuration changed'
            evidence['agents'][kind] = {
                'version': json.loads((package / 'package.json').read_text())['version'],
                'effectiveConfiguration': {'PI_CODING_AGENT_DIR': str(agent), 'settings': config.read_text(),
                                           'storageRoot': str(sessions), 'settingsSha256': digest(config.read_bytes())},
                'configBefore': {p: digest(data) for p, data in configs_before.items()},
                'configAfter': {p: digest(data) for p, data in configs_after.items()},
                'seedRun': first, 'observedRun': second, 'nativeReference': str(actual), 'header': header,
                'oldInventoryBefore': old_before, 'oldInventoryAfter': {p: after[p] for p in old_before},
                'addedNativeEntries': added, 'nativeDiscovery': discovery,
            }
    finally:
        server.shutdown()
    print(json.dumps(evidence, ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()
