# 从历史会话列表 Resume Chat

Status: ready-for-agent

Contract: [已批准合同 — Rapid iteration](contract.md)

## Problem Statement

用户在 herdr-bot 中可以查看历史会话列表，但不能从列表直接回到已有 Chat，也不能在原 agent 进程退出、Herdr Tab 关闭之后加载原生对话。用户必须自行查找终端、会话文件或 agent 恢复命令。

历史会话列表独立于当前会话登记：登记离线、注销、被清除或服务重启，不表示 Chat 被删除。当前历史保存原生 session ID、可选 agent 类型、工作目录及可选会话文件引用，但没有持久保存 Herdr Workspace 信息。历史既包含通过 zone 发起的 Chat，也包含通过 connect-bot 接入、不一定原本由 Herdr 托管的 Chat。

用户需要统一的 Resume 入口：对话仍在 Herdr 中时直接定位；否则通过 Herdr 加载同一原生对话，供查阅或由用户继续交流，而不是自动重新执行初始任务。

## Solution

为历史会话列表中的每个 Chat 提供 Resume 操作，不以已有 Herdr 绑定为前提。统一先查找 Herdr 中对应的已有对话，找到就切换焦点；未找到才在 Herdr 中创建新的 Tab 并启动对应 agent 加载原生对话。

新启动的恢复优先回到记录中的 Herdr Workspace；记录缺失或原 Workspace 已删除时，使用配置中的默认 Herdr Workspace。已有对话的定位始终切到其实际所在位置，不为了满足 Workspace 偏好再启动副本。

恢复不发送 Bot 固定初始指示，不重新应用 Bot 预设的模型或思考级别，不自动继续中断的任务。用户看到明确的定位、恢复、警告或失败反馈；无法确定启动结果时不把它误报为成功或安全失败。

## User Stories

1. As a herdr-bot user, I want a Resume action on every historical Chat, so that I can return to a conversation without manually locating its terminal or session file.
2. As a herdr-bot user, I want to resume a Chat after its current session registration disappears, so that registration cleanup does not prevent access to conversation history.
3. As a herdr-bot user, I want to resume a Chat originally connected through connect-bot, so that prior Herdr ownership is not required to load my conversation.
4. As a herdr-bot user, I want Resume to look for the conversation already hosted in Herdr before starting an agent, so that an existing conversation is reused instead of duplicated.
5. As a herdr-bot user, I want a matching conversation to receive Herdr focus, so that I can immediately locate its Workspace, Tab and Pane.
6. As a herdr-bot user, I want locating an existing Chat to send no conversation input, so that a running or idle agent is not instructed to do additional work.
7. As a herdr-bot user, I want a warning when multiple Herdr targets match the Chat, so that I know the conversation is already open in multiple places.
8. As a herdr-bot user, I want one matching target to be selected automatically when several exist, so that Resume does not require an extra selection interaction.
9. As a herdr-bot user, I want a new Herdr Tab to load the original conversation when no existing target is found, so that closing the original Tab does not make the Chat inaccessible.
10. As a herdr-bot user, I want focus to move to the newly restored conversation, so that I do not have to search Herdr after Resume finishes.
11. As a herdr-bot user, I want the original Herdr Workspace to be remembered, so that later recovery returns the Chat to the same grouping when possible.
12. As a herdr-bot user, I want records without Workspace information to use the configured default Herdr Workspace, so that older history remains usable.
13. As a herdr-bot user, I want a deleted original Workspace to fall back to the configured default, so that removing a Workspace does not prevent recovery of the conversation itself.
14. As a herdr-bot user, I want a clear error if the default Workspace cannot be resolved, so that Resume does not silently place my Chat somewhere arbitrary.
15. As a herdr-bot user, I want a stored session file to be preferred when loading a conversation, so that the intended native conversation can be identified precisely.
16. As a herdr-bot user, I want a native session ID to be usable when a session file reference is unavailable, so that history without stored file paths can still be resumed.
17. As a herdr-bot user, I want recovery to use the recorded working directory, so that agent-specific session lookup happens in the intended project context.
18. As a herdr-bot user, I want an explicit error for a missing working directory, unknown agent type or unavailable native conversation, so that a different or empty conversation is not substituted for my Chat.
19. As a herdr-bot user, I want Resume to avoid resending the Bot's initial instructions, so that revisiting history does not repeat the original task.
20. As a herdr-bot user, I want Resume to avoid reapplying the Bot preset's model and thinking settings, so that returning to a Chat is not treated as launching a new Chat from a preset.
21. As a herdr-bot user, I want restoration to load history without automatically continuing interrupted operations, so that I decide whether and when work resumes.
22. As a herdr-bot user, I want simultaneous Resume requests for the same Chat to result in at most one in-flight operation, so that repeated clicks or requests from multiple pages do not launch duplicate processes.
23. As a herdr-bot user, I want an agent startup timeout to be reported as an unconfirmed result, so that I do not mistake a possibly running process for a safe opportunity to start another copy.
24. As a herdr-bot user, I want the target created by an unconfirmed startup to remain available for inspection, so that useful state is not destroyed merely because startup confirmation timed out.
25. As a herdr-bot user, I want subsequent Resume attempts to search for an existing conversation again, so that an agent that became available after a timeout can be located rather than duplicated.
26. As a herdr-bot user, I want Herdr query errors to be distinguished from an empty result, so that a failed lookup does not accidentally trigger a duplicate startup.
27. As a herdr-bot user, I want older history records to remain readable without manually adding new metadata, so that the feature does not require rewriting my existing history.
28. As a herdr-bot user, I want success messages to describe Herdr navigation accurately, so that internal focus changes are not mistaken for bringing a Windows desktop window to the foreground.

