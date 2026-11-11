# 05: 交付 chief 入口与单次发起设置

**What to build:** 用户从 server 左侧 chief bot 按钮打开当前 chief；不存在时使用既有 Chat 发起窗口审阅和修改本次设置。

**Delivers:** P1, P2, P4, P5, P6; P3（重复合法发起覆盖）

**Test owner for:** P3（仅本票下述分区；整体完成仍需 03 与 05）。

**Blocked by:** 03：交付可复用 chief Task 定义；04：约束唯一 chief 的创建恢复与登记

**Repositories:** herdr-bot

**Status:** ready-for-agent

来源：[approved contract](../contract.md)、[spec](../spec.md)。

## Acceptance criteria

- [ ] 真实浏览器核对按钮在 server 左侧并可命中；有当前 chief 时打开该 Chat，无当前 chief 时打开窗口，多浏览器选择服从后端。
- [ ] 三个默认值独立字面断言为 chief-bot、default/bot-playground、chat with chief bot；实际提交与显示一致。
- [ ] 仅列出 chief Task；切换 Task 并修改 zone、模型、思考级别及名称，实际启动使用本次值且不改写 Task 预设。
- [ ] 默认 zone 缺失时要求选择已登记有效项，公开登记读回无隐式目录/绑定；选择有效项后能发起。
- [ ] 经合法注销/重新取得身份多次发起后，公开 Task 读回证明 chief-bot 身份和数量不变（P3 的本票唯一案例）。

## Coverage ownership

P3：03 拥有定义测试，本票独占重复合法发起后读回测试；P29/P30 的并发及时间阈值完整案例由 07。

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
