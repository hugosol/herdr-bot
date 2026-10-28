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

manager-chat 需要在独立 toolkit 中通过公共 `PromptDeclaration` 的 `first-enabled-run` 提供 status/dispatch 及下文 complete/finish 指引，并将工具接到对应协议。node-chat 同样通过公共 Prompt 能力给出节点职责与稳定路径读取指引。后端协议和页面不是该外部扩展已交付或真实注入已验证的证据。部署缺少 metadata 指定的任一角色扩展入口时先补齐实际扩展，不能用空函数、手填 sessionId 或 Tab 代替真实角色工具验收。

## 派发验证入口与证据边界

确定性派发检查入口为 `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch -- --nocapture`，完整后端交付检查为 `node scripts/test-backend.mjs`（先按测试指引构建前端）。这些测试使用真实 HTTP TestServer、每例独占的实际文件及外部记录型 Herdr 副本，覆盖关联提交屏障、完整字面触发输入、批内拒绝/失败/成功、重复拒绝、Run 隔离、磁盘故障与 Workspace 不回退。夹具进程记录实际 cwd，但它不是 omp 原生 Chat，也不验证模型或公共 Prompt 注入。

真实宿主复验还需部署 metadata 指定的 manager-chat/node-chat，使用独占验收 zone、数据目录与 Herdr 实例：经 manager 实际工具先查 A/B ready、C 被 A/B 阻塞，再派发 A/B；保存原生 session_id、实际 cwd 和 Workspace ID、实际输入及三份记录。与 Task 的独立预期逐字段核对 model/thinking、初始 Skill、固定 prompt 和 node.input；管理者本次要求与 Bot/Map 文档使用互异标记，必须不出现在 node 实际初始输入中。让验收 node Chat 读取传入的 zone 外说明/记录路径，而不以服务端自己可读代替。最后重复派发，观察外部无新增启动。角色扩展未部署或无法隔离宿主时，这部分仍是未验证，不用上述受控结果替代。

## 显式交接完成与结束 Run

管理者检查结果确实可交接后才使用 `complete`，不必等执行 Chat 停轮或退出。管理者工具的业务参数只有 `nodeId` 与 `reason`；`reason` 必须为非空说明，应记录接受的结果、地址及限制，原文保留。传输身份仍由公共 Session 能力自动取得，不让模型或用户手填归属：

```http
POST /api/workflow/manager/complete
Content-Type: application/json

{"sessionId":"<当前管理者真实身份>","nodeId":"A","reason":"结果已可交接；清理工作仍在原 Chat 继续"}
```

Run running 或 paused 时，running node 可以完成；人工已处理并确认可交接的执行阶段 failed 也可完成，但 pending 或派发阶段 failed 不能借 complete 跳过工作或核实。完成不产生报告、不补发材料、不结束原 Chat、不自主派发下游。所需结果地址与限制仍应通过原 Chat 的 report 补齐，后端不额外判断结果质量。A/B → C 中只完成 A 不够：C 仍由 B 阻塞；两者均 completed 后，管理者须另行显式 dispatch C。

成功后返回 `changed=true`、`snapshot`、`ready`、`blocked`、`canFinish`、`reports` 和 `pendingReports`。说明与操作 ID、时间、真实管理者 `actor` 保存在 `snapshot.nodes.<nodeId>.completion`，状态与说明在同次当前事实写入中提交；对应 history 的 `action=complete` 含前后状态、说明与执行引用。同 Run 动作与整个派发批次串行，动作快照在该边界内生成，后续动作仍检查最新事实。completed node 不再列入可操作报告待办；补充 report 不重新打开节点或 Run。

Run 尚未结束时重复 complete 返回 `changed=false`，不覆盖首次说明、时间、操作 ID 或成功历史。拒绝返回 HTTP 409，并附 `code`、原因、`changed=false` 和当前查询字段；例如 `node-not-completable`、`completion-reason-required`、`node-unknown` 或 `run-terminal`。无关联身份仍按管理者权限拒绝，不接受额外 Run/归属字段。

