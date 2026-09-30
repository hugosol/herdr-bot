# 内网访问以 mDNS 主机名 omp-bot.local 为主入口

- 状态：accepted
- 日期：2026-09-30

局域网访问的主入口是 mDNS 主机名 `omp-bot.local`（`http://omp-bot.local:<端口>/`），保留 `http://<IPv4>:<端口>/` 作为兜底。该名字由后端进程内的常驻 mDNS responder（`mdns-sd`）发布，**不**由 Axum/Tower 承担——Tower 是 HTTP 请求/响应中间件抽象，而 mDNS 是 UDP 5353 组播，HTTP 请求不经过它。主机名硬编码为 `omp-bot`，不提供主机名配置项；同一局域网里若已有设备占用该名字，**不改名**，而是不发布 mDNS、保留 IP 访问并告警（`mdns-sd` 探测到冲突会发 `DaemonEvent::NameChange`，据此注销）。发布地址默认取主路由 IPv4，可用 `[mdns] address` 覆盖；控制面板只提供 `[mdns] enabled`（默认 `true`）与 `[mdns] address` 两个开关。关机时不特殊注销 mDNS：进程下次启动重新注册的记录带 cache-flush，会刷新客户端缓存，空窗期只是机器本来就关着的时间；正常退出时 best-effort 关闭 daemon。

## 备选与代价

- 备选：只暴露 IP（用户否掉）；自定义域名或内网 DNS（需要路由器/DNS 设施，超出应用范围）；冲突时自动改名（用户否掉，硬编码域名不能被改）；发布全部非回环 IPv4（用户否掉，客户端可能先拿到 VPN/WSL 地址）。
- `.local` 能否解析取决于客户端与网络：Windows 10/11 的 mDNS 解析受 `mDNSEnabled` 策略影响，Android 13+ 才原生支持，旧设备与启用组播隔离的 Wi-Fi 可能失败；mDNS 本身无认证、名字可能冲突。
- 因此 IP 访问、真机验收与“仅可信局域网”的限制都不可省。
