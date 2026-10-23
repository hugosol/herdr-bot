# 07：Pi／OMP 真实扩展加载材料与两项确认交接

## 结论与验收边界

已在**全新专用隔离环境**通过实际交付页面 → 生产 HTTP／Herdr adapter → 真实 Herdr 0.9.5 → 已安装 Pi 1.4.0／OMP 18.12.6，完成两 agent 各三组加载观察。不是受控 Herdr，也不是仅核对 argv。每个样例均保留真实扩展 `session_start` 回调、PID、原生 ID／文件、实际 cwd、Herdr 目标、HTTP 与页面反馈。

**H-Pi、H-OMP 均未验证：没有用户确认。** 本文只交付材料，不代签 P4／P5／P7 的人工分项，不宣称这些承诺整体已验收。显式、部分和默认集合复用每个 agent 的同一项确认；没有新增身份、浏览器、极端空集合或其他人工项。

最终人工确认轮尚未发起。[05 的自动检查汇总](integrity-05.md)记录前端 205 测试与 73 浏览器用例通过，后端全量 **446 passed／1 failed／5 ignored**；唯一失败是 `zone_sync::configured_timeout_stops_the_running_command_and_keeps_effective_pull_state` 的 HTTP 200／409 差异。本文不重跑掩盖它、不越票修复，也不声称全量全绿。主代理须先处置该已记录失败并汇总全部已分配自动检查，再一次性交出下述两项确认。等待人工不阻止其他可执行工作。

P21 的当前交付文案已核对：README 的两组配置、最新环境、初始 Skill 调用／加载型 Skill、用户输入／系统提示词，以及不传 `--session-dir`、原生 autoResume 不属于本项目自动续接的说明，与 GLOSSARY／integration-boundaries 一致；06 的整合生命周期由主代理核对，不是本票真实加载准备的硬阻塞。

## 原始材料

- [runtime-evidence.json](loading-07/runtime-evidence.json)：六个真实页面恢复的原始 HTTP 收据、页面文本、实际目标与终端输出、扩展回调、精确身份；另含直接原生控制与观察器校准。`runtimeCases` 才是页面→真实 agent 的六组证据，`directNativeControls` 不冒充页面恢复证据。
- [fixture-bundle.json](loading-07/fixture-bundle.json)：完整专用配置／扩展源码、原生 seed 副本、来源 Bot、各场景状态文件、当前服务地址与独立部署位置。入口可读或参数正确都不作为加载结论。
- 截图：[Pi 显式](loading-07/pi-explicit.webp)、[Pi 部分](loading-07/pi-partial.webp)、[Pi 默认](loading-07/pi-native-default.webp)、[OMP 显式](loading-07/omp-explicit.webp)、[OMP 部分](loading-07/omp-partial.webp)、[OMP 默认](loading-07/omp-native-default.webp)。截图中的捕获端口为 7735／7736；交接进程已迁至独立部署，见下表。

## 隔离环境与精确身份

专用根目录 `$R = 'C:\Users\hugos\AppData\Local\Temp\resume-loading07-hia3_os4'`。没有向用户现有 Chat 输入、关闭其目标、修改其 Bot 或原生记录；所有被关闭的目标均为本次新建的专用目标。Herdr 命令始终显式带本次新分配的 `--session`，不使用默认／focused session，也没有伪造调用方 `HERDR_ENV`。