全部节点 completed 后，管理者显式结束本次 Run：

```http
POST /api/workflow/manager/finish
Content-Type: application/json

{"sessionId":"<当前管理者真实身份>"}
```

finish 没有模型业务参数。running 或 paused 的 Run 全部节点 completed 才能成功；仍有未完成节点返回 HTTP 409、`code=nodes-incomplete`，不自动完成它们。成功返回 `changed=true` 及同样的查询字段，Run 变为 completed，`snapshot.finished` 保留首次操作 ID、时间与真实管理者，history 追加 `action=finish`。`canFinish` 表示**当前仍可执行 finish**，所以终态为 false；重复 finish 仍返回已有成功事实、`changed=false`，不追加历史。终态可查询、可接收当前 node 的补充报告，但拒绝 complete 和新 dispatch；dispatch 拒绝项不会调用宿主。活动中的 Chat 不阻止完成或 finish，也不被这些动作强制结束。

当前事实保存失败返回 HTTP 500、`complete-failed` 或 `finish-failed`，不采用候选状态或追加成功历史。当前事实已保存但历史追加失败时，仍返回已生效快照与非空 `historyError`，不要当作全部未生效或盲目重试。三份记录不承诺跨文件事务；重复调用也不会修复或重复追加首次缺失的历史。

manager-chat 的公共 `PromptDeclaration` / `first-enabled-run` 应将 status → 读取报告/核实交接 → complete → 显式 dispatch 下游 → 全部完成后 finish 的职责与上述拒绝/部分生效语义注入真实管理者 Chat。node-chat 指引应明确 report、停轮、退出都不替代管理者交接。本仓库没有外部 toolkit 的角色工具实现；HTTP 路由与此说明不是工具已装配或 Prompt 已注入的证明，不通过手填 HTTP 身份冒充真实角色工具验收。

## 失败核实、原执行恢复与人工交接

失败不等于 Chat 已退出，更不等于运行应暂停。后端不会因派发或执行失败自动 retry、pause、终止 Chat、补发初始指示或传递材料。先读取 status、原 Chat 与三份记录，再请用户核实或修复现场。Run running/paused 均允许以下非启动动作；completed 终态全部拒绝（HTTP 409、`run-terminal`），保留首次 finish 事实。

### 记录执行失败：fail

`POST /api/workflow/manager/fail`：

```json
{"sessionId":"<公共 Session 能力取得的当前管理者身份>","nodeId":"A","reason":"已核实：缺少上游源文件，原 Chat 暂不能继续"}
```

只有 running node 可首次 fail；`reason` 必须非空。成功保存 `status=failed`、`failure.stage=execution`、`failure.message` 和原执行 ID、Task、zone、真实 Chat，不改变 Run 派发资格。执行失败不凭空生成启动效果字段；派发失败的 `failure.effects` 仍保留启动时观察到的实际效果。history 的 `action=fail` 包含首次原因、前后状态、执行引用、管理者 actor、操作 ID 与时间。重复 failed 的 fail 返回 `changed=false`，不覆盖首次失败、当前文件或历史，不调用宿主；它也不能把派发失败改成执行失败。pending/completed 首次 fail 返回 `node-not-failable`，空原因返回 `failure-reason-required`。

### 只核实派发失败：reconcile_dispatch

`POST /api/workflow/manager/reconcile_dispatch`：

```json
{"sessionId":"<当前管理者身份>","nodeId":"A","finding":"uncertain","reason":"用户尚不能确认原派发是否创建了 Chat"}
```

模型业务参数仅 `nodeId`、`finding`、`reason`；没有任意目标状态或调用者自报 Chat 归属。只有派发阶段 failed 可核实；执行阶段 failed 必须 recover 或确认可交接后 complete，返回 `node-not-dispatch-failed` 防止绕过。非空原因必须说明人工核实依据。

