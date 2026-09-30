//! mDNS 主机名发布：让局域网设备用固定的 `omp-bot.local` 打开同一个控制面板。
//!
//! 发布者是控制面板后端进程内的常驻 mDNS responder（`mdns-sd`），与 Axum 服务并列运行；它发布的
//! 是一个主机名（`omp-bot.local.` 的 A 记录）和一个 `_http._tcp` 服务实例（端口取实际监听端口，
//! TXT 放访问路径与版本），因此 IP 入口始终保留为兜底。
//!
//! 发布什么地址由一个**不依赖发布、也不碰真实网卡**的纯函数决定（[`select_publish_address`]）：
//! 优先配置里的 `[mdns] address`，否则优先主路由 IPv4（用“UDP socket 连外部地址读本地地址”推断），
//! 再者退回第一块非回环 IPv4。回环与 `169.254.0.0/16` 在任何路径上都不会被发布。
//!
//! 发布地址还必须由本机**拥有**：本机不拥有的地址（`address` 填错、换了网段、地址被别人占走）会让
//! `mdns-sd` 注册成功却一条记录也播不出去，所以这种配置按发布失败告警并跳过发布，见
//! [`address_is_local`]。
//!
//! 名字冲突时**不改名**、**不发布**：主机名硬编码，`omp-bot-2.local.` 之类的名字绝不对外出现。
//! 两步策略覆盖两条路径——注册前用一次主机名查询做预探测（[`hostname_is_taken`]），有应答就
//! 判定冲突、不注册并告警；确有占用者在预探测之后才应答时，注册后监听 `DaemonEvent::NameChange`
//! （[`unregister_on_name_change`]），一旦 mdns-sd 因冲突把我们改名就立即注销该记录并告警。
//!
//! 诊断输出用固定的类别词（`published` / `disabled` / `conflict` / `no-address` / `error`）加稳定的
//! `key=value` 字段，行首为 `[omp-bot] mDNS 状态：`，因此发布结果可以被自动化断言。
//!
//! 生命周期：TCP 监听绑定成功后再开始发布（端口取实际监听端口）；进程正常退出时 best-effort 关闭
//! responder。关机路径不特殊注销记录——下次启动重新发布的记录会刷新客户端缓存。

use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use mdns_sd::{DaemonEvent, HostnameResolutionEvent, ServiceDaemon, ServiceInfo};

use crate::config::MdnsConfig;

/// 发布的主机名标签（不含 `.local.` 后缀），硬编码、不可配置。
pub const HOST_LABEL: &str = "omp-bot";

/// 发布的完整主机名。`mdns-sd` 要求主机名以 `.local.` 结尾（漏写会直接报错）。
pub const HOSTNAME: &str = "omp-bot.local.";

/// 服务类型：控制面板是一个 HTTP 服务。
pub const SERVICE_TYPE: &str = "_http._tcp.local.";

/// 服务实例名。
pub const INSTANCE_NAME: &str = "omp-bot";

/// 服务实例的完整名字（`ServiceInfo::get_fullname()`）。
///
/// 注销时用它，而不是改名后的名字：`mdns-sd` 会按这个名字把记录解析成线上真正播出的名字。
pub const SERVICE_FULLNAME: &str = "omp-bot._http._tcp.local.";

/// TXT 里的访问路径。
pub const ACCESS_PATH: &str = "/";

/// 推断主路由 IPv4 时连接的目标：只用于让操作系统按路由表选出本机源地址（UDP 不握手、不发包）。
const MAIN_ROUTE_PROBE: &str = "8.8.8.8:80";

/// 诊断输出的固定前缀与类别词；自动化据此断言发布结果类别。
const DIAGNOSTIC_PREFIX: &str = "[omp-bot] mDNS 状态：";

/// 注册前冲突预探测的等待上限（毫秒）：超过它仍无应答就按名字空闲处理。
const PREPROBE_TIMEOUT_MS: u64 = 1_500;

