//! mDNS 发布失败时的行为（票 03）：只告警、不阻止 HTTP 服务，失败路径不发布任何记录。
//!
//! seam：S3 诊断日志（契约批准的 seam）。被验收的是**真实二进制**的启动输出、同机 HTTP 入口、以及
//! 同机解析/浏览结果——没有任何替身参与。
//!
//! **制造失败的手段**：把 `[mdns] address` 指向本机不拥有的地址 `192.0.2.1`（RFC 5737 TEST-NET-1，
//! 保留给文档使用，永远不会分配给任何主机）。发布者无法放出这个地址的记录，`publish` 因此走真实的
//! 失败分支：只告警、跳过 mDNS 发布、HTTP 服务照常启动。
//!
//! 解析/浏览观测用测试进程自己的 mDNS daemon。若本机同时有别的测试占用 `omp-bot.local`（票 02 的
//! 用例），观测不会失真：omp-bot 主界面的记录必然携带它配置的地址与自己实际监听的端口，而两者都被断言
//! 不得出现在线上，因此“记录是谁发的”始终可区分。

mod common;

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, UdpSocket};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use mdns_sd::{HostnameResolutionEvent, ServiceDaemon, ServiceEvent};
use omp_bot::mdns::{HOSTNAME, SERVICE_TYPE};

/// 失败地址：RFC 5737 TEST-NET-1，文档保留地址，本机不可能拥有它。
const UNOWNED_ADDRESS: Ipv4Addr = Ipv4Addr::new(192, 0, 2, 1);

/// 受测的真实 omp-bot 主界面进程：与交付运行时同一条 `main`，启动输出被收进缓冲区供轮询。
struct Panel {
    child: Child,
    log: Arc<Mutex<Vec<u8>>>,
    port: u16,
    _config_dir: common::TempDir,
}

