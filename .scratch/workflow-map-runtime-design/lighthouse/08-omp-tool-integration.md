# omp extension 接入与可信执行身份：Lighthouse（部分）

> Question and discussion: [omp extension 接入与可信执行身份](../decision/08-omp-tool-integration.md)

## Question

本部分确定独立 Chat 进程间通信的大体接入方向与既有实现调查要求，不决定具体传输、工具 API 或 extension 布局。本票仍 open，未被协作会话认领。

## Discussion

### herdr-bot 统筹 HTTP 通信

extension 通过 HTTP 与 herdr-bot 交互，由 herdr-bot 统筹跨 Chat 通信、消息事实与路由；不采用轮询消息文件作为传输方案。消息记录用于持久化及查看，不由各 Chat 直接追加共享文件再靠后端扫描发现。具体接收方式、连接管理、投递回执和失败处理的实现由本票继续决定。

SSE、长轮询等尚未锁定；现有后端 POST／SSE 会话观察模式只是参考，不表示已有运行通信接口。若发送结果承诺“已交给 Chat 输入机制”，必须验证该事实，不能把网络写成功冒充模型已阅读。具体确认机制仍未定。

### 优先调查 connect-bot

设计时先核查现有 connect-bot extension 的相关底层实现，评估共享传输代码或整合方式，避免建立重复逻辑；不预先批准将扩展合并。用户提出 manager／executor 两个角色 extension 共用通信代码作为接入设计输入，本票需决定其加载、注册和代码组织；通信票不锁定布局。

确认来源：2026-10-28 [通用协作消息、交付与决策会话](../decision/04-collaboration-protocol.md)中用户明确“先确定大体方案用http请求，由herdr-bot统筹，具体实现方式可以由extension的ticket再进一步讨论”，并要求考虑 connect-bot 底层整合。Q37 明示归属文件 `08-omp-tool-integration.md` 后用户答复“确认”，最终授权发布。早期“必须单一运行 extension”建议被撤回，从未成为批准决定。

### 接入仍须提供的契约

消费[通信权威](04-collaboration-protocol.md)：可信运行上下文、注册时使用规则、初始记录位置、自动输入／唤醒、可打断且默认 90 分钟的可配置 collect 等待。需核查真实 omp API；不以子 Agent 进程内 IrcBus 证明独立 Chat 已能通信。不自动恢复进程或补投消息。

## Decision

采用 herdr-bot 统筹的 HTTP 通信方向，并在具体设计前调查 connect-bot 复用；具体接收、连接、回执与 extension 组织仍未决定。

## User stories

1. As a 独立 Chat 中的参与者, I want 通过 herdr-bot 与其他 Chat 通信, so that 不必依赖共享内存或轮询消息文件。
2. As a 接入维护者, I want 先调查 connect-bot 的既有底层能力, so that 可以复用适用逻辑而不重复建设传输代码。

## Preconditions

- 通信票定义消息语义，持久化票提供保存保证；本票仍需验证真实工具加载与宿主接口。
- connect-bot 是用户指定的调查输入，本轮未核查其底层代码，不能宣称可直接复用。

## Postconditions

- 实现设计不将 SSE 等候选当既定要求，不将 HTTP 方向当成接口已实现。
- 原决策票状态、Assignee 和其他未回答问题不变；不接管其可信身份与生命周期设计。

## Invariants

- 首版只接入 omp，工具 extension 落在 my-agent-toolkit；不建设任意宿主插件平台。
- manager／executor 的工具权限不同不要求复制通信实现；共享代码也不意味着跨进程共享内存。
