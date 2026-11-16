"""P22: real Herdr effects via workflow dispatch and manager complete (Windows).

The approved seams are S2 (production HTTP) and S3 (installed Herdr CLI).
Independent outcome: the exact execution Tab disappears, a live child in that
Tab exits, the sentinel Tab survives, and native Pi history still contains both
our unique input and the controlled model's literal reply. A no-op/wrong-target
close can leave workflow completed but must fail these external observations.
No test adapter or direct tab-close command participates in the assertion path.
"""
import argparse
import json
import os
from pathlib import Path
import sys
import time

from developer_paths import REPO, backend_suffix, resolve_arguments
from acceptance.workflow.close import Acceptance


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--controlled-process', type=Path, help=argparse.SUPPRESS)
    parser.add_argument('--paths-file', type=Path, help='Machine roots TOML (default: repository paths.local.toml)')
    parser.add_argument('--herdr', type=Path)
    parser.add_argument('--backend', type=Path)
    parser.add_argument('--pi-package', type=Path)
    parser.add_argument('--pi-state-extension', type=Path)
    parser.add_argument('--toolkit', type=Path)
    parser.add_argument('--node', default='node')
    args = parser.parse_args()
    if args.controlled_process:
        pending = args.controlled_process.with_suffix('.pending')
        pending.write_text(json.dumps({'pid': os.getpid()}))
        pending.replace(args.controlled_process)
        while True:
            time.sleep(1)
    args.backend = (args.backend or REPO / backend_suffix()).resolve()
    resolve_arguments(parser, args, {
        'herdr': ('herdr', 'herdr.exe'),
        'pi_package': ('pi_package', ''),
        'pi_state_extension': ('pi_agent', 'extensions/herdr-agent-state.ts'),
        'toolkit': ('toolkit', ''),
    })
    args.repo = REPO
    return Acceptance(args).run()


if __name__ == '__main__':
    sys.exit(main())
