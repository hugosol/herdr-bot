# omp extension 接入与可信执行身份：Lighthouse

> Question and discussion: [omp extension 接入与可信执行身份](../decision/08-omp-tool-integration.md)

## Question

首版如何通过 my-agent-toolkit 的 omp 扩展复用 Task → Chat 启动链，以后端运行关联校验身份，并实现管理者操作、节点交接报告和 collect？本票发布设计，不实现产品或执行真实 Agent 验收。

## Discussion

### 确认来源与修订范围

2026-10-28 本票会话 Q8–Q37 及随后启动澄清；用户最终“请发布”。具体来源：Q8 确认 Task 类型；Q13 确认先保存关联再提交指示；Q14 接受同机信任；Q16 收窄协作、Q19 保留报告通知；用户明确改用 Herdr 输入并接受分层；Q23/Q31 确认工具与 action；Q24 人工处理提前停轮；Q26/Q27 确认等待取消与报告部分失败；Q30–Q36 明确提示词、文档及路径；Q37 沿用现有错误机制。最后用户明确 omp 未开启 autoResume，不再讨论该边界，禁止传 --session-dir。

原部分结果中的 herdr-bot 统筹 HTTP 与先调查 connect-bot 保留；扩展组织、身份与投递现已收口。报告与 collect 语义在原[通信权威](04-collaboration-protocol.md)修订；Task 类型在[字段权威](02-map-definition-contract.md)；Workspace 在[绑定权威](01-execution-binding.md)。不维护第二套相反政策。

### 孪生 extension 与 framework 分层

在 `E:/Developer/myhub/my-agent-toolkit` 实现；开发先读 `DOCS-MAP.md`，遵循 extension 开发地图及 model-cost 分层。manager-chat、node-chat 是同一业务目录中的两份独立扩展声明与自动装配，在 `entrypoints/omp-auto` 提供两个薄出口，不提供手动模式。herdr-bot 按 Task 环境加载，不在启动时偷偷追加角色扩展；Task 类型及不可取消角色扩展消费字段契约。

2026-10-28 Traverse 审计发布补齐装配表面：两个角色 ID 都须接入 `entrypoints/startup/auto.yaml` 的自动启用策略，并由各自动装配显式选择该策略；仅创建 omp-auto 出口不构成启用保证。业务配置须明确后端 endpoint、请求预算的文件归属，以显式 `config.path` 和稳定 `configBaseDir` 指向实际配置，不让同目录双 ID 落到默认的两个 ID 目录，也不依赖 Chat 的 cwd。共用或分角色文件的具体组织及配置键由 spec 按 framework 现有机制整理，不新增配置框架。用户在审计收口汇总后“同意”统一发布这些交接补充。

内部以 shared-logic 下 manager、node、common 按实际职责组织，可进一步按 business/foundation 分层，不为目录对称创建空层或转发 module。manager/node 消费 common；common 不反向依赖两侧，foundation 不 import business。业务协议留在 extension，公共语义与通用 HTTP 等消费 framework/interface，宿主专属接入在 harnesses 或公共 adapter 的合适层；入口不承载业务。

共享的是实现，不是不同 Chat 的内存、身份或连接。首版 node 只提交报告，manager 也不装配 extension 接收通道：报告接收由 Herdr 路径完成。不要预建 SSE、后台长轮询或双向消息模块。未来复用通用机制时仍需补业务规则与验证，不能宣称仅打开开关即可。

connect-bot 已有原生 session 信息、自动启动及生命周期接入经验；HTTP 使用 framework 公共能力，直接复用该能力，不复制客户端。connect-bot 本身只有会话上报／心跳，没有现成的后端到 Chat 收信通道。新运行能力不依赖它的启用开关，不把 Run 协议塞入 framework 或 connect-bot 私有实现。

### 身份与后端归属

extension 通过 toolkit adapter 的会话能力取得当前原生 session_id，自动附在请求中，不是 Agent 填写的工具参数。Task 只持有可复用类型与预设，Chat 数据不新增 Run/node 归属字段；当前归属以 run.json 中实际 Chat 关联为准，旧执行关联保留在运行历史。后端可维护派生索引，不另建权威。

