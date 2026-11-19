# herdr-bot 同时适配 TUI herdr 与 herdr-app

Status: ready-for-agent
Labels: ready-for-agent

用户已确认方案。本文是正式规格，不代表实现已经完成。[Contract](contract.md) 已通过三个决策 gate，验收模式为 Rapid iteration；下一步为 `/to-tickets`。

## Problem Statement

herdr-bot 当前通过 TUI herdr 的控制接口托管 OMP/Pi Chat。用户希望也能使用 herdr-app，同时保留 TUI 使用方式，在同一个 herdr-bot 实例中仅通过 TOML 配置选择其中一端，而不是维护两套业务规则。

App 与 TUI 的外部协议、容器标识、进程状态和关闭确认不同。App 的窗口控制与 core 的终端控制分离，不能仅替换可执行文件就获得等价能力。尤其是当前的受管启动、原生会话身份取证和初始输入提交依赖 TUI；App/core 的终端状态不直接提供同等的会话身份与启动确认。

用户需要保留普通 Chat、历史恢复、chief、Run、节点派发、报告通知、定位和自动关闭等现有能力，也需要确认提示音状态链路不会因切换而失效。适配不能通过修改上游源码解决，不能将“终端创建成功”冒充“对话已可提交输入”，也不能把同名容器或终端 ID 当作原生会话身份。

## Solution

一个 herdr-bot 实例一次只使用 TUI 或 App。通过 `[herdr].backend` 选择 `tui` 或 `app`，省略时默认 TUI。App 模式组合使用 herdr-app 与配套 hc 的现成公开接口；上层继续使用同一套 Chat、历史、chief 和 Run 业务规则。

历史位置复用现有 `originalWorkspaceId` 字段，同时容纳 TUI Workspace ID 和 App Space ID。恢复时只向当前配置的宿主查询该 ID；缺失或成功查询确认不存在时，沿用按配置名称匹配默认容器的逻辑，不新增跨端判断、宿主标签或两端位置映射。默认容器名称始终来自 `[herdr].workspace_name`；`bot-playground` 只是当前配置值，绝不写死。cwd 恢复顺序不变；无法可靠找到历史目录或主工作树时，进入默认容器，由宿主选择实际目录并明确报告降级。

在 my-agent-toolkit framework 层提供统一的会话级 `ready`，由 OMP/Pi 各自适配并通过 connect-bot 上报给 herdr-bot。两种 Herdr 运行方式共用该初始化判断及原生身份关联。`ready` 表示启动输入管线已初始化，不保证此后永久可输入。初始消息仍通过所选宿主的终端输入接口提交，保留命令、Skill 和模板语义以及提交时的阻塞检查。

首次初始消息提交若被明确拒绝且能确认没有提交，等待 5 秒后最多重试一次；结果不明不重试，不通过强制输入绕过模态。提示音继续由各端原生机制播放。允许自动关闭目标 Tab 及因此为空的容器，不扩大关闭其他工作。

切换的操作前提是旧 agent 已退出、旧端已停止，同一时间仅运行一端；不迁移运行中的进程，不为 Run 增加跨端迁移或恢复机制。

## User Stories

