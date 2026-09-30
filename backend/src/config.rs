//! 服务端配置：监听地址与固定脚本操作的绑定。
//!
//! 脚本目标与工作目录只来自这里的配置，不作为 HTTP 请求参数，客户端无法替换。

use std::fs;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::Deserialize;

/// 控制面板运行配置。
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

/// 控制面板允许执行的固定操作集合：只有“启动”与“停止”两个，目标只能来自配置。
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
}
