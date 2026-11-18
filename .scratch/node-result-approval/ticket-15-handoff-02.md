# Ticket 15 第二次交接：发布 V2 双宿主验收（2026-11-18）

**结论：单请求重放通过；正式 OMP Run 失败；Pi 未在本轮重跑；整票未通过。** 用户接受把偶发的上游异常作为工作假设，不再把 `0008` 单请求能否取得模型决定作为阻塞项。不能据此判定服务端根因、自动重试业务决定、修改正式 coverage、集成或关票。

- **只在票据 worktree 操作**：herdr-bot `C:/Users/hugos/AppData/Local/Temp/hyper-ticket-SpWusv`（`eef1b75`）；配对 toolkit `C:/Users/hugos/AppData/Local/Temp/hyper-ticket-D1g6EG`（`4997d9f`）。不要直接在 `dev` 改造。本文及相对证据链接均在前一棵 worktree，不要删除该临时目录。
- [原票与正式命令](implementation/15-published-v2-native-acceptance.md) · [批准的契约](contract.md) · [第一次交接](evidence/ticket15-handoff.md) · [历史提交与收尾账本](evidence/ticket15-handoff-ledger.json)。第一次交接是历史截面；本文补充其后实际运行，不覆盖原始失败。

## 一、哪些已经改了，哪些没改

| 已有改造 | 落点、作用和边界 |
|---|---|
| `276be3f`：V2 专项入口 | `scripts/accept-node-approval-v2.py` 编排，`scripts/node_approval_v2_acceptance.py` 驱动真实宿主，`scripts/node-approval-v2-browser.mjs` 检查打包页面。不是待从零编写的场景。 |
| `b9ef35c`：Codex 错误安全诊断 | `scripts/workflow-codex-completion.mjs` 与 wrapper 对非成功 HTTP 状态保留白名单分类、请求形状，有离线回归。未解决 **HTTP 200 后 SDK 无有效决定** 的分类缺口。 |
| `d2cc513`、`2c58b01`：shell 与完成门禁 | OMP 有效 YAML 的 shell 配置已归入共享 `NativeAcceptance.configure_model`，不应再补票内副本。OMP 普通 bash 使用内嵌 pi-shell，不要把 Git Bash 预检当成它的执行证据。自有 409 拒绝不再算成功 complete，但只数 200 仍不充分。 |
| `281201a`、`2c58b01`：分阶段授权、独占 Temp、Ask/Esc | Task 阶段不批准创建 Bot，需求阶段不批准最终保存。只有本驱动实际 Esc 且匹配同一 Ask 调用时，才发送一次范围澄清。独占 `TMP/TEMP/TMPDIR` 不等于 OS 沙箱。 |
| `eef1b75`：统一只读仓库根 | 去掉多余 repository 副本，Task 文档与 `run_instructions` 指向参与 worktree。修复后没有新的 Pi 完整业务成功证据。 |

**本轮没有修改或提交生产代码、正式验收脚本、发布定义、toolkit 或 `dev`。** 本轮新增的是离线差分、单次真实请求、临时诊断探针、一次正式 Run 的证据，以及[限定范围的通过判定](evidence/ticket15-omp-fresh-0008-plain-20261118T082317Z/case-verdict.json)。探针只作诊断，未安装到正式 adapter。九项全仓检查是[此前已通过的记录](evidence/ticket15-resumed-check-attribution.json)，不是本轮新跑或双宿主业务通过。

## 二、具体哪次脚本没过，停在哪里