/// 预探测在库自报超时之外额外等待的宽限，用来收完最后一批事件。
const PREPROBE_GRACE: Duration = Duration::from_millis(500);

/// 诊断逃生舱：设置（非空）时跳过注册前的冲突预探测，直接注册。
///
/// 只用于验收“预探测未命中、注册后才被改名”的兜底路径——正常情况下占用者会在预探测里被看到，
/// 这条路径只在占用者晚应答时才真实发生，无法靠时序稳定复现。正常运行不设置这个变量。
const SKIP_PREPROBE_ENV: &str = "OMP_BOT_MDNS_SKIP_PREPROBE";

/// 已发布的 mDNS responder 句柄；持有它，进程正常退出时做 best-effort 关闭。
pub struct Publisher {
    daemon: ServiceDaemon,
}

impl Publisher {
    /// best-effort 关闭 responder：失败只意味着记录会随进程结束自然消失，不影响已经退出的进程。
    pub fn shutdown(&self) {
        let _ = self.daemon.shutdown();
    }
}

/// 依据配置与实际监听端口发布 mDNS 记录，并输出可断言的诊断状态。
///
/// 返回 `Some` 表示记录已注册（`published`）；`None` 表示未发布，原因已写进诊断输出：
/// `disabled`（配置关闭）、`no-address`（没有**可发布**的 IPv4）、`conflict`（名字已被占用，见模块文档）
/// 或 `error`（发布地址不在本机网卡上、预探测/注册/绑定失败）。任何一条路径都不影响 HTTP 服务与
/// IP 入口。
pub fn publish(config: &MdnsConfig, port: u16) -> Option<Publisher> {
    if !config.enabled {
        println!("{DIAGNOSTIC_PREFIX}disabled（[mdns] enabled = false，未发布任何 mDNS 记录）");
        return None;
    }

    let Some(address) = select_publish_address(config.address, &candidate_addresses()) else {
        // 没有可发布的 IPv4：跳过发布并告警。回环与链路本地地址在这条路径上被拒绝，不会被发布。
        match config.address {
            Some(rejected) => eprintln!(
                "{DIAGNOSTIC_PREFIX}no-address（[mdns] address = {rejected} 不是可发布的局域网 IPv4：\
                 只发布 IPv4，且回环与 169.254.0.0/16 永不被发布；已跳过 mDNS 发布，IP 入口照常可用）"
            ),
            None => eprintln!(
                "{DIAGNOSTIC_PREFIX}no-address（没有可发布的 IPv4：只发布 IPv4，且回环与 \
                 169.254.0.0/16 永不被发布；已跳过 mDNS 发布，IP 入口照常可用）"
            ),
        }
        return None;
    };

    // 发布地址必须是本机网卡上真实存在的地址。指向本机不拥有的地址时，`mdns-sd` 会“注册成功却
    // 一条记录也播不出去”（它只在地址所属网段的本机网卡上播出记录），局域网里因此什么都解析不到，
    // 而配置看起来是生效的——这正是本路径要如实报出的发布失败。只告警，不阻止 HTTP 服务启动。
    if !address_is_local(address) {
        eprintln!(
            "{DIAGNOSTIC_PREFIX}error（mDNS 发布失败：[mdns] address = {address} 不是本机网卡上的\
             地址，发布者无法放出该地址的记录；已跳过 mDNS 发布，HTTP 服务照常启动，IP 入口可用）"
        );
        return None;
    }

    // 记录构造与 responder 启动失败都只是“发布失败”，不阻止 HTTP 服务启动。
    let service = match build_service_info(address, port) {
        Ok(service) => service,
        Err(error) => {
            eprintln!(
                "{DIAGNOSTIC_PREFIX}error（mDNS 发布失败：{error}；HTTP 服务照常启动，IP 入口可用）"
            );
            return None;
        }
    };
    let daemon = match ServiceDaemon::new() {
        Ok(daemon) => daemon,
        Err(error) => {
            eprintln!(
                "{DIAGNOSTIC_PREFIX}error（mDNS 发布失败：{error}；HTTP 服务照常启动，IP 入口可用）"
            );
            return None;
        }
    };

    if !preprobe_skipped() {
        match hostname_is_taken(&daemon, HOSTNAME) {
            // 名字已被占用：不注册、不改名，只告警。HTTP 服务与 IP 入口不受影响。
            Ok(true) => {
                eprintln!(
                    "{DIAGNOSTIC_PREFIX}conflict phase=preprobe host={HOSTNAME}（该名字已有应答：\
                     局域网里已有别的设备占用它，因此不注册、不改名，也未发布任何 mDNS 记录；\
                     HTTP 服务照常启动，IP 入口可用）"
                );
                let _ = daemon.shutdown();
                return None;
            }
            Ok(false) => {}
            // 预探测本身跑不起来时仍然发布：占用风险交给注册后的改名兜底，可用性优先。
            Err(error) => eprintln!(
                "{DIAGNOSTIC_PREFIX}error（mDNS 冲突预探测无法执行：{error}；\
                 仍按“名字空闲”继续发布，注册后的改名兜底仍然有效）"
            ),
        }
    }

    match daemon.register(service) {
        Ok(()) => {
            // 预探测只能证明“探测那一刻没有应答”；占用者晚应答时靠改名兜底注销，绝不改名发布。
            if let Err(error) = unregister_on_name_change(&daemon) {
                eprintln!(
                    "{DIAGNOSTIC_PREFIX}error（mDNS 改名兜底监视无法启动：{error}；\
                     记录已发布，但出现冲突改名时不会自动注销）"
                );
            }
            println!(
                "{DIAGNOSTIC_PREFIX}published host={HOSTNAME} address={address} port={port} \
                 url=http://{HOST_LABEL}.local:{port}/"
            );
            Some(Publisher { daemon })
        }
        // 注册/绑定失败只告警，不阻止 HTTP 服务启动。
        Err(error) => {
            eprintln!(
                "{DIAGNOSTIC_PREFIX}error（mDNS 发布失败：{error}；HTTP 服务照常启动，IP 入口可用）"
            );
            None
        }
    }
}

