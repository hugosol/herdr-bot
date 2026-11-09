# 规划交付契约

概要规划者编写文档、Map 生成者核验方案时使用本契约。Task 型能力的稳定执行步骤在各自 Task.prompt，Map 型能力复用已核实的目标图与输入接口；本次目标与额外限制在调用 node.input。文件来源、交付标识与业务输入分工见 [node.input 的业务职责](scheduler.md#nodeinput-的业务职责)，workflow 文件读写遵循[统一文件交接约定](workflow-runtime.md#统一文件交接约定)。

## 共同身份

《能力建设需求》与《概要方案》分别面向自己的接收者，以共同方案标识、修订标记及稳定缺口编号 G1 等关联。各文档自包含，互相索引只用于追踪；接收者无需拼接另一文档或聊天历史才能工作。工作用 W1 等稳定编号，与真实 node ID 区分。

修订标明替代版本、受影响工作及交付，保留仍约束实施的结论，不附完整讨论历史。用户明确确认后记录当前版本的确认范围及依据；“已确认、待补能力”可作为中间状态，但不等于可翻译或可启动。业务确认、报告发送确认、服务结构校验与启动条件分别记录。

## 规划文档保存与交接

规划者交付《能力建设需求》或《概要方案》时使用本节；转换者读取方案时核实其身份、确认范围及完整内容。workflow 文件地址与报告协议由初始化和运行时约定提供；独立 Chat 未要求文件时可直接交付完整 Markdown 正文。

要求文件时，先核实本次材料写入授权、真实路径及父路径的链接安全，避免越界或覆盖其他产物；指定正式位置优先，workflow 内部材料使用初始化提供的位置。独立 Chat 要求文件但缺位置或授权时澄清，不自行选目录。以 UTF-8 完整保存，每份完整读回并与预期逐字比较。修订已有文件前重读基线，出现并发变化即停止。失败或效果不明先查询现场，保留原错误及已发生效果，区分已生效、未生效和无法确认，不盲重试或自动回滚。

完成条件：材料完整可读，身份及确认状态准确；要求文件时安全保存且完整读回一致。文档保存、业务批准、报告发送和管理者接受交接是不同结论。

## 创建 Map 的整图交接

“创建map”的管理者验收规划、转换与优化节点交接时读取本节；执行步骤由各节点 Task 承担，依赖、命名材料和报告策略以 Map 结构化定义为准。

- 概要规划交接：核实当前执行报告、用户对当前方案版本的明确确认，以及方案满足下文「概要方案」的可翻译条件；manual 报告须按运行时确认后发送。`can_complete` 只是完成声明，不替代业务确认。草案、待补齐能力或阻塞报告不能作为完成交接的依据；验收后才完成规划节点并安排转换。
- Map 转换交接：核实实际文件、完整读回和服务定义校验结果后，接受转换结果并安排优化，不在此时完成运行。
- Map 优化交接：核实最终 Map 与优化说明、实际保存和完整读回结果、定义检查、共享影响提醒及用户在优化 Chat 中确定的边界；manual 报告发送确认不替代保存授权或管理者验收。存在阻塞或未完成修改时不能按成功结束；验收通过后才完成运行。交付结论区分文本／定义优化与真实业务行为验证，不启动生成的业务 Run，也不修改当前规划 Map。

## 能力建设需求

有能力缺口、准备交给能力创建者时，按以下六项组织独立 Markdown；零缺口时省略本文，仅在概要方案中说明无需能力建设。

1. **身份与确认**：方案标识、本文修订标记、用户确认范围及相关缺口编号。
2. **能力要求**：各项能力的目标、职责与非目标、输入、预期输出及完成条件。
3. **复用调查**：已调查的相关 Bot 与不能直接复用的原因。
4. **缺口与证据**：配置缺口或真实能力缺口，已有工具、代码、数据、资源位置与权限，尚缺条件及依据。
5. **操作边界**：允许操作、需另行授权的操作、用户指定的宿主和模型约束。
6. **返回契约**：真实 Bot 名称与 ID、目标类型及 normal Task ID 或确切 Map 文件名、绑定、使用条件、已验证项、未验证项、阻塞及部分失败。

本文只描述能力建设，不代替整体方案。完成条件：每个缺口有稳定编号、明确所需结果、资源与权限边界以及可核实的返回要求。

## 概要方案

编写概要方案或核验其能否转换为 Map 时，逐项使用以下九章节。

1. **方案身份与确认**：方案标识、本文修订、替代版本、确认状态（草案／已确认）、当前版本确认范围及依据。
2. **目标与权限**：用户目标、验收标准、非目标、约束及已授权／未授权操作。
3. **工作划分**：每项稳定工作编号、职责、输入材料用途、预期交付内容与完成条件。
4. **能力选用**：各项真实 Bot ID、目标类型及绑定 normal Task ID 或确切 Map 文件名、适用依据与限制。Map 型工作还记录本次调用的 manager Task ID 及 manager 类型核实依据、业务 input、管理者 zone 和输入／输出映射（见下节）；不展开内部工作。待补项用缺口编号和所需结果概述表示，身份待核实，不夹带详细建设指令。
5. **依赖与材料交接**：前置工作编号、材料提供方、输出 ID、消费者所需内容与用途。已有直接文件给出可读地址；未来上游产物声明逻辑交接关系，不预造实际地址。交接说明与纯正文有独立用途时分别声明。保留用户指定的正式交付位置、格式和业务命名；依赖不代替材料规则。
6. **执行条件**：已核实资源与各工作 zone；Map 型工作所需的本次 manager、zone、业务 input 与材料接口均须落实。外层新业务根 Run 的管理者 Task、管理者 zone 和 userInput 仅记录用户明确提供的值（userInput 可明确为空）。未知根启动配置列缺项，不沿用规划 Run 配置；规划 zone 不默认适用于所有业务工作，调用节点的 zone 也不覆盖子 Map 内部各工作 zone。
7. **证据与未决项**：分别列事实、推断、未验证条件、阻塞和问题，注明来源、试验授权、实际结果与局限。
8. **下游要求**：可落实工作、必须先补齐的条件及本次额外限制。转换者按当前 Map 契约和真实能力绑定落实全部工作；Task 的稳定流程不复制进 node.input，材料来源与交付标识进入结构化规则。业务启动由用户另行决定。
9. **可选能力建设索引**：建设需求的标题、修订、位置和缺口编号，仅用于追踪。

可翻译条件：当前方案已明确确认，影响转换的能力绑定、业务输入、依赖、材料关系、zone 和权限均明确且无矛盾，Map 型调用自身的 manager Task 也已核实。仅缺外层根 Run 的启动配置不阻止交付可翻译方案；真实工具或业务效果未验证必须显式列出，不能声称验证通过。草案与阻塞方案可以保存供讨论，但不得作为可翻译成果交接。

## 两类 Bot 的能力选用与调用

概要规划和方案转换都调查两类 Bot，按职责与接口选用，而非只搜索 Task 型。通过公开 Bot 目录和完整定义读取 `when`、`plan_guide`、`node_guide`，再核实实际目标。指南、名称相似或目录摘要只是发现线索，不能证明可执行；局部损坏、目标不可读、未绑定或资源／权限不足分别记录为真实缺口，不能用相似名称替代、自动补建 Task／Bot 或调度能力创建者。能力补齐仍由用户人工衔接，带回结果后重新核实目标。

| 选用类型 | 必须核实的真实目标 | 生成调用 |
| --- | --- | --- |
| Task 型 | `task_id` 指向存在的 normal Task；完整固定职责、宿主配置、使用限制与所需资源 | 一个普通 node；不设置 `manager_task_id` |
| Map 型 | `map_file` 为服务目录内确切文件名；完整定义、`input_files`、内部 Bot/Task/Map 绑定、各 zone 和资源；本次 manager Task 必须为 manager 类型 | 一个调用 node，使用该 Bot ID；本次 `manager_task_id`、`zone` 与原文业务 `input` 明确，不展开内部节点 |

Map 型 Bot 只复用 Map 引用，不保存 manager 或每次业务要求。调用 node 的 zone 是本次子管理者位置，不覆盖子 Map 内部节点的 zone；本次 input 是子 Run 的业务要求，不复制整个子图流程、注入身份或规定 Run/execution 文件发现协议。三份指南继续用于发现、安排与填写输入，不要求框架自动注入执行 Chat；执行必需的业务差异不能仅留在指南里。

### 本次材料映射

- 先读子 Map 的 `input_files`。父调用 node 的 `input_rule.id` 必须对应子接口的同名 ID；规则仍按既有 `file`／`node`／`run` 来源填写。例如上游 `prepare` 输出 `orders_csv`，子接口要求 `source`，则调用规则为 `{"id":"source","type":"node","node_id":"prepare","output_id":"orders_csv"}`。可选项未提供就缺席；必需项无来源即阻塞，不生成占位地址。
- 父调用 node 用 `output_rule` 声明本次对外交付 ID，例如 `review_result`；子 manager 按该执行要求登记真实路径。Map 本身不增加输出声明或映射字段，规划也不预造实际路径。下游可用 `{"id":"summary","type":"node","node_id":"review","output_id":"review_result"}` 将调用节点的交付映射为自己的命名输入；`blocked_by: ["review"]` 单独表达依赖。
- 方案记录来源、接口 ID、业务用途和交付对应关系；转换者落实既有结构化字段。子流程 finish、向父执行报告、父 manager 接受交接是不同动作；保存／预检不能作为这些动作或业务已运行的证据。

### 保存与证据边界

转换者的保存步骤与覆盖授权由 Task.prompt 和本次委托承担；“创建map”的转换节点只授权新增，不修改被调用 Map、Bot 或当前规划 Map；后续优化节点的相关对象修改范围与共享影响提醒由其 node.input 指定，用户在优化 Chat 中把控具体边界，仍遵守运行禁写规则。交付须完整读回并通过公开定义查询独立校验形状、引用和依赖。

只有服务已接受定义，且本次明确提供外层根 Run 的 `managerTaskId`、zone、userInput（可明确为空）及必需文件绑定时，才执行只读 preflight；否则报告未执行及缺项，不代选、不借用规划 Run 配置。Map 型调用自身缺 manager 等条件不属于根启动豁免。保存、预检和真实业务验证分别说明，业务启动由用户另行决定。

启用回归为 `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch plan_workflow:: -- --nocapture`：公开读取实际交付的规划 Task／Bot，保存并读回 Task 型＋Map 型混合节点，核对同名子输入、父输出与下游映射，预检真实目标存在／移除后的接受／拒绝，并观察没有 Run／宿主调用。共享材料的实际受控派发由 `shared_business::` 检查；两类能力的真实模型选择与完整规划场景由独立 P03 验收拥有，不以提示词存在或上述确定形状检查替代。运行准备与最小原生共享 Task 材料烟测见[测试指引](agents/testing.md#共享-bot-指南材料的启用检查)。

`scripts/test-workflow-file-handoff.py --scenario plan-workflow` 的原生规划场景随当前三节点定义部署优化者及 writing-for-agents 技能；在优化 Chat 单独确认 manual 报告，核对最终 Map 文件与保存定义一致。隔离场景将实际写入边界收窄为新业务 Map，相关 Task／Bot／文档只读；它不覆盖共享对象修改分支。定义回归与脚本语法检查不证明该真实模型场景通过。

## 两类能力的真实原生规划验收

`python scripts/accept-workflow-nesting-planning.py` 独立拥有 P03，不能用上述确定形状回归、提示词检查或旧外层包装案例代替。公开边界为真实规划／转换 Task 的 Chat HTTP、原生工具和 SessionManager 历史、实际授权模型传输、Map 完整严格查询与只读 preflight、Run 目录及所属 OS 进程。入口不增加产品测试 API、不替模型选择 Bot、不提供生成的 JSON。

先在当前 herdr-bot worktree 配置独立绝对 `CARGO_TARGET_DIR`，构建当前静态页面和二进制；toolkit 使用指定 worktree 的源码、锁文件和 Bun 1.4.4。缺依赖时才安装：

```powershell
$env:CARGO_TARGET_DIR = Join-Path $HERDR_BOT_WT 'backend/target/planning-acceptance'
npm --prefix frontend ci
npm --prefix frontend run build
cargo build --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-paths --bin herdr-fixture --example safe_shutdown_shell
bun install --frozen-lockfile --cwd $TOOLKIT_WT
python scripts/accept-workflow-nesting-planning.py `
  --backend (Join-Path $env:CARGO_TARGET_DIR 'debug/herdr-bot.exe') `
  --toolkit $TOOLKIT_WT --hosts declared `
  --paths-file 'D:/acceptance/dependencies.toml' `
  --model-config 'D:/acceptance/model.json' `
  --evidence-dir 'D:/acceptance/new-exclusive-planning-run'
```

`--hosts declared` 保留实际交付的概要规划、概要方案转 Map 和 manager Task 的宿主映射，不把它们统一改成 Pi。业务夹具本身的普通 Task 仍分别声明 Pi／OMP；这里只核实和预检业务目标，不启动它们。`--hosts omp,pi` 另行执行完整 OMP 和完整 Pi 两组原生规划入口，分别保留结果，两组都通过才成功；显式宿主覆盖会记录，不能作为原始混合声明的通过证据。每阶段观察窗口 `--timeout` 为 1..900 秒，默认 300 秒；失败不重试业务或模型决定。

依赖文件只声明已安装程序、只读集成入口和规划技能来源，不指向用户认证或业务配置。入口通过同目录本 worktree 的 `herdr-paths.exe` 解析根引用；不会安装宿主、提权或改全局 PATH。所需根：

```toml
[roots]
herdr = 'D:/installed/herdr'
pi_package = 'D:/node_modules/@earendil-works/pi-coding-agent'
omp_package = 'D:/node_modules/@oh-my-pi/pi-coding-agent'
pi_state_extension = 'D:/readonly-integrations/herdr-agent-state.ts'
omp_state_extension = 'D:/readonly-integrations/herdr-omp-agent-state.ts'
wayfinder_skills = 'D:/sources/oh-my-wayfinder/skills'
# Windows 原生 bash 工具需使用已有 Windows shell，不落到 WSL。
native_shell = 'D:/Program Files/Git/bin/bash.exe'
```

模型声明显式提供 `provider`、`model`、无内联凭据的 `baseUrl` 和 `apiKeyEnv`；只读取这个明确命名的专用环境变量，不读用户 auth store。支持实际 OpenAI-compatible chat-completions 服务；需要宿主公开 text completion 时，可使用[测试指引中的透明 completion 传输](agents/testing.md#文件交接真实角色自动验收)，其证据仅证明此实际模型传输下的业务行为，不证明原生 provider wire 合规。模型配置仅写入隔离原生目录，退出时移除 credential-bearing `models.json`／`models.yml`；证据不保存密钥。

独立业务预期在任何模型请求前固定：本地准备 CSV 的场景只选择 Task 型能力；本地核对的场景安排准备工作和一个 Map 型调用，校验子 manager、业务 input、独立 zone、`source` 同名输入、上游 `orders_csv` 到子输入的映射和 `review_result` 对外交付，禁止展开子图。相似名称的远程发布能力是干扰项，不以目录包含名称算选择正确。三份指南加入只有真实完整读取才可取得的标记，原生 tool result 必须分别证明规划者和转换者读到所选指南及实际 Task／子 Map；初始化 system/developer 消息不得自动注入这些指南。

每个正例先保存草案，证明尚未生成 Map，再通过真实 Herdr 原生输入提交当前版本的明确确认。转换者独立读取已确认方案、生成 Map、通过公开严格查询及恰好一次只读预检。缺口场景缺生产目标决定、实现、凭据和授权，只能交付明确 G1 阻塞，不能补建 Task/Bot、生成 Map 或自主启动。全过程观察真实 HTTP、空 Run 列表、显式 Chat 请求、全部原生 Session 轨迹（包括允许的只读子代理）、既有定义哈希和所属进程；子代理 Session 不等于额外 Herdr Chat，模型自述“不启动”不算证据。读取概要方案中引用的指南文字不算重读实际能力：分别要求当前原生 Chat 的真实 Bot／目标 HTTP 响应及原生 JSON 读取结果。

证据目录必须不存在，由入口排他创建。Windows 安装版 Herdr 的 socket／原生存储目录仍受其实际路径支持限制；使用有足够空间的短绝对目录（例如 `E:/Temp/p03-<唯一标识>`），不要将深层源码路径再叠加长运行标识。路径或磁盘错误必须保留失败，不能改连用户 daemon。原生 worker broker 可能在 Herdr 退出后短暂保留；入口对已持有句柄的所属进程逐项停止并确认退出，不只检查主进程。根 `evidence.json` 包含两仓根／revision／修改身份、锁文件、Cargo depfile／产物及静态页面身份、安装宿主身份、声明／实际宿主映射和每组证据位置；组内包含原始用户输入及确认、原生历史和实际文件字节、完整模型及工具／HTTP 请求结果、OS 观察、逐场景结论、原始错误与单独清理错误。HOME／APPDATA／XDG／原生存储、命名 Herdr、业务定义／运行目录和动态数值回环端口均独占，不启用 mDNS/LAN、不连接用户 Chat、不执行关机。成功和失败均停止并等待所属进程，清理失败保持非零。

`PLANNING_ACCEPTANCE_TOOLKIT=<绝对 toolkit worktree>` 下运行 `python scripts/accept-workflow-nesting-planning.test.py` 只核对公开 CLI 缺构建时不启动、不伪报 P03 的安全边界；不是上述真实模型专项通过。缺安装宿主、专用模型授权、能力或隔离／清理失败，专项必须非零且 P03 保持未验证。两仓完整检查仍按[测试指引](agents/testing.md)及各仓规范分别执行；专项成功也不替代它们。
