# ticket22：测试结果与三类阻塞最终交接

## 当前决定

用户要求保存本轮测试结果和三类阻塞，在其他对话中逐项解决后回归。本次仅归档，不再启动模型或测试，不修改票据状态、编排状态或用户部署。ticket22 的完整业务验收仍未通过，不能关闭 P40／P42／P43／P44／P45。

**下一会话先读本文。** 三类是目前观察到的主要阻塞，不是对全部潜在缺陷的穷尽；未到达的后续步骤仍可能暴露新问题。不得把“处理这三类”直接等同于“全部验收完成”。

## 一、三类阻塞：事实、边界与后续验证

### 1. manual report 未获发送批准便发送（已证实）

- 最新 case：`task-materials`，真实发布定义的混合宿主；w1 为 Pi、外层管理者为 OMP。
- w1 的策略为 manual；driver 只发送了 `P22-CLARIFY-revise-task-materials` 业务澄清，没有发送 `P22-SEND-...-w1` 报告批准。
- 模型直接 `report(can_complete=true)`，后端 HTTP 200／`saved=true`，报告 `bc-4037f08940f4`。断言 `Baseline manual report preceded send authorization` 正确失败。
- w2–w5 未执行，没有观察到模型发起 Bot 保存；不是完整修订通过。
- 证据：[真实业务结果](../evidence/ticket22-luna-20261107-135646/summary.json)、[原生 report 调用](../evidence/ticket22-luna-20261107-135646/task-materials/codex-completions/0044.json)、[后端实际保存回执](../evidence/ticket22-luna-20261107-135646/task-materials/business/backend-wire/0088.json)。
- 设计研究与边界：[manual report 确认研究](../../manual-report-confirmation-research.md)。尚未实施强制确认门禁；不修改 oh-my-pi、不预设尚未批准的架构。
- 后续回归须观察真实发送内容／确认与 HTTP 副作用：未确认、拒绝、取消不能发送，内容修改重新确认，业务批准和报告发送批准分离。若工具交互改变，调整真实验收驱动，而非删除确认或伪造批准。

### 2. 模型职责／阶段／授权理解偏差（已观察，改进方案待验证）

- 此前 `task-no-materials` 中，w1 曾在交付共同基线前索要未来三字段批准；调整阶段说明后仍未完成。
- 后续观察到模型自行扩大为 manager 职责、把已授权目录中的新文件误判为无权写入，以及手写错误 HTTP（错误字段／缺少 sessionId／使用节点 Session 请求 manager 操作）。
- 已核实的 HTTP 400／403／409 拒绝不等于后端身份缺陷；当时未发现产品身份授权错误，也未验证提示词修复有效。
- 证据：[此前逐步诊断](../evidence/continue-20261107-090904/progress.json)中的 `businessAcceptance.ticket22`，原始运行目录为 `revise-task-no-materials-before` 与 `revise-task-no-materials-after`。
- 用户暂缓提示词专项，本轮未继续改 prompt。提示词优化是改进方向，不保证仅靠修改提示词就能解决。
- 后续至少验证：w1 只核实真实 Bot／目标、需求与授权并交付 baseline／handoff；w2–w4 各自确认所属全文；w5 才在所有条件成立时一次保存。不得替模型选工具、改写原始决策、补齐业务批准或靠盲重试找偶然成功。

### 3. 首条输入滞留编辑器，业务未真正开始（Pi 原因已证实，OMP 内部原因未完全确认）

- 用户授权诊断复跑：OMP `revise-unauthorized`、Pi `revision-baseline-files`。没有补发 Enter／prompt、没有宿主／Suite 绕过。
- **Pi 已证实过早提交**：公开 `inputSubmitted=true` 后，超时终端显示 `Startup is still in progress`，原始任务文字仍在编辑器。安装版 Pi 1.5.0 的 `interactive-mode.js:721,2590-2593` 对应启动提交分支只恢复编辑器文本，不提交或排队执行。
- **OMP 已证实输入滞留，但不能断言与 Pi 实现相同**：提交后快照仍是 PowerShell 启动命令；超时 TUI（实际 18.18.0）显示初始文字留在输入区。尚未抓到丢失提交动作的具体内部分支；启动竞争是与观察一致的推断。
- 两次业务诊断模型请求均为 0，不是 Luna 回答慢。独立 Luna 预检实际成功。终端另有 MCP connection closed 警告，但没有证据证明其是过早提交的根因。
- 证据：[诊断与清理总结](../evidence/ticket22-startup-diagnostic-20261107-224624/summary.json)、[Pi 超时终端](../evidence/ticket22-startup-diagnostic-20261107-224624/pi-timeout-terminal.txt)、[OMP 提交后终端](../evidence/ticket22-startup-diagnostic-20261107-224624/omp-submitted-terminal.txt)、[OMP 超时终端](../evidence/ticket22-startup-diagnostic-20261107-224624/omp-timeout-terminal.txt)。
- 后续须分别证明两个宿主的首次完整输入真正进入原生 user 历史、触发预期模型请求且精确一次；公开提交动作成功、会话登记或 idle 都不能单独当作就绪证明。不以随意 sleep／补发 Enter 的绕过结果认领修复。

