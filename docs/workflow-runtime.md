# Map Run 运行说明

## 发起与观察

在 scheduler 选中 Map，打开「+ 发起 Run」，选择 manager 类型的 omp Task 与已登记 zone。Workspace 名称来自管理者 zone 的目录名，不退回默认或当前 Workspace。可先校验配置；「启动 Run」会重新执行同一整图预检，然后创建独立 Run。三份 Bot 文档允许空文本，不是业务质量门禁。

启动前请确认角色 Task 的固定扩展入口已在部署的 my-agent-toolkit 中交付并可加载、omp 未开启 autoResume，且管理者能按绝对路径读取 Map、本说明及运行记录。运行期间保持定义与 Task 配置不变。本服务不增加路径可读性探针，也不修改原生会话目录。缺失扩展入口会明确拒绝启动，不用空扩展代替。

管理者 Task 的固定 prompt 定义职能与限制；初始 Skill、model/thinking 和固定 prompt 按 Task 原配置生效。本次输入只提供 Run ID、Map 路径、本说明及三份记录的路径，最后以独立段落附上非空本次要求的原文。空要求不附段落。Bot 文档、完整 Map 正文与 node.input 不隐式注入管理者。

取得 Run ID 后自动进入 workflow 的基础只读详情，链接形如 `#workflow/<Run ID>`。刷新链接、切换入口后返回、手动刷新只读取事实，不创建 Run 或 Chat。本次要求不可编辑；详情中的 Run/node 状态不是 Chat 在线或轮次状态。此入口没有启动进度页、状态操作按钮或日志浏览器。

## 记录及配置

沿用服务 TOML 的 `[scheduler]`：

```toml
[scheduler]
map_directory = 'D:\service-data\maps'
bot_directory = 'D:\service-data\bots'
run_directory = 'D:\service-data\runs'
run_instructions = 'D:\herdr-bot\docs\workflow-runtime.md'
```

相对目录在服务启动时按服务工作目录解析一次；向 Chat 提供绝对路径，不按 Chat 的 zone/cwd 重新推导。默认 Run 根为 `.data/workflow-runs`。部署时应显式设置稳定的 `run_instructions`，不要依赖编译机器保留源码；默认指向随源码构建的本说明。

每个 Run 目录只有三份业务记录：

- `run.json`：当前已提交事实，包含 Map 引用、时间、管理者 Task/zone/Workspace、本次要求、管理者真实 Chat 关联及节点状态。
- `history.jsonl`：追加执行/状态历史；新 Run 初始为空。
- `messages.jsonl`：追加报告；新 Run 初始为空。

正常顺序为整图预检 → 三份记录全部初始化 → Workspace/Tab/agent 启动 → 获取原生 session_id → 持久保存 Run 的真实关联 → 提交初始指示。单文件更新使用同目录临时文件后替换，不先删除正式文件。不承诺三文件事务、断电恢复、多进程写入协议或外部 Chat 回滚。

## 基础管理者状态协议

`POST /api/workflow/manager/status` 的传输正文仅为 `{"sessionId":"<当前真实原生身份>"}`，返回 `snapshot`，内容来自当前已提交的 `run.json`，不保存第二套 ready 状态。角色扩展必须通过公共 Session 能力自动取得身份；这不是模型业务参数，模型不提供 Run ID、角色或归属。

后端先校验同机来源，再通过已保存的管理者原生身份解析唯一的当前后端 Run 归属；没有关联、重复关联、非管理者均拒绝，不按名称、Task、Tab 或 cwd 猜测。session_id 不是秘密认证凭据：接受同机进程冒用风险，不构成强认证或沙箱。重启后旧 Run 只读，普通 Chat Resume 不代表接管 Run。

status 除 `snapshot` 外返回派生的 `ready`（可派发 node ID）、`blocked`（每个 node 的当前阻塞原因数组）及 `canFinish`。依赖只在所有前置 node 为 completed 时满足；running、failed 等非 pending 节点不可首次派发。查询不保存第二套 ready 真相，不自动选择路线或创建 Chat。

### 管理者显式批量派发

`POST /api/workflow/manager/dispatch` 的传输正文为：

```json
{"sessionId":"<公共 Session 能力取得的当前真实身份>","nodes":["A","B"]}
```

模型业务参数只有目标 node ID 数组；不得让模型填写 sessionId、Run ID、角色或归属。后端按真实管理者关联解析本次 Run，逐项重新检查状态与依赖，返回 `results` 和与 status 相同的查询字段。每项包含 `nodeId`、`status`（`rejected`、`failed` 或 `running`）。拒绝项有 `reasons`，启动失败项有 `failure.stage/message/effects`；已发生的 Workspace、Tab、pane、agent、原生身份及指示尝试/确认均如实保留。`persistenceError` 表示失败事实未能落盘，`historyError` 表示当前事实已提交但追加历史失败；调用者不能把它们当作全量成功或自动重试理由。