| 实例 | 已观察到 | 没有发生或不能据此认定 |
|---|---|---|
| **最新正式 OMP**：`accept-workflow-codex.py --scenario node-approval-v2 --hosts omp --native-only`，调用正式 `accept-node-approval-v2.py`；Run `bc-493511c0ab12` | 配置页、发起页通过；`manager/status`、`manager/prepare_inputs` 均 200，`inputsReady=true`。8 次模型成功；第 9 次（本轮 `0008`）在一次 `read` 后、首次派发前失败：一次上游 POST 得 HTTP 200，但 adapter 退出 1，分类 `provider-error`。本地转接返回 502，随后 12 个请求在本地被终止，未转为上游重试。 | 最后公开状态：`create_task`、`create_bot_v2` 都是 `pending`，无节点报告或批准，未启动子 Run。wrapper 与业务入口均退出 1，`transportFailed=true`。本次不仅是 `--native-only` 不能认证全票，**业务确实失败**。`cleanupErrors=[]`，两个受观察原生进程及两次 Chrome 已确认退出。见[专项详情](evidence/ticket15-omp-fresh-after-0014-replay/business/omp/evidence.json)、[失败回执](evidence/ticket15-omp-fresh-after-0014-replay/codex-completions/0008-error.json)。 |
| **较早正式 OMP**：`ticket15-omp-business-single-root`，Run `bc-9573d988da95` | `create_task` 已派发且 running。14 次上游成功；第 15 次（这轮的 `0014`）在带 `dispatch` 结果请求续答时，上游 HTTP **400**，白名单分类全为 `other`。 | 没有 Task 交接或后续节点。这不是最新正式 Run 的 HTTP 200 故障。见[原 400](evidence/ticket15-omp-business-single-root/codex-completions/0014-error.json)。 |
| **Pi 既有运行**：`ticket15-pi-independent`、`ticket15-pi-scoped-authorization` | 前者曾观测原生 Yes、公开 manual 来源及最新 false 报告不能 complete；后者 worker 错读 zone 下的文档后提交 false 报告，后续正确读取不能改变首报时序，manager 自主 fail。此前也有由驱动主动终止的无效夹具，不能混同。 | 本轮没有新的 Pi 正式 Run；没有三个人工节点、嵌套自动父节点和父子 finish 的同轮证据。旧文档根冲突已改，不等于新 Pi 已通过。见[第一次交接的 Pi 时序与证据索引](evidence/ticket15-handoff.md)。 |

**编号只在各自证据目录内有意义。** `ticket15-omp-business-single-root/0008` 是用于对照 `0014` 的旧成功样本；`ticket15-omp-fresh-after-0014-replay/0008` 是最新正式 Run 的第 9 次失败请求。不要只写“0008 失败/成功”。

### 请求调查已到哪里（不必从头重放）

- **旧 `0014` 的 400 未复现。** [离线出站差分](evidence/ticket15-offline-outbound-diff-0008-0014.json)用真实 SDK 校准同会话旧 `0008` 成功归档：后一次保留原 18 个 input items，只新增空 assistant 文本、`run(dispatch)` 与匹配结果。另一个 Run 的成功样本 `0012` 也有这种结构。**“只有 `i` 不同”只指最后一次 dispatch 的参数**；整份请求的用户上下文、路径、历史工具结果、Run 快照和 ID 都有差别。用相同模型分别重放[固定的成功请求体](evidence/ticket15-codex-frozen-0012-replay/summary.json)和[未经修改重建的原失败请求体及逐项差分](evidence/ticket15-codex-original-0014-replay/summary.json)，均 HTTP 200。历史失败未保留完整出站字节或具体拒绝原因，不能认定安全策略、`i` 或某项格式导致 400。
- **新 `0008` 是间歇性结果。** [第一次精确重试](evidence/ticket15-omp-fresh-0008-one-retry/summary.json)仍为 HTTP 200 + `provider-error`。随后[带探针的真实请求](evidence/ticket15-omp-fresh-0008-http-sdk-trace/live-trace.json)取得 `response.completed`、SDK `toolUse`；[第二次真实调试](evidence/ticket15-omp-fresh-0008-live-decision-20261118T081931Z/summary.json)取得 `run(dispatch, create_task)`。该次 assistant 可见文本中出现过像错误的字符串，但它不是 HTTP 错误、SSE error 或工具返回。最终[原版 adapter 无探针重放](evidence/ticket15-omp-fresh-0008-plain-20261118T082317Z/summary.json)也成功，普通文本为空、返回一个 `run(dispatch)`。均未执行工具；仅此重放 case 判通过。
- **DeepSeek 是独立协议对照，不是 Ticket 15 的模型验收。** [默认设置](evidence/ticket15-deepseek-flash-0014-comparison/summary.json)和[仅设 `reasoning_effort=low`](evidence/ticket15-deepseek-flash-0014-effort-low/summary.json)均 400，指向 thinking 工具历史缺 `reasoning_content`。不能用它解释 Codex 400；没有伪造 reasoning，没有做关闭 thinking 的对照。

