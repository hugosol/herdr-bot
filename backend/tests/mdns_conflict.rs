//! `herdr-bot.local` 被占用时的发布行为（票 02）：不改名、不发布、告警，且不阻止 HTTP 服务。
//!
//! seam：S3 诊断日志（契约批准的 seam）。受控占名用第二个真实的 mDNS 发布者——本进程内一个独立的
//! `mdns-sd` daemon，发布与后端完全相同的主机名与服务实例名（原型用的等价手段；人工验收另用 Bonjour
//! 自带的 `dns-sd -P` 做独立实现的复核）。被验收的是**真实二进制**的启动输出与同机解析/浏览结果。
//!
//! 两个用例都要独占 `herdr-bot.local`，因此串行执行。

mod common;

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, UdpSocket};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use herdr_bot::mdns::{HOSTNAME, INSTANCE_NAME, SERVICE_FULLNAME, SERVICE_TYPE};
use mdns_sd::{DaemonEvent, HostnameResolutionEvent, ServiceDaemon, ServiceEvent, ServiceInfo};

/// 受控占名用的服务端口：与受测 herdr-bot 主界面的实际监听端口必然不同，因此线上实例的端口能指认发布者是谁。
const OCCUPIER_PORT: u16 = 18788;

/// 绕过预探测、强制注册的诊断逃生舱（与 `backend/src/mdns.rs` 的 `SKIP_PREPROBE_ENV` 同名）。
const SKIP_PREPROBE_ENV: &str = "HERDR_BOT_MDNS_SKIP_PREPROBE";

/// 两个用例都要独占 `herdr-bot.local`：串行执行，避免两个占用者互相把对方的名字挤掉。
static SERIAL: Mutex<()> = Mutex::new(());

fn serial_guard() -> std::sync::MutexGuard<'static, ()> {
    SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 受控占名：独立发布者持有 `herdr-bot.local.` 与 `herdr-bot._http._tcp.local.`，直到被丢弃。
struct Occupier {
    daemon: ServiceDaemon,
}

impl Occupier {
    /// 发布同名记录并等到它真正播出去（收到 `Announce`）才返回：否则受测 herdr-bot 主界面的预探测可能赶在
    /// 占用者上线之前跑完，得到“名字空闲”的假阴性。
    fn start(address: Ipv4Addr) -> Self {
        let daemon = ServiceDaemon::new().expect("创建占用者 daemon");
        let events = daemon.monitor().expect("监听占用者事件");
        let service = ServiceInfo::new(
            SERVICE_TYPE,
            INSTANCE_NAME,
            HOSTNAME,
            IpAddr::V4(address),
            OCCUPIER_PORT,
            &[("path", "/")][..],
        )
        .expect("构造占用者记录");
        daemon.register(service).expect("注册占用者记录");

        let deadline = Instant::now() + Duration::from_secs(15);
        while let Ok(event) = events.recv_deadline(deadline) {
            if let DaemonEvent::Announce(..) = event {
                return Occupier { daemon };
            }
        }
        panic!("受控占用者未在 15 秒内把记录播出去");
    }
}

impl Drop for Occupier {
    fn drop(&mut self) {
        let _ = self.daemon.unregister(SERVICE_FULLNAME);
        let _ = self.daemon.shutdown();
    }
}

/// 受测的真实 herdr-bot 主界面进程：与交付运行时同一条 `main`，启动输出被收进缓冲区供轮询。
struct Panel {
    child: Child,
    log: Arc<Mutex<Vec<u8>>>,
    port: u16,
    _config_dir: common::TempDir,
}

