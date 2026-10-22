# Bot 模板最终集成与 H1/H2 交接

日期：2026-10-22。基线为合并到本票 worktree 的 `9381daa`（含 01–07 的实现）。**自动部分已完成；P23/H1、P50/H2 仍未获用户确认，功能整体验收未通过，本票不标整体 closed。** 本文是验收材料，不是模拟的人工结论。

## 1. 已执行的最终集成

批准 seam：S2 公开 HTTP 联合真实 Herdr 外部对象观察，S3 程序启动、真实文件和重启；S1 浏览器复用既有检查。没有新增生产 seam、遥测或另一套 P22/P49 测试。

预期来自本票与最终配置：保存的六模板应跨真实进程重启保留，公开选项应来自当前元数据，创建应写入 `.data/bot-sessions` 并启动真实 Herdr agent。可辨认的违规是重启丢失模板、仍使用旧根目录、或实际原生参数漏掉固定项/带入未选扩展。通过公开读回、实际磁盘及独立 `herdr pane process-info` 的原生 argv 区分，不从服务端回显重算预期。既有功能已经实现，因此核验现有行为与覆盖，没有制造生产 red 或重写前票测试。

在本票根目录执行 `backend/target/debug/herdr-bot.exe --config herdr-bot.toml`，**没有更换交付 TOML、元数据、Herdr 可执行文件或路径**，也没有触发 server 页的启动/停止/关机操作：

- 真实进程 PID `37672`：公开保存六模板、登记 `E:\Developer\bot-playground`，实际出现 `.data/bot-templates.json`、`.data/zones.json`、`.data/.runtime-layout-v1`。
- 停止该进程，再以相同命令启动 PID `24084`：公开 GET 读回六个完整模板，与停止前逐项相同；同一服务公开 GET 返回当前五个扩展入口和模型/thinking/Skill 选项。
- 当前目录的 `.data` 为隔离 worktree 数据，不是集成分支工作区或用户部署数据。真实会话位于 `<本票根>\.data\bot-sessions\<creation-id>\*.jsonl`，不是根目录 `bot-sessions`。每次创建实际使用独立目录。
- 真实 Herdr 0.9.5 可达、协议兼容，配置名精确命中 `bot-playground`（实得 Workspace `wT`）。首次公开创建返回 `202`，最终 `succeeded` / `inputSubmitted: true`，实得 Tab `wT:t7`、Pane `wT:p7`、别名 `bot-9477d77cab5f`。
- 继续在独立 ambient zone 中公开创建四个 OMP 组合，均收到提交确认；实际进程 argv 的扩展尾部与预写集合相同，详见下表及 [原始运行证据](evidence/bot-template-integrated/final-runtime.json)。这不是六组合受控测试的替代品，也不等于用户已确认真实加载。
- 最终服务监听 `0.0.0.0:18787`；本机请求 `http://192.168.5.3:18787/` 与 `http://herdr-bot.local:18787/` 均实际返回 `200`，日志显示 mDNS published，地址 `192.168.5.3`。这只证明本机访问，**不证明手机或路由器/防火墙可达**。

实际初始化输入为 `/help`。安装版 OMP 把它作为普通消息，产生了模型回复；不把它描述为无需模型请求的内建帮助命令。这些是创建后的初始化输入，**没有把付费模型请求、余额或账号探测加入依赖检查**。下面复现改用已观察到的 OMP `/extensions`；Pi 准备使用原生 `/settings`，先处理其信任提示，不在未知对话框上发送输入。

### 自动检查实际结果