1. As an herdr-bot 使用者, I want a TOML 选项选择 TUI 或 App, so that 我能按自己的交互偏好托管 Chat。
2. As an 现有 TUI 使用者, I want a 省略新选项时仍使用 TUI 的默认行为, so that 既有配置无需为本次适配立即修改。
3. As an 部署维护者, I want a 两端可执行路径分别配置或使用默认路径的方式, so that 日常切换只需要改 backend。
4. As an App 使用者, I want a 连接同一目标实例的 App/core 控制组合, so that 窗口中的 Tab 与实际运行终端一致。
5. As an Task 使用者, I want a 在两端均可发起 Chat 的能力, so that 不必复制 Task 或维护宿主专用业务预设。
6. As an Task 使用者, I want a 保留模型、thinking、Suite 和初始指示语义的发起过程, so that 切换宿主不改变 Task 的用途。
7. As an Chat 使用者, I want a 统一的初始化确认, so that 初始消息不会仅因终端刚创建就被过早发送。
8. As an OMP 使用者, I want a 晚于主编辑器启动闸门解除的 ready, so that session_start 尚未完成初始化时不会被误判可提交。
9. As a Pi 使用者, I want a 基于实际初始化时序而不是 OSC idle 的 ready, so that 初始输入不会依赖不可靠的状态猜测。
10. As an framework 使用者, I want a 独立于扩展自身 startup/initialization 的会话级 ready, so that 扩展初始化与 agent 输入初始化不会混淆。
11. As an herdr-bot 使用者, I want a 绑定本次启动和原生会话身份的 ready 上报, so that 旧进程或其他 Chat 的事件不会触发本次初始消息。
12. As an Chat 使用者, I want a 不因重复 ready 而重复提交的行为, so that 一次发起不会重复执行任务。
13. As an Task 使用者, I want a 保持现有 Skill、模板和终端命令语义的输入方式, so that 初始指示不会因为改用原生消息 API 而变成普通文本。
14. As an Chat 使用者, I want a 初始消息被明确阻塞时的提示, so that 我知道消息尚未提交而不是误以为 agent 已开始工作。
15. As an Chat 使用者, I want a 明确未提交后等待 5 秒再尝试一次的行为, so that 短暂启动阻塞有一次自动恢复机会。
16. As an Chat 使用者, I want a 第二次仍失败时停止自动尝试的行为, so that 系统不会无限重试或持续干扰终端。
17. As an Chat 使用者, I want a 超时、断线或部分写入时不自动重发的行为, so that 不会把已经接收的初始消息重复发送。
18. As an Chat 使用者, I want a 重试期间目标退出或会话变化时取消提交的行为, so that 初始消息不会发给错误的会话。
19. As an 历史会话使用者, I want a 在当前端加载另一端登记的 OMP/Pi 原生对话, so that 切换界面不丢失可访问的对话历史。
20. As an 历史会话使用者, I want a 有可靠证据时核对实际原生身份、缺少证据时明确报告未确认的恢复结果, so that 传入恢复参数不被误报为已经恢复同一 Chat，也不为普通恢复强制改变本地扩展配置。
21. As an 历史会话使用者, I want a 同端按原容器 ID 找回位置的行为, so that 已有 Workspace/Space 仍可继续承载恢复后的 Tab。
22. As an 历史会话使用者, I want a 当前端查不到原 ID 时进入配置默认容器的行为, so that 跨端恢复自然复用已有降级逻辑。
23. As an 部署维护者, I want a 默认容器名称完全来自 workspace_name 的行为, so that 修改配置即可改变所有默认选择和恢复兜底。
24. As an Chat 使用者, I want a 默认容器名称无匹配或有多个匹配时明确报错的行为, so that 系统不会任意选一个位置或偷偷创建容器。
25. As an 历史会话使用者, I want a 保持原始容器记录而不被每次恢复覆盖的行为, so that 历史位置语义保持稳定。
26. As an 历史会话使用者, I want a 有效历史 cwd 优先使用的行为, so that 更换宿主不会改变 agent 的工作目录。
27. As an 历史会话使用者, I want a 仅凭可靠 Git 归属证据回退主工作树的行为, so that 不会因为名称相似而在错误仓库恢复。
28. As an 历史会话使用者, I want a 无法找回目录时明确说明由默认容器选择目录的行为, so that 能识别本次恢复位置发生了降级。
29. As an Task 使用者, I want a 恢复时使用来源 Task 最新环境而不重放 trigger config 的行为, so that 恢复对话不会重复初始任务。
30. As an 历史 Chat 使用者, I want a 无来源 Task、来源已删除或恢复环境配置读取/解析失败时统一使用原生默认环境的降级行为，并在 Resume 按钮旁看到统一提示, so that 我能继续请求恢复并了解实际环境变化，而系统不猜测 Task、不清除已有来源关联。
31. As an chief 使用者, I want a 所选宿主下保留分派、身份关联与报告接收能力, so that App 适配不削弱需求分流流程。
32. As an Run 管理者, I want a 所选宿主下保留管理者和节点发起、输入、报告、定位及关闭能力, so that 切换选择不会只支持普通 Chat 而遗漏 workflow。
33. As an 节点 Chat 使用者, I want a 恢复时沿用已有执行角色而不新建执行的行为, so that 原执行关联不会因宿主适配而改变。
34. As an Run 使用者, I want a 完成后自动关闭准确关联 Tab 的行为, so that 不再需要手动清理已完成节点的终端。
35. As an App 使用者, I want a 自动关闭最后一个 Tab 时连带关闭空 Space 的行为, so that 无需额外手动确认空容器关闭。
36. As an 并行工作使用者, I want a 自动关闭不会扩大到其他工作的限制, so that 关闭一个节点不会终止无关终端。
37. As an 提示音使用者, I want a 正确接通所选宿主的 agent 状态上报, so that 等待输入或需要处理时仍能收到该端原生提醒。
38. As an 提示音使用者, I want a 保留各端声音开关、资源和触发规则的行为, so that 不会出现 bot 与宿主重复播放或互相覆盖设置。
39. As an 维护者, I want a 全面检查 Herdr 相关调用、固定扩展和目录归属的适配, so that 不会只改控制模块而遗漏模块外依赖。
40. As an 维护者, I want a 不修改上游源码的方案, so that 不必维护 herdr、App、core 或 agent 的私有分叉。
41. As an 部署维护者, I want a 明确说明单端运行与旧 agent 已退出的前提, so that 不会把关闭 App 窗口误当作 core 终端已经结束。
42. As an 验收者, I want a 区分真实实验、受控 provider 和生产端到端验收的证据, so that 不会把候选 ready 的局部实验当作整个适配完成。

