# Herdr：关闭 tab、进程生命周期与 Pi / OMP 会话恢复调查

调查日期：2026-10-22。

源码仓库：`E:\Developer\myhub\herdr`。下列源码路径均相对此仓库，行号是调查时的位置，后续可能变化。

## 1. 结论速览

- 关闭 tab：`herdr tab close <tab_id>`。
- 关闭 tab 不是分离客户端：会清理其中的终端，并主动终止 shell 及相关会话进程 / 进程树。
- 要离开界面而保持任务运行：默认快捷键 `Ctrl+B`，然后 `q`；再次运行 `herdr` 重连。
- 命名 Herdr 会话通过 `herdr session attach <name>` 重连，没有独立的 `herdr resume` 子命令。
- 服务器停止或机器重启后，原来的进程不会复活。Herdr 恢复保存的布局，并可重新启动受支持的智能体，调用智能体自身的恢复参数加载对话。
- Pi 的内置恢复命令：`pi --session <会话文件绝对路径或ID>`。
- oh-my-pi（OMP）的内置恢复命令：`omp --resume=<会话文件绝对路径或ID>`。
- 可以主动使用 `herdr agent start ... -- <智能体参数>`，在空闲 shell 窗格中重新启动 Pi / OMP 并加载指定对话。
- Herdr 的 session 名称、Pi / OMP 的对话 ID、Herdr 智能体名称和 pane ID 是不同概念，不能混用。

## 2. 关闭 tab 的命令与边界

```bash
# 获取 tab ID
herdr tab list
herdr tab list --workspace <workspace_id>

# 关闭指定 tab（示例 ID）
herdr tab close w1:t2
```

默认关闭当前 tab 快捷键：`Ctrl+B`，然后 `Shift+X`。tab 右键菜单的 Close 也走关闭路径。

关闭工作区的最后一个 tab 会同时关闭该工作区。启用 `ui.confirm_close` 时，TUI 关闭最后一个 tab 会要求确认；普通 CLI/API 关闭通常立即执行。如果关闭会连带关闭整个 worktree group，可能返回 `confirmation_required`，不能把返回错误当成关闭成功。

Socket API 对应方法：

```json
{"id":"close-1","method":"tab.close","params":{"tab_id":"w1:t2"}}
```

### 关闭 tab 后，进程还在吗？

普通情况下不会保留。源码路径概括：

```text
CLI tab close / TUI 关闭快捷键 / 菜单 Close
  → API tab.close
  → 移除 tab（最后一个 tab 则关闭 workspace）
  → 清理不再被 pane 引用的 terminal
  → TerminalRuntime.shutdown
  → PaneRuntime.shutdown
  → 停止检测和 PTY I/O，终止 shell 及相关进程
```

终止策略会分阶段执行 Hangup、Terminate、Kill，每阶段等待最多约 250ms：

- Linux / macOS：按 shell 的 session ID 收集进程，发送 SIGHUP、SIGTERM、SIGKILL。
- Windows：收集 shell PID 和进程树后代；Terminate / Kill 阶段调用 TerminateProcess。

不能将其表述成“保证杀死所有曾经由该 shell 启动的进程”。例如 Unix 下自行 setsid / daemonize 脱离原 session 的程序可能不在枚举范围内；权限或竞态也可能导致终止失败。普通 `command &` 不应被当成关闭 tab 后可靠保活的方式。

主要源码依据：

- `src/cli/tab_bindings.rs:51–66`：CLI 关闭命令映射。
- `src/app/api/tabs.rs:216–291`：服务端关闭 tab。
- `src/app/actions.rs:628–645,671–695`：终端引用检查与关闭工作区清理。
- `src/app/runtime.rs:30–40`：移除 runtime 并 shutdown。
- `src/terminal/runtime.rs:19–22`、`src/pane.rs:2233–2247`：shutdown 路径。
- `src/pane.rs:1760–1808`：分阶段终止策略。
- `src/platform/linux.rs:800–839`。
- `src/platform/macos.rs:1126–1139,1180–1195`。
- `src/platform/windows.rs:2710–2746`。

