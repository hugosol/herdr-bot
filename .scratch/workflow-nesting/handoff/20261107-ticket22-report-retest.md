# ticket22：report 优化后的联合重测清单

最新总入口：[测试结果与三类阻塞最终交接](20261107-ticket22-three-blockers-pause.md)。本文件保留 report 依赖包与独立测试的详细证据，完整状态及历史／本轮区别以总交接为准。

用户授权：先记录 report 失败用例及其依赖用例，待 report 工具优化后一起重测；当前转向不依赖该确认边界的其他验证。暂缓不等于通过，不修改 ticket22 状态或运行内部状态。

## 已失败的直接用例

- `task-materials`：Task 型既有 Bot＋可选材料的五节点修订。
- 最近证据：`../evidence/ticket22-luna-20261107-135646/summary.json`。
- w1 的 `reportPolicy=manual`，只收到 `P22-CLARIFY-revise-task-materials` 业务澄清；没有收到 `P22-SEND-...-w1` 发送批准。
- 原生模型 `codex-completions/0044.json` 调用 `report(can_complete=true)`；后端 `backend-wire/0088.json` 返回 HTTP 200、`saved=true`，报告 `bc-4037f08940f4`。验收正确失败：`Baseline manual report preceded send authorization`。
- w2–w5 未执行，未观察到模型发起 Bot 保存；不能把材料生成或报告保存当作流程完成。
- 1 次预检及 48 次业务模型调用均核对服务端 `openai-codex/gpt-6-luna`。末轮回环响应投递发生 WinError 10054；此错误与已证实的提前报告分别保留。8 个已观察所属进程退出，cleanupErrors=[]，临时模型配置已移除、授权文件未改动。

## report 优化后须一起重测的直接／依赖用例

| Case／覆盖 | 当前处理 | 重测重点 |
| --- | --- | --- |
| `task-materials` | 已失败，等待重测 | 不得在发送批准前提交 w1 报告 |
| `task-no-materials` | 暂缓完整业务链重测 | 无材料合法缺席；同样经过 manual 边界 |
| `map-no-materials` | 暂缓完整业务链重测 | Map 真实目标读取和完整保留；同样经过 manual 边界 |
| `map-materials` | 暂缓完整业务链重测 | Map＋材料；同样经过 manual 边界 |
| 四个场景的 w2/w3/w4 | 依赖上游 w1 交接，未认领通过 | 每路全文批准与报告发送批准分离；拒绝／取消不发送，内容变更重新确认 |
| 四个场景的 w5 | 依赖三路交接，未认领通过 | 三份实际批准全文逐字匹配；真实 revision；一次合法三字段保存、完整读回、目标与非目标保持 |
| 专属 Task `task-revise-bot-baseline-v1` 的 workflow 配对部分 | 依赖完整流程 | 实际 Task/宿主身份、材料直接消费，不用 completed 替代过程证据 |
| P40/P42/P43/P44/P45 的完整业务覆盖结论 | 保持未验证 | 所有所属用例有实际证据后再认领；子集通过不能关闭票据 |

不得删掉批准步骤、放宽断言、模拟用户批准或以重复运行寻求偶然通过。未来 report UI 改变后，验收驱动需按真实交互调整，仍观察批准和实际发送副作用的先后关系；不预设尚未批准的设计。

## 当前可独立验证（不依赖 manual report）

- `revise-unauthorized`、`revise-baseline-conflict`、`revise-target-conflict`、`revise-running`：独立保存者 Chat，只读核实后拒绝不合法保存；running 额外有保持等待的真实 Run，不经过 manual 报告链。
- `revise-revision`：真实并发 revision 冲突，精确 409／bot-conflict，禁止盲重试。
- `revise-applied`、`revise-not-applied`、`revise-unknown`：真实 HTTP 故障边界及保存结果核实；均非完整五节点流程。
- `revision-baseline-files`：专属基线核实 Chat 面对实际 junction／越界的拒绝、原字节保持。
- `dedicated-chat` 不依赖 report，但受已知 Pi 独立 Chat 启动就绪问题另行阻塞；不为本轮补发 Enter/prompt、修改 Suite 或安装宿主来绕过。