## Implementation Decisions

### 配置、范围与兼容性

- `[herdr].backend` 的允许值为 `tui`、`app`，省略时为 `tui`。保持现有严格配置校验，不对未知选择静默回退。
- `[herdr].workspace_name` 继续是唯一默认容器名称来源。TUI 按 label、App 按 name 精确匹配，要求唯一；零匹配或多匹配明确失败，不自动创建默认容器。`bot-playground` 仅是当前配置值，不是保留名、代码常量或隐藏缺省值。
- 两端可执行路径分别配置或使用默认路径；日常切换只改变 backend。App 与 core 的实例选择必须一致，不得把某窗口的容器和另一个 core 的终端拼成成功结果。
- 实现范围为 herdr-bot 和 my-agent-toolkit。使用现成上游公开能力；不修改 herdr、herdr-app、herdr-core、OMP 或 Pi 源码。
- 适配范围包含所有现有 Herdr 功能，而非仅创建终端。覆盖 Chat 发起与恢复、chief、Run 与各类节点调用、报告文本通知、定位、关闭、固定状态扩展选择、配置、界面反馈及部署操作绑定。固定启动/停止脚本的调用成功仍不等于服务已经就绪。
- 业务选择规则共用，宿主协议与字段差异留在相应 adapter。已有 Herdr 控制与位置决策职责继续作为调查基础；本规格不新增或选择其他 seam，具体承诺与覆盖由 `/to-contract` 决定。

### 位置、历史与恢复

