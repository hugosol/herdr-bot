# scheduler：可复制的静态定义样例

这些是**人工编写的业务能力示例，不是已实现执行的证明**。天气、额度与清单都不会被查询、计算或生成；保存定义不启动 Chat。Map 与 node 是静态工作定义，不是运行状态或执行实例。共享 `Bot` 描述业务能力，Task 则是发起 Chat 的可复用预设，两者不同。

## 文件与事先规定的核对结果

- `maps/trip-readiness.json`：A「上海出行天气」、B「北京出行天气」并行，**仅 A → C、B → C** 两条边，C「汇总出行准备」汇合，D「独立额度估算」无连接。
- `maps/another-trip.json`：独立 E「广州出行天气」。
- `bots/weather-capability.json`：身份为 `destination-weather`，被 A、B、E 跨 node／跨 Map 复用；**文件名故意不同于 id**。
- `bots/summary-capability.json`：身份为 `trip-summary`，用于 C。
- `bots/quota-capability.json`：身份为 `quota-estimate`，用于 D。

|node|input 原文（换行分隔）|前置|
|---|---|---|
|A|城市：上海；日期：2026-11-01；偏好：步行；带伞。|无|
|B|城市：北京；日期：2026-11-02|无|
|C|范围：上海与北京的出行准备；核对主题：天气、随身物品。材料位置由管理者明确提供。|A、B|
|D|账户：人工示例账户；人工观察的剩余额度：25%；观察时间：未知。|无|
|E|城市：广州；日期：2026-11-03|无|

每项 zone 都是 `C:/example/travel-not-registered`，三份 Bot 均暂未绑定 Task。这些内部合法但外部未完成的定义可保存，不制造默认绑定，不能据此启动。A、B 的不同 input 不会被 Bot 文档或彼此内容覆盖。C 的边不会自动传入 A、B 材料。

## 在隔离目录运行