以上真实宿主场景共用独立 Chat 启动设施；若实际遇到该已知启动故障，记录未进入／未验证，不因没有保存副作用而判负例通过。真实模型只允许 `openai-codex/gpt-6-luna`。每个场景各自独占证据、保留原错误和清理错误；模型传输或清理失败后不继续提交模型决策。已完成的受控机制回归只认领机制，不替代真实业务证据。

## 转向其他测试后的实际结果

证据：`../evidence/ticket22-independent-20261107-143650/summary.json`。

| 独立场景／检查 | 实际结果 |
| --- | --- |
| `revise-unauthorized`（OMP 保存者） | 已启动一次；公开 Chat `succeeded`／`inputSubmitted=true`，等待真实回答边界 300 秒超时；模型请求 0，未核实业务拒绝 |
| `revise-revision`（OMP 保存者） | 已启动一次；同样等待真实回答超时，模型请求 0；未触发实际业务 save／revision 冲突，不算通过 |
| `revision-baseline-files`（Pi 基线核实者） | 实际 junction 前置创建成功，第一项 link Chat 等待真实回答超时，模型请求 0；link 未验证，后续 traversal 未进入 |
| `workflow_dispatch shared_business` | 3/3 通过；公开材料消费、一次保存／完整目标保持及 stale revision 机制 |
| `workflow_dispatch revise_bot_guides` | 1/1 通过；五节点发布／材料接口机制 |
| `accept-workflow-nesting-revise.test.py` | 6/6 通过；CLI 缺依赖拒绝及 case 选择边界，不是模型业务 |
| `check-revise-bot-guides.py` | 安全进程 HTTP 发布／预检烟测通过，errors=[] |

三个真实场景均固定配置 `openai-codex/gpt-6-luna`，但本轮模型请求实际为 0，不能声称已验证服务端新响应或模型职责遵循。分别有 8、8、6 个已观察所属进程确认退出，cleanupErrors=[]，临时模型配置已移除。

与 report 无关的另一前置阻塞：独立 Chat 已登记／公开输入提交成功并不证明真实业务已开始。OMP 保存者也出现了无模型请求的回答观察超时；不能直接把它定性为此前 Pi 的已知启动 handler 问题。当前不补发输入、不改宿主／Suite、不以无保存副作用冒充正确拒绝。

用户随后授权只读诊断复跑，证据：`../evidence/ticket22-startup-diagnostic-20261107-224624/summary.json`。OMP `revise-unauthorized` 与 Pi `revision-baseline-files` 均重新复现，未补发输入。Pi 超时终端明确显示 `Startup is still in progress`，原始任务文字仍在编辑器；安装版 Pi 1.5.0 的 `interactive-mode.js:721,2590-2593` 对应启动提交分支只把文字恢复到编辑器，不提交或排队，故本次 Pi 也确认是过早提交。OMP 的提交后快照仍显示 PowerShell 启动命令，超时快照显示初始任务文字留在输入区，模型请求 0；输入滞留已观察到，但尚未确定丢失提交动作的具体内部处理分支，不能等同 Pi 的实现。单独 Luna 预检通过并核对服务端身份，两个业务诊断均没有模型请求。临时诊断代码已移除，两文件哈希与运行前一致；8／6 个已观察所属进程均退出，cleanupErrors=[]，临时模型配置已移除。该诊断不改变业务 case 的未验证状态。

剩余 `revise-baseline-conflict`、`revise-target-conflict`、`revise-running`、`revise-applied`、`revise-not-applied`、`revise-unknown` 共用上述 OMP 保存者 Chat 前置条件，本轮未再逐一启动，保持未验证；`dedicated-chat` 仍受原有独立 Chat 边界限制。恢复真实业务验证需要先具备可靠的独立 Chat 启动／回答观察前提，与 report 优化后的四场景联合重测分开跟踪。
