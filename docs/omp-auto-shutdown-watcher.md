# omp 无人值守长任务：会话状态推送 → 外部客户端 → 自动关机（设计决策记录）

- 日期：2026-09-29
- 目标机：本机 Windows（win32），omp 与关机目标是同一台机器
- 被调查的仓库：`E:\Developer\myhub\oh-my-pi`（`packages/coding-agent` 为主）
- 状态：**方向已变更并确认**（从"外部读文件"改为"扩展推送 + 外部客户端消费"）；事件格式与详细设计待下一阶段
- 版本：v2（v1 为"外部 companion 轮询 session 文件"方案，本文件已按推送模型重写）

---

## 1. 需求与约束

| 项 | 内容 |
|---|---|
| 场景 | 用 omp 跑超长自动化任务后离开，任务结束时由外部客户端推消息并自动关机 |
| 侵入性 | 最低：你照常使用 TUI 观察输出；omp 侧只增加一个**被动加载**的全局扩展 |
| 触发时机 | agent 一轮对话结束、真正停下来等用户 |
| 延迟 | 可接受（秒级到几十秒级） |
| 稳定性 | 尽量稳定；**防误触发优先**（误关机不可撤销） |
| 开关 | 运行时开关（内存态），开启时才注册推送；**开启时必须先确认客户端已启动，否则提示并中止**（不进入 armed） |
| 取消 | 客户端侧显示倒计时，可取消；取消后回到监控 |
| 关机 | 通知各进程释放资源/保存文件并留出时间，但**最终不能被阻塞**（无人值守） |
| 通道 | **Windows 命名管道**（扩展 → 常驻客户端），不走磁盘 journal |
| 常驻 | omp 侧无常驻逻辑（扩展被动加载、未 arm 时零动作）；客户端本身是常驻进程（用户自己启动/维护） |
| 作用域 | 全局扩展（任何会话都会加载），由运行时开关决定是否生效 |

---

## 2. 最终方案（推送模型）

**形态：一个全局扩展（被动加载）+ 一个你手动启动的常驻客户端。你用斜杠命令 arm；扩展从那一刻起把会话事件按固定格式写进命名管道；客户端收到事件后执行倒计时/关机等动作。**

```
   [你手动启动客户端]  ← 监听 \\.\pipe\<name>（常驻）
              │
              ▼
   [omp TUI] 扩展被动加载（未 arm 时零动作）
              │
        斜杠命令 arm ──► 尝试连接命名管道
              │             ├─ 失败（客户端没起）→ ui.notify 提示 + 中止（保持未 arm）
              │             └─ 成功 → armed
              ▼
   armed 期间：选定事件 → 序列化一行 JSON → 写入管道（不 await 网络）
              │
              ├─ 写失败/断线 → 自动 disarm + 通知
              └─ session_shutdown → 发最后一条 + 关闭
              ▼
   [客户端] 收到 session_stop 类事件 → 倒计时（可见、可取消）
              ├─ 倒计时期间收到"新活动"事件 → 取消 → 回到监控
              └─ 倒计时结束 → EWX_FORCEIFHUNG 关机
```

要点：

- 扩展**全局**安装（`~/.omp/agent/extensions/` 或 `.omp/extensions/`），**被动**加载，未 arm 时不做任何事。
- **开关是内存态**（斜杠命令），不落盘；omp 重启后回到未 arm。
- **arm 时必须先连上客户端**，连不上就提示并中止——这是整套方案的 armed 闸门。
- 事件推送是**单向**（扩展 → 客户端）；客户端不做请求-响应。
- 客户端侧负责：倒计时 UI、按键取消、"新活动则取消"、以及最终关机（见 §4 Q18）。

---

## 3. 关键事实（带证据）

### 3.1 不能 attach 到已运行的 omp，也没有对外事件通道

- RPC 模式是**启动时**经 `--mode rpc` 选定的 stdio NDJSON（`modes/rpc/rpc-mode.ts:781,786-795`），客户端自己 spawn 新进程（`modes/rpc/rpc-client.ts:332`），没有 attach flag。
- 进程不会主动向任何 socket / HTTP 推送事件（**本次方案的推送能力来自扩展，而不是 omp 自带的对外通道**）。
- 仓库里的两个本地管道都不是 agent 事件通道：collab registry `\\.\pipe\omp-collab-<entryId>`（ops 仅 `snapshot|link`，`collab/registry.ts:455-468`）；daemon broker `\\.\pipe\omp-daemon-<hash>`（`launch/protocol.ts:30-52`，只服务 omp 后台守护进程）。
- session JSONL 是 omp 自带的唯一"进程外可观测面"——**本方案已不再依赖它作为主判据**（保留为客户端侧的兜底/自检，见 3.4）。

