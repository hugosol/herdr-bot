# 20: 跨角色归属与公共扩展能力集成验证

**What to build:** 实际 manager/node 扩展在不同 cwd 和稳定配置下正确装配；跨 Run、跨角色、无关联及旧执行冒充当前执行的越权调用被拒绝，connect-bot 关闭不影响运行。旧执行迟到报告按已批准规则保留历史，但不能作为当前执行依据。

**Delivers:** P22, P23, P58, P60, P67

**Test owner for:** P60.02, P67.04（仅本票所列分区；整个 promise 须全部分区完成）。

**Blocked by:** 10 — collect 通知保留、有界等待与取消；13 — retry 独立新执行与报告适用性

**Status:** ready-for-agent

## 契约与范围

依据：[approved contract](../contract.md) 与其继承的 [spec](../spec.md)。P 编号与 S1–S4 的含义以 contract 为准；本票不重新定义 promise 或新增观察 seam。

基本入口约束与工具指引已随功能交付，本票负责跨场景矩阵、自动装配及公共 interface 的集成证据，并修复发现的问题。使用真实 session_id 的后端关联，不接受模型自报 Run/role/node；同机来源限制不是认证，不增加 token 或接收连接。

仅交付本票的覆盖分区；共享 promise 不因某一分区通过就算整体完成。实现前沿是所有列出的阻塞票均完成的票，不按编号强制全串行。

## 验收标准

- [ ] **P22.01**（本票编写并执行的契约检查）：检查实际工具参数不接受模型自报归属；两 Run、manager/node、无关联及旧/当前身份交叉矩阵覆盖允许和拒绝，后端按真实 session 关联及当前执行判断，不能用普通正例替代负向覆盖。
- [ ] **P23.01**（本票编写并执行的契约检查）：按既有网络护栏使用允许/拒绝来源的独立样例；同机但无关联仍拒绝，session_id 非秘密凭据，保留同机冒用风险说明，不引入 token，不擅自监听 LAN/mDNS。
- [ ] **P58.01**（本票编写并执行的契约检查）：真实双角色自动装配、Task 类型切换和不同 node cwd 下检查工具/指引/目标及稳定配置；显式接入自动启用策略，不能用入口文件存在代替运行成功。
- [ ] **P60.02**（本票编写并执行的契约检查）：connect-bot 未启用仍实际运行；验证 toolkit 公共 interface 兼容及依赖方向：角色消费 common，common 不反向依赖角色，foundation 不依赖 business，不依赖 connect-bot 私有实现，不为内部 helper 新建 seam。
- [ ] **P67.04**（本票编写并执行的契约检查）：不同 node cwd 下显式 endpoint、config.path/configBaseDir 定位稳定，真实请求目标及有效配置一致，不按角色 ID 偶然推导目录。
- [ ] 完成上述能力所需调用者、错误反馈和角色/用户指引；不留占位实现、假成功、运行时旧格式兼容或未接线入口。
- [ ] 保留本票真实入口的可重复验证证据及检查结果；相关长期说明随能力更新，不只更新临时票据。

## 验证与证据

实际启动两角色、切换 Task 类型、改变 node cwd 和非默认 endpoint/configBaseDir；记录自动启用、Prompt 注入与请求目标。执行交叉身份和来源负向样例，以及公共 HTTP 取消与依赖规则检查。

遵守仓库测试指引：复用真实 HTTP TestServer、现有前端/npm 浏览器入口及 toolkit 公共 interface；页面能力观察实际交付页面，宿主能力需要真实 Herdr/omp 证据。受控 adapter 只证明确定性故障/时序，不证明模型或原生宿主行为。

普通检查只用数值回环、独占临时数据/cwd/动态端口与必要 Herdr 副本，持续消费并清理所属子进程；不操作用户对话或产物，不绕过 npm 网络护栏。LAN/mDNS、真实符号链接和关机仍遵守显式人工授权规则。

自动化预期取自契约、独立字面样例或预先算定结果，不用被测实现重算预期。负向自动行为检查注明有界观察窗口并保留外部调用证据；取消/错误不得伪造快照或自动重试。真实环境缺失如实报告阻塞，不以替身或跳过宣称通过。

## 共享 promise 的 coverage partition

下面列出本票涉及的共享 promise 的完整分区。每个分区只有一个交付票和测试 owner；其他票的行是依赖说明，不是本票重复认领的验收项。各分区最终均通过后 promise 才完成。

| Promise / coverage item | Coverage | Ticket | Test owner |
| --- | --- | --- | --- |
| P60.01 | collect 使用 toolkit 公共 HTTP 的调用方取消能力，验证取消/断开及后续请求，不复制私有客户端或创建业务协议 framework。 | 10 | 10 |
| P60.02 | connect-bot 未启用仍实际运行；验证 toolkit 公共 interface 兼容及依赖方向：角色消费 common，common 不反向依赖角色，foundation 不依赖 business，不依赖 connect-bot 私有实现，不为内部 helper 新建 seam。 | 20 | 20 |
| P67.01 | 非默认运行存储根与不同服务工作目录下观察真实记录位置，沿项目既有配置机制，不依赖偶然默认目录。 | 04 | 04 |
| P67.02 | 非默认 collect 期限和大于后端期限的请求预算实际生效，不沿用 connect-bot 短预算。 | 10 | 10 |
| P67.03 | 非默认轮询配置在实际页面请求节奏生效，无界面配置平台。 | 19 | 19 |
| P67.04 | 不同 node cwd 下显式 endpoint、config.path/configBaseDir 定位稳定，真实请求目标及有效配置一致，不按角色 ID 偶然推导目录。 | 20 | 20 |

## Comments

尚未实施或验收；本票发布不代表其 promise 已通过。