herdr-bot 在每次请求中解析 Run、角色、node 与当前执行，校验管理者权限、状态及报告适用性。新执行不能消费旧执行报告作为完成依据。没有关联或不能确定归属就拒绝，不按名称、目录、Tab 或 Task 猜测。不新增供 extension 或 Agent 先查询自身归属／联系人的接口；保留业务所需 run status 和 collect 快照。

首版只接受同机运行请求，采用同机信任与后端关联校验，不签发独立凭据。session_id 不是秘密；明确接受同机进程知道 ID 后可能冒用的风险。这防止普通模型调用误填归属，不是身份认证或同进程沙箱；不修改既有会话上报 API 的 ADR 0004。

后端重启不接管旧 Run，拒绝旧 Run 的状态修改、报告与 collect，消费[持久化权威](07-persistence-consistency.md)。用户排除 /new、/resume 等活动期间会话切换边界，不建设切换检测、自动重绑定或恢复机制，也不承诺支持这些场景。

### 启动与关联保存

复用 zone 创建 Chat 的 Task 解析、扩展加载、Tab 创建和 agent 启动路径。用户明确目标 omp 未开启 autoResume，作为首版环境前提；不为此增加检测或另建启动机制，不传恢复参数，不传 --session-dir，不搬移原生会话记录。独立新会话要求消费绑定契约。

正常顺序：校验本次 Map/Task/zone 等执行配置 → 生成 Run ID、目录并初始化三份运行记录 → 按 Task 启动 manager Chat → 取得真实原生身份 → 保存 Run–Chat 关联 → 成功后提交 Task.prompt 与本次 input。manager 关联尚未建立时明确为空。节点派发／重试复用启动及身份采集方式，在其 Run 关联保存成功后才提交初始指示。

身份采集参考现有 collect_native_history/native_session，复用采集结果分别保存 Chat history 与 Run 关联；不让 Chat history 成为运行归属权威。普通 zone 创建的后台尽力保存策略不直接套给 Run：关联失败不提交指示，不宣称运行接入成功。已经启动的外部 Chat 不被虚构为已回滚。文件锁不跨外部启动等待；状态操作串行语义及部分提交消费原权威，不增加跨文件事务。

manager Workspace 从所选 zone 推导，记录实际 Workspace 标识，节点使用各自 zone、同一 manager Workspace，详细政策由绑定权威持有。

### 工具 Interface

三个工具：manager 注册 run 与 collect；node 注册 report。名称按本轮接口约定；不为每个状态动作注册独立工具，不注册普通消息、广播、联系人查询或接收工具。调用者 session_id 由 extension 自动附带，工具参数不接受自报 Run/角色/执行归属。

| run action | 业务参数 |
| --- | --- |
| status | 无 |
| dispatch | node_ids |
| complete | node_id、reason |
| fail | node_id、reason |
| reconcile_dispatch | node_id、核实结果、reason；确认已创建时附实际 Chat 引用 |
| recover | node_id、reason；原 Chat 可继续 |
| retry | node_id、reason；新 Chat 执行 |
| pause | 无 |
| resume | 无 |
| finish | 无 |

每个 action 只接受必要参数，准入与效果唯一消费[运行状态机](03-runtime-state-operations.md)。返回操作结果与最新快照；dispatch 有逐 node 结果。reconcile_dispatch 是已确认人工核实，不是任意 setStatus／关联后门。实际 Chat 引用复用既有 Chat 接口。HTTP 路由拼写、序列化类型等由最终 spec 按现有后端惯例收敛，不增加新的产品决策。

collect 无必填业务参数，等待预算来自配置，返回结束原因、运行快照、按 node 汇总的交接待办及报告标识，不重复正文。report 接受交接报告正文，固定报告当前执行、通知 manager；返回报告标识、保存与 Herdr 投递情况。报告不直接完成节点。