- 复用现有 `originalWorkspaceId`，存放 TUI Workspace ID 或 App Space ID，视为不透明容器标识。不新增 spaceId 字段、宿主类型标记、跨端分支、每端位置映射或 ID 字符串推断。
- 恢复向当前宿主按保存的 ID 查询。字段缺失或成功查询确认无匹配时回退默认容器；查询失败、畸形响应等不是“无匹配”。保留原始位置只记一次的语义，不在恢复时覆盖或补记另一端位置。
- 接受单字段方案不隔离跨宿主同值 ID 的后果。当前两端生成格式不同，但不建立额外防碰撞机制；若同值在当前端匹配成功，按当前端目标处理。
- cwd 与容器独立决定，保留现有恢复顺序：有效历史绝对目录优先；目录不可用时，仅在能证明准确工作树归属并找到有效唯一主工作树时回退；否则使用配置默认容器、不指定 cwd，由宿主选择实际目录，并反馈降级。
- App 缺少 TUI 的准确 worktree 归属元数据时，接受上述最终回退。不按名称猜 Git 归属，不另增固定回退目录，也不承诺两端最终选择出相同目录。历史 cwd 无法可靠恢复而需要宿主选目录时，沿用默认容器分支，而非强行保留原容器。
- 原生 session ID、session file、启动关联与 Workspace/Space/Tab/Pane/terminal ID 是不同身份。按名称、cwd 或终端存在不能证明加载了指定原生对话。有可靠原生身份上报时，恢复后仍核对实际身份及现有规则要求的工作目录。
- Contract 约束检查 B1 及用户后续补充已获批准：resume 不提交初始消息，不为等待初始输入 ready 额外强制加载 connect-bot。无来源 Task、来源 Task 已删除、本次恢复的 Task/Suite 环境配置读取失败或 Suite 配置错误，统一使用原生默认发现降级策略，不显式补充 connect-bot、其他 Task 固定扩展或 Suite。配置失败与无来源共用策略，但保留各自真实原因，不清除已保存的 sourceTaskId 或伪称来源已删除。
- 上述降级后若缺少 connect-bot，且当前宿主没有其他可靠原生身份取证来源，允许按指定历史请求恢复并定位目标终端，但只报告“已请求恢复并定位终端，原生身份未确认”，不得宣称已确认恢复同一会话。
- 来源 Task 与其环境配置正常时，继续装配该 Task 当前选择的 Suite、最新 env config 及按执行角色适用的固定扩展（包括 connect-bot），不因恢复无需初始输入就跳过资源。恢复的是当前定义的环境，不是历史 Suite 快照。
- 恢复降级 module 统一拥有降级策略及提示文本。每次实际选择降级，均在对应 Resume 按钮附近显示本次原因、已采用的策略和恢复结果；前端呈现统一结果，不自行按错误类型复制分支或另造策略。即使后续恢复失败或身份未确认，也保留此前降级提示。
- 环境配置降级仅适用于 resume。整体应用启动 TOML/宿主选择配置非法、历史目标无法读取或解析、agent 类型冲突、已知原生身份冲突及真实启动/定位失败，不因此变成原生默认环境的正常恢复。新建 Chat 仍保留原有环境校验和身份/ready 要求。
- 该降级意味着故障 Suite 的限制不再生效，实际资源来自本机原生默认配置，范围可能与原 Suite 不同；提示必须明确已经切换环境，不能仍宣称原 Suite 已恢复。
- 上述未确认结果不自动改绑历史/业务身份，也不因缺少身份确认自动重启或重复恢复。有可靠证据时仍正常核对；已知身份冲突、真实启动失败或定位失败不能被掩盖为正常未确认。没有接入不妨碍用户在终端人工检查并继续使用。此例外只收窄恢复确认保证，不放宽有初始输入的新建 Chat 的身份与 ready 要求。
- 保留已有历史数据与身份约束：冲突不能静默覆盖；尚未落盘的可信启动身份不等于可恢复历史，不伪造原生文件或提前投递业务输入以制造身份。新增由受控启动关联的 agent 原生上报证据，与现有 TUI 官方引用取证的信任区别须明确记录。
- 恢复使用来源 Task 的最新 env config，不重放 trigger config、模型设置、初始 Skill 或初始消息；来源缺失沿用现有原生默认发现，不能猜测来源。Run/node 角色依据执行关联保留，恢复本身不新建执行。
- 操作前提是切换前旧 agent 已退出且旧端已停止，同一时间只运行一端。不新增跨端存活检查、锁或人工确认。关闭 App 窗口不代表 core 中 agent 已退出。不增加 Run 跨端迁移、切换门禁或专门恢复逻辑，也不承诺旧 Run 自动跨端续跑。

### framework ready、身份上报与初始输入

