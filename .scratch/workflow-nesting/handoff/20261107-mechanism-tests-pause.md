# 14／22 流程机制续测：用户要求暂停

## 当前动作与入口

用户要求：Bot 版本冲突问题解决后保存进度并暂停，剩余测试在新会话继续。已完成该修复的定向回归和重建产物公开 HTTP 核实；随后不再启动测试。

- 本轮机器记录及完整命令／日志：[summary.json](../evidence/mechanisms-20261107-114350/summary.json)。
- 暂缓问题与研究边界：[manual-report-confirmation-research.md](../../manual-report-confirmation-research.md)。
- 之前真实 Luna 业务运行：[progress.json](../evidence/continue-20261107-090904/progress.json)。本轮机制检查没有调用真实模型。
- 两仓根：`D:/developer/myhub/herdr-bot`、`D:/developer/myhub/my-agent-toolkit`。当前为未提交工作树；保留此前全部改动，不 reset／清理它们。源码基线、初始 diff 摘要及本轮文件哈希见 summary。

## 用户授权暂缓：不重新诊断或靠重试寻求通过

1. manual 报告内置是／否发送门禁，以及相关询问语义、停轮和观察标记问题；后续独立需求处理，不改 oh-my-pi。
2. 提示词专项：22 票 W1 基线节点角色／阶段混淆、模型自行承担 manager 职责、误解文件授权／手写错误 HTTP，以及三路业务批准等模型遵循问题。本轮不再修改 prompt。
3. 独立 Chat 的 Pi 启动就绪边界是已知问题，单独保留；不补发 Enter／prompt、不改 Suite／安装版宿主绕过。

暂缓不等于通过，不关闭 14／22 或改编排状态。真实模型仍只允许 `openai-codex/gpt-6-luna`，不回退其他模型；本轮的模型名称均为受控原生协议夹具／后端目录项，不是其他真实模型调用。

## 已完成的本轮检查

| 检查 | 实际结果及源码边界 |
|---|---|
| 当前源码安全构建 | `herdr-bot`、`herdr-fixture`、`safe_shutdown_shell` 构建成功；修复冲突后已再次构建 |
| `workflow_dispatch` 完整目标 | 165/165，通过时间在 HTTP 冲突分类修复之前 |
| toolkit `tests/workflow-chat` | 25/25、555 断言，通过时间在 HTTP 冲突分类修复之前 |
| 新增两宿主联合用例 | OMP／Pi 2/2、70 断言；各实际调用 run／collect／report 共12次，连接真实配对后端，检查内部 finish、外报归属、最新报告、显式父 complete 和精确一次受控关闭 |
| toolkit `bun run test:dock` | 181/181、50文件、2360断言，通过时间在 HTTP 冲突分类修复之前；不能标成修复后全量结果 |
| 冲突修复后 Rust 定向回归 | `workflow_dispatch shared_business` 3/3，含 Task／Map 的精确409／错误码及完整定义保持 |
| 冲突修复后实际进程探针 | 重建后的安全壳公开 HTTP 返回409、`bot-conflict`，当前完整定义不变；passed=true、cleanupErrors=[] |
| 双轴静态审查 | 新联合用例与冲突修复分别经 Standards／Spec 审查，无 finding；审查不是运行证明 |

此前78个后端目标＋doctest、前端244及toolkit canonical1819属于此前源码／产物记录，不是本次冲突修复后的完整回归。

## 实际修复与新增测试

### Bot 版本冲突分类已修复

- 修复前公开进程探针：HTTP500、`code: bot-conflict`，数据虽未覆盖但错误分类不正确。证据：`revision-probe-evidence.json`。
- 永久回归先红：`revision-contract-before.stdout.log`，明确实际500／预期409；不是模型或夹具失败。
- 根因：`DirectoryError::into_response` 仅把 `map-conflict` 映射409，`bot-conflict` 落入500默认分支。
- 最小修复：`backend/src/scheduler/mod.rs` 将两个 conflict 都映射409；不改保存、revision 校验、授权或状态机逻辑。
- `backend/tests/workflow_dispatch/shared_business.rs` 从宽泛 `>=400` 收紧为409及`bot-conflict`，两种绑定完整读回和非目标保持断言仍保留。
- `scripts/workflow_create_safety.py` 的共享真实 revision 负例也收紧为409／错误码，创建与修订共用；仍禁止盲重试。没有启动该脚本的真实模型业务场景。
- `docs/business-materials.md` 已记录接口分类；前端 `writeDefinition` 统一按 `response.ok` 拒绝，无500专用兼容分支，故前端代码未改。
- 修复后实际进程证据：`revision-after-fix/revision-probe-evidence.json`。初始失败、定向回归、构建和后验命令均保留于本轮根目录。

