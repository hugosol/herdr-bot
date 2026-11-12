# Chief Bot：统一需求入口、确认式委派与结果报告

Status: ready-for-agent
Label: ready-for-agent
Type: spec
Repositories: herdr-bot, my-agent-toolkit
Contract: [已批准 contract — Full automation](contract.md)

## Problem Statement

用户目前需要自行寻找可复用 Bot、区分 Task 与 Map 的执行方式、准备发起参数并进入不同 Chat。缺少一个能够理解需求、推荐现有能力、在用户授权后分派工作，并向用户呈现结果的统一管家入口。

现有 manager 负责具体 Run 的管理，不能用它代替跨需求分流的管家。用户也不希望入口 agent 自己执行业务、未经批准创建重复能力，或在结果返回后自主开展下一项工作。

现有节点报告面向 Run 的 manager。chief 委派的独立 Chat 与 Map manager 需要向当前 chief 报告，同时复用既有报告保存与通知能力，而不是复制一套相互不一致的协议。

## Solution

在主界面右上角 server 按钮左侧增加 `chief bot`。同一后端服务只认可一个当前 chief Chat：存在时打开它，不存在时打开现有发起 Chat 窗口，默认使用可复用的 `chief-bot` Task、`default/bot-playground` zone，以及 `chat with chief bot` 名称。

新增 `chief` Task 类型和对应固定扩展，支持 omp、pi。chief 是统一需求入口：澄清需求，优先按 Bot 的 `when` 搜索可复用能力，准备 Bot、业务要求、zone、必要的 manager 与材料参数。用户可在对话中修改参数；工具以类似现有 report 的原生确认展示实际派发内容，明确批准后才启动目标。

Task 型 Bot 发起独立 Chat；Map 型 Bot 发起 Run 与其 manager Chat。chief 不执行业务，不等待工作结束，不接管 manager 的流程职责。没有合适能力时，通过已有创建 Task／创建 Map 能力解决能力缺口；用户否决复用判断后，由用户明确指定创建 workflow、创建 Task 或放弃，chief 不自行决定替代路线。

被委派的独立 Chat 与 Map manager 使用专门的 chief 报告工具，经用户确认发送结果。报告先由 herdr-bot 保存，再尝试通知当前 chief；即使当前 chief 不是派发者，也可依据完整报告向用户呈现结果。chief 仅询问结果是否满足要求，随后等待用户。

报告发送失败保留已保存报告，但没有任何自动重投或启动后补投。只有用户在被委派的 Bot 会话中明确要求重新发送，经工具确认后才重发；同一报告沿用稳定 ID，通过接收确认与处理状态避免重复处理。

## User Stories

### 统一入口与可复用预设

1. As a herdr-bot user, I want a chief bot button immediately to the left of server, so that I have a consistent entry point for my requests.
2. As a user, I want the button to open the current chief Chat when one exists, so that I do not accidentally create competing chiefs.
3. As a user, I want the button to open the existing Chat creation dialog when no current chief exists, so that I can review startup settings before launching.
4. As a user, I want a reusable chief-bot Task rather than a new Task for every conversation, so that the chief configuration remains reusable.
5. As a user, I want the launch dialog to default to the chief-bot Task, the default/bot-playground zone, and the name chat with chief bot, so that routine startup needs little configuration.
6. As a user, I want the chief launch dialog to offer only chief-type Tasks, so that this entry cannot inadvertently start a normal worker or manager.
7. As a user, I want to adjust the chief Task, zone, model, thinking level, and Chat name before startup, so that defaults do not prevent legitimate customization.
8. As a user, I want a missing default zone to require selection of a registered zone rather than silently creating a directory or binding, so that startup location stays explicit.
9. As a user, I want chief to work with both omp and pi, so that the role is not tied to one host.
10. As a user, I want chief capabilities to load as a fixed role extension independently of Suite selection, so that the role is available even without a Suite.

### 需求判断与确认式分派

