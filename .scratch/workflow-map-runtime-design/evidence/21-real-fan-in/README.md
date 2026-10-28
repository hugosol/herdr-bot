# 21 实际执行证据：真实验收受阻

本目录不是 P34/P62/P63 通过证明，也不是 H1 完整证据包。没有启动真实 manager/node，没有 Run ID、真实三份产物、角色工具结果、原生 Chat、三份运行记录或页面截图；不创建空文件充数，不修改业务记录。票据不能据此关闭。

## 已执行

1. 读取当前 `meta-data/task-options.toml`，核对两个固定角色入口。实际部署目录只有 `codebase-tools.ts` 和 `connect-bot.ts`；`Test-Path -LiteralPath … -PathType Leaf` 对 manager-chat.ts 和 node-chat.ts 均返回 false。`prerequisites.json` 保存实际检查时间、metadata SHA-256、精确入口、命令位置和未验证范围。系统返回的 UTC 时间原样保留，不人为改为会话日期。
2. 在 PowerShell 通过 `Get-Command herdr,omp` 找到可执行入口；这不证明真实宿主隔离、扩展加载或模型可用。原始第一次经 shell 嵌套传参的前置命令因 `$roles` 被外层展开而出现 PowerShell 空管道解析错误，未执行检查；改用 Python subprocess 的参数数组传递相同 PowerShell 表达式后得到上述实际结果，不把前一次解析错误误报为环境故障。
3. 使用 `subprocess.run(['powershell','-NoProfile','-Command', <指南准备代码块>])` 执行长期指南的第一个 PowerShell 代码块，exit 0。`preparation.json` 保存原始输出。结果恰有三个 `actualArtifactsExist=false`，Run 目录为空；这是输入准备的事实，不是运行成功。`prepared/` 保存本轮实际生成的 Map、Bot、manager input 和三份独立预期。Bot 故意未绑定 Task，不可启动。
4. 用相同入口执行指南第二个 PowerShell 代码块，先将 `$root` 设为准备结果。exit 1，明确在 A 实际产物不存在时抛 FileNotFoundException，见 `missing-artifact-check.json`。该负向检查证明缺失材料不会被此核对片段补造或冒充通过；不是 P62.01，也没有虚构模型 red/green。
5. 归档输入后，仅删除上述本轮随机 Temp 根，exit 0，`existsAfterCleanup=false`，见 `cleanup.json`。没有服务、agent 或 Herdr 子进程待清理；未访问默认用户 Herdr 实例、对话或产物。归档中的绝对路径仅对应已清理的历史准备目录；重复执行必须按长期指南生成全新目录，不复用归档路径。

## 验收归属

| criterion | 实际结论 | 缺少的证据 |
| --- | --- | --- |
| P34.01 | 阻塞，未执行 | 真实 manager 工作中/空闲/collect 等待三类现场，以及报告标识、Herdr 提交和独立 collect 的关联。 |
| P62.01 | 阻塞，未执行 | 真实 Agent A/B→C、C 实读两份预定材料、三份实际产物、工具及身份、三份运行记录和页面。只有启动前准备已完成。 |
| P63.01 | 阻塞，未执行 | 真实报告后收尾与 complete、显式 C dispatch、finish 和实际 Chat 活动顺序；没有真实停轮或继续输入可记录。准备方案中的 gate 是拟执行的协调，不是已发生干预。 |
| H1/P69 | 未交付完整证据，用户未判断 | 不以“人工待判断”掩盖前三项缺失；须先由执行 Agent 补齐真实验证。 |

长期入口为 `docs/workflow-fan-in-acceptance.md`，运行说明已链接它。固定预期、C 的预定绝对材料地址、三个真实通知现场、报告后收尾 gate、完整干预/时间线与 H1 交接要求均在长期指南中，不依赖本临时证据目录解释能力。

精确外部阻塞：当前 metadata 指向的独立 my-agent-toolkit 真实 `manager-chat.ts`、`node-chat.ts` 未部署。没有跨仓源码交付授权/工作区，未在本仓库伪造角色入口或另造客户端。独占 Herdr、模型访问、实际 Prompt/Session/HTTP/取消、上游单节点真实闭环也仍需验证，不从入口缺失推断它们一定失败。角色部署与所有真实前置满足后，从新独占目录执行长期指南全程，并保存全新证据；本目录不能解除上游阻塞。

未执行后端/前端回归或浏览器构建：本轮只改指南及保存前置/准备证据，没有可用的真实角色入口，常规受控回归不能证明本票真实 seam。没有用跳过、替身或既有确定性测试代替本票通过结果。
