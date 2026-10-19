# OMP `message_update` 实时事件流：原理、API 与只读镜像方案

本文说明如何用 OMP（oh-my-pi coding-agent）的扩展事件 `message_update`，把 assistant 输出以 **token 级（逐 delta）** 实时导出到文件或推给自建 UI，同时**不改变 OMP 的对话、持久化与 TUI 行为**。

> 适用对象：想给 OMP 做一个只读镜像 UI / 实时导出器 / 自定义聊天前端的人。
>
> 不覆盖：镜像 UI 的具体渲染技术（Markdown、工具卡片、主题等由你自己决定）。

---

## 目录

1. [结论速览](#1-结论速览)
2. [事件链路与原理](#2-事件链路与原理)
3. [相关 API 速查](#3-相关-api-速查)
4. [生命周期](#4-生命周期)
5. [只读不变量与隔离模型](#5-只读不变量与隔离模型)
6. [完整实现：只读镜像到文件](#6-完整实现只读镜像到文件)
7. [加载与安装](#7-加载与安装)
8. [调试与排错](#8-调试与排错)
9. [方案对比](#9-方案对比)
10. [参考源码定位](#10-参考源码定位)

---

## 1. 结论速览

| 问题 | 结论 |
| --- | --- |
| `message_update` 是流式逐 token 触发吗？ | **是**。它是逐 delta（provider chunk）触发，不是"单条消息完成时"。 |
| 那"完成时"触发的是什么？ | `message_end`（每条消息 finalize 时）。 |
| 用它能做只读镜像吗？ | **能**。`message_update` / `message_start` / `message_end` 都是通知型，不改变状态。 |
| 会影响 OMP 原本功能吗？ | 事件投递路径是隔离的（不阻塞 agent loop、错误被兜住）；但扩展**同进程无隔离**，你自己的代码若阻塞事件循环或抛出致命异常会拖垮整个 session。 |
| 相比 watch session 文件？ | 优势是 **token 级粒度** + thinking/工具参数流 + 无需解析文件；劣势是**同进程**且要写/装扩展。 |
| 想要"进程外 + token 级"？ | 用 `omp --mode json` 输出事件流，外部程序 tail。 |

三类"实时"粒度：

| 粒度 | 机制 | 能否看到流式半截文本 |
| --- | --- | --- |
| token / delta | `message_update`（扩展）、`--mode json`、RPC 事件流 | 能 |
| 消息 | session `.jsonl` 文件、`message_end` | 不能（只在消息完成后） |
| 轮次 / 生命周期 | `turn_start` / `turn_end` / `agent_start` / `agent_end` | 不适用 |

---

## 2. 事件链路与原理

### 2.1 总览

```
provider SSE / NDJSON stream
        │  每来一个 chunk 产生一个 AssistantMessageEvent
        ▼
packages/agent/src/agent-loop.ts
        │  在 provider 流事件循环里，对每个 text_delta / thinking_delta /
        │  toolcall_delta / *_start / *_end 构造一个增量快照并 push
        ▼
AgentSession (packages/coding-agent/src/session/agent-session.ts)
        │
        ├─ #emit(event)                  → 内建消费者（TUI 等），同步
        └─ #queueExtensionEvent(event)   → 扩展消费者，微任务队列，异步、不阻塞
                    │
                    ▼
        ExtensionRunner.emit("message_update")
                    │  每个 handler 30s 超时、错误被捕获
                    ▼
        pi.on("message_update", handler)
```

### 2.2 产生：真正的逐 delta 推送

`packages/agent/src/agent-loop.ts` 在遍历 provider 流事件时，`message_update` 的 case 覆盖：

`text_start` / `text_delta` / `text_end` / `image_end` / `thinking_start` / `thinking_delta` / `thinking_end` / `toolcall_start` / `toolcall_delta` / `toolcall_end`

简化后的形态：

```ts
switch (event.type) {
  case "start":
    // ... 首次出现时 push message_start
    break;

  case "text_start":
  case "text_delta":
  case "text_end":
  case "image_end":
  case "thinking_start":
  case "thinking_delta":
  case "thinking_end":
  case "toolcall_start":
  case "toolcall_delta":
  case "toolcall_end":
    if (partialMessage) {
      partialMessage = event.partial;
      // 增量快照：只深拷贝发生变化的块，finalized 块跨快照共享
      const messageSnapshot = turnSnapshot
        ? snapshotAssistantMessageIncremental(partialMessage, turnSnapshot, contentIndex ?? -1, openBlocks)
        : snapshotAssistantMessage(partialMessage);
      turnSnapshot = messageSnapshot;
      stream.push({
        type: "message_update",
        assistantMessageEvent: snapshotAssistantMessageEvent(event, messageSnapshot),
        message: messageSnapshot,
      });
    }
    break;
}
```

因此：**频率 = provider 的流式 chunk 频率**，不是每条消息一次。

### 2.3 分发：内建同步 + 扩展异步队列

`AgentSession` 对 `message_update` 的处理有两条独立路径：

```ts
if (event.type === "message_update") {
  this.#emit(event); // 内建消费者（TUI 等），同步
  // Per-delta hot path: only queue the serialized extension emit when something listens
  if (this.#extensionRunner?.hasHandlers("message_update")) this.#queueExtensionEvent(event);
  return;
}
```

`#queueExtensionEvent` 不 await，只入队并安排一个微任务：

```ts
#queuedExtensionEvents: AgentSessionEvent[] = [];
#drainingExtensionEvents = false;

#queueExtensionEvent(event: AgentSessionEvent): void {
  this.#queuedExtensionEvents.push(event);
  if (this.#drainingExtensionEvents) return;
  this.#drainingExtensionEvents = true;
  queueMicrotask(() => void this.#drainExtensionEvents());
}

async #drainExtensionEvents(): Promise<void> {
  try {
    while (this.#queuedExtensionEvents.length > 0) {
      const batch = this.#queuedExtensionEvents;
      this.#queuedExtensionEvents = [];
      for (const event of batch) {
        try {
          await this.#emitExtensionEvent(event);
        } catch {
          // A failed notification must not hold later updates in the queue.
        }
      }
    }
  } finally {
    this.#drainingExtensionEvents = false;
  }
}
```

要点：

- 你的 handler 慢**不会**卡住 agent loop 或 TUI。
- `hasHandlers("message_update")` 门控：**没订阅时零额外成本**。
- 队列按顺序串行 drain：你的 handler 如果持续比 delta 慢，`#queuedExtensionEvents` 会积压（内存 / 你自己的视图延迟），但不会拖慢核心对话。

### 2.4 快照是"共享只读"的

`agent-loop.ts` 的注释是硬约束：

> `message` and `assistantMessageEvent.partial` intentionally share one snapshot ... finalized blocks are shared across ALL message_update snapshots of the turn.
> Consumers MUST treat both fields — and every content block inside them — as **read-only**: mutating a snapshot would corrupt every earlier and later snapshot of the turn.

含义：**只读消费是安全的；修改 payload 会污染同一轮的所有快照。** 做只读镜像天然满足这个契约。

`message_update` 的扩展映射**不做防御性克隆**（直接引用上述快照）；只有 `message_end` 会 `cloneMessageNotification` 出独立副本。

---

## 3. 相关 API 速查

### 3.1 扩展入口

```ts
import type { ExtensionAPI } from "@oh-my-pi/pi-coding-agent";

export default function (pi: ExtensionAPI) {
  // 在这里注册 handler / tool / command ...
}
```

工厂签名：

```ts
export type ExtensionFactory = (pi: ExtensionAPI) => void | Promise<void>;
```

工厂在**加载阶段**被调用，用于注册；此时会话级 action 方法尚未初始化（调用 `pi.sendMessage()` 等会抛 `ExtensionRuntimeNotInitializedError`）。

### 3.2 订阅事件

```ts
pi.on("message_update", handler);
```

handler 签名：

```ts
export type ExtensionHandler<E, R = undefined> =
  (event: E, ctx: ExtensionContext) => Promise<R | void> | R | void;
```

`message_update` 的返回值为 `void`（通知型，返回值不被用于改变状态）。

### 3.3 `MessageUpdateEvent` 字段

```ts
/** Fired during assistant message streaming with token-by-token updates */
export interface MessageUpdateEvent {
  type: "message_update";
  /** 当前 assistant 消息的增量快照（只读；与 assistantMessageEvent.partial 同源） */
  message: AgentMessage;
  /** 本次流事件（含 delta 或块边界信息） */
  assistantMessageEvent: AssistantMessageEvent;
}
```

### 3.4 `AssistantMessageEvent` 联合类型

来自 `packages/ai/src/types.ts`。实际由 `message_update` 携带的是下面除 `done` / `error` 之外的成员；`done` / `error` 是流终止事件（通过 `message_end` / `agent_end` 体现）。

```ts
export type AssistantMessageEvent =
  | { type: "start";          contentIndex?: undefined; partial: AssistantMessage }
  | { type: "text_start";     contentIndex: number; partial: AssistantMessage }
  | { type: "text_delta";     contentIndex: number; delta: string; partial: AssistantMessage }
  | { type: "text_end";       contentIndex: number; content: string; partial: AssistantMessage }
  | { type: "thinking_start"; contentIndex: number; partial: AssistantMessage }
  | { type: "thinking_delta"; contentIndex: number; delta: string; partial: AssistantMessage }
  | { type: "thinking_end";   contentIndex: number; content: string; partial: AssistantMessage }
  | { type: "image_end";      contentIndex: number; content: ImageContent; partial: AssistantMessage }
  | { type: "toolcall_start"; contentIndex: number; partial: AssistantMessage }
  | { type: "toolcall_delta"; contentIndex: number; delta: string; partial: AssistantMessage }
  | { type: "toolcall_end";   contentIndex: number; toolCall: ToolCall; partial: AssistantMessage }
  | { type: "done";  contentIndex?: undefined; reason: Extract<StopReason, "stop" | "length" | "toolUse">; message: AssistantMessage }
  | { type: "error"; contentIndex?: undefined; reason: Extract<StopReason, "aborted" | "error">; error: AssistantMessage };
```

常用字段：

- `text_delta` / `thinking_delta` / `toolcall_delta`：`contentIndex` + `delta`
- `*_start` / `*_end`：块的开始 / 结束边界
- `partial`：当前完整消息快照（只读，与 `MessageUpdateEvent.message` 同源）

### 3.5 只读伴随事件

做只读镜像时，建议只订阅下面这些**通知型**事件（返回值不被用于改写状态）：

| 事件 | 载荷要点 |
| --- | --- |
| `message_start` | `{ message }` —— 一条消息开始 |
| `message_update` | `{ message, assistantMessageEvent }` —— 逐 delta |
| `message_end` | `{ message }` —— 一条消息结束（payload 为独立克隆，可安全持有） |
| `tool_execution_start` | `{ toolCallId, toolName, args, intent? }` |
| `tool_execution_update` | `{ toolCallId, toolName, args, partialResult }` —— 工具输出流 |
| `tool_execution_end` | `{ toolCallId, toolName, result, isError }` |
| `turn_start` | `{ turnIndex, timestamp }` |
| `turn_end` | `{ turnIndex, message, toolResults }` |
| `agent_start` | `{}` |
| `agent_end` | `{ messages, willContinue? }` |

**不要**为了"只读"去注册这些**改写型**事件：`assistant_message`（可替换最终消息）、`context`（替换消息数组）、`before_provider_request`、`input`、`tool_call` / `tool_result`（可 block / patch）、`session_before_*`。

### 3.6 `ExtensionContext` 常用成员

handler 的第二个参数 `ctx`：

| 成员 | 用途 |
| --- | --- |
| `ctx.agent` | `{ kind: "main" \| "sub", id, name, depth, parentId? }`；**用它过滤子 agent** |
| `ctx.sessionManager` | 只读会话查询（见 3.8） |
| `ctx.hasUI` | 当前是否有 UI |
| `ctx.isIdle()` | agent 是否空闲 |
| `ctx.cwd` | 当前工作目录 |
| `ctx.setInterval(cb, ms)` | **受控**定时器：回调抛错被兜住并上报，不会崩溃进程 |
| `ctx.setTimeout(cb, ms)` | 同上，一次性 |
| `ctx.clearTimer(timer)` | 清除上面的定时器 |
| `ctx.abort()` | 中断当前 agent 操作（只读镜像不要调用） |
| `ctx.model` / `ctx.models` | 当前模型 / 只读模型查询 |
| `ctx.getContextUsage()` | 上下文用量 |

### 3.7 `ExtensionAPI` 常用成员

| 成员 | 用途 |
| --- | --- |
| `pi.on(event, handler)` | 订阅事件 |
| `pi.logger` | 文件 logger（`debug` / `info` / `warn` / `error`） |
| `pi.getFlag(name)` | 读取通过 `registerFlag` 声明的 CLI flag |
| `pi.registerCommand(name, opts)` | 注册 `/name` 命令 |
| `pi.exec(cmd, args, opts?)` | 执行外部命令 |
| `pi.registerTool(...)` | 注册工具 |
| `pi.registerFlag(...)` | 声明 flag |
| `pi.setLabel(label)` | 扩展显示名 |
| `pi.appendEntry(customType, data)` | 追加自定义 session entry |
| `pi.events` | 共享 EventBus |

> 注意：不要用 `console.log`。若代码可能运行在 TUI/RPC/worker 上下文，请用 `pi.logger`，否则会破坏渲染或协议。

### 3.8 `ctx.sessionManager` 只读查询

`ReadonlySessionManager` 暴露（部分）：

| 方法 | 说明 |
| --- | --- |
| `getBranch(fromId?)` | 当前 leaf 到根的 entry 数组（防御性拷贝） |
| `getEntries()` | 全部 entry（浅拷贝，不含 header） |
| `getSessionFile()` | 会话 JSONL 路径（懒创建，可能尚未落盘） |
| `getSessionId()` / `getSessionName()` | id / 名称 |
| `getHeader()` | 会话 header |
| `getLeafId()` / `getLeafEntry()` | 当前 leaf |
| `getTree()` | 会话树 |

用它可以在收到事件时补齐"完整状态"（例如 UI 首次连接时先拉全量，再按 delta 增量更新）。

---

## 4. 生命周期

### 4.1 加载阶段（load）

1. 发现模块路径（见第 7 节）。
2. 并发 `import`（Bun），然后**按路径顺序串行**调用各模块的 factory。
3. factory 里调用 `pi.on(...)` / `pi.registerX(...)` 完成注册。
4. 加载期 action 方法不可用；`process.exit` 等被 `withHostGuard()` 拦截为 `ExtensionExitError`。

### 4.2 运行阶段（run）

1. `ExtensionRunner.initialize()` 完成后，事件开始分发。
2. 每个 handler 由 `#runHandlerWithTimeout` 包裹：
   - 默认预算 `EXTENSION_HANDLER_TIMEOUT_MS = 30_000`；
   - 超时 → 记录日志 + `emitError`，不抛进调用方；
   - handler 抛错 → 捕获 + `emitError`，不抛进调用方；
   - `session_shutdown` 另有 2s 预算且并发执行。
3. `message_update` 的扩展投递走微任务队列（见 2.3）。

### 4.3 子 agent 复用

> Extension factories are **rebound to every subagent session** (task tool, eval `agent()`, `/tan` clones).

所以 `message_update` 也会为子 agent 触发。只镜像主会话时：

```ts
pi.on("message_update", (event, ctx) => {
  if (ctx.agent.kind !== "main") return;
  // ...
});
```

### 4.4 清理

- 在 `session_shutdown` 里清除定时器、flush 缓冲、关闭 server。
- `dispose` 需幂等。
- 用 `ctx.setInterval` / `ctx.setTimeout` 的 timer 会在 `session_shutdown` 自动清除，但仍建议显式清理。

### 4.5 热重载

主会话中修改 settings 的 `extensions` / `disabledExtensions` 会 **suspend / resume** 已加载的扩展（其 handler/command/tool 停止或恢复参与），但**模块不会卸载或重新求值**；新启用的、启动时未 import 的模块需要重启。

---

## 5. 只读不变量与隔离模型

### 5.1 读只读，改会串

- `message_update` 的 `message` / `assistantMessageEvent.partial` 是**同一轮的共享快照**。
- **只读**：累加 `delta`、读取 `contentIndex`、读取 `partial` —— 安全。
- **写入/修改 payload 字段**：会污染同一轮的所有快照，可能破坏 OMP 渲染或其他消费者 —— 禁止。
- `message_end` 的 `message` 是独立克隆，可以安全保存/处理。

### 5.2 两层隔离

| 层 | 隔离程度 | 说明 |
| --- | --- | --- |
| 事件投递路径 | **隔离** | 微任务队列、不 await 主路径、30s 超时、错误捕获，不破坏 agent loop / TUI |
| 扩展进程 | **不隔离** | 同进程、同线程、无沙箱；阻塞/致命异常会拖垮整个 session |

### 5.3 红线清单

| 不要做 | 后果 | 正确做法 |
| --- | --- | --- |
| handler 里同步阻塞事件循环（死循环、超大 `readFileSync`、每 delta 巨型序列化） | 整个 OMP 冻结（超时定时器也无法触发） | handler 只做内存累加；I/O 放到节流后的 flush |
| 裸 `setInterval` / `setTimeout` / 游离 promise 里抛错 | `uncaughtException` → 整个 session 被 teardown | 用 `ctx.setInterval` / `ctx.setTimeout`，或自己 try/catch |
| 修改 `message_update` 的 payload | 污染同一轮所有快照 | 只读 |
| handler 持续慢于 delta 速率 | 扩展事件队列积压（内存/延迟） | 合并/丢弃中间态，只保留最新快照 |
| 在 `message_update` 里做重 I/O | 你的视图延迟、队列积压 | 缓冲 + 定时 flush |

### 5.4 性能量级

- 频率 = provider chunk 频率（流式越快越高）。
- 有效负载：`message_update` 按引用共享快照，本身不做深拷贝；每次映射会创建一个薄包装对象。
- 只要 handler 轻量，对核心对话的影响可忽略。

---

## 6. 完整实现：只读镜像到文件

下面是一个可直接使用的扩展示例：把主会话的逐 delta 事件按 JSONL 追加到 `.omp/live/<sessionId>.events.jsonl`，并在消息结束时追加最终消息。

```ts
import * as fs from "node:fs/promises";
import * as path from "node:path";
import type { ExtensionAPI, ExtensionContext } from "@oh-my-pi/pi-coding-agent";

/** flush 间隔：在实时性和 I/O 之间取平衡。 */
const FLUSH_MS = 75;

export default function (pi: ExtensionAPI) {
  const outDir = path.join(process.cwd(), ".omp", "live");

  let outFile: string | undefined;
  let pending: string[] = [];
  let timer: ReturnType<ExtensionContext["setInterval"]> | undefined;

  const isMain = (ctx: ExtensionContext): boolean => ctx.agent.kind === "main";

  async function flushPending(): Promise<void> {
    if (pending.length === 0 || !outFile) return;
    const chunk = pending.join("");
    pending = [];
    await fs.appendFile(outFile, chunk, "utf8");
  }

  pi.on("message_start", async (event, ctx) => {
    if (!isMain(ctx) || event.message.role !== "assistant") return;
    outFile = path.join(outDir, `${ctx.sessionManager.getSessionId()}.events.jsonl`);
    pending = [];
    await fs.mkdir(outDir, { recursive: true });
    timer ??= ctx.setInterval(() => {
      // 受控定时器：回调抛错被上报，不会崩溃进程。
      void flushPending().catch(err => pi.logger.warn("live mirror flush failed", { error: String(err) }));
    }, FLUSH_MS);
  });

  pi.on("message_update", (event, ctx) => {
    // 只读累加，不做 I/O；真正的写入交给定时 flush。
    if (!isMain(ctx) || !outFile) return;
    const d = event.assistantMessageEvent;
    if (d.type === "text_delta" || d.type === "thinking_delta" || d.type === "toolcall_delta") {
      pending.push(
        `${JSON.stringify({
          t: Date.now(),
          kind: d.type.replace("_delta", ""), // "text" | "thinking" | "toolcall"
          index: d.contentIndex,
          delta: d.delta,
        })}\n`,
      );
    }
  });

  pi.on("message_end", async (event, ctx) => {
    if (!isMain(ctx) || !outFile || event.message.role !== "assistant") return;
    // message_end 的 message 是独立克隆，可安全序列化保存。
    pending.push(`${JSON.stringify({ t: Date.now(), kind: "final", message: event.message })}\n`);
    await flushPending();
  });

  pi.on("session_shutdown", async (_event, ctx) => {
    if (timer) ctx.clearTimer(timer);
    timer = undefined;
    await flushPending().catch(() => {});
    outFile = undefined;
  });
}
```

输出示例：

```jsonc
{"t":1750000000001,"kind":"text","index":0,"delta":"Hel"}
{"t":1750000000002,"kind":"text","index":0,"delta":"lo"}
{"t":1750000000010,"kind":"thinking","index":1,"delta":"Let me think"}
{"t":1750000000020,"kind":"toolcall","index":2,"delta":"{\"path\":"}
{"t":1750000000999,"kind":"final","message":{"role":"assistant","content":[...]}}
```

### 6.1 变体：维护"当前消息快照"文件

网页只想显示"当前正在流出的完整文本"时，可以维护一个快照文件（原子写）：

```ts
async function writeSnapshot(file: string, data: unknown): Promise<void> {
  const tmp = `${file}.tmp`;
  await fs.writeFile(tmp, JSON.stringify(data));
  await fs.rename(tmp, file); // 同一目录内 rename 是原子的
}
```

在 `message_update` 里只更新内存累加器并置 `dirty`，在定时 flush 里用 `partial` 或自建结构写快照。`assistantMessageEvent.partial` 就是当前完整消息，可直接序列化（**只读**）。

### 6.2 变体：直接推给网页（SSE）

扩展在进程内，可以用 `Bun.serve()` 起一个本地端点，把 flush 的内容通过 SSE 推给浏览器：

```ts
const server = Bun.serve({
  port: 0,
  fetch(req) {
    if (new URL(req.url).pathname !== "/live") return new Response("not found", { status: 404 });
    return new Response(
      new ReadableStream({
        start(controller) {
          const send = (obj: unknown) => controller.enqueue(`data: ${JSON.stringify(obj)}\n\n`);
          // 订阅上面的 pending/flush，把事件 send 出去
          subscribers.add(send);
        },
        cancel() {
          /* 移除 subscriber */
        },
      }),
      { headers: { "content-type": "text/event-stream", "cache-control": "no-cache" } },
    );
  },
});
// 在 session_shutdown 里 server.stop(true)
```

> 注意：同进程起 server 会占用该进程资源；端口、鉴权、生命周期都要自己管理。

---

## 7. 加载与安装

### 7.1 自动发现位置

- 项目：`<cwd>/.omp/extensions/`（只认 cwd，不向上遍历）
- 用户：`<agentDir>/extensions/`，默认 `~/.omp/agent/extensions/`
  - `omp --profile <name>` → `~/.omp/profiles/<name>/agent/extensions/`
- 自动扫描支持 `.ts` / `.js`（显式配置的文件和插件 manifest 还支持 `.mjs` / `.cjs`）

### 7.2 显式配置

```bash
omp --extension ./my-ext.ts        # 或 -e
```

```yaml
# ~/.omp/agent/config.yml
extensions:
  - ~/my-exts/live-mirror.ts
  - ./local/ext-pack
```

```json
{ "extensions": ["./.omp/extensions/live-mirror"] }
```

目录解析顺序：`package.json#omp.extensions` → `index.ts` → `index.js` → 一级扫描。

### 7.3 启用 / 禁用

- `--no-extensions`：关闭环境发现；显式 `-e` / `--hook` 仍加载。
- `disabledExtensions`：按 id 过滤，扩展模块 id 形如 `extension-module:<derivedName>`（由路径推导，`foo.ts` → `foo`，`bar/index.ts` → `bar`）。

```yaml
disabledExtensions:
  - extension-module:live-mirror
```

### 7.4 开发提示

- 修改扩展现有代码：settings 变更可 suspend/resume 已加载扩展，但模块不重新求值。
- Windows 下 loader 的 `?mtime` cache-buster 不生效（Bun 忽略 `file://` query），**热重载不如 POSIX**；改动后建议重启。
- 最小验证：放一个只 `pi.logger.info` 的扩展，确认被加载。

---

## 8. 调试与排错

| 现象 | 可能原因 | 排查 |
| --- | --- | --- |
| 完全没有事件 | 扩展没加载 / 被 `disabledExtensions` 过滤 / 路径写错 | 在 factory 里 `pi.logger.info` 打点；用 `--extension` 显式加载 |
| 只有最终消息，没有 delta | 你订阅的是 `message_end`，或把完成事件当成流事件了 | 确认订阅 `message_update`，类型注释写的是 token-by-token |
| 事件来了但内容重复/错乱 | 修改了只读快照，或对 `partial` 做了原地修改 | 只读；用自建累加器而非改 payload |
| 子 agent 内容混进来 | 扩展被 rebind 到子会话 | `if (ctx.agent.kind !== "main") return;` |
| OMP 卡顿 / 冻结 | handler 同步阻塞事件循环，或每 delta 做重 I/O | 只做内存累加，定时 flush |
| 整个 session 崩溃 | 裸 timer / 游离 promise 抛错 | 用 `ctx.setInterval` / `ctx.setTimeout`，或 try/catch |
| 网页读到的行不完整 | 读到正在 append 的半行 | 按行解析，跳过最后一行不完整内容 |
| 想连历史一起显示 | 只有 live 事件，没有旧历史 | 首连时用 `ctx.sessionManager.getBranch()` / `getEntries()` 拉全量，再按 delta 增量 |

---

## 9. 方案对比

| 方案 | 实时粒度 | 进程关系 | 需要轮询 | 能看 thinking/工具流 | 适合场景 |
| --- | --- | --- | --- | --- | --- |
| 扩展 `message_update` | token / delta | 同进程 | 否（推送） | 能 | 交互式 TUI + 自定义镜像 UI |
| watch session `.jsonl` | 消息 | 进程外 | 否（`fs.watch` 也是推送） | 不能 | 零侵入、只关心完成的消息 |
| `omp --mode json` 输出流 | token / delta | 进程外 | 否（tail 输出） | 能 | 脚本化/程序化跑 prompt + 自建 UI |
| `--mode rpc` / `rpc-ui` | token / delta | 进程外 | 否（推送） | 能 | 想同时"看"和"发/控制"的完整前端 |

选择建议：

- 要交互式 TUI 里的 token 级镜像 → 扩展 `message_update`。
- 只要完成消息、完全不想碰进程 → watch session 文件。
- 要进程外 + token 级 → `--mode json`。

---

## 10. 参考源码定位

事件与类型：

- `packages/coding-agent/src/extensibility/extensions/types.ts`
  - `MessageUpdateEvent`（约 884 行）
  - `MessageEndEvent`（约 895 行）
  - `ExtensionHandler`（1343 行）
  - `ExtensionAPI`（1357 起）、`on("message_update", ...)`（1423 行）
  - `ExtensionFactory`（1785 行）
- `packages/ai/src/types.ts` —— `AssistantMessageEvent`（约 1590 行）

事件产生与只读不变量：

- `packages/agent/src/agent-loop.ts`
  - per-delta `message_update` push（约 2395-2500 行）
  - 只读共享快照说明（约 2458-2470 行）

分发与隔离：

- `packages/coding-agent/src/session/agent-session.ts`
  - `#queueExtensionEvent` / `#drainExtensionEvents`（3173-3199 行）
  - `message_update` 快路径（3210-3215 行）
- `packages/coding-agent/src/extensibility/extensions/runner.ts`
  - `EXTENSION_HANDLER_TIMEOUT_MS`（130 行）
  - `#runHandlerWithTimeout`（1488-1577 行）
- `packages/coding-agent/src/extensibility/extensions/lifecycle-mirror.ts`
  - `extensionEventFromSessionEvent`（115 行起；`message_update` 按引用，`message_end` 克隆）
- `packages/coding-agent/src/extensibility/extensions/managed-timers.ts` —— `ctx.setInterval` / `ctx.setTimeout` 的隔离实现

文档：

- `docs/extensions.md` —— 扩展总览、事件表面、UI、后台工作
- `docs/extension-loading.md` —— 发现、加载顺序、失败隔离、目录布局
- `docs/session.md` —— session 文件格式与持久化语义（"Streaming partial text is not persisted"）
- `docs/provider-streaming-internals.md` —— 流式管线与 `message_update` 的生命周期
- `docs/rpc.md` —— RPC 事件流（需要双向控制时）
- `packages/coding-agent/examples/extensions/README.md` —— 扩展示例

---

## 附：一句话总结

`message_update` 是 OMP 暴露给扩展的**逐 delta 流式事件**；它走与内建 TUI 分离的、非阻塞且带超时/错误隔离的投递路径，payload 是**只读共享快照**。只要你的扩展只订阅通知型事件、只读、不阻塞事件循环、用受控定时器做节流 I/O，就能在不影响 OMP 正常功能的前提下，实时复刻一份任意形态的只读 UI。