以下 PowerShell 从**仓库根目录**执行；先按根 [README](../../README.md#构建与运行) 安装依赖。所有可变数据均写入新建临时目录，不使用用户的 Task、zone、Map 或 Bot。不将样例自动安装到用户数据。

先构建页面和现有安全关机壳；它与交付程序使用同一应用及读取接口，但不会真的关机。固定操作使用已有无害夹具。此页面仅供本机体验，不进行 LAN／mDNS 验收，也不要从此页面发起 Chat。

```powershell
npm --prefix frontend run build
cargo build --manifest-path backend/Cargo.toml --bins --example safe_shutdown_shell
```

准备与复制（`$demo` 每次均为新目录，配置文件故意不在 cwd 中）：

```powershell
$repo = (Get-Location).Path
$demo = Join-Path ([IO.Path]::GetTempPath()) ('herdr-map-preview-' + [guid]::NewGuid())
$work = Join-Path $demo 'process-cwd'
New-Item -ItemType Directory -Path $work | Out-Null
New-Item -ItemType Directory -Path (Join-Path $work 'plans'), (Join-Path $work 'capabilities') | Out-Null
Copy-Item "$repo/docs/example/maps/*.json" (Join-Path $work 'plans')
Copy-Item "$repo/docs/example/bots/*.json" (Join-Path $work 'capabilities')
$config = Join-Path $demo 'preview.toml'
@"
listen_addr = '127.0.0.1:0'
[mdns]
enabled = false
[operations.start]
script = '$repo/fixtures/operations/success.bat'
working_directory = '$repo/fixtures/operations'
[operations.stop]
script = '$repo/fixtures/operations/success.bat'
working_directory = '$repo/fixtures/operations'
[herdr]
workspace_name = 'scheduler-preview-unused'
[tasks]
storage_file = '.data/tasks.json'
metadata_file = '$repo/meta-data/task-options.toml'
[zones]
registry_file = '.data/zones.json'
[sessions]
history_file = '.data/session-history.json'
[scheduler]
map_directory = 'plans'
bot_directory = 'capabilities'
"@ | Set-Content -Encoding utf8 $config
Write-Host "隔离目录：$demo"
Push-Location $work
try {
    & "$repo/backend/target/debug/examples/safe_shutdown_shell.exe" --config $config
} finally {
    Pop-Location
}
```

打开日志中实际打印的 `http://127.0.0.1:<动态端口>/`，保持控制台运行；Ctrl+C 结束。上面用独立安全壳，不要换成生产二进制做关机体验。清理时先结束服务，保留截图和需要的反馈后，只删除本轮打印的 `$demo` 目录。

### 依次操作

1. 初始是 `chat`；点击左上角展开导航后选择 `scheduler`，或在顶部导航条左滑到配置组再选择。列表恰有 `another-trip.json` 和 `trip-readiness.json`，不应出现三个 Bot 文件。
2. 选择 `trip-readiness.json`，默认显示节点清单与详情空态，依赖图在节点清单上方常驻展开。核对 A、B、C、D 标题与 ID，以及 C 的 A／B 前置关系。可在目录输入文件名筛选；没有匹配项时仍保留当前 Map 和 node，清空筛选恢复目录。
3. 从节点清单逐项选择 A、B、C、D，与上表核对本次输入与 zone和依赖。详情直接展示关联 Bot 的 `node_guide` 原文；前置与后继节点可点击定位，关闭详情后焦点回到节点清单。点“Bot 明细”进入共享 Bot 预览，外侧同步选中 `bot`，点外侧 `scheduler` 入口继续核对工作；不再内联展开。也可从外侧 `bot` 入口打开共享列表，再按名称与 ID 预览。在依赖图核对 A → C、B → C，D 独立；图与清单选中联动。图自动适应视口，可拖动图内空白处平移；不拖动编辑节点或创建连接。
4. A、B 进入同一个「目的地天气摘要」预览（ID 为 `destination-weather`）；每次通过外侧 `scheduler` 入口回到节点清单。分别切换 `when`、`plan_guide`、`node_guide` 阅读三份独立文档，允许任意顺序；`json` 只读展示完整 JSON。左侧列表始终保留，可按名称或 ID 筛选；筛选无匹配时右侧仍保留已选 Bot。C 为「出行准备清单汇总」，D 为「额度恢复时间估算」。逐项文本以交付 Bot JSON 为准；它们都是预期说明，不是实际结果。
5. 选择 `another-trip.json` 与 E，核对广州、日期和同一「目的地天气摘要」。可以在隔离 capabilities 内重命名 weather 文件，点击“刷新全部”，关联仍按 id 成立。
6. 在 Bot 预览点 `edit`：独立 ID 只读，JSON 只加载当前真实内容且不含顶层 id。修改 name 或 plan_guide 后显式保存，直接显示新预览，无需 `refresh`；外侧 `scheduler` 入口重读来源并保留仍存在的 node 选择。来源 node 或 Map 被外部移除则显示失效原因。所有引用同 ID 的 A、B、E 都读取同一份共享能力，不产生 Bot 副本。若用外部编辑器改文件，显式刷新可重读；刷新会清除 node 选择，依赖图仍常驻展示（Bot 页按钮为 `refresh`，Map 页为“刷新全部”）。**避免在页面提交期间直接修改相关定义**，不承诺与不参与协调的外部编辑器跨文件原子提交。
   直接点页面上方 `new`（空列表也可新增）：ID 初始为空，JSON 预填天气示例除 id 外的完整内容。该示例随应用编译打包，以本目录 weather-capability.json 为单一内容来源，不要求部署时存在开发仓库。可以删改条目但保留必需结构；打开或放弃不会写文件。页面不要求文件名，保存后自动使用安全且不覆盖既有文件的名称，编辑已有 Bot 保留原名。无效 JSON、结构错误、重复 ID 或旧版本冲突均保留原始草稿并显示原因，不跳到成功预览；需要重新加载时先复制草稿，再放弃并重开。修改后返回、切换 tab／server、刷新全部或浏览器刷新均确认放弃，取消继续编辑。
7. 在当前 Map 点“新增 node”：独立 ID 初始为空，JSON 预填本目录 trip-readiness.json 的 A（上海、2026-11-01、步行／带伞说明、示例 zone 与空 blocked_by），示例随应用打包，部署不依赖仓库。先放弃确认文件未变化，再重新打开，填入不同 ID 并实际删改示例后保存。不会自动安装 destination-weather Bot。选中已有 node 点“修改 node”，ID 只读且 JSON 只来自真实 node。改变标题、input、zone 或 blocked_by，保存后不点刷新，核对当前 Map、选中 node、列表、详情和依赖图立即更新。其他 node 的输入、zone 与依赖、Map 标题说明和其他 Map 文件不应改变（当前 Map 排版可变化）。
   node 只在所属 Map 内唯一，不同 Map 可复用同一 ID；整个 Map 的未知前置、自依赖或环均被拒绝，不自动修复。字符串 input（包括 Markdown）、未登记 zone／Task、缺失 Bot 引用仍可保存，但页面明确提示缺失 Bot。失败保留独立 ID 与原始 JSON；整个 Map 文件是冲突单位，即使只改另一 node，旧草稿也须重新加载，不自动合并。离开编辑器、选择其他 Map、Bot 管理、切换页面或刷新均沿用草稿保护。**页面提交期间不要直接修改相关定义**。
   将另一个 JSON 副本放入隔离 plans，点击“刷新全部”看到新增文件；删除该副本后刷新则移除。刷新也重新读取当前 Map 与 Bot；不自动监听文件。
8. 从 scheduler 点右上角 `server`，原页面内进入独立内容页，再点右上角原位置的“返回”。不要用新窗口或在功能分组中寻找 server。
9. 窄屏使用约 390 × 844 viewport；按文件目录 → 节点清单 → node 详情纵向访问，再进入 Bot 预览；通过外侧 `scheduler` 入口往返，图按需展开。滑动滚动页面；可用 Tab／Shift+Tab、Enter／Space 操作，图内聚焦节点会按需移入视口，顶部 tab 另支持方向键、Home／End。

### 默认目录及错误对照

两个 scheduler 字段可各自省略；默认分别为 cwd 下的 `.data/workflow-maps/` 和 `.data/bots/`。无论配置文件在哪里，相对目录都按**进程 cwd** 解析。绝对路径也可用。配置修改需重启；目录文件变化只需显式刷新或重载。

首次读取自动创建缺失的配置目录及父目录，保留已有文件，不覆盖定义，也不安装这份样例。要观察默认空态，在另一份新临时目录中按上述配置运行但删去整个 `[scheduler]` 段，不复制样例；进入 scheduler 应有明确的 Map／Bot 空态，而不是样例图。服务还会维护自己的既有持久数据；scheduler 的只读约束专指 Map／Bot 定义。

- 目录路径被普通文件占据、权限不足或无法创建／读取：明确显示目录错误，不冒充空目录。先停止服务或在隔离目录内修复，再刷新。
- Map JSON、结构或依赖无效：清除旧图与详情，显示文件及可用字段位置；仍可选择其他有效 Map。
- Bot 缺失、不可读、格式无效：保留合法 Map 图和输入并提示问题，不捏造说明或默认 Task。重复 id 列出冲突文件，不任选其一；正常 Bot 仍可查看。
- Bot 目录出错与单文件问题分开提示。修复后刷新或重载读取当前文件；具体排查见 [共享 Bot 问题排查](../scheduler.md#共享-bot-问题排查)。
- 两个目录均只扫描顶层 JSON（后缀不区分大小写），不递归；链接、目录联接和其他重解析点不能绕过读取范围。

## 受支持格式（生成者也使用同一种格式）

字段名固定为 **snake_case**；只支持新格式，拒绝全部旧字段与 schema_version（包括 1）。每张 Map、每个 Bot 独立保存；没有专供图形展示的 edges、位置或 Bot 副本。

|对象|必需字段与类型|
|---|---|
|Map|`title`、`description`: 字符串；`nodes`: Map 内 ID 到 node 的对象|
|node|`title`、`bot_id`、`input`、`zone`: 字符串；`blocked_by`: 同图前置 ID 字符串数组|
|Bot|`id`、`name`、`when`、`plan_guide`、`node_guide`: 字符串；另可省略 `task_id` 字符串引用|

node ID 是 `nodes` 的 key，共享 Bot 按 `id` 而非文件名关联。Bot 只引用 Task，不复制 Task 配置或 Chat；静态定义不保存执行状态、报告和运行历史。`zone` 是登记目录的路径引用，不是稳定 ID。

三份 Bot 文档分别说明选用场景、规划安排与节点工作，可任意顺序独立读取、独立保存空文本；Markdown 不是结构校验或业务门禁。`input` 是原文字符串，不做 JSON 解析、变量替换或文档自动注入。文中路径和指令只作文本展示，不读取或执行。

结构检查拒绝未知字段、错误类型、缺必需字段及非法依赖，拒绝前保留原定义。`nodes: {}`、`blocked_by: []`、字符串 `""` 合法，必需字段不能缺省或为 null。Bot 可以暂缺 task_id，也可保存尚不存在的 Task；node 可引用缺失 Bot、未登记或失效的 zone，不补默认绑定。完整职责与接口见 [scheduler 参考](../scheduler.md#定义职责与唯一格式)。

## 体验材料与保留项（P33）

浏览器验收 `frontend/browser-tests/schedulerExamples.spec.ts` 直接复制本目录文件，经 S1 真实主界面核对上表、全部 Bot 明细、跨 Map 复用、重载、刷新和 server 返回；不使用专供文档的读取接口。项目完整回归入口见 [测试指引](../agents/testing.md)。

格式切换前的历史截图（不是当前格式证据）：[桌面图](screenshots/desktop-graph.png)、[桌面明细](screenshots/desktop-details.png)、[窄屏图](screenshots/narrow-graph.png)、[窄屏明细](screenshots/narrow-details.png)。它们分别记录 1280 × 900 和 390 × 844 下的完整页面。自动检查与截图准备不代表主观体验通过。

**P33 仍待用户确认**：在全部自动工作完成后，按上述可运行步骤检查并反馈：

- 静态图的并行、汇合、独立 node 布局与箭头是否容易核对。
- 桌面与窄屏的 node 标题、ID、输入、zone 及 Bot 说明是否可读。
- 先看具体工作、再进入 Bot 预览的层次与返回导航是否清楚；窄屏纵向滚动是否顺畅。

上述四张图是静态浏览功能的历史材料，不作为下述定义管理 P36 的最终构建证据。用户未确认前，既有 P33 的主观项也不能仅凭截图标为通过。

## 定义管理体验验收（P36）

**人工部分未验收，尚无用户确认。** 自动操作事实、用户体验判断与未检查项应分别记录。所有自动工作（含跨操作验证）结束后，才向用户发起最终体验确认；若其后页面或数据行为改变，须刷新构建、复查受影响流程并重新生成相应截图。自动脚本或截图不能代替用户判断。

### 可运行页面与自动复现

沿用上面的隔离 PowerShell 启动步骤，页面为日志中实际打印的 `http://127.0.0.1:<动态端口>/`，不要使用真实配置或将样例复制到用户目录。服务进程、cwd、定义目录和持久路径均属于这一轮 `$demo`；关闭服务后可保留该目录核对修改。样例中的天气／额度／清单只是静态说明，**不要发起 Chat、点击固定操作或执行工作**。

自动检查从仓库根目录执行，保留 npm 网络护栏：

```powershell
# 开发时定向重现 P36（先构建实际交付页面与后端）
npm --prefix frontend run test:browser:build -- schedulerUsability.spec.ts --workers=1
# 交付时执行完整浏览器入口，不使用过滤器
npm --prefix frontend run test:browser:build -- --workers=1
```

`frontend/browser-tests/schedulerUsability.spec.ts` 在 1280 × 900 与 390 × 844 下分别启动独占真实应用，使用本目录上海 Map 与天气 Bot 的副本，加一个独立损坏 Bot 以核对诊断。该自动场景故意不复制 C／D 的 Bot，因此截图同时显示 `trip-summary`／`quota-estimate` 缺失；上方人工启动步骤复制全部 Bot，不应显示这两条缺失。它通过实际 Tab／Enter、键盘输入、浏览器确认框、真实校验错误和外部文件冲突验证两类对象新增／修改、Bot 往返和草稿恢复；不模拟后端响应，不使用用户数据。截图保存在 Playwright 打印的 `frontend/test-results/schedulerUsability-*` 用例目录中（文件名以 `desktop-`／`narrow-` 开头）：

格式切换前的历史副本（1280 × 900／390 × 844；当前格式请使用最后一次实际测试输出）：

|场景|桌面|窄屏|
|---|---|---|
|从 node 到共享 Bot 预览，含文件诊断|[桌面预览](screenshots/desktop-bot-preview.png)|[窄屏预览](screenshots/narrow-bot-preview.png)|
|JSON 校验失败且保留独立 ID／原始草稿|[桌面校验错误](screenshots/desktop-validation-draft.png)|[窄屏校验错误](screenshots/narrow-validation-draft.png)|
|Map 外部变化后拒绝覆盖，保留草稿并提示重新加载|[桌面冲突](screenshots/desktop-conflict-draft.png)|[窄屏冲突](screenshots/narrow-conflict-draft.png)|
|新 Bot 的独立 ID 与内容 JSON 编辑|[桌面编辑](screenshots/desktop-bot-editor.png)|[窄屏编辑](screenshots/narrow-bot-editor.png)|

最终体验验收应重新生成与待验收构建一致的截图，不能使用上述旧副本；单次定向通过不能声称全部浏览器回归通过。完整回归会重新生成测试输出目录，分享材料应取自最后一次成功执行，避免链接到旧截图。

### 人工操作步骤与独立核对点

桌面与窄屏各执行一次；窄屏使用 390 × 844。可用浏览器设备工具设置 viewport，不要求真实 LAN。只用 Tab／Shift+Tab 走到目标，Enter／Space 激活按钮；JSON 用键盘全选、输入与上下移动，观察输入、错误和保存／返回按钮能否通过纵向滚动访问。

1. 从外侧 `scheduler` 入口进入 `trip-readiness.json`，选择「上海出行天气 A」。点「Bot 明细」，核对 Bot ID 为 `destination-weather`，分别阅读 `when`、`plan_guide` 和 `node_guide`，点 `edit`，ID 应只读，JSON 无顶层 id。仅修改 plan_guide 后保存，应立即预览新说明；通过外侧 `scheduler` 返回后仍选中 A，再次进入可看到新说明。
2. 返回并「修改 node」，确认 JSON 来自真实上海工作而非新增示例，改变 title 后保存，应立即选中新的标题。点「新增 node」，ID 初始为空，填 `manual-node`，把 JSON 改成下面的内容后保存；检查选中 `manual-node`、input 原文包含杭州和业务说明，zone 路径原样保存，且不发起 Chat。

   ```json
   {"title":"人工新增核对","bot_id":"destination-weather","input":"城市：杭州；业务说明：人工核对。","zone":"C:/example/travel-not-registered","blocked_by":[]}
   ```

3. 修改该 node，在 JSON 中输入 `{ "title": ` 后保存，检查错误可读、ID 仍为 `manual-node` 且原始输入未丢。改回合法内容，改变 title 后点「放弃编辑」：先取消，草稿必须保留；再操作并确认放弃，回到原持久内容。不要把“放弃”理解成撤销已保存的内容。
4. 再修改该 node 并保留一份合法未保存草稿。在另一个编辑器中打开本轮 `$work/plans/trip-readiness.json`，只改变 Map description 并保存。回页面保存旧草稿，检查提示重新加载且没有成功预览，ID 与 JSON 仍在。**先复制需要保留的草稿**，放弃并重开后核对外部说明，再重做修改保存。Map 是冲突单位，即使改的是另一个 node 也会冲突；没有自动合并或自动重试。
5. 点外侧 `bot` → `new`，填 `manual-capability`，JSON 删除顶层 id（若手工粘入）并保持 name、when、plan_guide、node_guide 的必需字符串结构；task_id 可省略，三份文档可分别清空。不要保留 schema_version 或旧字段。可把天气示例改为自己的静态核对说明，保存后应直接进入新 Bot 预览，左侧列表同步更新，可直接选择其他 Bot 再选回核对。编辑时列表仍保留；修改草稿后选择另一 Bot，取消放弃应保留原草稿，确认才切换。
6. 在本轮隔离 `$work/capabilities` 新建 `broken.json`，内容为 `{ invalid JSON`，点击 `refresh`，检查问题明确列出该文件及结构错误，其他合法 Bot 仍可打开。删除这个隔离故障文件并刷新，错误应消失；页面不会自动修复它。Map 中 C／D 的 Bot 都已在上方完整启动步骤复制，因此不应凭空出现缺失错误。
7. 尝试删除天气 Bot，应列出 `trip-readiness.json` 的 A／B 和 `another-trip.json` 的 E 等引用并阻止删除。选择 C 依赖的 A 尝试删除 node，应列出后继 C，不改写依赖。只删除刚建且无引用的 `manual-node`／`manual-capability`：先取消最终确认，核对仍存在；再确认，核对当前 Map 保留、node 详情清除／Bot 回到列表。不提供强制删除、级联或撤销。
8. 编辑一份新草稿，试切换外侧入口、刷新全部及浏览器刷新；node 编辑还可试「返回 Map」；取消放弃应继续处理原草稿，确认则执行所选动作。浏览器刷新使用原生提示，文案由浏览器决定。结束前不要将未保存草稿当作已持久化结果。

### 请用户评价（等待所有自动工作结束后）

请分别反馈桌面与窄屏的：

- 阅读舒适度：标题、ID、输入、zone、Bot 能力说明与诊断能否方便核对？
- 编辑顺手程度：ID 与 JSON 分离、键盘移动、保存和返回、取消／确认放弃是否顺手？
- 提示易懂程度：能否分清 JSON 校验失败、文件读取问题和冲突？是否清楚何时需先复制草稿再重新加载？删除引用提示是否足够明确？

反馈请带上 viewport、操作步骤、相关截图及期望；未收到反馈或尚需修正的维度保持未验收。修改后复查受影响的完整流程并再次请求确认，只有用户明确确认后 P36 才全部完成。

### 当前能力与已知限制

支持当前 Map 的 node 及共享 Bot 新增、修改、安全删除；ID 只读意味着不提供重命名。引用检查覆盖整个配置 Map 目录，损坏／不可读 Map 会阻止 Bot 删除；不能把检查失败当零引用，不删引用者或创建副本。无引用对象仍需最终确认，提交时复核版本与引用。

不提供 node 跨 Map 搬移、独立 node 库、文件名输入、Map JSON 编辑、内容搜索、自动文件监听或 Bot 快照；仅提供 Map 文件名本地筛选。所有文本与 JSON 都作数据处理，scheduler 不调度或执行工作。浏览与打开草稿不安装示例或自动修复定义。

同一应用实例协调写入；node 以整个 Map 为冲突单位，Bot 保留原文件与 ID 关联。提交前检查已发生的外部变化，但**不承诺任意外部编辑器与页面提交之间的跨文件原子性**；避免在页面提交期间直接编辑相关定义。文件写到一半可报错，写完后显式重载。窄屏纵向布局需要滚动，不据此声称任意大图均可读或性能达标。本机回环自动检查也不代表真实手机、LAN／mDNS 或主观舒适度已验收。