| 项 | Pi | OMP |
| --- | --- | --- |
| 专用 Herdr session | `loading07-pi-hia3os4` | `loading07-omp-hia3os4` |
| 唯一默认 Workspace | `w1`／`loading07-pi` | `w1`／`loading07-omp` |
| 当前独立页面 | http://127.0.0.1:1594/ | http://127.0.0.1:1595/ |
| 原生完整 ID | `01a1672d-b70b-72d3-b25d-c96f455256c7` | `01a16737-5b3b-76f7-ab1b-05324809594b` |
| 本轮原生文件 | `$R\pi\native.jsonl` | `$R\omp\native.jsonl` |
| 实际／历史 cwd | `C:\Users\hugos\AppData\Local\Temp\herdr-native-storage-vysibcs5\pi\zone` | `C:\Users\hugos\AppData\Local\Temp\herdr-native-storage-vysibcs5\omp\zone` |
| 来源 Bot ID | `tpl-18e12de7a8cca864-0` | `tpl-18e12deaf75ce734-0` |
| 保留的默认发现目标 | `w1:t4`／`w1:p4` | `w1:t4`／`w1:p4` |

原生文件来自此前**专用验收 seed 的副本**，不是用户 Chat。history 的 source fixture 是独立预写，不冒充 P1／P2 的可信发起收录证据；可信来源与完整三类默认分支的自动覆盖仍归其他票。Bot 的 A→B 编辑本轮确实经过公开 POST／PUT；OMP 删除来源确实经过公开 DELETE。

每个 agent 的原生配置目录分别为 `$R\<agent>\agent`，HOME／USERPROFILE 分别为其专用 `home`。`settings.json` 为 `{}`；OMP 的 `config.yml` 禁止 autoResume 与 setup wizard；模型配置仅指向此前专用 loopback storage-probe，没有复制用户凭据。未向模型提交任务，未重放 Bot 中的 `DO NOT REPLAY LOADING07`。Herdr 配置禁止 server 重启自动恢复 agent，默认 shell 只在专用配置中设为 cmd.exe。

当前应用来自 `$R\deployment\target\debug\herdr-bot.exe`，打包页面位于 `$R\deployment\static`，源码／manifest 也是本轮隔离部署副本。已停止所有依赖本 worktree 可执行文件的临时应用进程，因此独立集成可以接管 worktree。专用 Herdr 与最后两目标保留；不是用户日常服务。

## P4／P5／P7 的实际观察

每个 probe 都是简单真实扩展：在原生公开 `session_start` 回调中追加 `{agent, marker, pid, cwd, sessionId, sessionFile}` 到 `$R\<agent>\loaded.jsonl`，并调用原生 UI notify。它不替代 loader，不依据 argv 推导结论，不写入对话消息。官方 Herdr 身份扩展另行实际加载并向 Herdr 报告同一原生引用。OMP 不保证启动 notify 留在终端 viewport，因此其实际加载结论使用原生进程执行的回调记录，而非要求终端出现一行文案。

| 覆盖／样例 | 独立预期与实际回调集合（两 agent 均相同） | 实际页面与目标 |
| --- | --- | --- |
| P4 显式 | `fixed` + `latest-b`；没有 `old-a`、`ambient`、`native-default`。Bot 原先选 A，公开编辑后选 B；官方 identity 也是固定项。 | 两者 HTTP 200／`restored`；Pi、OMP 各在本 session 的 `w1:t2`／`w1:p2`。缺失原 Workspace 的反馈同时保留，回退至本次唯一默认 Workspace。 |
| P5 部分可用 | 保留有效 fixed／identity，将所选 `latest-b.ts` 改名使入口缺失；实际回调只有 `fixed`，没有 ambient。 | 两者 HTTP 200／`restored`；`w1:t3`／`w1:p3`。页面明确标出 `latest-b`、入口不存在、跳过该项、仍关闭扩展自动发现，同时保留 Workspace 回退。 |
| P7 原生默认 | 恢复 B 文件为可读后，Pi 独立旧记录移除 source 关联但保留同名 Bot；OMP 保留 source 关联并公开删除 Bot。实际回调仅 `ambient` + `native-default`，官方 identity 从原生默认目录发现；没有 Bot 显式 fixed／A／B。 | 两者 HTTP 200／`restored`；`w1:t4`／`w1:p4`。页面分别说明缺少来源／来源已删除、原生默认发现，以及 Workspace 回退。 |