## Implementation Decisions

### 统一流程与身份

- 所有历史 Chat 均提供 Resume；已有 Herdr 绑定不是恢复资格。具备恢复入口不保证记录中的信息、文件和外部程序一定可用。
- 每次 Resume 都先查找 Herdr 中与该原生对话相符的已有 agent，再决定定位或新启动。当前会话登记的在线／离线状态不能代替 Herdr 查询。
- 查找依据是原生对话身份，不以 Chat 显示名称、Bot 名称或 cwd 相同推断为同一对话。Herdr agent 名称、Pane 身份与原生 session ID 不能混用。
- Herdr 可能只返回会话路径或 ID 中的一种；实现必须处理这种身份表达差异，不把“返回的是路径而历史保存的是 ID”直接当作不同对话。具体解析、匹配机制留给后续设计，不在本 spec 中指定 seam。
- 唯一匹配时直接定位；多个匹配时页面给出明确提示，按稳定、可重复的顺序自动选择其中一个。不提供额外选择交互，不再启动副本。
- 查询失败不是未匹配；查询或焦点操作失败时报告对应错误，不能通过新启动掩盖失败。

### 新进程恢复

- 只有成功完成查找且没有匹配目标时，才新建 Herdr Tab 并启动 agent 加载指定原生对话；成功恢复后切换到新目标。
- 优先使用历史保存的会话文件引用。没有可用文件引用时，允许以原生 session ID 在记录的工作目录中恢复，前提是仍明确加载同一原生对话。
- Pi 使用其原生 session 加载能力；OMP 使用 resume 能力，不使用会继续中断工作的 restore 能力。
- agent 类型必须能够确定且受支持；未知、缺失或不支持时明确失败。原生对话无法找到、工作目录不存在时明确失败，不猜类型、不换目录、不创建空白对话冒充恢复。
- 启动使用当前配置的 agent 命令及所需集成，不重放 Bot 固定初始指示，不重新应用 Bot 预设中的模型或思考级别。Resume 的目的为查阅或由用户继续旧对话，不自动向 agent 提交新一轮输入。
- 恢复是新进程加载原生对话，不是复活旧 PTY、旧 PID、普通 shell 任务或任意外部进程。

### Herdr Workspace 与历史兼容

- 持久历史增加用于识别 Herdr Workspace 的可选信息；保持旧记录无该信息时仍可读取和发起 Resume。字段形态、更新入口及接口划分留给后续设计。
- 原 Workspace 可用时，在该 Workspace 中新建恢复目标；记录缺失或原 Workspace 已删除时，回退到配置中的默认 Herdr Workspace，并在操作结果中说明回退。
- Herdr 无法连接、Workspace 查询失败等情况不等于 Workspace 已删除，不能据此静默回退。
- 默认 Workspace 必须能按现有配置规则唯一解析。默认目标不存在、存在重名歧义或查询失败时明确报错，不任意选择、不自动创建替代 Workspace。
- 已有匹配目标在其他 Workspace 时直接聚焦其真实位置，不搬迁该目标，也不在偏好的 Workspace 再启动一份。
- 历史仍是一份独立于当前会话登记的持久目录；Resume 不把登记存活性当成对话是否存在的证明，不删除原生聊天记录，也不因恢复为同一原生对话增加重复历史条目。

