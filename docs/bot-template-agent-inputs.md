# Bot 模板：OMP / Pi 文本初始化能力核对

日期：2026-10-21。本文记录源码调查结论，不表示 Bot 模块已经实现。

## 已确认的产品边界

用户已确认 Q6：模板按目标 agent 的原生规则表达初始化输入，通过 Herdr 投递；不要求不同 agent 具有统一的 Skill 协议。connect-bot 是独立于 Herdr 的会话接入与管理机制，不为该初始化路径增加依赖。

用户已确认 Q7：将 Skill 调用、模板固定 prompt 和用户在界面填写的 input 合成为一次输入提交；不先单独调用 Skill 再发送任务。对于 OMP / Pi 的显式 Skill 调用，命令名后用普通空格接固定 prompt，再分段接用户 input。这里的 prompt 是用户消息中的固定指示，不是 system-role 消息。

用户已确认 Q8：创建流程展示“正在启动”和“正在提交初始指示”两个阶段；只有 Herdr 确认目标 agent 已可交互，且合成输入提交成功，才显示“创建成功，初始指示已提交”。不等待模型完成任务，不将 connect-bot 接入作为成功条件，也不声称 Skill 已执行成功。Agent 就绪但输入提交失败时，必须区分已存在的 agent 与未确认提交的初始指示。

用户已确认 Q9：每个 Bot 对应固定 Herdr Workspace 下新建的独立 Tab 及其初始 Pane 中的一个 agent；每次创建显式将工作目录设为所选 Zone 的目录。同一 Zone 可创建多个 Bot，不复用已有 agent。

用户已确认 Q10：目标 Workspace 必须显式配置在服务端 TOML 中，不能为空，初始配置名称为 `bot-playground`。Herdr 未运行或目标 Workspace 不存在时，阻止创建并明确提示；不自动启动 Herdr、创建 Workspace、选择当前 Workspace 或回退其他目标。浏览器不能覆盖服务端配置。用户已确认 Q12：创建时按配置名称精确匹配 Workspace，必须恰好匹配一个；零个或多个均报错。缺失、空字符串或纯空白配置在 herdr-bot 启动时即报配置错误；Herdr 运行状态和目标存在性在创建时检查。

用户已确认 Q11：部分失败时保留已经创建的 Tab / agent，展示失败阶段及可定位对象信息；不自动删除、不自动重新创建，也不自动重发初始输入。超时不证明操作未执行，不将失败伪装为成功。

用户已确认 Q1：新增独立 `bot` 入口（承载 Bot 模板管理），支持新增、编辑、删除模板，服务端保存并由电脑与手机共享；Zone 行上的 `+` 只负责选模板、填写初始 input 和提交创建。

用户已确认 Q4：模板编辑仅影响后续创建，删除模板不关闭已有 Bot；请求被接受时使用当时的模板内容，后续编辑不影响正在进行的创建。

用户对 Q13 明确要求：在 bot-template 中新增 `bot-name` 一行，用来配置自动创建的 Bot 名称由哪些字段拼接；Bot 名称用于 Herdr Tab 展示，与模板显示名区分。用户已确认 Q14–Q16：`bot-name` 使用固定文字与占位符组成的单行规则，编辑时提供字段插入与示例预览；首批字段为 `{zone.name}`（Zone 末级目录名）、`{template.name}`（模板显示名）、`{agent}`（Agent 类型）、`{bot.id}`（本次 Bot 的短实例标识，非 Herdr Tab ID）。创建弹窗只预览生成结果，不允许手工覆盖。用户已确认 Q17：当前接受同名 Tab，严格按命名规则生成显示名，不自动追加去重后缀；推荐初始规则包含 `{bot.id}`，但用户可以移除。显示名不充当对象身份，不能直接作为要求唯一的 Herdr Agent 控制别名。

用户已确认 Q2：模板字段为 `name`、`bot-name`、`agent`、`skill`、`prompt`。`name` 和 `bot-name` 必填，`bot-name` 初始规则为 `{zone.name} · {template.name} · {bot.id}`；`agent` 必选一个固定选项（`omp`、`pi`）。`skill` 可不选，选择时只能从固定列表单选，初始选项显示为 `/teach`、`/code-review`，不允许任意文本输入；用户已确认 Q20：两者表示真正的 Skill，实际分别投递 `/skill:teach …` 与 `/skill:code-review …`，不是原样执行同名扩展命令或提示词模板。`prompt` 是可选的多行固定指示。

