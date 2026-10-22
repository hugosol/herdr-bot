//! 从 zone 创建一个 Bot：命名、单次输入合成、后端拥有的创建生命周期。
//!
//! 一次创建只属于它自己：显示名可以重复，对象身份来自 Herdr 返回的 Tab / Pane 标识，agent 的
//! 控制别名由后端单独生成，不拿可能重名、含中文或空格的 Tab 名当唯一身份。创建任务在后端运行，
//! 与提交它的 HTTP 请求连接解耦；请求断开只失去观察，不会取消已经接受的创建。
//!
//! 生命周期只有四个可见阶段：`starting`（查找 Workspace、新建 Tab、启动 agent）、`submitting`
//! （提交合成后的初始指示）、`succeeded`（就绪与提交都已确认）、`failed`。失败保留已经取得的
//! 对象标识，不删除、不重建、不重发；没有取得的标识不会被补造。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tokio::sync::Mutex;

use crate::herdr::{HerdrClient, HerdrError};
use crate::templates::BotTemplate;

/// Bot 短实例标识的长度（小写十六进制字符数）。
pub const BOT_ID_LEN: usize = 6;

/// 一次创建在页面上可见的阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    /// 正在查找 Workspace、新建 Tab 并等待 agent 就绪。
    Starting,
    /// 目标 agent 已就绪，正在提交初始指示。
    Submitting,
    /// 就绪与提交都已确认。
    Succeeded,
    /// 任一步失败或超时；已取得的现场标识仍保留在结果里。
    Failed,
}

impl Stage {
    fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed)
    }
}

/// 失败发生的位置；与顶部 `stage` 分开，避免把“启动失败”和“提交失败”混成同一种情况。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FailureStage {
    Workspace,
    Tab,
    Start,
    Submit,
}

/// 一次失败的可定位说明。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    pub stage: FailureStage,
    pub code: String,
    pub message: String,
}

/// 公开的创建状态；也是创建被接受时的回执与页面轮询读取的同一形状。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreationView {
    /// 本次创建的生命周期标识（不是 Herdr Tab ID）。
    pub id: String,
    /// 按本次有效命名规则（创建覆盖或模板默认）生成并实际用作 Tab 显示名的名称。
    pub name: String,
    /// 发起创建的已登记 zone 目录。
    pub zone: String,
    /// 创建被接受时使用的模板标识。
    pub template_id: String,
    pub stage: Stage,
    /// 已取得的 Herdr Tab 标识；未取得时不出现。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab_id: Option<String>,
    /// 已取得的 Herdr Pane 标识；未取得时不出现。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pane_id: Option<String>,
    /// 后端生成的 agent 控制别名；未开始启动时不出现。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_name: Option<String>,
    /// 只有初始指示提交被确认后才为 true；失败时不把未知说成已提交。
    pub input_submitted: bool,
    /// 失败阶段与说明；成功时不出现。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<Failure>,
}

impl CreationView {
    pub fn starting(id: String, name: String, zone: String, template_id: String) -> Self {
        Self {
            id,
            name,
            zone,
            template_id,
            stage: Stage::Starting,
            tab_id: None,
            pane_id: None,
            agent_name: None,
            input_submitted: false,
            failure: None,
        }
    }
}

/// 进程内的创建生命周期表：按创建标识读取当前状态。
///
/// 这里不是 Bot 列表，也不提供历史或恢复；它只让提交创建的页面在请求结束后观察同一次创建。
/// 每条创建各自独立，不共享全局创建资格。
pub struct BotCreations {
    jobs: Mutex<HashMap<String, Arc<Mutex<CreationView>>>>,
}

impl BotCreations {
    pub fn new() -> Self {
        Self {
            jobs: Mutex::new(HashMap::new()),
        }
    }

    /// 登记一次已接受的创建并返回它的状态句柄。
    pub async fn insert(&self, view: CreationView) -> Arc<Mutex<CreationView>> {
        let handle = Arc::new(Mutex::new(view.clone()));
        self.jobs
            .lock()
            .await
            .insert(view.id.clone(), handle.clone());
        handle
    }

    /// 读取一次创建当前的状态；未知标识是 `None`，不会被解释成成功或失败。
    pub async fn get(&self, id: &str) -> Option<CreationView> {
        let handle = self.jobs.lock().await.get(id).cloned()?;
        let view = handle.lock().await.clone();
        Some(view)
    }
}

