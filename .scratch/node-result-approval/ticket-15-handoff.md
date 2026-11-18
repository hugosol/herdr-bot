# Ticket 15：发布 V2 原生验收交接

日期：2026-11-18。用户要求本轮收尾，测试代码留待其他会话继续。**未完成，不认证成功、不集成、不关闭，未成功执行 hyper_auto_coding finish（工具因三票未集成而拒绝结束）。** 停止新增模型调用、测试、修复和400调查；保留原始失败及分支/worktree。

工作流最终已 collect：无 active／integrating，15为带未完成error的 `ready`，不是验收通过；总体 `finished=false`。见 `evidence/handoff-workflow-state.json`。下次获准继续时用原工作流 `status`／`dispatch` 恢复实现，不用 `retry_integration`，不重建票据。

## 恢复位置与提交

- 原票：`implementation/15-published-v2-native-acceptance.md`；P34/P35的V2发布、职责、配置／运行及S2/S3分区。
- herdr-bot：`C:/Users/hugos/AppData/Local/Temp/hyper-ticket-SpWusv`；分支 `hyper/node-result-approval/15-published-v2-native-acceptanc-253c5d-349645ddb346`。
- toolkit：`C:/Users/hugos/AppData/Local/Temp/hyper-ticket-D1g6EG`；分支 `hyper/node-result-approval/15-published-v2-native-acceptanc-253c5d-3a3cc4cfa470`。
- 原实现 `276be3f`；toolkit `4997d9f` 仅全量检查自动生成的验收JSON。
- `b9ef35c`：共享Codex adapter最小安全诊断与离线回归。
- `d2cc513`：当时票内shell配置／409成功complete区分。
- `281201a`：逐阶段业务授权及显式临时环境。
- `2c58b01`：将已知OMP YAML接线修入既有共享父类，删除15重复接线；仅匹配真实Esc/Ask call的单次scope澄清。
- `eef1b75`：删除票内不必要repository副本接入，字典、只读范围和run_instructions统一参与worktree。
- 最终收尾 ledger：`ticket15-handoff-ledger.json`。herdr-bot HEAD=`eef1b75a65f6176579399237d59571cd1348faf3`；toolkit HEAD=`4997d9f6d7b866e85b36597baf6e0b841f44dd56`；owner报告两仓Git可见未提交文件均无，忽略的证据保留。所有已启动自有任务均已结束，无保留运行／服务。
- 这些是原票开发分支提交，不代表已合入dev，不按此短hash清单重放全部提交。

## 覆盖状态

| 项目 | 已观察 | 仍未满足 |
|---|---|---|
| 实际宿主／模型／后端 | 安装OMP/Pi、生产后端、真实HTTP和材料；多轮实际canonical gpt-6-luna回执 | 请求成功不代表业务交付 |
| 配置及启动页面 | 早期两宿主真实Chrome配置与launch已过 | 最终四节点状态／来源／材料与页面交叉证据未完成 |
| manual批准 | Pi曾观察真实问题、选中Yes、true返回及公开manual来源 | 三个人工节点完整链路未完成 |
| complete门禁 | 最新false报告不能complete有真实拒绝证据 | 不能把自有409拒绝探针算作成功complete |
| 四节点V2交付／嵌套auto／显式交接 | 已启动真实Run并取得部分调用／报告事实 | 完整Task/Bot交付、子图、auto及manager report/complete/finish未通过；P34/P35不能认证 |

没有修改V1/V2发布业务定义或生产backend/frontend/toolkit业务实现以制造通过。当前主要修复的是验收驱动错误接入。

## 最新单OMP实际业务：当前首要卡点

`ticket15-omp-business-single-root/`：

- 使用统一后的参与worktree只读根，同一个正式入口Run／独占transport渐进观察；未启动新双宿主矩阵。
- 14份实际canonical成功回执后，第15次真实上游HTTP400，`requests=1`，transport terminal，无补发。
- `create_task`仍running，没有成功交接；`cleanupErrors=[]`。
- 原件：`codex-completions/0014-error.json`。`providerError`白名单字段均other。
- 安全request shape：21输入项、6 message、16工具、6调用+6结果，`toolBindingsMatch=true`、instructionsLength=9891。
- 这只排除了部分明显缺失call/result形状；**不能证明Responses payload完全合法，也不能认定400根因**。