11. As a user, I want chief to clarify my request without performing the business work itself, so that execution remains with the selected Bot.
12. As a user, I want chief to search existing Bots by their when guidance before proposing new capabilities, so that reusable work is preferred.
13. As a user, I want chief to explain its selected capability and proposed delegation, so that I can correct a mistaken match.
14. As a user, I want chief to fill in delegation parameters from my request and available definitions, so that I do not manually assemble every launch request.
15. As a user, I want to change proposed parameters through conversation before confirmation, so that the final delegation reflects my intent.
16. As a user, I want a native confirmation showing the actual Bot, business input, zone, manager where applicable, and relevant materials, so that I approve the action that will really be sent.
17. As a user, I want rejection, cancellation, timeout, or unavailable confirmation UI to prevent dispatch, so that lack of approval cannot be interpreted as consent.
18. As a user, I want changed dispatch parameters to require a new confirmation, so that an earlier approval cannot authorize different work.
19. As a user, I want a Task-backed Bot to start an independent Chat, so that single-executor capabilities remain simple to use.
20. As a user, I want a Map-backed Bot to start a Run with its own manager Chat, so that workflow execution retains its established management semantics.
21. As a user, I want every delegation to specify its zone explicitly, so that the chief's own location does not silently determine the worker's location.
22. As a user, I want a delegated Map to retain the zones of its internal nodes, so that selecting a manager zone does not rewrite the workflow.
23. As a user, I want chief to become available for other requests after dispatch instead of waiting for completion, so that long-running work does not block the entry point.

### 能力缺口与用户决策

24. As a user, I want chief to propose the existing Task-creation capability for a single-executor capability gap and the existing Map-planning capability for multi-step work, so that new capabilities use established creation workflows.
25. As a user, I want creating a capability to require an explicitly confirmed delegation, so that failure to find a reusable Bot does not authorize creation automatically.
26. As a user, I want my rejection of a reusable Bot to stop automatic route selection until I specify what to do next, so that my correction cannot be treated as permission to create something new.
27. As a user, I want to explicitly choose a new workflow, a new Task, or abandonment after rejecting reuse, so that the next step follows my judgment.
28. As a user, I want capability creation to remain separate from executing the original business request, so that approval to create does not become approval to run.
29. As a user, I want existing workflow-stage approvals to remain in place, so that chief's launch approval does not replace later business or report approvals.

### 结果报告与人工重发

30. As a delegated worker, I want a dedicated chief-report tool when my Chat was launched by chief, so that I can return results without pretending to be a workflow node.
31. As a delegated Map manager, I want to report my workflow's result outward to chief while retaining my internal manager duties, so that chief receives the appropriate summary rather than every internal report.
32. As a workflow participant, I want internal node reports to continue going to their own manager, so that chief integration does not disrupt nested workflow reporting.
33. As a user, I want chief reports to describe success, failure, or blockage and include the request context, result, deliverables, and decisions needed from me, so that the receiving chief does not need the original conversation.
34. As a user, I want report transmission to use explicit native confirmation, so that preparing a result is not treated as permission to send it.
35. As a user, I want the backend to save a chief report before attempting notification even when no chief is available, so that delivery failure does not erase the result.
36. As a delegated worker, I want separate feedback for saving and notification outcomes, so that I know whether the report exists and whether delivery was successful or uncertain.
37. As a user, I want delivery failure to leave the report available without automatic retries, so that the system does not send messages later without my instruction.
38. As a user, I want to request a resend in the delegated Bot's own Chat and confirm it there, so that I control recovery from failed delivery.
39. As a user, I want resending the same report to retain its identity, so that it cannot appear as a newly completed piece of work merely because it was sent again.
40. As a user, I want stable report identities and receipt tracking to prevent duplicate processing and repeated questions, so that an uncertain earlier delivery does not create confusing duplicate results.
41. As a user, I want a report to reach the current chief rather than necessarily its original dispatcher, so that I can replace a chief without migrating its conversation context.
42. As a user, I want chief to present the result and ask whether it meets my requirements, then wait, so that it does not autonomously launch another task or prompt for the next execution.
43. As a user, I want report receipt to remain distinct from my reading it or accepting the business outcome, so that delivery is not mistaken for approval.