### 焦点、并发与结果

- 焦点是 Herdr 内部的 Workspace、Tab、Pane 导航；接受公开焦点操作影响同一 Herdr 会话的其他已连接客户端，不要求 Windows 桌面窗口置前。
- 同一 Chat 在同一 herdr-bot 实例中同时最多执行一次 Resume；该约束不能仅依赖单个浏览器按钮禁用。重复请求不排出后续重复启动任务。
- 启动超时不视为未启动。页面显示“启动结果未确认”，保留已创建目标供检查，不自动另开 Tab 重试，不因超时销毁可能已运行的目标。
- 后续用户发起的 Resume 仍从查找已有目标开始，不盲目重放上一次启动。
- 操作成功必须对应已确认的定位或恢复结果；命令已提交、启动等待超时或单纯创建 Tab 成功均不能冒充完整恢复成功。
- 沿用后端权威、前端呈现确认结果的原则；Resume 不更改会话事件上报的来源边界，不增加对外任意命令执行入口。

## Testing Decisions

- 本轮是规格制定，没有执行新功能、构建或测试，也没有完成实际 Herdr 恢复实验。已有 CLI 帮助和源码调查不是端到端恢复成功的证据。
- 既有历史持久化、默认 Workspace 精确解析、Chat 发起及 Herdr CLI 适配行为可作为后续验证的先例；不在此指定测试边界、替身形态或测试文件。
- 实现后的验证必须包含真实运行的恢复路径：先定位已有对话；再在没有匹配目标时加载指定旧对话，并观察 Herdr 目标及加载的原生身份。不能以仅编译通过、命令形状正确或返回非空代替行为证明。
- 验证应证明没有发送 Bot 初始指示、没有把原生对话加载失败替换为空白对话，并区分启动完成与超时未确认。
- 验证需覆盖旧历史缺少 Workspace、原 Workspace 删除、ID／路径身份表达差异、多目标提示并自动选取、同一 Chat 并发请求，以及查询失败不得启动副本等用户可见边界；具体 seams、承诺和覆盖分配交由 /to-contract。
- 真实 Herdr 场景只能使用可控的专用测试对话与目标，不通过关闭、重启或改动用户现有 Chat 来证明恢复行为。

## Out of Scope

- Bot 预设编辑、重新发起 Chat、自动重放初始指示、自动继续中断回合或子智能体任务。
- 在 Herdr 之外实现另一套终端、进程或原生会话管理系统。
- Windows 窗口置前，以及只控制某一个 Herdr 客户端的独立导航。
- 多目标选择弹窗、用户手动选择恢复目标的额外交互。
- 恢复旧进程、PTY、构建任务或其他普通 shell 运行状态。
- 自动创建缺失的 Workspace、自动修改工作目录、猜测 agent 类型、以空白新对话替代无法恢复的原生对话。
- 跨机器迁移原生会话文件、多 herdr-bot 实例的分布式互斥，以及阻止用户在本服务之外手动启动同一原生对话。
- 重构会话登记生命周期、关机监听、connect-bot 失败恢复策略或既有会话事件写入信任边界。
- 本阶段决定模块边界、API 路径、内部接口或测试 seams；这些属于 /to-contract。

## Further Notes

- 用户明确修正了“必须已有 Herdr 绑定才能恢复”的初始限制：有可用原生身份及所需恢复上下文即可尝试恢复，缺 Workspace 时进入默认 Workspace。
- 用户明确选择多目标时提示后自动切到其中一个，不增加目标选择交互。
- 术语遵循项目 GLOSSARY：Bot 为可复用预设，Chat 为实际对话，agent session 为原生身份，历史会话列表不是当前会话登记，也不承载对话正文。
- 遵循 Bot／Chat 术语决策和既有后端权威原则；此功能为历史列表新增明确操作，不放宽会话事件只接受同机来源的边界。
- 当前 Herdr 没有按原生对话身份直接完成查找并聚焦的统一操作，也不在 agent 启动时保证同一原生对话全局唯一；实现不能假定外部系统已提供这些保证。
- 本 spec 按已完成的设计质询及用户生成 spec 的指示发布。获批承诺、seams 和验收责任见关联合同；下一步为 /to-tickets，依据合同切分交付工作。
