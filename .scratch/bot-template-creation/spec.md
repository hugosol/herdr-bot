# Bot 模板管理与从 Zone 创建 Bot

Status: ready-for-agent
Label: ready-for-agent
Type: spec
Date: 2026-10-21

## Problem Statement

用户目前需要离开 herdr-bot 主界面，手工在 herdr 中安排终端、进入目标目录、启动 agent、选择模型与思考级别，并输入 Skill 和任务指示。重复创建相似用途的 agent 时，这些操作容易遗漏、配置不一致，也不方便从手机完成。

现有 zone 登记列表能表达任务所在的本机目录，现有会话列表则只表达通过 connect-bot 接入的 agent session；两者都不能代替“按固定模板在 herdr 中创建一个独立 Bot”的能力。用户需要在保持这几个概念独立的前提下，从 zone 直接创建使用预定配置的 Bot，并知道创建进展、何时成功，以及失败后留下了什么。

## Solution

新增独立的“Bot 模板”tab，集中管理电脑和手机共享的 bot-template。模板保存显示名、Bot 自动命名规则、agent、模型、思考级别、可选 Skill 和固定 prompt。

zone 登记列表的每个 zone 提供“+”入口。用户选择模板、填写可为空的初始 input、预览自动生成的名称，确认后由服务端在显式配置的 herdr Workspace 中新建独立 Tab，以 zone 目录为工作目录启动 agent，待其可交互后，将 Skill、模板 prompt 和本次 input 合成一次输入提交。

界面区分“正在启动”和“正在提交初始指示”。只有 agent 已就绪且输入提交成功，才显示“创建成功，初始指示已提交”。失败或超时则显示失败阶段和可定位的对象信息，保留已创建的现场，不自动删除、重建或重发。

创建 Bot 不依赖 connect-bot，不自动登记到现有会话列表，也不自动跳转。connect-bot 继续独立承担其原有会话接入与管理职责。

## User Stories