### 当前 chief 的唯一性与生命周期

44. As a user, I want one effective chief per backend service across all browsers, zones, and chief Tasks, so that report routing and dispatch authority are unambiguous.
45. As a user, I want concurrent create or resume attempts to be constrained before registration completes, so that a startup race cannot create two effective chiefs.
46. As a user, I want an offline chief to retain its slot while its session registration is retained, so that a brief interruption does not permit a competing chief.
47. As a user, I want a new or resumed chief to acquire the slot after the old registration is removed, so that an abandoned registration does not block me forever.
48. As a user, I want an old process returning after replacement to be denied chief authority and report reception, so that it cannot reclaim the role from the current chief.
49. As a user, I want backend restart and concurrent re-registration to preserve the one-effective-chief rule, so that recovery does not create competing authorities.
50. As a user, I want creating or resuming chief to leave saved failed reports untouched until I explicitly request resend from their sender, so that recovery never becomes automatic replay.
51. As a maintainer, I want existing node reporting and nested Map behavior to remain unchanged, so that shared implementation does not silently alter existing workflows.
52. As a maintainer, I want both repositories to reuse their existing report and host abstractions at the agreed layers, so that chief does not introduce duplicate storage authority or host-specific business copies.

## Implementation Decisions

### 已确认的领域与界面约定

- Task 是可复用预设，Chat 是实际对话；Bot 是跨 Map 共享的业务能力定义。chief-bot 是 Task，不因名称带 bot 就自动成为 Bot 定义。
- 新增 chief Task 类型，与 normal、manager 并列；chief 不等于 Run manager，也不是节点执行角色。扩展现有 Task 类型约定，不撤销节点角色由实际执行上下文确定的规则。
- chief-bot 为默认可复用预设；入口默认 zone 使用稳定 ID `default/bot-playground`，不是目录名，也不是同名 Herdr Workspace。默认 Chat 名称为字面值 `chat with chief bot`。
- 当前 chief 已存在时入口打开该 Chat；没有当前 chief 才显示发起窗口。窗口仅选择 chief 类型 Task，保留 zone、模型、思考级别及名称的编辑能力。
- 缺失默认 zone 时使用既有有效 zone 选择与校验，不创建隐式 zone 或目录。Herdr Workspace 不与 zone 合并，也不因这项功能另立命名规则。
- omp、pi 同时支持；chief 的固定扩展按角色与宿主装配，不通过 Task 名称猜测身份，不依赖 Suite 勾选。

### 分派与用户授权

- chief 通过 Bot 的 when 寻找候选，必要时读取能力及执行目标定义，不能将搜索命中等同于执行目标有效。
- 派发以 Bot 身份为入口：Task 目标启动普通独立 Chat；Map 目标启动真实 Run 和 manager Chat，不能只启动没有 Run 的 manager。
- chief 自动准备本次 Bot、业务输入、显式 zone、Map 所需 manager Task 及必要材料参数。参数来自需求与已知定义，用户可通过对话修正；未知信息先澄清，不根据唯一候选或 chief 所在位置隐式决定。
- zone 与 manager 的选择参考嵌套 Map 的显式调用语义。普通 Chat 同样使用本次委派指定的 zone；Map 内部节点保持自身 zone。复用嵌套执行的已有能力，不为 chief 伪造父 Run、node 或 execution。
- 原生工具确认绑定实际待发送内容；确认后参数不能被替换。只有明确批准才发送；拒绝、取消、超时或确认能力不可用均不派发。业务对话中的同意不能替代工具确认。
- 原生确认是本工具的执行门禁，不声称它构成阻止任意 HTTP 调用或任意本机工具的安全沙箱。
- chief 分派后不阻塞等待，不负责执行、接管 manager、自动恢复或自动改派失败业务。
- 用户否决复用判断后，必须等待用户明确指定后续处理；不能把否决视为自动创建能力的授权。
- 没有合适能力时复用已有创建 Task／创建 Map 的 Bot 入口。对话中的 create_task、plan_workflow 是能力指代，不是要求新增同名工具，更不能用简单保存 Task 的操作替代完整创建能力。
- 能力创建和使用能力执行原始需求是独立授权。结果返回后只询问是否达到要求并等待用户，不自动检索后续方案、推进下一项工作或启动新执行。

