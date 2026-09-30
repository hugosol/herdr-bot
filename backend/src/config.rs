//! 服务端配置：监听地址与固定脚本操作的绑定。
//!
//! 脚本目标与工作目录只来自这里的配置，不作为 HTTP 请求参数，客户端无法替换。

use std::fs;
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
