# scheduler：可复制的静态定义样例

这些是**人工编写的业务能力示例，不是已实现执行的证明**。天气、额度与清单都不会被查询、计算或生成；保存定义不启动 Chat。Map 与 node 是静态工作定义，不是运行状态或执行实例。共享 `Bot` 描述业务能力，Task 则是发起 Chat 的可复用预设，两者不同。

## 文件与事先规定的核对结果

- `maps/trip-readiness.json`：A「上海出行天气」、B「北京出行天气」并行，**仅 A → C、B → C** 两条边，C「汇总出行准备」汇合，D「独立额度估算」无连接。
- `maps/another-trip.json`：独立 E「广州出行天气」，含只有总体说明、未声明文件的公开输入输出规则；`trip-readiness.json` 整体省略两侧规则。
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

每项 zone 都是 `example/travel`，三份 Bot 均暂未绑定 Task。这些内部合法但外部未完成的定义可保存，不制造默认绑定，不能据此启动。A、B 的不同 input 不会被 Bot 文档或彼此内容覆盖。C 的边不会自动传入 A、B 材料。

## zone 与文件引用

示例使用未登记的稳定 ID `example/travel`，不是目录。仅浏览无需登记；实际启动前显式将该 ID 登记到本机已有目录，另一台机器可为同一 ID 登记不同目录。Map 不保存本机绑定；Run 启动时冻结所需绑定。直接 File 输入使用 `{"id":"source","type":"file","zone_id":"example/travel","path":"inputs/source.txt"}`，其中 path 是以 `/` 分隔的相对路径，省略 zone_id 时使用管理者 zone，不使用 node zone。

更新旧定义前先停止服务并备份。显式为本机目录指定稳定 zone ID，把绝对文件引用改成选定 zone 内的相对路径；有多个候选根时不猜，也不改写历史 Run 或 Chat。仅浏览示例不用登记 zone。

## 在隔离目录运行