下一会话应先只读已有失败请求／SDK转换差分，不再开新业务Run碰400。待证假设包括Responses item id/call_id跨轮复用、reasoning/encrypted字段来源及工具schema/序列化；未有结论，不能移除字段绕过校验。若旧脱敏边界已经丢失原因，明确缺少哪些信息，而不是把other解释成具体服务错误。

## 已证实的驱动问题与修复

### 1. shell接线遗漏：不是新seam

已有正确模式在 `workflow_nesting_outer_acceptance.py`、`workflow_nesting_planning_acceptance.py`。共享 `acceptance/host/native.py` 原先先让model helper写settings.json，后生成不含shellPath的OMP config.yml，挡住legacy配置迁移。票内局部补接线不是理想复用；`2c58b01`已将字段修回既有 `NativeAcceptance.configure_model` 并删15重复。

安装宿主实际读取配置回归先红后绿：旧为System32/bash，修后为测试显式PowerShell/-Command；不是仅检查输出字段。

**归因更正：OMP18.20普通bash工具调用嵌入式pi-shell，shellPath服务PTY/ACP/SHELL等spawn路径。** 原生工具文件／HTTP成功不能称其生成了外部Git Bash。Pi显式Git Bash是另一条真实外部shell路径。YAML接线修复不能冒称解决了OMP普通bash、HTTP400或模型路径消费根因。

### 2. 阶段授权过宽

原business_reply每阶段统一授权“新Task和新Bot”，但create_task职责不允许创建Bot。不能简单扩Yes词表批准越界候选。

现按原定义收窄：create_task只Task/request，align只需求/target_materials，design仅真实展示且符合原要求的Bot候选保存。实际business_form/business_reply共用纯验收授权策略；缺候选／未知动作不给授权，不代写材料，不指定模型工具顺序。窄回归曾移除guard使两项越权用例实际失败，恢复后通过；仅证明driver边界，不认证S2。

### 3. Ask取消边界

真实链为driver对无候选Ask执行Esc→实际toolResult cancelled→OMP合法aborted／Interrupted by user。旧driver在处理已排队scope澄清前把aborted视为fatal。

最小修复只允许**本次已记录Esc且匹配toolCallId**的pending_scope发送一次明确业务澄清，并清队列；其他aborted、provider错误仍失败。不增加一般30秒容忍，不重发输入。

`ticket15-esc-minimal-native-aux-aware/native/omp/evidence.json` 已实际通过：OMP Ask→Esc→匹配aborted→一次澄清→宿主消费真实新user turn，clarificationsObserved=1、exit0、cleanupErrors=[]。本地机械provider仅选动作，不认证P34/P35。前探针错误把title-request side-channel当业务，原失败保留。

### 4. 409探针误计complete

已修为只把真正成功响应计入complete；自有完成门禁409是拒绝证据，不是成功交接。不得因HTTP请求存在就认证应用动作成功。

### 5. 文档根与许可范围冲突

真正失败worker `01a1eae9-3d0e-7007-9b82-f8ee5a41c877` 的 `ticket15-pi-scoped-authorization/codex-completions/0006.json` 确实包含repository根、zone、相对文档、worktree绝对说明和run.json，不能说只有manager收到了路径。

实际时序：先成功读repository/docs/usage.md；误拼zone/docs/workflow-runtime.md返回ENOENT（1794942558636），两毫秒后首个false报告；再后两次正确读取worktree/docs/workflow-runtime.md成功，但manager已收到阻塞报告并最终自主fail。后续成功不能倒推首报前已读到。