ambient 正是同一个默认目录中的有效扩展：显式／部分样例不加载它，而默认样例确实加载，排除了“ambient 本身坏了”的伪证。另以每个 agent 的直接原生控制实际加载 `old-a`，证明旧选项不是不可执行才被排除。

观察器敏感性也已校准：在独立直接 CLI 控制中故意省掉 `--no-extensions`，两 agent 都执行了 ambient／native-default 加上 fixed／B 的回调，独立的 P4 字面期望因此拒绝该集合。未修改产品来制造失败，也未把故意泄漏的控制结果计作页面通过。此票没有新产品行为，因此不重复增加 02／03／04 已拥有的自动测试或测试私有接口。

真实终端均保留原 seed 的旧历史，Herdr 报告实际精确文件、cwd、idle 与 focused。Pi 的副本字节未变。OMP 在本票直接 RPC 控制退出及专用目标关闭时会自行追加 `custom/session_exit` 元数据，故**不宣称 OMP 整个文件 hash 不变**；原有全部 message 条目逐项不变，没有新增 user／assistant／tool 活动。两个原始专用 seed 文件 hash 前后均不变。此差异及所有原生行保留在 JSON，未通过修剪数据掩盖。

## 可重复步骤（只操作本次专用资源）

以下是已准备的复现步骤，不是现在向用户发起最终确认。主代理完成自动检查门槛后，H-Pi／H-OMP 各执行同一套显式→部分→默认材料，不另加人工项。

1. 核对 `$R`、对应 session、完整原生 ID／文件与 cwd。只附着 `herdr session attach loading07-pi-hia3os4` 或 `herdr session attach loading07-omp-hia3os4`。另一浏览器打开表中对应独立页面，进入历史会话。当前保留目标为默认场景；不要关闭不在本 session 的任何目标。
2. 每个 agent 依次复现 `explicit`、`partial`、`native-default`。先用 `herdr --session <本次 session> agent list` 核对要替换的是本轮目标，再且仅再关闭它返回的 Tab：`herdr --session <本次 session> tab close <刚核对的 tab_id>`。不能照抄旧目标 ID、不关闭初始 shell 或用户其他目标。
3. 在 PowerShell 选择 `$a = 'pi'` 或 `'omp'`、`$s = 'explicit'`／`'partial'`／`'native-default'`，应用已准备的**专用 fixture 状态**：

   ```powershell
   $R = 'C:\Users\hugos\AppData\Local\Temp\resume-loading07-hia3_os4'
   $a = 'pi'                       # 第二项改为 omp
   $s = 'explicit'                 # 然后 partial，最后 native-default
   'bots.json','history.json','options.toml' | ForEach-Object {
     Copy-Item "$R\$a\scenarios\$s\$_" "$R\$a\$_" -Force
   }
   $b = "$R\$a\probes\$a-latest-b.ts"
   if ($s -eq 'partial') {
     if (Test-Path $b) { Move-Item $b "$b.available" -Force }
   } else {
     if (Test-Path "$b.available") { Move-Item "$b.available" $b -Force }
   }
   (Get-Content "$R\$a\loaded.jsonl").Count  # 记住本次起始行号
   ```

   这些拷贝只恢复本票隔离 fixture，绝不能应用于真实用户持久记录；它们不作为正常发起／来源持久化的证明。不要覆盖 `native.jsonl` 或清空累计日志。默认样例恢复 B 的可读性，是为了实际证明默认分支没有追加 Bot 的可用 fixed／B。