/// 是否跳过了注册前的冲突预探测（诊断逃生舱，见 [`SKIP_PREPROBE_ENV`]）。
fn preprobe_skipped() -> bool {
    std::env::var_os(SKIP_PREPROBE_ENV).is_some_and(|value| !value.is_empty())
}

/// 注册前的冲突预探测：查询主机名，有任何地址应答就说明这个名字已被局域网里的其他发布者占用。
///
/// 这一步必须在 `register` 之前跑：本进程此时还没有发布任何记录，因此应答只可能来自别人。
/// `Ok(false)` 表示预探测窗口内没有应答；`Err` 表示预探测本身无法执行（**不是**“名字空闲”）。
fn hostname_is_taken(daemon: &ServiceDaemon, hostname: &str) -> mdns_sd::Result<bool> {
    let events = daemon.resolve_hostname(hostname, Some(PREPROBE_TIMEOUT_MS))?;
    let deadline = Instant::now() + Duration::from_millis(PREPROBE_TIMEOUT_MS) + PREPROBE_GRACE;
    let taken = loop {
        match events.recv_deadline(deadline) {
            Ok(HostnameResolutionEvent::AddressesFound(_, addresses)) if !addresses.is_empty() => {
                break true;
            }
            // 开始搜索、地址被移除、库自报超时：都不是“已被占用”的证据。
            Ok(HostnameResolutionEvent::SearchTimeout(_)) => break false,
            Ok(_) => continue,
            // 预探测窗口结束或事件通道关闭：没有应答。
            Err(_) => break false,
        }
    };
    let _ = daemon.stop_resolve_hostname(hostname);
    Ok(taken)
}