impl Panel {
    /// 启动真实二进制。`skip_preprobe` 时为它设置诊断逃生舱，绕过注册前的冲突预探测。
    fn start(address: Ipv4Addr, skip_preprobe: bool) -> Self {
        let config_dir = common::TempDir::new("mdns conflict");
        let config_path = config_dir.path().join("herdr-bot.toml");
        std::fs::write(
            &config_path,
            format!(
                "listen_addr = \"0.0.0.0:0\"\n\n\
                 [operations.start]\nscript = 'C:\\missing\\start.bat'\nworking_directory = 'C:\\missing'\n\n\
                 [operations.stop]\nscript = 'C:\\missing\\stop.bat'\nworking_directory = 'C:\\missing'\n\n\
                 [mdns]\naddress = \"{address}\"\n\n\
                 [tasks]\nmetadata_file = '{metadata}'\n\n\
                 [herdr]\nworkspace_name = 'bot-playground'\n",
                metadata = common::test_metadata_file().display(),
            ),
        )
        .expect("写临时配置");

        let mut command = Command::new(env!("CARGO_BIN_EXE_herdr-bot"));
        command
            .current_dir(common::repo_fixtures_dir().parent().unwrap())
            .arg("--config")
            .arg(&config_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if skip_preprobe {
            command.env(SKIP_PREPROBE_ENV, "1");
        } else {
            // 诊断逃生舱不继承自测试进程：用例要验收的正是“有预探测”的那条路径。
            command.env_remove(SKIP_PREPROBE_ENV);
        }
        let mut child = command.spawn().expect("启动 herdr-bot 主界面进程");

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
        let text = self.wait_for_log("[herdr-bot] 监听：http://", Duration::from_secs(30));
        let line = text
            .lines()
            .find(|line| line.contains("[herdr-bot] 监听：http://"))
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
            log.lock()
                .expect("写日志")
                .extend_from_slice(&buffer[..read]);
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
        .filter(|address| herdr_bot::mdns::is_publishable_ipv4(*address));

    main_route.or_else(|| {
        if_addrs::get_if_addrs()
            .ok()?
            .into_iter()
            .find_map(|interface| match interface.addr {
                if_addrs::IfAddr::V4(address)
                    if herdr_bot::mdns::is_publishable_ipv4(address.ip) =>
                {
                    Some(address.ip)
                }
                _ => None,
            })
    })
}

/// 主机名此刻是否有发布者在应答：用**全新**的解析器查询，它没有缓存，因此“有应答”只可能来自
/// 仍在线的发布者。
fn hostname_is_answered(hostname: &str) -> bool {
    let daemon = ServiceDaemon::new().expect("创建解析器 daemon");
    let events = daemon
        .resolve_hostname(hostname, Some(1_500))
        .expect("发起主机名解析");
    let deadline = Instant::now() + Duration::from_secs(4);
    let answered = loop {
        match events.recv_deadline(deadline) {
            Ok(HostnameResolutionEvent::AddressesFound(_, addresses)) if !addresses.is_empty() => {
                break true;
            }
            Ok(HostnameResolutionEvent::SearchTimeout(_)) => break false,
            Ok(_) => continue,
            Err(_) => break false,
        }
    };
    let _ = daemon.shutdown();
    answered
}

/// 断言主机名在 `timeout` 内不再被任何人解析，并且之后保持解析不到。
fn assert_hostname_stays_unresolvable(hostname: &str, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while hostname_is_answered(hostname) {
        assert!(
            Instant::now() < deadline,
            "{hostname} 仍被解析：改名后的名字被留在了线上"
        );
    }
    assert!(
        !hostname_is_answered(hostname),
        "{hostname} 在确认之后又被解析到，说明注销没有生效"
    );
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

/// 一次最小 HTTP/1.1 GET：直接经 TCP 连端口，不经过页面、按钮或任何前端代码。
fn http_get(addr: SocketAddr, path: &str) -> (String, String) {
    let mut stream = TcpStream::connect(addr).expect("连接 herdr-bot 主界面");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("设置读取超时");
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    )
    .expect("发送请求");
    let mut raw = String::new();
    stream.read_to_string(&mut raw).expect("读取响应");
    let (head, body) = raw.split_once("\r\n\r\n").expect("响应含头体分隔");
    (
        head.lines().next().expect("响应含状态行").to_string(),
        body.to_string(),
    )
}

/// 冲突路径不阻止 HTTP 服务：本机与局域网 IPv4 两个入口都能打开打包页面。
fn assert_panel_serves(panel: &Panel, address: Ipv4Addr) {
    for entry in [Ipv4Addr::LOCALHOST, address] {
        let (status, body) = http_get(SocketAddr::new(IpAddr::V4(entry), panel.port), "/");
        assert!(
            status.contains("200"),
            "入口 {entry} 打不开 herdr-bot 主界面：{status}"
        );
        assert!(
            body.contains("<title>herdr-bot</title>"),
            "入口 {entry} 返回的不是打包页面：{body}"
        );
    }
}

/// 验收 1、2、4：名字被占用 → 诊断为 `conflict`（不是 disabled / no-address / error）、不发布、
/// 不改名，且 HTTP 服务照常。
#[test]
#[ignore = "需要独占 herdr-bot.local. 并依赖真实 mDNS/防火墙许可；用 --ignored 显式运行"]
fn conflicting_hostname_is_neither_published_nor_renamed_and_panel_still_serves() {
    let _serial = serial_guard();
    let Some(address) = local_publishable_ipv4() else {
        eprintln!("跳过：本机没有可发布的局域网 IPv4，无法构造受控占名");
        return;
    };

    let occupier = Occupier::start(address);
    let panel = Panel::start(address, false);

    let log = panel.wait_for_log("mDNS 状态：conflict", Duration::from_secs(20));
    assert!(
        log.contains("conflict phase=preprobe"),
        "占用必须在注册前的预探测里被发现：\n{log}"
    );
    assert!(
        !log.contains("mDNS 状态：published"),
        "名字被占用时不得发布任何记录：\n{log}"
    );
    for other in ["disabled", "no-address", "状态：error"] {
        assert!(
            !log.contains(other),
            "冲突类别不得与 {other} 混在一起：\n{log}"
        );
    }

    assert_hostname_stays_unresolvable("herdr-bot-2.local.", Duration::from_secs(5));
    let instances = browse_http_instances(Duration::from_secs(3));
    assert!(
        instances
            .iter()
            .any(|(fullname, port)| fullname == SERVICE_FULLNAME && *port == OCCUPIER_PORT),
        "受控占用者必须在线，否则这条观测毫无意义：{instances:?}"
    );
    assert!(
        instances.iter().all(|(_, port)| *port != panel.port),
        "被占名时 herdr-bot 主界面不得发布任何服务实例：{instances:?}"
    );

    assert_panel_serves(&panel, address);
    eprintln!(
        "受测 herdr-bot 主界面的原始诊断输出：\n{}",
        panel.log_text()
    );
    drop(occupier);
}

/// 验收 3：绕过预探测强制注册、随后被 mdns-sd 改名时，herdr-bot 主界面立即注销该记录并告警。
#[test]
#[ignore = "需要独占 herdr-bot.local. 并依赖真实 mDNS/防火墙许可；用 --ignored 显式运行"]
fn registration_forced_past_the_preprobe_is_unregistered_once_mdns_renames_it() {
    let _serial = serial_guard();
    let Some(address) = local_publishable_ipv4() else {
        eprintln!("跳过：本机没有可发布的局域网 IPv4，无法构造受控占名");
        return;
    };

    let occupier = Occupier::start(address);
    let panel = Panel::start(address, true);

    let log = panel.wait_for_log("mDNS 状态：conflict", Duration::from_secs(20));
    assert!(
        log.contains("conflict phase=renamed"),
        "预探测未命中的冲突必须由改名兜底告警：\n{log}"
    );
    assert!(
        log.contains("mDNS 状态：published"),
        "绕过预探测时应先注册成功，再由改名兜底注销：\n{log}"
    );

    assert_hostname_stays_unresolvable("herdr-bot-2.local.", Duration::from_secs(20));
    let instances = browse_http_instances(Duration::from_secs(3));
    assert!(
        instances.iter().all(|(_, port)| *port != panel.port),
        "改名后的服务实例必须已被注销：{instances:?}"
    );

    assert_panel_serves(&panel, address);
    eprintln!(
        "受测 herdr-bot 主界面的原始诊断输出：\n{}",
        panel.log_text()
    );
    drop(occupier);
}