每个合法目标沿 node → Bot → node 类型 omp Task 解析完整 trigger config 和 env config，在定义中仍登记且有效的 zone 创建独立原生 Chat。目标 Workspace 只取已保存的管理者实际 Workspace ID；目标不存在或查询失败即报错，不按目录名重选，不退回当前/默认 Workspace。原生身份必须不同于本 Run 的管理者及已有 node Chat，宿主返回的 cwd 必须与定义 zone 一致。真实关联保存前不提交初始指示；运行文件锁不跨宿主调用。

节点初始输入按原有 Skill → Task 固定 prompt → 本次 input 组合规则提交。本次 input 仅包含自身 node 标识/名称、运行说明及三份记录的绝对路径，最后保留 node.input 原文；不附 manager user_input、Bot 文档或完整 Map。node 应主动按需读取说明与记录，在自身 zone 工作；记录路径可以位于 zone 外，不按 cwd 推导替代路径，也不增加可读性门禁。node 不得通过自行退出、停轮或报告假定完成交接，流程推进由管理者显式决定。

同批拒绝或启动失败不取消其他合法目标，不关闭或回滚已创建的 Chat。成功派发后 node 为 running；启动失败为 failed，保留执行 ID、来源 Task、zone、可得 Chat 关联及失败阶段/原因，并追加执行历史。正常已记录的 running/failed 再次 dispatch 均拒绝，不增加外部启动、不覆盖首次事实。崩溃或关联保存失败窗口不承诺完整幂等，先核实已发生效果，勿盲目重发。同 Run 派发批次互斥；不同 Run 的身份与事实独立。

manager-chat 需要在独立 toolkit 中通过公共 `PromptDeclaration` 的 `first-enabled-run` 提供上述 status/dispatch 指引，并将工具接到上述协议。node-chat 同样通过公共 Prompt 能力给出节点职责与稳定路径读取指引。后端协议和页面不是该外部扩展已交付或真实注入已验证的证据。部署缺少 metadata 指定的任一角色扩展入口时先补齐实际扩展，不能用空函数、手填 sessionId 或 Tab 代替真实角色工具验收。

## 派发验证入口与证据边界

确定性派发检查入口为 `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch -- --nocapture`，完整后端交付检查为 `node scripts/test-backend.mjs`（先按测试指引构建前端）。这些测试使用真实 HTTP TestServer、每例独占的实际文件及外部记录型 Herdr 副本，覆盖关联提交屏障、完整字面触发输入、批内拒绝/失败/成功、重复拒绝、Run 隔离、磁盘故障与 Workspace 不回退。夹具进程记录实际 cwd，但它不是 omp 原生 Chat，也不验证模型或公共 Prompt 注入。

真实宿主复验还需部署 metadata 指定的 manager-chat/node-chat，使用独占验收 zone、数据目录与 Herdr 实例：经 manager 实际工具先查 A/B ready、C 被 A/B 阻塞，再派发 A/B；保存原生 session_id、实际 cwd 和 Workspace ID、实际输入及三份记录。与 Task 的独立预期逐字段核对 model/thinking、初始 Skill、固定 prompt 和 node.input；管理者本次要求与 Bot/Map 文档使用互异标记，必须不出现在 node 实际初始输入中。让验收 node Chat 读取传入的 zone 外说明/记录路径，而不以服务端自己可读代替。最后重复派发，观察外部无新增启动。角色扩展未部署或无法隔离宿主时，这部分仍是未验证，不用上述受控结果替代。

## 节点报告与独立通知

已关联的当前 node Chat 使用 `report`，模型业务参数**只有正文**（`body`）。正文应包含可交接结果的地址、使用方法与限制；不要求 node 自报 Run、node、执行 ID 或接收者。角色扩展须从 toolkit 公共 Session 能力取得当前原生身份，并调用：

```http
POST /api/workflow/node/report
Content-Type: application/json

{"sessionId":"<公共 Session 能力取得的当前真实身份>","body":"结果地址与使用限制"}
```

后端按当前后端已管理 Run 的真实 node 执行关联归属，拒绝无关联、管理者身份、重复关联及调用方填写的 `runId`、`nodeId`、`target` 等额外字段。系统生成报告 `id`、`at`、`type=report`，保存 `runId`、`nodeId`、`executionId`、真实来源 `source` 和同 Run 管理者 `target`。sessionId 不是模型参数，也不是秘密凭据；同机信任限制与 manager 工具相同。重启后的旧 Run 不接受 report。

每次成功提交都在 `messages.jsonl` 追加独立报告，包括同一执行的重复报告；不修改 `run.json` 或执行历史，不自动完成、暂停、重试或派发。暂停或已完成现场的当前关联 node 仍可补充事实，不重新打开已完成工作。报告记录缺失、损坏或尾部没有换行时明确报错，不当空文件重建、不截断后继续写。

