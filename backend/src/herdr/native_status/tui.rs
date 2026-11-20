//! Official TUI Pane lifecycle and native-session-reference extensions.
//! App uses the agent's native terminal status instead of these hooks.

pub(super) fn is_status_extension(id: &str) -> bool {
    matches!(id, "omp-herdr-agent-state" | "pi-herdr-agent-state")
}
