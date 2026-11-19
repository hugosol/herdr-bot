# 14: 跨仓验收：chief 与 Run 的真实业务链路

**What to build:** 提供可重复运行的配对专项入口，证明生产链路在所选宿主保留 chief 授权、Run 角色、首条业务输入顺序和报告权威语义。依据[合同](../contract.md)的 S1/S2/S3 与[规格](../spec.md)。

**Delivers:** P23, P37, P38, P39, P40, P41, P42, P52, P54, P55

**Blocked by:** 01 — framework 发布会话级 ready；04 — 明确未提交时只重试一次；09 — chief 授权分派并通知当前接收会话；13 — 恢复 Run Chat 时保留原执行角色

**Repositories:** herdr-bot, toolkit

**Status:** ready-for-agent

- [ ] 覆盖真实 TUI/App，涉及 agent 输入、环境、身份或生命周期时跑 OMP/Pi × TUI/App。通过公开流程观测 chief 分派、普通管理者与普通节点、嵌套管理者、委派、结束节点；用第一条真实终端业务输入对照此前保存的 Chat/原生/Run 关联，核查权限与既有业务结果（P23/P37/P39）。
- [ ] 预设当前 chief 并非原分派者，观察真实接收会话及投递；分别制造 chief 与 Run 通知失败，保留原故障与可读回的同一报告。检查实际初始提交重试不重放业务创建、恢复或普通报告通知；独立记录外部调用次数（P38/P40/P52）。
- [ ] 观察 Run 目标的原生布局／焦点、错误目标与聚焦失败；验证恢复后的真实角色行为与原执行关联，未新建执行。环境降级时不声称角色资源已加载，不迁移旧 Run（P41/P42）。
- [ ] 从本票 `herdr-bot` worktree 用独占绝对 `CARGO_TARGET_DIR` 构建本次页面和后端 binaries/安全关机壳，分别通过 `node scripts/test-backend.mjs`、`npm --prefix frontend test`、`npm --prefix frontend run test:browser:build`；从本票 `toolkit` worktree 用冻结依赖分别通过 `bun run typecheck`、`bun run test:framework`、`bun run check:integrations`、`bun run test`，另跑受影响的 connect-bot、报告与角色 docks，配对 dock 使用本次安全壳。分别归档原始日志。
- [ ] 从 `herdr-bot` worktree 根运行本票专项，文档提供可复制命令，显式传入两仓绝对路径、本次后端、隔离配置、所需场景及新证据目录。按场景保存安装身份、原生记录、公开关联、失败及退出证据；仅使用自有实例，明确标注受控依赖与授权真实网络，不碰现存 Chat 或认证（P54/P55）。缺授权或真实场景不以全仓通过补齐。