- 已在讨论中确定的职责分工：my-agent-toolkit framework 定义会话级 `ready`；OMP/Pi adapter 提供各自可证实的初始化依据；connect-bot 订阅并上报；herdr-bot 核对启动关联及原生身份后推进初始提交。TUI 侧也使用这一共同判断，不保留两套相互独立的业务 ready 规则。
- `ready` 的已接受含义是“本会话的启动输入管线已初始化”，不是永久可输入、全部工具已连接、模型已接收或业务已完成。它不同于 framework 扩展自身 startup/initialization、HTTP 登记成功、在线或 idle。
- 上报需能关联本次启动、当前原生会话及宿主目标，不能用旧会话事件推进新启动。重复 ready 不重复提交或重置重试次数；旧目标结束或切换后，陈旧证据不继续有效。具体消息结构与传输落点留给后续契约设计，不预先决定新增端点。
- OMP 已验证的候选依据是主编辑器真实焦点及提交闸门。session_start 可能早于闸门解除，不能直接转发为 ready，也不能阻塞 session_start 等待由其结束后才会解除的闸门。
- Pi 已验证的 session_start 晚于正式提交处理器安装，可以作为受限初始化证据，但后续启动 hook 仍可打开模态。不得把这个事件描述为完整、永久的 UI 可输入保证。
- 继续通过所选宿主的终端输入方式提交，保留既有普通文本、Skill、模板和命令语义。不将原生 sendUserMessage 当作透明替代，不新增 connect-bot 反向消息队列。
- 保持现有业务顺序，尤其 chief/Run 所需的真实身份、权威关联及历史保存必须发生在业务初始输入之前；不能把 App 的 task 参数或 core 模板首消息当作绕过关联顺序的替代。
- 每次提交仍尊重宿主的 blocked 与目标有效性判断，不能因已经 ready 就强制绕过审批或模态。提交结果与模型开始工作、已读、完成分别表达；启动回执或 started:false 不能自行解释成未提交。
- 新身份与 ready 上报保持现有同机写入限制及后端业务权威，不把启动关联值变成新的远程认证方案，不修改会话在线/离线、关机监听等既有语义。

### 初始消息的一次延迟重试

- 只针对本次发起流程中的初始消息提交；恢复不重放初始消息，报告文本通知不自动套用此新策略。
- 首次尝试被宿主明确拒绝，并能确认未提交，例如发送前 blocked，显示“未提交，5 秒后重试一次”。从收到明确拒绝开始等待 5 秒，再向同一启动目标尝试一次。
- 重试前核对启动目标和原生会话仍一致；期间目标退出或会话改变则取消。重试不新建 Tab、不重启 agent、不重复创建/恢复，也不重新执行整个业务流程。
- 自动提交尝试总数最多两次。第二次仍失败即结束自动尝试并反馈原因；重复 ready 不延长这个预算。
- 超时、断线、部分写入、响应不明确、started:false 或其他不能证明“未提交”的结果不自动重试。结果未知需如实呈现，不能凭缺少即时历史记录或模型响应断定失败。

### 关闭、状态与提示音

- 自动关闭准确关联的目标 Tab，允许连带关闭因此为空的 Workspace/Space。App 可使用其显式确认能力完成这一动作，不需要用户再次确认空容器关闭。
- 自动关闭不扩大到其他工作；TUI 若要求连带关闭额外 worktree group，则报告失败，不借本次授权关闭它们。保留关闭失败不撤销节点完成的既有语义。
- 关闭响应、布局消失与实际进程退出不是同一事实，不将异步关闭回执冒充进程退出证明；park、settle 或关闭 App 窗口不能替代结束目标终端。
- 声音由所选宿主原生播放，保留各自资源、开关、焦点抑制和触发规则，不在 bot 新增播放器或重复播音。
- TUI 状态扩展与 App/core 原生 OSC 状态通道不同。共同业务状态和 ready 契约不要求强行统一两端原生声音传输；固定扩展加载必须适配当前端，不能使 App 模式依赖无用的 TUI 集成文件。
- 核查 Herdr 控制模块之外的调用方、固定扩展目录及部署配置，保持业务调用与宿主实现的既有职责区分，不以含有 herdr 字样为由全部搬到一个目录。报告文本通知不等于显式声音通知。

## Testing Decisions

### 现有测试先例与约束

