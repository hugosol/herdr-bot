# 10: 适配 manager 原生工具指引与脚本结果呈现

**What to build:** manager 继续使用原 run/collect 工具管理无 Chat 的脚本节点，正确理解派发返回、自动报告、显式 complete 和失败后的 fail/retry，不寻找脚本 Chat 或新增工具。

**Delivers:** enabling: unblocks P34, P35

**Blocked by:** 01 — 明确脚本执行与业务材料契约，解除必要的 Chat 耦合。

**Repositories:** toolkit

**Status:** ready-for-agent

## Acceptance criteria

- [ ] 消费 01 明确的公开执行/报告表示，检查原 manager 工具请求与结果通路；保留原 run/collect 动作、Session 自动身份和后端归属检查，不让模型填写脚本 Run/execution/收件者身份，不新增脚本专用工具。
- [ ] 指引明确脚本 dispatch 返回表示已派发，不表示业务结束；自动报告进入原通知/collect，collect 不含正文时按原显式记录读取，不期待节点 Chat 或单独接收工具。
- [ ] 指引说明 true 只允许 manager 显式 complete，不要求模型重复确定性 Rust 业务验收，不自动完成/派发下游；区分执行结束、node 状态与交接。
- [ ] 失败结果呈现原错误、实际效果/限制。先显式 fail，再按原确认规则 retry；脚本重跑产生新执行但没有新 Chat，已结束脚本不适用 recover。旧报告不能完成新执行。
- [ ] 保留 Task/Map 节点的 Chat、recover、嵌套角色和原报告确认语义；不要全局将 retry“有新 Chat”改成“所有类型都无 Chat”，也不改变 chief 入口或权限。
- [ ] OMP/Pi 适用入口实际加载更新的角色指引和结果处理。受控协议专项覆盖脚本结果/错误消费及原动作透传，不以提示词全文快照、工具注册或类型检查冒充真实 manager 会话闭环。
- [ ] 更新 toolkit 现有角色/协议文档，明确本票受控协议检查不证明真实业务模型理解或 H1/H2 完成，提供 11 可运行的原生适配及专项入口。

## Verification and ownership

本票仅为 P34/P35 提供可用工具适配及指引，最终跨仓测试/人工证据唯一归 11。不修改协调仓文件或创建第二份 ticket 序列，不重复认领 S1 后端完成门禁。

在 toolkit 对应 worktree 根按该仓规则和 frozen lock 使用 Bun 1.4.4，分别完成 `bun run typecheck`、`bun run test`、`bun run check:integrations`，并运行受影响的 manager 原生协议专项，记录实际命令和覆盖边界。若原生 dock 需要配对后端，由 11 提供参与 worktree 的安全关机壳与元数据；不能以生产后端运行关机回归。隔离原生状态、网络和子进程，确认退出后清理，不接触正常会话或发现用户凭据。