### 3.2 "提示音"的真相（TUI 专属，只作体感参考）

- 提示音是交互 TUI 写给终端的通知序列（BEL `\x07` / OSC 9 / OSC 99），唯一触发链在 EventController（`modes/interactive-mode.ts:1559` 唯一构造点）：`agent_end (isTerminal !== false)` → `#handleAgentEnd`（`modes/controllers/event-controller.ts:2033`；`isStreaming` 早退 2043；`isTerminal === false` 早退 2046）→ `sendCompletionNotification` / `sendErrorNotification` → `TERMINAL.sendNotification`（`packages/tui/src/terminal-capabilities.ts:234`）。
- 三个设置默认值：`completion.notify` 开、`ask.notify` 开（`tools/ask.ts:790`）、`error.notify` **关**（`modes/settings.ts:1065-1089,1110`）。
- print / RPC / headless 没有这个时刻（RPC 还设 `PI_NOTIFICATIONS=off`，`modes/rpc/rpc-mode.ts:777`）。
- 结论：提示音不能被外部观测；**推送模型直接使用驱动它的进程内事件**，比提示音更权威。

### 3.3 扩展推送面（本方案的核心，已核实）

- **加载方式**：默认运行时只有一套 `ExtensionRunner`；`.omp/extensions/`、`~/.omp/agent/extensions/` 自动发现；`.omp/hooks/pre|post/*` 并入同一管线（`extensibility/extensions/loader.ts:179,621-632`）；`--hook` == `--extension`；print 模式也加载（`print-mode.ts:157`）。**不需要改启动方式。**
- **事件表**（`extensibility/extensions/types.ts:1280-1345` + `extensibility/shared-events.ts`）：
  - 消息级：`message_start` / `message_update` / `message_end`（+ 用户输入提交事件）；
  - 回合级：`turn_start` / `turn_end`；`tool_call` / `tool_result` / `tool_execution_*`；
  - 会话级：`session_start` / `session_stop` / `session_shutdown` / `agent_end`；
  - 其它：`auto_retry_*`、`auto_compaction_*`、`mcp_notification`、`resources_discover` 等。