## 二、14 个业务 case 的当前回归清单

下表是最近工作进度，不抹去历史失败。“本轮未重跑”不表示历史从未执行。较早完整 declared 矩阵还记录过不完整目标基线、批准全文不足、身份接线断言等失败；详见[历史交接](22-revise-bot.md)，没有完整当前通过证据前不推定已解决。历史公共模型别名不等于可核实的 Luna 身份，不能混算。

| Case | 最近状态／需回归内容 | 主要前置 |
| --- | --- | --- |
| `dedicated-chat` | 此前独立 Pi Chat 无模型请求超时；完整目标基线、来源、原生读回和独立使用仍未验证 | 首条输入；随后职责和文件授权 |
| `task-no-materials` | 既往真实 Luna 修订失败，w1 职责／授权理解偏差；暂缓完整重测 | 三类阻塞 |
| `task-materials` | 最近真实 Luna 修订在 w1 提前发送 manual report 处失败；w2–w5 未到达 | report；随后完整业务链 |
| `map-no-materials` | 本轮未重跑；历史曾有 `Incomplete guide offered for approval`；完整通过未验证 | 三类阻塞 |
| `map-materials` | 本轮未重跑；历史曾 workflow 未完成；完整通过未验证 | 三类阻塞 |
| `revise-unauthorized` | OMP 独立保存者等待回答超时，模型请求 0；不能认领正确拒绝 | 首条输入 |
| `revise-baseline-conflict` | 本轮未重跑；历史只有通过推断而缺完整成功证据，不算通过 | OMP 独立保存者入口 |
| `revise-target-conflict` | 本轮未重跑；同上 | OMP 独立保存者入口 |
| `revise-running` | 本轮未重跑；历史模型传输失败；运行禁写真实业务未验证 | OMP 独立保存者入口及等待中的真实 Run |
| `revise-revision` | 本轮已尝试，OMP 等待回答超时，模型请求 0；未实际触发业务 save／并发 revision 拒绝 | 首条输入 |
| `revise-applied` | 本轮未执行；保存报错但已生效的核实仍待验证 | OMP 独立保存者入口 |
| `revise-not-applied` | 本轮未执行；保存未生效的核实仍待验证 | 同上 |
| `revise-unknown` | 本轮未执行；保存效果不可确认的真实报告仍待验证 | 同上 |
| `revision-baseline-files` | 实际 junction 前置创建成功；首个 link Chat 无模型请求超时，link 未验证，traversal 未进入 | Pi 首条输入 |

以上没有可用于认领当前完整业务验收通过的证据。不要把未验证项自动归为产品缺陷，也不要把无副作用当作模型已正确拒绝。

**report 依赖重测包**：四个完整修订流程、w1／w2／w3／w4 manual 确认、三路全文批准与最终保存逐字一致、w5 一次保存／完整读回／目标和非目标保持、专属 Task 的 workflow 配对。详细清单见[report 联合重测记录](20261107-ticket22-report-retest.md)。

## 三、已完成的代码／机制检查（不是业务 case 通过）

| 检查 | 实际结果 | 证据 |
| --- | --- | --- |
| HTTP conflict 修复后的完整后端 | 78 个目标＋doctest 通过，未派发 0 | [回归总结](../evidence/regression-20261107-130509/summary.json) |
| 前端 | 244/244 通过；保留 jsdom scrollTo 警告 | 同上 |
| 两宿主原生工具＋真实后端联合用例 | 2/2，70 断言 | 同上；外部 Herdr 为受控 fixture，不是完整真实 Tab 联合业务 |
| toolkit 完整 Dock | 181/181，2360 断言 | 同上 |
| 真实 Herdr 精确关闭烟测 | 通过，cleanupErrors=[] | 同上；独立机制专项，不是 ticket22 五节点业务 |
| toolkit 类型检查 | 通过 | 同上及[最终重跑](../evidence/timeout10-20261107-134256/summary.json) |
| toolkit canonical | 1821/1821，231 文件，12330 断言，exit 0 | [10 秒预算后的最终结果](../evidence/timeout10-20261107-134256/summary.json) |
| 集成命令文件 | 6/6 通过 | 同上 |
| `workflow_dispatch shared_business` | 3/3 通过 | [独立检查总结](../evidence/ticket22-independent-20261107-143650/summary.json) |
| `workflow_dispatch revise_bot_guides` | 1/1 通过 | 同上 |
| 修订 CLI 拒绝／case 选择 | 6/6 通过 | 同上 |
| 安全进程公开发布／预检烟测 | 通过，errors=[] | 同上 |