错误沿用现有机制（Q37），不新建统一错误平台；此前已确认的部分生效事实不可隐藏。报告保存成功但 Herdr 投递失败，不撤销报告、不自动暂停或重复提交，工具指引明确不能仅为投递失败重发。

### collect 的 HTTP 等待与取消

后端按通信契约即时返回或有界等待；默认 90 分钟可配置，HTTP 请求预算略长于后端等待期限，不复用 connect-bot 的 1 秒预算。公共 HTTP 补调用方取消支持，业务规则不下沉框架。工具中断或 Chat 结束取消请求，后端清理等待者，不修改 Run 状态；不依赖宿主专属 IRC 打断机制。

正常等待到期返回原因与现场；用户取消或网络错误不保证取得快照，不伪造正常结果，不自动重试或将 node 标为 failed。等待不持有文件锁／状态操作串行入口。manager 提前停轮由人工处理（Q24），不增加 session_stop 强制续轮或后台选路。

### Herdr 报告输入

后端保存报告后形成交接待办与通知，并独立通过现有 HerdrClient::prompt/agent prompt 向关联 manager 提交正文。collect 不等待或依赖该投递成功；无需确保正文先于 collect 结果进入上下文。报告标识允许 manager 从消息文件读取正文。

接受终端输入的首版限制：正文标注 node 来源与报告标识，但不宣称原生 Agent 来源认证；不保证安全边界注入、自动打断 collect 或严格输入顺序。Herdr 成功只表示输入提交，不是模型已阅读／处理。失败如实返回，不自动重投，不建立投递记录文件；诊断日志与保存策略消费持久化契约。工作中、空闲及等待时的实际表现属于实施验收，不伪装成 IRC 等价保证。

### 提示词、文档与启动 input

Task 完整 trigger config 沿用现有首次创建语义：model/thinking 用于启动参数，初始 Skill、Task.prompt 与本次 input 复用 compose_initial_input 的组合规则，不因重排关联保存步骤丢失 Skill。manager、node 及 retry 的新 Chat 均在关联保存成功后提交初始指示；Resume 不重放 trigger config，消费既有 Task/Chat 契约。此项是 Traverse 对现有复用要求的交接澄清，已获统一发布授权，不增加新的 Skill 配置机制。

三层分工：Task.prompt 记录角色不变的职能与业务要求；extension 使用 framework 公共 PromptDeclaration，以 first-enabled-run 将工具使用指引注入消息；本次 input 提供工作信息和文件路径。工具 description 说明参数与返回，不能代替消息内的动作适用条件、推进步骤和故障指引。不直接调用 omp 注入工具指引，不把手册复制进每个 Task。

参考 hyper-auto-coding 的工具说明与启动步骤，不迁入 Git/worktree/自动集成或 session_stop 机制。工具规则覆盖全部 run action 的用途与前置、collect 与 report 语义，以及不得直接修改运行文件、旧报告不得冒充新执行等约束。

herdr-bot 以独立文档维护文件本身无法表达的运行背景与阅读说明；对于该说明文档，input 只给路径，Agent 主动读取，不由后端加载文档正文注入。manager input 提供 Map 路径、说明文档路径、Run ID 和三份运行记录路径，不复制完整 Map；随后追加非空 user_input 原文，使用独立的“本次用户要求”段落，空值不追加。不修改 Task.prompt。node input 提供自身标识/名称、node.input 与说明/运行记录路径，不重复整张 Map、不额外注入 Bot 能力文档。

