//! 服务端配置：监听地址、固定脚本操作的绑定、会话登记的两个窗口与关机倒计时时长。
//!
//! 脚本目标与工作目录只来自这里的配置，不作为 HTTP 请求参数，客户端无法替换。
//! 会话登记的两个窗口（`[sessions] online_timeout_seconds` / `offline_retention_seconds`）与关机倒计时
//! 时长（`[shutdown] countdown_seconds`）同样是启动期配置：配置在进程启动时读取一次，运行中改写文件
//! 不会被重新读取（没有热加载），非法值在启动时就被拒绝。

use std::fs;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Context;
use serde::Deserialize;
use serde::de::{self, Deserializer};

use crate::sessions::{DEFAULT_OFFLINE_RETENTION_SECONDS, DEFAULT_ONLINE_SECONDS, SessionPolicy};
use crate::zones::DEFAULT_REGISTRY_FILE;

/// herdr-bot 主界面运行配置。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Axum 监听地址，例如 `0.0.0.0:8787`（非回环，局域网可达）。
    pub listen_addr: String,
    /// 固定脚本操作绑定。
    pub operations: Operations,
    /// mDNS 发布设置；整个 `[mdns]` 段可缺省，缺省等于默认（启用、地址自动选择）。
    #[serde(default)]
    pub mdns: MdnsConfig,
    /// 关机倒计时设置；整个 `[shutdown]` 段可缺省，缺省等于默认（10 秒）。
    #[serde(default)]
    pub shutdown: ShutdownConfig,
    /// 会话登记的两个窗口；整个 `[sessions]` 段可缺省，缺省等于默认（在线 10 秒、离线保留 30 秒）。
    #[serde(default)]
    pub sessions: SessionsConfig,
    /// zone 登记列表的存储位置；整个 `[zones]` 段可缺省，缺省为进程工作目录下的
    /// `.data/zones.json`。
    #[serde(default)]
    pub zones: ZonesConfig,
    /// Bot 模板的存储位置；整个 `[bot_templates]` 段可缺省，缺省为进程工作目录下的
    /// `.data/bot-templates.json`。
    #[serde(default)]
    pub bot_templates: BotTemplatesConfig,
    /// Herdr 控制设置；整个 `[herdr]` 段必须显式配置目标 Workspace 名称。
    ///
    /// 这一段不是后备缺省：默认 Workspace 由部署者明确指定，缺失、空串或纯空白都会让
    /// 进程启动失败，而不是落到任何猜测出来的位置。
    pub herdr: HerdrConfig,
    /// Git 命令执行设置；整个 `[git]` 段可缺省，缺省每条命令最多执行 300 秒。
    #[serde(default)]
    pub git: GitConfig,
}

/// Herdr 控制设置（`[herdr]` 段）。
///
/// 默认按 `workspace_name` 精确查找唯一的 Herdr Workspace；本次选择“当前”时按 zone 根目录名匹配。
/// `executable` 是生产调用 `herdr` CLI 的路径，缺省按 `PATH` 查找；它不改变目标 Workspace 的归属。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HerdrConfig {
    /// 目标 Herdr Workspace 的显示名，必须显式配置且非空白。
    pub workspace_name: String,
    /// Herdr CLI 可执行文件；缺省 `herdr`（按 PATH 查找）。
    #[serde(default = "herdr_executable_by_default")]
    pub executable: PathBuf,
}

impl HerdrConfig {
    /// 用明确的目标 Workspace 名称构造配置；可执行文件取缺省 `herdr`。
    pub fn new(workspace_name: impl Into<String>) -> Self {
        Self {
            workspace_name: workspace_name.into(),
            executable: herdr_executable_by_default(),
        }
    }
}

/// `[herdr] executable` 的缺省值：按 PATH 查找的 `herdr`。
fn herdr_executable_by_default() -> PathBuf {
    PathBuf::from("herdr")
}

/// Git 命令执行设置（`[git]` 段）。
///
/// 这是本功能的执行时限：每条 Git 命令只允许运行到这里配置的时长。超过时限时停止本次执行并
/// 中断后续步骤；它不是 Git 自己的退出结果，页面会明确标出这是本功能取消的命令。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitConfig {
    /// 每条 Git 命令的执行时限；缺省 300 秒。
    #[serde(default = "command_timeout_seconds_by_default")]
    pub command_timeout_seconds: CommandTimeoutSeconds,
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            command_timeout_seconds: command_timeout_seconds_by_default(),
        }
    }
}

/// 缺省的 Git 命令执行时限：300 秒。
pub const DEFAULT_COMMAND_TIMEOUT_SECONDS: u32 = 300;

/// `[git] command_timeout_seconds` 的缺省值：300 秒。
fn command_timeout_seconds_by_default() -> CommandTimeoutSeconds {
    CommandTimeoutSeconds(DEFAULT_COMMAND_TIMEOUT_SECONDS)
}

/// 每条 Git 命令的执行时限（秒）：合法范围 1..=u32::MAX。
///
/// 缺省 300 秒；零、负数、小数、字符串或拼错的字段名都会让启动失败，而不是静默回退。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "u32")]
pub struct CommandTimeoutSeconds(u32);

impl CommandTimeoutSeconds {
    pub fn new(seconds: u32) -> Result<Self, &'static str> {
        if seconds == 0 {
            Err("command_timeout_seconds 必须是正整数（默认 300）")
        } else {
            Ok(Self(seconds))
        }
    }

    pub fn get(self) -> u32 {
        self.0
    }

    /// 秒数对应的时限。
    pub fn duration(self) -> Duration {
        Duration::from_secs(u64::from(self.0))
    }
}