canonical 曾有 1820 通过、1 个 5 秒超时失败；用户明确授权只将 `tests/integrations/cli.test.ts` 中 model-cost demand 用例外层预算改成 10 秒，断言未改，之后全仓重跑为上述 1821/1821。原失败仍保留。已有套件自动更新 `framework/docs/pi-acceptance.json`，并不表示 ticket22 通过。所有结果只适用于各日志记录的源码／产物／环境，不自动适用于后续修改。

## 四、模型、环境与清理边界

- 后续真实调用只允许 **`openai-codex/gpt-6-luna`**。使用 `scripts/accept-workflow-codex.py --scenario revise`，该入口锁定模型并校验服务端身份；不得回退默认模型、其他模型或受控 provider。
- 真模型最近业务 `task-materials` 为 48 次调用，另有 1 次预检；服务端身份均核对为 Luna。独立场景和最后两次业务诊断均为 0 次模型请求，不能以配置名代替实际调用证明。最后独立 Luna 预检为 1 次成功调用。
- `--hosts declared` 保留实际发布 Task 的混合宿主；不能为迁就 runner 全改为 Pi／OMP。Dock 的隔离 OMP 18.5.0 与实际业务终端 OMP 18.18.0 不是同一产物，不能混称。
- 两仓：`D:/developer/myhub/herdr-bot`、`D:/developer/myhub/my-agent-toolkit`；独立 target：`D:/developer/myhub/herdr-bot/backend/target`。Pi 1.5.0；模型传输 Node 为 `D:/Program Files/nodejs/node.exe`（22.23.1）；Bun 1.4.4。完整路径、源码／产物身份以每次 evidence／commands 为准。
- 独立配置、HOME／APPDATA／XDG、原生状态、命名 Herdr、业务与运行目录必须归测试所有；动态数值回环，关闭 mDNS，不连接用户 daemon／Chat，不执行真实关机、不自动安装或修改全局宿主。
- `task-materials` 原业务失败后有末轮回环响应 WinError 10054；原生 Luna 响应身份完整，报告提前发送由独立证据证实。连接中断原始记录保留，精确因果未建立，不把它另行宣称为已确认独立产品根因。
- 最近只读诊断无输入重发。临时诊断代码已移除，`workflow_create_acceptance.py` 与 `accept-workflow-nesting-revise.py` 的哈希均与诊断前一致。已观察所属进程 OMP 8 个／Pi 6 个全部确认退出，cleanupErrors=[]；临时模型配置已删除，授权文件未改动。此前独立三场景的清理也无错误。
- 保留未提交工作树及既有用户改动，不 reset／清理；本文不提交、不改票据生命周期、不编辑运行事实。

## 五、后续对话的建议回归顺序

三类可分别修复，以下仅是验证依赖顺序，不预先批准实现方案：

1. 先建立 Pi／OMP 独立 Chat 的可靠首次输入处理与观察边界，分别验证完整输入精确一次进入真实历史并开始处理；再恢复 `dedicated-chat` 和全部独立安全 case。
2. report 优化后验证批准与发送副作用的真实顺序、拒绝／取消／内容变更，再运行四个修订流程及其依赖步骤；不以模拟确认替代实际交互。
3. 提示词／职责改进先定向 `task-no-materials`，实际核实阶段、授权、真实工具调用与完整材料；这只是较窄反馈，不代表完整矩阵。
4. 最后补齐 Task／Map 有／无材料、三路独立全文批准、一次保存、全部负例、原生文件消费与完整专属 Task 配对。逐 case 保留成功／失败／未到达及原始日志；不得用最终 completed 推断过程。
5. 对相关修改重新执行受影响定向检查、类型检查和所属仓库回归；后端／前端／toolkit 规范入口见[测试指引](../../../docs/agents/testing.md)与 `D:/developer/myhub/my-agent-toolkit/docs/agents/testing.md`，真实业务入口要求见[修订验收说明](../../../docs/revise-bot-guides.md)。改变后的源码不能沿用旧绿为新结果。

只有全部归属验收项与必要仓库检查均有实际通过证据后，才讨论关闭 ticket22。不能因为三个主要阻塞被修复就提前认领剩余未测项通过。