| 执行命令 | 实际结果 | 覆盖/边界 |
|---|---|---|
| `npm ci --prefix frontend` | 成功，安装 106 包 | 验收依赖 |
| `cargo test --manifest-path backend/Cargo.toml` | 363 passed，0 failed，3 ignored；约 338 秒 | 包含 runtime_data 8、bot_creation 21、dynamic_catalog 3、bot_templates 8 及已有并发/快照/持久化检查；不是受控结果冒充真实 agent |
| `npm test --prefix frontend` | 26 文件、184 tests passed | 模板三步、动态目录、失效恢复等现有组件行为 |
| `npm run build --prefix frontend` | TypeScript 与 Vite 生产构建成功 | 实际服务提供这一生产页面 |
| `cargo build --manifest-path backend/Cargo.toml --bins` | 成功 | 使用实际交付二进制 |
| 在 `frontend` 执行 `npx playwright test` | 56 passed，约 67 秒 | 复用全部生产浏览器用例；Chrome 155.0.8059.85，单 worker；包含 P22 六组合及 P49 两视口 |

3 个 ignored 是仓库既有 `mdns_conflict` 的两项独占主机名检查与 `mdns_failure` 的真实 mDNS/防火墙检查，本次未显式运行，不能计为通过。它们不是本功能新增的未完成自动用例。Node 仅报告 `NO_COLOR`/`FORCE_COLOR` 提示，无失败。

### 复用前票证据，不重复认领

- **04 / P22**：原用例 `frontend/browser-tests/templateExtensionLaunch.spec.ts`。本次复跑原检查，归档其原始附件：[六组合](evidence/bot-template-integrated/controlled-herdr-six-combinations-NOT-real-loading.json)（6 组、24 次独立受控调用、6 个受控 Tab）及 [动态入口](evidence/bot-template-integrated/controlled-herdr-dynamic-entry-mapping-NOT-real-loading.json)。只证明调用、开关、入口、未改写配置；夹具不是 Herdr，不能填写 H1 真实加载结论。
- **07 / P49**：复用 [桌面/移动视口操作说明、全部截图与操作 JSON](bot-template-responsive-browser.md)。本次完整浏览器运行再次通过其中 desktop、mobile-viewport 两例。不复制实现同一测试，不把 390×844 移动视口当作手机。
- [04 的 Skill/输入材料](bot-template-h1-skill-inputs.md) 是既有补充资料，不新增本票的最终人工范围；本票负责批准契约中的 P23/H1 与 P50/H2。

## 2. H1：六组模板、预期与实际准备状态

[六组模板和独立字面预期](evidence/bot-template-integrated/h1-cases.json) 可直接导入。模板保存稳定标识；预期入口另列，不由生产参数代码或当前 GET 生成。全部 model/thinking 为 `default`、无 Skill、固定 prompt 为空；固定 model-cost 不可取消。

| key / 模板 | **预写应实际加载的完整集合** | 已执行/待执行与实际定位 |
|---|---|---|
| omp-none / H1 OMP 零项 | OMP model-cost | 真实创建和提交确认；`wT:tA` / `wT:pA`，`bot-0e5e469c3f1c`，`bc-812c5389a321`；H1 用户判断待确认 |
| omp-codebase / H1 OMP codebase-tools | OMP model-cost、OMP codebase-tools | 真实创建和提交确认；`wT:tB` / `wT:pB`，`bot-0a14d62bb3dd`，`bc-ef92a40137e6`；H1 用户判断待确认 |
| omp-connect / H1 OMP connect-bot | OMP model-cost、OMP connect-bot | 真实创建和提交确认；`wT:tC` / `wT:pC`，`bot-22f53ebb0792`，`bc-d1f2f1808ba7`；H1 用户判断待确认 |
| omp-both / H1 OMP 两项 | OMP model-cost、OMP codebase-tools、OMP connect-bot | 真实创建和提交确认；`wT:tD` / `wT:pD`，`bot-c4f5bd529a53`，`bc-4aea59381e64`；H1 用户判断待确认 |
| pi-none / H1 Pi 零项 | Pi model-cost | 模板已保存；真实模板创建待用户处理 Pi 项目信任，无可报告的组内 Tab/Pane 或实际 argv |
| pi-codebase / H1 Pi codebase-tools | Pi model-cost、Pi codebase-tools | 模板已保存；同上，不虚构组内标识或加载结果 |