impl Panel {
    /// 用指向本机不拥有地址的 `[mdns] address` 启动真实二进制；固定操作绑到夹具里立即成功的脚本，
    /// 因此“固定操作接口仍然可用”可以在真实进程上被观测到。
    fn start(address: Ipv4Addr, fixtures: &common::FixtureTree) -> Self {
        let script = fixtures.script("success.bat");
        let working_directory = fixtures.operations();
        let config_dir = common::TempDir::new("mdns failure");
        let config_path = config_dir.path().join("omp-bot.toml");
        std::fs::write(
            &config_path,
            format!(
                "listen_addr = \"0.0.0.0:0\"\n\n\
                 [operations.start]\nscript = '{script}'\nworking_directory = '{fixtures}'\n\n\
                 [operations.stop]\nscript = '{script}'\nworking_directory = '{fixtures}'\n\n\
                 [mdns]\naddress = \"{address}\"\n",
                script = script.display(),
                fixtures = working_directory.display(),
            ),
        )
        .expect("写临时配置");

        let mut child = Command::new(env!("CARGO_BIN_EXE_omp-bot"))
            .arg("--config")
            .arg(&config_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("启动 omp-bot 主界面进程");

        let log = Arc::new(Mutex::new(Vec::new()));
        for source in [
            Box::new(child.stdout.take().expect("stdout")) as Box<dyn Read + Send>,
            Box::new(child.stderr.take().expect("stderr")) as Box<dyn Read + Send>,
        ] {
            capture(source, Arc::clone(&log));
        }

        let mut panel = Panel {
            child,
            log,
            port: 0,
            _config_dir: config_dir,
        };
        panel.port = panel.wait_for_listen_port();
        panel
    }

    /// 诊断输出的完整文本（stdout 与 stderr 按到达顺序拼接）。
    fn log_text(&self) -> String {
        String::from_utf8_lossy(&self.log.lock().expect("读取日志")).into_owned()
    }

    /// 等到日志里出现 `needle`；超时则把已收到的输出一起报出来。
    fn wait_for_log(&self, needle: &str, timeout: Duration) -> String {
        let deadline = Instant::now() + timeout;
        loop {
            let text = self.log_text();
            if text.contains(needle) {
                return text;
            }
            if Instant::now() >= deadline {
                panic!("等待诊断输出 {needle:?} 超时；已收到的输出：\n{text}");
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// 从启动输出里的监听行取得实际监听端口（配置用的是 0 端口，端口由系统分配）。
    fn wait_for_listen_port(&mut self) -> u16 {
        let text = self.wait_for_log("[omp-bot] 监听：http://", Duration::from_secs(30));
        let line = text
            .lines()
            .find(|line| line.contains("[omp-bot] 监听：http://"))
            .expect("监听行已在上面确认存在");
        let after_scheme = line.split("//").nth(1).expect("监听行含协议前缀");
        let host_port = after_scheme.split('/').next().expect("监听行含路径");
        host_port
            .rsplit(':')
            .next()
            .expect("监听行含端口")
            .parse()
            .expect("监听端口是数字")
    }

    /// 进程是否仍在运行：mDNS 失败不得让 omp-bot 主界面退出。
    fn is_running(&mut self) -> bool {
        self.child
            .try_wait()
            .expect("查询子进程状态失败")
            .is_none()
    }
}

impl Drop for Panel {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// 把一路输出持续读进共享缓冲区：按行断言因此在进程运行期间也能拿到。
fn capture(mut source: Box<dyn Read + Send>, log: Arc<Mutex<Vec<u8>>>) {
    std::thread::spawn(move || {
        let mut buffer = [0u8; 1024];
        while let Ok(read) = source.read(&mut buffer) {
            if read == 0 {
                break;
            }
            log.lock().expect("写日志").extend_from_slice(&buffer[..read]);
        }
    });
}

/// 本机可发布的局域网 IPv4：与后端的选择顺序一致（先主路由推断，再退回网卡枚举）。
fn local_publishable_ipv4() -> Option<Ipv4Addr> {
    let main_route = UdpSocket::bind("0.0.0.0:0")
        .and_then(|socket| {
            socket.connect("8.8.8.8:80")?;
            socket.local_addr()
        })
        .ok()
        .and_then(|address| match address {
            SocketAddr::V4(address) => Some(*address.ip()),
            SocketAddr::V6(_) => None,
        })
        .filter(|address| omp_bot::mdns::is_publishable_ipv4(*address));

    main_route.or_else(|| {
        if_addrs::get_if_addrs()
            .ok()?
            .into_iter()
            .find_map(|interface| match interface.addr {
                if_addrs::IfAddr::V4(address)
                    if omp_bot::mdns::is_publishable_ipv4(address.ip) =>
                {
                    Some(address.ip)
                }
                _ => None,
            })
    })
}

/// 用**全新**的解析器查询主机名：它没有缓存，因此解析到的地址只可能来自此刻仍在线的发布者。
fn resolved_addresses(hostname: &str) -> Vec<IpAddr> {
    let daemon = ServiceDaemon::new().expect("创建解析器 daemon");
    let events = daemon
        .resolve_hostname(hostname, Some(1_500))
        .expect("发起主机名解析");
    let deadline = Instant::now() + Duration::from_secs(4);
    let mut addresses = Vec::new();
    while let Ok(event) = events.recv_deadline(deadline) {
        if let HostnameResolutionEvent::AddressesFound(_, found) = event {
            addresses.extend(found.iter().map(|scoped| scoped.to_ip_addr()));
        }
    }
    let _ = daemon.shutdown();
    addresses
}

/// 浏览 `_http._tcp` 一段时间，返回期间解析到的服务实例（完整名字，SRV 端口）。
fn browse_http_instances(duration: Duration) -> Vec<(String, u16)> {
    let daemon = ServiceDaemon::new().expect("创建浏览器 daemon");
    let events = daemon.browse(SERVICE_TYPE).expect("发起浏览");
    let deadline = Instant::now() + duration;
    let mut instances = Vec::new();
    while let Ok(event) = events.recv_deadline(deadline) {
        if let ServiceEvent::ServiceResolved(service) = event {
            instances.push((service.get_fullname().to_string(), service.get_port()));
        }
    }
    let _ = daemon.shutdown();
    instances
}

/// 一次最小 HTTP/1.1 请求：直接经 TCP 连端口，不经过页面、按钮或任何前端代码。
fn http_request(addr: SocketAddr, request: &str) -> (String, String) {
    let mut stream = TcpStream::connect(addr).expect("连接 omp-bot 主界面");
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .expect("设置读取超时");
    write!(stream, "{request}").expect("发送请求");
    let mut raw = String::new();
    stream.read_to_string(&mut raw).expect("读取响应");
    let (head, body) = raw.split_once("\r\n\r\n").expect("响应含头体分隔");
    (
        head.lines().next().expect("响应含状态行").to_string(),
        body.to_string(),
    )
}

/// mDNS 失败不阻止启动：每个入口都能打开打包页面。
fn assert_page_serves(panel: &Panel, entries: &[Ipv4Addr]) {
    for entry in entries {
        let addr = SocketAddr::new(IpAddr::V4(*entry), panel.port);
        let (status, body) = http_request(
            addr,
            &format!("GET / HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"),
        );
        assert!(status.contains("200"), "入口 {entry} 打不开 omp-bot 主界面：{status}");
        assert!(
            body.contains("<title>omp-bot</title>"),
            "入口 {entry} 返回的不是打包页面：{body}"
        );
    }
}

/// mDNS 失败不阻止启动：固定操作接口照常执行脚本（裸 POST，不带查询串与请求体）。
fn assert_operation_is_served(panel: &Panel) {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), panel.port);
    let (status, body) = http_request(
        addr,
        &format!(
            "POST /api/operations/start HTTP/1.1\r\nHost: {addr}\r\nContent-Length: 0\r\n\
             Connection: close\r\n\r\n"
        ),
    );
    assert!(status.contains("200"), "固定操作接口不可用：{status} {body}");
    eprintln!("固定操作接口观测：{status} {body}");
    assert!(
        body.contains("\"outcome\":\"completed\"") && body.contains("\"exitCode\":0"),
        "固定操作没有真正执行成功：{body}"
    );
}

/// 验收 1–4：`[mdns] address` 指向本机不拥有的地址 → 诊断为 `error`（不是 disabled / no-address /
/// conflict / published）、不发布任何记录、HTTP 服务照常、进程不退出。
#[test]
fn unowned_publish_address_only_warns_and_keeps_serving_without_publishing() {
    let fixtures = common::FixtureTree::new("mdns failure");
    let mut panel = Panel::start(UNOWNED_ADDRESS, &fixtures);

    // 验收 1：诊断输出给出失败告警，类别机器可断言且与其他类别互相区分。
    let log = panel.wait_for_log("mDNS 状态：", Duration::from_secs(30));
    assert!(
        log.contains("mDNS 状态：error"),
        "mDNS 发布失败必须报为 error 类别：\n{log}"
    );
    for other in [
        "mDNS 状态：disabled",
        "mDNS 状态：no-address",
        "mDNS 状态：conflict",
        "mDNS 状态：published",
    ] {
        assert!(
            !log.contains(other),
            "失败类别不得与 {other} 混在一起：\n{log}"
        );
    }
    assert!(
        log.contains(&UNOWNED_ADDRESS.to_string()),
        "告警必须指出是哪个地址发布不了：\n{log}"
    );

    // 验收 2：失败不阻止启动——两个入口都能打开页面，固定操作接口照常执行脚本，进程不退出。
    let mut entries = vec![Ipv4Addr::LOCALHOST];
    match local_publishable_ipv4() {
        Some(address) => entries.push(address),
        None => eprintln!("跳过 IP 入口断言：本机没有可发布的局域网 IPv4"),
    }
    assert_page_serves(&panel, &entries);
    eprintln!("页面入口断言通过：{entries:?}（端口 {}）", panel.port);
    assert_operation_is_served(&panel);
    assert!(
        panel.is_running(),
        "mDNS 发布失败不得让进程退出：\n{}",
        panel.log_text()
    );

    // 验收 3：失败路径不发布任何 mDNS 记录——omp-bot 主界面的身份（配置地址、实际端口）都不在线上。
    let resolved = resolved_addresses(HOSTNAME);
    assert!(
        !resolved.contains(&IpAddr::V4(UNOWNED_ADDRESS)),
        "失败路径把配置地址播了出去：{resolved:?}"
    );
    let instances = browse_http_instances(Duration::from_secs(3));
    assert!(
        instances.iter().all(|(_, port)| *port != panel.port),
        "失败路径发布了 omp-bot 主界面实例：{instances:?}"
    );

    eprintln!("omp-bot.local 解析到的地址：{resolved:?}（为空表示完全解析不到）");
    eprintln!("_http._tcp 浏览结果：{instances:?}");
    eprintln!("受测 omp-bot 主界面的原始诊断输出：\n{}", panel.log_text());
}
