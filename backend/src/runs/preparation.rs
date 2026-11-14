//! Manager-attested business input preparation; structural checks stay generic.
use super::*;
use super::operation::{Action, Decision};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PrepareInputsRequest {
    pub(crate) session_id: String,
    #[serde(deserialize_with = "crate::scheduler::object_array")]
    pub(crate) file_inputs: Vec<crate::scheduler::ResolvedFile>,
    pub(crate) confirmed: bool,
    pub(crate) reason: String,
}

pub(crate) async fn manager_prepare_inputs(state: AppState, input: PrepareInputsRequest) -> RunResult {
    operation::execute(state, &input.session_id, Action::PrepareInputs {
        files: input.file_inputs, confirmed: input.confirmed, reason: input.reason,
    }).await
}

pub(super) fn prepare(
    state: &AppState, run: &mut Run, files: Vec<crate::scheduler::ResolvedFile>,
    confirmed: bool, reason: String,
) -> Result<Decision, String> {
    if !matches!(run.status.as_str(), "running" | "paused") {
        return Decision::reject("run-terminal", "Run 已结束，不接受输入准备。");
    }
    if confirmed && reason.trim().is_empty() {
        return Decision::reject("input-confirmation-required", "确认输入前请按 Map 输入要求检查业务内容，并说明满足要求的依据。");
    }
    if let Some(message) = crate::scheduler::check_prepared_inputs(&state.map_directory, &run.map, &files, confirmed)? {
        return Decision::reject("run-inputs-invalid", message);
    }
    let before = serde_json::json!({"inputsReady":run.inputs_ready,"fileInputs":run.file_inputs});
    run.file_inputs = files;
    run.inputs_ready = confirmed;
    let event = serde_json::json!({
        "id":crate::chats::new_creation_id(),
        "at":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
        "actor":run.manager.as_ref().ok_or("缺少管理者真实关联。")?,
        "action":"prepare_inputs", "before":before,
        "after":{"inputsReady":run.inputs_ready,"fileInputs":run.file_inputs}, "reason":reason,
    });
    Ok(Decision::Commit(event))
}