### 启动参数

四组 OMP 的**实际原生 argv**来自 Herdr 进程观察，完整值保留于 `final-runtime.json` 的 `ompAmbientGroups[].actualNativeArgv`；均为安装版 Bun + `@oh-my-pi/pi-coding-agent/dist/cli.js`，后接：

```text
--session-dir <本票根>\.data\bot-sessions\<上表 creation-id>
--no-extensions
--extension E:\Developer\myhub\my-agent-toolkit\entrypoints\omp\model-cost.ts
```

仅 codebase-tools 再接 `--extension E:\Developer\myhub\my-agent-toolkit\entrypoints\omp-auto\codebase-tools.ts`；仅 connect-bot 再接 `--extension E:\Developer\myhub\my-agent-toolkit\entrypoints\omp-auto\connect-bot.ts`；两项时按上述 codebase-tools、connect-bot 顺序各接一次。与预写字面尾部核对均相同。

Pi 两组**尚未执行**的预期参数为 `--session-dir <届时实际独立目录> --no-extensions --extension E:\Developer\myhub\my-agent-toolkit\entrypoints\pi\model-cost.ts`；有 codebase-tools 再接 `--extension E:\Developer\myhub\my-agent-toolkit\entrypoints\pi-auto\codebase-tools.ts`。不可把这段预期写为观察到的实际 argv；执行后从真实 `pane process-info` 补录。

### 可辨认额外加载的 ambient 对照

隔离目录为 `<本票根>\.data\h1-ambient-zone`。只在其中 `.omp/extensions/h1-ambient-extra.ts` 和 `.pi/extensions/h1-ambient-extra.ts` 放置 [ambient-probe.ts](evidence/bot-template-integrated/ambient-probe.ts) 的副本；没有改用户/项目现有 agent 设置，没有把 probe 加入元数据或所选集合。probe 初始化时仅在当前 zone 追加 `h1-ambient-loaded.jsonl`，包含真实 cwd、PID 与时间，不访问模型、账号或余额。它是外部验收资源，不是新增产品遥测。

1. **正对照**：相同 zone、不加 `--no-extensions`，原生启动对应 agent；必须先观察 probe 确实可发现并执行，否则阴性没有辨识力。实际 OMP 正对照 `wT:t8` / `wT:p8`、`h1-positive-omp` 成功，得到 [真实 marker](evidence/bot-template-integrated/ambient-positive.jsonl)（PID 34120）。已关闭这个专为正对照创建的 Tab，然后将 marker 改名为 `h1-positive-before-negative.jsonl`，与下一阶段分离。
2. **OMP 阴性**：四个公开创建仍使用同一 zone；实际进程带关闭发现开关与显式入口。四组创建和观察后未再次出现 `h1-ambient-loaded.jsonl`。该观察支持“这个可识别 ambient 未执行”，**不能独自证明加载了全部期望项，也不能推广为所有 ambient 种类已验证**。
3. **Pi 正对照真实阻塞**：实得 `wT:t9` / `wT:p9`、`h1-positive-pi`。其实际启动参数仅为 `pi --session-dir <本票根>\.data\h1-control-pi`，但是 TUI 停在 **Trust project folder?**。原文保留在 `final-runtime.json` 的 `piTrustObservation`。agent 未代用户选择 Trust，也未向对话框投递模板初始输入。故没有 Pi 正对照 marker，不能宣称 Pi ambient 对照通过。用户只应审阅并信任这一隔离 zone，不应为省事信任整个临时目录父级。
4. 用户完成 Pi 信任后，先建立 Pi 正对照 marker，保存后改名，再从 zone `+` 分别创建两个 Pi 模板，记录真实新标识、argv、加载观察与 marker 阴性。此前的 OMP 正对照不能代替 Pi 正对照。