## 三、下一位逐项分析的问题清单

下列是**静态发现的脚本判定缺口**，不是上面两次模型失败的已证根因。每一项给出可独立讨论的完成判据。主文件 `scripts/node_approval_v2_acceptance.py` 下称 V2 driver。

| 编号 | 问题和入口 | 解决到什么程度才算完 |
|---|---|---|
| A · HTTP 200 后错误被压平 | `scripts/workflow-codex-completion.mjs:221,251–252,309,341,27`：调用前置 `failure='provider-error'`；HTTP 200 不进非 2xx 分类；SDK `error/aborted` 时抛空 `Error`，catch 用阶段标签和 `diagnostics` 组成 JSON。`accept-workflow-codex.py:88–116` 白名单归档后抛异常；`acceptance/host/completion_transport.py:111–113` 转成 502。不是 toolkit 业务响应。 | 若决定持久化诊断，用离线 200+失败事件、流未完整结束、SDK 本地异常分别验证**能区分**；只留终止事件、EOF、SDK 停止/异常类别。不记录原文、凭据、工具参数，不加重试。[受控失败追踪](evidence/ticket15-omp-fresh-0008-http-sdk-trace/offline-trace.json)证明 `response.failed` 可以产生该 envelope；不证明历史两次就是 `server_error`。此项不阻塞已通过的重放 case。 |
| B · 显式交接与 409 探针竞态 | V2 driver `native_yes`、`verify_run`。根据 HTTP 200 次数推断首次 complete 会把 `changed=false` 的幂等响应算进去；用报告响应结束时间代替持久化顺序也可能误判。首次 Yes 后，若 ready 报告已并发到达，测试自己发的 complete 探针可能实际推进业务。 | 关联真实 manager 工具结果、Run/execution、`changed=true`、`completion/finished.actor` 与已保存报告。409 只在可靠控制“不具备完成资格”的前提下取证；不能由驱动完成业务。区分子 finish、父 approve、父 report、父 complete、根 finish。 |
| C · 三个人工／一个自动批准与父子身份 | V2 driver `drive`、`verify_run`。`self.yes` 只在人工分支填充；据其为空不能证明自动节点没弹窗。按 session 拼接调用、或只比两个 Run ID，不足以证明批准归属。 | 每个人工执行关联完整问题、可见 Yes、返回 true、原生调用、成功回执、公开 manual；自动节点检查其 session 原始确认记录无弹窗，子 manager finish 后对正确父 execution 主动批准，来源 auto。比对实际 Map、父 node/execution、childRunId 及 manager/worker 身份。 |
| D · 材料接线和真实业务效果 | V2 driver `authorization`、`material`、`check_guidance`、`exercise`。遍历实际返回的 inputs 会漏掉缺失项；用 execution 自报 outputRule 作预期会产生循环验证。能读文件不等于展示候选、实际保存、下游消费的是同一成果。 | 按两张发布 Map 固定 `request → 父输入 → 子 Run → align`、W1 的 `bot_requirements/target_materials → design`、W2 的 `saved/handoff → 父输出`。检查阶段 Markdown 的业务内容、JSON 严格类型/目标/真实 revision、授权交付目录、候选与完整公开 Task/Bot 读回一致；按角色和动作前时序验证读取。保留 V2 authorization/confirmation、不改 Task.prompt、不借 V1；`materials` 可缺席，不执行 CSV 业务。 |
| E · 业务问答授权边界 | V2 driver `authorize_candidate`、`business_form`、`business_reply`。三指南非空不是候选满足业务的充分条件；多候选、Markdown Ask、未知字段或类型有误可能误拒/误授权。另核查该 node 明确绑定的隔离 `material_contract` 文件是否落在文字说明允许读取的范围内。 | 以本阶段**确实展示**的完整候选和允许的效果作决定，确认与实际保存相互核实。未知/歧义不给权限；不扩“Yes”词表、不代写材料、不向模型指定工具顺序、不扩大到共享目录。 |
| F · 最终浏览器证据归属 | `scripts/node-approval-v2-browser.mjs` 的 `facts`。配置页及 launch 已有通过；在整页搜索报告正文不能保证正文属于所选 execution。 | 在对应节点的执行事实、适用报告和累计 outputs 区域逐项比 execution ID、模式/来源、报告 ID、输出 ID/path 与独立 HTTP 读回。S3 验证页面展示；文件字节由业务场景核对，不要求页面读取文件。 |
| G · 两宿主传输隔离 | `scripts/accept-workflow-codex.py` 的 `NativeCompletionTransport` 包住整次业务调用；正式入口虽顺序建宿主 runner，一次 Codex `--hosts omp,pi` 中一方终止仍可能遮挡另一方。 | 单宿主诊断继续用新 evidence dir/新 transport。若改双宿主 wrapper，沿用同一传输实现，但每宿主独立计数、终止状态、清理和证据；失败仍然失败，不补发、不拼接不同 Run 作一次通过。 |