以下 PowerShell 从**仓库根目录**执行；先按根 [README](../../README.md#构建与运行) 安装依赖。所有可变数据均写入新建临时目录，不使用用户的 Task、zone、Map 或 Bot。不将样例自动安装到用户数据。

先构建页面和现有安全关机壳；它与交付程序使用同一应用及读取接口，但不会真的关机。固定操作使用已有无害夹具。此页面仅供本机预览。不要点击固定操作、发起 Chat 或用于 LAN／mDNS 验收。

```powershell
npm --prefix frontend run build
cargo build --manifest-path backend/Cargo.toml --bins --example safe_shutdown_shell
```

准备与复制（`$demo` 每次均为新目录，配置文件故意不在 cwd 中；选项目录使用独立副本和仅供预览的扩展文件，不读取本机路径根）：

```powershell
$repo = (Get-Location).Path
$target = if ($env:CARGO_TARGET_DIR) { (Resolve-Path -LiteralPath $env:CARGO_TARGET_DIR).Path } else { Join-Path $repo 'backend/target' }
$demo = Join-Path ([IO.Path]::GetTempPath()) ('herdr-map-preview-' + [guid]::NewGuid())
$work = Join-Path $demo 'process-cwd'
New-Item -ItemType Directory -Path $work | Out-Null
New-Item -ItemType Directory -Path (Join-Path $work 'plans'), (Join-Path $work 'capabilities') | Out-Null
Copy-Item "$repo/docs/example/maps/*.json" (Join-Path $work 'plans')
Copy-Item "$repo/docs/example/bots/*.json" (Join-Path $work 'capabilities')
$config = Join-Path $demo 'preview.toml'
"throw new Error('Preview only: do not launch agents');" | Set-Content -Encoding utf8 (Join-Path $demo 'preview-extension.ts')
$metadata = (Get-Content -Raw "$repo/meta-data/task-options.toml") -replace '(?m)^entrypoint = .*$', "entrypoint = '$demo/preview-extension.ts'"
$metadata | Set-Content -Encoding utf8 (Join-Path $demo 'task-options.toml')
@"
listen_addr = '127.0.0.1:0'
static_directory = '$repo/backend/static'
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
storage_file = 'meta-data/tasks.json'
metadata_file = 'task-options.toml'
[zones]
registry_file = '.data/zones.json'
[sessions]
history_file = '.data/session-history.json'
[scheduler]
map_directory = 'process-cwd/plans'
bot_directory = 'process-cwd/capabilities'
"@ | Set-Content -Encoding utf8 $config
Write-Host "隔离目录：$demo"
Push-Location $work
try {
    & (Join-Path $target 'debug/examples/safe_shutdown_shell.exe') --config $config
} finally {
    Pop-Location
}
```

打开日志中实际打印的 `http://127.0.0.1:<动态端口>/`，保持控制台运行；Ctrl+C 结束。上面用独立安全壳，不要换成生产二进制做关机体验。清理时先结束服务，保留截图和需要的反馈后，只删除本轮打印的 `$demo` 目录。

### 依次操作

1. 从配置组进入 **scheduler**。目录只有两张 Map，不包含 Bot 文件。选 **出行准备核对（人工示例）**（`trip-readiness.json`），在常驻依赖图中核对 A → C、B → C 和独立的 D。逐项选择节点，对照上表的 ID、输入和依赖。图可选中节点，不能拖动节点改定义或创建连线。
2. 从外侧 **bot** 入口选择 `destination-weather`。按任意顺序阅读 `when`、`plan_guide`、`node_guide`，在 `json` 查看原始定义。它们是说明，不是工作结果。回到 **scheduler** 打开另一张 Map，核对 E 仍引用同一个 ID。在隔离 `capabilities` 目录改天气 Bot 的文件名，回到 **scheduler** 点 **刷新全部**，核对关联不变。
3. 在 **bot** 里修改天气 Bot 的 `plan_guide` 并显式保存。已有 ID 只读，内容 JSON 不含顶层 `id`。A、B、E 仍引用同一份定义。Bot 页 **refresh** 重读 Bot；Map 页 **刷新全部** 重读目录、当前 Map 与 Bot，并保留仍存在的节点选择。
4. 回到 `trip-readiness.json`，选 **新增 node**，在独立 ID 控件填 `manual-node`。设为固定 zone `example/travel`，在独立的成果批准设置选自动，保持自动关闭和结束节点未勾选。切到 **JSON 编辑**，填入下面的内容并保存。再次打开节点，核对设置和文字；A、B、C、D 的定义和依赖不变，也不应启动 Chat。

   ```json
   {"title":"人工新增核对","bot_id":"destination-weather","input":"城市：杭州；业务说明：人工核对。","blocked_by":[]}
   ```

5. 从 **scheduler** 打开 **server**，再用 **返回** 回到原页面。

### 默认目录及错误对照

目录基准与默认值见 [scheduler 目录配置](../scheduler.md#目录与定义管理)。要观察正常空态，在新隔离目录中删去 `[scheduler]` 配置段且不复制示例。Map 与 Bot 页应显示空态，不会安装样例。文件错误不是空态；排查见[共享 Bot 问题](../scheduler.md#共享-bot-问题排查)。

## 本例使用的格式子集

A、B、D、E 使用自动成果批准，C 使用人工成果批准。浏览预览不会批准成果、提交报告或执行节点。三份 Bot 的 `task_id` 均为 `null`；页面不补默认执行目标或 zone 登记。完整格式见 [scheduler 参考](../scheduler.md#定义职责与唯一格式)，运行时的批准与报告见[运行说明](../workflow-runtime.md#节点报告与独立通知)。

## 体验材料与保留项（P33）

在桌面和约 390 × 844 的窄屏窗口分别按上面的教程操作。核对并行、汇合及独立节点布局、标题与 ID、输入与 Bot 指南的可读性，以及进入详情和返回的导航。用 Tab、Shift+Tab、Enter、Space 核对键盘可达性，检查窄屏能否滚动到输入、错误、保存与返回。自动结果和截图不能代替用户对清晰度与可用性的判断。

## 定义管理体验验收（P36）

只用本轮独占 `$demo` 的副本，不发起 Chat、不点击固定操作。以[定义保存与校验](../scheduler.md#保存与校验边界)作为独立预期：

1. 编辑 `manual-node`，输入无效 JSON 后保存。错误应可读，ID 和原始草稿应保留。改回合法草稿，尝试离开或刷新，取消放弃后继续编辑；再次操作并确认后才离开。
2. 保留一份合法草稿，在隔离的 `plans` 目录单独修改该 Map 的 `description`。外部编辑结束后再保存页面草稿。旧草稿应因版本冲突保留，原文件不应被覆盖。先复制要留的草稿，再放弃并重新加载，不自动合并。
3. 在 **bot** 里新建自己的无绑定 Bot，保存后核对列表与预览，确认未改其他定义。在隔离 `capabilities` 中添加一个损坏 JSON，再点 **refresh**。错误应标明文件，其他健康 Bot 仍可查看。移除该故障文件后重新读取。
4. 尝试删除天气 Bot 和 A，分别核对跨 Map 引用与 C 的依赖使删除失败。只删除本轮新增且无引用的对象，先取消、确认未删，再显式确认删除；不级联修改其他定义。

测试入口与通用安全规则见[测试指引](../agents/testing.md)。分别记录自动事实、用户体验判断和未检查项。用户反馈应注明视口、步骤、实际提示及预期；不能以一次抽检或截图替代用户确认。
