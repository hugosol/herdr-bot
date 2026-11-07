"""Real-model Bot wrapping acceptance at public Chat/Run/HTTP/native-history seams.

The shipped Tasks retain their declared hosts. Explicit omp,pi runs both complete
homogeneous matrices. No controlled provider chooses business steps, and neither
an observer nor a node report supplies user authorization.
"""
import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import runpy
import shutil
import subprocess
import sys
import tempfile
import time
import traceback

from developer_paths import REPO, backend_suffix, resolve_arguments

shared = runpy.run_path(str(Path(__file__).with_name('test-workflow-file-handoff.py')))
Acceptance, RolesAcceptance = shared['Acceptance'], shared['RolesAcceptance']
WireProxy, require, until = shared['WireProxy'], shared['require'], shared['until']
PROMISES = ['P39', 'P40', 'P41', 'P43', 'P44', 'P45']
TASK_IDS = ['task-18e33fdc55177d1c-1', 'task-18e3404478db1ef8-2',
    'task-18e340545eb481bc-3', 'task-18e340f5287462ec-4',
    'task-18e31d34797f1464-3', 'task-18e3415003d8cd00-5']
FIELDS = ['when', 'plan_guide', 'node_guide']


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


class CreateAcceptance(Acceptance):
    def __init__(self, args):
        super().__init__(args, prefix='bot-create-')
        self.evidence.update(promise=None, promises=PROMISES, scenario='workflow-nesting-create',
            requestedHosts=args.hosts, verifiedHosts=[], coverage={p: {'verified': False} for p in PROMISES},
            taskPairs={key: {'standalone': False, 'workflow': False} for key in TASK_IDS},
            interventions=[], cases=[], nativeProcesses=[])
        self.host = getattr(args, 'execution_host', 'declared')
        self.backend_wire = self.model_wire = None
        self.native_cache = {}
        self.native_root = None
        self.chat_hosts = {}
        self.handles = []
        self.env = {k: v for k, v in self.env.items() if not any(term in k.upper()
            for term in ['SECRET', 'PASSWORD', 'CREDENTIAL', 'PROXY', 'CBM_'])}
        for key, name in [('XDG_DATA_HOME', 'xdg-data'), ('XDG_CACHE_HOME', 'xdg-cache'),
                          ('TEMP', 'temp'), ('TMP', 'temp')]:
            directory = self.root / name
            directory.mkdir(exist_ok=True)
            self.env[key] = str(directory)
            self.evidence['isolation'][key] = str(directory)

    def prepare(self):
        require(self.args.backend == (REPO / backend_suffix()).resolve(),
            'Build/use this worktree backend in the same independent CARGO_TARGET_DIR')
        require(self.args.backend.is_file(), ('Missing current worktree backend', str(self.args.backend)))
        require(self.args.model_config.is_file(), ('Missing explicit authorized model configuration', str(self.args.model_config)))
        config = json.loads(self.args.model_config.read_text(encoding='utf-8'))
        require(config.get('apiKeyEnv') and os.environ.get(config['apiKeyEnv']),
            'Missing explicit authorized model credential environment')
        self.original_tasks = json.loads((REPO / 'meta-data/tasks.json').read_text(encoding='utf-8'))['tasks']
        self.original_tasks = {task['id']: task for task in self.original_tasks}
        require(all(key in self.original_tasks for key in TASK_IDS), 'Missing stable shared Task')
        self.hosts = sorted({self.original_tasks[key]['agent'] for key in TASK_IDS}) if self.host == 'declared' else [self.host]
        self.evidence['declaredTaskHosts'] = {key: self.original_tasks[key]['agent'] for key in TASK_IDS}
        self.evidence['creationSpecificTasks'] = []  # Published wrapper adds no Task.
        resolve_arguments(self.parser, self.args, {'herdr': ('herdr', 'herdr.exe'),
            'pi_package': ('pi_package', ''), 'omp_package': ('omp_package', ''),
            'pi_state_extension': ('pi_agent', 'extensions/herdr-agent-state.ts'),
            'omp_state_extension': ('omp_agent', 'extensions/herdr-omp-agent-state.ts'),
            'wayfinder': ('wayfinder', ''), 'wayfinder_skills': ('wayfinder_skills', ''),
            'pi_extensions': ('pi_extensions', ''), 'codebase_memory_mcp': ('codebase_memory_mcp', '')})
        for name, root, lock in [('herdr-bot', REPO, 'backend/Cargo.lock'), ('toolkit', self.args.toolkit, 'bun.lock')]:
            source = {'root': str(root), 'lockSha256': digest(root / lock)}
            for label, argv in [('revision', ['rev-parse', 'HEAD']), ('status', ['status', '--porcelain']), ('diff', ['diff', '--binary', 'HEAD'])]:
                source[label] = subprocess.run(['git', *argv], cwd=root, capture_output=True,
                    text=True, encoding='utf-8', check=True).stdout
            self.evidence.setdefault('sources', {})[name] = source
        self.evidence['runnerSources'] = {str(path): digest(path) for path in [
            REPO / 'scripts' / name for name in ['accept-workflow-nesting-create.py', 'workflow_create_acceptance.py',
                'workflow_create_safety.py', 'test-workflow-file-handoff.py', 'test-real-herdr-close.py']]}
        self.evidence['businessSourceIdentity'] = {str(path): digest(path) for path in [REPO / 'meta-data/tasks.json',
            REPO / 'meta-data/suites.json', REPO / 'meta-data/mcp.json', REPO / 'meta-data/task-options.toml', REPO / 'meta-data/workflow-maps/create-bot-wrapper.json',
            *sorted((REPO / 'meta-data/bots').glob('*.json'))]}
        self.evidence['backend'] = {'path': str(self.args.backend), 'sha256': digest(self.args.backend)}
        self.evidence['pathsFile'] = {'path': str(self.args.paths_file), 'sha256': digest(self.args.paths_file)}
        self.evidence['pageAndLockIdentity'] = {str(path): digest(path) for path in [REPO / 'frontend/package-lock.json',
            *sorted((REPO / 'backend/static').rglob('*'))] if path.is_file()}
        require((REPO / 'backend/static/index.html').is_file(), 'Build current worktree static page first')
        require(self.command([self.args.bun, '--version']).strip() == '1.4.4', 'Bun 1.4.4 is required')
        for host in self.hosts:
            package = getattr(self.args, host + '_package')
            entry = package / ('dist/cli.js' if host == 'omp' else 'dist/bundle/cli.js')
            launcher = shutil.which(host)
            require(entry.is_file() and launcher, ('Missing installed native host/launcher on unchanged PATH', host, str(entry)))
            self.evidence.setdefault('installedHosts', {})[host] = {'entry': str(entry), 'entrySha256': digest(entry),
                'launcher': launcher, 'launcherSha256': digest(Path(launcher)),
                'package': json.loads((package / 'package.json').read_text(encoding='utf-8'))['version']}
        Acceptance.prepare(self)
        self.backend_wire = WireProxy(self.base, self.root / 'backend-wire')
        (self.root / 'toolkit/extensions/workflow-chat/config.yaml').write_text(
            f'endpoint: {self.backend_wire.base}\nrequestTimeoutMs: 30000\ncollectTimeoutMs: 5460000\n', encoding='utf-8')
        self.provision()

    def configure_model(self):
        RolesAcceptance.configure_model(self)
        agent = self.root / 'agent'
        shutil.copy2(agent / 'models.json', agent / 'models.yml')
        (agent / 'config.yml').write_text('autoResume: false\nstartup:\n  setupWizard: false\n  showSplash: false\n  checkUpdate: false\nmarketplace:\n  autoUpdate: off\n', encoding='utf-8')
        self.native_root = Path(tempfile.mkdtemp(prefix='p21-native-'))
        settings = json.loads((agent / 'settings.json').read_text(encoding='utf-8'))
        settings['sessionDir'] = str(self.native_root / 'sessions')
        (agent / 'settings.json').write_text(json.dumps(settings), encoding='utf-8')
        for name in ['models.json', 'models.yml', 'settings.json', 'config.yml']:
            shutil.copy2(agent / name, self.native_root / name)
        self.env['PI_CODING_AGENT_DIR'] = str(self.native_root)
        self.evidence['isolation']['PI_CODING_AGENT_DIR'] = str(self.native_root)
        from workflow_create_acceptance import prepare_source_resources
        prepare_source_resources(self)

    def start(self, argv, label):
        if label == 'herdr':
            config = self.root / 'herdr.toml'
            config.write_text(config.read_text(encoding='utf-8').replace(
                'default_shell = "cmd.exe"', 'default_shell = "powershell.exe"'), encoding='utf-8')
        if label.startswith('backend-'):
            config = self.root / 'backend.toml'
            config.write_text('paths_file = "paths.local.toml"\n' + config.read_text(encoding='utf-8'),
                encoding='utf-8')
        return super().start(argv, label)

    def provision(self):
        from workflow_create_acceptance import provision
        provision(self)

    def native(self, chat):
        session = chat['sessionId']
        for path in self.native_root.rglob('*.jsonl'):
            try:
                with path.open(encoding='utf-8') as stream:
                    header = json.loads(stream.readline())
                    if header.get('type') == 'title':
                        header = json.loads(stream.readline())
            except (ValueError, OSError):
                continue
            if header.get('type') != 'session' or header.get('id') != session:
                continue
            fingerprint = digest(path)
            if session in self.native_cache and self.native_cache[session][0] == fingerprint:
                return self.native_cache[session][1]
            host = self.chat_hosts[session]
            package = getattr(self.args, host + '_package')
            module = package / ('src/session/session-manager.ts' if host == 'omp' else 'dist/core/session-manager.js')
            code = 'import {SessionManager} from ' + json.dumps(module.as_uri()) + ';const s=await SessionManager.open(' + json.dumps(str(path)) + ');console.log(JSON.stringify({id:s.getSessionId(),branch:s.getBranch()}));'
            argv = [self.args.bun, '--eval', code] if host == 'omp' else [self.args.node, '--input-type=module', '--eval', code]
            history = json.loads(self.command(argv))
            require(history['id'] == session, 'Native history identity mismatch')
            destination = self.root / 'histories' / session
            destination.mkdir(parents=True, exist_ok=True)
            shutil.copy2(path, destination / path.name)
            self.native_cache[session] = (fingerprint, history)
            return history
        return None

    @staticmethod
    def messages(history):
        return [entry['message'] for entry in history['branch'] if entry.get('type') == 'message']

    def await_chat(self, chat, after=None):
        def ready():
            history = self.native(chat)
            if not history:
                return None
            messages = self.messages(history)
            if after and not any(m.get('role') == 'user' and after in RolesAcceptance.message_text(m) for m in messages):
                return None
            if not messages or messages[-1].get('role') != 'assistant':
                return None
            last = messages[-1]
            require(last.get('stopReason') not in ['error', 'aborted'], ('Native model error', last))
            return history if not any(p.get('type') == 'toolCall' for p in last.get('content', []) if isinstance(p, dict)) else None
        return until('real Chat reaches user boundary', ready, self.args.timeout)

    def send(self, chat, message, purpose):
        self.evidence['interventions'].append({'purpose': purpose, 'chat': chat, 'text': message, 'at': time.monotonic()})
        self.evidence['interventions'][-1]['result'] = self.herdr('agent', 'prompt', chat['paneId'], message)
        return self.await_chat(chat, message)

    def observe_processes(self, expected_host=None):
        owned = str(self.root).replace("'", "''")
        code = ("$all=@(Get-CimInstance Win32_Process);$owned=@{};"
            "$all | Where-Object { $_.CommandLine -like '*" + owned + "*' } | ForEach-Object {$owned[$_.ProcessId]=$true};"
            "do {$changed=$false;foreach($p in $all){if($owned.ContainsKey($p.ParentProcessId) -and -not $owned.ContainsKey($p.ProcessId)){$owned[$p.ProcessId]=$true;$changed=$true}}}while($changed);"
            "$all | Where-Object {$owned.ContainsKey($_.ProcessId)} | Select-Object ProcessId,ParentProcessId,ExecutablePath,CommandLine | ConvertTo-Json -Compress")
        value = json.loads(self.command(['powershell.exe', '-NoProfile', '-Command', code]) or '[]')
        processes = value if isinstance(value, list) else [value]
        parse = ctypes.windll.shell32.CommandLineToArgvW
        parse.argtypes = [ctypes.c_wchar_p, ctypes.POINTER(ctypes.c_int)]
        parse.restype = ctypes.POINTER(ctypes.c_wchar_p)
        free = ctypes.windll.kernel32.LocalFree
        free.argtypes = [ctypes.c_void_p]
        for process in processes:
            count = ctypes.c_int()
            arguments = parse(process.get('CommandLine') or '', ctypes.byref(count))
            require(bool(arguments), 'Cannot parse actual Windows command line')
            try:
                process['arguments'] = [arguments[index] for index in range(count.value)]
            finally:
                free(arguments)
        self.evidence['nativeProcesses'].extend(processes)
        if expected_host:
            expected = Path(self.evidence['installedHosts'][expected_host]['entry']).resolve()
            require(any(any(Path(arg).resolve() == expected for arg in process['arguments']
                if arg.replace('\\', '/').endswith('/cli.js')) for process in processes),
                ('No actual declared installed native host OS process', expected_host, str(expected)))
        for process in processes:
            pid = process['ProcessId']
            if pid == os.getpid() or 'powershell' in str(process.get('ExecutablePath', '')).lower() or pid in {p for p, _ in self.handles}:
                continue
            self.handles.append((pid, shared['shared']['LiveProcess'](pid)))
        return processes

    def exercise(self):
        from workflow_create_acceptance import exercise
        try:
            exercise(self)
        finally:
            try:
                self.observe_processes()
            except BaseException:
                error = traceback.format_exc()
                self.evidence.setdefault('processObservationErrors', []).append(error)
                self.evidence['cleanupErrors'].append(error)
                self.evidence['passed'] = False

    def run(self):
        code = super().run()
        if code:
            self.evidence['verifiedHosts'] = []
            for item in self.evidence['coverage'].values():
                item['observedBeforeFailure'] = item['verified']
                item['verified'] = False
            (self.root / 'evidence.json').write_text(json.dumps(self.evidence, ensure_ascii=False, indent=2), encoding='utf-8')
        return code

    def cleanup_extra(self):
        errors = []
        for pid, handle in self.handles:
            try:
                until('owned process exits ' + str(pid), lambda: not handle.alive(), 15)
                self.evidence.setdefault('processExitConfirmations', []).append({'pid': pid, 'exited': True})
            except BaseException:
                errors.append(traceback.format_exc())
                try:
                    subprocess.run(['taskkill', '/PID', str(pid), '/T', '/F'], capture_output=True, timeout=45)
                    until('forced owned process exit', lambda: not handle.alive(), 15)
                except BaseException:
                    errors.append(traceback.format_exc())
            finally:
                handle.close()
        for proxy in [self.backend_wire, self.model_wire]:
            if proxy:
                try:
                    proxy.close()
                except BaseException:
                    errors.append(traceback.format_exc())
        for directory in [self.root / 'agent', self.native_root]:
            if directory is not None:
                for name in ['models.json', 'models.yml']:
                    (directory / name).unlink(missing_ok=True)
        if self.native_root is not None:
            archive = self.root / 'native-storage'
            shared['copy_evidence'](self.native_root, archive)
            self.evidence['nativeStorageArchive'] = str(archive)
        require(not errors, errors)


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