## 四、未通过的正式场景及继续方式

| Ticket 15 所有的覆盖 | 完整通过需要的证据（OMP、Pi 各自一套） |
|---|---|
| **P34/V2/S2** 原生成果批准 | `create_task`、`align_requirements`、`design_and_create` 三个 manual；子 Run 显式 finish 后，对父 `create_bot_v2` 单独 auto。模式配置不等于已批准，报告发送不等于批准。 |
| **P35/V2/S2** 独立业务交付 | 一次独立父子 Run 中实际创建 Task/Bot、完成命名材料交付、真实模型及 manager 消费、最新可交接报告与约定输出齐备、manager 显式 complete/finish；V1 未变。不重跑 14 的 V1，不拿模型建议的工具替代实际执行。 |
| **P34/V2/S3** 打包页面 | 同一完成 Run 的父子四个节点：页面配置、冻结模式、来源、当前报告和累计交付地址与公开事实一致；只有配置和 launch 不能判通过。 |

**建议顺序**：先单独处理 B（门禁探针不能误推进）、C/D（身份、材料和交接判据），再处理 E/F。A 是诊断增强，不用等原 `0008` 再失败；G 在恢复双宿主组合时处理。之后从原正式入口做 OMP 一次闭环，确认实际材料与最终页，再做 Pi。阶段间不要另起假 Run、伪造 manual 来源或把历史成功子标记相加。

复跑命令（PowerShell，从 `hyper-ticket-SpWusv` 根执行；仅在获准新业务运行时使用）：

```powershell
python scripts/accept-workflow-codex.py --scenario node-approval-v2 --hosts omp --native-only --backend "$env:HERDR_BOT_BINARY" --toolkit "$env:TOOLKIT_WT" --paths-file "$env:ISOLATED_PATHS" --pi-package "$env:PI_PACKAGE" --auth-file "$env:AUTHORIZED_AUTH_FILE" --evidence-dir "$env:NEW_EVIDENCE_DIR" --timeout 300
```

变量均为显式绝对路径；新证据目录必须不存在。上次的 backend、toolkit、paths、Pi 安装与子命令可在[正式 wrapper 运行清单](evidence/ticket15-omp-fresh-after-0014-replay/evidence.json)查到；不要复用已删除的临时 `model.json`，由 wrapper 新建。保持独占资源、只读授权认证、现有产品预算与退出确认。单宿主 `--native-only` 即使业务成功也不认证整票；应查看 host coverage、原始回执和 cleanup，而不是只看进程退出码。本次交接不启动新模型请求或测试，不改变票据状态。
