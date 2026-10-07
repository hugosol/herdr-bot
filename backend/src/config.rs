//! 服务端配置：监听地址、固定脚本操作的绑定与关机倒计时时长。
//!
//! 脚本目标与工作目录只来自这里的配置，不作为 HTTP 请求参数，客户端无法替换。
//! 关机倒计时时长（`[shutdown] countdown_seconds`）同样是启动期配置：前端不再持有倒计时时长，
//! 配置在进程启动时读取一次，运行中改写文件不会被重新读取。

use std::fs;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Context;
use serde::de::{self, Deserializer};
use serde::Deserialize;

/// omp-bot 主界面运行配置。
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
            return Err("countdown_seconds 必须是 1 至 4294967295 的整数秒，0 不是合法的倒计时时长");
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

/// omp-bot 主界面允许执行的固定操作集合：只有“启动”与“停止”两个，目标只能来自配置。
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

    /// 解析 TOML 配置文本。
    pub fn from_toml(raw: &str) -> anyhow::Result<Config> {
        Ok(toml::from_str(raw)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 旧配置（没有任何 `[mdns]` 段）必须仍能解析，且缺段等于默认启用、地址自动选择。
    const LEGACY_CONFIG: &str = r#"
listen_addr = "0.0.0.0:8787"

[operations.start]
script = 'C:\start.bat'
working_directory = 'C:\work'

[operations.stop]
script = 'C:\stop.bat'
working_directory = 'C:\work'
"#;

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
}
