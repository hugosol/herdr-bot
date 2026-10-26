# scheduler：可复制的静态定义样例

这些是**人工编写的业务能力示例，不是已实现执行的证明**。天气、额度与清单都不会被查询、计算或生成；`chat` 仅声明实现类别，不启动 Chat。Map 与 node 是静态工作定义，不是运行状态或执行实例。共享 `node_template` 描述业务能力，Bot 则是发起 Chat 的可复用预设，两者不同。

## 文件与事先规定的核对结果

- `maps/trip-readiness.json`：A「上海出行天气」、B「北京出行天气」并行，**仅 A → C、B → C** 两条边，C「汇总出行准备」汇合，D「独立额度估算」无连接。
- `maps/another-trip.json`：独立 E「广州出行天气」。
- `node-templates/weather-capability.json`：身份为 `destination-weather`，被 A、B、E 跨 node／跨 Map 复用；**文件名故意不同于 id**。
- `node-templates/summary-capability.json`：身份为 `trip-summary`，用于 C。
- `node-templates/quota-capability.json`：身份为 `quota-estimate`，用于 D。

|node|本次参数（独立预期）|前置|
|---|---|---|
|A|city=`上海`；date=`2026-11-01`；preferences=`{"walking":true,"notes":["带伞",null]}`|无|
|B|city=`北京`；date=`2026-11-02`；zone=`尚未登记的出行资料目录`|无|
|C|scope=`上海与北京的出行准备`；checklist=`["天气","随身物品"]`|A、B|
|D|account=`人工示例账户`；remaining_percent=`25`；observed_at=`null`|无|
|E|city=`广州`；date=`2026-11-03`|无|

A、B 的不同参数不会被模板输入说明或彼此的内容覆盖。B 的 zone 名称不要求真实存在，A 的 preferences 未在模板 inputs 声明也合法。C 的边不把 A、B 的输出自动传入 C；材料绑定与执行路由尚未定义。

## 在隔离目录运行