- 复用现有 Chat 创建/恢复、历史身份、chief 分派与报告、workflow 启动/恢复/报告/自动关闭的验证先例；它们的配置、持久记录、真实输入与错误分类为现有行为参考，不以新增的实现细节断言替代消费者行为。
- toolkit 已有生命周期和真实安装宿主 dock 设施，可作为 framework ready、启动身份和旧 owner 失效的验证先例。测试使用真实 OMP/Pi 版本及原生会话身份，不能只证明回调被调用。
- 本轮已完成的实验是 prior art，不是正式适配验收。覆盖真实交互进程、终端输入、原生 input 事件与历史、首次启动和同一会话恢复；不得以 print/RPC 或扩展原生发送冒充 PTY 输入。
- 新功能仍需实际 TUI/App 表面及生产 framework/connect-bot/bot 链路验证。core 终端输入成功不能单独证明 App 已认领到正确 Space、能定位关闭、会播放原生提示音或业务流程已经完整适配。
- 初始消息 5 秒一次重试尚未运行实证。验证必须区分明确发送前拒绝、第二次仍阻塞、目标变更、重复 ready、超时与部分输入；结果未知不重试的边界不能用“未立刻看到响应”替代。
- 所有原生实验隔离 HOME、APPDATA、XDG、agent 状态与 core 实例；只操作自有进程，不读取或复制用户认证，不操作默认服务或现存用户 Chat。清理以进程句柄等实际退出证据确认，不能仅凭 kill 命令成功。
- 本地受控 provider 可验证初始化、输入和历史机制，但必须标明受控，不代表真实模型选择或业务完成；真实网络验收仅使用明确授权的配置。
- 通过正常终端操作观察 UI 状态，不替换 editor/handler 或篡改状态制造 ready。`--force` 只用于本轮反例诊断，不能进入生产提交方案。
- 保存原始失败、版本与安装来源。实验脚本缺陷修正后的成功不得抹去原始记录，也不能把源码 checkout 的版本当成实际安装版本。
- 依项目测试规则选择实际受影响的检查；不以单个 fixture 或单端实验替代完整承诺。seam、各 seam 承诺及覆盖分配统一留给 `/to-contract`，本规格不作指定。

### 已有实验事实

| 实验 | 观察到的事实 | 不代表什么 |
| --- | --- | --- |
| OMP 19.0.0 / Bun 1.4.4 / Win32 ConPTY | 挂起启动 hook 时闸门关闭且没有 ready；释放后公开 UI 条件成立，首次启动与恢复均收到一次完整 interactive input，原生历史保存对应用户消息；恢复使用同一 ID/file | 不覆盖 App/core、生产上报链路、全部启动向导或所有自定义编辑器 |
| Pi 1.5.2 / Node 24.18.0 / hc 0.1.21 | 候选事件后默认 hc send 在首次与恢复场景均将输入送入管线；后续 hook 结束后出现 interactive input 和原生历史；恢复使用同一 ID/file | hc 成功返回 started:false 不等于未提交，也不等于模型已开始处理 |
| Pi 后续启动模态反例 | ready 后出现模态时，默认 hc send 两次均以 blocked 拒绝且未发送标记；诊断性强制发送则被模态接收，未进入核心对话 | 不保证 ready 后任意未来时刻提交成功，不能因此绕过 blocked |

所有最终实验的所属进程及受控 provider 均已确认退出。OMP 的安装版本与当时源码 checkout 不同，实验以实际安装行为和 bundle 指纹为准。上述受控回合不是完整 TUI/App 双端适配通过结论。

## Out of Scope

- 修改或维护 herdr、herdr-app、herdr-core、OMP、Pi 私有源码分叉。
- 同时连接两端、按 Chat 动态选端、热切换、跨端存活探测、互斥锁、自动停机或新的切换确认流程。
- 搬迁运行中的 agent、Run 跨端迁移、自动接管旧 Run、迁移历史代码现场。
- 为位置增加宿主标签、独立 spaceId、每端位置表、跨端 ID 碰撞防护或按名称猜测工作树归属。
- 更改 cwd 恢复规则、硬编码默认容器名称、增加统一固定 cwd 或自动创建缺失默认容器。
- 将 ready 扩展为所有工具已连通、永久可输入、模型已读、业务已完成或恰好一次交付保证。
- 为输入新增反向消息队列、普遍重试框架、无限重试、结果未知时自动重发，或将初始输入重试扩展到业务报告通知。
- 强制穿透审批/模态，或者自动关闭无关 Tab、Space、Workspace 或 worktree group。
- 在 bot 新增声音播放器、统一两端原生提示音触发规则、加入 App 专属新功能或未被现有生产功能调用的通用显式播音能力。
- 改写既有在线/离线、关机监听、同机写入信任模型或 Task/Chat/Run 领域语义。
- 在本规格阶段选择新的 seam、制定逐 seam 测试合同或直接开始正式实现。