原生 OMP `/extensions` 已实际打开 Extension Control Center，原始屏幕保存在 `final-runtime.json` 的 `nativeDashboard`；屏幕列出的 ALL/发现目录不等同于运行时已加载集合。用户应结合实际扩展注册、原生加载错误/资源面板、model-cost 输出、codebase-tools 可用工具及 connect-bot 注册/启动反馈核对**完整集合**，不是仅看“开关对了”或“某一个命令存在”。若安装版不能区分发现候选与已加载资源，应保留未确认并补充原生运行证据，不凭参数勾选通过。

两个选 connect-bot 的真实 OMP 屏幕已出现“无法连接 herdr-bot，未启用”提示。它说明自动接入未成功；可能涉及 toolkit 自己的 endpoint 配置，本文没有探测账号或改写其配置。创建成功只表示 agent 就绪和初始提交确认，**不表示 chat 已登记、connect-bot 已启用、扩展功能质量通过**。请在 H1 记录中区分加载、初始化错误和接入成功。

### 在交付目录重建材料

以下命令在最终交付目录的 PowerShell 中执行；仅新增模板，不删除已有模板。若当前实例已有上表六模板，可直接使用，勿重复导入。以实际地址替换 `$base`，不要从本票历史标识推导新标识。

```powershell
# 构建命令见上表；前台启动，根目录必须是最终交付目录。
.\backend\target\debug\herdr-bot.exe --config .\herdr-bot.toml
```

另开 PowerShell：

```powershell
$base = 'http://127.0.0.1:18787'
$cases = Get-Content .\docs\evidence\bot-template-integrated\h1-cases.json -Raw -Encoding UTF8 | ConvertFrom-Json
foreach ($case in $cases.cases) {
  $json = $case.template | ConvertTo-Json -Depth 8 -Compress
  Invoke-RestMethod "$base/api/bot-templates" -Method Post -ContentType 'application/json; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes($json))
}
# 在实际隔离 zone 放置 probe；已有 marker 先留存并改名，勿覆盖既有证据。
$zone = Join-Path (Get-Location) '.data\h1-ambient-zone'
New-Item -ItemType Directory -Force "$zone\.omp\extensions", "$zone\.pi\extensions" | Out-Null
Copy-Item .\docs\evidence\bot-template-integrated\ambient-probe.ts "$zone\.omp\extensions\h1-ambient-extra.ts"
Copy-Item .\docs\evidence\bot-template-integrated\ambient-probe.ts "$zone\.pi\extensions\h1-ambient-extra.ts"
# zone 页登记 $zone；bot 页核对六模板；zone 的 + 逐一选择，不临时覆盖扩展。
```

真实 Herdr 控制必须在 `HERDR_ENV=1` 的受管终端中进行；先执行 `herdr status server`、`herdr workspace list` 取得当前唯一 `bot-playground` ID。正对照用 `herdr tab create --workspace <实际ID> --cwd <隔离zone> --label "H1 ambient positive <agent>" --no-focus`，再按创建响应中的 pane ID 执行 `herdr agent start <本次唯一别名> --kind <omp或pi> --pane <实际pane> -- --session-dir <新建独立正对照目录>`。不创建/替代缺失的 Workspace，不回答未知信任或批准提示。

模板创建的本次 input：OMP 使用 `/extensions`，Pi 在确认信任且正常输入界面后使用 `/settings`。它们旨在观察原生界面，不以模型回复质量为验收项。创建返回对象中记录本次真实名称、Tab、Pane 与 agentName；通过 `herdr pane process-info --pane <实际pane>` 保存原生 argv，通过 `herdr agent read <实际agentName> --source recent-unwrapped --lines 100` 保存实际加载/错误观察。必要时在实际 TUI 截图，不只保存参数。

**每组用户记录内容：** 模板 key、预期全集、实际加载全集及证据位置、实际 argv、zone/cwd、Tab/Pane/别名、正对照 marker、阴性 marker、加载错误、通过/失败/无法判断与原因、确认人和时间。六组任何一项集合不符或 ambient 额外执行都不能通过 H1。只有参数正确的组仍是未验证。