- `finding=uncertain`：当前事实完全保持 failed，保留原失败原因与执行。`changed=false`，history 追加这次不确定核实及依据，但不触发宿主调用或派发。
- `finding=not_created`：人工确认未创建原生 Chat，恢复 pending，清除当前失败/候选执行；旧执行、原失败与依据仍在 history。已记录真实 session_id 或 Chat 关联时拒绝 `dispatch-chat-already-known`，不能用此动作抹除已发生事实。pending 只表示再次有资格，不自动 dispatch；已有 Tab 也不会被关闭。
- `finding=created`：用户确认原 Chat 存在并可跟踪，后端只对**原派发效果中记录的 agent** 调用只读原生身份查询。身份必须为真实 omp 会话、cwd 与原 zone 一致，不得替换已知原身份或复用管理者/其他节点身份；关联原执行的 Chat 后恢复 running。原执行 ID、Task、zone、Workspace/Tab/pane/agent 引用不变，不创建 Chat、不提交初始指示、不补发遗漏材料。缺少原派发现实目标或宿主身份不可确认时返回 `dispatch-chat-unverified`，身份/zone 冲突返回 `dispatch-chat-mismatch`，保持 failed。无已知原派发目标时不能通过猜测或直接编辑记录强行关联。

每次成功核实的 history `action=reconcile_dispatch` 保留 `finding`、依据、原 failure/执行和当前执行引用；确认后当前 failure 清空，但失败历史不删除。created 查询失败不伪造成功，也不自动再查或启动。

### 恢复原执行跟踪：recover

`POST /api/workflow/manager/recover`：

```json
{"sessionId":"<当前管理者身份>","nodeId":"A","confirmed":true,"reason":"用户已补齐源文件，确认原 Chat 可继续"}
```

业务参数仅 `nodeId`、`confirmed`、`reason`。只有执行阶段 failed 且有关联原 Chat 才能 recover；派发失败返回 `node-not-recoverable`。人工确认 `confirmed=true` 与非空修复依据缺一不可，否则返回 `recovery-confirmation-required`。成功将原执行跟踪恢复 running、清空当前 failure；原执行 ID/Task/zone/Chat 全部不变，history 保留原 failure、修复说明和 `action=recover`。这不是 Chat Resume，不调用宿主、不重新发起或恢复进程、不补发消息；即使 Run paused 也可执行，不恢复派发资格。已恢复 running 后再次 recover 拒绝，不重复追加成功记录。

### 人工已处理且确实可交接

用户已人工补齐结果且确认可交接时，管理者可对执行阶段 failed 显式调用上文 `complete`，用非空 reason 写清结果地址、限制与人工处理依据。pending/派发 failed 不能这样跳过核实。该动作只完成状态交接、解除依赖，不产生 report、不补发材料、不启动下游或结束原 Chat；失败和原执行历史仍可读。需要向管理者补充正文时，在**已有关联的 node Chat** 使用现有 report 写明地址与限制；completed 后补充报告不重新打开待办。管理者须在后续显式 dispatch 下游时自行提供所需材料，不依赖新增消息工具或自动传播。

### 错误、调用者与用户处理路径

上述 HTTP 只接受同机、唯一当前管理者真实 session_id；角色扩展必须通过公共 Session 能力自动取得它，模型和用户不填写身份/Run/角色/归属。未知字段、无效 finding 或缺少必要参数明确拒绝，不存在任意状态后门。成功返回与 complete 相同的查询字段、`changed`、`historyError`。状态写入失败返回 HTTP 500、`fail-failed` / `recover-failed` / `reconcile-dispatch-failed`，不采用候选状态或追加成功历史；状态已提交但历史追加失败时保留生效快照并返回非空 `historyError`，不假称全部未生效或自动重试。

用户从 workflow 查看 Run/node，进入已关联管理者 Chat 表达意图，而不是手填工具参数或编辑运行文件：