impl TryFrom<u32> for CommandTimeoutSeconds {
    type Error = &'static str;

    fn try_from(seconds: u32) -> Result<Self, Self::Error> {
        Self::new(seconds)
    }
}

/// zone 登记设置（`[zones]` 段）。
///
/// 登记列表由后端以文件持久保存，电脑与手机读取同一份列表；路径相对进程工作目录，与既有配置的
/// 路径语义一致。登记文件不存在表示还没有任何登记，不是错误。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZonesConfig {
    /// 登记文件路径；缺省 `.data/zones.json`。
    #[serde(default = "registry_file_by_default")]
    pub registry_file: PathBuf,
    /// 新 zone 的创建父目录；缺省未配置，只禁用“新增”而不猜测写入位置。
    #[serde(default)]
    pub create_parent_directory: Option<PathBuf>,
    /// 单个文件文本预览的大小上限；缺省 1 MiB。超限只返回限制说明，不加载完整内容。
    #[serde(default = "preview_limit_bytes_by_default")]
    pub preview_limit_bytes: PreviewLimitBytes,
    /// 每个 zone 自动刷新合并事件的最小间隔；缺省 10 秒。
    #[serde(default = "refresh_interval_seconds_by_default")]
    pub refresh_interval_seconds: RefreshIntervalSeconds,
}

impl Default for ZonesConfig {
    fn default() -> Self {
        Self {
            registry_file: registry_file_by_default(),
            create_parent_directory: None,
            preview_limit_bytes: preview_limit_bytes_by_default(),
            refresh_interval_seconds: refresh_interval_seconds_by_default(),
        }
    }
}

/// Bot 模板设置（`[bot_templates]` 段）。
///
/// 模板由后端以文件持久保存，电脑与手机读取同一份；路径相对进程工作目录，与既有配置的
/// 路径语义一致。存储文件不存在表示还没有任何模板，不是错误。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BotTemplatesConfig {
    /// 模板存储文件路径；缺省 `.data/bot-templates.json`。
    #[serde(default = "storage_file_by_default")]
    pub storage_file: PathBuf,
    /// 手工维护的选项目录；相对进程工作目录，运行中每次操作重新读取。
    #[serde(default = "metadata_file_by_default")]
    pub metadata_file: PathBuf,
}

impl Default for BotTemplatesConfig {
    fn default() -> Self {
        Self {
            storage_file: storage_file_by_default(),
            metadata_file: metadata_file_by_default(),
        }
    }
}

fn metadata_file_by_default() -> PathBuf {
    PathBuf::from("meta-data/bot-options.toml")
}

/// `[bot_templates] storage_file` 的缺省值：进程工作目录下的 `.data/bot-templates.json`。
fn storage_file_by_default() -> PathBuf {
    PathBuf::from(crate::templates::DEFAULT_STORAGE_FILE)
}

/// `[zones] registry_file` 的缺省值：进程工作目录下的 `.data/zones.json`。
fn registry_file_by_default() -> PathBuf {
    PathBuf::from(DEFAULT_REGISTRY_FILE)
}

/// 缺省的单文件文本预览上限：1 MiB。
pub const DEFAULT_PREVIEW_LIMIT_BYTES: u64 = 1024 * 1024;

/// `[zones] preview_limit_bytes` 的缺省值：1 MiB。
fn preview_limit_bytes_by_default() -> PreviewLimitBytes {
    PreviewLimitBytes(DEFAULT_PREVIEW_LIMIT_BYTES)
}

/// 单个文件的文本预览上限（字节）：合法范围 1..=u64::MAX。
///
/// 解析本身承担范围检查：零会让任何文件都不可预览，负数、小数、字符串、布尔值都无法解析成
/// 无符号整数——每种情形都让**启动失败**，不做静默回退，也不用缺省值掩盖写错的配置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviewLimitBytes(u64);

impl PreviewLimitBytes {
    /// 构造一个合法的预览上限；零不是合法上限。
    pub fn new(bytes: u64) -> Result<Self, &'static str> {
        if bytes == 0 {
            Err("preview_limit_bytes 必须是正整数（默认 1048576）")
        } else {
            Ok(Self(bytes))
        }
    }

    /// 上限的字节数。
    pub fn get(self) -> u64 {
        self.0
    }
}

impl<'de> Deserialize<'de> for PreviewLimitBytes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes = u64::deserialize(deserializer)?;
        Self::new(bytes).map_err(de::Error::custom)
    }
}

/// 缺省的每 zone 自动刷新最小间隔：10 秒。
pub const DEFAULT_REFRESH_INTERVAL_SECONDS: u32 = 10;

/// `[zones] refresh_interval_seconds` 的缺省值：10 秒。
fn refresh_interval_seconds_by_default() -> RefreshIntervalSeconds {
    RefreshIntervalSeconds(DEFAULT_REFRESH_INTERVAL_SECONDS)
}

/// 每个 zone 自动刷新合并事件的最小间隔（秒）：合法范围 1..=4294967295。
///
/// 零会让窗口立刻结束、节流形同虚设；负数、小数、字符串与超过上界的值都无法解析成这个类型。
/// 每种非法情形都让**启动失败**，不静默回退到缺省值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefreshIntervalSeconds(u32);

