# 02: 选择 TUI 或 App，并核对目标实例

**What to build:** 用户配置一次两端路径后，仅改变 backend 就能选择目标宿主；默认容器与 server 固定操作都遵循所选宿主，不把调用回执当作运行状态。依据[合同](../contract.md)的 S1/S3 与[规格](../spec.md)。

**Delivers:** P1, P2, P3, P4, P5, P8, P9, P51

**Test owner for:** P1/P2/P4/P5 的配置、路由与实例判定，P8 的新操作默认名称解析；05 拥有真实 Task 发起证据。

**Blocked by:** None (can start immediately)

**Repositories:** herdr-bot

**Status:** ready-for-agent

- [ ] 显式 tui/app 与省略（TUI）分别使用预先声明的目标；两端路径不变时只改 backend。独立非法值导致应用启动拒绝且没有宿主创建副作用，不回退另一端。
- [ ] 正确 App/core 的容器与终端属于同一实例；人为错配明确失败，不能拼接异实例响应。以公开操作和独立目标观测验证；真实发起由 05 验收。
- [ ] 新操作的默认名称只取 workspace_name，验证 bot-playground 与另一字面名称；精确匹配零个或多个均拒绝，真实容器集合无任意选择或新建。
- [ ] server 入口使用两端适用的固定启动／停止绑定；在隔离场景分别检查脚本调用结果与实际宿主状态，不显示为服务已就绪／已退出。
- [ ] 沿用已有宿主控制和位置职责，协议差异留在对应 adapter；不接入未被本轮行为使用的上游命令，不修改上游。