1. As a herdr-bot 用户, I want 在独立的“Bot 模板”tab 中管理模板, so that 我不用在每次创建 Bot 时重复设置相同内容。
2. As a herdr-bot 用户, I want 新增一个 Bot 模板, so that 我能为一种固定用途保存创建配方。
3. As a herdr-bot 用户, I want 查看和编辑已有模板, so that 后续创建能使用调整后的配置。
4. As a herdr-bot 用户, I want 删除不再使用的模板, so that 选择列表保持清晰。
5. As a 电脑与手机交替使用的用户, I want 两端共享服务端保存的模板, so that 不需要按浏览器重复配置。
6. As a herdr-bot 用户, I want 模板在服务重启后仍被保存, so that 常用配置不会随着一次运行结束而丢失。
7. As a 模板作者, I want 为模板填写显示名, so that 创建时能辨认其用途。
8. As a 模板作者, I want 在 bot-name 中组合固定文字和字段, so that 创建出的 Bot 名称符合我的辨认习惯。
9. As a 模板作者, I want 插入可用命名字段并查看示例预览, so that 不必猜测规则的效果。
10. As a 模板作者, I want 名称可包含 zone 名、模板名、agent 类型和短实例标识, so that 我能按需要表达任务来源并区分实例。
11. As a 模板作者, I want 从 omp 和 pi 中选择一个 agent, so that 相同创建入口可启动不同 agent。
12. As a 模板作者, I want 从固定模型选项中选择模型, so that 不需要手工输入容易出错的模型标识。
13. As a 模板作者, I want 选择 gpt-6.1-sol, so that Bot 通过约定的 Codex 接入使用该模型。
14. As a 模板作者, I want 选择 deepseek-flash, so that Bot 通过约定的 DeepSeek 接入使用该模型。
15. As a 模板作者, I want 模型可沿用 agent 默认值, so that 我可以复用 agent 已有的运行配置。
16. As a 模板作者, I want 思考级别只能选择当前 agent 和模型支持的固定选项, so that 不会主动配置明显不适用的级别。
17. As a 模板作者, I want 思考级别可沿用默认值, so that 不必总是覆盖 agent 的原生行为。
18. As a 模板作者, I want 模型沿用默认值时思考级别也沿用默认值, so that 不会对未知的实际模型设置不匹配的级别。
19. As a 模板作者, I want 更换 agent 后重置模型和思考级别, so that 不会把另一种 agent 的配置误用过来。
20. As a 模板作者, I want 更换模型后保留仍受支持的思考级别、否则重置并提示, so that 配置变化既可预期又不会被悄悄替换成另一档。
21. As a 模板作者, I want 从固定列表选择 /teach 或 /code-review, so that 无需手工输入 Skill 调用语法。
22. As a 模板作者, I want 也能不选 Skill, so that 普通指示不被强制绑定到某个 Skill。
23. As a 模板作者, I want 填写可多行且可为空的固定 prompt, so that 可复用的指示与本次任务保持区分。
24. As a zone 用户, I want 在 zone 行点击“+”创建 Bot, so that 无需另行寻找或输入任务目录。
25. As a zone 用户, I want 在弹窗中选择模板并填写初始 input, so that 我能把本次具体任务交给预设的 agent。
26. As a zone 用户, I want 初始 input 可以留空, so that 固定 prompt 或单独的 Skill 调用也能用于初始化。
27. As a zone 用户, I want 创建时预览自动生成的 Bot 名称但不必手工命名, so that 创建过程保持简单且遵循模板规则。
28. As a zone 用户, I want 同一 zone 每次创建独立 Bot, so that 新任务不会误投递给已有 agent 对话。
29. As a 使用多个 zone 的用户, I want 不同 zone 的 Bot 都位于同一个配置的 herdr Workspace 中但各自使用正确目录, so that 终端集中管理而任务目录不混淆。
30. As a zone 用户, I want 普通目录和 Git 工作树都能作为 Bot 的目录, so that 创建 Bot 不以 Git 仓库为前提。
31. As a zone 用户, I want 系统先确认目标 agent 可交互再发送初始化输入, so that 指示不会在启动尚未完成时被提前提交。
32. As a zone 用户, I want Skill、固定 prompt 和本次 input 一次提交, so that 不会先开启无任务的 Skill 轮次再插入另一条任务。
33. As a zone 用户, I want Skill 选择真正触发 /skill:teach 或 /skill:code-review, so that 不会误调用同名扩展命令或提示词模板。
34. As a zone 用户, I want 不因 Skill、prompt、input 的空值组合受到额外创建前限制, so that 系统按我的选择尝试创建和提交，而不是强制我补充任务文字。
35. As a zone 用户, I want 输入被 herdr 拒绝时明确失败并保留现场, so that 不会看到虚假的创建成功。
36. As a zone 用户, I want 看到正在启动和正在提交初始指示的阶段, so that 能理解等待发生在哪一步。
37. As a zone 用户, I want 只有 agent 就绪且输入提交成功才看到创建成功, so that 成功提示有明确且一致的含义。
38. As a zone 用户, I want 创建成功不必等待模型完成任务, so that 长任务不会把创建弹窗一直挂起。
39. As a zone 用户, I want 结果展示 Bot 名、zone 和 herdr Tab / Pane 标识, so that 我能找到新建实例。
40. As a zone 用户, I want 已启动但提交失败的 agent 与完全未创建的情况被区分, so that 我知道现场是否已经存在。
41. As a zone 用户, I want 超时不会触发自动重建或重发, so that 同一个任务不会因为响应不确定被重复执行。
42. As a zone 用户, I want 失败后保留已创建的 Tab 和 agent, so that 我能检查终端中的登录、审批或其他问题。
43. As a 模板作者, I want 编辑或删除模板不影响已有 Bot, so that 模板管理不会改变正在运行的任务。
44. As a zone 用户, I want 创建被接受后使用当时的模板内容, so that 创建途中发生的模板编辑不改变本次行为。
45. As a herdr 管理者, I want 明确配置目标 Workspace 名称, so that Bot 不会被创建到当前焦点或其他意外位置。
46. As a herdr 管理者, I want 未配置有效 Workspace 名称时服务启动报错, so that 部署错误不会被隐藏成默认目标。
47. As a herdr 管理者, I want 找不到 Workspace、存在同名歧义或 herdr 未运行时创建明确失败, so that 系统不会自行启动服务或另建容器。
48. As a herdr 用户, I want 允许按我的命名规则创建同名 Tab, so that 系统不会偷偷改变名称或追加后缀。
49. As a herdr 用户, I want 控制对象不依赖显示名唯一, so that 同名 Tab 仍可被正确定位。
50. As a connect-bot 用户, I want Bot 创建与原有会话列表互不自动关联, so that 独立来源的会话不会被混成一种登记。
51. As a zone 用户, I want 结果留在创建弹窗并由我关闭, so that 不会被自动切到会话列表。
52. As a 手机用户, I want 拥有与电脑端相同的模板管理和创建能力, so that 在可访问该实例的设备上都能完成操作。

## Implementation Decisions

### 领域与兼容边界

