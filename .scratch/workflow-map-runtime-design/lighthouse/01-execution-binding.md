# Bot、Task、Chat 与管理角色的绑定：Lighthouse

> Question and discussion: [Bot、Task、Chat 与管理角色的绑定](../decision/01-execution-binding.md)

## Question

确定业务能力、具体工作安排、执行预设与实际对话的职责，以及首版的管理者启动、隔离和执行绑定边界。字段归属由[Map、Bot 与运行配置的契约边界](02-map-definition-contract.md)持有，不在此重复维护字段政策。

## Discussion

### 能力发现与执行分层

Bot 是面向人类或 Agent 规划者的、可发现且可调用的业务能力单元，可以由 LLM 驱动自主解决特定问题；其定义不是运行中的 Agent。规划者根据 use_when 筛选能力，理解能力边界与输出，再形成具体工作安排。node 是 Map 内对能力的具体使用，不是 Bot 的运行时实例。Task 是发起预设，Chat 是实际执行对话。

首版 chat 类型 Bot 显式引用一个 Task，多个 Bot 可共享 Task；node 和管理者不得在派发时临时替换该绑定。不复制 Task 到 Bot 或 node，不为失效引用选择默认 Task。不同目的或固定限制应以不同 prompt 的 Task 管理。

确认来源：本次发布会话 Q3 用户“是”；Q11 用户“完全正确……它是一个可以由llm驱动的，一个能自主解决问题的单元”；Q14 用户“集中在task中管理……创建多个不同prompt的task”。phase3 自动生成 Map 是定位背景，不纳入本次交付。

### 执行输入的组合

执行使用 Task 的固定 prompt 与 node 的具体 input，不自动将 Bot 的能力说明作为第三份业务提示词。Bot 的描述服务于发现与正确调用，不能借此隐式覆盖 Task 的固定限制。具体字段归属、input 替代 parameters 及 zone 决定阶段见[字段契约](02-map-definition-contract.md)。

旧候选 Q5/Q12 曾建议额外注入 Bot 业务说明，已由 Q13/Q14 明确选择 Task.prompt 取代；它们不是并存方案。未批准复杂参数类型系统、运行时提示词生成器或逻辑位置绑定。

### Chat 隔离与运行关联

首版每次派发重新发起独立的原生 Chat，不自动续接历史对话；接入会话用户明确目标 omp 未开启 autoResume，作为环境前提，复用现有启动链且禁止传 --session-dir，不改变原生记录位置；不再展开自动续接及 /new、/resume 边界。相同 Bot、Task 或目录不允许导致隐式共用上下文。新 Tab 不是原生对话隔离的证明。未来 Resume 继承上下文不在本版设计中。

不要求独立的“Bot 运行时实例”对象。Map 运行文件按 node 关联 Chat ID，不复制 Chat 自身的其他状态；这不排除流程自身的派发、完成等事实。Chat 身份使用原生 session_id，启动时先取得并保存关联再提交指示；具体关联与历史消费[接入契约](08-omp-tool-integration.md)及持久化权威；不提前要求独立 execution_id，也不提前禁止必要的可信身份机制。

确认来源：Q2 用户“曳光弹版本先保持每次重新发起Chat的语义”；用户提出“chat运行时对象，除了id在map运行时文件中不需要记录其他字段”；Q23 的完整收口获用户“可以，请发布”确认。后续已发布故障/持久化/接入契约承接最小重试历史与报告适用性，不把未来 Resume 设想作为批准。

### 管理者与 Workspace

管理者不必经过 Bot，也不是伪造的业务 node。首版由人类启动时选择管理者 Task 与 zone；Herdr Workspace 按所选 zone 的目录名推导，不再单独选择。运行系统提供本次运行上下文。是否将工作流包装成可发现的 Bot 属未来能力入口问题，不迫使本版创建管理 Bot。

本次运行记录管理者实际所在的 Workspace 标识，全部节点 Tab 创建到该 Workspace，各节点仍使用自己的 zone。不依赖派发时的“当前 Workspace”，也不退回默认 Workspace；目标不可用则报告无法派发。Workspace 属运行配置而非静态 node 或 Bot。