### toolkit 新联合机制用例

- 新文件：`tests/harness-dock/nested-role-completion.test.ts`。
- `tests/workflow-chat/support/dual-role-dock.ts` 仅增加可选 endpoint，使正式原生工具能直接访问真实配对后端；旧受控 peer 仍用于原有取消／恢复用例。
- `docs/agents/testing.md` 已补运行入口与证据边界。
- 初期夹具错误（不支持的模型目录项、直接 Bot 文件缺 id、Pi 受控身份错误声明OMP、异步关闭观察及日志字段）均保留原日志，未认作生产缺陷。修正夹具后两宿主通过；已有关闭预算及预期没有放宽。
- OMP dock 使用已安装的隔离18.5.0；Pi明确1.5.0；Bun1.4.4，Node22.23.1。OMP helper 在安装缺失时可能安装，续测前确认缓存已在，禁止为验收自动安装。

## 新会话剩余执行顺序（本次用户暂停后未执行）

1. toolkit `bun run typecheck`（新增 TypeScript 测试／辅助接口后尚未执行）。
2. herdr-bot `node scripts/test-backend.mjs --binary-jobs 2`，使用当前 `CARGO_TARGET_DIR`；这是 HTTP 分类修复后完整后端回归，不能复用修复前78目标为修复后绿。
3. `python scripts/check-revise-bot-guides.py --target-dir D:/developer/myhub/herdr-bot/backend/target`：完整发布／预检烟测尚未执行。本次实际进程探针只用了其安全准备入口并专门验证保存冲突，不等价于完整 run。
4. 修复后的配对产物重新执行必要原生回归；若认领完整 Dock，则重新执行 `bun run test:dock`。已有181属于修复前产物。原生测试是受控provider机制，不是Luna业务。
5. 前端 `npm --prefix frontend test -- --maxWorkers=2`；当前接口分类修复后未重跑。
6. 视剩余机制目标决定执行既有 `scripts/test-real-herdr-close.py`：真实 Herdr 目标 Tab／进程精确关闭的独立烟测，本次未启动。必须使用独占配置、显式依赖根／路径和安全后端，不连接用户daemon，不实际关机。不凭受控 Herdr 命令记录认领真实 Tab 验收。
7. 根据新会话目标评估是否需要 toolkit canonical；本次未执行修复后 canonical。按 testing.md 顺序运行昂贵套件，保持源码和产物稳定，保留失败，不自动重试业务。

原生机制环境示例（PowerShell，toolkit 根；不会安装依赖）：

```powershell
$env:HERDR_BOT_REPO = 'D:/developer/myhub/herdr-bot'
$env:CARGO_TARGET_DIR = 'D:/developer/myhub/herdr-bot/backend/target'
$env:OMP_BASELINE_RECORD_ROOT = 'D:/developer/myhub/my-agent-toolkit'
$env:PI_CLI = 'C:/Users/hugos/.pi/agent/install/releases/1.5.0/node_modules/@earendil-works/pi-coding-agent/dist/bundle/cli.js'
# PI_NODE 的本次实际绝对路径已保存在 summary.json.environment.node；复用或显式核对既有 Node。
bun test tests/harness-dock/nested-role-completion.test.ts
bun run test:dock
```

每次新证据使用独占目录；可设置 `WORKFLOW_PROMPT_EVIDENCE_DIR` 和 `WORKFLOW_DUAL_EVIDENCE_DIR`。不要覆盖本轮目录或依赖上一会话Python内存变量。

## 仍然不能认领的业务覆盖

- 真实模型 Task／Map × 有／无材料完整修订、三路批准版本与一次保存逐字对应、专属 Chat／workflow 配对。
- baseline／真实目标冲突、业务授权、运行禁写、原生文件写入授权、已生效／未生效／未知叙述等当前由模型工作流指引承担的规则。后端没有等价“业务批准API”，不能通过模拟批准接口把这些改判为代码通过。
- 两宿主原生双角色＋真实嵌套后端＋真实 Herdr Tab 生命周期的完整联合验收。新用例只证明原生工具与后端联合机制，外部 Herdr 仍是fixture。

暂停时没有运行中的测试任务；已完成／失败任务记录不是活动任务。验证错误与清理错误保留，真实进程冲突探针清理无错误。未提交、更改票据状态或修改编排内部状态。
