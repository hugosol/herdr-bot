//! Resume navigation verifies both the bound core terminal and the exact App
//! pane. A successful command alone does not establish the App's focus.

use super::{AppHost, AppTerminal, CORE_GET, QUERY_TIMEOUT, nonempty, run_cli, unexpected};
use crate::herdr::{HerdrError, NavigationTarget};
use serde_json::{Value, json};

const FOCUS: &str = "herdr-app pane.focus";
const STATUS: &str = "herdr-app status";

impl AppHost {
    pub(in crate::herdr) async fn focus_bound_pane(
        &self, target: &NavigationTarget, terminal: &AppTerminal,
    ) -> Result<Value, HerdrError> {
        let record = self.observe_launch(target, terminal).await?;
        if record.get("state").and_then(Value::as_str) != Some("live") {
            return Err(unexpected(CORE_GET, "App 绑定的终端不再运行"));
        }

        let args = json!({"pane": target.pane_id}).to_string();
        let reply = run_cli(&self.executable, FOCUS, &["do", "pane.focus", &args], QUERY_TIMEOUT).await?;
        if reply.get("ok").and_then(Value::as_bool) != Some(true) {
            return Err(unexpected(FOCUS, "缺少 App 聚焦回执"));
        }
        let status = run_cli(&self.executable, STATUS, &["status"], QUERY_TIMEOUT).await?;
        if nonempty(status.get("space")) != Some(&target.workspace_id)
            || nonempty(status.get("tab")) != Some(&target.tab_id)
            || nonempty(status.get("focused_pane")) != Some(&target.pane_id)
        {
            return Err(unexpected(STATUS, "App 当前焦点与目标 Space、Tab 或 Pane 不一致"));
        }
        Ok(record)
    }
}