修订来源：2026-10-28 [运行启动、只读图与人工观察路径](../decision/09-observation-experience.md)会话 Q10–Q12 确认可空多行 Markdown 输入；用户明确“只作为manager-bot的输入，不作任何后续逻辑”，并对 Q21/Q22 明确“同意”，确认本文件作为接入规则归属并授权发布。旧 manager input 仅列系统运行信息，现增加末尾用户要求，使同一 Map 能表达不同的运行意图；不增加插值、节点传参或 manager→node 消息，不改写 node.input，也不将后续传递机制作为必决缺口。保存原文消费[持久化契约](07-persistence-consistency.md#当前状态内容与派生数据)，表单及只读展示消费[观察契约](09-observation-experience.md)。

允许各 Chat 按绝对路径读取 zone 外的上述文档与记录，不复制文件、不强制相同工作目录；这是执行环境需满足的读取条件，不扩大 herdr-bot 文件浏览权限。路径与 Run ID 是工作上下文，不是 Chat 数据中的归属字段或权限凭据。未批准额外的启动文档可读性门禁。

### 证据与限制

静态核查：herdr-bot backend/src/chats.rs:215–373（扩展参数、启动、初始指示及后台身份保存），herdr.rs:346–388（native_session），lib.rs:1065–1074（zone 目录名推导 Workspace）；Herdr src/integration/assets/omp/herdr-agent-state.ts:374–427（启动上报身份），src/app/api/agents.rs:346–363（终端输入提交）。oh-my-pi main.ts:1331–1355 说明 autoResume 条件；用户提供未启用这一环境事实，不另行重测。

my-agent-toolkit DOCS-MAP.md、framework/docs/extension-development-map.md、capability-layering.md、interface/prompt.ts 与 general/http-contract.ts 提供分层／Prompt／HTTP 契约；当前 HTTP 尚无外部取消信号。connect-bot 的 client.ts 与 config.yaml 表明它是短预算上报，不是双向通道。oh-my-wayfinder extensions/hyper-auto-coding/agent-tools.ts 提供说明参考，其单工具与强制续轮不直接复制。

未修改产品、未运行真实 Agent 或测试；新角色类型、公共 HTTP 取消、同目录双声明装配、启动关联门槛、Herdr 报告路径均需实施与验证。现有能力证据不等于已交付本设计。接口命名与 schema 的统一整理由跨模块交接票消费这些已定语义，不重新引入被撤回的功能。

## Decision

采用同目录孪生自动 extension、Task 类型固定角色扩展、同机 session_id 关联校验和复用 Chat 启动链；首版只交付 run/collect/report 与 Herdr 单向报告通知，保持 framework 分层，不建设通用双向接收系统。

## User stories

1. As a 运行启动者, I want 复用 Task 与 zone 创建 Chat 并先保存关联, so that Agent 开始工作时后端已能确定其运行身份。
2. As a 扩展维护者, I want 孪生扩展共用实际需要的业务与 framework 能力, so that 角色工具独立而通信代码不重复、不依赖宿主私有实现。
3. As a 管理者, I want 运行工具、collect 和报告通知, so that 能根据已保存交接推进流程而不依赖正文投递成功。
4. As a 执行者, I want 只提交自身报告正文, so that 不需要模型猜测运行归属或接收者身份。
5. As a Agent 使用者, I want 分层工具指引及文档路径, so that 能按需阅读现场而不用维护多份提示词或复制文件。

## Preconditions

- Task 类型、绑定与状态机、通信及持久化契约提供对应权威；启动 UI 已由观察权威决定，最终 spec 尚待授权整理。
- 用户确认 omp 未启用 autoResume；首版同机可信环境接受 ID 冒用风险。
- Herdr/omp、toolkit 目标版本需实施验证，Prompt 和可取消 HTTP 必须通过公共接口接入。

## Postconditions

- 不使用 --session-dir，不改变原生记录位置；关联保存是初始指示的前置。
- 前后端／扩展消费同一 Run 归属；Task/Chat 不另存归属，Agent 不自报身份。
- 观察与交接票据据此提供启动、Chat 导航、类型配置与验收契约；不宣称实现已完成。

## Invariants

- Map 是定义，Run 是运行事实，Task 是预设，Chat 是实际原生对话；zone 与 Herdr Workspace 不混用。
- 参照 toolkit/model-cost 的业务与 foundation 分层，framework 不依赖运行扩展；入口与装配保持薄。
- 后端权威校验不依赖提示词，collect 不依赖 Herdr 投递；同机来源不是认证。