1. 派发失败：查看已发生效果与原 Chat → 必要时普通 Resume 进入已存在 Chat → 核实「已创建 / 未创建 / 不确定」→ 将结论和依据告诉管理者，由其 reconcile_dispatch。不确定继续 failed，不盲目重复 dispatch。
2. 执行失败：确认原 Chat 和当前原因 → 人工修复现场 → 告诉管理者「已修复，继续跟踪原执行」，由其 recover；需要恢复原生 Chat 进程时另用普通 Resume，它不代替 recover 或接管 Run。
3. 人工处理后已可交接：告诉管理者结果地址、限制与确认依据，由其 complete；原 Chat 可用现有 report 补充，不制造假报告。下游只获得资格，管理者必须再显式决定派发。

manager-chat 的公共 `PromptDeclaration` / `first-enabled-run` 必须包含上述动作矩阵、人工确认责任、禁止自动行为与错误/部分生效语义，node-chat 必须明确 report 不改变状态、失败后补充材料仍使用已有 Chat。**本仓库没有外部 toolkit 角色工具源码**，当前 HTTP 接线与说明不代表实际角色工具或 Prompt 已交付。metadata 指向的 `manager-chat.ts` 入口未部署时，真实工具/Herdr/omp 演练仍受阻；不能让用户手填 sessionId 或用受控原生身份宣称已验收。

### 可重复验证与证据边界

```text
cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch recovery:: -- --nocapture
node scripts/test-backend.mjs
```

`backend/tests/workflow_dispatch/recovery.rs` 通过真实 HTTP TestServer、独占磁盘及外部记录型 Herdr 副本检验：

- `execution_fail_preserves_original_chat_first_reason_and_history_without_host_actions`：P20.04、P38.02，首次原因/原身份/查询/磁盘一致、重复 fail 不生效。
- `dispatch_reconciliation_keeps_uncertain_failed_then_confirmed_absence_pending` 和 `created_dispatch_reconciliation_observes_original_host_identity_without_start_or_input`：P39.01 的不确定/未创建/已创建三分支，原失败历史可读，不伪造提交效果。
- `confirmed_recover_restores_original_execution_while_paused_without_relaunch`：P40.01，人工确认前提、paused 可恢复原跟踪且身份不变。
- `manual_failed_execution_handoff_unlocks_dependency_without_fabricating_or_forwarding_reports`：P42.01，执行失败不可借核实重写，显式交接才解除依赖，无假报告或自动传材料，原 Chat 可补充地址/限制。
- `terminal_run_rejects_failure_reconciliation_recovery_and_manual_handoff`：P35.03，通过真实动作到达终态后拒绝本票全部变更，首次终态事实与外部调用不变。
- `reconciliation_rejects_unverified_or_conflicting_chat_facts_without_mutation`、`failure_actions_reject_wrong_role_unknown_fields_and_invalid_transitions` 和 `failure_actions_do_not_publish_uncommitted_state_or_success_history`：真实身份观察冲突、权限/参数/状态准入及真实只读文件错误反馈。

P68.06 由上述失败、三种核实、recover、人工 complete 的真实 adapter 全部调用记录支持：正常/拒绝/错误处理及之后 **150ms 有界观察窗口**无新增启动、终止、输入或自动重试；created 仅允许明确的只读 `agent_get`，测试中的显式原 Chat report 则只允许通知管理者。该证据不证明无限期无动作，也不证明真实 omp、模型决策或角色 Prompt 注入。

真实 S3 复验必须先部署 metadata 指定的角色入口并隔离 Herdr、zone、数据与原生 Chat，再由实际管理者工具分别处理三种核实、paused recover、人工 complete，保存原身份、输入/启动/终止记录、三份运行记录、明确起止的无新增动作观察窗口和用户意图/人工干预。缺入口或不可隔离宿主时报告未验证；HTTP 与受控 adapter 检查不能替代它。

## 暂停新执行与恢复派发资格