### chief 报告与共用实现

- chief 委派的普通 Chat 与 Map manager 获得专门的 chief 报告能力；它与节点报告是不同工具、不同归属规则。普通 Chat 不因使用相同 Task 就获得向 chief 报告的资格。
- 只有真实 chief 委派关系关联的执行者可提交 chief 报告；目标是当前有效 chief，不由调用者任意指定。Map 内部报告继续走其既有 manager 链路。
- chief 报告允许成功、失败、受阻说明及交付物引用，并提供足够的需求和执行背景。接收者不要求是派发者，也不依赖原 chief 的上下文。
- 发送报告同样要求原生确认。报告保存、通知提交、接收确认、用户阅读和业务验收是不同事实，不相互冒充。
- herdr-bot 是报告保存的权威：先保存，随后尝试通知当前 chief。没有当前 chief 或通知失败仍保留已经保存的报告，并分别返回保存与通知结果；请求结果不确定时不能宣称没有发生效果。
- 不设计自动重投、定时重投、chief 注册后补投或 resume 后补投。人工重发只能由用户在被委派的 Bot 会话中明确要求，并再次经过工具确认。
- 重发同一报告沿用原报告 ID，不重复创建另一份业务报告。保留接收确认与报告处理状态，用于避免重复处理；这些状态不等于迁移 chief 对话上下文。确认时机与崩溃窗口的精确契约交由后续 contract 阶段明确，不宣称网络天然提供恰好一次展示。
- 已明确的复用落点：herdr-bot 抽取报告保存与通知的共同实现；my-agent-toolkit 在 workflow-chat 的 common 层复用确认、Session 绑定请求及共同结果处理。各自保留身份校验、收件对象和业务策略，不把后端存储搬到扩展。
- toolkit 遵守其文档导航规定的分层：工具与提示词等业务归 extension，消费 framework 的公共 interface；宿主机制归 adapter／薄装配，发现入口不承载业务。共享业务不因有多个调用者而上移到 framework，不直接依赖 framework 内部实现。
- 原 node report 的先保存再通知、接收对象和现有失败语义保持不变；不借本次抽取给所有 report 新增自动重试。

### 唯一有效 chief

- 唯一性范围是同一后端服务，不按用户账号、浏览器、zone 或 chief Task 划分；保证唯一有效职责，不保证操作系统只存活一个原生进程。
- 后端负责唯一性，connect-bot 会话登记提供存活依据。创建、恢复及注册之间必须有一致的占位约束，不能仅在注册完成后检查唯一性。
- 进入离线状态但登记尚未清除时不允许创建另一 chief；保留期内恢复上报仍属原 chief。登记清除或明确注销后允许创建／恢复以取得当前身份。
- 新 chief 已取得身份后，旧进程迟到上报不能抢占，旧 chief 不再有派发与接收 chief 报告的职责；不以强制终止失联进程作为实现前提。
- 后端重启后的重新登记也必须满足唯一有效 chief 规则；不能因为登记暂时为空而同时授予多个 chief 权限。

### 尚未在 spec 阶段决定的内容

除上述已在对话确认的复用层次外，本 spec 不选择新的 seam、模块 interface、HTTP 路由、数据库或文件布局、锁与占位实现、扩展注册名称、接收确认协议或实现票拆分。后续通过 /to-contract 在既有实现上选择承载承诺的最少必要 seam，并明确失败与并发契约。

## Testing Decisions

