# 05: 跨仓验收：真实 Task 启动、ready 与输入

**What to build:** 提供可重复运行的配对专项入口，从配置、Task 发起和真实终端观察证明生产 ready 与初始输入贯通 TUI/App；本票拥有跨仓测试，不替代 01–04 的本地层检查。依据[合同](../contract.md)的 S1/S2/S3 与[规格](../spec.md)。

**Delivers:** P1, P2, P4, P5, P6, P7, P8, P16, P17, P18, P19, P20, P22, P24, P25, P26, P27, P28, P49, P50, P54, P55

**Blocked by:** 01 — framework 发布会话级 ready；04 — 明确未提交时只重试一次

**Repositories:** herdr-bot, toolkit

**Status:** ready-for-agent

- [ ] 跑 OMP/Pi × TUI/App：使用实际安装的宿主与生产 framework/connect-bot/bot；记录显式选择、旧配置默认 TUI、仅改 backend、App/core 同实例和人为错配、实际当前及默认容器与两种名称（P1/P2/P4–P8）。记录实际模型/thinking、Suite、固定资源及初始命令、Skill、模板效果；App 不安装 TUI 状态扩展仍发起并上报（P6/P22/P49）。
- [ ] 原生初始化、安装宿主的公开订阅和终端事实交叉核对：OMP 闸门未开不 ready；Pi 输入初始化及后续模态反例；旧 owner/错误会话不推进，重复／乱序加页面刷新不重复提交（P16–P20）。不通过替换 editor/handler 或强制输入制造成功。
- [ ] 实际模态下无原生用户消息或业务答复；释放后保留拒绝时间与实际仅一次且不早于 5 秒的重试；两次拒绝后无第三次。退出／替换目标后无投递或替代进程；超时、断线、部分输入、模糊响应及 started:false 按需要注入受控故障，独立观测有无发送；无证据不重发。打包页面、公开状态和延迟原生历史区分初始化、已提交、明确未提交与未知（P24–P28/P50）。
- [ ] 从票据 `herdr-bot` worktree 用独占绝对 `CARGO_TARGET_DIR`，构建当前页面和后端 binaries/安全关机壳，分别通过 `node scripts/test-backend.mjs`、`npm --prefix frontend test`、`npm --prefix frontend run test:browser:build`。从票据 `toolkit` worktree 用冻结依赖分别通过 `bun run typecheck`、`bun run test:framework`、`bun run check:integrations`、`bun run test`；在本次真实安装版本运行受影响的 lifecycle/connect-bot docks，配对 dock 只使用本次安全壳。分别留原始日志。
- [ ] 从 `herdr-bot` worktree 根运行本票专项，文档记录可复制的命令：显式传入两个 worktree 的绝对路径、本次后端、安装版本与隔离实例配置、全组合选择、新证据目录；缺依赖或授权不通过。分别记录真实安装身份、公开状态、原生历史及故障，标明受控 provider 不代表真实模型决策；原生实验只用自有实例与存储，确认所属进程实际退出后清理（P54/P55）。全仓绿、模拟结果和旧实验均不补齐缺项。