4. 刷新对应专用页面，进入历史列表，点该完整原生 ID 对应的 Resume 一次，等待权威结果。记录新返回的 Workspace／Tab／Pane 与页面完整降级反馈。`agent get <实际 pane_id>`、`agent read <实际 pane_id> --source recent-unwrapped --lines 180` 核对同一原生引用和实际加载历史；不要向 agent 发送任何输入来“证明”加载。
5. 查看 `Get-Content "$R\$a\loaded.jsonl" | Select-Object -Skip <刚才起始行号>`。只计本次新进程的 PID／同一原生 ID／文件／cwd 对应行，不把早先直接控制、故意 ambient 泄漏或旧选项正控制的行混入。与上表固定字面集合比较；官方 identity 另以 Herdr agent_session 及 Pi 的加载资源展示交叉观察。页面截图／HTTP 是反馈证据，回调是实际执行证据，二者不可互相替代。
6. 每种场景记录后再按步骤 2 替换本次目标；最后保留默认目标。只请用户分别确认：**H-Pi：这三组 Pi 真实加载集合与 ambient 对照符合表格／不符合；H-OMP：这三组 OMP 真实加载集合与 ambient 对照符合表格／不符合。** 如有差异保留本次输出与目标，不自动重试或向其他 Chat 输入。用户明确确认前，两项始终未验证。

### 专用进程退出时

先确认本次命名 session 是否仍运行；不要因为页面退出就重启／停止 Herdr。仅缺少本次专用 server 时，在两个独立终端分别设置本 agent 的 `HOME`／`USERPROFILE=$R\<agent>\home`、`PI_CODING_AGENT_DIR=$R\<agent>\agent`、`HERDR_CONFIG_PATH=$R\<agent>\herdr.toml`，清除继承的 `HERDR_SESSION`、`HERDR_SOCKET_PATH`、`OMPCODE` 及用户模型凭据，再启动 `herdr --session <表中 session> server`。不要全局改变环境或原生配置。

应用只需在同样隔离的对应终端启动：`& "$R\deployment\target\debug\herdr-bot.exe" --config "$R\<agent>\panel.toml"`。读取专用启动日志的新端口，不假设仍为 1594／1595。当前编译的静态资源路径是专用 deployment/static，不依赖原 worktree；重建命令是在 `$R\deployment` 下 `cargo build --bins`。

若临时目录已清理，`fixture-bundle.json` 保存了原生 seed 和所有文本 fixture，可先在**同名专用目录**重建其 `files`（将 `native.seed.jsonl` 复制为缺失的 `native.jsonl`，不覆盖任何现存对话），并从交付源码重新部署后构建页面／应用。缺少的真实前置条件是已安装相同版本 CLI／官方身份集成、隔离 cwd、独立部署或命名 session；不得借用用户现有 Chat 替代。完整原生文件身份来自 seed 的 session header（OMP 不是首行），不得按最近文件猜测。

## 本票实际运行的检查

- `frontend`: `npm ci && npm run build` —— 成功，交付页面构建完成。
- `backend`: `cargo build --bins` —— 成功。
- 独立 deployment 副本：`cargo build --bins` —— 成功；重开两专用应用后，从启动日志核实静态资源路径完全位于隔离目录，公开历史 HTTP 均 200。
- Eval 驱动真实交付页面与公开 HTTP／CLI：**6／6 场景**分别观察字面扩展集合、原生引用、实际 cwd、聚焦及页面结果；部分样例缺失反馈、默认样例原因均实际出现。
- 原生 RPC `get_state` 控制：两 agent 的三种集合均成功；旧 A 正控制与 ambient 泄漏校准各两项成功观察。RPC 没有提交 prompt。
- 初次浏览器 raw `tab.run` 的 click／screenshot 超时，没有触发 Resume；改用实际可工作的直接 tab helpers 后取得上述证据，已报告工具问题。首次材料断言还发现 OMP 正常 dispose 会写 exit 元数据；原始差异已解释保留，不把它误报为无新任务失败或悄悄改写原生文件。
- 未在本票重跑其他票自动回归；其实际命令／失败／ignored 以 [05 汇总及原始输出](integrity-05.md) 为准。**尚待：主代理完成全部自动检查门槛、独立整合、用户 H-Pi 与 H-OMP 两项明确确认。**