同时driver第47行只读范围是repository/Skills，第37行却要求worktree/docs；复制副本和保留shared run_instructions造成双根冲突。不能仅凭文件存在归因模型错误。`eef1b75`删除这段副本，复用既有 `workflow_plan_acceptance.py` 的参与worktree绑定，不造zone别名、不扩大额外根、不改产品路径解析。

原件摘录：`ticket15-failed-worker-native-upstream-contract.json`、`ticket15-failed-worker-path-contract.json`、`ticket15-worker-read-results-and-report.json`。

### 6. 临时目录边界

一轮Pi曾扫描共享Windows Temp并写 `/tmp/tasks-response.json`；不得认证为正确隔离，也不能删除未证归属的共享文件。已显式设置独占TMP/TEMP/TMPDIR并记录继承；提示／环境配置不等于OS沙箱。

`ticket15-pi-invalid-fixture-termination.json` 对应一轮由验收driver公开fail终止的无效fixture，不能算模型自主fail。另 `ticket15-pi-scoped-authorization` 是manager自己收到false报告后自主fail，两者必须区分。

## 保留的其他真实失败

- `ticket15-native-fixed-catalog`：OMP64份成功后第65次provider HTTP400；共享terminal让Pi没有真实上游请求。
- `ticket15-pi-independent`：20份真实回执、原生Yes/manual及门禁拒绝；shell／技能可达性阻碍业务，manager显式fail。
- `ticket15-native-windows-shell`：OMP16份成功后再HTTP400；单写OMP settings.json不能证明有效YAML配置。其后独立transport消除双宿主terminal互相遮挡。
- `ticket15-omp-safe-diagnostic`：43真实成功、无transport错误，暴露阶段授权错误。
- `ticket15-pi-safe-diagnostic`：路径消费／越界与过宽授权使fixture无效，不能用受控终止认证业务。

所有旧失败保留，不把不同运行的局部事实拼成一次完整通过。

## 检查及安全诊断

既有两仓九项检查已exit0：前端274、浏览器375／1既有skip、toolkit1932、具名dock10、typecheck/integrations及本仓构建／后端。见 `ticket15-final-ledger.json`、`ticket15-resumed-check-attribution.json`；后续未重跑全仓，不代表新driver完整S2/S3已过。

后续窄检包括授权／Esc／共享配置13例、真实安装SDK安全400诊断3例、受影响CLI共14例。共享诊断通过字面失败样本验证恶意error字段／正文不泄露；固定白名单type/code/param及无秘密request shape，未知为other。仅对HTTP非成功边界抓取，不覆盖14的HTTP200+SDK错误；不输出原文、headers、认证、tool ID或业务参数。一次POST、maxRetries=0、terminal、无fallback不变。

## 下一会话顺序

1. 读原票、contract、本文及最新stop ledger，确认原worktree提交／未提交修改；先复用已有最小回路，不新增phase CLI或第二传输。
2. 最先处理已有HTTP400证据的最小诊断，区分原生消息与SDK最终payload。没有根因时不要再开完整矩阵、重复模型决定或把预检绿当业务绿。
3. 根因明确、修复验证后，沿正式单宿主同一Run逐阶段完成create_task→align→design／保存及嵌套auto、manual、报告、complete/finish；真实材料和来源必须观察，不代模型执行。
4. 再完成另一宿主和最终页面交叉验证；保留每轮原始失败／scope／退出码。涉及共享接线提交与12/14的集成冲突时按实际调用链合并，不能复制第二套。

## 保存和清理

协调仓副本：`evidence/ticket-15-handoff/`。原证据根为本worktree `.scratch/node-result-approval/evidence/`；保持相对结构，原JSON绝对路径不改写。归档清单／完整性见 `preservation.json`；完整未复制model-wire、native storage等仍在保留worktree。

各已收尾轮次报告cleanupErrors=[]；旧Pi3个held handles及两组Chrome有退出确认，最新单OMP也已结束。最终进程／Chrome快照和未提交文件以stop记录为准；不因“无清理错误”宣称系统无所有残留。认证文件不归档、不复制、不刷新；不删除源码、worktree、原始证据、归属不明共享Temp文件或其他票资源。