impl RefreshIntervalSeconds {
    /// 构造一个合法的最小刷新间隔；零不是合法间隔。
    pub fn new(seconds: u32) -> Result<Self, &'static str> {
        if seconds == 0 {
            Err("refresh_interval_seconds 必须是 1 至 4294967295 的整数秒，0 不是合法的刷新间隔")
        } else {
            Ok(Self(seconds))
        }
    }

    /// 间隔秒数本身（1..=4294967295）。
    pub fn get(self) -> u32 {
        self.0
    }

    /// 秒数对应的时长。
    pub fn duration(self) -> Duration {
        Duration::from_secs(u64::from(self.0))
    }
}

impl<'de> Deserialize<'de> for RefreshIntervalSeconds {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let seconds = u32::deserialize(deserializer)?;
        Self::new(seconds).map_err(de::Error::custom)
    }
}

/// 关机设置（`[shutdown]` 段）。
///
/// 倒计时时长在**启动期**读取一次：它是服务端拥有的配置，手动关机与完成后关机监听共用同一个值，
/// 前端不再持有倒计时时长。运行中改写配置文件不会被重新读取（没有热加载），非法值在启动时就被拒绝。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShutdownConfig {
    /// 手动与完成后关机共用的倒计时秒数；缺省 10 秒。
    #[serde(default = "countdown_seconds_by_default")]
    pub countdown_seconds: CountdownSeconds,
}

impl Default for ShutdownConfig {
    fn default() -> Self {
        Self {
            countdown_seconds: countdown_seconds_by_default(),
        }
    }
}

/// `[shutdown] countdown_seconds` 的缺省值：缺段的旧配置得到 10 秒。
fn countdown_seconds_by_default() -> CountdownSeconds {
    CountdownSeconds(10)
}

/// 关机倒计时时长（秒）：合法范围 1..=4294967295。
///
/// 解析本身承担范围检查：零不是合法倒计时时长（会让关机立刻发生，等于没有安全窗口），
/// 负数、小数与超过 `u32` 上界的值都无法解析成这个类型——三种情形都让**启动失败**，
/// 不做静默回退，也不用缺省值掩盖写错的配置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CountdownSeconds(u32);

impl CountdownSeconds {
    /// 由秒数构造：零与超过 `u32` 上界的值不是合法倒计时时长。
    ///
    /// 配置解析与测试都用这一个入口，因此「什么算合法」只有一处定义。
    pub fn new(seconds: u32) -> Result<Self, &'static str> {
        if seconds == 0 {
            return Err(
                "countdown_seconds 必须是 1 至 4294967295 的整数秒，0 不是合法的倒计时时长",
            );
        }
        Ok(Self(seconds))
    }

    /// 秒数本身（1..=4294967295）。
    pub fn get(self) -> u32 {
        self.0
    }

    /// 秒数对应的时长。
    pub fn duration(self) -> Duration {
        Duration::from_secs(u64::from(self.0))
    }
}

impl<'de> Deserialize<'de> for CountdownSeconds {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // `u32` 解析已经排除负数、小数与超过上界的值（它们都不是 32 位无符号整数）。
        let seconds = u32::deserialize(deserializer)?;
        Self::new(seconds).map_err(de::Error::custom)
    }
}

/// 会话设置（`[sessions]` 段）。
///
/// 两个窗口在**启动期**读取一次，分别给出「登记最近一次上报仍算在线」与「上报过期后登记仍保留」的时长；
/// 快照（读路径）、写入与维护扫描共用 [`SessionsConfig::policy`] 转换出的同一份 [`SessionPolicy`]，
/// 运行中改写配置文件不会被重新读取（没有热加载），非法值在启动时就被拒绝。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionsConfig {
    /// 在线有效期秒数；缺省 10 秒。
    #[serde(default = "online_timeout_seconds_by_default")]
    pub online_timeout_seconds: SessionWindowSeconds,
    /// 离线保留期秒数；缺省 30 秒。
    #[serde(default = "offline_retention_seconds_by_default")]
    pub offline_retention_seconds: SessionWindowSeconds,
}

impl Default for SessionsConfig {
    fn default() -> Self {
        Self {
            online_timeout_seconds: online_timeout_seconds_by_default(),
            offline_retention_seconds: offline_retention_seconds_by_default(),
        }
    }
}

impl SessionsConfig {
    /// 两个窗口对应的登记生命周期策略。
    ///
    /// 这是「配置值 → 实例行为」的唯一转换点：会话表只按它判定在线、离线与清除，因此启动期读到的窗口
    /// 贯通快照读取、维护与监听裁定。
    pub fn policy(&self) -> SessionPolicy {
        SessionPolicy::from_seconds(
            self.online_timeout_seconds.get(),
            self.offline_retention_seconds.get(),
        )
    }
}

/// `[sessions] online_timeout_seconds` 的缺省值：缺字段（或整段）得到 10 秒。
fn online_timeout_seconds_by_default() -> SessionWindowSeconds {
    SessionWindowSeconds(DEFAULT_ONLINE_SECONDS)
}

/// `[sessions] offline_retention_seconds` 的缺省值：缺字段（或整段）得到 30 秒。
fn offline_retention_seconds_by_default() -> SessionWindowSeconds {
    SessionWindowSeconds(DEFAULT_OFFLINE_RETENTION_SECONDS)
}

/// 会话窗口时长（秒）：合法范围 1..=4294967295。
///
/// 解析本身承担范围检查：零会让窗口失去意义，负数、小数、字符串、布尔值与超过 `u32` 上界的值都无法
/// 解析成这个类型——每一种情形都让**启动失败**，不做静默回退，也不用缺省值掩盖写错的配置。
/// 出错的字段名由解析器在错误里给出（TOML 错误会指出出错的键），因此两个窗口共用这一个类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionWindowSeconds(u32);

