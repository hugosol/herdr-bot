"""Worktree, artifact and dependency provenance for acceptance entrypoints."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

from developer_paths import REPO
from acceptance.host.isolation import require


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def command(argv, cwd):
    result = subprocess.run([str(x) for x in argv], cwd=cwd, capture_output=True,
                            encoding='utf-8', errors='replace', timeout=60)
    require(result.returncode == 0, {'argv': list(map(str, argv)), 'exit': result.returncode,
                                   'stdout': result.stdout, 'stderr': result.stderr})
    return result.stdout


def preflight(args, evidence):
    require(args.repo == REPO and Path.cwd().resolve() == REPO, 'Run from the explicitly supplied herdr-bot worktree')
    require((args.toolkit / '.git').is_file(), 'toolkit-worktree must be an explicit Git worktree, not an installed fallback')
    depfile = args.backend.with_suffix('.d')
    require(args.backend.name == 'herdr-bot.exe' and depfile.is_file(), 'backend provenance: executable needs its Cargo .d identity')
    deps = depfile.read_text(encoding='utf-8').replace('\\', '/')
    require(str(args.repo / 'backend/src/main.rs').replace('\\', '/') in deps,
            'backend provenance: Cargo depfile does not name this worktree source')
    source = list((args.repo / 'backend/src').rglob('*.rs')) + [args.repo / 'backend/Cargo.toml', args.repo / 'backend/Cargo.lock']
    require(all(p.stat().st_mtime <= args.backend.stat().st_mtime for p in source), 'backend provenance: binary predates current source; build this worktree')
    resolver = args.backend.parent / 'herdr-paths.exe'
    require(resolver.is_file(), 'Build this worktree herdr-paths binary beside --backend; no source/deployment fallback')
    roots = json.loads(command([resolver, '--paths', args.paths_file], args.repo))
    for key in ['herdr', 'pi_package', 'omp_package']:
        require(key in roots, f'Isolated dependency config requires roots.{key}')
    args.herdr = Path(roots['herdr']) / 'herdr.exe'
    args.pi_package = Path(roots['pi_package'])
    args.omp_package = Path(roots['omp_package'])
    required = [args.herdr, args.pi_package / 'dist/bundle/cli.js', args.omp_package / 'dist/cli.js',
                args.toolkit / 'entrypoints/pi/pi-suite.ts', args.repo / 'backend/static/index.html']
    for path in required:
        require(path.is_file(), ('Missing real dependency/artifact', str(path)))
    launchers = {}
    for name, package in [('pi', args.pi_package), ('omp', args.omp_package)]:
        path = Path(shutil.which(name) or '')
        require(path.is_file(), f'Installed {name} must be on PATH; runner does not install or replace it')
        text = path.read_text(encoding='utf-8').replace('\\', '/')
        package_name = json.loads((package / 'package.json').read_text())['name']
        require(package_name in text, ('PATH launcher package differs', name, path))
        # npm launchers resolve node_modules from their own directory. Check actual package bytes.
        installed = path.parent / 'node_modules' / package_name
        require(installed.resolve() == package.resolve(), ('PATH launcher does not load declared package', installed, package))
        launchers[name] = {'path': str(path), 'sha256': digest(path), 'version': json.loads((package / 'package.json').read_text())['version']}
    evidence['launchers'] = launchers
    evidence['host'] = {name: command([name, '--version'], args.repo).strip() for name in ['node', 'bun', 'python']}
    evidence['herdrVersion'] = command([args.herdr, '--version'], args.repo)
    evidence['identity'] = {}
    for name, root in [('herdr-bot', args.repo), ('toolkit', args.toolkit)]:
        tracked = command(['git', '-C', root, 'ls-files'], root).splitlines()
        evidence['identity'][name] = {'root': str(root), 'revision': command(['git', '-C', root, 'rev-parse', 'HEAD'], root).strip(),
            'diff': command(['git', '-C', root, 'diff', '--', '.', ':!.scratch'], root),
            'sha256': {name: digest(root / name) for name in tracked if (root / name).is_file() and not name.startswith('.scratch/')}}
    frontend_sources = [p for p in (args.repo / 'frontend/src').rglob('*') if p.is_file()]
    frontend_sources += [args.repo / 'frontend/package.json', args.repo / 'frontend/bun.lock', args.repo / 'frontend/vite.config.ts']
    require(all(p.stat().st_mtime <= (args.repo / 'backend/static/index.html').stat().st_mtime for p in frontend_sources),
            'Frontend output predates current source; build the supplied worktree')
    evidence['frontend'] = {str(p.relative_to(args.repo)): digest(p) for p in (args.repo / 'backend/static').rglob('*') if p.is_file()}
    evidence['binary'] = {'path': str(args.backend), 'sha256': digest(args.backend), 'depfile': deps,
                          'sourceHashes': {str(p.relative_to(args.repo)): digest(p) for p in source}}
    evidence['paths'] = {'file': str(args.paths_file), 'sha256': digest(args.paths_file), 'roots': roots}


def source_identity(root, lock, evidence_dir):
    outputs = [evidence_dir]
    if os.environ.get('CARGO_TARGET_DIR'):
        outputs.append(Path(os.environ['CARGO_TARGET_DIR']).resolve())
    result = {'root': str(root), 'lockSha256': digest(root / lock),
              'excludedOwnedOutputs': [str(path) for path in outputs]}
    for key, argv in [('revision', ['git', 'rev-parse', 'HEAD']), ('status', ['git', 'status', '--porcelain']),
                      ('diff', ['git', 'diff', '--binary', 'HEAD'])]:
        command = subprocess.run(argv, cwd=root, capture_output=True, text=True, encoding='utf-8', errors='replace', check=True)
        result[key] = command.stdout
    listed = subprocess.run(['git', 'ls-files', '--cached', '--others', '--exclude-standard'], cwd=root,
                            capture_output=True, text=True, encoding='utf-8', errors='replace', check=True)
    result['fileSha256'] = {name: digest(root / name) for name in listed.stdout.splitlines()
                            if not name.startswith('.scratch/') and (root / name).is_file()
                            and not any((root / name).is_relative_to(path) for path in outputs)}
    return result