管理者的 `pause` / `resume` 都没有模型业务参数。传输正文仅由公共 Session 能力提供真实身份，后端仍按唯一管理者关联与同机来源检查；node、未关联身份及额外的 Run/node/角色字段不能借此修改状态：

```http
POST /api/workflow/manager/pause
Content-Type: application/json

{"sessionId":"<公共 Session 能力取得的当前管理者身份>"}
```

`resume` 使用相同正文，路径为 `POST /api/workflow/manager/resume`。不要把普通 Chat 的 **Resume**（定位或恢复原生对话）当作此状态动作；普通 Resume 不接管 Run，也不恢复派发资格。

- `pause` 将 running Run 转为 paused，排在同 Run 已接受的完整派发批次之后生效。它不关闭或终止任何 Chat、不等待它们停轮或退出。生效后 `ready=[]`，新 dispatch 的逐项结果为 `rejected`，原因含 `Run 状态为 paused`，不产生宿主启动调用。
- 已有执行仍可 report，管理者仍可在确认可交接后 complete。报告仍只通知管理者，不自动完成。paused Run 全部节点 completed 后可以直接 finish，无需先 resume，活动 Chat 不阻止成功。
- `resume` 只将 paused Run 转回 running，重新给出由当前事实与依赖派生的 `ready` / `blocked`。它不自动 dispatch、retry、补发输入或终止已有 Chat；管理者必须另外显式选择目标并 dispatch。
- 已在目标状态时重复 pause/resume 返回 `changed=false` 与当前查询字段，不重写当前记录、不覆盖首次操作 ID/时间/actor、不重复追加成功历史，也不产生外部调用。一次真正的反向转换后再次调用是新的显式状态动作，不是重复请求。
- completed 终态的 pause/resume 返回 HTTP 409、`code=run-terminal`、`changed=false` 和当前查询字段，不改变首次 finish 事实。重复 finish 仍返回首次结果。

成功动作返回与 complete/finish 相同的 `snapshot`、`ready`、`blocked`、`canFinish`、`reports`、`pendingReports`、`changed` 与 `historyError`。当前状态在 `run.json`，首次生效转换在 `history.jsonl` 中保留 `id`、`at`、真实管理者 `actor`、`action=pause/resume`、`before`、`after`。所有检查、写入和返回快照沿用同 Run 动作串行边界；文件锁不跨宿主调用。当前事实保存失败返回 HTTP 500、`pause-failed` / `resume-failed`，不采用未提交状态、不追加成功历史；历史追加失败则如实返回已生效状态和非空 `historyError`，不回滚、不自动修复或重试。

### 用户与管理者操作指引

人类需要处理现场时，先从 workflow 查看 Run 与节点事实并进入已关联的管理者 Chat，表达「暂停新派发」；由管理者调用 pause，而不是退出所有节点 Chat、关闭 Tab 或直接编辑运行文件。已有 node 可以继续提交含结果地址与限制的 report，管理者可以确认 complete；暂停不表示工作已经完成。处理后表达「恢复派发资格」，由管理者 resume、重新读取状态并决定下一次显式 dispatch。不要将 resume 工具成功当作下游已启动。若全部工作已交接，直接 finish，不必为结束而恢复派发。

manager-chat 的公共 `PromptDeclaration` / `first-enabled-run` 必须覆盖上述边界及重复/终态/部分生效反馈；node-chat 应明确 pause 不撤回其已有工作或报告职责。这是角色指引的交付要求，不是本仓库已交付外部工具或已验证真实 Prompt 注入的声明。当前 worktree 只交付 HTTP 能力和本文；metadata 指向的外部角色入口未部署时，真实角色工具与 Herdr/omp 链路仍未验证，不能让用户手填 sessionId 代替。

### 暂停与恢复的验证入口及证据边界