impl SessionWindowSeconds {
    /// 由秒数构造：零与超过 `u32` 上界的值不是合法窗口。
    pub fn new(seconds: u32) -> Result<Self, &'static str> {
        if seconds == 0 {
            return Err("会话窗口必须是 1 至 4294967295 的整数秒，0 不是合法的窗口时长");
        }
        Ok(Self(seconds))
    }

    /// 秒数本身（1..=4294967295）。
    pub fn get(self) -> u32 {
        self.0
    }
}

impl<'de> Deserialize<'de> for SessionWindowSeconds {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // `u32` 解析已经排除负数、小数、字符串、布尔值与超过上界的值。
        let seconds = u32::deserialize(deserializer)?;
        Self::new(seconds).map_err(de::Error::custom)
    }
}

/// mDNS 发布设置（`[mdns]` 段）。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MdnsConfig {
    /// 是否在启动时发布 mDNS 主机名与服务记录；缺省为 `true`。
    #[serde(default = "mdns_enabled_by_default")]
    pub enabled: bool,
    /// 覆盖要发布的 IPv4；缺省时由后端自动选择主路由 IPv4。
    #[serde(default)]
    pub address: Option<Ipv4Addr>,
}

impl Default for MdnsConfig {
    fn default() -> Self {
        Self {
            enabled: mdns_enabled_by_default(),
            address: None,
        }
    }
}

/// `[mdns] enabled` 的缺省值：发布默认开启。
fn mdns_enabled_by_default() -> bool {
    true
}

/// herdr-bot 主界面允许执行的固定操作集合：只有“启动”与“停止”两个，目标只能来自配置。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operations {
    /// “启动”操作。
    pub start: OperationBinding,
    /// “停止”操作。
    pub stop: OperationBinding,
}

/// 一个固定操作的服务端绑定：批处理文件与其执行工作目录。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationBinding {
    pub script: PathBuf,
    pub working_directory: PathBuf,
}