用户已确认 Q18：创建结果展示生成名称、Zone、Herdr Tab / Pane 标识及成功或失败阶段，用户自行关闭弹窗；不自动跳转 `chat` 入口，不插入 connect-bot 会话登记。本次不增加 Bot 列表、聊天或关闭 Bot 功能。创建 Bot 与现有 `chat` 入口没有自动关联。

用户已确认 Q19：所有能访问当前实例的设备均可管理模板和创建 Bot，本次不增加账号或权限系统；这是可在服务端启动 Agent 和提交任务的写入能力，不将局域网来源视为认证。

用户在 Q21 / Q26 明确要求不增加额外的非空组合校验：prompt 与 input 即使都为空，也继续按所选 Skill 合成输入并提交；单独的 `/skill:名称` 仍是有效的非空输入。不引入“仅创建、不发送”的成功分支。如果 Herdr 拒绝提交，则按 Q11 保留已创建的 Tab / agent、展示错误并中断，不自动删除、重建或重发，不能宣称创建成功。用户已在 Q27 明确拒绝 Skill 必选限制，保留 Skill 可不选。即使 Skill、prompt、input 全为空，也不增加此组合的创建前校验，不跳过消息提交来宣称成功；由 Herdr 返回结果，拒绝时按既定错误流程保留现场并中断。

## 模板扩展与创建前文件检查（2026-10-22）

创建弹窗只选择已保存模板和本次 input，不接收扩展、固定项取消或入口路径覆盖。服务端每次创建重新读取当前 `meta-data/bot-options.toml`，校验模板稳定标识及 Agent 适用关系，固定本次完整启动映射；模板不保存入口副本。

在查询 Herdr、新建 Tab、启动 agent 或提交初始输入之前，自动检查本次固定和所选扩展入口存在、为普通文件且可读取。缺失、目录冒充文件或读取拒绝会以 `extension-entrypoint-invalid` 报告扩展名称、标识、入口与具体原因，并明确未创建 Herdr Tab、未启动 agent、未提交输入。元数据或选择失效也在外部创建之前拒绝。未选扩展的文件缺失不影响有效组合；全局元数据结构和关系仍须合法。

OMP 与 Pi 均在 `--session-dir <独立目录>` 后传 `--no-extensions`，再按元数据顺序重复 `--extension <入口>`，集合恰为对应 Agent 的固定项与模板所选项。不会写入用户或项目 Agent 设置。零可选项仍加载对应 model-cost；不选 connect-bot 不影响既有成功定义，不等待或伪造 chat 登记，也不自动跳转 chat。

检查只打开、检查文件类型并读取文件，不导入或执行扩展，不调用模型或余额/账号探测。它不证明扩展或 Skill 初始化/执行成功，也不保证检查到实际加载之间文件不会变化。通过后仍须 Herdr 确认 agent 就绪，再提交一次原生合成初始输入；只有提交确认才成功。后续失败保留真实 Tab、Pane 和 agent 定位，不删除、重建或重发。

自动证据位于 `frontend/browser-tests/templateExtensionLaunch.spec.ts`：六组合字面预期、真实隔离文件故障（包括 Windows 独占句柄读取拒绝）、实际页面错误与独立 Herdr 调用/对象记录、同进程入口映射更新及未选缺失入口。Playwright 附件 `controlled-herdr-six-combinations-NOT-real-loading` 与 `controlled-herdr-dynamic-entry-mapping-NOT-real-loading` 供真实加载验收复用。它们仅证明受控 Herdr 调用正确，不表示真实 OMP/Pi 已通过加载或 ambient 对照；真实 Herdr→OMP/Pi 六组合及 H1 判断仍由 08 负责。

## 模型与思考级别：固定选项方案已确认

