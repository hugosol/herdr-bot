# 13: 恢复 Run Chat 时保留原执行角色

**What to build:** 用户在当前所选宿主恢复 Run 相关历史 Chat 时，沿用原 Run/node/委派关联与角色，不新建执行；降级时如实说明没有装配的资源。依据[合同](../contract.md)、[规格](../spec.md)及 ADR 0015。

**Delivers:** P42

**Test owner for:** P42 的 S1 关联、角色及降级；14 拥有恢复后的真实原生角色行为。

**Blocked by:** 07 — 恢复历史 Chat 并如实显示环境与身份结果；10 — Run 管理者与节点在所选宿主运行

**Repositories:** herdr-bot

**Status:** ready-for-agent

- [ ] 从实际 Run/节点／委派发起记录出发，经公开 Resume 与独立公开运行事实核对原执行 ID、角色及数量不变；不根据来源 Task 类型重建角色、不重放业务输入。
- [ ] 正常恢复保留原关联决定的角色能力；结束节点仍保留普通 Chat 职责，独立普通 Chat 不获得节点职责。
- [ ] 环境降级保留真实执行归属，提示缺少角色资源，不声称资源已加载。不新增 Run 跨端接管或迁移机制。
