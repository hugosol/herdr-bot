//! Native agent status integration for the selected Herdr host.
//!
//! Start here to trace native reminders and sound:
//! - TUI keeps the official OMP/Pi Pane status extensions identified in `tui`.
//!   Their configured paths and execution-role applicability remain in
//!   `meta-data/task-options.toml` and `tasks::catalog`.
//! - App omits those TUI hooks. OMP/Pi emit native OSC 7501 through the core
//!   terminal created by `super::app`; `super::client` preserves that terminal
//!   path when launching the agent. There is no bot-side OSC adapter to load.
//! - Playback, custom sound selection and focus suppression belong to the
//!   external hosts: TUI `[ui.sound]` (`done_path`, `request_path`), App
//!   `settings.json` → `sounds` (`done`, `blocked`). See the native agent
//!   reminders section of `docs/usage.md` for the runtime requirements.
//!
//! Session state from connect-bot, launch ready and report delivery are not
//! playback requests. This module selects extensions only; callers retain
//! their existing ordering, entrypoint validation and Resume degradation.
//!
//! Existing checks: `backend/tests/chats/native_reminders.rs` and the App
//! launch case in `backend/tests/chats/chat_creation.rs`. Installed-host sound
//! observations use `scripts/accept-paired-close-reminders.py`, documented in
//! `docs/agents/testing.md`; controlled CLI checks do not prove playback.

mod tui;

use super::HerdrClient;
use crate::tasks::catalog::ResolvedEnvConfig;

pub(crate) fn select_host(herdr: &HerdrClient, env: &mut ResolvedEnvConfig) {
    if herdr.is_app() {
        env.extensions.retain(|extension| !tui::is_status_extension(&extension.id));
    }
}