可重复检查为 `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch -- --nocapture`，完整后端检查为 `node scripts/test-backend.mjs`。`pause_blocks_new_dispatch_but_preserves_active_chat_handoff_and_first_fact` 覆盖 P18.03、P20.03 的 pause 分区、P36.01 和 P68.04 的 pause 分区；`resume_restores_only_eligibility_until_explicit_dispatch_and_preserves_first_fact` 覆盖 P20.03 的 resume 分区、P37.01 和 P68.04 的 resume 分区；`paused_run_finishes_with_active_chat_and_terminal_eligibility_actions_are_rejected` 覆盖 P35.02。测试复用真实 HTTP TestServer、独占临时记录及外部记录型 Herdr 副本，先实际派发/暂停，再尝试 dispatch、report、complete，并实际 resume 后观察，最后显式派发；另一路径在 paused 下直接结束。输出保留动作/查询、历史和真实外部调用列表。每个无自主行为观察窗口为 **150 ms**，不把有限窗口宣称为无限期保证，也不把受控 working 事件或宿主副本当作原生 Chat 活动证据。

真实角色复验须先实际部署 manager-chat/node-chat 并隔离 Herdr、数据及验收 zone。由真实 manager 工具 pause，保留状态、拒绝派发结果、实际外部启动/终止记录；已有 node Chat 继续 report 与 complete；由真实工具 resume，记录明确起止的观察窗口内无新增启动，之后显式 dispatch 才新增执行。再在 paused 且 Chat 仍活动的现场直接 finish，核对终态拒绝和首次历史。不具备真实角色入口/隔离宿主时如实保留未验项，HTTP 结果不替代真实工具、Prompt、原生身份或模型执行证据。

### 完成与结束的验证入口及边界

可重复确定性入口为 `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch -- --nocapture`，完整后端交付检查为 `node scripts/test-backend.mjs`。`explicit_complete_unlocks_only_satisfied_dependencies_and_preserves_first_fact` 核对独立 A/B → C 字面快照、只完成 A 的拒绝和首次说明/历史；`finish_requires_every_handoff_and_terminal_actions_preserve_success` 核对显式派发 C、结束条件、终态拒绝、查询和重复 finish；`reports_and_observed_chat_lifecycle_never_replace_explicit_handoff` 分别经 HTTP 提交 report、停轮状态事件、退出登记事件，再显式完成并验证迟到报告不重新打开工作。各负向观察窗口为 **150 ms**，测试输出保留实际外部调用列表、事件和动作/查询结果；有限窗口不证明无限期没有自动行为。会话事件是受控输入，外部 Herdr 副本不是原生 omp，不能宣称证明了真实 Chat 停轮/退出。

真实宿主复验需要 metadata 指定的 manager-chat/node-chat 实际交付：在独占数据、zone 与 Herdr 实例中，通过公共角色工具执行前置 report 后保持 running（让该 Chat 继续收尾），确认 C 拒绝；管理者 complete A/B 后显式 dispatch C，全部 complete 后 finish，保留原生工具结果、Prompt 注入、Chat 身份/活动顺序、三份运行记录和无强制结束/自主派发的有界外部调用证据。角色入口缺失时，本仓库确定性检查可执行，但原生工具/Prompt/真实 Chat 仍未验证，不能用受控 HTTP 结果宣称本票完整宿主验收。

## 单节点真实宿主闭环验收

本节是进入多节点验收前的独立入口，只验证一个无前置 node 的 manager → dispatch → 实际文件/report → complete → finish。即使本节全部通过，也不代表 P62/P63 的 A/B → C、并行、汇合或多轮推进已经通过。HTTP TestServer、记录型 Herdr 副本和手填身份请求都不能代替本节。

### 独立预期与观察 seam

开始前先保存本轮预期，不能读取实际产物后再改预期。固定 node ID 为 `ONE`，`blocked_by=[]`，输出为 node zone 根目录的 `single-node-result.txt`。文件必须是无 BOM 的 UTF-8，行尾仅 LF，末尾有一个 LF，完整内容固定为：

```text
workflow-single-node
answer=42
```