impl Config {
    /// 从 TOML 配置文件读取配置。
    pub fn load(path: &Path) -> anyhow::Result<Config> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("读取配置文件失败：{}", path.display()))?;
        Config::from_toml(&raw).with_context(|| format!("解析配置文件失败：{}", path.display()))
    }

    /// 解析 TOML 配置文本，并拒绝没有明确目标 Workspace 的部署。
    ///
    /// `[herdr] workspace_name` 缺失由 TOML 反序列化直接拒绝；空串与纯空白在这里拒绝，避免把
    /// 空白当作一个可能匹配到其他 Workspace 的名称。合法值去掉首尾空白后按字面保存。
    pub fn from_toml(raw: &str) -> anyhow::Result<Config> {
        let mut config: Config = toml::from_str(raw)?;
        config.herdr.workspace_name = config.herdr.workspace_name.trim().to_string();
        if config.herdr.workspace_name.is_empty() {
            anyhow::bail!(
                "[herdr] workspace_name 不能为空或纯空白：必须在服务端配置中明确指定目标 Herdr Workspace。"
            );
        }
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 除 `[herdr]` 外的最小合法配置；用于观察 `[herdr]` 段本身的缺失与字段校验。
    const BASE_CONFIG: &str = r#"
listen_addr = "0.0.0.0:8787"

[operations.start]
script = 'C:\start.bat'
working_directory = 'C:\work'

[operations.stop]
script = 'C:\stop.bat'
working_directory = 'C:\work'
"#;

    /// 旧配置（没有任何 `[mdns]` 段）必须仍能解析，且缺段等于默认启用、地址自动选择。
    const LEGACY_CONFIG: &str = r#"
listen_addr = "0.0.0.0:8787"

[operations.start]
script = 'C:\start.bat'
working_directory = 'C:\work'

[operations.stop]
script = 'C:\stop.bat'
working_directory = 'C:\work'

[herdr]
workspace_name = 'bot-playground'
"#;

    #[test]
    fn legacy_config_without_git_section_defaults_to_five_minutes() {
        let config = Config::from_toml(LEGACY_CONFIG).expect("旧配置必须仍可解析");
        assert_eq!(
            config.git.command_timeout_seconds.get(),
            DEFAULT_COMMAND_TIMEOUT_SECONDS,
            "缺省 [git] 段时每条 Git 命令最多执行 300 秒"
        );
        assert_eq!(
            config.git.command_timeout_seconds.duration(),
            Duration::from_secs(300)
        );
    }

    #[test]
    fn explicit_git_command_timeout_is_read_verbatim() {
        let raw = format!("{LEGACY_CONFIG}\n[git]\ncommand_timeout_seconds = 7\n");
        let config = Config::from_toml(&raw).expect("合法 Git 命令时限必须可解析");
        assert_eq!(config.git.command_timeout_seconds.get(), 7);
    }

    #[test]
    fn git_section_with_only_other_defaults_keeps_timeout_default() {
        let raw = format!("{LEGACY_CONFIG}\n[git]\n");
        let config = Config::from_toml(&raw).expect("空的 [git] 段必须可解析");
        assert_eq!(
            config.git.command_timeout_seconds.get(),
            DEFAULT_COMMAND_TIMEOUT_SECONDS
        );
    }

    #[test]
    fn illegal_git_command_timeouts_are_rejected_instead_of_falling_back() {
        for (literal, why) in [
            ("0", "零不是合法的执行时限"),
            ("-1", "负数不是秒数"),
            ("1.5", "小数不是整秒"),
            ("'300'", "字符串不是数字"),
        ] {
            let raw = format!("{LEGACY_CONFIG}\n[git]\ncommand_timeout_seconds = {literal}\n");
            let error = Config::from_toml(&raw)
                .err()
                .unwrap_or_else(|| panic!("{why}，必须导致启动配置解析失败：{literal}"));
            assert!(
                error.to_string().contains("command_timeout_seconds"),
                "解析错误应指出出错的字段，实际：{error}"
            );
        }
    }

    #[test]
    fn misspelled_git_command_timeout_field_is_rejected() {
        let raw = format!("{LEGACY_CONFIG}\n[git]\ncommand_timeout_second = 7\n");
        let error = Config::from_toml(&raw).expect_err("写错的字段名必须导致解析失败");
        assert!(
            error.to_string().contains("unknown field")
                && error.to_string().contains("`command_timeout_second`"),
            "解析错误应指出未知字段名，实际：{error}"
        );
    }

    #[test]
    fn legacy_config_without_mdns_section_parses_with_publishing_enabled_by_default() {
        let config = Config::from_toml(LEGACY_CONFIG).expect("旧配置必须仍可解析");
        assert!(config.mdns.enabled, "缺省 [mdns] 段等于默认启用发布");
        assert_eq!(config.mdns.address, None, "缺省时不指定覆盖地址");
    }

    #[test]
    fn mdns_enabled_and_address_are_read_when_present() {
        let raw = format!("{LEGACY_CONFIG}\n[mdns]\nenabled = false\naddress = \"192.168.5.4\"\n");
        let config = Config::from_toml(&raw).expect("带 [mdns] 段的新配置必须可解析");
        assert!(!config.mdns.enabled);
        assert_eq!(config.mdns.address, Some(Ipv4Addr::new(192, 168, 5, 4)));
    }

    #[test]
    fn mdns_section_with_only_address_keeps_publishing_enabled_by_default() {
        let raw = format!("{LEGACY_CONFIG}\n[mdns]\naddress = \"10.0.0.7\"\n");
        let config = Config::from_toml(&raw).expect("只写 address 的 [mdns] 段必须可解析");
        assert!(config.mdns.enabled, "只写 address 时 enabled 仍取默认 true");
        assert_eq!(config.mdns.address, Some(Ipv4Addr::new(10, 0, 0, 7)));
    }

    #[test]
    fn misspelled_mdns_field_name_is_rejected() {
        let raw = format!("{LEGACY_CONFIG}\n[mdns]\nenable = false\n");
        let error = Config::from_toml(&raw).expect_err("写错的字段名必须导致解析失败");
        assert!(
            error.to_string().contains("enable"),
            "解析错误应指出未知字段名，实际：{error}"
        );
    }

    #[test]
    fn unknown_top_level_field_is_still_rejected() {
        let raw = format!("{LEGACY_CONFIG}\nunknown_field = 1\n");
        assert!(
            Config::from_toml(&raw).is_err(),
            "顶层未知字段必须仍然解析失败（deny_unknown_fields 保持）"
        );
    }

    #[test]
    fn config_without_zones_section_defaults_to_centralized_registry() {
        let config = Config::from_toml(LEGACY_CONFIG).expect("旧配置必须仍可解析");
        assert_eq!(
            config.zones.registry_file,
            PathBuf::from(".data/zones.json"),
            "缺省 [zones] 段时登记文件取工作目录下的 .data/zones.json"
        );
    }

    #[test]
    fn legacy_config_without_create_parent_directory_disables_zone_creation() {
        let config = Config::from_toml(LEGACY_CONFIG).expect("旧配置必须仍可解析");
        assert_eq!(
            config.zones.create_parent_directory, None,
            "没有显式配置时不能猜测一个创建父目录"
        );
    }

    #[test]
    fn explicit_create_parent_directory_is_read_verbatim() {
        let raw = format!(
            "{LEGACY_CONFIG}\n[zones]\ncreate_parent_directory = 'E:\\Developer\\bot-playground'\n"
        );
        let config = Config::from_toml(&raw).expect("显式创建父目录必须可解析");
        assert_eq!(
            config.zones.create_parent_directory,
            Some(PathBuf::from(r"E:\Developer\bot-playground"))
        );
    }

    #[test]
    fn misspelled_create_parent_directory_field_is_rejected() {
        let raw =
            format!("{LEGACY_CONFIG}\n[zones]\ncreate_parent = 'E:\\Developer\\bot-playground'\n");
        assert!(
            Config::from_toml(&raw).is_err(),
            "创建父目录字段拼错时必须导致解析失败"
        );
    }

    #[test]
    fn zones_registry_file_is_read_when_present() {
        let raw = format!("{LEGACY_CONFIG}\n[zones]\nregistry_file = 'E:\\data\\zones.json'\n");
        let config = Config::from_toml(&raw).expect("带 [zones] 段的新配置必须可解析");
        assert_eq!(
            config.zones.registry_file,
            PathBuf::from(r"E:\data\zones.json")
        );
    }

    #[test]
    fn misspelled_zones_field_name_is_rejected() {
        let raw = format!("{LEGACY_CONFIG}\n[zones]\nregistry = 'x.json'\n");
        assert!(
            Config::from_toml(&raw).is_err(),
            "[zones] 里写错字段名必须导致解析失败"
        );
    }

    #[test]
    fn legacy_config_without_preview_limit_defaults_to_one_mebibyte() {
        let config = Config::from_toml(LEGACY_CONFIG).expect("旧配置必须仍可解析");
        assert_eq!(
            config.zones.preview_limit_bytes.get(),
            DEFAULT_PREVIEW_LIMIT_BYTES,
            "缺省 [zones] 段时预览上限取 1 MiB"
        );

        let raw = format!("{LEGACY_CONFIG}\n[zones]\nregistry_file = 'x.json'\n");
        let config = Config::from_toml(&raw).expect("只有 registry_file 的配置必须可解析");
        assert_eq!(
            config.zones.preview_limit_bytes.get(),
            DEFAULT_PREVIEW_LIMIT_BYTES,
            "缺少 preview_limit_bytes 字段时取 1 MiB"
        );
    }

    #[test]
    fn explicit_preview_limit_is_read_verbatim() {
        let raw = format!("{LEGACY_CONFIG}\n[zones]\npreview_limit_bytes = 2097152\n");
        let config = Config::from_toml(&raw).expect("合法预览上限必须可解析");
        assert_eq!(config.zones.preview_limit_bytes.get(), 2_097_152);
    }

    #[test]
    fn preview_limit_with_wrong_type_is_rejected() {
        let raw = format!("{LEGACY_CONFIG}\n[zones]\npreview_limit_bytes = '1 MiB'\n");
        let error = Config::from_toml(&raw).expect_err("非整数预览上限必须导致解析失败");
        assert!(
            error.to_string().contains("preview_limit_bytes"),
            "解析错误应指出出错的字段，实际：{error}"
        );
    }

    #[test]
    fn zero_preview_limit_is_rejected_instead_of_falling_back() {
        let raw = format!("{LEGACY_CONFIG}\n[zones]\npreview_limit_bytes = 0\n");
        let error = Config::from_toml(&raw).expect_err("零预览上限必须导致解析失败");
        assert!(
            error.to_string().contains("preview_limit_bytes"),
            "解析错误应指出上限非法，实际：{error}"
        );
    }

    #[test]
    fn negative_preview_limit_is_rejected() {
        let raw = format!("{LEGACY_CONFIG}\n[zones]\npreview_limit_bytes = -1\n");
        assert!(
            Config::from_toml(&raw).is_err(),
            "负预览上限必须导致解析失败"
        );
    }

    #[test]
    fn legacy_config_without_refresh_interval_defaults_to_ten_seconds() {
        let config = Config::from_toml(LEGACY_CONFIG).expect("旧配置必须仍可解析");
        assert_eq!(
            config.zones.refresh_interval_seconds.get(),
            DEFAULT_REFRESH_INTERVAL_SECONDS,
            "缺省 [zones] 段时自动刷新最小间隔取 10 秒"
        );

        let raw = format!("{LEGACY_CONFIG}\n[zones]\nregistry_file = 'x.json'\n");
        let config = Config::from_toml(&raw).expect("只有 registry_file 的配置必须可解析");
        assert_eq!(
            config.zones.refresh_interval_seconds.get(),
            DEFAULT_REFRESH_INTERVAL_SECONDS,
            "缺少 refresh_interval_seconds 字段时取 10 秒"
        );
    }

    #[test]
    fn explicit_refresh_interval_is_read_verbatim() {
        let raw = format!("{LEGACY_CONFIG}\n[zones]\nrefresh_interval_seconds = 25\n");
        let config = Config::from_toml(&raw).expect("合法自动刷新间隔必须可解析");
        assert_eq!(config.zones.refresh_interval_seconds.get(), 25);
        assert_eq!(
            config.zones.refresh_interval_seconds.duration(),
            Duration::from_secs(25)
        );
    }

    #[test]
    fn refresh_interval_accepts_the_legal_lower_bound() {
        let raw = format!("{LEGACY_CONFIG}\n[zones]\nrefresh_interval_seconds = 1\n");
        let config = Config::from_toml(&raw).expect("最小合法自动刷新间隔必须可解析");
        assert_eq!(config.zones.refresh_interval_seconds.get(), 1);
    }

    #[test]
    fn zero_refresh_interval_is_rejected_instead_of_falling_back() {
        let raw = format!("{LEGACY_CONFIG}\n[zones]\nrefresh_interval_seconds = 0\n");
        let error = Config::from_toml(&raw).expect_err("零自动刷新间隔必须导致解析失败");
        assert!(
            error.to_string().contains("refresh_interval_seconds"),
            "解析错误应指出间隔非法，实际：{error}"
        );
    }

    #[test]
    fn negative_refresh_interval_is_rejected() {
        let raw = format!("{LEGACY_CONFIG}\n[zones]\nrefresh_interval_seconds = -1\n");
        assert!(
            Config::from_toml(&raw).is_err(),
            "负自动刷新间隔必须导致解析失败"
        );
    }

    #[test]
    fn refresh_interval_with_wrong_type_is_rejected() {
        let raw = format!("{LEGACY_CONFIG}\n[zones]\nrefresh_interval_seconds = '10s'\n");
        let error = Config::from_toml(&raw).expect_err("非整数自动刷新间隔必须导致解析失败");
        assert!(
            error.to_string().contains("refresh_interval_seconds"),
            "解析错误应指出出错的字段，实际：{error}"
        );
    }

    /// 缺 `[shutdown]` 段的旧配置必须仍能加载，且倒计时取缺省的 10 秒。
    #[test]
    fn legacy_config_without_shutdown_section_defaults_to_ten_seconds() {
        let config = Config::from_toml(LEGACY_CONFIG).expect("旧配置必须仍可解析");
        assert_eq!(
            config.shutdown.countdown_seconds.get(),
            10,
            "缺段等于缺省 10 秒"
        );
        assert_eq!(
            config.shutdown.countdown_seconds.duration(),
            Duration::from_secs(10)
        );
    }

    /// 显式配置的秒数被如实读取（这里用的是本票断言里那个独立字面量 3）。
    #[test]
    fn explicit_countdown_seconds_are_read_verbatim() {
        let raw = format!("{LEGACY_CONFIG}\n[shutdown]\ncountdown_seconds = 3\n");
        let config = Config::from_toml(&raw).expect("显式配置必须可解析");
        assert_eq!(config.shutdown.countdown_seconds.get(), 3);
    }

    /// 合法范围的上下界都接受：1 与 4294967295。
    #[test]
    fn countdown_seconds_accepts_the_legal_bounds() {
        for seconds in [1u32, 4294967295] {
            let raw = format!("{LEGACY_CONFIG}\n[shutdown]\ncountdown_seconds = {seconds}\n");
            let config = Config::from_toml(&raw)
                .unwrap_or_else(|e| panic!("{seconds} 必须是合法的倒计时秒数：{e}"));
            assert_eq!(config.shutdown.countdown_seconds.get(), seconds);
        }
    }

    /// 非法值必须明确拒绝启动，而不是静默回退到缺省：零、负数、小数与超过上界。
    #[test]
    fn illegal_countdown_seconds_are_rejected_instead_of_falling_back() {
        for (literal, why) in [
            ("0", "零会让关机立刻发生，等于没有安全窗口"),
            ("-1", "负数不是秒数"),
            ("0.5", "小数不是整数秒"),
            ("4294967296", "超过 u32 上界"),
        ] {
            let raw = format!("{LEGACY_CONFIG}\n[shutdown]\ncountdown_seconds = {literal}\n");
            let error = Config::from_toml(&raw)
                .err()
                .unwrap_or_else(|| panic!("{literal}（{why}）必须让配置解析失败"));
            assert!(
                error.to_string().contains("countdown_seconds"),
                "拒绝必须指出出错的字段（{literal}，{why}）：{error}"
            );
        }
    }

    /// 拼错的字段名（`[shutdown]` 段内的未知字段）必须明确拒绝，而不是被忽略后取缺省值。
    #[test]
    fn misspelled_shutdown_field_name_is_rejected() {
        let raw = format!("{LEGACY_CONFIG}\n[shutdown]\ncountdown_second = 3\n");
        let error = Config::from_toml(&raw).expect_err("写错的字段名必须导致解析失败");
        assert!(
            error.to_string().contains("countdown_second"),
            "解析错误应指出未知字段名，实际：{error}"
        );
    }

    /// 缺段与写错段名同样区分：把段名拼错时，倒计时不会悄悄取缺省值。
    #[test]
    fn misspelled_shutdown_section_is_rejected() {
        let raw = format!("{LEGACY_CONFIG}\n[shutdowns]\ncountdown_seconds = 3\n");
        assert!(
            Config::from_toml(&raw).is_err(),
            "拼错的段名必须让解析失败（顶层 deny_unknown_fields）"
        );
    }

    /// 缺 `[sessions]` 段的旧配置必须仍能加载，且两个窗口取缺省的 10／30 秒。
    #[test]
    fn legacy_config_without_sessions_section_defaults_to_ten_and_thirty_seconds() {
        let config = Config::from_toml(LEGACY_CONFIG).expect("旧配置必须仍可解析");
        assert_eq!(
            config.sessions.online_timeout_seconds.get(),
            10,
            "缺段等于缺省在线有效期 10 秒"
        );
        assert_eq!(
            config.sessions.offline_retention_seconds.get(),
            30,
            "缺段等于缺省离线保留期 30 秒"
        );
        assert_eq!(
            config.sessions.policy(),
            SessionPolicy::from_seconds(10, 30),
            "缺段得到的策略就是缺省 10／30"
        );
    }

    /// 只写一个字段时，只有缺的那个字段取默认值，写下的字段如实生效。
    #[test]
    fn only_the_missing_session_window_falls_back_to_its_default() {
        let online_only = Config::from_toml(&format!(
            "{LEGACY_CONFIG}\n[sessions]\nonline_timeout_seconds = 7\n"
        ))
        .expect("只写 online_timeout_seconds 必须可解析");
        assert_eq!(online_only.sessions.online_timeout_seconds.get(), 7);
        assert_eq!(online_only.sessions.offline_retention_seconds.get(), 30);

        let retention_only = Config::from_toml(&format!(
            "{LEGACY_CONFIG}\n[sessions]\noffline_retention_seconds = 13\n"
        ))
        .expect("只写 offline_retention_seconds 必须可解析");
        assert_eq!(retention_only.sessions.online_timeout_seconds.get(), 10);
        assert_eq!(retention_only.sessions.offline_retention_seconds.get(), 13);
    }

    /// 两个字段的合法上下界都接受：1 与 4294967295，都如实保留。
    #[test]
    fn session_windows_accept_the_legal_bounds() {
        for seconds in [1u32, 4294967295] {
            let online = Config::from_toml(&format!(
                "{LEGACY_CONFIG}\n[sessions]\nonline_timeout_seconds = {seconds}\n"
            ))
            .unwrap_or_else(|e| panic!("{seconds} 必须是合法的在线有效期：{e}"));
            assert_eq!(online.sessions.online_timeout_seconds.get(), seconds);

            let retention = Config::from_toml(&format!(
                "{LEGACY_CONFIG}\n[sessions]\noffline_retention_seconds = {seconds}\n"
            ))
            .unwrap_or_else(|e| panic!("{seconds} 必须是合法的离线保留期：{e}"));
            assert_eq!(retention.sessions.offline_retention_seconds.get(), seconds);
        }
    }

    /// 非法值必须明确拒绝启动，而不是静默回退到缺省：零、负数、小数、字符串、布尔值与超过上界。
    ///
    /// 两个字段都要覆盖同一种非法输入，且错误必须点到出错的字段上。
    #[test]
    fn illegal_session_windows_are_rejected_instead_of_falling_back() {
        for field in ["online_timeout_seconds", "offline_retention_seconds"] {
            for (literal, why) in [
                ("0", "零不是合法的窗口时长"),
                ("-1", "负数不是秒数"),
                ("0.5", "小数不是整数秒"),
                ("\"10\"", "字符串不是整数秒"),
                ("true", "布尔值不是整数秒"),
                ("4294967296", "超过 u32 上界"),
            ] {
                let raw = format!("{LEGACY_CONFIG}\n[sessions]\n{field} = {literal}\n");
                let error = Config::from_toml(&raw)
                    .err()
                    .unwrap_or_else(|| panic!("{field} = {literal}（{why}）必须让配置解析失败"));
                assert!(
                    error.to_string().contains(field),
                    "拒绝必须指出出错的字段（{field} = {literal}，{why}）：{error}"
                );
            }
        }
    }

    /// 拼错的字段名（`[sessions]` 段内的未知字段）必须明确拒绝，而不是被忽略后取缺省值。
    #[test]
    fn misspelled_session_field_name_is_rejected() {
        let raw = format!("{LEGACY_CONFIG}\n[sessions]\nonline_timeout_second = 7\n");
        let error = Config::from_toml(&raw).expect_err("写错的字段名必须导致解析失败");
        assert!(
            error.to_string().contains("unknown field")
                && error.to_string().contains("`online_timeout_second`"),
            "解析错误应指出未知字段名，实际：{error}"
        );
    }

    /// 缺段与写错段名同样区分：把段名拼错时，两个窗口不会悄悄取缺省值。
    #[test]
    fn misspelled_sessions_section_is_rejected() {
        let raw = format!("{LEGACY_CONFIG}\n[session]\nonline_timeout_seconds = 7\n");
        let error = Config::from_toml(&raw).expect_err("拼错的段名必须让解析失败");
        assert!(
            error.to_string().contains("unknown field") && error.to_string().contains("`session`"),
            "解析错误应指出未知的段名，实际：{error}"
        );
    }

    /// Bot 模板存储文件缺省为当前工作目录下的 .data/bot-templates.json。
    #[test]
    fn legacy_config_without_bot_templates_section_uses_default_storage_file() {
        let config = Config::from_toml(LEGACY_CONFIG).expect("旧配置必须仍可解析");
        assert_eq!(
            config.bot_templates.storage_file,
            PathBuf::from(".data/bot-templates.json")
        );
    }

    /// 显式配置的模板存储路径按字面读取；拼错的字段名明确失败。
    #[test]
    fn explicit_bot_templates_storage_file_is_read_verbatim() {
        let raw = format!(
            "{LEGACY_CONFIG}\n[bot_templates]\nstorage_file = 'D:\\\\templates\\\\bot-templates.json'\n"
        );
        let config = Config::from_toml(&raw).expect("合法模板存储路径必须可解析");
        assert_eq!(
            config.bot_templates.storage_file,
            PathBuf::from(r"D:\templates\bot-templates.json")
        );

        let misspelled = format!("{LEGACY_CONFIG}\n[bot_templates]\nstoragefile = 'x.json'\n");
        let error = Config::from_toml(&misspelled).expect_err("写错的字段名必须导致解析失败");
        assert!(
            error.to_string().contains("unknown field")
                && error.to_string().contains("`storagefile`"),
            "解析错误应指出未知字段名，实际：{error}"
        );
    }

    /// 目标 Workspace 没有代码缺省值：整个 `[herdr]` 段缺失时启动配置必须失败。
    #[test]
    fn config_without_herdr_section_is_rejected() {
        let error = Config::from_toml(BASE_CONFIG).expect_err("[herdr] 缺失必须让启动配置失败");
        assert!(
            error.to_string().contains("herdr"),
            "解析错误应指出缺失的段，实际：{error}"
        );
    }

    /// 空串与纯空白都不是合法目标；合法值去掉首尾空白后按字面保存。
    #[test]
    fn herdr_workspace_name_must_be_explicit_and_non_blank() {
        for literal in ["''", "'   '", "\"\\t\\n\"", "\" \\t \""] {
            let raw = format!("{BASE_CONFIG}\n[herdr]\nworkspace_name = {literal}\n");
            let error = Config::from_toml(&raw)
                .err()
                .unwrap_or_else(|| panic!("workspace_name = {literal} 必须让启动配置失败"));
            assert!(
                error.to_string().contains("workspace_name"),
                "拒绝必须指出出错的字段，实际：{error}"
            );
        }

        let raw = format!("{BASE_CONFIG}\n[herdr]\nworkspace_name = '  explicit-target  '\n");
        let config = Config::from_toml(&raw).expect("合法显式名称必须可解析");
        assert_eq!(config.herdr.workspace_name, "explicit-target");
        assert_eq!(config.herdr.executable, PathBuf::from("herdr"));
    }

    /// 拼错 `[herdr]` 字段或段内未知字段必须明确失败，不能悄悄忽略。
    #[test]
    fn misspelled_herdr_field_is_rejected() {
        let raw = format!("{BASE_CONFIG}\n[herdr]\nworkspace = 'bot-playground'\n");
        let error = Config::from_toml(&raw).expect_err("写错的字段名必须导致解析失败");
        assert!(
            error.to_string().contains("unknown field")
                && error.to_string().contains("`workspace`"),
            "解析错误应指出未知字段名，实际：{error}"
        );
    }
}