- Bot 是从 bot-template 为某个 zone 创建、由 herdr 托管的独立 agent 对话实例，不是 herdr 自己称作 Session 的服务端命名空间，也不是 connect-bot 的会话登记。
- 一个 Bot 对应配置 Workspace 下新建的独立 Tab，以及其初始 Pane 中启动的一个 agent。每次创建新实例，不复用已有 agent；同一 zone 可以创建多个 Bot。
- zone 仍是用户登记的服务器本机目录；工作目录显式取该 zone 的目录。不能用 Workspace 的当前焦点目录或隐式继承替代。zone 无需是 Git 仓库。
- 保持现有会话列表、会话登记、keepAlive、状态上报和关机行为不变。不得用创建 Bot 的请求伪造 connect-bot 的登记事件。
- 遵守现有“zone 与 herdr Workspace 分名”的领域决策，以及“connect-bot 独立于 herdr”的定位。

### 模板管理与字段

- 新增独立的“Bot 模板”tab，提供新增、编辑、删除。模板服务端持久化，电脑和手机共享；不是浏览器私有保存。
- 模板字段为 name、bot-name、agent、model、thinking、skill、prompt。
- name 必填，是模板显示名，不是新 Bot 的固定名字。
- bot-name 必填，是命名规则；初始规则为“{zone.name} · {template.name} · {bot.id}”。
- agent 必选一个固定值 omp 或 pi。
- skill 可不选；选择时只能从 /teach、/code-review 固定列表单选，不允许任意文本输入。这两项实际表示名为 teach、code-review 的 Skill，而非同名自定义命令。
- prompt 是可多行编辑、可为空的固定指示文本。它是最终用户输入的一部分，不是 system-role 消息，也不是另一个 agent 提示词模板的名称。
- model、thinking 均通过预置选项选择，不允许自由输入。各自保留“沿用 Agent 默认值”；未显式指定时不传对应启动参数。
- 用户已明确不强制 skill 必选，也不增加“prompt 与 input 至少一项非空”或“skill、prompt、input 至少一项非空”的组合检查。该决定不取消已确认的模板名称、命名规则、agent 选择和 Workspace 配置要求。
- 创建请求被接受时采用当时模板内容；之后编辑不影响正在创建的 Bot。模板编辑只影响后续请求；删除不关闭已有 Bot。

### 命名规则

- bot-name 是单行固定文字加占位符，不是脚本或可执行表达式。
- 首批字段固定为 {zone.name}（zone 末级目录名）、{template.name}（模板显示名）、{agent}（agent 类型）、{bot.id}（本次 Bot 的短实例标识，非 herdr Tab ID）。
- 编辑模板时提供字段插入和示例预览；创建弹窗预览本次生成的名称，不提供手工覆盖。
- 命名结果作为 herdr Tab 显示名。允许同名 Tab，严格使用规则结果，不自动追加去重后缀。
- Tab 显示名不充当对象身份。内部控制使用 herdr 返回的对象标识及符合其要求的独立 agent 控制别名；不能直接拿可能重名、含中文或空格的 Tab 名作为要求唯一的 agent 别名。
- 后续模板改名不重命名已创建 Bot。

### 模型与思考级别

- 只提供 gpt-6.1-sol、deepseek-flash 两项显式模型，以及沿用默认值；不引入全部内置模型目录。
- omp 与 pi 的 gpt-6.1-sol 都固定映射到 openai-codex/gpt-6.1-sol；deepseek-flash 都固定映射到 deepseek/deepseek-flash。
- 不用裸模型名模糊匹配，不自动换 provider。界面展示名称与实际 provider / model 标识可以区分，但选择结果必须确定。
- 根据当前两种 agent 的源码能力固定可选项，不做运行时模型发现、账号可用模型筛选或自动刷新。预置选项不承诺账号已有凭据或调用权限。
- omp、pi 当前都能通过原生启动参数 --model 和 --thinking 接受配置；由 herdr 的启动参数透传能力传入，在发送第一条任务前完成设置，不通过启动后的模型选择器或 slash 命令切换。
- 思考选项按 agent 和具体模型的能力固定，不把全局 CLI 支持级别当成每个模型都支持。OMP 的 CLI 包含 auto；Pi 不接受 auto。模型能力数据和原生级别映射必须尊重各自代码，不能直接将 OMP 的整份能力表当作 Pi 的能力表。
- 选择具体模型后，只提供该 agent / model 适用的 thinking；非推理模型只保留沿用默认值。model 沿用默认值时，thinking 也必须沿用默认值。
- 更换 agent 时，model 和 thinking 重置为默认值。更换 model 时，原 thinking 仍适用就保留，否则重置为默认并提示。