这个字面预期独立于模型与运行实现；不要用模型报告、服务返回值或被测实现生成期望值。以下 PowerShell 在本轮实际 node zone 已保存为 `$nodeZone` 后核对原始字节；缺文件会报错，不生成替代产物，错误内容、BOM 或 CRLF 都不能通过。保存本段原文作为启动前预期，并在 manager 核对时保留实际执行结果；本次环境阻塞，尚未执行此产物检查。

```powershell
$expected = [Text.Encoding]::UTF8.GetBytes("workflow-single-node`nanswer=42`n")
$artifact = Join-Path $nodeZone 'single-node-result.txt'
$actual = [IO.File]::ReadAllBytes($artifact)
if ([Convert]::ToBase64String($actual) -cne [Convert]::ToBase64String($expected)) {
    throw "实际产物字节与独立预期不符：$artifact"
}
Write-Output "产物字节符合独立预期：$artifact"
```

node.input 应明确要求写上述文件、读取文件核对内容，然后通过真实 node 角色工具 report 绝对路径、编码和限制。管理者本次要求为：只派发 ONE；使用 status 与记录读取观察进展；收到 report 后亲自读取实际文件并与事先保存的预期逐字节比较；只有吻合才 complete ONE（非空 reason 记录文件地址与核对结论），再显式 finish。collect 未交付时不得伪造 collect 或以循环代替其承诺。

观察 seam 是实际交付页面的发起 Run 入口、真实 manager/node 角色工具及其公共 Session 身份、node 实际文件、三份运行记录与 Herdr 原生 Chat。可区分的具体违规是“node 报告成功但未写文件或内容错误”：仅凭 report 或 completed 快照会漏过；独立读取实际文件并比较完整字节才能拒绝它。此真实场景未可执行时，不制造受控模型的 red/green 来声称覆盖。

### 隔离与执行顺序