impl Default for BotCreations {
    fn default() -> Self {
        Self::new()
    }
}

/// 生成一个短实例标识：小写十六进制，只用于显示名中的 `{bot.id}`，不是 Herdr Tab ID。
pub fn new_bot_id() -> String {
    format!("{:06x}", next_token() & 0xff_ffff)
}

/// 生成一次创建的生命周期标识。
pub fn new_creation_id() -> String {
    format!("bc-{:012x}", next_token() & 0xffff_ffff_ffff)
}

/// 生成 agent 控制别名：满足 Herdr 的 `[a-z][a-z0-9_-]{0,31}`，且不依赖显示名。
pub fn new_agent_name() -> String {
    format!("bot-{:012x}", next_token() & 0xffff_ffff_ffff)
}

/// 校验预览返回的短实例标识；创建请求只能沿用后端分配的标识，不能用任意文本改写名称结构。
pub fn is_valid_bot_id(value: &str) -> bool {
    value.len() == BOT_ID_LEN
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// 每个 Bot 的会话目录根：模板存储文件所在目录下的 `bot-sessions`。
///
/// 每个 Bot 使用其中以本次创建标识命名的独立目录。显式会话目录让 OMP 跳过 `autoResume` 的隐式
/// 续接，也让 Pi 不依赖其默认查找；目录位于 herdr-bot 自己的数据位置，不写进 zone 工作目录。
/// 启动路径在服务启动时解析成绝对路径，避免 agent 以自己的 cwd 解释相对路径。
pub fn session_root(storage_file: &Path) -> PathBuf {
    let parent = storage_file
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::path::absolute(parent)
        .unwrap_or_else(|_| parent.to_path_buf())
        .join("bot-sessions")
}

/// 按模板的命名规则渲染展示名。
///
/// 只有四个已承诺字段被替换；未知占位符保持原样。`{bot.id}` 取本次的短实例标识，不是 Herdr
/// Tab ID。`zone.name` 是 zone 目录的末级名称。
pub fn render_bot_name(rule: &str, zone: &str, template: &BotTemplate, bot_id: &str) -> String {
    let zone_name = Path::new(zone)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    // 只扫描规则本身一次；替换值中的占位符按字面保留，与页面本地预览一致。
    let mut rendered = String::with_capacity(rule.len());
    let mut remaining = rule;
    while let Some(start) = remaining.find('{') {
        rendered.push_str(&remaining[..start]);
        remaining = &remaining[start..];
        let replacement = [
            ("{zone.name}", zone_name.as_str()),
            ("{template.name}", template.name.as_str()),
            ("{agent}", template.agent.label()),
            ("{bot.id}", bot_id),
        ]
        .into_iter()
        .find(|(placeholder, _)| remaining.starts_with(*placeholder));
        if let Some((placeholder, value)) = replacement {
            rendered.push_str(value);
            remaining = &remaining[placeholder.len()..];
        } else {
            rendered.push('{');
            remaining = &remaining[1..];
        }
    }
    rendered.push_str(remaining);
    rendered
}

/// 把 Skill、固定 prompt 与本次 input 合成一次提交的正文。
///
/// 有 Skill 时使用 `/skill:<name>`；命令名后用普通空格接正文。固定 prompt 与本次 input 以空行
/// 分段，并保留各自的多行内容。三者都为空时仍是空字符串：不补造内容，也不跳过提交。
pub fn compose_initial_input(
    template: &BotTemplate,
    native_skill: Option<&str>,
    input: &str,
) -> String {
    let mut sections: Vec<&str> = Vec::new();
    if !template.prompt.is_empty() {
        sections.push(template.prompt.as_str());
    }
    if !input.is_empty() {
        sections.push(input);
    }
    let body = sections.join("\n\n");

    match native_skill {
        Some(skill) if body.is_empty() => format!("/skill:{skill}"),
        Some(skill) => format!("/skill:{skill} {body}"),
        None => body,
    }
}

/// 目标 agent 的原生启动参数：只有显式配置时才传 `--model` / `--thinking`。
///
/// 模型使用本次元数据快照中的完整 provider / model 映射；默认值不传覆盖。
/// `--session-dir` 给本次 Bot 全新会话位置，避免自动续接同 cwd 的旧对话。
/// 两种 Agent 均关闭扩展自动发现，只显式加载本次快照的固定与模板所选入口。
pub fn agent_launch_args(
    preset: &crate::catalog::ResolvedPreset,
    session_dir: &Path,
) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(model) = &preset.provider_model {
        args.push("--model".to_string());
        args.push(model.clone());
    }
    if let Some(thinking) = &preset.thinking {
        args.push("--thinking".to_string());
        args.push(thinking.clone());
    }
    args.push("--session-dir".to_string());
    args.push(session_dir.to_string_lossy().into_owned());
    args.push("--no-extensions".to_string());
    for extension in &preset.extensions {
        args.push("--extension".to_string());
        args.push(extension.entrypoint.to_string_lossy().into_owned());
    }
    args
}