### 固定 Workspace 与部署行为

- 目标 herdr Workspace 名称必须显式配置在服务端 TOML 中，初始配置名称为 bot-playground；不是运行时猜测的缺省值。
- 配置缺失、空字符串或纯空白时，herdr-bot 启动即报配置错误。浏览器不能覆盖服务端指定的目标。
- 每次创建在选定的 herdr 连接目标中按配置名称精确查找 Workspace，必须恰好匹配一个；零个或多个都明确失败，不任取第一个。
- herdr 未运行、目标不存在或存在重名歧义时，不自动启动 herdr，不新建 Workspace，不使用当前焦点目标，不回退其他 Workspace。
- 连接配置、持久化结构、HTTP 资源及内部接口的具体形状尚未在讨论中确定，本 spec 不据此选择接缝。

### 创建交互与单轮输入

- zone 行提供独立“+”操作；打开弹窗后选择模板、填写初始 input、预览生成的 Bot 名称，确认后创建。
- 每次显式创建新 Tab，并把 cwd 设为 zone 目录，然后启动指定 agent，等待其可交互后再提交输入。
- 初始化按“可选 Skill 调用、固定 prompt、用户 input”的次序合成一次提交，不先独立提交 Skill 再另发任务。
- 有 Skill 时，使用 /skill:teach 或 /skill:code-review；命令名称后若存在正文，先用普通空格分隔，再附固定 prompt 与用户 input。固定指示与本次任务分段表达，并保留多行正文的含义。
- 无 Skill 时，仅合成固定 prompt 与用户 input。不将 prompt 正文中的 slash 文本当成需再次递归解析的初始化步骤。
- prompt 和 input 均为空但有 Skill 时，提交单独的 Skill 调用。三者均为空时也不新增组合校验、不补造内容、不跳过提交，接受 herdr 的实际结果；被拒绝则走普通失败流程。
- 不新增“仅创建、不发送也算成功”的分支，不以减少失败为理由强制用户填写任务或选择 Skill。

### 就绪、成功与失败

- 创建进展至少区分“正在启动”和“正在提交初始指示”。
- 启动命令已排队或原始 agent.start 返回，不等于 agent 可交互。必须等待 herdr 确认预期目标 agent 已就绪，而不是固定延时后假定成功。
- herdr 的 CLI 启动命令已有等待就绪能力，原始接口需要观察对应就绪信息；CLI 或 socket 客户端的选择不在本 spec 中锁定。
- 成功条件是目标 agent 已可交互且合成输入提交成功；结果文案明确为“创建成功，初始指示已提交”。不等待模型完成任务，也不以 connect-bot 接入为成功条件。
- 输入提交成功仅表达投递结果，不承诺模型已开始工作、Skill 已正确加载或任务已完成。
- 创建结果展示生成 Bot 名、zone、已知的 herdr Tab / Pane 标识，以及成功或失败阶段；结果保留在弹窗中，由用户关闭，不自动跳转。
- Agent 已就绪但提交失败时，必须说明现场已存在而输入提交失败或结果未确认，不暗示整个 Bot 完全不存在。
- 任一步错误或超时都中断当前创建流程，保留已创建的 Tab / agent，展示已知定位信息。不自动删除、不自动重建、不自动重发。
- 超时不证明操作未执行。无法确认输入是否已投递时不能把它表述成确定未发送，也不能宣称创建成功。

### 访问边界与既有行为

- 能访问当前 herdr-bot 实例的电脑和手机均可管理模板并创建 Bot。本次不增加账号、token 或权限系统；用户接受该操作可在服务端启动 agent 和提交任务的风险。
- 新增浏览器侧创建意图不改变 connect-bot 会话事件写入的同机来源限制；两种写入是不同的产品能力，不能为支持手机创建而放宽现有会话事件边界。
- 创建 Bot 与现有会话列表没有自动联系，不插入会话登记、不修改 keepAlive 或关机监听；若 agent 另外接入 connect-bot，仍按原有协议处理。
- 本 spec 只记录已确认的产品与技术行为，不指定新增模块边界、HTTP 路由、DTO、存储格式、进度传输机制或测试接缝；由 /to-contract 决定接缝并批准相应承诺。

## Testing Decisions