确认来源：Q4、Q18、Q20/Q21 确定管理者与共享 Workspace。2026-10-28 接入会话用户进一步指定“对应workspace也从选择的zone推导”，替代原单独选择 Workspace 方案；仍记录实际 Workspace，node 不按自己的 zone 重新推导。

### 启动前检查与配置变化假设

启动前检查整张 Map 的已知执行配置错误，包括 Bot 不存在、未绑定有效 Task、Task 不是首版支持的 omp、node.zone 无效。拒绝已知无效启动，不静默替换引用，不审查 input 的业务质量。检查通过不保证稍后的外部启动一定成功，实际故障由故障处理契约承担。

曳光弹版本假设运行期间配置不变，不设计配置快照、版本锁定或变更检测保证；这既不选择“总是读最新”，也不保证运行中修改配置的行为。此选择取代旧交接计划中必须固定全部定义／配置快照的要求，不取消运行事实持久化，也不修改普通历史 Chat 恢复时使用来源 Task 最新环境的既有语义。

确认来源：Q19 用户“假设配置没有变化即可”；Q22 用户“是，可以提前检查最好”；Q23 收口授权发布。

### 证据与限制

本轮静态阅读 backend/src/scheduler.rs 的 Bot、BotImplementation、MapNode，以及 backend/src/lib.rs 的 CreateChatRequest、WorkspaceTarget。当前 Bot implementation 仅有 type；当前请求具有 zone、taskId、chatId、input 等字段，WorkspaceTarget 仅有 Default/Current。它们不是目标设计已实现的证据，尤其不能据此证明可指定管理者实际 Workspace 或强制新原生 Chat。

未执行产品、测试或真实 Agent 验收；本地图只发布设计。Task.prompt 的精确接口、输入组合顺序和宿主能力由接入契约核查，不能将对话中的字段称呼当作已验证 API。

## Decision

采用 Bot 能力定义 → node 具体工作安排 → Task 发起 Chat 的职责划分，管理者由人类直接通过 Task 发起。首版使用独立 Chat、统一管理者 Workspace 和提前配置检查，并在运行期间配置不变的假设下设计。

## User stories

1. As a 流程规划者, I want 发现并理解 Bot 能力与调用要求, so that 我能将需求组织为具体 node。
2. As a 节点执行者, I want 使用 Task 固定指示与 node 的具体输入启动独立 Chat, so that 不同工作不意外共享对话上下文。
3. As a 人类启动者, I want 选择管理者配置并集中节点 Tab, so that 跨 zone 工作仍能在同一 Workspace 观察。
4. As a 运行启动者, I want 提前拒绝已知无效绑定, so that 不因早已可知的配置错误执行部分流程。

## Preconditions

- 字段表达与迁移消费字段契约票据；已发布字段与 Task 类型契约，精确交付整理由跨模块交接票承接。
- 真实 Task → omp Chat 接入必须提供独立会话、精确 Workspace 目标和可信关联；由接入票据验证，不视为现成能力。
- 运行期间配置不变；运行中修改不受本版保证。

## Postconditions

- 后续状态、协作、故障与存储设计遵守上述绑定，不重复引入 Bot 运行实例或隐藏输入来源。
- 运行自己的事实与 Chat 内部状态分开；新旧执行关联及报告适用性消费已发布接入/持久化/故障权威；复杂迟到反馈不扩展首版。
- 本票完成需求与职责决定，不代表字段契约、宿主接入或产品验收已完成。

## Invariants

- 与 GLOSSARY、Task/Chat 命名 ADR 对齐：Bot 不等于 Task，node 不等于运行实例，Chat 不等于 Tab 或进程。
- 复用 zone 发起 Chat 的 Task 预设能力，但不继承可能自动续接的行为假设；zone 与 Herdr Workspace 是不同概念。
- 业务能力说明不是第三份执行 prompt；静态 Map 不承载本次运行状态。
