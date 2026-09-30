//! 手动启动入口：读取配置，绑定非回环监听地址，提供页面与操作接口。

use std::path::PathBuf;

use anyhow::Context;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config_path = config_path_from_args()?;
    let config = omp_bot::Config::load(&config_path)?;

    let static_dir = omp_bot::static_dir();
    if !static_dir.join("index.html").is_file() {
        eprintln!(
            "[omp-bot] 警告：未找到打包页面 {}，请先在 frontend 目录执行 npm run build。",
            static_dir.join("index.html").display()
        );
    }

    let listener = tokio::net::TcpListener::bind(&config.listen_addr)
        .await
        .with_context(|| format!("绑定监听地址失败：{}", config.listen_addr))?;
    let local_addr = listener.local_addr().context("读取监听地址失败")?;

    println!("[omp-bot] 控制面板已启动");
    println!("[omp-bot] 配置文件：{}", config_path.display());
    for (label, binding) in [
        ("启动", &config.operations.start),
        ("停止", &config.operations.stop),
    ] {
        println!(
            "[omp-bot] {label}操作：{}（工作目录 {}）",
            binding.script.display(),
            binding.working_directory.display()
        );
    }
    println!("[omp-bot] 打包页面：{}", static_dir.display());
    println!("[omp-bot] 监听：http://{local_addr}/");
    println!(
        "[omp-bot] 局域网设备请访问 http://<本机局域网 IPv4>:{}（ipconfig 查看地址）",
        local_addr.port()
    );

    // TCP 监听绑定成功后发布 mDNS：端口取实际监听端口，局域网设备于是能用固定主机名访问。
    let mdns = omp_bot::mdns::publish(&config.mdns, local_addr.port());

    if !omp_bot::console_available() {
        eprintln!(
            "[omp-bot] 提示：本次启动没有连接控制台。脚本中依赖控制台输入的命令（例如现有脚本末尾的 \
             timeout）会以非交互方式失败，操作结果会如实显示为该退出码；要与手动运行脚本一致，\
             请在控制台窗口中启动控制面板。"
        );
    }

    let app = omp_bot::app(&config);
    let serve_result = axum::serve(listener, app)
        .await
        .context("Axum 服务异常退出");

    // 进程正常退出时 best-effort 关闭 mDNS responder；关机路径不特殊注销记录（见 ADR 0001）。
    // 这里不引入信号处理，也不等待脚本：服务退出不牵连脚本、不牵连执行资格。
    if let Some(mdns) = mdns {
        mdns.shutdown();
        println!("[omp-bot] mDNS 发布已关闭");
    }

    serve_result
}

/// 解析 `--config <path>`；缺省使用当前目录下的 `omp-bot.toml`。
fn config_path_from_args() -> anyhow::Result<PathBuf> {
    let mut args = std::env::args().skip(1);
    let mut config_path = PathBuf::from("omp-bot.toml");
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => {
                let value = args.next().context("--config 需要一个文件路径参数")?;
                config_path = PathBuf::from(value);
            }
            other => anyhow::bail!("未知参数：{other}（用法：omp-bot [--config <path>]）"),
        }
    }
    Ok(config_path)
}