## Further Notes

### 领域与既有决策

使用根 GLOSSARY 的 Task、Chat、agent session、会话登记、历史会话列表、来源 Task、zone、Herdr Workspace、chief、Run 和节点执行角色定义。App Space 在本规格中承担容器分组职责，不是 zone 或 cwd；ready 不是 agent 工作状态或会话在线状态。

继续遵守：

- [ADR 0009](../../docs/adr/0009-rename-workspace-to-zone-for-herdr.md)：zone 与 Herdr Workspace 不混用。
- [ADR 0004](../../docs/adr/0004-session-write-boundary-same-host.md)：会话写入只接受同机来源，启动关联不是远程认证。
- [ADR 0006](../../docs/adr/0006-server-authoritative-session-lifecycle.md)：后端权威会话状态，浏览器观察不决定会话生命周期。
- [ADR 0015](../../docs/adr/0015-node-role-from-execution-context.md)：节点角色来自执行上下文，恢复不凭 Task 用途重新赋予角色。
- [ADR 0014](../../docs/adr/0014-unpersisted-omp-startup-identity.md)：保留先关联再输入、不伪造原生文件和不忽略身份冲突。其现有取证描述以 TUI 官方引用为基础；本规格确认增加由启动关联约束的 agent 原生上报来源，并由获批 B1 为环境降级且无可靠证据的 resume 增加显式“身份未确认”结果，而不是宣称恢复成功。正式实现应同步更新这两处决策说明，不能把普通登记、任意路径或请求参数当作已核对身份。

### 实验证据与代码调查入口

- [OMP 最终控制器证据](../../../herdr-adaptation-experiment/experiment-omp/attempt4/host-events.jsonl)、[首次启动](../../../herdr-adaptation-experiment/experiment-omp/attempt4/first/omp-events.jsonl)、[恢复](../../../herdr-adaptation-experiment/experiment-omp/attempt4/resume/omp-events.jsonl)。同目录保留 ConPTY 输出与原生历史；早期失败保留于 attempt1–attempt3。
- [Pi 最终汇总](../../../herdr-adaptation-experiment/experiment-pi/runs/20261120-013722-d7c0259c/summary.json)、[控制器时序](../../../herdr-adaptation-experiment/experiment-pi/runs/20261120-013722-d7c0259c/controller.jsonl)。各格保留原生事件、终端输出、历史与 provider 日志；两个早期脚本失败亦已保留。
- 当前控制与位置决策的调查入口为 `backend/src/herdr/`；实际业务调用还存在于 chats、runs、chief 和报告投递模块，不能只修改一个目录便宣布覆盖。
- 固定 Task 扩展清单中存在 TUI 的 OMP/Pi 状态扩展，应与 toolkit framework/adapter/connect-bot 变更一起核查。
- 当前 bot 源码的定向检索未发现显式 `notification show --sound` 生产调用；已确认的是宿主状态驱动的自动提示音及向 Chat 输入文本的报告通知。此结论不代表已经实播验证。
- 本地调查仓库为 herdr-bot、my-agent-toolkit、herdr、herdr-app、herdr-core、oh-my-pi 与 pi；core 调查版本为 0.1.21。App 最低 core 版本及静态方法表兼容只是部署参考，不替代实际实例握手与运行验收。

### 发布与下一步

本规格已按本地 Markdown issue tracker 发布并标记 `ready-for-agent`。用户已经确认设计及上述明确取舍；实验只证明记录的范围，正式实现与验收尚未发生。

[获批 Contract](contract.md) 已记录 S1/S2/S3、逐项承诺与覆盖、恢复降级的最终取舍及 Rapid iteration 验收分工。唯一人工项是全部自动工作完成后的两端原生提示音实际听音确认；其余实现与自动检查由 agent 完成。

下一步：执行 `/to-tickets .scratch/herdr-app-adaptation/contract.md`，按获批合同切分交付工作。若实现推翻 seam 或 promise，先修订合同，再重开受影响票据。
