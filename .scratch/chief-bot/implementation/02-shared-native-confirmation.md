# 02: 复用 Session 绑定的原生确认

**What to build:** 在现有节点报告路径复用原生确认、活动 Session 绑定请求及结果解释，使后续 chief 工具沿用相同执行门禁。

**Delivers:** enabling: unblocks P10, P20, P22, P36, P37

**Blocked by:** None (can start immediately)

**Repositories:** my-agent-toolkit

**Status:** ready-for-agent

来源：[approved contract](../contract.md)、[spec](../spec.md)。

## Acceptance criteria

- [ ] 原节点 manual 报告的预览包含实际完整载荷；批准前无报告请求，批准后仅发送批准载荷。
- [ ] 两宿主均覆盖拒绝、取消、超时、迟到批准、会话结束和无确认能力；均不发送、不自动重试。
- [ ] HTTP 已发出后的取消、响应丢失及非法响应保持结果不确定，不输出未发生或已回滚。
- [ ] 共同业务留在 workflow-chat common 层；宿主差异由公共 framework interface 或薄原生装配承载，不依赖私有 framework。

## Coverage ownership

P10 的 Task/Map 案例分别由 12/13；P20、P22、P36、P37 由 16。

## 约束与证据

- 继承已批准 contract 的 S1/S2/S3、Implementation Decisions 与全部 Out of Scope，不重新设计 seam。公开操作的具体参数由实现确定，但必须与消费方同步，不留兼容空壳、假工具或假结果。
- 本票只拥有下述 coverage item 的测试；依赖票的永久用例可引用，不复制同一场景重新认领。受控响应仅证明协议/故障，不能证明真实模型职责。
- 所有实际模型调用，包括烟测，固定 GPT-6-luna，记录实际返回身份；不可用或身份不符即阻塞，不回退。测试驱动只提供用户输入/原生批准，不替模型选择业务工具或伪造结果。
- 使用测试独占定义、存储、目录、动态回环端口、HOME/APPDATA/XDG、原生会话和 Herdr 实例；不读取用户凭据/Chat，不操作正常服务或真实关机。明确授权的独立验收凭据只通过现有安全入口使用，不写入证据。
- 时间与无自动行为检查声明实际配置、启动/确认/维护预算及有界观察窗口；不得用即时断言、无限等待或新增产品等待/重试策略冒充证明。保留原始失败和清理失败；退出并确认所属进程结束，不以删除目录代替。
- 功能完成后更新对应仓库现有使用/测试说明或 changelog；记录命令、退出状态、源码/锁文件/产物/安装宿主身份及逐项证据。缺真实依赖或原生能力保持未验证，不能把计划内自动验收交给用户代办。

## 本仓检查

在本票 my-agent-toolkit worktree 根使用 Bun 1.4.4：

- TypeScript 变更运行 `bun run typecheck`。
- 运行 `bun test tests/harness-dock`，覆盖受影响的 OMP/Pi 实际原生工具/确认/Session 路径及本票新增行为；共享 common 变更还须检查现有消费者。
- 共享行为变更完成时运行 `bun run test`；涉及框架/宿主装配或依赖边界运行 `bun run check:integrations`。静态检查不代替原生行为。
- 重型运行遵循仓库安静窗口和既有并发策略；定向绿不宣称全仓绿，不以其他仓构建替换本 worktree 源码。