## 3. 分离客户端和重新连接

```bash
# 默认会话：启动或重新连接
herdr

# 查看命名会话
herdr session list

# 连接命名会话
herdr session attach work

# 也可通过全局选项使用或创建命名会话
herdr --session work
```

默认 `Ctrl+B`，然后 `q` 分离客户端。客户端关闭或连接断开，不等于关闭 tab；服务器仍在时，窗格和任务继续运行。

```bash
# 这些是停止，不是 detach
herdr server stop
herdr session stop work

# 删除已经停止的命名会话
herdr session delete work
```

不要混淆：

| 情况 | 再次连接后的含义 |
| --- | --- |
| 客户端 detach，服务器仍运行 | 连接仍然存活的原终端和进程 |
| 服务器完全停止或机器重启 | 根据持久化状态重建；不是恢复原进程 |
| 手动关闭 tab | attach 不会撤销关闭，也不会复活被终止的进程 |

依据：`docs/next/website/src/content/docs/zh-cn/persistence-remote.mdx:10–40`；客户端 detach 与断线处理见 `src/client/endpoint/registry.rs:365–372`、`src/server/headless.rs:946–992,2640–2650`。

## 4. 服务器重启后，Pi / OMP 对话是怎么恢复的？

### 4.1 运行时记录对话引用

Herdr 的 Pi / OMP 集成扩展读取：

```typescript
ctx.sessionManager.getSessionFile()
ctx.sessionManager.getSessionId()
```

优先上报会话文件的绝对路径，拿不到有效路径才使用 ID。上报通过 socket API 的 `pane.report_agent_session` 以及带会话引用的 `pane.report_agent` 完成。

- Pi 在 `session_start`、`agent_start` 等事件更新并上报引用。
- OMP 还监听 `session_switch`，切换对话后更新恢复目标。
- Herdr 把相应的会话引用保存在持久化状态里。

源码：

- `src/integration/assets/pi/herdr-agent-state.ts:74–130,229–250`。
- `src/integration/assets/omp/herdr-agent-state.ts:99–164,375–397`。
- `src/agent_resume.rs:115–131`：官方来源检查和路径优先策略。

### 4.2 重启时生成并执行恢复命令

```text
Pi / OMP 运行
  → 集成扩展上报当前对话路径 / ID
  → Herdr 保存引用
服务器停止或机器重启
  → 再次启动 Herdr
  → 读取保存的布局及会话引用
  → 创建新的终端 / 智能体进程
  → 使用智能体自己的 CLI 恢复指定对话
```

当前内置计划：

```bash
pi --session "<会话文件绝对路径或ID>"
omp "--resume=<会话文件绝对路径或ID>"
```

不是向 TUI 输入 `/resume`，也不是调用一个统一的 `herdr resume`。

源码：`src/agent_resume.rs:234–240` 明确区分 Pi 的 `--session` 和 OMP 的 `--resume=`。

`src/persist/restore.rs:813–858,881–920` 负责选择恢复计划。若持久化状态包含有效的显式 `resume_argv`，优先使用它，否则依据保存的智能体会话引用生成内置计划；恢复过程中会去重，避免同一轮恢复把同一个原生智能体会话重复启动到多个窗格。原生智能体恢复时，对话历史由智能体负责，不把保存的 pane 展示历史当作原生对话重新注入。

### 4.3 生效条件

默认启用：

```toml
[session]
resume_agents_on_restore = true
```

需要集成成功上报且状态已保存、智能体会话文件仍可访问、对应 `pi` / `omp` 命令可用。只启动过智能体但没有获得有效会话引用，不应假定可自动恢复。

配置默认值见 `src/config/model.rs:269–279`。

### 4.4 恢复对话不等于继续所有中断的工作

本机 OMP v18.12.6 的 CLI 帮助区分：

