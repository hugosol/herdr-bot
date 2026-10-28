# 真实 A／B → C 验收与通知取证

这是实际角色工具、Herdr/omp（S3）、三份持久记录与实际文件（S4）的可重复场景，不是受控模型测试。单节点先行条件见 [运行说明](workflow-runtime.md#单节点真实宿主闭环验收)；页面构建、HTTP/进程与网络约束见 [测试指引](agents/testing.md)。执行者负责跑完并交证据，不将实现验证推给用户。用户最终只判断 H1 的实际使用效果。

**准备成功、文件存在、页面 completed 都不是本场景通过。** 未部署真实角色入口、不能隔离宿主或没有模型访问时，保存真实前置错误，停止在启动之前；不得用手填 sessionId、空角色入口、受控模型或手改运行记录代替。单节点真实闭环未通过时，本场景也不能解除该前置条件。运行时不修改 Map、Bot 或 Task 配置。

## 固定预期与可区分的违规

启动前固定以下字节（无 BOM UTF-8，只有 LF，最后一个 LF 必需）：

| 文件 | 完整字面内容（`\n` 表示一个 LF） |
| --- | --- |
| A zone 的 `material-a.txt` | `source=A\nitem=blue-widget\nunits=7\n` |
| B zone 的 `material-b.txt` | `source=B\nitem=blue-widget\nunit_price=6\n` |
| C zone 的 `combined.txt` | `item=blue-widget\nunits=7\nunit_price=6\ntotal=42\n` |

42 是事先算定的 7 × 6，不由服务、模型报告或实际产物重算预期。C 的 node.input **启动前**写入上述两份材料的绝对路径，只说明读取、合并和计算规则，不注入两份源文件正文或最终答案。C 的原生对话必须显示它实际读取两份指定文件的工具调用与结果，再写第三份文件；只看到正确结果不能证明材料使用。期望文件仅供验收者和 manager 核对，不能作为 C 的输入或替代材料。

- P62.01 的具体违规：C 只用一份材料、猜测结果或抄期望。实际读取两份固定路径的工具轨迹、三份产物完整字节与独立预期联合排除它；只有最终状态/报告无法区分。
- P63.01 的具体违规：等前置 Chat 退出才交接，或后端暗中派发 C。A 的报告后保留真实收尾活动，manager 的原生 complete → 显式 dispatch C → complete C → finish 工具结果与活动时间线联合观察；不把 running 节点状态冒充真实 Chat 活动。
- P34.01 的具体违规：把通知提交确认当作已读，或必须依赖 Herdr 通知成功才 collect。三个实际现场分别关联报告 ID、保存/投递结果、manager 输入和独立 collect 结果；不得从“模型随后做对”推断阅读或严格顺序。

## 先准备独占目录与静态定义

以下 PowerShell 在仓库根目录执行，只准备输入与独立预期，**不启动服务、Chat，不生成运行记录或实际产物**。目录每次随机新建；保留输出中的 root。四个 zone 与 records 均分离，gate 文件只是验收协调，不是业务记录。三个 node 共享一个 Bot/节点 Task，但有独立 cwd 与 Chat。

```powershell
$ErrorActionPreference = 'Stop'
$root = Join-Path ([IO.Path]::GetTempPath()) ('herdr-fan-in-' + [guid]::NewGuid())
$utf8 = New-Object Text.UTF8Encoding($false)
$paths = @{}
foreach ($name in @('service-cwd','manager','A','B','C','maps','bots','runs','gates','expected','evidence')) {
    $paths[$name] = Join-Path $root $name
    New-Item -ItemType Directory -Path $paths[$name] | Out-Null
}
$a = Join-Path $paths.A 'material-a.txt'
$b = Join-Path $paths.B 'material-b.txt'
$c = Join-Path $paths.C 'combined.txt'
[IO.File]::WriteAllText((Join-Path $paths.expected 'material-a.txt'), "source=A`nitem=blue-widget`nunits=7`n", $utf8)
[IO.File]::WriteAllText((Join-Path $paths.expected 'material-b.txt'), "source=B`nitem=blue-widget`nunit_price=6`n", $utf8)
[IO.File]::WriteAllText((Join-Path $paths.expected 'combined.txt'), "item=blue-widget`nunits=7`nunit_price=6`ntotal=42`n", $utf8)
$gateRule = "等待文件只读检查，每次最长 180 秒，超时明确停止并报告，不制造 gate、不静默重试。gate 目录：$($paths.gates)。所有报告使用实际 node 角色工具；投递失败不为同一事实重报。不要读取 expected 目录。"
$map = [ordered]@{
    title = '真实双材料汇合验收'
    description = 'A/B 独立并行，只有 A → C 和 B → C。不是执行结果。'
    nodes = [ordered]@{
        A = [ordered]@{
            title = '数量材料与报告后收尾'; bot_id = 'fan-in-worker'; zone = $paths.A; blocked_by = @()
            input = "写入 $a，无 BOM UTF-8、LF、末尾 LF，完整正文为 source=A\nitem=blue-widget\nunits=7\n（将 \n 写为 LF）。读取核对。等待 working-report.release 后 report 材料地址/编码/限制，说明还在收尾。然后等待 collect-report.release，再 report 一条新的事实：材料不变、收尾尚未结束。随后用实际工具等待 tail.release，期间不改交接材料，释放后写 tail-finished.txt 记真实 UTC 时间并结束。$gateRule"
        }
        B = [ordered]@{
            title = '单价材料'; bot_id = 'fan-in-worker'; zone = $paths.B; blocked_by = @()
            input = "写入 $b，无 BOM UTF-8、LF、末尾 LF，完整正文为 source=B\nitem=blue-widget\nunit_price=6\n（将 \n 写为 LF）。读取核对。等待 idle-report.release 后 report 材料地址/编码/限制；之后可结束。$gateRule"
        }
        C = [ordered]@{
            title = '读取双材料并汇合'; bot_id = 'fan-in-worker'; zone = $paths.C; blocked_by = @('A','B')
            input = "必须用真实读取工具分别读取 $a 和 $b；不得从 expected、运行说明或报告正文抄材料。核对两份 item 一致，从 A 取 units、B 取 unit_price，相乘得 total。写入 $c，无 BOM UTF-8、LF、末尾 LF，依次为 item=值、units=值、unit_price=值、total=值四行。读取核对，再 report 地址/两个来源/编码/限制。$gateRule"
        }
    }
}
$bot = [ordered]@{
    id = 'fan-in-worker'; name = '双材料验收节点'
    when = '只用于独占真实验收目录。'
    plan_guide = '按 Map 输入派发，依赖不会自动传递材料。'
    node_guide = '按自身输入实际读写材料并使用真实角色 report。report、停轮和退出均不等于 completed。'
}
[IO.File]::WriteAllText((Join-Path $paths.maps 'fan-in.json'), ($map | ConvertTo-Json -Depth 8), $utf8)
[IO.File]::WriteAllText((Join-Path $paths.bots 'fan-in-worker.json'), ($bot | ConvertTo-Json -Depth 8), $utf8)
$managerInput = @"
仅执行本次 A/B→C 验收。先读本轮固定 Map 与运行说明，status 核对 A/B ready、C 被 A/B 阻塞；同一显式 dispatch 批派发 A、B，检查各项真实结果。先不要 complete。
为工作中通知现场，用实际工具等待 $($paths.gates)\working-hold.release（最长 180 秒，超时报错停止），不创建 gate。释放后查报告和材料，再有意停轮，等待验收者确认空闲通知现场。任何人工要求继续都保留记录，不保证自主续轮。
继续后显式 collect 排空既存通知并保留结果；再调用一次 collect 等待新报告。验收者会在确实进入等待后释放 collect-report.release。collect 失败不伪造快照、不自动重试；Herdr 提交不是阅读凭据。
三种现场留证后，读取 $a 与 $b 并逐字节对照 $($paths.expected) 中各自固定预期。确认可交接后 complete A、B，各写非空依据；A 尚在实际收尾不妨碍交接。随后显式 dispatch C。核查 C 真实读取两个指定材料的轨迹及 $c 字节与固定预期；report 本身不算完成。显式 complete C，再 finish。不要等待 A 的 tail.release 才交接或 finish，也不要创建任何 gate。出错如实停止说明，不重派/重报/改运行文件。gate 与工具等待超时应保留，不宣称强制持续执行。
"@
[IO.File]::WriteAllText((Join-Path $root 'manager-input.txt'), $managerInput, $utf8)
[ordered]@{
    root = $root; map = (Join-Path $paths.maps 'fan-in.json')
    managerInput = (Join-Path $root 'manager-input.txt')
    actualArtifactsExist = @((Test-Path $a), (Test-Path $b), (Test-Path $c))
    runFiles = @(Get-ChildItem -LiteralPath $paths.runs -File | Select-Object -ExpandProperty Name)
} | ConvertTo-Json -Depth 4
```

这些定义未绑定 Task，不能启动。前置检查通过后，在隔离服务的实际页面登记 manager/A/B/C 四个 zone；创建 manager/node 类型的 omp Task，将 Bot 绑定 node Task。保存完整 trigger/env 配置、model/thinking、初始 Skill、prompt、扩展及定义的启动前快照。manager 本次要求使用生成的 `manager-input.txt`。不得把 expected 文件当作实际产物。

## 部署门与启动

1. 读取**本轮实际 metadata** 中 manager/node 固定入口并用 `Test-Path -LiteralPath <实际路径> -PathType Leaf` 检查；还要验证真实加载和公共 Prompt/Session/HTTP 能力。路径存在只是门的第一步。缺失时记录命令、时间、metadata hash 和结果，不启动，不改指向替身。检查 connect-bot 关闭仍能用角色工具，omp 未开 autoResume。
2. 明确建立本轮独占 Herdr 实例/命名 session 与模型访问；查清生产 adapter 如何指向该实例，无法证明隔离时停止，不能试探默认用户实例。缺少角色部署不是已证明 Herdr 或模型故障。
3. 按运行说明单节点步骤先取得真实闭环证据；上游未完成则记录前置未满足。服务与 agent stdout/stderr 持续消费，端口使用数值回环动态分配，关闭 mDNS，不使用真实关机、LAN 或符号链接。
4. 按[静态样例隔离配置](example/README.md#在隔离目录运行)和运行说明建立本轮独占配置：cwd 为 service-cwd，Map/Bot/Run 指向上述目录，Task、zone、历史持久路径全部归 root；明确配置稳定运行说明路径和角色 endpoint、collect 期限/较长请求预算。不能仅将 TOML 放到 Temp 却继承用户数据。通过 npm 浏览器入口构建交付页面与安全壳，不绕过网络护栏。
5. 从 scheduler 页面发起本轮 Run，保存 Run ID、页面截图与实际响应。manager 实际角色工具查 status，再显式派发 A/B。检查原生身份互不相同、两个 node cwd 符合定义、Workspace 为 manager 实际 Workspace。记录两者实际活动区间的交叠，不把批请求本身当作并行证据。

## 三个真实通知现场（P34.01）

gate 仅由验收者创建于本轮 gates 目录，每次创建的 UTC 时间、命令、操作者和目的都计入**验收协调/干预记录**。不能把 gate 协调包装成系统自主推进。每个现场保存起止，不用任意 sleep 推断已经进入状态。

| 现场 | 到达条件与动作 | 必须观测的结果 |
| --- | --- | --- |
| manager 工作中 | 实际原生 Chat/Herdr 显示 manager 正在执行 working-hold 的等待工具。确认 A/B 已派发并实际工作后，创建 `working-report.release`，使 A 的首次真实 report 到达；先取证再创建 `working-hold.release`。 | A report 原始结果中的保存成功、报告 ID、Herdr submission 结果；同一 ID 在消息记录和 manager 实际输入中的关联；输入提交时 manager 仍有工具活动。不得要求通知必然打断它。 |
| manager 空闲 | manager 按要求实际停轮后，保存原生回合结束/Herdr 空闲事实，再创建 `idle-report.release`，触发 B 的 report。 | B 的独立报告 ID、提交结果和 manager 输入。通知可能使 manager 续轮，也可能仍需补充输入；实际观察原样记录。未真正空闲时不得拿此报告充数。 |
| collect 等待 | 先用真实 manager collect 消费既存通知，保留结果；新一轮 collect 的工具/HTTP 开始事件已观察到，且仍未返回时，创建 `collect-report.release`，使 A 追加新的收尾事实报告。 | 新报告 ID、collect 的 notification/报告标识/快照、report 的保存与投递结果分别保存。保留两条真实调用起止，确认 collect 观察保存事实不以 Herdr 提交/模型阅读为凭据。既存通知导致即时返回不算本现场，应明确重建等待，而不是改结果。 |

若通知自动续轮导致错过空闲或 collect，记录失配及补充自然语言指示，再从真实状态重建对应现场；不能伪造状态或删除报告。每份报告保留独立 ID，重复报告只是新的事实，不用于盲目补投。任一现场没有达到、失败或被取消应单列为未完成，不能把别的两类通知推断为它通过。正常集成没有制造 Herdr 失败时，只证明本轮实际提交结果，不额外声称已证明投递失败情形。

## 显式交接、产物与活动顺序（P62.01/P63.01）

三个现场后，由 manager 亲自读 A/B 实际文件并与启动前预期比较完整字节。仅报告不解除 C 的依赖；manager 显式 complete A/B 后才能显式 dispatch C。保留各动作**原始工具参数、结果、真实会话身份与时间**，而不是只抄一句“成功”。C 的输入和定义在发起前已保存，不临时追加材料地址。

A 在第二份报告后要实际执行等待 tail.release 的收尾工具。验收者应在 complete A/B、C dispatch、finish 的时点分别保存 A 的原生活动/工具起止证据。若 A 超时停轮，窗口已失效，不能凭 node running 或未退出进程宣称“仍在收尾”；记录超时与任何继续指示，重新取得真实活动证据。gate 的 180 秒是有界测试协调窗口，不是系统持续执行保证，也不是产品 collect 默认期限。

C 实际读取两份指定材料、写入并读取 combined.txt 后 report；manager 核对完整字节后 complete C、finish。在确认 finish 已提交后验收者才释放 `tail.release`，保存 A 的真实工具结束与 `tail-finished.txt` 时间。目标顺序为：A 首次 report → A 继续活动 → manager complete A/B → manager 显式 dispatch C → C 实际读取/产物/report → complete C → finish → A 收尾结束。跨来源时钟不可比时，用调用开始/结束与 gate 释放的因果关联说明，不强拼未经证实的全序，更不声称通知严格有序。

字节核对可复用以下片段（`$root` 为本轮准备输出）。这是读取实际文件，不会补建它们；缺文件、BOM、CRLF、内容差异均失败。不要用实际产物或 report 重写 expected 后再比较。

```powershell
$ErrorActionPreference = 'Stop'
foreach ($pair in @(@('A','material-a.txt'), @('B','material-b.txt'), @('C','combined.txt'))) {
    $actual = Join-Path (Join-Path $root $pair[0]) $pair[1]
    $expected = Join-Path (Join-Path $root 'expected') $pair[1]
    if ([Convert]::ToBase64String([IO.File]::ReadAllBytes($actual)) -cne
        [Convert]::ToBase64String([IO.File]::ReadAllBytes($expected))) {
        throw "实际字节不符：$actual"
    }
    Write-Output "字节吻合：$actual"
}
```

## H1 交接与证据完整性

证据包须保留以下**实际存在**内容及其路径/hash；缺失写明缺失，不生成空 Chat、空运行 log 或伪造截图充数：

- 启动前预期、Map/Bot/Task/本次要求、配置与版本、精确启动命令；实际模型/扩展启用与两种角色 Prompt 注入证据。
- Run ID 与 manager/A/B/C 原生 Chat ID、执行 ID、cwd、Workspace/Tab/pane/agent 关联；可复查完整原生对话与工具结果，包括 C 两次真实读取、三个通知现场的保存/提交/collect 关联。
- A/B/C 三份实际产物的原始字节、固定预期与完整比较输出；A 收尾工具/标记的实际证据，不能用准备文件替代。
- 服务生成的原始 `run.json`、`history.jsonl`、`messages.jsonl`，保留 complete/dispatch/finish 与报告标识。只读复制，不手改；任何当前状态保存失败、历史部分生效、投递失败分别记录。
- scheduler 发起结果、workflow 图和选中 A/B/C 详情、报告/完成说明、真实 Chat 入口及最终状态的实际页面截图。页面通过 npm 浏览器入口观察，HTTP 返回不能代替页面证据。
- 每次停轮、人工输入、gate、取消、重试意图、操作对象/时间/原因/结果。没有干预时才明确记录没有；本方案有 gate 协调，不能宣称零人工介入。任何被排除自动行为的检查都注明起止、窗口及全部外部调用，有限窗口不证明无限期性质。
- 正常和异常退出都清理本轮所属服务/agent/Herdr 资源并保存清理结果；不关闭用户已有 Workspace 或对话，保留证据后才清理 root。

Agent 报告分别列 P34.01、P62.01、P63.01 的实际证据和未完成项；只有全部实际通过才能关闭实现验证。**H1/P69 另列人工待判断**：用户检查对话推进、三类通知及全部干预是否符合批准预期。没有用户确认不得代用户宣布 H1 通过；H1 待判断也不能替代上述缺失的真实验证。