Herdr 的 `agent.start` 接受 `args: Vec<String>`，用于传递目标 agent 的原生启动参数（[Herdr 参数定义](../../herdr/src/api/schema/agents.rs#L186-L199)）。OMP 与 Pi 当前均支持 `--model <model>`、`--thinking <level>`，因此应在启动时指定，而不是先启动再通过 slash 命令切换。

- [OMP 参数表](../../oh-my-pi/packages/coding-agent/src/cli/flag-tables.ts#L143-L145)与[思考参数转换](../../oh-my-pi/packages/coding-agent/src/cli/flag-tables.ts#L211-L222)；[思考选择器](../../oh-my-pi/packages/tui/src/thinking.ts#L1-L9)包含 `auto`。
- [Pi 参数解析](../../pi/packages/coding-agent/src/cli/args.ts#L117-L118)、[思考参数解析](../../pi/packages/coding-agent/src/cli/args.ts#L167-L175)；合法级别为 `off`、`minimal`、`low`、`medium`、`high`、`xhigh`、`max`，不含 `auto`。
- 需按 agent 区分可用模型、合法级别及模型能力；相同的 CLI 级别名称不保证不同模型的实际推理行为相同。Herdr 只透传参数，不负责解释模型配置。
- 已运行 Pi 生产 `parseArgs` smoke，确认 `--model openai/gpt-5.4 --thinking high` 解析结果为对应 model/thinking。没有发起模型请求或验证该账号模型可用性。OMP 源码参数转换 smoke 仍受 checkout 缺少 Windows native addon 阻挡；先前已运行的安装版 `omp --help` 明确列出这两个参数。

用户已确认 Q22：模板增加 `model` 和 `thinking`，均可选择“沿用 Agent 默认值”；仅显式指定时传入对应启动参数。用户补充要求两项都只能从预置配置选项选择，不允许自由输入；选项按当前 OMP / Pi 代码中的可选项固定下来，按 agent 区分，不做运行时模型发现或自动更新。固定模型选项不表示对应账号已获权限或模型服务一定可用。源码核对结果：

- OMP 的固定目录为 [`packages/catalog/src/models.json`](../../oh-my-pi/packages/catalog/src/models.json)，本次实际读取 77 个 provider、5,714 条记录，其中 5,487 条为 chat（未写 `kind` 的记录按 chat 处理）。应排除 image、embedding 等非对话类型；选择值保留 `provider/id`，不混入用户自定义配置或运行时发现结果。已用 Node 直接读取 JSON 并验证所有记录有 provider/id；直接 import catalog API 仍触发缺失 native addon，因此不能声称该 API 可在当前 checkout 无依赖执行。
- Pi 的内置目录由 [`models.generated.ts`](../../pi/packages/ai/src/models.generated.ts) 和 provider 分片引用生成的 `providers/data/*.json` 及 `.manifest.json`；当前 checkout 缺失该目录。生成流程见 [`generate-models.ts`](../../pi/packages/ai/scripts/generate-models.ts)，不能拿生成脚本内的局部覆盖常量冒充完整模型列表。安装版 `pi --list-models` 走鉴权过滤后的 `getAvailable`（[`list-models.ts`](../../pi/packages/coding-agent/src/cli/list-models.ts#L29-L44)），也不等价于当前源码内置全集。因此 Pi 的准确完整快照尚未取得；该限制只影响全量模型方案，不应静默用 OMP 清单代替。用户后续已缩小为两项模型，不再以获取完整目录为实施前置。
- OMP 思考能力元数据为 `reasoning` 与 `thinking.efforts`，见 [`model-thinking.ts`](../../oh-my-pi/packages/catalog/src/model-thinking.ts)；Pi 为 `reasoning` 与 `thinkingLevelMap`，见 [`models.ts`](../../pi/packages/ai/src/models.ts#L1230-L1258)。全局 CLI 接受的级别不等于每个模型都支持，agent 自身还可能调整不支持的级别。用户已确认 Q23：模板思考级别随具体模型支持集合联动；非推理模型仅保留默认值，模型沿用默认值时思考级别也必须沿用默认值。用户已确认 Q24：更换 Agent 时 model / thinking 重置为默认；更换 model 时保留仍支持的 thinking，否则重置为默认并提示。

用户进一步将模型范围收敛为仅 `gpt-6.1-sol` 和 `deepSeek-flash` 两项（另保留“沿用 Agent 默认值”），不纳入全部内置模型。源码中的 DeepSeek 原生标识是小写 `deepseek-flash`，provider 为 `deepseek`；用户已确认 Q25：OMP、Pi 的两项模型分别固定映射到 `openai-codex/gpt-6.1-sol` 和 `deepseek/deepseek-flash`，不使用裸模型名模糊匹配，也不自动更换 provider。该范围下不再需要取得 Pi 的完整模型目录来实现全部模型选项，但仍需准确核对所选两项的能力与标识。

## 结论

OMP 和 Pi 的交互 TUI 都可以用文本加提交键触发 Skill、内建 slash 命令、扩展注册命令及提示词模板，不要求手工选择自动补全项。Herdr 的 `agent.prompt` 可以承担文本提交；但文本提交成功不等于命令执行成功，也不等于 Skill 已被加载。

| 输入类型 | OMP | Pi |
| --- | --- | --- |
| 显式 Skill | `/skill:<name> <任务>` | `/skill:<name> <任务>` |
| 正文中嵌入 Skill token | 支持，受输入前缀限制 | 不支持，必须以 `/skill:` 开头 |
| 扩展命令 | 已加载扩展注册的 `/<command> <args>` | 已加载扩展注册的 `/<command> <args>` |
| 提示词模板 | 已发现的 `/<template> <args>` | 已发现的 `/<template> <args>` |
| 交互选择器 | 命令可打开，但不能仅凭该文本完成选择 | 同左 |

命令名与参数之间统一使用普通空格。例如：

```text
/skill:review 请检查当前目录的登录逻辑
重点关注权限检查。
```

Pi 的 Skill 解析使用第一个普通空格分隔名称和参数；不要改成紧接名称的换行或 Tab。OMP 支持任意空白分隔，但无需依赖这一差异。[P1][O1]

## Skill 并非持久模式切换

OMP 读取 Skill 文件，去除 frontmatter，构造用户归属的 custom message；参数作为同一条消息中的用户请求。Pi 将 Skill 内容展开成 `<skill ...>...</skill>` 块，再附加任务参数，作为用户输入。空闲时，这种提交会发起一轮模型处理，不只是“启用 Skill 等待下一条任务”。[O2][O3][P1]

根据已确认的 Q7，初始化采用同一次 `/skill:<name> <固定 prompt 与用户 input>` 调用，避免先发起无具体任务的 Skill 轮次，再发送另一条任务。

## 必要前提与限制

- Skill、扩展命令、提示词模板必须已被对应 agent 加载；项目资源还可能受项目信任设置限制。[O4][P2][P3]
- OMP 的 `skills.enableSkillCommands` 关闭时不会按 TUI Skill 命令处理；Pi 的 `enableSkillCommands` 只控制命令发现，手工输入仍可展开。[O4][P2]
- 未知 Skill 可能作为普通文本发给模型，而不是报告初始化失败。因此 Herdr 写入成功不能用作资源存在性验证。[O2][P1]
- 不要在提示词模板正文中放 `/skill:...` 并期待再次触发命令。两者的 Skill 识别都先于提示词模板展开，不会对展开结果重新跑 Skill 命令解析。[O5][P1]
- `/settings` 等会打开选择器；需要其他按键交互。OMP 的 `/model` TUI 路径打开选择器，不能假定所有 slash 命令都是无交互设置命令。[O6][P4]
- 终端必须处于预期的 agent 输入界面；Herdr 在识别为 `blocked` 时拒绝 `agent.prompt`。它提供的是终端文本与 Enter 提交，不是带角色的聊天 API。[H1]
- 多行文本应作为一次粘贴再提交，不要逐行模拟多个 Enter。Pi 的 bracketed-paste 路径会取消自动补全，避免提交键仅确认补全项。[P5][H1]

## 新对话与会话目录

创建 Bot 必须得到独立的新对话，不能因为 agent 的隐式续接设置载入同一 cwd 的旧上下文。

- OMP 的 `autoResume` 设置在没有显式会话标志或会话目录时，会续接 cwd 最近的会话
  （[设置说明](../../oh-my-pi/docs/settings.md)、[会话操作](../../oh-my-pi/docs/session-operations-export-share-fork-resume.md)）。
  其启动决议在 `--session-dir` 存在时直接创建新会话，不再进入 autoResume 分支
  （[main.ts](../../oh-my-pi/packages/coding-agent/src/main.ts)）。`--session-dir` 是启动参数
  （[flag-tables.ts](../../oh-my-pi/packages/coding-agent/src/cli/flag-tables.ts)）。
- Pi 在没有 `--continue` / `--resume` 时默认创建新会话；同样支持 `--session-dir`
  （[args.ts](../../pi/packages/coding-agent/src/cli/args.ts)）。
- herdr-bot 每次创建都把 `--session-dir` 设为模板存储目录旁的
  `bot-sessions/<本次创建标识>`，并在该目录中启动目标 agent。这样启动选择不依赖 cwd 查找，
  也不以新 Tab 本身作为“新对话”的证据；目录是真实外部位置，不是伪造的 Herdr 标识。
- 受控 Herdr 记录可以证明启动选择与参数；真实 OMP / Pi TUI 是否未载入旧上下文仍需人工 H1
  在实际终端中对照旧、新会话文件或会话标识。

## 验证记录

已执行两组无模型请求的生产模块 smoke：

1. OMP：直接 import `parseSlashCommand`、`parseCommandArgs`、`allowsSkillTokens`、`SKILL_TOKEN_RE`，验证命令解析、带引号参数、嵌入 Skill token 与其他 slash 命令的隔离。结果 PASS。
2. Pi：直接 import `expandPromptTemplate`，验证 `/review login.ts` 与换行参数展开，以及未知模板原样透传。结果 PASS。

完整源码会话模块的直接 import 未成功：OMP checkout 缺 Windows `pi_natives` addon；Pi checkout 缺 provider `data/.manifest.json`。未修改这些仓库或构建依赖。以上不是 Herdr → TUI → 模型的端到端验证；Skill 实际执行链路结论来自下列源码，未声称已观察到模型调用或任务完成。

## 一手来源

路径均相对于当前仓库；外部仓库源码可能随开发变化。

- [O1] [OMP Skill 解析](../../oh-my-pi/packages/coding-agent/src/extensibility/skills.ts#L639-L664)。
- [O2] [OMP Skill 命令识别与消息构造](../../oh-my-pi/packages/coding-agent/src/modes/skill-command.ts#L53-L122)。
- [O3] [OMP Skill 消息模板](../../oh-my-pi/packages/coding-agent/src/prompts/skills/user-invocation.md)。
- [O4] [OMP Skill 交互文档](../../oh-my-pi/docs/skills.md#L155-L180)。
- [O5] [OMP TUI 输入路由](../../oh-my-pi/packages/coding-agent/src/modes/controllers/input-controller.ts#L1039-L1210)；[会话 Prompt 处理](../../oh-my-pi/packages/coding-agent/src/session/agent-session.ts#L7288-L7317)。
- [O6] [OMP 内建命令的 TUI handler 优先规则](../../oh-my-pi/packages/coding-agent/src/slash-commands/builtin-registry.ts#L141-L190)。
- [P1] [Pi Prompt 处理及 Skill 展开](../../pi/packages/coding-agent/src/core/agent-session.ts#L2058-L2201)。
- [P2] [Pi Skill 文档](../../pi/packages/coding-agent/docs/skills.md#L43-L67)。
- [P3] [Pi 资源加载](../../pi/packages/coding-agent/src/core/resource-loader.ts)；[提示词模板展开](../../pi/packages/coding-agent/src/core/prompt-templates.ts#L309-L326)。
- [P4] [Pi TUI 内建命令路由](../../pi/packages/coding-agent/src/modes/interactive/interactive-mode.ts#L3201-L3399)。
- [P5] [Pi 编辑器粘贴处理](../../pi/packages/tui/src/components/editor.ts#L1157-L1325)。
- [H1] [Herdr agent 输入说明](../../herdr/docs/next/website/src/content/docs/agent-automation.mdx#L61-L81)；[输入提交实现](../../herdr/src/app/api/agents.rs#L235-L269)。
