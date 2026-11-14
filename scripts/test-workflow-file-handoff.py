"""S2 workflow file handoff: real Herdr, native Pi, actual model, paired source trees.

Approved seams: production HTTP, native Chat/provider traffic, installed Herdr,
external business program and read-only runtime records. No Session is supplied
by the driver and no runtime record is edited. Controlled-host parameter coverage
is separately owned by toolkit's node-report-contract dock (OMP and Pi).

Independent oracle: obsolete execution output must not reach the consumer; a
current false report cannot complete; retained pendingReports cannot repeatedly
wake collect; a new true report wakes it, but only explicit manager actions can
complete and dispatch. Detailed business material remains in external files."""
import argparse
from pathlib import Path
import sys

from developer_paths import REPO, backend_suffix, resolve_arguments
from acceptance.workflow.handoff import RolesAcceptance, FanInAcceptance
from acceptance.host.processes import require

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--scenario', choices=['roles', 'fan-in', 'plan-workflow'], required=True)
    parser.add_argument('--paths-file', type=Path, required=True, help='Explicit isolated dependency roots, never user business/session configuration')
    parser.add_argument('--backend', type=Path, required=True)
    parser.add_argument('--toolkit', type=Path, required=True)
    parser.add_argument('--herdr', type=Path)
    parser.add_argument('--pi-package', type=Path)
    parser.add_argument('--pi-state-extension', type=Path)
    parser.add_argument('--model-config', type=Path, help='Acceptance-only compatible-model JSON; otherwise roots.workflow_model/model.json')
    parser.add_argument('--shell-path', type=Path, help='Explicit native Pi shell (for example Git Bash on Windows), written only to isolated settings')
    parser.add_argument('--node', default='node')
    parser.add_argument('--bun', default='bun')
    parser.add_argument('--timeout', type=int, default=300, help='Bounded per-phase observation window; no scenario retries')
    args = parser.parse_args()
    require(1 <= args.timeout <= 900, '--timeout must be between 1 and 900 seconds')
    resolve_arguments(parser, args, {'herdr': ('herdr', 'herdr.exe'), 'pi_package': ('pi_package', ''),
        'pi_state_extension': ('pi_agent', 'extensions/herdr-agent-state.ts'), 'model_config': ('workflow_model', 'model.json')})
    args.backend = args.backend.resolve()
    args.toolkit = args.toolkit.resolve()
    args.repo = REPO
    if args.scenario == 'plan-workflow':
        from workflow_plan_acceptance import PlanWorkflowScenario

        class PlanWorkflowAcceptance(PlanWorkflowScenario, RolesAcceptance):
            """Use the same isolated real-host lifecycle and wire observers."""

        return PlanWorkflowAcceptance(args).run()
    return (FanInAcceptance if args.scenario == 'fan-in' else RolesAcceptance)(args).run()


if __name__ == '__main__':
    sys.exit(main())