- **`session_stop` 是权威触发信号**：在 settle 路径的**最后一道闸门**发出（所有自动续跑判定之后）、awaited、只对 main 会话发（子代理不发）、异步任务未排空前会**推迟**（`agent-session.ts:4535-4590`、`docs/extensions.md:350`、`test/agent-session-todo-reminder-async-jobs.test.ts:238-255`）；handler 返回 `continue` 可再跑一轮，上限 8 次（`SESSION_STOP_CONTINUATION_CAP = 8`，`agent-session.ts:418`），返回 `block` 优先且不耗额度（`extensions/runner.ts:1512-1519`）。
- **`agent_end.willContinue` 只是"每个 settle 都发"的通知型信号**：自动重试 saga 中每一次失败 attempt 都会发且 `willContinue=true`（`test/agent-session-retry-cap.test.ts:1877-1941`），只有终态 settle 才为 falsy；扩展**拿不到** display 事件的 `isTerminal`/`yielded`（那是 TUI/RPC 侧内部字段）。
- **推荐触发条件**：`session_stop`（handler 不返回 `continue`/`block`）+ 自检 `ctx.isIdle()` / `ctx.hasPendingMessages()` / `ctx.getAsyncJobSnapshot()`（`extensions/types.ts:452-563`）。若要"终态全覆盖"，再叠加 `agent_end && willContinue !== true` 兜底下面 7 类**不发 `session_stop` 的终态**：无 assistant 消息、skip-post-turn-maintenance、successful yield、compaction 的 deferredHandoff / automaticContinuationBlocked、最后一条消息仍带 toolCall、abort、子代理。
- **与提示音的关系**：TUI 响铃对应的是 display `agent_end`（`isTerminal !== false`，可能被 prompt 收尾推迟，`event-controller.ts:2033-2078`），**不是**扩展订阅的同一个事件；但 `session_stop` 必定在铃响之前跑完。print/RPC/TUI/ACP 在 AgentSession 层行为一致（同一 runner），差异只在 display 通知与完成等待策略（RPC 的完成判定 `modes/rpc/rpc-session-settle.ts:21-28,58-59`）。
- **handler 的调度与预算**：handler 被 agent loop **串行 await**；通用预算 30s，`session_shutdown` 仅 **2s**（`extensions/runner.ts:100,124,135-138`；超时/竞速见 `262-346`、`~1400-1483`；分发 `1455-1526`）。**⇒ 推送动作必须便宜，绝不能 await 网络。**
- **无热重载**：运行中的会话不会因为扩展文件变化而重绑 handler（模块导入 / mtime cache-bust 见 `loader.ts:416-419,485-488`、`legacy-pi-compat.ts:1118-1125,1190-1195`；`AgentSession.reload:10045-10049`）。**⇒ 开关必须是 handler 体内的运行时判断。**
- **扩展能力**：进程内、无沙箱的 TS/JS 模块；`node:fs`、`Bun.*`、`@oh-my-pi/pi-utils` 都可用（兼容层只重映射 `@*/pi-*` 等 specifier，`legacy-pi-compat.ts:795-808,1126-1175`）。
- **命令与控制**：`registerCommand` / `registerFlag` / `getFlag`（`extensions/types.ts:1410-1443`），命令上下文见 `572-601`；`ctx.ui.notify` 用于提示（`types.ts:258`）。
- **命名管道的可行先例**：Windows 命名管道用 `net.createConnection({ path: "\\\\.\\pipe\\<name>" })`（`launch/client.ts:110-112`）；POSIX 用 `Bun.connect({ unix })`（`dap/client.ts`）。
- **可用的中央工具**（别自己造）：`@oh-my-pi/pi-utils` 的 `TempDir` / `removeWithRetries` / `withFileLock`（Windows 命名互斥的跨进程锁）/ `openCloexecSync`（**持有 append fd 跨子进程时必须用，Bun 不设 O_CLOEXEC**）/ `parseJsonlLenient`（`packages/utils/src/index.ts`、`fs-open.ts`、`file-lock.ts`）。

### 3.4 session JSONL（客户端侧的兜底 / 自检，不再是主判据）

- 路径：`<agentDir>/sessions/<cwd-slug>/<ISO>_<sessionId>.jsonl`，`agentDir` 默认 `~/.omp/agent`（`session/session-paths.ts:45-47`、`session/session-manager.ts:123-125`、`packages/utils/src/dirs.ts:899-902`）；每条 entry 一次 `fs.writeSync`、无 fsync、对同机进程立即可见。
- **没有任何"本轮 settle"entry**；assistant 消息在 `message_end` 整条落盘（`agent-session.ts:3459-3462 → 3147-3241 → 3124-3145`）；轮内唯一中途写入是 `tool_execution_start`（`2808-2819`）；`session_exit` 只在 dispose / postmortem 写（`5136-5137`、`1936-1938`、`2822-2860`），交互 TUI 不退出就永远没有。
- 仓库自带的回合判定（若客户端要做自检可复刻）：`SessionStatus = complete | interrupted | aborted | error | pending | unknown`（`session/session-listing.ts:29`）；算法为"读末尾 32KB、倒序找最后一条 `type === "message"` entry、按 role/stopReason 分类"（`deriveSessionStatus:162-182`、`statusFromTailMessage:194-221`）：assistant 无 toolCall 且正常收尾 → `complete`；含 toolCall 或 `stopReason:"length"` → `interrupted`；`toolResult` → `interrupted`；`user` → `pending`。**该函数未导出**，需要直接引源码路径或复刻。
- 会话定位（若客户端需要）：terminal breadcrumb `<agentDir>/terminal-sessions/<terminalId>` 记录 `cwd/sessionFile/fresh/cwdstat`（`session-paths.ts:308-336` 写、`338-383` 读）；进程存活看 `~/.omp/run/daemons/<hash>/clients/<pid>-<uuid>.json`（`launch/presence.ts:36-52`，`process.kill(pid,0)` 判活 `60-101`）；可照抄 `cli/ps-data.ts:66-124`、`cli/gc-cli.ts:379-397`、`session-listing.ts:702-708`。