- `--resume=<value>`：按 ID 前缀、路径恢复会话；省略值时可打开选择器。
- `--restore=<value>`：先恢复会话，再继续上一进程未完成的回合和当时运行的子智能体工作。

Herdr 当前内置 OMP 恢复计划使用 **`--resume=`，不是 `--restore=`**。因此不能宣称 Herdr 自动恢复会继续所有中断的任务。普通 shell 进程、构建进程的运行状态也不会随对话文件恢复。

## 5. 主动指定对话路径 / ID，让 Herdr 重新启动智能体

支持。命令形式：

```text
herdr agent start <NAME> --kind <KIND> --pane <ID> [OPTIONS] [-- [AGENT_ARG]...]
```

### 5.1 在已有空闲 shell 窗格中恢复

下面 `w1:p2` 是示例窗格 ID，文件路径也是示例，需要替换。

Pi：

```bash
herdr agent start pi-restored --kind pi --pane w1:p2 -- --session "E:\sessions\pi-session.jsonl"

# 用 ID
herdr agent start pi-restored --kind pi --pane w1:p2 -- --session "<会话ID>"
```

OMP：

```bash
herdr agent start omp-restored --kind omp --pane w1:p2 -- "--resume=E:\sessions\omp-session.jsonl"

# 用 ID / ID 前缀
herdr agent start omp-restored --kind omp --pane w1:p2 -- "--resume=<会话ID>"
```

若明确要 OMP 尝试继续中断的工作，可以传它自己的 `--restore`：

```bash
herdr agent start omp-restored --kind omp --pane w1:p2 -- "--restore=E:\sessions\omp-session.jsonl"
```

注意：`--restore` 可能继续执行操作，不只是打开历史。

### 5.2 原 tab 已关闭时，先创建新 tab

```bash
herdr tab create --workspace w1 --cwd "E:\your-project" --label restored --focus
```

返回 JSON 中 `result.root_pane.pane_id` 是新窗格 ID，填入后续 `agent start` 的 `--pane`。`--cwd` 应选择相应项目目录；使用会话 ID 时，也要注意智能体自身的会话查找规则和目录配置。绝对路径能明确指定会话文件。

### 5.3 限制和参数含义

- `pi-restored` / `omp-restored` 是 Herdr 中的智能体名称，不是对话 ID。
- 名称在存活智能体中必须唯一，格式为 `[a-z][a-z0-9_-]{0,31}`。
- `--pane` 必须指向现有窗格，并且交互式 shell 在前台、处于可用提示符；不能有前台命令、编辑器或智能体占用。
- `agent start` 不负责创建 tab / pane，拓扑需要先单独创建。
- `--` 后的参数传给智能体本身。
- 正常成功返回表示在同一个终端中检测到预期智能体，且已经可接受交互输入。
- 默认启动等待 30000ms，最大 300000ms；启动被阻塞或超时，不要简单等同于进程从未启动，可用 `agent read` 等查看状态。
- 这是创建新进程并加载对话，不是复活旧 PTY、旧 PID 或任意外部进程。

依据：`docs/next/website/src/content/docs/zh-cn/cli-reference.mdx:329–338`、`src/cli/agent/bindings.rs:107–109,266–268`。

## 6. 本次调查的验证范围

实际执行并查看过以下帮助输出：

```bash
herdr --help
herdr session --help
herdr tab --help
herdr tab close --help
herdr tab create --help
herdr agent --help
herdr agent start --help
pi --help
omp --help
```

这些输出确认了命令存在、参数形状、智能体参数透传，以及本机 OMP v18.12.6 的 resume / restore 区别。行为原理依据当前 Herdr 仓库源码和文档调查。

**未执行实际关闭用户 tab、停止服务器、机器重启、启动 / 恢复用户对话的端到端实验；未运行构建或测试。** 不把 CLI 帮助和源码调查等同于已完成恢复实测。当前源码实现也不自动代表任意旧版 Herdr 或 Pi / OMP 的行为。