以下 PowerShell 从**仓库根目录**执行；先按根 [README](../../README.md#构建与运行) 安装依赖。所有可变数据均写入新建临时目录，不使用用户的 Bot、zone、Map 或模板。不将样例自动安装到用户数据。

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
Copy-Item "$repo/docs/example/node-templates/*.json" (Join-Path $work 'capabilities')
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
[bots]
storage_file = '.data/bots.json'
metadata_file = '$repo/meta-data/bot-options.toml'
[zones]
registry_file = '.data/zones.json'
[sessions]
history_file = '.data/session-history.json'
[scheduler]
map_directory = 'plans'
node_template_directory = 'capabilities'
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

1. 初始是 `chat`；切到第四个 tab `scheduler`。列表恰有 `another-trip.json` 和 `trip-readiness.json`，不应出现三个模板文件。
2. 选择 `trip-readiness.json`，默认显示节点清单与详情空态，依赖图收起。核对 A、B、C、D 标题与 ID，以及 C 的 A／B 前置关系。可在目录输入文件名筛选；没有匹配项时仍保留当前 Map 和 node，清空筛选恢复目录。
3. 从节点清单逐项选择 A、B、C、D，与上表核对本次参数和依赖。详情中的前置与后继节点可点击定位，关闭详情后焦点回到节点清单。模板默认收起，点“模板明细”才看到能力说明；切 node 后再次收起。展开“依赖图”核对 A → C、B → C，D 独立；图与清单选中联动。拖动图内空白处平移，加减按钮缩放，“适应全部”恢复总览；不拖动编辑节点或创建连接。
4. A、B 展开同一个「目的地天气摘要」；核对适用场景、业务边界、命名输入 city／date、命名输出 forecast 和 chat。C 为「出行准备清单汇总」（scope／checklist → summary），D 为「额度恢复时间估算」（account／remaining_percent／observed_at → estimate）。逐项文本以交付模板 JSON 为准；它们都是预期说明，不是实际结果。
5. 选择 `another-trip.json` 与 E，核对广州、日期和同一「目的地天气摘要」。可以在隔离 capabilities 内重命名 weather 文件，点击“刷新全部”，关联仍按 id 成立。
6. 修改隔离模板的 title 或 description，点击“刷新全部”，重新选 node 并展开，看到新说明；Map 内容修改也同样重读。当前 node／模板详情会清除，依赖图收起，随后可重新选择 node，不保留旧明细。
7. 将另一个 JSON 副本放入隔离 plans，点击“刷新全部”看到新增文件；删除该副本后刷新则移除。刷新也重新读取当前 Map 与模板；不自动监听文件。
8. 从 scheduler 点右上角 `server`，原页面内进入独立内容页，再点“返回 scheduler”。不要用新窗口或寻找第五个 tab。
9. 窄屏使用约 390 × 844 viewport；按文件目录 → 节点清单 → node 详情 → 模板明细纵向访问，图按需展开。滑动滚动页面；可用 Tab／Shift+Tab、Enter／Space 操作，图内聚焦节点会按需移入视口，顶部 tab 另支持方向键、Home／End。

### 默认目录及错误对照

两个 scheduler 字段可各自省略；默认分别为 cwd 下的 `.data/workflow-maps/` 和 `.data/node-templates/`。无论配置文件在哪里，相对目录都按**进程 cwd** 解析。绝对路径也可用。配置修改需重启；目录文件变化只需显式刷新或重载。

首次读取自动创建缺失的配置目录及父目录，保留已有文件，不覆盖定义，也不安装这份样例。要观察默认空态，在另一份新临时目录中按上述配置运行但删去整个 `[scheduler]` 段，不复制样例；进入 scheduler 应有明确的 Map／模板空态，而不是样例图。服务还会维护自己的既有持久数据；scheduler 的只读约束专指 Map／模板定义。

- 目录路径被普通文件占据、权限不足或无法创建／读取：明确显示目录错误，不冒充空目录。先停止服务或在隔离目录内修复，再刷新。
- Map JSON、结构或依赖无效：清除旧图与详情，显示文件及可用字段位置；仍可选择其他有效 Map。
- 模板缺失、不可读、无效、未知实现类型：保留合法 Map 图和参数并提示问题，不捏造说明或回退 chat。重复 id 列出冲突文件，不任选其一；正常模板仍可查看。
- 模板目录出错与单文件问题分开提示。修复后刷新或重载读取当前文件；具体排查见 [共享模板问题排查](../../README.md#共享模板问题排查)。
- 两个目录均只扫描顶层 JSON（后缀不区分大小写），不递归；链接、目录联接和其他重解析点不能绕过读取范围。

## 受支持格式（生成者也使用同一种格式）

字段名固定为 **snake_case**，版本 `schema_version` 为**整数 1**，不是字符串。每张 Map、每个模板各占独立 JSON 文件；没有专供图形展示的 edges、位置或模板副本格式。

|对象|必需字段与类型|
|---|---|
|Map|`schema_version`: 1；`title`、`description`: 文本；`nodes`: Map 内 ID 到 node 的对象|
|node|`title`、`node_template_id`: 文本；`parameters`: 命名参数对象；`blocked_by`: 同一 Map 内前置 ID 文本数组|
|node_template|`schema_version`: 1；`id`、`title`、`use_when`、`description`: 文本；`inputs`、`outputs`: 名称到文本说明的对象；`implementation`: 对象|
|implementation|`type`: 目前仅接受 `chat`；没有模型、Bot、命令、执行步骤等配置|

node ID 是 `nodes` 的 key，只在所属 Map 内使用，不在值中重复添加 id。共享模板的 `id` 是跨 Map 身份，`node_template_id` 按它关联而非文件名。Map 不含 `node_templates`。B 的 `blocked_by` 含 A 就是 A → B；未知前置、自依赖或环均报错，不静默删边。

`use_when` 解释适用需求与场景；`description` 说明泛化业务能力、结果与重要边界，不以启动系统资源指令代替。inputs／outputs 的业务名称自由，值必须是**说明文本**；输出不是实际产出，也不引入 type、required 或验证表达式。

固定结构严格检查必需字段、类型、未知字段和版本：误写 blockedBy、漏填 blocked_by、数组写成字符串、未知版本或未知实现类型都不支持。显式 `nodes: {}`、`parameters: {}`、`blocked_by: []`、`inputs: {}`／`outputs: {}` 和文本 `""` 合法，不等于漏填或 `null`；`implementation: {}` 则缺少必需 type。

`parameters` **保持开放**：任意业务键与直接 JSON 值（文本、数字、布尔、null、数组、对象）均可，不做 inputs 匹配、必填性、参数类型、zone／Bot 存在性或结果有效性校验。固定结构不能用 null 代替必需类型。嵌套对象的 key 显示顺序不作承诺，JSON 值保持不变。所有业务文本只作文本显示；路径和指令也不读取或执行。

## 体验材料与保留项（P33）

浏览器验收 `frontend/browser-tests/schedulerExamples.spec.ts` 直接复制本目录文件，经 S1 真实主界面核对上表、全部模板明细、跨 Map 复用、重载、刷新和 server 返回；不使用专供文档的读取接口。项目完整回归入口见 [测试指引](../agents/testing.md)。

本次交付截图：[桌面图](screenshots/desktop-graph.png)、[桌面明细](screenshots/desktop-details.png)、[窄屏图](screenshots/narrow-graph.png)、[窄屏明细](screenshots/narrow-details.png)。它们分别记录 1280 × 900 和 390 × 844 下的完整页面。自动检查与截图准备不代表主观体验通过。

**P33 仍待用户确认**：在全部自动工作完成后，按上述可运行步骤检查并反馈：

- 静态图的并行、汇合、独立 node 布局与箭头是否容易核对。
- 桌面与窄屏的 node 标题、ID、参数及模板说明是否可读。
- 先看具体工作、再展开模板的层次是否清楚；窄屏纵向滚动是否顺畅。

已知限制：只预览，不执行；没有编辑、内容搜索、自动文件监听、模板快照或跨文件原子快照；仅提供 Map 文件名本地筛选；外部文件写到一半可报错，写完后显式重载。窄屏为纵向布局，完整明细需要页面滚动。未承诺任意大图的性能或可读性，也未以本机回环检查宣称真实 LAN 已验收。用户未确认前不能把 P33 标为通过；收到反馈后应修正并重查受影响行为。
