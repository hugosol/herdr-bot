//! 浏览器／人工验收用的**安全外壳**：与交付运行时同一份路由，但关机动作是可替换接缝的记录型替身。
//!
//! 交付运行时（`src/main.rs`）永远使用真实的 Win32 动作；而真实浏览器驱动的手动确认／取消／结果流程
//! 需要一个能走到「到期执行」的后端。因此这个示例只服务安全验收：
//!
//! - 关机动作由记录型替身承担：只打印调用序号与结果，**不接触任何系统调用**；
//! - 命令（开始／取消）、启动期配置、共享观察流与交付运行时完全一致；
//! - `--outcome failed` 让替身回报失败，用来观察失败结果页；缺省 `ok`。
//!
//! 用法：
//!
//! ```text
//! cargo run --example safe_shutdown_shell -- --config <path> [--outcome ok|failed]
//! ```
//!
//! 它也是自动化真实进程验证启动的进程：`backend/tests/manual_shutdown_process.rs` 用同一个安全壳
//! 证明「登记、监听意图与安排都不跨进程恢复」，因此那里同样只驱动这个记录型替身，绝不调用真实关机。
//! 它不是生产开关：真实二进制不认识这些参数，也不会因为这里的存在而改变行为。

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Context;

/// 记录型关机动作：只记录调用次数并打印，不接触系统关机。
struct RecordingPowerOff {
    calls: AtomicUsize,
    fails: bool,
}

impl omp_bot::PowerOff for RecordingPowerOff {
    fn power_off(&self) -> Result<(), omp_bot::PowerOffFailure> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        let outcome = if self.fails { "failed" } else { "ok" };
        println!("[omp-bot] 安全壳：记录到关机动作调用 #{call}（结果 {outcome}）");
        if self.fails {
            Err(omp_bot::PowerOffFailure::Privilege(
                "安全壳的失败结果（不会真的关机）".to_string(),
            ))
        } else {
            Ok(())
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let (config_path, fails) = arguments()?;
    let config = omp_bot::Config::load(&config_path)?;

    let listener = tokio::net::TcpListener::bind(&config.listen_addr)
        .await
        .with_context(|| format!("绑定监听地址失败：{}", config.listen_addr))?;
    let local_addr = listener.local_addr().context("读取监听地址失败")?;

    println!("[omp-bot] 安全壳已启动（关机动作为记录型替身，绝不真的关机）");
    println!("[omp-bot] 配置文件：{}", config_path.display());
    println!(
        "[omp-bot] 倒计时配置：{} 秒",
        config.shutdown.countdown_seconds.get()
    );
    println!(
        "[omp-bot] 会话窗口：在线 {} 秒、离线保留 {} 秒",
        config.sessions.online_timeout_seconds.get(),
        config.sessions.offline_retention_seconds.get()
    );
    println!("[omp-bot] 到期结果：{}", if fails { "failed" } else { "ok" });
    println!("[omp-bot] 监听：http://{local_addr}/");

    let sessions = Arc::new(omp_bot::sessions::SessionsState::with_policy(
        config.sessions.policy(),
    ));
    let power_off = Arc::new(RecordingPowerOff {
        calls: AtomicUsize::new(0),
        fails,
    });
    let (app, state) = omp_bot::app_with_state(&config, power_off, Arc::clone(&sessions));
    omp_bot::sessions::spawn_expiry_scan(sessions, Arc::clone(&state.shutdown));

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await
    .context("Axum 服务异常退出")
}

/// 解析 `--config <path>` 与可选的 `--outcome ok|failed`。
fn arguments() -> anyhow::Result<(PathBuf, bool)> {
    let mut args = std::env::args().skip(1);
    let mut config_path = PathBuf::from("omp-bot.toml");
    let mut fails = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => {
                config_path = PathBuf::from(args.next().context("--config 需要一个文件路径参数")?);
            }
            "--outcome" => {
                let value = args.next().context("--outcome 需要 ok 或 failed")?;
                fails = match value.as_str() {
                    "ok" => false,
                    "failed" => true,
                    other => anyhow::bail!("--outcome 只能是 ok 或 failed，收到：{other}"),
                };
            }
            other => anyhow::bail!("未知参数：{other}（用法：safe_shutdown_shell [--config <path>] [--outcome ok|failed]）"),
        }
    }
    Ok((config_path, fails))
}
