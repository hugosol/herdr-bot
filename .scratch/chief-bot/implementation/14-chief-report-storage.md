# 14: 保存合法 chief 报告后通知当前 chief

**What to build:** 仅真实 chief 委派的独立 Chat 或直接 Map manager 可提交 chief 报告；后端先保存，随后一次性通知当前有效 chief。

**Delivers:** P17, P21; enabling: unblocks P18, P19, P20, P22, P35, P37

**Blocked by:** 01：复用报告保存与单次通知；08：按 Bot 委派独立 Task Chat；09：按 Bot 委派真实 Map Run

**Repositories:** herdr-bot

**Status:** ready-for-agent

来源：[approved contract](../contract.md)、[spec](../spec.md)。

## Acceptance criteria

- [ ] 公开接口对比合法独立 Chat/直接 manager、无关联身份、同 Task 的其他 Chat、普通节点和伪造归属参数；不凭工具存在或名称授权（P17）。
- [ ] 通知触发前报告已可公开核实；无 chief、通知失败后重启仍读回同一报告，保存失败不通知、不宣称成功（P21）。
- [ ] 报告保留需求背景、执行对象、成功/失败/受阻结果、交付物及待判断事项；目标来自当前权威，不接受调用者指定任意收件人。
- [ ] 保存/通知/不确定结果分开反馈；共享保存与通知机制但保留 chief 与 node 各自身份和路由策略，不把后端存储搬到扩展。
- [ ] 为原报告只读、稳定身份和后续接收/重发提供公开行为；失败无自动重投，初始化/恢复不扫描补投。

## Coverage ownership

P17、P21 的 S2 消费者测试归本票；P18/P19/P20/P22/P36/P37 由 16；P35 的完整内容不变与重发验收由 19。

## 约束与证据

- 继承已批准 contract 的 S1/S2/S3、Implementation Decisions 与全部 Out of Scope，不重新设计 seam。公开操作的具体参数由实现确定，但必须与消费方同步，不留兼容空壳、假工具或假结果。
- 本票只拥有下述 coverage item 的测试；依赖票的永久用例可引用，不复制同一场景重新认领。受控响应仅证明协议/故障，不能证明真实模型职责。
- 所有实际模型调用，包括烟测，固定 GPT-6-luna，记录实际返回身份；不可用或身份不符即阻塞，不回退。测试驱动只提供用户输入/原生批准，不替模型选择业务工具或伪造结果。
- 使用测试独占定义、存储、目录、动态回环端口、HOME/APPDATA/XDG、原生会话和 Herdr 实例；不读取用户凭据/Chat，不操作正常服务或真实关机。明确授权的独立验收凭据只通过现有安全入口使用，不写入证据。
- 时间与无自动行为检查声明实际配置、启动/确认/维护预算及有界观察窗口；不得用即时断言、无限等待或新增产品等待/重试策略冒充证明。保留原始失败和清理失败；退出并确认所属进程结束，不以删除目录代替。
- 功能完成后更新对应仓库现有使用/测试说明或 changelog；记录命令、退出状态、源码/锁文件/产物/安装宿主身份及逐项证据。缺真实依赖或原生能力保持未验证，不能把计划内自动验收交给用户代办。

## 本仓检查

从本票 herdr-bot worktree 根运行，使用本 worktree 独立绝对 CARGO_TARGET_DIR 和构建的静态页面，不借用另一 worktree 的二进制：

- 后端/共享实现变更完成时：`node scripts/test-backend.mjs`。
- 前端变更：`npm --prefix frontend test`；涉及真实页面/HTTP/进程的验收：`npm --prefix frontend run test:browser:build`。
- 开发反馈可用新/既有公开行为测试的定向入口；它不能代替上述受影响层的完成检查。报告实际测试名称及证据，不能把本文当已执行记录。
