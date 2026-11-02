"""Load machine roots through the backend's single TOML/path implementation."""
import json
import os
from pathlib import Path
import subprocess


REPO = Path(__file__).resolve().parent.parent


def resolve_arguments(parser, args, defaults):
    """Explicit CLI paths win; only missing arguments require machine roots.

    defaults maps argument names to (root name, relative suffix).
    """
    missing = [name for name in defaults if getattr(args, name) is None]
    roots = {}
    if missing:
        paths_file = (args.paths_file or REPO / 'paths.local.toml').resolve()
        try:
            result = subprocess.run(
                ['cargo', 'run', '--quiet', '--manifest-path', str(REPO / 'backend/Cargo.toml'),
                 '--bin', 'herdr-paths', '--', '--paths', str(paths_file)],
                capture_output=True, text=True,
            )
        except OSError as error:
            parser.error(f'Cannot load machine paths via herdr-paths: {error}')
        if result.returncode:
            parser.error(f'Cannot load {paths_file}: {result.stderr.strip()}')
        try:
            roots = json.loads(result.stdout)
        except ValueError as error:
            parser.error(f'Invalid herdr-paths JSON: {error}')
        for name in missing:
            root, suffix = defaults[name]
            if root not in roots:
                parser.error(f'{paths_file}: missing roots.{root}; set --{name.replace("_", "-")} explicitly')
            setattr(args, name, Path(roots[root]) / suffix)
    for name in defaults:
        setattr(args, name, getattr(args, name).resolve())


def backend_suffix():
    target = Path(os.environ['CARGO_TARGET_DIR']).resolve() if os.environ.get('CARGO_TARGET_DIR') else Path('backend/target')
    return target / 'debug' / ('herdr-bot.exe' if os.name == 'nt' else 'herdr-bot')
