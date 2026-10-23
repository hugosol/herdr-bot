# 会话写入只接受同机来源，读取对局域网开放，不引入 token

- 状态：accepted
- 日期：2026-10-01

> 术语注记（2026-10-21）：项目已更名 **herdr-bot**，mDNS 主机名现为 `herdr-bot.local`；原因与硬切换范围见 [ADR 0010](./0010-rename-project-to-herdr-bot.md)。正文保留原文。

`POST /api/sessions/events`（`register` / `heartbeat` / `state` / `unregister` 四种事件）只接受**同机来源**
的请求：TCP 对端 IP 是回环地址（`ip.is_loopback()` 覆盖 `127.0.0.0/8` 与 `::1`），或属于本机网卡地址集合
——后者在**每次写入**时用 `if-addrs` 现枚举，DHCP 换址后旧地址不会残留成误放行的口子；枚举失败时按「不是
本机」拒绝，边界默认关闭。其余来源一律 `403 forbidden`，且拒绝发生在建行或改动任何会话之前。读取侧相反：
页面与 SSE 长连接 `GET /api/sessions/events` 对局域网**全开**，**不引入 token**。

默认端点沿用扩展配置里的 `http://omp-bot.local:8787`（`extensions/connect-bot/config.json` 单键
`endpoint`）：同机扩展经 mDNS 主机名连进来时，服务端看到的对端地址是本机网卡地址，仍在接受范围内，因此
ADR-0001 的「mDNS 主机名为主入口」不受影响，扩展不必改配置。**v1 不支持非同机扩展**（跨机需要认证与实例
身份，属新 effort）。

这是 ADR-0002「仅可信局域网、无认证」信任边界之内的一次收紧：拓扑固定为「omp 扩展与 omp-bot 同机、浏览器在
局域网」，这条闸门保护的是会话写入的完整性（局域网设备不能伪造或刷会话行）。但**来源校验不是认证**——同一台
机器上的任意进程仍然可以写。实现落在 `backend/src/sessions.rs` 的 `is_local_source`；来源校验要求拿到 TCP
对端地址，因此 `backend/src/main.rs` 的 `axum::serve` 与测试 `TestServer`（`backend/tests/common/mod.rs`）
都改用 `into_make_service_with_connect_info::<SocketAddr>()`。

## 备选与代价

- 备选：**仅回环**（只接受 `127.0.0.1`）——会挡掉同机经由 `omp-bot.local` 发来的连接，其源地址是本机网卡
  地址而非回环；采用它就得把扩展默认端点改成 `127.0.0.1:8787`，与 ADR-0001「mDNS 主机名为主入口」相抵
  —— 否决。**局域网全开**（ADR-0002 现状）——零接线，但局域网任意设备都能伪造或刷会话行 —— 否决。
  **轻量 token**——需要分发与配置，超出 v1 范围 —— 留待非同机扩展或多用户（新 effort）。
- 代价：来源校验不是认证，同机任意进程仍可写；且 `axum::serve` 与测试 `TestServer` 必须改用
  `into_make_service_with_connect_info::<SocketAddr>()` 才能拿到对端地址（`if-addrs` 已是 omp-bot 依赖，
  枚举本身不引入新依赖）。
- 因此写入与读取是两条不同的边界：局域网设备读页面与 SSE 无需任何凭据，但写会话事件只有同机来源能通过。

## v1 单实例假设

v1 假定同一台机器上只有一个 omp-bot 实例：会话协议**不带实例身份**，因此「端点指向另一个实例」与「原实例
重启」在线上不可区分、行为一致——新实例不认识旧会话，非 `register` 事件回 `404 unknown-session`，扩展连续
3 次失败后自动 disarm、需手动重新 `/connect-bot`；两条路径退化为同一条恢复路径。区分实例（多实例身份、
路由、隔离）属于新 effort，不在 v1 范围内。

> 历史讨论路径为 `.scratch/omp-connect-bot/lighthouse/11-write-boundary-adr.md`（当前检出不包含该文件）；本 ADR 承接这一决策。