### 3.5 其它路径的事实（备选，记录备用）

- **wrapper 包住 `omp -p`**：判据最硬（`$LASTEXITCODE`）、代码最少，但改变启动方式、看不到 TUI。
- **分离子进程能活过 omp 退出**：omp 退出是硬 `process.exit()`，无 Job Object、不等待也不清理子进程；正确写法 `Bun.spawn(..., { stdio: "ignore" })` + `unref()`（Windows 用 `windowsHide` 而非 `detached`），先例 `launch/client.ts:320-328`、`launch/broker.ts:884-908`；**不要用 `pi.exec`**（await + 超时整树 kill，`packages/utils/src/ptree.ts:673`）。
- **项目作用域扩展 + 分离 sleeper**：与本次方案的差别只是"客户端形态"，推送面事实相同。

---

## 4. 决策清单（问题 → 结论 → 理由）

| # | 问题 | 结论 | 理由 |
|---|---|---|---|
| Q1 | 交付形态 | **受开关控制的全局扩展 + 常驻客户端**（v2 变更） | 进程内事件比文件启发式精确；开关解决误触发 |
| Q2 | "任务结束"的定义 | agent 一轮结束、真正停下等用户 | 与提示音体感一致，且 `session_stop` / `agent_end(willContinue=false)` 可精确表达 |
| Q3 | 外部程序 | 自研常驻客户端（Rust / Python / Bun 均可） | 只需监听命名管道、显示倒计时、执行关机 |
| Q4 | 关机条件 | 被监控任务无论成功/失败/中断都进入关机流程 | 无人值守；失败若无信号会一直挂着 |
| Q5 | 目标机器 | 本机 Windows | omp 与关机目标同机 |
| Q6 | 精确 vs 语义 | **精确**（直接用进程内事件） | 推送模型不再需要文件启发式 |
| Q7 | armed 闸门 | 斜杠命令 arm；**arm 时连不上客户端就提示并中止** | 只有你亲手 arm 且客户端在场时才生效 |
| Q10 | 观测通道 | **扩展 → Windows 命名管道 → 常驻客户端**（v2 变更） | 延迟低、无磁盘中转；先例已在仓库中验证 |
| Q11 | 监控范围 | 全局扩展，靠运行时开关门控（v2 变更） | 用户明确选择全局扩展 + 开关 |
| Q12 | 稳定性优先级 | 防误触发优先 | 误关机不可撤销 |
| Q13 | 运行形式 | 扩展被动加载（未 arm 零动作）；客户端常驻 | 满足了"omp 侧无常驻逻辑" |
| Q14 | 文件轮询 companion | **降级为兜底/自检**（不再作为主方案） | 推送模型已覆盖其用途 |
| Q15 | 无头还是 TUI | 交互 TUI | 用户要求照常观察输出 |
| Q16 | 取消的粒度 | 取消本次倒计时，回到监控；下一轮结束再次倒计时 | 符合原始描述 |
| Q17 | 倒计时期间出现新活动 | 客户端自动取消并回到监控 | 兜住"用户刚好在倒计时里打字" |
| Q18 | 关机方式 | `ExitWindowsEx(EWX_SHUTDOWN \| EWX_FORCEIFHUNG)` | 见下方"为什么不用 shutdown.exe" |
| Q19 | omp 进程消失 | 客户端在管道断开/收到 `session_shutdown` 时按"任务结束"处理 | Q4"任何情况都关机"的延伸 |
| Q20 | 推送通道 | **Windows 命名管道直连常驻客户端**（v2 变更） | 用户选择；客户端必须在场 | 
| Q21 | 开关形态 | **内存态斜杠命令**（不落盘） | 用户选择；omp 重启后回到未 arm |
| Q22 | 推送事件集 | **待详细设计阶段确定**（候选：`session_start`、`message_end`、`session_stop`、`agent_end`、`session_shutdown`，以及倒计时期间用于"取消"的活动事件） | 用户要求下一阶段再定 |
| Q23 | 作用域 | 全局扩展 + 开关 | 用户选择 |
| Q24 | 方向变更 | 确认：文档按推送模型重写（本文件） | — |

### 为什么不用 `shutdown.exe`（Q18 的理由）