1. 按[测试指引](agents/testing.md)构建交付页面和安全关机壳。参考[静态样例的隔离配置](example/README.md#在隔离目录运行)，但不要对其用户配置或静态多节点样例直接发起 Chat。为本轮新建独占 service cwd、数据根、manager zone、node zone 和证据目录；配置数值回环动态端口、`mdns.enabled=false`、显式 `scheduler.run_directory` 和稳定的本说明绝对路径。三份记录留在 node zone 外。所有存储路径都归本轮所有，不能只把配置文件放到 Temp 却沿用用户数据。
2. 先核实当前 metadata 指定的 manager-chat/node-chat 是真实部署入口，能够加载公共 Session、角色工具及 `first-enabled-run` Prompt；同时核实 omp 未启用 autoResume。确认可以通过本轮独占的 Herdr 实例/命名 session 隔离 Workspace、Tab 和 agent。命名 session 或配置隔离未建立前不要调用用户的默认 Herdr 实例；只存在 Herdr 可执行文件不足以证明隔离可用。任一前提缺失就保存真实错误并停止，不修改 metadata 指向空扩展、不手写运行事实。
3. 在隔离服务的实际页面登记两个 zone，创建 manager/node 类型的 omp Task，固定本轮 model/thinking、初始 Skill、prompt 和扩展选择并保存其预期。创建一个引用 node Task 的 Bot 和仅含 ONE 的 Map，node.zone 为独占 node zone，输入采用上述字面任务。启动前保存这些实际定义及本次要求；运行期间不再修改。
4. 经 scheduler 页面显式发起 Run，保存页面结果及 Run ID。由真实 manager 使用角色工具查 status，确认 ONE ready，显式 dispatch ONE；不能由验收脚本手填 sessionId 冒充管理者。记录 manager 与 node 不同的原生 session_id、实际 cwd、Workspace/Tab/pane/agent 关联、公共 Prompt 实际注入与工具原始参数/结果。应由角色扩展取得身份，不允许模型提供归属。
5. node 在自身 cwd 写文件并调用真实 report。管理者用 status 与传入记录路径读取已保存报告，亲自读取该绝对文件，核对以上独立字节预期；随后显式 complete ONE，再 finish。若 manager 停轮，可人工要求继续核对/完成，但每次补充输入、时间、对象原生身份和原因都必须保留，不声称强制续轮。通知失败与保存失败分开解释，不盲目重报或重派。
6. 保存实际文件的原始字节、全部原生工具结果、两份原生 Chat 引用/可供复查的对话、最终 `run.json`、`history.jsonl`、`messages.jsonl` 和完整人工干预记录。用 status/记录确认 ONE completed、非空首次 completion reason、Run completed、首次 finished 事实，以及对应 report/complete/finish 历史。缺失的记录应标为缺失，不能生成空记录充数。任何保存/历史/通知部分失败均单列，不凭最终状态抹去。
7. 持续消费所属服务/agent 的 stdout/stderr；正常或失败退出都只清理本轮拥有的进程与资源。保留证据后再删除临时数据，不关闭用户已有 Workspace/Chat，不调用真实关机。保存准确的启动命令、配置、版本和清理结果，以便从新临时目录重复执行。

### 2026-10-28 实际前置检查：阻塞，未运行闭环

在本票独占 worktree 检查当前部署，实际结果为 `herdr 0.9.5`、`omp/18.13.2`，两者可执行文件均可找到。metadata 的两个固定角色入口分别为：

```text
E:\Developer\myhub\my-agent-toolkit\entrypoints\omp-auto\manager-chat.ts
E:\Developer\myhub\my-agent-toolkit\entrypoints\omp-auto\node-chat.ts
```

实际目录读取仅见 `codebase-tools.ts` 与 `connect-bot.ts`；对以上两个绝对路径执行 `Test-Path -PathType Leaf` 均为 `false`。对 toolkit 文件名另行查找也未找到这两个角色入口。可重复前置检查（PowerShell；这是部署检查，不是闭环通过证明）：

```powershell
$paths = @(
    'E:\Developer\myhub\my-agent-toolkit\entrypoints\omp-auto\manager-chat.ts',
    'E:\Developer\myhub\my-agent-toolkit\entrypoints\omp-auto\node-chat.ts'
)
$checks = @($paths | ForEach-Object {
    [pscustomobject]@{ path = $_; isFile = (Test-Path -LiteralPath $_ -PathType Leaf) }
})
$checks | ConvertTo-Json
if (@($checks | Where-Object { -not $_.isFile }).Count -gt 0) { exit 2 }
```

上述带退出码的前置检查已实际执行，退出码 **2**，原始结果为：

```json
[
    {
        "path": "E:\\Developer\\myhub\\my-agent-toolkit\\entrypoints\\omp-auto\\manager-chat.ts",
        "isFile": false
    },
    {
        "path": "E:\\Developer\\myhub\\my-agent-toolkit\\entrypoints\\omp-auto\\node-chat.ts",
        "isFile": false
    }
]
```

本次检查在创建服务、Run 或 Chat 之前停止，未接触用户对话/产物，未创建持续运行的验收子进程；版本/部署探针均已退出，无遗留清理对象。没有可交付的原生身份、工具调用、实际产物或三份运行记录；它们是**未产生**，不是验收通过。检查期间第一次 PowerShell 调用因外层 shell 展开 `$` 而语法失败，改用单引号包裹 PowerShell 程序后才得到两个 `false`；没有通过修改运行事实或模型输入干预结果。除此之外没有运行内人工干预，因为本轮尚未启动。

当前精确阻塞是独立 my-agent-toolkit 尚未部署上述真实角色入口，不能执行要求的原生身份/Prompt/工具路径。Herdr 隔离启动、模型访问与完整闭环也仍未验证，不能从可执行文件版本推断可用；没有证据据此修复宿主运行 bug。部署真实入口后须从新的独占环境按本节全程复验并另存实际证据；本次前置检查不能解除单节点宿主验收，也不能将 P62/P63 标为完成。

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
