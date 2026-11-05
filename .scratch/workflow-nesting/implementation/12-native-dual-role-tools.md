# 12: OMP 与 Pi 双角色工具及指引共存

**What to build:** 同一原生 Session 同载 manager-chat 与 node-chat，工具及指引各自生效，明确内部管理和外部交付的作用层级，保持普通单角色行为。

**Delivers:** enabling: unblocks P20, P22, P25, P32, P34

**Blocked by:** None (can start immediately)

**Repositories:** toolkit

**Status:** ready-for-agent

依据：[批准契约](../contract.md)；[规格](../spec.md)。契约 Approval 已获批准，本轮 24 票修订方案由用户授权发布。发布不代表实现、自动验收或用户部署操作已完成。

## Acceptance criteria

- [ ] 分别在实际 OMP／Pi 同一 Session 同载两扩展，run／collect／report 均可发现并实际调用；不能用两个单角色进程代替。
- [ ] 两套指引同时贡献、不覆盖、不重复注入；说明内部 run/collect 与外部 report、显式 finish 后仍可报告及外层 manual 的边界，不固定提示词全文快照。
- [ ] 身份仍取自公共原生 Session observation；拒绝模型自报 session、Run、execution、接收者等越权字段，保留严格参数处理及失败阶段。
- [ ] 保留 manager-only／node-only 行为及取消、session_end、恢复资源生命周期；finish 不被当作 session_end。
- [ ] 复用已有 adapter、独立 extension 实例与指引身份，不预设必须重构 framework。
- [ ] 可在受控 HTTP 的原生 dock 验证同载机制，但不得声称真实后端授权、真实业务遵从或完整承诺已通过；与 04／09 协调公开关联语义，不自造字段协议。
- [ ] 所有受影响调用方与既有行为检查同步切换，删除被本次替代的死路径，不引入旧字段别名或以兜底吞掉失败。
- [ ] 更新所属长期使用／运行／验证说明，记录真实执行的入口及证据边界；临时 ticket／证据不作为长期文档唯一依赖。

## 覆盖分区与测试所有权

| Promise | 本票覆盖项／实现贡献 | Contract 测试 owner | 其余归属 |
|---|---|---|---|
| P20 | 原生同载机制贡献 | 14 | 完整跨仓覆盖归 14，本票机制检查不冒充该用例 |
| P22 | 双层工具及身份注入贡献 | 14 | 真实后端定位测试：04／14 |
| P25 | finish 与会话生命周期分离贡献 | 14 | 真实 finish 后 report：14 |
| P32 | 恢复资源与双指引贡献 | 15 | 服务端：09；原生恢复：15 |
| P34 | 内外报告策略指引贡献 | 14 | 真实不同策略／确认：14 |

表中 owner 是相应覆盖项的唯一测试编写与维护者。一个用例可以支持多个 promise，但不能由两票重复认领；promise 必须在全部覆盖项通过后才完整交付。enabling 票的单仓机制／公开接线检查不替代上述 contract 覆盖。

## Worktree checks 与机制烟测

在本票 toolkit worktree 根执行，遵循其测试规范、Bun 1.4.4 及昂贵检查协调约束：

- `bun test tests/workflow-chat`。
- `bun test tests/harness-dock/role-identity-integration.test.ts tests/harness-dock/role-prompts.test.ts tests/harness-dock/role-session-isolation.test.ts`，并扩展同一 Session 双角色的真实 OMP／Pi dock；原单角色检查不是同载证据。
- 运行受影响 collect／取消／原生恢复 dock；`bun run typecheck`；`bun run test`。
- 配套真实后端 dock 时，用参与 herdr-bot worktree 构建安全壳及 herdr-fixture；显式设置 `HERDR_BOT_REPO`、`HERDR_BOT_METADATA` 与 `HERDR_BOT_BINARY` 指向该 worktree 和测试安全产物。不同源码使用独立 `CARGO_TARGET_DIR`；必要 baseline sibling 定位仅作只读 oracle。

实际启动两个宿主的正常扩展入口，在同一原生 Session 观察三工具及双指引、严格参数和生命周期结果；保留原始请求及宿主日志。受控 HTTP／provider 只证明机制，不代替 14／15 的真实后端和模型行为。缺宿主保持失败／未验证，不安装依赖或改全局 PATH 绕过。成功失败均清理所属进程，不操作用户 Session。

## 完成标准

上表本票覆盖项、全部验收条件和所属仓库检查均有实际通过证据后才能关闭。Full automation 无人工验收豁免；真实依赖缺失如实保留阻塞，不以提示词存在、配置文件、可编译 scaffold、受控成功或单次最终状态宣称完整交付。