/// 执行一次已接受的创建；调用方已经固定了模板与 zone 的当时取值。
///
/// 阶段推进只发生在真实控制结果之后：`agent start` 成功（Herdr 确认可交互）才提交输入，
/// `agent prompt` 被确认后才进入成功；不等待模型轮次、Skill 执行或 connect-bot 接入。
pub async fn run_creation(
    herdr: Arc<HerdrClient>,
    job: Arc<Mutex<CreationView>>,
    template: BotTemplate,
    preset: crate::catalog::ResolvedPreset,
    zone: String,
    input: String,
    session_dir: PathBuf,
    workspace_name: String,
) {
    let workspace = match herdr.workspace(&workspace_name).await {
        Ok(workspace) => workspace,
        Err(error) => {
            fail(&job, FailureStage::Workspace, &error).await;
            return;
        }
    };

    let name = job.lock().await.name.clone();
    let created = match herdr.create_tab(&workspace.id, &zone, &name).await {
        Ok(created) => created,
        Err(error) => {
            fail(&job, FailureStage::Tab, &error).await;
            return;
        }
    };
    {
        let mut view = job.lock().await;
        view.tab_id = Some(created.tab_id);
        view.pane_id = Some(created.pane_id);
    }

    let agent_name = new_agent_name();
    {
        let mut view = job.lock().await;
        view.agent_name = Some(agent_name.clone());
    }
    let args = agent_launch_args(&preset, &session_dir);
    let kind = template.agent.label();
    let pane_id = job.lock().await.pane_id.clone().unwrap_or_default();
    if let Err(error) = herdr.start_agent(&agent_name, kind, &pane_id, &args).await {
        fail(&job, FailureStage::Start, &error).await;
        return;
    }

    {
        let mut view = job.lock().await;
        view.stage = Stage::Submitting;
    }
    let text = compose_initial_input(&template, preset.native_skill.as_deref(), &input);
    if let Err(error) = herdr.prompt(&agent_name, &text).await {
        fail(&job, FailureStage::Submit, &error).await;
        return;
    }

    let mut view = job.lock().await;
    view.input_submitted = true;
    view.stage = Stage::Succeeded;
}

/// 记录失败：阶段固定为 `failed`，失败位置、类别与说明保留；已取得的标识不动。
async fn fail(job: &Arc<Mutex<CreationView>>, stage: FailureStage, error: &HerdrError) {
    let mut view = job.lock().await;
    if view.stage.is_terminal() {
        return;
    }
    view.stage = Stage::Failed;
    view.failure = Some(Failure {
        stage,
        code: error.code().to_string(),
        message: stage_message(stage, error),
    });
}