/// 注册后的兜底监视：注册成功不等于名字归我们所有——占用者可能在预探测之后才应答或上线。
///
/// `mdns-sd` 探测到冲突会自行改名（`DaemonEvent::NameChange`）；一旦被改的是我们发布的记录，
/// 就立即注销整条服务记录（`mdns-sd` 会把注销解析到改名后的名字上），绝不把改名后的名字留在线上。
fn unregister_on_name_change(daemon: &ServiceDaemon) -> mdns_sd::Result<()> {
    let events = daemon.monitor()?;
    let watcher = daemon.clone();
    std::thread::spawn(move || {
        while let Ok(event) = events.recv() {
            let DaemonEvent::NameChange(change) = event else {
                continue;
            };
            if !is_our_record(&change.original) {
                continue;
            }
            // 注销一次即可：记录从线上撤走后不会再有新的改名事件。
            let _ = watcher.unregister(SERVICE_FULLNAME);
            eprintln!(
                "{DIAGNOSTIC_PREFIX}conflict phase=renamed original={original} renamed={renamed}\
                 （注册后才发现名字被占用：mdns-sd 已改名，控制面板立即注销该记录、不改名发布；\
                 HTTP 服务照常运行，IP 入口可用）",
                original = change.original,
                renamed = change.new_name,
            );
            break;
        }
    });
    Ok(())
}

/// 改名事件里的原名字是否属于本进程发布的记录：主机名或服务实例名任一被改都算。
fn is_our_record(original: &str) -> bool {
    original.eq_ignore_ascii_case(HOSTNAME) || original.eq_ignore_ascii_case(SERVICE_FULLNAME)
}

/// 构造要发布的 mDNS 记录：`omp-bot.local.` 的 A 记录 + `_http._tcp` 服务实例。
///
/// 只包含**一个 IPv4 地址**（因此发布内容只含 IPv4）；访问路径与版本放进 TXT。
fn build_service_info(address: Ipv4Addr, port: u16) -> mdns_sd::Result<ServiceInfo> {
    let properties = [
        ("path", ACCESS_PATH),
        ("version", env!("CARGO_PKG_VERSION")),
    ];
    ServiceInfo::new(
        SERVICE_TYPE,
        INSTANCE_NAME,
        HOSTNAME,
        IpAddr::V4(address),
        port,
        &properties[..],
    )
}

/// 该 IPv4 是否可以发布：只排除回环与链路本地（`169.254.0.0/16`）。
pub fn is_publishable_ipv4(address: Ipv4Addr) -> bool {
    !address.is_loopback() && !address.is_link_local()
}

/// 纯函数（不依赖发布、不碰真实网卡）：从候选地址里选出要发布的 IPv4。
///
/// `override_addr` 来自 `[mdns] address`：一旦设置就只考虑它自己——即使它不可发布也**不会**退化成
/// 别的地址或回环地址，而是返回 `None`（跳过发布）。
///
/// 未设置时按顺序取 `candidates` 里第一个可发布地址；调用方把主路由推断值放在最前，其后才是按
/// 系统顺序的网卡地址。
pub fn select_publish_address(
    override_addr: Option<Ipv4Addr>,
    candidates: &[Ipv4Addr],
) -> Option<Ipv4Addr> {
    match override_addr {
        Some(address) => is_publishable_ipv4(address).then_some(address),
        None => candidates
            .iter()
            .copied()
            .find(|address| is_publishable_ipv4(*address)),
    }
}

/// 候选发布地址：主路由推断值在前，其后是按系统顺序的网卡 IPv4（过滤交给纯函数）。
fn candidate_addresses() -> Vec<Ipv4Addr> {
    let mut candidates = Vec::new();
    if let Some(main_route) = infer_main_route_ipv4() {
        candidates.push(main_route);
    }
    candidates.extend(nic_ipv4_addresses());
    candidates
}