- `shutdown /s /t N`（N>0）**隐含 `/f`**：直接强制关闭、不发通知，可能丢数据（[MS Learn: shutdown](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/shutdown)）。
- `shutdown /s /t 0`（不加 `/f`）才是优雅通知，但只要有应用拒绝，它会**无限期停在"应用阻止关机"**——无人值守下等于永不关机。
- 需要的语义恰好是 [`ExitWindowsEx` 的 `EWX_FORCEIFHUNG`](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-exitwindowsex)：向所有进程发 `WM_QUERYENDSESSION` / `WM_ENDSESSION` 让它们释放资源、保存文件，给一个有界窗口，**只强制超时未响应者**，因此不可能被阻塞（需要 `SeShutdownPrivilege`，属进程常规权限）。
- 零原生代码的退路：两段式——先 `shutdown /s /t 0` 优雅，同时挂看门狗，N 秒仍未关成则 `shutdown /s /f /t 0` 强制。

---

## 5. 被否决 / 降级的方案

| 方案 | 处置 | 理由 |
|---|---|---|
| attach 到已运行的 omp 进程 | 否决 | 不存在：无对外控制通道，RPC 必须启动时选定 |
| 外部 companion 轮询 session JSONL | **降级为兜底/自检** | 判定是启发式（长工具、后台任务挂起都难分），推送模型可精确表达 |
| wrapper 包住 `omp -p` | 否决（保留为极端简单备选） | 判据最硬但改变启动方式、看不到 TUI |
| 项目作用域扩展 + 分离 sleeper | 否决（与 v2 同源，只是客户端形态不同） | 用户选择常驻客户端 + 命名管道 |
| Windows 计划任务轮询 | 否决 | 引入调度 + 文件启发式双重复杂度 |
| 嗅探终端 OSC/BEL | 否决 | 需要 PTY / 会破坏 TUI 显示 |
| 磁盘 journal 中转 | 否决（本轮） | 用户选择直连命名管道；若将来要 fail-safe 回放可再叠加 |

---

## 6. 已接受的风险与假设

1. **客户端不在场时整套方案不可用**：这是设计选择（arm 时即校验）。客户端中途死亡 → 写管道失败 → 扩展自动 disarm 并通知；**不会**静默关机，也不会自动改走别的通道。
2. **写管道的背压风险**：handler 被 agent loop 串行 await，若客户端读取慢、管道缓冲区满，同步写会卡住 agent。**设计约束**：handler 内只做异步 write + 有界队列，队列超阈值即自动 disarm + 通知；绝不 await 网络。
3. **安全边界**：命名管道是"能触发关机"的入口。管道名不可预测、仅当前用户可写（Windows 默认 ACL），并避免在共享机器上使用。
4. **开关是内存态**：omp 重启 / 会话重载后回到未 arm；需要重新 arm（这是有意的安全属性，不是缺陷）。
5. **事件集与格式未定**（Q22）：倒计时期间"新活动自动取消"（Q17）需要扩展推送活动类事件——事件集必须覆盖这一点，否则该安全网失效。
6. **触发条件的已知缺口（已核实，不再是未知项）**：没有任何单一事件能覆盖全部终态。`session_stop` 是权威信号（在所有自动续跑判定之后发出；异步任务未排空前会**推迟**，因此"后台任务挂起导致提前关机"的风险已解除），但**有 7 类终态不发它**：无 assistant 消息、skip-post-turn-maintenance、successful yield、compaction 的 deferredHandoff / automaticContinuationBlocked、最后一条消息仍带 toolCall、abort、子代理。**实现要求**：以 `session_stop` 为主触发 + `ctx.isIdle()/hasPendingMessages()/getAsyncJobSnapshot()` 自检；若要覆盖 abort 等终态，再叠加 `agent_end && willContinue !== true`（代价：自动重试期间会发多次，客户端必须去抖）。
7. **判定依赖 omp 内部事件与文件格式**（非公开契约）：扩展事件表是公开文档化的（`docs/extensions.md`），但 `session_stop` 的细节实现可能随版本变化。
8. **fail-safe 的方向**：omp 侧不会主动关机；只有 armed 的扩展 + 在线的客户端同时成立才可能触发。
9. `--no-session` 不影响本方案（事件推送不依赖 session 文件）。

---

## 7. 事件格式草案（待详细设计阶段定稿）

