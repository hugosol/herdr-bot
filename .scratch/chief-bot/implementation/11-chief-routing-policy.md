# 11: 交付澄清复用与能力缺口决策

**What to build:** chief 根据需求和 Bot.when 选择能力、准备参数并接受修正；无匹配走既有完整能力创建 Bot，否决后等待明确指示。

**Delivers:** enabling: unblocks P8, P9, P14, P15, P16, P28

**Blocked by:** 10：交付原生确认式 Task 与 Map 委派工具

**Repositories:** my-agent-toolkit

**Status:** ready-for-agent

来源：[approved contract](../contract.md)、[spec](../spec.md)。

## Acceptance criteria

- [ ] 已知匹配、含糊需求、无匹配分别处理；查询命中不等于目标有效，未知输入先澄清，不以唯一候选或 chief 所在 zone 偷补参数。
- [ ] 单执行者/多步骤缺口分别选择完整创建 Task/创建 Map 能力；不新增同名假工具，不用裸 Task 保存替代创建流程。
- [ ] 仅否决复用时不自选替代；明确要求创建 Task、创建 Map、放弃分别处理，无批准不启动。
- [ ] 创建授权不扩大为原始业务执行，既有阶段审批继续生效；结果返回后不自动检索后续方案或发起下一次执行确认。

## Coverage ownership

P8 及 Task 参数修正真实模型案例由 12；Map 参数修正、能力创建及否决分支由 13；结果呈现后等待由 16。

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