保存完成后才向目标管理者真实 agent 提交一次 Herdr 输入，正文明确标出 Run、node、报告 ID 和报告正文。文件锁及 Run 操作锁不跨投递。Herdr 提交确认仅表示接入层确认输入提交，不表示已阅读、严格顺序、原生 IRC 或必然打断正在进行的工作。

返回结果分阶段解释：

- 保存失败：HTTP 500，`code=report-save-failed`、`saved=false`、`notification.status=not-attempted`；不生成报告待办、不调用 Herdr。
- 保存成功：HTTP 200，`saved=true` 及系统生成的完整 `report`；`notification.status` 分别为 `submitted` 或 `failed`，`reason` 说明提交结果。投递等待时，已保存报告已可直接读取，不必等待通知完成。
- 诊断单独返回 `diagnostic.status=written|failed`。诊断写入失败也不撤销报告、不重投、不暂停。不因诊断失败把已保存报告假报为保存失败。

**仅通知失败或诊断失败时不要重发 report。** 已保存事实仍在，管理者应直接读报告并显式处理交接；report、停轮和退出 Chat 都不代表 node completed。确有新的结果或需要更正正文时可以再 report，会产生新的独立历史，不覆盖旧记录。

Run 详情返回 `reports` 完整追加历史及 `pendingReports`（node ID → 最新适用报告 ID），manager status/dispatch 的快照同样提供这些字段。待办仅取当前执行、当前仍可交接完成的 node（running，或 execution 阶段 failed）；completed 不列为待办。按追加顺序取同一 node 当前执行的最新报告，不借旧执行完成新执行。连续读取和 Herdr 通知提交都不清除待办。这是保存记录的派生视图，不是第四份业务记录；collect 的等待与检查语义应由 collect 能力单独验收。

投递诊断使用 JSON stderr 记录 `event=workflow-report-delivery`、Run、报告 ID、完整目标 Chat 引用、`result` 及类型化 `reason`，不记录正文或可能回显正文的宿主错误详情。输入确认原因是 `input-submission-confirmed`，失败原因沿 Herdr adapter 错误类别。调用者仍从通知返回值取得具体错误。诊断不写入三份业务记录，也不成为重试队列。

### 节点公共 Prompt 的部署要求

node-chat 的实际工具与公共 Prompt 属于独立 my-agent-toolkit 交付。其 `PromptDeclaration` 应使用 `first-enabled-run` 注入本节报告职责：何时交接、正文写地址和限制、仅提供正文、报告不等于完成、保存与通知分离、仅通知失败不要重复 report；不以工具 description 代替注入。部署仍须将固定 node 角色入口接到公共 Session/HTTP/Prompt 与自动启用策略，配置稳定 endpoint 与预算，不依赖 node cwd 或 connect-bot 开关。本仓库的 HTTP 协议、静态入口配置或说明文字都不构成外部扩展已装配、原生工具实际调用或 Prompt 已注入的证据。

### 报告验证入口与证据边界

确定性入口：`cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch -- --nocapture`。报告用例覆盖系统归属、真实文件追加、只读文件保存失败、受控外部投递失败/延迟、连续读取保留待办和真实关闭 stderr 管道的诊断写入失败。投递/诊断故障后均观察 **300 ms**，输出该窗口的外部进程调用记录，证明有限窗口无补投或自动重报，不声称无限期性质。受控 Herdr 副本只证明 adapter 的确定性处理，不证明真实模型、原生会话或通知质量。

真实复验必须使用隔离 Herdr/omp 与已部署 node-chat：记录公共 Prompt 实际注入内容和原生 report 调用（业务参数仅 body），同时核对源/目标真实身份、当前执行、追加消息、状态不变和 manager 实际输入。制造保存失败、投递失败/延迟和诊断失败；分别保留输入提交结果与无自主重投窗口。若外部角色入口缺失，本仓库部分可通过，但 P24.01 的真实工具、P59.02 的公共 Prompt 及真实宿主通知证据仍未验证，不能用手填 HTTP 身份或受控 adapter 宣称这些项完整通过。

## 启动失败

初始化失败不启动 Chat。原生身份或关联保存失败不提前提交指示。已启动的 Tab/agent 不会被假报回滚；初始指示提交失败也不代表外部端绝无接收效果。详情列出可得的 Workspace、Tab、pane、agent 及原生身份，说明哪些步骤已确认完成。先核实这些引用，再处理现场；不要盲目再次发起，异常窗口不承诺完整幂等。

管理者启动错误只保存在当前后端内存，刷新仍能查询。诊断输出包含 Run、失败阶段及已发生效果，不重复本次要求正文。错误不写入业务历史；后端重启后不重建页面错误，只保留持久事实与既有诊断。没有自动重试、自动关闭 Chat 或自动派发节点。