建议的推送行（一行一个 JSON，NDJSON over named pipe）：

```json
{ "v": 1, "ts": "2026-09-29T12:34:56.789Z", "sessionId": "…", "cwd": "E:\\…", "kind": "session_stop", "data": { } }
```

- `kind` 候选：`session_start`（注册/心跳）、`message_end`（携带助手最终文本）、`session_stop`（**主触发**）、`agent_end`（兜底，带 `willContinue`）、`session_shutdown`（进程退出）、以及倒计时期间用于"取消"的活动事件（候选：`message_start` / `turn_start` / 用户输入提交）。
- 扩展侧 arm 时的校验握手：连接成功后先发一条 `session_start`（含版本/会话 id/cwd），客户端据此确认。
- 协议版本 `v` 用于将来演进。

**扩展侧伪代码（草案）**

```ts
// 全局扩展：默认被动；arm 后才推送
let armed = false;
let conn: net.Socket | null = null;
let queue: string[] = [];               // 有界；超阈值 → disarm + notify

export default function (pi: ExtensionAPI) {
  pi.registerCommand("shutdown-watch", {
    description: "arm/disarm the shutdown watcher",
    handler: async (args, ctx) => {
      if (args.trim() === "off") { disarm(); return; }
      try {
        conn = await connectPipe(PIPE_NAME);   // net.createConnection({ path })
      } catch {
        ctx.ui.notify("客户端未启动，已中止", "error");
        return;                                 // 保持未 arm
      }
      conn.on("error", () => disarmAndNotify("客户端断开"));
      armed = true;
      send({ kind: "session_start", sessionId: ctx.sessionManager.getSessionId?.() });
    },
  });

  pi.on("session_stop", (_e, ctx) => {
    if (!armed) return;
    send({ kind: "session_stop" });             // 不 await 网络
    if (queue.length > MAX_QUEUE) disarmAndNotify("推送积压");
  });
  pi.on("session_shutdown", () => { if (armed) { send({ kind: "session_shutdown" }); flushSync(); } });
}
```

（实现细节：使用 `pi-utils` 的 `openCloexecSync` 处理需要跨子进程持有的 fd；队列与异步 flush 不阻塞 handler；`ctx.ui.notify` 用于所有提示。）

---

## 8. 下一步

1. **详细设计阶段**（用户指定）：确定 Q22 事件集与 §7 的事件格式/schema、管道命名与握手协议、队列/背压策略、"新活动取消倒计时"的触发事件。
2. **实现**：扩展（全局安装、斜杠命令开关、命名管道推送）+ 客户端（监听管道、倒计时 UI、按键取消、`EWX_FORCEIFHUNG` 关机）。
3. **验证清单**：先启动/不启动客户端各 arm 一次；长任务中途 arm；倒计时取消与新活动取消；杀客户端；omp 正常退出与强杀；确认后台任务挂起时 `session_stop` 确实被推迟到任务排空之后；确认选定的触发条件对 abort / 无 assistant 消息这两类终态的处理符合预期（风险 6）。

---

## 9. 参考

**外部文档**

- [shutdown 命令（Microsoft Learn）](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/shutdown)
- [ExitWindowsEx 函数（Microsoft Learn）](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-exitwindowsex)

**本仓库关键位置**

- 扩展事件表与 API：`packages/coding-agent/src/extensibility/extensions/types.ts`、`extensibility/shared-events.ts`、`docs/extensions.md`
- 扩展加载与发现：`extensibility/extensions/loader.ts`、`docs/extension-loading.md`、`src/discovery/builtin.ts`
- handler 预算与分发：`extensibility/extensions/runner.ts`
- 会话事件发射：`src/session/agent-session.ts`（`session_stop`、`agent_end`、`willContinue`、`#hasPendingAsyncWake`）
- 命名管道先例：`src/launch/client.ts`（`net.createConnection({path})`）、`src/dap/client.ts`（`Bun.connect({unix})`）
- 中央工具：`packages/utils/src/index.ts`、`fs-open.ts`、`file-lock.ts`
- 提示音链路（参考）：`src/modes/controllers/event-controller.ts`、`packages/tui/src/terminal-capabilities.ts`
- 文件兜底/自检：`src/session/session-listing.ts`、`src/session/session-storage.ts`、`src/launch/presence.ts`、`packages/stats/src/parser.ts`