- 当前仓库已有后端配置、zone 登记与创建、目录访问、共享观察、会话写入来源边界及失败恢复等测试先例；前端已有 Vitest 测试设施。沿用既有测试习惯，不在 spec 阶段指定新测试落在哪个接缝。
- 前期已执行无模型请求的源码 smoke：OMP 的命令解析、参数和 Skill token 识别；Pi 的提示词模板展开，以及 model / thinking 参数解析。这些只能证明对应本地解析行为，不能代替创建流程验收。
- 当前尚未完成 herdr 到真实 OMP / Pi TUI 的端到端创建和投递验证，不能在实现交付时把前期静态调查或解析 smoke 当作其成功证据。
- 显著行为变更需实际运行并观察目标路径；网页交互需在实际浏览器界面验证，创建行为需观察真实 herdr 中的目标 Workspace、Tab、工作目录、agent 就绪及输入提交结果。
- 测试与验证不得通过强制 Skill 必选、阻止空文本组合、吞掉 herdr 错误、伪造就绪或假报提交成功来让场景通过。
- 失败和超时验证必须尊重“可能已经执行”的边界，不能用自动重发或清理现场掩盖部分结果。
- 尚未选择专用测试替身或集成工具。接缝位置、接缝承诺与每项承诺的覆盖方案均留给 /to-contract；此处不固定测试层级或额外架构。

## Out of Scope

- 扩展 connect-bot 的初始化、命令投递或会话创建能力；将它变成 herdr 的附属层。
- 自动接入 connect-bot、自动关联现有会话列表、自动设置关机监听或自动跳转会话列表。
- 新增 Bot 列表、聊天界面、后续消息管理、关闭 Bot、恢复 Bot 或其他生命周期管理功能。
- 复用现有 agent，多个 Bot 共用同一 Tab 的分屏布局，或每个 Bot 单独创建 Workspace。
- 自动启动 herdr、自动创建或修复 Workspace、自动选择替代目标。
- 自动回滚、删除现场、重建、重发、任务级重试队列或为本次功能另加自动恢复机制。
- Skill 自动安装、资源注册表管理、通用跨 agent Skill 协议、自定义命令编辑器或多步初始化脚本。
- 多 Skill 组合、带角色的消息序列、把固定 prompt 当作 system prompt、递归执行模板正文中的 slash 命令。
- 用户在创建弹窗手工覆盖 Bot 名称、模型、thinking 等模板配置；本次用户专属输入只有初始 input。
- 强制 Skill 必选，或增加 prompt / input / skill 的非空组合校验。
- 模型自由输入、完整模型目录、运行时模型发现、动态刷新、自动 provider 回退，以及模型凭据的配置和分发。
- 以创建成功证明模型可用、Skill 执行成功或任务完成。
- 新增账号、权限系统、认证令牌或公网访问方案；放宽既有 connect-bot 会话写入来源限制。
- 在本 spec 中选择模块或测试接缝、HTTP 协议细节及持久化技术。

## Further Notes

- 本 spec 综合本轮已确认的设计，用户已接受整体收敛结果。此前提出但被拒绝的 Skill 必选、非空组合检查和“仅创建不发送”成功分支均不得重新引入。
- 领域用语依据根词汇表。相关既有 ADR：0009 将 zone 与 herdr Workspace 分开命名；0010 确立 herdr-bot、herdr、connect-bot 的定位；0004 的会话事件同机写入边界保持不变；0006 的既有会话登记与后端权威语义保持独立。
- 一手代码调查与已执行 smoke 的证据见 [Bot 模板输入能力记录](../../docs/bot-template-agent-inputs.md)。其全量模型目录调查只是背景，本 spec 的显式模型范围始终只有已确认的两项。
- 当前 Pi checkout 缺少完整生成模型目录，OMP checkout 的部分直接源码 import 受缺失 Windows native addon 限制。它们不是本产品的运行时回退理由，也不要求为两个固定模型先建设完整目录系统；实现应依据目标版本的相关模型定义核对所需能力，不伪称已完成端到端验证。
- 模型、Skill、agent 可执行程序及其账号凭据需在服务所在电脑的实际运行环境中可用；本功能负责按模板调用，不负责安装或授权。固定选项表示允许选择的配置，不是账号权限承诺。
- 接缝、可观察承诺及验收责任由 /to-contract 记录；本 spec 负责问题叙述、需求与范围，不复制契约决策。

## Comments

### 2026-10-21 — 契约已批准

已发布 [Contract — Bot 模板管理与从 Zone 创建 Bot](contract.md)。接缝选择、约束审查和 Rapid iteration 验收责任均已确认；具体承诺与人工覆盖以契约为准。这表示设计契约获准，不表示功能已实现或验收通过。

下一步：`/to-tickets .scratch/bot-template-creation/contract.md`。
