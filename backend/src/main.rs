//! 手动启动入口：读取配置，绑定非回环监听地址，提供页面与操作接口。

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config_path = config_path_from_args()?;
    let config = herdr_bot::Config::load(&config_path)?;

    let static_dir = herdr_bot::static_dir();
    if !static_dir.join("index.html").is_file() {
        eprintln!(
            "[herdr-bot] 警告：未找到打包页面 {}，请先在 frontend 目录执行 npm run build。",
            static_dir.join("index.html").display()
        );
    }

    let listener = tokio::net::TcpListener::bind(&config.listen_addr)
        .await
        .with_context(|| format!("绑定监听地址失败：{}", config.listen_addr))?;
    let local_addr = listener.local_addr().context("读取监听地址失败")?;

    println!("[herdr-bot] 主界面已启动");
    println!("[herdr-bot] 配置文件：{}", config_path.display());
    for (label, binding) in [
        ("启动", &config.operations.start),
        ("停止", &config.operations.stop),
    ] {
        println!(
            "[herdr-bot] {label}操作：{}（工作目录 {}）",
            binding.script.display(),
            binding.working_directory.display()
        );
    }
    println!("[herdr-bot] 打包页面：{}", static_dir.display());
    println!(
        "[herdr-bot] 会话窗口：在线 {} 秒、离线保留 {} 秒",
        config.sessions.online_timeout_seconds.get(),
        config.sessions.offline_retention_seconds.get()
    );
    println!(
        "[herdr-bot] zone 登记文件：{}",
        config.zones.registry_file.display()
    );
    println!(
        "[herdr-bot] Bot 模板存储文件：{}",
        config.bot_templates.storage_file.display()
    );
    println!(
        "[herdr-bot] 单文件预览上限：{} 字节",
        config.zones.preview_limit_bytes.get()
    );
    println!(
        "[herdr-bot] Git 命令执行上限：{} 秒/条",
        config.git.command_timeout_seconds.get()
    );
    println!("[herdr-bot] 监听：http://{local_addr}/");
    println!(
        "[herdr-bot] 局域网设备请访问 http://<本机局域网 IPv4>:{}（ipconfig 查看地址）",
        local_addr.port()
    );

    // TCP 监听绑定成功后发布 mDNS：端口取实际监听端口，局域网设备于是能用固定主机名访问。
    let mdns = herdr_bot::mdns::publish(&config.mdns, local_addr.port());

    if !herdr_bot::console_available() {
        eprintln!(
            "[herdr-bot] 提示：本次启动没有连接控制台。脚本中依赖控制台输入的命令（例如现有脚本末尾的 \
             timeout）会以非交互方式失败，操作结果会如实显示为该退出码；要与手动运行脚本一致，\
             请在控制台窗口中启动 herdr-bot 主界面。"
        );
    }

    // 会话表由主入口持有：同一份表既要交给应用，也要交给后台过期扫描，因此不用 `herdr_bot::app`
    // 的内部空表。监听已经绑定成功，从这里开始按固定节奏把超时的在线会话翻为离线。
    // 两个窗口只在这里从启动期配置读入一次（没有热加载），因此运行中改写文件不改变本实例的判定。
    let sessions = Arc::new(herdr_bot::sessions::SessionsState::with_policy(
        config.sessions.policy(),
    ));
    let (app, state) = herdr_bot::app_with_state(
        &config,
        Arc::new(herdr_bot::SystemPowerOff),
        Arc::clone(&sessions),
    );
    // 时钟驱动的会话维护跟随进程的生命周期：句柄不保留（丢弃句柄只是让任务脱管，任务本身继续跑），
    // 服务器关停时随运行时一起结束。它同时负责「心跳过期 → 移出完成后关机监听」这一遍重判。
    herdr_bot::sessions::spawn_expiry_scan(sessions, Arc::clone(&state.shutdown));

    // 会话事件的写入来源校验要拿到 TCP 对端地址，因此交给服务的是带连接信息的 make-service：
    // 只有它会把 `ConnectInfo<SocketAddr>` 放进请求，缺了它写入端点在运行时取不到来源。
    // 读取路径（页面与 SSE）不读连接信息，这一层装配对它们没有可见影响。
    let serve_result = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .context("Axum 服务异常退出");

    // 进程正常退出时 best-effort 关闭 mDNS responder；关机路径不特殊注销记录（见 ADR 0001）。
    // 这里不引入信号处理，也不等待脚本：服务退出不牵连脚本、不牵连执行资格。
    if let Some(mdns) = mdns {
        mdns.shutdown();
        println!("[herdr-bot] mDNS 发布已关闭");
    }

    serve_result
}

/// 解析 `--config <path>`；缺省使用当前目录下的 `herdr-bot.toml`。
fn config_path_from_args() -> anyhow::Result<PathBuf> {
    let mut args = std::env::args().skip(1);
    let mut config_path = PathBuf::from("herdr-bot.toml");
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => {
                let value = args.next().context("--config 需要一个文件路径参数")?;
                config_path = PathBuf::from(value);
            }
            other => anyhow::bail!("未知参数：{other}（用法：herdr-bot [--config <path>]）"),
        }
    }
    Ok(config_path)
}