- 本次只发布需求规格，不把文档存在或代码阅读结果当作功能通过；尚未执行 chief 功能测试。
- 用户在 contract 约束审查阶段明确：所有测试中的实际模型调用必须使用 GPT-6-luna，包括烟测、原生角色验证及端到端验收。记录实际调用的模型身份；模型不可用时报告阻塞，不得改用其他模型或用受控响应冒充真实模型证据。此限制不改变产品允许用户选择模型的能力。
- 已有先例包括 Chat 发起／恢复、会话离线清除及迟到登记、workflow 派发、嵌套 manager 内外角色、节点报告先保存后通知，以及原生报告确认。后续复用这些测试设施和领域约定，不预先规定新 seam 或逐项覆盖归属。
- 原生 report 确认已有“确认前无发送、确认后载荷与预览一致、取消不发送”的行为先例；chief 派发与报告需要遵循同类可观察语义，而不是只断言提示词或工具 schema 文本存在。
- 测试区分后端持久事实、Herdr 接受输入、原生扩展实际加载、模型遵循职责、chief 接收确认与用户验收。受控 provider、配置存在和最终完成状态不能相互代替这些证据。
- 支持 omp、pi 的承诺需要各自的证据；某一宿主或静态 integration check 通过不外推为另一宿主的原生行为通过。
- 遵循各仓现有测试约定。herdr-bot 后端使用既有隔离 HTTP／进程设施，前端通过既有 npm 入口保留护栏；共享后端实现修改遵循完整后端回归要求，开发反馈可定向运行。
- 可变数据、端口、原生会话、Herdr 实例与进程均归测试独占；不读用户凭据，不操作真实用户 Chat，不运行真实关机，不以删除临时目录代替进程清理。
- 离线保留、确认超时、迟到上报、无自动重投及无恢复补投是涉及时间与生命周期的行为，不以即时断言或文本快照冒充证明；具体观察窗口与测试层次在 contract 阶段决定。
- 测试以消费者可观察的行为、权限、失败、并发与状态转换为目标，不增加复制实现、字面提示词快照或仅验证接线的永久测试。

## Out of Scope

- chief 自己执行业务、成为另一层 Run manager、同步等待全部委派结果。
- 自动推进、自动改派、自动重试业务，或收到创建能力结果后自动执行原始需求。
- 报告自动重投、轮询重试、chief 启动／恢复后的自动补投，以及无需用户指示的发送。
- 迁移或共享 chief 的原生对话上下文、按 chief Task 或用户账号设计多收件箱。
- 多个同时有效的 chief、多用户权限系统、跨后端服务的全局 chief。
- 强制杀死旧 chief 进程来实现唯一性。
- 截取全部 node report；改变与 chief 无关的 workflow 报告去向。
- 把工具确认包装成任意 HTTP／本机能力的后端安全沙箱。
- 用虚构父 Map／Run／node 复用嵌套机制，或为本功能重新实现现有 scheduler。
- 自动创建缺失的默认 zone、目录、Herdr Workspace 或迁移用户部署数据。
- 在本 spec 中确定尚未讨论的接口、存储布局、seam 与测试覆盖矩阵。

## Further Notes

- 最新用户决定优先：曾讨论过 chief 可用后自动重投，但已明确撤回。最终方案是先保存、尝试通知、失败保留、用户在发送方 Bot 会话中明确要求后确认重发。任何旧讨论中的自动重投表述均不构成本 spec 的要求。
- ADR 0012 的 Task／Chat 区分、ADR 0013 的 Bot 能力定义保持成立；新增 chief 扩展 ADR 0015 的 Task 类型集合，但不改变执行上下文决定节点角色的原则。
- zone 按 ADR 0017 使用稳定身份和 server 本机绑定，不能把 bot-playground 的目录名或 Workspace 名替代 zone ID。
- 会话登记移除不等于原生 Chat 或进程结束；沿用 ADR 0006 的离线保留语义，额外约束当前 chief 职责。最终接收去重只处理报告身份与状态，不构成用户已阅读证明。
- 本次研究已确认 herdr-bot 和 my-agent-toolkit 的相关既有实现；功能跨两仓交付，不能仅提交按钮、提示词或后端骨架就视为完成。
- 下一步：对本 spec 执行 `/to-contract`，选择承载承诺的 seam 并批准契约；本状态不表示契约或实现已经通过验收。