/// 在 Herdr 的原始说明之外补上现场状态；不把未知的提交结果说成确定。
///
/// 同一阶段里，“明确被拒绝”和“超时/响应不可理解”是两种不同事实：前者可以说提交失败，
/// 后者只能说明结果未确认，不能把它表述成确定未发送。已经取得的对象标识不会被撤销。
fn stage_message(stage: FailureStage, error: &HerdrError) -> String {
    let base = error.message();
    let outcome_unknown = matches!(
        error,
        HerdrError::Timeout(_) | HerdrError::Unavailable(_) | HerdrError::UnexpectedResponse { .. }
    );
    match stage {
        FailureStage::Workspace => format!("{base} 本次没有新建 Tab，也没有启动 agent。"),
        FailureStage::Tab if outcome_unknown => {
            format!("{base} 无法确认 Tab 是否已经创建；本次没有继续启动 agent。")
        }
        FailureStage::Tab => format!("{base} 新建 Tab 被拒绝；本次没有继续启动 agent。"),
        FailureStage::Start if outcome_unknown => format!(
            "{base} Tab 与 Pane 已创建并保留；agent 的就绪状态未确认，因此没有提交初始指示。"
        ),
        FailureStage::Start => {
            format!("{base} Tab 与 Pane 已创建并保留；agent 启动被拒绝，因此没有提交初始指示。")
        }
        FailureStage::Submit if outcome_unknown => format!(
            "{base} agent 已就绪；初始指示的投递结果未确认，不能据此断定未发送。Tab、Pane 与 agent 已保留。"
        ),
        FailureStage::Submit => {
            format!("{base} agent 已就绪，但初始指示的提交被明确拒绝。Tab、Pane 与 agent 已保留。")
        }
    }
}

/// 时间、进程与计数混合的短标识，避免同一进程内快速连续创建取到相同值。
fn next_token() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let counter = NEXT.fetch_add(1, Ordering::Relaxed);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos() as u64)
        .unwrap_or(0);
    now.rotate_left(17) ^ counter.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ u64::from(process::id())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::templates::{Agent, BotTemplate};

    fn template(rule: &str) -> BotTemplate {
        BotTemplate {
            id: "tpl-literal".to_string(),
            name: "审查".to_string(),
            bot_name: rule.to_string(),
            agent: Agent::Omp,
            model: "default".into(),
            thinking: "default".into(),
            skill: None,
            prompt: String::new(),
            extensions: Vec::new(),
        }
    }

    /// P8：四个已承诺字段与固定文字按预写的完整名称渲染，不依赖实现计算期望。
    #[test]
    fn renders_the_four_fields_with_literal_expected_names() {
        let template = template("{zone.name} · {template.name} · {agent} · {bot.id}");
        assert_eq!(
            render_bot_name(
                &template.bot_name,
                r"C:\Projects\alpha",
                &template,
                "a7f3c2"
            ),
            "alpha · 审查 · omp · a7f3c2"
        );
        assert_eq!(
            render_bot_name(
                &template.bot_name,
                r"D:\work\团队 目录",
                &template,
                "0f1e2d"
            ),
            "团队 目录 · 审查 · omp · 0f1e2d"
        );
    }

    /// `{bot.id}` 是短实例标识；未知占位符保持原样，不被当作脚本或表达式执行。
    #[test]
    fn leaves_unknown_placeholders_literal() {
        let template = template("{template.name} · {bot.id} · {unknown} · {zone.name}");
        assert_eq!(
            render_bot_name(&template.bot_name, "/tmp/beta", &template, "00a1b2"),
            "审查 · 00a1b2 · {unknown} · beta"
        );
    }

    #[test]
    fn bot_id_is_six_lowercase_hex_characters() {
        let id = new_bot_id();
        assert_eq!(id.len(), BOT_ID_LEN);
        assert!(is_valid_bot_id(&id), "{id}");
        for invalid in ["", "A7F3C2", "a7f3c", "a7f3c2x", "a7f3c\n"] {
            assert!(!is_valid_bot_id(invalid), "{invalid:?} 不应是合法短标识");
        }
    }

    /// 无 Skill、无 prompt、非空 input 时按字面提交本次任务。
    #[test]
    fn plain_input_is_submitted_once_without_extra_prefix() {
        let template = template("{bot.id}");
        assert_eq!(
            compose_initial_input(&template, template.skill.as_deref(), "检查登录\n第二行"),
            "检查登录\n第二行"
        );
    }

    /// 有 Skill 时只生成一次 `/skill:` 调用，名称后接普通空格与正文。
    #[test]
    fn skill_uses_one_native_invocation_prefix() {
        let mut template = template("{bot.id}");
        template.skill = Some("code-review".into());
        template.prompt = "固定指示".to_string();
        assert_eq!(
            compose_initial_input(&template, template.skill.as_deref(), "本次任务"),
            "/skill:code-review 固定指示\n\n本次任务"
        );
    }
}