## 3. H2：真实手机访问与操作

### 前提和入口

- 手机和 Windows 服务主机在同一可信 Wi-Fi/局域网；Windows 防火墙允许该专用网络的 TCP 18787。AP/client isolation、访客 Wi-Fi、手机 VPN/移动数据分流可能阻断；本票没有修改防火墙或路由器设置。
- 本次已观察的候选入口：`http://herdr-bot.local:18787/`，IP 兜底 `http://192.168.5.3:18787/`。DHCP、服务重启或搬移机器后先用 `ipconfig` 与启动日志重新取得地址；不要使用 Tailscale/WSL 虚拟地址冒充同网段入口。
- 电脑访问成功不证明手机成功；先在手机上实际打开页面。旧 Android 或网络禁组播可用 IP 兜底，不把 mDNS 不可解析误写成模板失败。
- 服务具有无账号的模板/创建写能力，只在可信局域网开放，不能为了验收转发到公网。
- [07 截图与逐步 HTTP/点击记录](bot-template-responsive-browser.md#截图索引) 是本次使用的桌面/移动视口参考。实际手机截图另存并注明机型、OS、浏览器版本、键盘/输入法、字体/缩放设置、方向和网络入口；本票没有手机截图或触摸结论。

### 真机操作顺序与判断标准

1. 打开 `bot` → 新增模板。用软键盘输入名称 `H2 真机巡检`；命名编辑器输入中文、空格、标点，在不同光标位置插入四类字段 `zone.name`、`template.name`、`agent`、`bot.id`，触摸删除一块、重新插入，核对规则和预览。检查光标/中文组合输入不丢字，软键盘不遮挡无法到达的操作区。
2. 第一步选择 OMP、`gpt-6.1-sol`、high、Skill，输入两行中文和引号/路径的 prompt；第二步用手指勾选 codebase-tools、connect-bot，确认固定 model-cost 不可取消。退回第一步再前进，逐项核对草稿。切到 Pi 时确认 model/thinking 重置且不继承 OMP 勾选，再选择 Pi 的有效配置。
3. 第三步逐项核对摘要；滑动到保存，**明确保存后**再次打开验证结果。在三个步骤往返时不用浏览器缩放规避不可操作控件；检查触摸目标、纵向滚动、键盘弹出/收起、系统返回键与横屏恢复，不把 DOM 存在当作可操作。
4. 通过下面的独立 H2 配置制造可恢复的失效勾选：仍能辨认原 ID，手机返回第二步明确取消，再到摘要明确保存。确认不能靠步骤往返/取消编辑悄悄修正。
5. 在同一独立配置制造创建前入口缺失：zone 行 `+`，选择模板、输入两行中文本次 input，提交后阅读路径/原因及“未创建 Tab、未启动 agent、未提交输入”；input 保留，能滚动并触摸关闭。修复元数据后重新打开，错误可以继续修正，不声称预览等于成功启动。
6. 手机用户给出可操作/失败/无法判断，并附实际触摸、软键盘、输入法与错误修正证据。任何阻断性横向溢出、无法触达按钮、输入丢失或错误无法修正都不能通过 H2。390px 电脑视口通过不能替代这一判断。

### 安全复现错误：独立 H2 配置

只在最终交付目录创建下列新文件；不要对部署元数据或用户模板制造故障。下列目录需为本次新建，若已有内容，应选另一个全新的名称并对应替换，不覆盖之前验收数据。

```powershell
New-Item -ItemType Directory .data\h2-acceptance -ErrorAction Stop | Out-Null
Copy-Item meta-data\bot-options.toml .data\h2-acceptance\bot-options.toml
$config = Get-Content herdr-bot.toml -Raw -Encoding UTF8
$config = $config.Replace('0.0.0.0:18787', '0.0.0.0:18788').Replace('.data/zones.json', '.data/h2-acceptance/zones.json').Replace('.data/bot-templates.json', '.data/h2-acceptance/bot-templates.json').Replace('meta-data/bot-options.toml', '.data/h2-acceptance/bot-options.toml')
$config += "`r`n[mdns]`r`nenabled = false`r`n"
[IO.File]::WriteAllText((Join-Path (Get-Location) '.data\h2-acceptance\herdr-bot.toml'), $config, [Text.UTF8Encoding]::new($false))
.\backend\target\debug\herdr-bot.exe --config .data\h2-acceptance\herdr-bot.toml
```

手机这时访问 **`http://<实际LAN-IP>:18788/`**，需要对应的专用网络防火墙许可；不发布第二份同名 mDNS。登记实际隔离 zone，先正常保存一个勾选 Pi codebase-tools 的模板。另一个 PowerShell 只改这份目录副本：

```powershell
$meta = Join-Path (Get-Location) '.data\h2-acceptance\bot-options.toml'
$valid = Get-Content $meta -Raw -Encoding UTF8
# 失效已选项：精确删除 pi-codebase-tools 所在 extension 段，兼容 CRLF。
$invalidSelection = [regex]::Replace($valid, '(?ms)^\[\[extensions\]\]\r?\nid = ''pi-codebase-tools''.*?(?=^\[\[|\z)', '')
[IO.File]::WriteAllText($meta, $invalidSelection, [Text.UTF8Encoding]::new($false))
# 此时手机重新编辑并实际取消失效项、明确保存。完成后恢复：
[IO.File]::WriteAllText($meta, $valid, [Text.UTF8Encoding]::new($false))
# 创建前入口失败：不删除真实扩展，只改隔离副本固定 Pi 入口。
$missing = Join-Path (Get-Location) '.data\h2-acceptance\missing-model-cost.ts'
$invalidEntry = $valid.Replace('E:\Developer\myhub\my-agent-toolkit\entrypoints\pi\model-cost.ts', $missing)
[IO.File]::WriteAllText($meta, $invalidEntry, [Text.UTF8Encoding]::new($false))
# 手机完成失败观察后恢复，无需服务重启；不要把这些分隔步骤一次全部粘贴执行。
[IO.File]::WriteAllText($meta, $valid, [Text.UTF8Encoding]::new($false))
```

这些是供用户逐阶段执行的 H2 真机材料，**本票没有把它们当成已执行真机证据**。桌面和移动视口的同类机制已由 07 的真实工作流验证。

## 4. 交接与未验证项

- 本票自动部分和可执行准备完成。H1：四组真实 OMP 已准备/观察，但全部六组的完整实际加载集合仍待用户最终确认；Pi 需要用户审阅并处理实际信任提示，再完成正对照及两个真实模板创建。H2：没有接入真实手机，触摸、软键盘和输入法结论未验证。
- 本票原始文件、截图链接、独立预期与真实外部标识均已保留；没有伪造 Pi 组内 Tab/Pane、真实手机结果或受控 adapter 的实际加载结果。
- 上述 PID/Tab/Pane 是本次观察时的真实标识，不是永久定位承诺。handoff 时最终 HTTP 验收服务已停止、18787 已释放；OMP 阴性组及 Pi 信任正对照仍保留供用户观察，OMP 正对照 `wT:t8` 已关闭。手机访问前需按上文在最终交付目录重新启动服务。运行资源引用本票忽略的 `.data`；在保留原生会话用于 H1 时，不应删除该 worktree 的数据。若集成工具回收 worktree，按本文在最终交付目录重建资源，重新记录新标识；版本库只交付材料，不迁入用户运行数据。
- H1/H2 用户确认前，P23/P50 与整体验收保持未验证，票据保持 `ready-for-human`，不是 `closed`。用户按实际证据记录通过、失败或无法判断；真实扩展集合/开关映射问题回到对应创建实现，UI 阻断回到响应式界面实现，修正后重验相应组，不把失败改写为通过，也不改其他票据或工作流报告。