/// 用“UDP socket 连外部地址、读被操作系统选中的本地地址”推断主路由 IPv4。
///
/// `connect` 只让操作系统按路由表选源地址，不发任何包；选中的就是默认路由所在网卡的地址，因此
/// WSL/vEthernet 与链路本地地址天然被排除。推断失败（无路由、无默认网关等）返回 `None`。
fn infer_main_route_ipv4() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect(MAIN_ROUTE_PROBE).ok()?;
    match socket.local_addr().ok()? {
        SocketAddr::V4(address) => Some(*address.ip()),
        SocketAddr::V6(_) => None,
    }
}

/// 按系统顺序枚举网卡 IPv4（含回环与链路本地；是否可发布由纯函数判定）。
///
/// 枚举失败时返回空列表：这不是错误路径，只是没有网卡候选，主路由推断值仍可能可用。
fn nic_ipv4_addresses() -> Vec<Ipv4Addr> {
    let Ok(interfaces) = if_addrs::get_if_addrs() else {
        return Vec::new();
    };
    interfaces
        .into_iter()
        .filter_map(|interface| match interface.addr {
            if_addrs::IfAddr::V4(address) => Some(address.ip),
            if_addrs::IfAddr::V6(_) => None,
        })
        .collect()
}

/// 该地址是否真的挂在本机网卡上（**拥有**，不是“同网段”）。
///
/// `mdns-sd` 只在地址所属网段的本机网卡上播出记录：不在本机任何网段上的地址（`[mdns] address` 填错、
/// 换了网段、地址被别人占走）会“注册成功却一条记录也播不出去”——配置看起来生效，局域网里却什么都
/// 解析不到。这里再收紧到“本机拥有”：同网段但不属于本机的地址客户端同样连不上，宁可判为发布失败并
/// 告警，也不对外播一个解析得到却不可达的 A 记录。
///
/// 与 [`select_publish_address`] 分开的原因：地址“不可发布”（回环/链路本地）是 `no-address`，
/// 而“可发布但本机没有”是发布失败，两者的诊断类别必须互相区分。
fn address_is_local(address: Ipv4Addr) -> bool {
    nic_ipv4_addresses().contains(&address)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 本机主路由 IPv4（独立给定的期望值：原型实测的 WLAN 地址）。
    const MAIN_ROUTE: Ipv4Addr = Ipv4Addr::new(192, 168, 5, 4);
    /// 不该被发布的其他网卡地址：WSL vEthernet 与两块链路本地。
    const WSL: Ipv4Addr = Ipv4Addr::new(172, 27, 224, 1);
    const LINK_LOCAL: Ipv4Addr = Ipv4Addr::new(169, 254, 52, 87);
    const LOOPBACK: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 1);

    #[test]
    fn override_address_wins_over_automatic_candidates() {
        // 期望值来自规格：设置 [mdns] address 时发布的就是该值。
        let chosen = select_publish_address(Some(Ipv4Addr::new(10, 0, 0, 7)), &[MAIN_ROUTE, WSL]);
        assert_eq!(chosen, Some(Ipv4Addr::new(10, 0, 0, 7)));
    }

    #[test]
    fn without_override_the_main_route_address_is_published_not_wsl_or_link_local() {
        // 期望值来自规格：未设置 address 时发布主路由 IPv4（独立给定：192.168.5.4）。
        // 候选里刻意混入 WSL 与链路本地，证明它们不会被选中。
        let candidates = [MAIN_ROUTE, WSL, LINK_LOCAL, LOOPBACK];
        assert_eq!(select_publish_address(None, &candidates), Some(MAIN_ROUTE));
    }

    #[test]
    fn loopback_candidate_is_skipped_and_never_chosen() {
        // 回环排在候选最前也不能被选中：跳过它，取主路由 IPv4（独立给定：192.168.5.4）。
        let candidates = [LOOPBACK, MAIN_ROUTE, WSL, LINK_LOCAL];
        assert_eq!(select_publish_address(None, &candidates), Some(MAIN_ROUTE));
    }

    #[test]
    fn only_loopback_and_link_local_candidates_yield_no_address() {
        let candidates = [LOOPBACK, LINK_LOCAL, Ipv4Addr::new(169, 254, 1, 1)];
        assert_eq!(
            select_publish_address(None, &candidates),
            None,
            "候选里只有回环/169.254.x 时必须给出“无可用地址”，绝不退化成回环地址"
        );
    }

    #[test]
    fn non_publishable_override_yields_no_address_instead_of_falling_back() {
        // 覆盖值本身不可发布时不退化成别的地址，也不发布它：跳过发布。
        assert_eq!(select_publish_address(Some(LOOPBACK), &[MAIN_ROUTE]), None);
        assert_eq!(select_publish_address(Some(LINK_LOCAL), &[MAIN_ROUTE]), None);
    }

    #[test]
    fn every_link_local_and_loopback_address_is_unpublishable() {
        for address in [
            Ipv4Addr::new(127, 0, 0, 1),
            Ipv4Addr::new(127, 1, 2, 3),
            Ipv4Addr::new(169, 254, 0, 1),
            Ipv4Addr::new(169, 254, 255, 254),
        ] {
            assert!(!is_publishable_ipv4(address), "{address} 不得被发布");
        }
        for address in [
            MAIN_ROUTE,
            WSL,
            Ipv4Addr::new(10, 0, 0, 7),
            Ipv4Addr::new(169, 255, 0, 1),
        ] {
            assert!(is_publishable_ipv4(address), "{address} 应可发布");
        }
    }

    #[test]
    fn name_change_of_our_own_records_triggers_the_fallback() {
        // 主机名改名（`omp-bot.local.` → `omp-bot-2.local.`）与服务实例改名
        // （`omp-bot._http._tcp.local.` → `omp-bot (2)._http._tcp.local.`）都是我们的记录被改。
        assert!(is_our_record(HOSTNAME));
        assert!(is_our_record(SERVICE_FULLNAME));
        // 局域网里别的设备的改名不关我们的事，也不会让我们注销自己的记录。
        assert!(!is_our_record("other-device.local."));
        assert!(!is_our_record("omp-bot-2.local."));
        assert!(!is_our_record(SERVICE_TYPE));
        // DNS 名字大小写不敏感，事件里的名字可能不是原样。
        assert!(is_our_record("OMP-BOT.LOCAL."));
    }

    #[test]
    fn fullname_constant_matches_the_published_service_instance() {
        // 注销用的是注册时的完整名字：它与实际发布的实例名必须一致。
        let service = build_service_info(MAIN_ROUTE, 8787).expect("固定的记录参数必然合法");
        assert_eq!(service.get_fullname(), SERVICE_FULLNAME);
    }

    #[test]
    fn published_service_info_carries_the_hostname_port_and_only_ipv4() {
        // 独立的规格期望：主机 omp-bot.local.、实例 omp-bot、给定端口，TXT 含访问路径与版本。
        let service = build_service_info(MAIN_ROUTE, 8787).expect("固定的记录参数必然合法");
        assert_eq!(service.get_hostname(), HOSTNAME);
        assert_eq!(service.get_fullname(), "omp-bot._http._tcp.local.");
        assert_eq!(service.get_port(), 8787);
        assert_eq!(
            service.get_addresses_v4().into_iter().copied().collect::<Vec<_>>(),
            vec![MAIN_ROUTE],
            "发布内容只含这一个 IPv4"
        );
        assert_eq!(service.get_property_val_str("path"), Some(ACCESS_PATH));
        assert_eq!(
            service.get_property_val_str("version"),
            Some(env!("CARGO_PKG_VERSION"))
        );
    }
}
