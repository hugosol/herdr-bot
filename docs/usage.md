# 使用说明

本文记录日常操作细节；安装与启动见 [README](../README.md#构建与运行)。

## 页面导航

外侧导航提供两种布局，内部页面与编辑方式相同：

- **紧凑布局**（默认）：顶部只显示一组入口。`chat / zone / workflow` 用于日常工作，`task / suite / bot / scheduler` 用于配置；不额外显示分类标题。两组在横向轨道上按日常工作在左、配置在右排列。仅在导航条内左滑进入配置组，右滑回到日常工作组；整条导航跟随手指移动，松手时拖动距离达到导航可见宽度的三分之一即可切换，否则回弹，不按速度判定，也不循环切换。内容区滑动仍交给当前页面。
- **展开布局**：点击左上角布局按钮，以窄侧栏同时显示两组七个入口。电脑也可直接展开后点击目标，无需拖动。

布局按钮只改变外侧导航，不重新加载页面，不清空当前编辑或筛选状态。地址中的 `?layout=expanded` 指定展开布局，`?layout=compact` 或无参数指定紧凑布局；切换会原地更新地址，不增加浏览器历史记录。返回紧凑布局时显示当前页面所属组；同一页面期间切换分组会记住每组上次访问的入口。

整页刷新保留 URL 指定的布局，但内容回到 `chat`，即使刷新前正在查看 Run。首次打开显式 `#workflow/<Run ID>` 链接仍可读取对应 Run 详情，不重新启动。scheduler 发起与角色 Task 前提、三份记录及启动失败处置见[Map Run 运行说明](workflow-runtime.md)。

`bot` 是原有共享 Bot 管理功能的独立入口；scheduler 中的关联 Bot 进入同一预览，返回 Map 请点击外侧 `scheduler` 入口。Bot 与 Scheduler 各自面板内提供“刷新全部”，不再显示共用介绍区或“Bot 管理”按钮。未保存内容的离开确认仍有效；单纯切换紧凑／展开布局不会离开编辑页。

右上角的 `server` 按钮在同一页面打开独立内容页，不打开新窗口；右上角原按钮变为“返回”，点击后回到进入前的入口并恢复为 `server`，内容区不另设返回按钮。键盘可用 Tab／Shift+Tab 聚焦可见入口，Enter／Space 激活，方向键及 Home／End 在可见入口之间导航。进入 server 后焦点移到返回按钮，返回后恢复到此前入口。紧凑导航切组使用横向滑动及回弹过渡，布局展开／收起保留淡入效果；减少动态效果的系统设置会关闭自动过渡，但保留手指直接拖动反馈。

### Zone 目录浏览与管理

Zone 列表显示目录名、稳定 ID 和服务器完整路径，可按名称、ID 或路径搜索；标题旁的数量为当前匹配数／登记总数。点击目录行进入只读文件浏览，点击“发起 Chat”仍以该行稳定 ID 打开创建窗口。Git 待提交数量使用蓝色纯数字角标，与领先 upstream 的数量分别显示；冲突保留警示色，悬停或辅助技术可读取完整状态，不把未知状态当作零。目录行不再另显示“浏览 →”文字，点击入口保持不变。

“登记已有目录”绑定服务器现有路径；“新建 Zone”仍受服务器创建父目录配置限制。取消登记需要二次确认，弹窗标明目标 ID 与路径；取消或按 Escape 不发送移除请求，确认后仅移除登记，不删除文件。登记与新建弹窗同样支持 Escape 关闭及焦点返回，请求进行中不能关闭。

电脑端文件浏览保留目录树／文件预览分栏，手机端保留目录与内容的单窗切换及返回按钮。进入目录从页首显示，返回列表恢复原滚动位置；搜索条件保持。文件阅读、变更审查、Commit、远端检查和 Sync 仍使用原有后端流程。

### 跨 server 使用 zone 与定义

在 zone 入口登记稳定 ID 与本机目录。推荐 ID 使用 `namespace/name`，例如 `hugosol/herdr-bot`；没有仓库来源时使用 `default/bot-playground`。命名由用户决定，系统不根据 Git remote 自动改名或克隆目录。一个 ID 在当前 server 上只绑定一个目录，不同 ID 可以共享目录。zone 登记属于各 server 的本机数据，不随 Git 共享；界面不提供重新绑定按钮，需要换目录时取消登记后重新登记。

另一台 server 使用同一张 Map 时，先准备本地目录并登记相同 ID；Map 不保存该机器的绝对执行目录。修改绑定不改变已经启动的 Run，本次 Run 继续使用启动时确定的路径；Run 历史不保证可跨 server 恢复。

直接文件输入保存 zone 内相对路径，`zone_id` 可省略，省略时使用本次 Run 的 **manager zone**，不是 node zone。需要其他目录中的文件时显式指定那个 zone ID，不通过 `..` 绕出根目录；文件必须在派发时存在，系统不自动同步。

配置提示词用 `${key}` 引用[本机提示词词典](../README.md#本机提示词词典)。保存与编辑保留占位符；系统发送配置字段时替换一次；仅当替换前的 Task 固定 prompt 含未转义且完整闭合的 `${key}` 时，才追加词典读取说明，便于 agent 直接阅读原始 Map/Bot 文件。用户输入、其他消息段、替换值、`$${key}` 和未闭合写法不触发说明注入。用户临时输入和 agent 产物中的同形字符串不替换。

### 删除 Map

在 scheduler 选中 Map 后点击“删除 Map”，在同一个弹窗中选择“删除历史”“保留历史”或“取消”。被任意 Bot 的 `map_file` 引用时，两种删除均拒绝并定位引用，不会自动解绑；Bot 定义损坏、不可读或越过链接安全边界时也拒绝，不能视为没有引用。没有引用且检查完整时，前两项都会删除 Map 定义；只有“删除历史”会同时删除关联 Run 记录，不受 Run 状态或 Chat 在线状态限制，也不会停止 Chat 或删除对话与工作文件。保留的 Run 仍可在 workflow 查看。完整边界与失败处理见[删除 Map 与历史 Run](scheduler.md#删除-map-与历史-run)。

### 节点报告策略的显示

scheduler 的 Map 图和 workflow 的 Run 图以形状区分报告策略：`auto` 为圆角矩形，`manual` 为四角切角矩形；图内不增加策略文字，选择节点后在详情查看 `report_policy: auto/manual`。Map JSON 的可选 `report_policy` 默认 `auto`。Run 中已有执行的节点使用该执行保存的策略，旧执行未保存策略时按 `auto` 显示；尚未派发的节点使用当前 Map 配置。编辑 Map 不会改变已有执行的策略。策略不新增审批按钮；`manual` 的交接确认在对应 node Chat 内以自然语言进行。

### 完成后的 Tab 清理与诊断

Map node 可选择“完成后自动关闭”；关闭会终止该 Tab 内的终端及相关进程，不删除 Chat 历史。此设置固定在每次派发的执行中，只有管理者合法完成该节点后才尽力清理。workflow 仍显示既有的 `completed` 状态，不显示关闭成功或失败，也不会为关闭弹出通知。

Tab 仍存在不代表工作未完成。缺少可靠 Tab 关联时系统直接跳过，不寻找替代目标；Herdr 命令失败、执行不可用或要求额外确认时，完成事实仍有效，后续合法业务照常推进。系统不会自动重试、强制确认或新增确认页面。请按 Run/node/执行 ID（目标已知时还含 Tab ID）在后端输出中查找关闭尝试与结果；只看到尝试不代表已关闭，失败或跳过不能当作成功。确认提示需要在 Herdr 中人工检查，不要通过重复 complete 期待补发关闭。

### 暂停或恢复 Map Run

workflow 是只读观察入口，没有暂停/恢复按钮。要暂停新派发，请进入当前 Run 已关联的管理者 Chat 表达意图，由管理者的 pause 工具落实；暂停不退出已有节点 Chat，它们仍可报告及完成交接。处理后由管理者显式 resume 恢复派发资格，再单独选择 dispatch；resume 本身不启动工作。全部节点 completed 时 paused Run 也可直接 finish。普通 Chat 的 **Resume** 仅定位/恢复原生对话，不等于恢复 Run 派发或接管重启前的 Run。

角色工具须先实际部署；没有 manager-chat/node-chat 入口时不要手填身份或编辑运行记录冒充执行。拒绝、重复调用与部分生效的解释见[暂停新执行与恢复派发资格](workflow-runtime.md#暂停新执行与恢复派发资格)。

### server 固定操作

在 `server` 内容页点击“启动”或“停止”，调用服务端预先配置的脚本；页面仅反馈脚本调用，
即使退出码为 0，也不代表后台程序已经启动、就绪或彻底停止。请求未结束时两按钮禁用，
结果区分别说明完成、失败、忙碌拒绝或结果无法确认，不自动重试或排队补做。
切回 tab 再进入 server 仍可查看本页最近一次调用的反馈。
手动关机入口也位于 server 内容页，保留确认流程；返回 tab 不是取消关机，
全局关机提醒在各 tab 和 server 页仍可见，取消必须显式操作并等待服务端确认。

## Suite 管理与权威定义

Suite 页面统一展示全局只读的 **Task 固定扩展**，各 Suite 的扩展则按宿主逐行配置入口路径，不提供可选扩展勾选目录。固定区按 OMP／Pi 分组，展示名称、当前解析后的入口路径和适用执行角色；无需先选择某个 Suite 即可查看。Task 编辑页不再重复展示固定列表，仍保留 Suite 选择器。这只是展示位置迁移：未选择 Suite 的 Task 仍按既有宿主／角色规则加载固定扩展，固定区不是某个 Suite 的专属配置，也不表示扩展已运行或连接成功。

`connect-bot` 已加入两宿主的固定扩展，分别使用 `${roots.toolkit}/entrypoints/omp-auto/connect-bot.ts` 和 `${roots.toolkit}/entrypoints/pi-auto/connect-bot.ts`；不再是 Suite 可选项。它与其他无角色限定的固定扩展一样适用于普通独立 Chat、manager 与 workflow node，不新增角色例外。auto 入口在新进程第一次主会话启动时异步尝试连接登记，收到有效回执才是已连接；可用 `/connect-bot off` 关闭，失败不会自动重试，新进程恢复不继承旧连接状态。不要同时加载 manual 和 auto 两个入口。

桌面采用工具集列表与编辑区双栏，列表显示名称、稳定 ID、适用宿主以及实际 Skills／MCP 数量；手机采用紧凑列表与单栏表单。编辑内容按基本信息、资源路径、语言服务和集中 MCP 分组，保存与关闭操作在编辑区顶部并随滚动保持可见；删除位于表单末尾，仍需确认。手机可通过左上角布局按钮收起侧栏，为路径和 JSON 编辑留出宽度。未选择工具集时显示编辑指引，读取失败时保留重新读取入口。

在 `suite` 入口创建、列表查看、编辑或删除场景工具集。Suite ID 由作者指定，使用英文字母、数字、`-`、`_`；保存后不能改 ID。适用宿主至少选一个。Skills 和 Prompt templates 按行声明具体资源位置；Extensions 按 OMP／Pi 在文本框逐行填写入口路径，不扫描整个来源目录或导入默认集合；LSP 使用服务名到原生配置对象的 JSON 声明。页面显示声明不表示语言服务或 MCP 已经运行，资源装配与原生消费是另外的启动步骤。

OMP 发起或以新进程恢复有 Suite 的 Chat 时，会将非空 `lsp` 声明写入本次私有生成目录的原生 `lsp.json`，通过 `--plugin-dir` 增量发现，不覆盖项目或用户的 LSP 文件。保留 OMP bundled defaults、HOME 根配置、插件、用户及项目的原生合并规则；用户／项目配置可覆盖 Suite，Suite 不是排除其他服务的白名单。Pi 跳过 LSP，不使用这一结果证明 OMP 已通过。

批准的四种服务如下（字段名采用 OMP 原生 `command`、`args`、`fileTypes`、`rootMarkers`）：

| 服务名 | command / args | fileTypes | 项目标记（任一） |
| --- | --- | --- | --- |
| `rust-analyzer` | `rust-analyzer` / `[]` | `.rs` | `Cargo.toml`, `rust-analyzer.toml` |
| `typescript-language-server` | `typescript-language-server` / `["--stdio"]` | `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs` | `package.json`, `tsconfig.json`, `jsconfig.json` |
| `jdtls` | `jdtls` / `[]` | `.java` | `pom.xml`, `build.gradle`, `build.gradle.kts`, `settings.gradle`, `.project` |
| `pyright` | `pyright-langserver` / `["--stdio"]` | `.py`, `.pyi` | `pyproject.toml`, `pyrightconfig.json`, `setup.py`, `setup.cfg`, `requirements.txt`, `Pipfile` |

声明本身不是已启用或运行的状态。OMP 根据项目标记、本地／PATH 可执行程序和原生优先级决定可用集合；程序缺失时不会启动。TypeScript 服务还需要兼容的 TypeScript 运行环境，Java 需要兼容 JDK 和完整 JDT LS 安装，Rustup 代理需要既有工具链组件，Python 服务需要相应 Node/Python 环境。Herdr-bot 不安装这些依赖、不修改系统 PATH、不把缺失或初始化失败显示为运行成功。在 Chat 中调用原生 `xd://lsp` 的 `capabilities` 和 `status` 观察初始化及 `ready`；`configured, not started` 仅表示可发现，不能当作运行证据。缺依赖时保留阻塞和未验证结论，专项入口见[测试指引](agents/testing.md#omp-suite-四语言-lsp-真实验收)。

部署可在 `herdr-bot.toml` 指定以下位置；普通相对路径相对该配置文件，支持既有 `${roots.name}` 路径根，改路径后重启后端：

```toml
[suites]
definition_file = 'meta-data/suites.json'
mcp_file = 'meta-data/mcp.json'
generated_directory = '.data/suites'
```

`definition_file` 是唯一权威文件，界面直接读写它，不读取生成目录中的同名文件，不建立数据库副本。缺少文件表示没有 Suite，首次保存会创建父目录；文件损坏或不可读明确报错，不覆盖成空清单。外部编辑后点击“重新读取 Suite”（有未保存编辑时先确认丢弃），或重启程序；两种方式都从该文件取得定义。保存不会主动重配已有 Chat。

一个支持两个宿主的独立声明示例：

```json
{
  "suites": [{
    "id": "review-tools",
    "name": "评审工具",
    "agents": ["omp", "pi"],
    "skills": {
      "omp": ["${roots.skills}"],
      "pi": ["${roots.skills}/code-review"]
    },
    "extensions": {
      "omp": ["${roots.toolkit}/entrypoints/omp-auto/codebase-tools.ts"],
      "pi": ["${roots.toolkit}/entrypoints/pi-auto/codebase-tools.ts", "${roots.pi_extensions}/pi-web-access/index.ts", "../extensions/review.ts"]
    },
    "prompts": ["../prompts/review.md"],
    "lsp": {},
    "mcp": ["catalog-search"],
    "pi": {"codemode": "on"}
  }]
}
```

资源字符串保留根引用与声明相对路径，其基准是 **Suite 声明文件** 所在目录，不是 Chat 或后端的 cwd。资源实体继续在原位置维护，本编辑器不复制源码、认证、模型或历史配置。

Suite 扩展不维护可选目录，也不提供勾选项。OMP Extensions／PI Extensions 每行填写一个入口，保存到 `extensions` 的对应宿主数组。`tasks.metadata_file` 中的扩展声明只用于固定环境；固定项仍在 Suite 页只读展示，不写入 Suite。

新增扩展时，将源码放在稳定目录，按该项目要求安装依赖（例如在包根运行 `npm install`），必要时先构建，再把兼容宿主的入口加入 Suite 并保存。不需要修改 herdr-bot 代码或新增可选 TOML 条目；启动不自动安装或更新依赖。独立 Skills、Prompt templates 仍需分别声明，扩展自身注入的提示随扩展运行。

每次发起或需要新进程的来源 Task 会话恢复，重新读取 Suite 中当前宿主的入口路径。`${roots.*}` 使用根配置；普通相对入口以 **Suite 声明文件** 所在目录为基准，不随 Chat cwd 改变。路径修改只影响后续新进程；已有进程不热切换。不同 Suite 的路径列表独立保存；保存时去重字面路径，启动时对同一实际目标再去重。

Suite 的 `extensions` 是按宿主分开的入口路径对象（例如 `{"omp":["entry.ts"],"pi":["pi-entry.ts"]}`），不接受旧数组或 `extensionOptions` ID 字段，也不把字符串猜作目录项 ID。启动只加载当前宿主声明的入口，直接把原始绝对路径传给宿主 `--extension`，与 Task 固定扩展方式一致。Pi 必须声明具体文件；OMP 仍可使用原生支持的文件或目录。herdr-bot 不展开扩展的 `package.json`，不生成扩展链接、包副本或扩展清单，也不递归预检 `node_modules`。模块导入与资产读取由原生宿主在源目录完成，固定扩展仍按既有执行角色规则单独加载。

每次启动检查所选扩展入口可读性；入口缺失或不可读会拒绝本次 Suite 启动，不沿用恢复流程中跳过单个旧固定扩展的降级行为。包内依赖与资产不在启动前递归预检，缺失模块或运行期资产错误由原生宿主报告。扩展与技能均从原路径加载，不需要创建目录符号链接；程序不会提权、修改开发者模式或以 junction／复制替代资源。生成目录只保存仍需要生成的宿主配置，不保存资源源码副本。

### 按宿主选择技能源路径

`skills` 是宿主键对象，不接受旧数组。`skills.omp` 的每项是技能**集合目录**：仅发现该目录直接子目录中的 `SKILL.md`，不递归，也不加载集合目录自身的 `SKILL.md`。多个集合目录共同构成所选范围，后续发现会包含其中新增的技能。`skills.pi` 的每项则是一个包含 `SKILL.md` 的技能目录。路径均相对 Suite 声明文件解析，并支持根引用。

`SKILL.md` 使用原生 YAML frontmatter；`description` 必须非空，省略 `name` 时使用来源目录名。同一真实资源重复引用去重；不同实体声明同名时明确报错并指出来源，不按目录顺序覆盖或自动改名。指定目录缺失或不可读时失败，不回退到其他来源。

发起前检查所选技能及配套资源可读，不生成技能链接目录。每次启动或需要新进程的来源 Task 会话恢复，重新读取当前集合，按 canonical 路径去重，并由本次有效直接子技能的最终名称生成 `includeSkills` 白名单；不同实体同名时拒绝启动，不能因其中一个未启用而绕过冲突检查。集合新增或移除有效技能会改变下一次启动的白名单。

OMP 的 `omp-skills.json` 使用现有全局安装已支持的原生 `--config`，只对 `skills` 做高优先级局部覆盖：直接传入多个源集合目录，设置 `includeSkills`、`ignoredSkills: []`，将已有七个来源开关 `enableCodexUser`、`enableClaudeUser`、`enableClaudeProject`、`enablePiUser`、`enablePiProject`、`enableAgentsUser`、`enableAgentsProject` 均设为 `false`，并按有效名称是否非空设置 `enabled`。不修改 OMP 源码、全局安装或用户／项目配置，也不要求部署新的宿主构建。该覆盖约束的是**原生最终可用技能列表**，不承诺禁止默认目录扫描；默认来源中不在白名单的技能不得成为可用技能，实际名称及来源仍需原生验收核对。原本属于默认来源的目录若被显式指定，仍可加载。

技能列表隔离不改变原生扩展、agent、MCP、LSP 或项目指令，不是文件访问沙箱，也不复制认证或改变模型／会话位置。明确选择空集合或没有有效技能的 Suite 使用空白名单与 `enabled: false`，不回退；**不选 Suite** 则不传技能覆盖，保留原生技能行为。正文及资源按原路径读取；已有会话中的历史正文不会被追溯替换，运行中的进程不保证自动重发现元数据。

生成位置默认 `.data/suites`，相对服务 TOML，可使用 `${roots.name}`；交付配置使用 `${roots.suite_resources}`。每次入口只有在资源检查和配置生成完整后才交给 Herdr，不原地改写已经交出的清单，不持有文件协调锁执行 Herdr。生成目录不是 Suite 权威副本、资源源码或认证目录，不要把它作为手工维护入口。源技能、资源或生成位置不可读／不可写时，公开启动返回 `400 task-suite-invalid` 及具体 Suite／路径原因；不创建 Tab，不以空环境启动，不自动提权、启用开发者模式或复制／junction 替代。清理失败另行保留在错误说明中。

同一次启动的技能及适用宿主的 LSP／MCP／Pi 配置共用独占新建的 `<suite-id>/<本次装配>` 目录，全部写好才发布启动参数。同 Suite 或不同 Suite 的并发启动各用自己的完整目录，不会改写或清理已经交付给另一启动的产物；协调范围在进入 Herdr 前结束。资源检查失败不创建半套输出；创建后失败仅删除本次确实新建的配置目录，不删除技能源路径，也不扫描删除生成根中的未拥有文件。清理失败会与原始失败一起反馈，不新增跨文件事务或跨进程写入协议。

成功交付的装配目录可能仍被已有进程消费，因此不会在下次装配时自动回收。需要手工重建生成目录时，先停止所有使用它的 Chat 进程，且只移除确认属于程序的装配子目录，不删除 Suite 权威定义或资源源目录。受控装配不会放宽 zone 浏览与普通持久化对任意重解析点的拒绝。普通权限下的原路径资源加载和失败清理入口见[装配安全验收](agents/testing.md#suite-装配发布与链接安全验收)。

真实消费专项入口见[测试指引](agents/testing.md#omp-suite-技能真实消费验收)。它使用真实 Herdr 与已安装 OMP 的交付启动入口；常规后端替身检查不能代替真实技能消费证据。

### Pi 原生启动资源

Pi 的共享 Suite 使用原生 `--no-skills` 和逐项 `--skill`，不关闭 builtin MCP。技能参数直接使用 `skills.pi` 中的原目录，不创建链接；明确空 Suite 与不选 Suite 不同，后者不添加技能／模板覆盖。Pi 的 `extensions.pi` 必须填写具体入口文件；后端逐个传原始源码绝对路径 `--extension`，不解析包的 `pi.extensions`，也不创建包链接。依赖和资产留在原目录，由 Pi 原生加载。Pi 本地模板通过原生 `--prompt-template` 逐项选择，初始化不展开；LSP 不传给 Pi。

交付目录的 `pi-suite` 是固定扩展，无须 Suite 重复声明。`--no-extensions` 同时关闭 builtin 自动发现，因此发起／恢复路径显式保留 `builtin:mcp` 与 `builtin:tool-search`；无 Suite 时还保留 `builtin:codemode`，有 Suite 时由 pi-suite 使用本次明确模式，避免两个 codemode 实例。配置产物为 `generated_directory/<suite-id>/<本次装配>/pi-suite.json`，通过 toolkit **公开工厂期 `--pi-suite-config <绝对路径>`** 通道交付；部署必须包含支持该 flag 的 toolkit，旧的仅环境变量通道不能由 Herdr 的 `agent start` 参数自动替代。配置只含版本、模式和本次引用解析出的 `mcpServers`，不会写全局模式、项目 `mcp.json` 或移动认证、模型、会话存储。MCP 的 `cwd`、带根引用或 `./`／`../` 的 command 以集中 MCP 文件目录解析，裸 command 保持 PATH 查找，args/env/url 保持原生字面语义。

开发真实烟测见[测试指引](agents/testing.md#pi-suite-启动开发烟测)。它不替代 Pi 完整资源／MCP／模式／模板契约矩阵；缺宿主能力或尚未交付的 toolkit flag 时应明确记为未验证，不能以参数或产物存在宣称原生消费通过。

### 宿主适用配置与集中 MCP

Pi 适用的 Suite 必须明确保存 `pi.codemode: "on"` 或 `"only"`。勾选 **Codemode only** 保存 `only`，取消保存明确的 `on`，不表示关闭 codemode 或继承全局 only。仅 OMP 的编辑不显示 Pi 控件，也不保存 `pi` 字段；重新勾选 Pi 从 `on` 开始。Pi 跳过 LSP，OMP 跳过 Suite 本地 prompt templates，不将它们变为初始消息或系统指令。

MCP 连接仍由维护者在集中文件编辑，界面只选择 ID，不新增 MCP 编辑平台或进程 manager。例如 `mcp_file` 内容：

```json
{"mcpServers":{"catalog-search":{"command":"search-server","args":["--stdio"]}}}
```

Suite 的 `mcp` 只保存引用，不保存连接配置副本；空数组表示不追加，不禁用宿主原生 MCP。定义页只读此集中目录，不执行其中的命令。保存 Suite 会重新读取集中目录并拒绝不存在的 MCP ID。外部删除 MCP 定义后，重新读取的 Suite 编辑器仍保留已选 ID，并标出“失效”；不会静默丢弃引用。只有显式取消该失效项或恢复集中定义后才能保存。

### OMP 原生增量 MCP

选择 Suite 发起或新进程恢复 Chat 时，后端把引用的集中定义发布为只含 `.mcp.json` 的独占资源包，通过原生 `--extension` 追加给 OMP。不向项目写入配置，不替换整个 agent 配置目录，也不建立另一套 MCP manager；连接、工具、资源列举／读取、服务器 prompts、通知刷新、instructions 与 `/mcp` 管理交互均由原生 OMP 提供。Suite 本地 prompt templates 的省略不影响 MCP 服务器 prompts。

MCP 的 `command` 支持 `${roots.name}`；`./`、`../` 等路径式命令及 `cwd` 相对 **集中 MCP 文件** 所在目录解析，不随 Chat cwd 或生成位置改变。裸命令仍由 PATH 查找，其余原生连接字段原样交给 OMP；命令参数仍遵循服务器／宿主语义，不逐个猜测为文件路径。

不读取或合并用户／项目 MCP 配置；同名胜者、异名等价连接去重、不同参数的独立连接、项目 `enabled: false` 及用户 `disabledServers` 都遵循 OMP 原生规则。合法的覆盖或禁用不是失效引用，不强制 Suite 胜出或重启服务器。空 MCP 清单不生成额外包，用户与项目原生服务器照常可用。

配置引用或资源发布失败拒绝本次启动；有效但不可达的服务器由 OMP 显示连接中／未连接等原生状态。Herdr 接受 Chat 不表示 MCP 已可用，不增加“所有服务器就绪”门禁。实际交付入口的专项自动验收见[原生 MCP 测试指引](agents/testing.md#omp-suite-原生-mcp-真实消费验收)。

### Task 单 Suite 选择与引用保护

Task 编辑第二步提供 **Suite（至多一个）** 单选，只提供当前 agent 兼容的 Suite 和“不选 Suite”。多个 Task 可以引用同一个 Suite；引用保存为 Task 的可选 `suite` 字符串（稳定 Suite ID），不复制 Suite 内容。`null` 或省略表示明确不选，数组、多选、不存在的 ID、空字符串或不兼容的选择会被直接 HTTP 拒绝。

- Task 列表与编辑器重新读取当前权威定义和集中 MCP 目录；失效引用仍显示原 ID 及原因。切换 agent 不会偷偷清空或改选 Suite，不兼容的原选择保留为不可用项，必须显式修正。
- Task 保存及发起 Chat 前均重新验证 Suite 存在、宿主兼容和集中 MCP 引用。发起后新进程恢复与 workflow manager/node 启动也验证来源 Task 的 Suite 引用。验证失败会明确拒绝，不以空环境继续，不删除引用；原生 MCP 的运行期连接状态仍由宿主处理，不新增“所有 MCP 已就绪”门禁。
- Suite 不替换 Task 的初始 Skill、固定 prompt、模型、思考级别或命名规则。修改定义不会主动重配已运行的 Chat。
- 删除仍被引用的 Suite 返回 `409 suite-in-use`，界面列出引用它的 Task 名称与稳定 ID。先显式编辑或删除这些 Task，再删除 Suite；拒绝后 Suite 和所有 Task 引用保持不变。读取 Task 失败时也不会冒险删除 Suite。
- 同一进程内新增／修改 Task 引用与删除 Suite 共用既有文件协调范围，不能同时成功留下悬空引用。手工编辑在下一次读取／保存／启动时重新检查；不承诺跨文件事务或多进程写入协议。

`GET /api/tasks` 的 `options.suites` 提供当前 Suite 定义，`options.mcp` 提供集中 ID 清单；Task POST/PUT 的 `suite` 字段与读取字段一致。引用校验失败返回 `400 task-suite-invalid` 和具体原因。读取失效 Task 本身仍成功，以便保留并显式修正引用。

公开 HTTP 入口：`GET /api/suites` 返回 `{suites, mcp, fixedExtensions}`（`mcp` 为集中 ID 清单，`fixedExtensions` 为只读固定目录，条目含 `id`、`label`、`agent`、`kind: "fixed"`、解析后的 `entrypoint` 和 `roles`，角色值为 `standalone`／`manager`／`node`）；`POST /api/suites` 创建；`GET /api/suites/{id}` 返回 `{suite}`；`PUT /api/suites/{id}` 修改同一 ID；`DELETE /api/suites/{id}` 删除。创建、更新与删除成功均返回相同的 `{suites, mcp, fixedExtensions}` 列表形状。保存请求体使用上例的单个 Suite 对象，不接受查询参数或未知字段，删除不接受请求体。错误以 JSON `code` / `message` 返回；只有成功写入权威文件才显示已保存。

定义管理的可复跑隔离验收为 `cargo test --manifest-path backend/Cargo.toml --test suites` 与 `npm --prefix frontend run test:browser:build -- suites.spec.ts --workers=1`。Task 引用完整性验收为 `cargo test --manifest-path backend/Cargo.toml --test suite_references` 与 `npm --prefix frontend run test:browser:build -- suiteReferences.spec.ts --workers=1`，覆盖单选及复用、集中引用失效后的读回／保存／启动拒绝、删除保护及受控并发请求；启动负例使用独占 Herdr 记录夹具确认没有外部启动调用，不宣称原生资源消费通过。浏览器用例独占临时配置、声明、动态回环端口和后端进程，真实操作页面并独立读取文件；包含外部修改及实际进程重启，不需要用户凭据或原生 agent 启动。它们证明定义管理和模式持久化，不证明原生 agent 已消费资源。

### 首批定义与显式离线迁移

交付的 `meta-data/suites.json` 和 `meta-data/mcp.json` 是当前默认权威定义，`meta-data/tasks.json` 是按批准稳定 ID 更新的源码预设。探索 Suite 支持 OMP/Pi、Pi 明确 `on`；OMP 选择声明的技能集合目录，Pi 保留逐技能选择。实现 Suite 仅 OMP，保留 `oh-my-wayfinder` 并加载所声明集合目录中的直接子技能。两者引用集中 `codebase-memory-mcp` 和批准的四语言声明；OMP 集合中新技能会参与后续发现，不再承诺旧的固定 18／11 技能数量。当前另提供仅支持 Pi 的「网页访问」Suite（`web-access`），Tibo-monitor（`tpl-18e0ceac0a89f9bc-0`）已选择它；探索 Suite 的 Pi 分支也选择同一个联网扩展；已有 Suite 扩展均直接声明入口路径，OMP 分支与实现 Suite 的能力选择保持不变。实际资源发现与命令消费仍需对应宿主验收，不能从声明或迁移成功推断。

`pi-web-access` 不是某个 Suite 专属的开关。「网页访问」是该能力的独立打包；探索 Suite 的 Pi 分支在原有资源上整合同一能力。两者的 `extensions.pi` 都声明 `${roots.pi_extensions}/pi-web-access/index.ts`，不从包 manifest 推导。以后支持 Pi 的 Suite 可直接添加或删除此入口，无需目录登记或勾选。依赖须事先准备，Chat 启动不执行 npm 安装或更新。独立网页访问 Suite 使用 `pi.codemode: "on"`，Skills、Prompt templates、LSP 与集中 MCP 清单均为空；关闭 Pi 默认技能和模板发现，但保留固定扩展及原生 MCP 合并行为，不替换 Task 模型、思考级别或初始提示词。

`pi-web-access` 源码包当前版本 `0.41.0` 没有声明独立 skills、prompt templates 或 themes；工具指引和内部模型调用提示词由扩展自身提供，不复制到 Suite 模板或 Task 初始消息中。接入不修改扩展的 `auto` 工具激活方式、功能开关或 provider 配置；扩展已注册不等于全部工具立即 active，也不等于搜索服务或凭据可用。源码包入口与 npm 发布包的 `./dist` 目录声明不同，不能直接互换。该扩展直接加载不需要符号链接权限；探索 Suite 中的非空技能同样使用原路径，不需要创建符号链接；修改后只影响后续发起及需要新进程的来源 Task 会话恢复，不重配运行中的 Chat。

已有部署的数据**不会在启动时自动套用**。旧 `skills: [...]` 须在升级前改为宿主键对象：Pi 将原技能目录列表放入 `skills.pi`；OMP 将原技能目录的父集合目录去重后放入 `skills.omp`，并确认接受集合内所有直接子技能参与发现。不要把单技能目录原样放入 OMP 集合列表。API 不保留旧数组兼容分支；OMP 技能覆盖复用现有安装的 `--config` 能力，无需修改或升级宿主。旧 `extensionOptions` 须在升级前显式迁移：按旧 TOML 将每个 ID 转为对应宿主的入口，合并到 `extensions` 并删除 ID 字段；根引用可以保留，TOML 相对路径必须换算为 Suite 相对路径或绝对路径。未知 ID 不可猜测或丢弃，应先修复来源。已成为固定项的 `connect-bot` 不重复加入 Suite，并移除 Suite 里同扩展的 manual／auto 入口。随后移除 TOML 可选声明，仅保留固定项。交付源码定义已完成转换，其他用户部署仍须显式更新，不保留旧 ID 运行兼容分支。

已有 `extensions` 路径继续使用，不转换回 ID；Pi 旧包目录须改为具体入口文件。下面的 `initial-suite-v1` 是历史首批探索／实现迁移，不包含后续网页访问 Suite 或 Tibo-monitor 的联网绑定；不要把重跑它当作本次扩展格式更新。维护者先停掉该部署的全部后端，核对 `migrations/initial-suite-v1/mapping.json` 的 22 个稳定 ID，并取得对目标数据的明确操作授权，再运行专项命令；以下路径仅为占位示例，不是已批准操作用户数据：

```powershell
cargo run --manifest-path backend/Cargo.toml --bin migrate-initial-suites -- `
  'D:/authorized-deployment/meta-data/tasks.json' `
  'D:/authorized-deployment/meta-data/suites.json' `
  'D:/authorized-deployment/meta-data/mcp.json' `
  'D:/authorized-deployment/.data/initial-suite-v1' `
  'C:/release/herdr-bot/migrations/initial-suite-v1'
```

五个参数分别为 Task 权威文件、Suite 权威文件、集中 MCP 文件、本次迁移独占状态目录、批准的只读发布输入目录。后者包含 `mapping.json`、`suites.json`、`mcp.json`；它们只是显式迁移输入，不是运行时回退副本。不要让不同部署共享状态目录，也不要从源码默认定义覆盖已有部署的权威文件。

命令按稳定 ID 核对每条当前 Task 的 agent 和旧扩展选择，不按显示名匹配。当前已有 Suite 字段、环境已改、缺少／重复 ID、未映射的旧格式 Task，或同 ID 的现有 Suite/MCP 与发布输入不同，都明确报冲突，不猜测覆盖。Task 的名称、trigger、类型和身份原样保留；Bot、Chat 历史、Run/node 文件不会被打开。首次操作前保留精确 `plan.json`（含原始字节），逐文件原子发布并独立读回，只有全部读回成功才生成 `complete.json`。中断或后项失败保留前项已生效文件；重新执行同命令按原计划继续，遇新编辑即停，不新增事务、自动重试或回滚。完成后的重复执行不读写用户新编辑，不重复生成定义。状态文件含 Task 原文，应按部署数据保护；冲突需要维护者明确协调，不要直接删状态文件来强制重跑。

自动验收只操作测试创建的临时数据：`cargo test --manifest-path backend/Cargo.toml --test initial_suite_migration` 覆盖 17/1/4 独立映射、保留 trigger/引用身份、真实进程中断、Windows 写入失败、冲突、重启及完成标记。测试或发布工单本身不授权操作任何用户生产 Task。

## Task 模型与删除

Task 页以紧凑清单展示名称、用途、Agent、模型／思考级别与 Suite。搜索支持名称、Agent、类型、模型、Skill 和 Suite 名称／ID，可与 Agent 筛选组合；计数显示当前匹配数与总数，清除筛选不会修改任何 Task。展开“查看配置与提示词”可读取命名示例、Task 设置与完整 prompt；固定扩展统一在 Suite 页面查看，失效 Suite 的错误不必展开即可看到。

新增与编辑仍经过“Task 预设 → 扩展配置 → 确认保存”三步，只有明确保存并收到后端成功响应才更新共享定义。编辑器自动聚焦显示名、限制键盘焦点在弹窗内，未保存时按 Escape 等同取消，关闭后焦点回到原入口；保存期间不能取消。底部操作保持可达。Task、Suite 与其他入口共用统一导航：`layout=expanded` 保持侧栏，切换页面不改变其他 Tab 的字号、图标边框或导航布局；手机上也可使用全局布局按钮切换至紧凑导航。

Task 的 Codex 模型选项包含 `GPT-6.1-sol`、`GPT-6-luna` 和 `GPT-6-astra`；
思考级别按所选 agent 与模型的能力提供，Pi 的 Luna 支持 `off`，Astra 不支持。
模型目录修改后下次打开编辑、保存或发起 Chat 时重新读取，无需重启。
删除 Task 时先询问是否删除，取消不发出删除请求；删除不影响已经发起的 Chat。

## Task 类型与固定角色扩展

在 task 入口新增或编辑 Task 时，第一步选择普通（`normal`，新建默认）或流程管理者（`manager`）。两种用途均可选择 omp 或 pi；切换用途不改变已选择的合法 agent、模型、思考级别及 Suite 选择。普通 Task 既可从 zone 独立发起 Chat，也可经 node → Bot → Task 用于节点执行，无需复制成节点专用 Task。manager Task 仍专用于管理者配置，不能绑定给节点执行。

角色扩展的适用规则由代码固定：独立发起普通 Task 不加载 manager-chat/node-chat，manager Task 加载 manager-chat；workflow 派发普通 Task 时强制加载对应宿主的 node-chat。角色扩展不能勾选、取消或混装；Task 编辑器不展示节点角色固定项。其他固定项仍沿用 Task 选项目录；场景资源只由 `suite` 引用提供。Task 不再接受、保存或运行旧 `extensions` 选择，空数组同样被拒绝。

扩展入口位置仍位于 `meta-data/task-options.toml`；代码按稳定 ID `omp-manager-chat`、`pi-manager-chat`、`omp-node-chat`、`pi-node-chat` 识别固定角色，元数据不再支持 `taskType` 适用规则。实际工具、Prompt 和原生扩展运行能力须由 my-agent-toolkit 及宿主环境提供；配置保存或页面展示成功不代表原生加载已验证。缺失或不可用入口在发起时明确报错；Resume 沿既有规则报告扩展降级，不伪称已加载。

需要新进程的 Resume 仍读取来源 Task 的最新会话环境，但从持久 Run 中的原 Chat 执行关联恢复节点角色，包含 retry 前保留的旧执行；不新建执行、不重放 trigger config。独立普通 Chat 不因此获得节点角色。本次不处理运行期间动态修改来源 Task 配置的冲突，也不引入配置快照或热更新协调。

### 数据格式要求

Suite 专项离线迁移不会补齐更早的数据类型升级；请先使用当前 Task 类型格式。Task 必须显式包含 `taskType: "normal"` 或 `"manager"`；不接受旧 `node` 类型，也不在运行时自动转换或补类型，缺少类型的 Task 文件会报 `tasks-corrupt`。

使用自定义 `tasks.metadata_file` 时，扩展条目不应包含 `taskType` 适用配置；固定角色保留上述稳定角色 ID、对应 agent、`kind = 'fixed'` 及真实入口路径。节点执行身份与既有 Chat 的恢复语义不因 Task 分类而改变。

## Task 配置与原生存储

Task 预设默认保存于 `meta-data/tasks.json`，选项目录仍为 `meta-data/task-options.toml`；均可通过配置指定其他路径。Task、Bot、Map 作为定义数据允许纳入 Git，页面编辑会修改这些文件；旧数据切换与冲突处理见 [定义布局升级](../README.md#从旧定义布局升级)。

Task 配置分为两组：

- **trigger config（发起配置）**：初始消息、初始 Skill 调用、model 与 thinking，仅在发起 Chat 时应用。
- **env config（会话环境配置）**：Suite 引用及固定扩展；发起 Chat 时加载，Resume 需要新进程时也加载。

初始 Skill 调用是随初始消息提交的动作，不是加载型 Skill；Task 固定 prompt 与本次 input 都属于用户输入，不是系统提示词注入。Task 自身不单独选择加载型 Skill 或系统提示词；加载型资源由所选 Suite 声明提供。

原生会话文件和附属记录由 agent 管理，herdr-bot 不搬动文件或修改 agent 原生配置。发起 Chat 不保证新建原生对话；agent 自身的自动续接设置不属于 herdr-bot 的功能。

## 历史 Chat Resume

在 `chat` 的历史视图中使用 `Resume` 返回 Pi／OMP 原生对话，不要求该 Chat 仍在当前登记中。已有目标只做 Herdr 内部导航；需要新进程时，加载同一原生对话，不自动继续中断任务，也不重放 Task 的发起配置或发送新 prompt。Herdr 内部导航不保证 Windows 窗口置前。

新启动的 omp / pi 会话可能尚未写入原生文件；pi 在首次用户或助手消息出现后才创建文件。herdr-bot 可按[受限原生路径规则](adr/0014-unpersisted-omp-startup-identity.md)保存身份及预期路径，但这不代表文件已落盘或当前可从磁盘恢复。历史文件中的内部 `startupIdentityPath` 仅由可信发起流程写入，普通 register 上报不能设置或覆盖；历史列表不公开该字段。Resume 只对保存的 ID、启动路径与当前官方引用完全吻合的未落盘候选允许定位，不把任意缺失文件当作在线 Chat。实际读取对应文件时若发现身份冲突，明确拒绝，不跳过冲突后启动替代 Chat、不改绑历史；不新增后台落盘检查。原进程已不存在时仍按既有精确 ID 加载及恢复确认流程处理，未确认同一会话不报告成功。

新进程优先使用仍然有效的历史工作目录。目录缺失、已删除或不可用时，只有 Herdr Workspace 的 worktree 元数据能够精确关联历史 checkout 路径，且通过元数据中的共享 Git 目录查询 worktree 信息，能够核实唯一且可用的主仓库根目录，才回退到该主目录；不会从目录名称推测关联。无法可靠证明主目录时，在配置的默认 Herdr Workspace 中新建 Tab，不传 `--cwd`，由 Herdr 选择实际工作目录。默认 Workspace 按配置名称唯一匹配，不采用当前焦点或列表中的第一个 Workspace；无法唯一找到时明确失败。主目录回退仍优先使用原 Workspace，原 Workspace 不可用时才使用默认 Workspace；交给 Herdr 选目录时则直接使用配置的默认 Workspace。

恢复成功前必须核实同一原生会话身份。显式指定目录时，还必须确认实际目录与指定目录相同；交给 Herdr 选目录时，必须观察到有效的实际目录。位置回退与实际工作目录会在页面显示，身份或目录核实失败不会报告成功。Resume 不执行代码 checkout，不弹出目录回退确认，也不改写 herdr-bot 历史记录（包括旧 cwd、Workspace、备忘与来源信息）；回退只影响本次恢复。herdr-bot 不直接改写原生聊天文件，agent 自身恢复时的原生存储行为仍由 agent 管理。

新进程使用来源 Task 的最新环境配置，而不是创建时的快照；没有来源 Task 或来源已删除时，使用 agent 的原生默认扩展配置。修改 Task 不会重新配置已经运行的 Chat。恢复中无法加载的扩展或位置回退会在页面提示。

历史记录支持多行纯文本备忘，电脑与手机共享；备忘不会发送给 agent 或修改原生聊天记录。

若页面显示“启动结果未确认”，外部目标仍可能已经启动。先检查页面提供的目标信息，不要把超时当作未执行；服务不会自动清理或重试。
