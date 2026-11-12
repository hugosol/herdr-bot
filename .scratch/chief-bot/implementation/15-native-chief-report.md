# 15: 交付原生 chief 报告与正常呈现

**What to build:** 实际委派的执行者与直接 Map manager 可以在两宿主原生确认后发送完整 chief 报告；chief 正常呈现结果后询问是否满足要求并等待。

**Delivers:** enabling: unblocks P18, P19, P20, P22, P28, P36, P37

**Blocked by:** 02：复用 Session 绑定的原生确认；06：交付两宿主 chief 固定角色入口；14：保存合法 chief 报告后通知当前 chief

**Repositories:** my-agent-toolkit

**Status:** ready-for-agent

来源：[approved contract](../contract.md)、[spec](../spec.md)。

## Acceptance criteria

- [ ] 活动 Session 绑定请求，由后端判断真实报告资格；仅加载工具不授予资格，不用同 Task 名称或模型参数伪造关联。
- [ ] 完整报告内容获得本次原生批准才发送；拒绝、取消、超时、无能力不发送，业务完成或聊天同意不替代原生确认。
- [ ] 新 chief 可仅凭原报告呈现成功、失败、受阻和能力创建结果，不猜测缺失结果；呈现后不启动业务或发起下一执行确认。
- [ ] 保存、通知、不确定及尚无可靠接收确认的事实分开表述；Herdr 接受输入不当作可靠接收，也不虚构已读。
- [ ] 直接 Map manager 对外报告不截取内部节点报告；nested manager 保留原有内外角色与门禁。
- [ ] 共同确认/请求/结果解释归 common，业务不下沉 framework；与 17/18 的持久接收恢复形成 clean cutover，不保留假接收兼容分支。

## Coverage ownership

本票原生受控协议回归；P18/P19/P20/P22/P28/P36/P37 的完整双宿主真实验收由 16。

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
