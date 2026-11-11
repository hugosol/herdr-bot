# 01: 复用报告保存与单次通知

**What to build:** 既有节点报告继续先保存再尝试一次通知，提取 chief 报告将复用的后端共同实现，不改变原节点归属与完成门禁。

**Delivers:** enabling: unblocks P21, P36, P37

**Blocked by:** None (can start immediately)

**Repositories:** herdr-bot

**Status:** ready-for-agent

来源：[approved contract](../contract.md)、[spec](../spec.md)。

## Acceptance criteria

- [ ] 通过公开节点报告接口验证：保存成功后通知失败仍可读回原报告；保存失败不通知、不宣称成功。
- [ ] 嵌套 manager 的内部报告仍发送给其自己的 manager；已有输出、完成资格与失败反馈不变。
- [ ] 外部通知不跨持久化协调锁；请求断开不撤销已保存事实，不新增重试、后台队列或补投。
- [ ] 仅抽取既有共同机制，身份、目标与业务策略仍由各报告路径持有；不新增第二存储权威。

## Coverage ownership

P21 的完整交付由 14；P36、P37 的跨仓验收由 16。

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
