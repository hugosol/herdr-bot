# Pi 生态 Workflow 方案设计对比

> 整理日期：2026-10-25。
> 目的：为自建支持依赖、循环和可视化的 workflow 系统提供设计参考，而不是推荐直接替换现有系统。
> 范围：当前本地 pi-subagents，以及网上调查的五个相关 Pi 扩展。

## 1. 阅读说明与证据边界

本文区分三类信息：

- **源码观察**：实际阅读了类型或实现片段，不代表已经运行验证。
- **文档描述**：作者 README、架构或使用文档说明的能力，没有独立验证全部实现。
- **设计建议**：根据资料作出的取舍建议，不是对应项目已经具备的功能。

没有安装或执行这些外部项目，也没有对性能、崩溃恢复、权限隔离或跨平台兼容性进行测试。不要把“源码存在”“作者宣称”理解成生产可靠性认证。

特别注意项目身份：

- 本文 **pi-subagents** 指 `E:/Developer/myhub/pi-extension/pi-subagents` 的本地实现，公共文档指向 [nicobailon/pi-subagents](https://github.com/nicobailon/pi-subagents)。本地内容不保证与公共仓库当前分支完全相同。
- [tintinweb/pi-subagents](https://github.com/tintinweb/pi-subagents) 是另一个仓库；pi-tasks 依赖的是这个版本。不能将其工具名称或兼容性直接套到本文本地 pi-subagents 上。
- `AgwaB/pi-workflow` 和 `osolmaz/pi-workflows` 是不同项目，名称仅差复数，不应混淆。
- 在线源码链接多指向 main 或 HEAD，后续会变化。调查时获得的目录树 SHA：osolmaz 为 `2cb8ab3a9c2e52d0c193d572728a9ccd7afdb77a`，AgwaB 为 `fc067baef33dbbd30b4f4a0902fcb29655db7141`；并非所有请求都锁定为同一提交快照。

## 2. 核心结论

这些项目不是同一种“workflow”：

1. **脚本编排型**：pi-subagents。流程由 JS 控制，宿主执行和监督子代理。
2. **声明式依赖图 + 实例展开型**：AgwaB/pi-workflow。保存 stage 模板，编译成任务依赖，循环和 foreach 展开成具体实例。
3. **控制流图 / 状态机型**：osolmaz/pi-workflows。当前节点执行完后按边跳转，允许回边；不是多前置依赖并行调度器。
4. **声明式步骤 + 依赖与路由型**：BlackBeltTechnology/pi-flows。YAML 定义前置依赖，分支节点提供选择和有界回跳。
5. **任务账本 + 依赖协调型**：tintinweb/pi-tasks。重点是任务跟踪、共享与自动级联，不是完整的通用循环工作流语言。
6. **会话循环型**：nicobailon/pi-review-loop。通过会话事件反复审查，不维护通用执行图。

针对“依赖 + 循环 + 方便可视化”的自建系统，优先参考 AgwaB 的定义/编译/运行分层与循环实例模型，借鉴 osolmaz 的 attempt 历史与视图投影，再参考 pi-flows 的事件驱动仪表盘。pi-subagents 更适合参考动态编排、子任务身份、监督和权限契约。

## 3. 一页对比

| 方案 | 定义数据结构 | 依赖/控制语义 | 循环表达 | 运行与存储 | 场景偏向 |
|---|---|---|---|---|---|
| 本地 pi-subagents | JS 脚本 + JSON args；命名资源解析为脚本 | await 表达顺序；runs.all 并行；程序显式传结果 | JS 循环/分支；runs.lanes 表达并行的顺序链 | keyed child、trace、children、graph snapshot、receipt、子任务 journal | 模型生成编排、动态 fan-out、监督多代理 |
| AgwaB/pi-workflow | JSON stage graph，支持嵌套 stages | from：数据+顺序；after：仅顺序；编译成 dependsOn | loop 模板按轮展开任务实例 | spec.json / compiled.json / run.json；产物独立 | 可重复产物管线、foreach/reduce、结构化迭代 |
| osolmaz/pi-workflows | TS 对象与回调；nodes 字典 + edges + startAt | 控制转移；单 currentNode 游标 | 普通回边、switch；controlLoop 辅助生成边 | run state、latest outputs/results、attempt/step 历史；文档描述 SQLite | plan→implement→verify→repair、人机交互和过程回放 |
| BlackBeltTechnology/pi-flows | YAML steps，显式步骤类型 | blockedBy 前置依赖；分支选择活动路径 | decision 指向较早步骤，max_iterations 限制 | 内存执行；生命周期事件落盘用于展示重建 | 多代理固定流程、确定性代码与模型混排、实时仪表盘 |
| tintinweb/pi-tasks | 结构化 task 记录 + blocks/blockedBy | 任务完成后解除阻塞；可选自动级联 | 本次未核实通用循环模型 | 内存或不同作用域的 JSON 文件；共享文件锁 | 任务列表、团队共享、轻量依赖协调 |
| nicobailon/pi-review-loop | reviewerLoop 配置 + 会话循环状态 | 每次回复后决定是否继续审查 | 迭代计数 + 文本退出/修复模式 | 本次未核实可持久恢复的执行图 | 单会话反复审查、上下文清理 |

### 3.1 能力与可视化边界

| 方案 | 并行/汇合 | 条件与循环 | 人工/运行中干预 | 可视化 | 恢复边界 |
|---|---|---|---|---|---|
| pi-subagents | runs.all；runs.lanes | 脚本控制；有脚本可移植性限制 | steer、子代理向 supervisor 提问、停止等 | FleetView、inspector、进度/图快照 | 特定 runtime-replaced 重启路径可复用成功 child、重新连接运行中 child；不是保存任意 JS 栈 |
| AgwaB | DAG readiness；foreach、reduce | 显式 loop，当前 child 严格串行且有结构限制 | 有 blocked/interrupted 等状态；不等于任意人工审批系统 | 只读 TUI：run→stage→task→detail | 作者明确 loop 暂不支持 resume；其他路径不能概括成“所有流程可恢复” |
| osolmaz | 已读执行器非通用 fan-out/fan-in | switch 和回边；maxSteps 是步数限制 | checkpoint、暂停/取消；持久运行合同见作者文档 | widget + 独立终端图 viewer、inspector、timeline/replay | 文档描述持久 server/SQLite 与恢复合同；未运行验证 |
| pi-flows | 依赖就绪后自动并行 | 用户/agent/code decision；有界回跳 | 用户 fork、自动路由、abort | TUI 卡片；配套 Web dashboard 集成 | 文档明确不可恢复执行；事件可重建展示 |
| pi-tasks | 可自动级联启动无阻塞任务 | 非完整分支/循环引擎 | 任务更新、owner、停止 | 任务列表 widget，不是 DAG 编辑器 | 任务记录可持久；不等于完整流程 continuation |
| pi-review-loop | 无通用并行图 | 审查轮次与退出检测 | 启停、最大轮数、用户输入退出 | 页脚轮次状态 | 未核实跨进程执行恢复 |

以上没有任何一项可仅凭本次调查被认定为“现成的拖拽式 DAG 编辑器”。运行图查看、任务面板和编辑画布是三种不同能力。

## 4. pi-subagents：脚本负责计划，宿主负责执行

### 4.1 数据结构

**源码观察 + 本地文档**：流程主体是 JS statement body，输入为受限 plain JSON args。来源可以是同一模型回复中的代码块、显式文件路径、或宿主解析的命名资源。

```js
const scan = await runs.run("scan", {
  agent: "scout",
  task: args.task
});
const results = await runs.all([
  { key: "security", agent: "reviewer", task: "Review security:\n" + scan.output },
  { key: "tests", agent: "reviewer", task: "Review test risks:\n" + scan.output }
]);
return results.map(result => ({ ok: result.ok, output: result.output }));
```

此例说明控制流，未实际运行。runs.all 返回有序数组，不是按 key 索引的对象。

核心运行结果可以分为：

```ts
// 本地类型的节选/简化，不是完整公开契约
interface WorkflowScriptResult {
  value: unknown;
  emits: unknown[];
  console: Array<{ level: "log" | "info" | "warn" | "error"; text: string }>;
  trace: WorkflowScriptTraceEntry[];
  children: WorkflowScriptChildResult[];
}

// child 还含停止、上下文、恢复、外部 adapter 等字段
interface ChildResultSketch {
  key: string;
  runId?: string;
  ok: boolean;
  state?: "running";
  output: string;
  structuredOutput?: unknown;
  artifactPaths: string[];
  reused?: boolean;
}
```

另有 WorkflowGraphSnapshot 用于图形/阶段状态投影，WorkflowReceipt 用于记录完成状态、child 引用、可恢复性和产物引用。子任务 journal 记录 start/settle、key、参数指纹及运行 ID 或结果。

### 4.2 依赖、循环与执行

- 顺序：await 前一步，再显式把 output/structuredOutput 传给下一步。
- 并行：runs.all 批量启动；runs.lanes 表达多条独立顺序链。
- 循环与分支：JS 自身表达；不同逻辑轮次应使用不同稳定 key。
- 相同 key + 不兼容参数会报错，不能用同一个 key 偷偷表达新一轮任务。
- runtime 运行在独立 Worker，内部创建 VM 上下文，runs 调用经消息桥交给宿主启动真实子代理。
- 普通脚本无直接文件、shell 或任意 Pi 工具权限。具明确授权的命名资源才可能获得受限 runs.host 能力。
- 不支持所有合法 JS 写法：例如嵌套 async helper 被拒绝；未观察的子调用也会在完成时判错。

### 4.3 能力和场景

适合模型临时生成并行调查、根据中间结果分支、稳定 keyed child、运行中 steering、监督对话、工作区隔离和聚合输出。预算、超时、context、产物与 worktree 等由宿主统一处理，而不是每份脚本自建。

对视觉编辑器不占优势：静态解析任意脚本不能可靠得到所有未来节点与分支。图快照和 trace 更适合表达已经物化的计划和执行事实，不能等同于完整可编辑流程定义。

### 4.4 注意事项

- 外层 async:true 是 workflow 后台运行；child 显式 async:true 则只返回派发回执。ok:false + state:running 不表示最终失败。
- runtime-replaced 后重新启动相同脚本/args 的特定路径，可复用已成功 child 或重新连接仍运行的 child；存在显式 reuseFrom。不能泛化成任意崩溃场景都可靠恢复。
- journal 源码采用尽力追加，写入失败会记录错误；记录丢失可能导致后来重复执行。不能由此承诺副作用 exactly-once。
- VM/Worker 的使用不应直接被理解成适合执行恶意多租户代码的充分隔离。

### 4.5 阅读入口

本地：

- `E:/Developer/myhub/pi-extension/pi-subagents/docs/workflows.md`：使用、来源、预算、lanes、worktree、supervisor。
- `src/workflows/scripted-workflow.ts:1116-1217`：child、trace、script result；`:1253-1292`：宿主注入 Interface。
- `src/workflows/scripted-workflow.ts:1027-1040,2111-2156`：VM 与 Worker；`:710-714`：key 指纹校验。
- `src/shared/types.ts:80-111,218-248`：graph snapshot 和 receipt。
- `src/workflows/workflow-reuse.ts:30-87`：digest、start/settle journal、复用索引。
- `src/runs/foreground/subagent-executor.ts:6280-6305`：实际 child 启动。

在线入口：[工作流文档](https://github.com/nicobailon/pi-subagents/blob/main/docs/workflows.md)。在线版本仅供导航，本节本地结构以实际读取内容为依据。

## 5. AgwaB/pi-workflow：声明模板、编译任务、按轮展开

仓库：[AgwaB/pi-workflow](https://github.com/AgwaB/pi-workflow)。

### 5.1 数据结构

**源码观察**：核心是 ArtifactGraphStageSpec，stage 类型包含 single、reduce、foreach、loop、dag、dynamic。关键字段包括：

```ts
// 类型节选，省略大部分执行配置
interface StageSketch {
  id: string;
  type?: "single" | "reduce" | "foreach" | "loop" | "dag" | "dynamic";
  from?: string | string[] | {
    source: string;
    path: string;
    streaming?: { enabled: true; minChunk?: number };
  };
  after?: string | string[];
  stages?: StageSketch[];
  outputFrom?: string;
  maxConcurrency?: number;
  maxRounds?: number | null;
  until?: unknown;
  progressPath?: string;
  onExhausted?: StageSketch;
}
```

- from：读取上游产物，同时形成执行依赖。
- after：只规定先后，不暴露上游产物。
- 没有显式关系时存在前一 stage 的隐式链；after:[] 可明确声明并行根。
- 引用只允许指向数组中更早的兄弟 stage。因此数组重排可能改变流程含义。
- 嵌套 dag 将子节点命名空间化，outputFrom 选择容器公开输出。

### 5.2 循环结构

**源码观察 + 文档描述**：loop 保存子 stage 模板；materializeLoopRound 生成每轮实例，例如 fix-loop.r01.check、fix-loop.r02.check。独立保存 LoopStateRecord、loopResults 和轮次结果。

```json
{
  "id": "doc-sync",
  "type": "loop",
  "maxRounds": 2,
  "until": { "stage": "draft", "path": "$.consistent", "equals": true },
  "stages": [
    { "id": "compare", "type": "single", "prompt": "Compare documents." },
    { "id": "draft", "type": "single", "prompt": "Revise and emit consistent." }
  ]
}
```

以上为作者文档核心形状的摘录，不是完整可直接运行的 spec。

until 支持字段比较及 all/any 组合；maxRounds 可为正整数或显式 null。文档还描述 no-progress 检测和 onExhausted。循环结果区分 failed、completed、exhausted、stopped_no_progress。

重要的是，外层任务 completed 可能对应循环 exhausted，而不代表业务目标达成。展示层必须读取 loop outcome，而不能只显示绿色 completed。

### 5.3 存储与执行

```text
.pi/workflows/<run-id>/
  spec.json                 定义快照
  compiled.json             编译/展开后的任务图
  run.json                  权威运行记录
  dynamic/events.jsonl      动态编排操作日志
  dynamic/state.json        可重建投影/缓存
  tasks/<task-id>/
    control.json            机器可读结果
    analysis.md             人类可读分析
    refs.json               引用证据
    output.log / stderr.log
```

任务记录区分 taskId、specId、stageId、dependsOn、status、statusDetail，以及模型、worktree、backend handle 等。流程记录还保存 parent/root run 身份和 loopStates/loopResults。

**源码观察**：store 存在临时文件+rename 的 atomic write、lease ownership 检查及 durable write 路径。这里只确认代码存在，未验证崩溃一致性。

**文档描述**：dynamic controller 用 ctx.agent 创建正式任务，写入 compiled/run，操作日志与请求哈希用于严格 replay。controller 为可信 bundle-local Node.js 代码，不是 OS sandbox。本次未完整核实 dynamic 实现，不能把文档的所有保证当作已验证事实。

### 5.4 场景、可视化与限制

适合多来源调查、批量 foreach、reduce 聚合、产物证据管线、结构化修复循环，以及希望静态定义和动态实例都可追踪的系统。

可视化是只读 TUI：runs→stages→tasks→task detail；没有核实拖拽连线画布。

文档当前限制：

- loop child 严格串行，不支持任意嵌套 loop/foreach/dag 等组合；不能把它当作无限制分层图语言。
- loop 暂不支持 `/workflow resume`。
- maxRounds:null 不自动意味着有无进展保护；未配置相应条件时可能持续循环。
- worktree 不自动 merge。
- 作者安装说明要求 macOS/Linux，native Windows 不支持，建议 WSL2；借鉴结构不受影响，直接安装需考虑。

### 5.5 阅读入口

- [类型：stage](https://github.com/AgwaB/pi-workflow/blob/main/src/types.ts#L294-L354)
- [依赖校验](https://github.com/AgwaB/pi-workflow/blob/main/src/artifact-graph-schema.ts#L670-L720)
- [readiness 调度](https://github.com/AgwaB/pi-workflow/blob/main/src/engine-run-graph.ts#L871-L935)
- [循环物化](https://github.com/AgwaB/pi-workflow/blob/main/src/loop-runtime.ts#L215-L290)
- [循环状态与退出](https://github.com/AgwaB/pi-workflow/blob/main/src/loop-runtime.ts#L418-L531)
- [存储](https://github.com/AgwaB/pi-workflow/blob/main/src/store.ts)
- [使用文档：loop、dynamic、run artifacts](https://github.com/AgwaB/pi-workflow/blob/main/docs/usage.md)
- [TUI 视图](https://github.com/AgwaB/pi-workflow/blob/main/src/workflow-view.ts#L66-L100)

## 6. osolmaz/pi-workflows：带回边的控制流图与过程回放

仓库：[osolmaz/pi-workflows](https://github.com/osolmaz/pi-workflows)。

### 6.1 数据结构与边的含义

定义以 TypeScript 编写，核心字段为 name、startAt、nodes 字典、edges，另有 includes、exits、maxSteps 等。节点可包含回调，因此不是完整可序列化的 JSON DSL；另有 definition/node snapshot 供展示与存档。

**源码观察**：边类型为：

```ts
type WorkflowEdge =
  | { from: string; to: string }
  | {
      from: string;
      switch: { on: string; cases: Record<string, string> };
    };
```

每个节点最多一条出边定义，但一条 switch 可有多个目标。执行器维护 currentNodeId，逐个 await 当前节点，再 resolveNext；不是维护多个 ready 节点的依赖调度器。

```text
implement → verify ── true → done
    ↑          │
    └─ false ──┘
```

这里 A→C 与 B→C 表示路径可以流入 C，不表示 C 等待 A、B 同时成功。读取 outputs.A 也不自动形成调度依赖。

### 6.2 循环、子流程与节点类型

- 回边就是循环；controlLoop helper 生成 switch 和普通返回边，不增加独立 runtime Loop 节点。
- maxSteps 默认 100，是执行步数保险，不是循环轮数。
- 文档要求 workflow 自己识别无进展；helper 不自动证明每轮有有效进展。
- include + named exits 支持子流程组合；重新进入子流程是新的 invocation。
- 节点类型包含 agent、compute、notify、action、checkpoint；shell 是 action 执行形式。
- 文档描述 agent 可在原 Pi 对话中执行，checkpoint 可等待输入，action 的副作用需有幂等或人工恢复契约。

### 6.3 运行与持久化

**源码观察**：run state 包含 runId、status、input、outputs、results、steps、currentNode/currentAttemptId、paused、waitingOn、finalOutput 等。

```text
outputs[nodeId] / results[nodeId]   节点最新值
steps / attemptId                  历次访问记录
```

再次成功会覆盖最新 output，失败会删除该节点旧 output，但历史 attempt 仍保留。暂停是独立 flag，不是已读 status 联合中的枚举值。

**文档描述**：生产持久存储使用 SQLite 的 runs、run_sources、node_attempts、run_steps 等规范化表，大值走内容寻址 blobs；生产由 server 打开活动数据库，extension、CLI、viewer 走协议。运行修订与展示修订、durable state 与 live display 分开。本次未逐条审计 DDL/事务与恢复协议。

### 6.4 可视化设计

**源码观察**：图布局将 switch 展开为可画边，用 DFS 标记回边，移除回边后做分层，长边插入虚拟过路节点，再通过排序减少交叉。

**文档描述**：Pi widget 是节点列表；完整 boxed graph 在独立 viewer 中。Rust piw 提供 graph、run browser、inspector、timeline 与 replay。节点卡片只显示短状态，完整 prompt/output/error 按需放在 inspector。结构 scene 与状态投影分离，避免状态更新导致节点到处移动。

### 6.5 场景与限制

适合以单执行路径推进的“观察→决定→执行→验证→修复”，人工 checkpoint，带环控制逻辑，以及重视过程回放的工具。

不适合直接充当通用并行 prerequisite DAG 引擎。若要等待多个前置、并行 fan-out/fan-in，必须另设 readiness/join 语义；不能仅添加多入边就认为实现了依赖调度。

### 6.6 阅读入口

- [类型定义](https://github.com/osolmaz/pi-workflows/blob/main/src/workflows/types.ts)
- [图校验与路由](https://github.com/osolmaz/pi-workflows/blob/main/src/workflows/graph.ts#L9-L145)
- [执行器](https://github.com/osolmaz/pi-workflows/blob/main/src/workflows/engine.ts)
- [controlLoop](https://github.com/osolmaz/pi-workflows/blob/main/src/workflows/control-loop.ts)
- [回边与图布局](https://github.com/osolmaz/pi-workflows/blob/main/src/render/graph.ts)
- [循环设计](https://github.com/osolmaz/pi-workflows/blob/main/docs/CONTROL_LOOPS.md)
- [持久存储设计](https://github.com/osolmaz/pi-workflows/blob/main/docs/SQLITE_STATE.md)
- [终端 viewer 设计](https://github.com/osolmaz/pi-workflows/blob/main/docs/TUI_VIEWER.md)

## 7. BlackBeltTechnology/pi-flows：YAML 依赖编排与事件式仪表盘

仓库：[BlackBeltTechnology/pi-flows](https://github.com/BlackBeltTechnology/pi-flows)。本节主要依据作者 README、架构和集成文档，未逐行核实执行器。

### 7.1 定义与调度

```yaml
steps:
  - id: research
    type: agent
    agent: researcher
    task: Investigate the requirements
  - id: implement
    type: agent
    agent: developer
    blockedBy: [research]
    inputs:
      context: "${{result.research.summary}}"
```

以上为格式示意，省略 flow/agent 所需其他定义，未运行验证。

- blockedBy 表达前置依赖，独立步骤在 max_concurrent 内并行。
- 模板变量在派发时绑定输入，可引用 task、input、上游 result。
- 五类显式步骤：agent、fork、agent-decision、code、code-decision。
- fork 面向用户选择，可启用自动路由；agent/code decision 提供不同决策执行方式。
- decision 的 branch 指向较早步骤可形成循环，要求 max_iterations。
- 每个 flow 是包含 flow.yaml 与代码 handler 的独立目录。

README Quick Start 的个别例子省略 type，但后面的格式说明要求显式 type。参考实现时应以完整 authoring reference 和对应版本校验为准，不直接复制简化例子。

### 7.2 事件、视图与持久化

```text
YAML 定义 → Flow Executor → flow:* 事件
                              ├→ TUI 卡片与摘要
                              ├→ 生命周期事件持久化
                              └→ bridge → Web dashboard 状态投影
```

文档给出的事件记录：

```ts
interface FlowEventRecord {
  seq: number;
  eventType: string;
  data: unknown;
  flowRunId: string;
}
```

引擎和配套 pi-agent-dashboard 分仓库，通过事件总线和 WebSocket 协议连接。Dashboard reducer 重建视图；部分展示迁移到 server-driven intent rendering，文档说明仍存在新旧路径共存。

### 7.3 场景与关键限制

适合固定多代理流水线、确定性代码/模型混排、用户选择与自动路由、实时查看多任务进展。

**作者架构文档明确：流程在父 Pi 进程内运行，不可恢复执行。**关闭会话会失去执行中的 promise、controller 和子 session。保存的生命周期事件可以重建历史卡片，但不是执行 checkpoint。

这提供一个重要设计教训：UI reload survival 与 executable workflow resume 是不同承诺，必须分别定义和验证。

### 7.4 阅读入口

- [README](https://github.com/BlackBeltTechnology/pi-flows#readme)
- [流程作者指南](https://github.com/BlackBeltTechnology/pi-flows/blob/HEAD/docs/flow-authoring.md)
- [执行、分支、循环与关闭会话行为](https://github.com/BlackBeltTechnology/pi-flows/blob/HEAD/docs/architecture.md)
- [Dashboard 事件与持久化](https://github.com/BlackBeltTechnology/pi-flows/blob/HEAD/docs/dashboard-integration.md)

## 8. tintinweb/pi-tasks：结构化任务账本与轻量依赖协调

仓库：[tintinweb/pi-tasks](https://github.com/tintinweb/pi-tasks)。本节主要依据 README，未审计 task-store 实现。

### 8.1 数据结构

从工具与存储文档可归纳出 task 的关键概念：id、subject、description、activeForm、status、owner、metadata、agentType、blocks、blockedBy。这里是字段概念归纳，不是原始完整 TypeScript 接口。

- blocks/blockedBy 双向维护。
- 状态更新工具使用 pending、in_progress、completed，以及表示删除操作的 deleted；删除会移除记录，不应将 deleted 简单理解为永久保留的执行终态。
- 多会话可共享 file-backed task list，并有文件锁。
- 可选择 memory、session、session-global、project 等存储作用域。

### 8.2 功能与场景

TaskCreate/List/Get/Update 管理账本；TaskOutput/Stop 观察和控制后台执行；TaskExecute 可通过 tintinweb/pi-subagents 启动任务。auto-cascade 在前置完成后自动启动解除阻塞的下游，并传递前置已存结果。

适合希望在普通代理对话上增加“任务清单 + 自动级联”的系统，不一定需要引入完整流程语言。界面是持久任务 widget，重点显示状态、阻塞、owner 和运行指标，不是完整 DAG 编辑器。

### 8.3 限制

- 环、自依赖、悬空引用，以及绕过未完成 blocker 的部分 TaskUpdate 操作，会保存并给出警告，不是全部强制拒绝。TaskExecute 有更严格的可执行检查。
- 任务 completed 不应自动等价为有验收证据的业务成功。停止说明也区分是否确认停止，但任务账本可能仍被标为 completed，后续生命周期事件再结算实际 outcome。
- 本次没有核实通用 loop、条件汇合或完整工作流恢复能力。
- 双向冗余边适合快速读取，但要保证更新一致性；自建系统也可以只存一份 edge 事实，反向关系派生。

参考：[依赖管理](https://github.com/tintinweb/pi-tasks#dependency-management)、[任务存储](https://github.com/tintinweb/pi-tasks#task-storage)、[TaskExecute](https://github.com/tintinweb/pi-tasks#taskexecute)。

## 9. nicobailon/pi-review-loop：会话级审查循环

仓库：[nicobailon/pi-review-loop](https://github.com/nicobailon/pi-review-loop)。本节依据 README，未审计完整运行代码。

### 9.1 数据与功能

它没有用通用 nodes/edges 图定义业务，而是通过 reviewerLoop 配置控制会话：

```json
{
  "reviewerLoop": {
    "maxIterations": 7,
    "autoTrigger": false,
    "freshContext": true,
    "reviewPrompt": "template:double-check"
  }
}
```

另支持 triggerPatterns、exitPatterns、issuesFixedPatterns，模式可 extend 或 replace，prompt 可来自模板、文件或内联文本。

功能包括手动启动、可选自动触发、每次回复后继续审查、轮数上限、退出检测、用户输入退出、页脚轮次显示和 fresh-context 过滤。fresh-context 保留审查前上下文，移除已完成旧轮次，保留本轮工具调用。

### 9.2 场景与限制

适合“重复检查，直到没有新问题”的单会话工具，是理解最小循环状态机和 Pi 会话 hook 的参考。

但自然语言模式检测不是强结构化验收，作者虽处理“修复了问题但说没有更多问题”等情况，也不能由此证明语义判断总正确。没有通用依赖、并行汇合、循环子图模型；本次也未核实跨进程持久恢复。

参考：[README](https://github.com/nicobailon/pi-review-loop#readme)、[Fresh Context](https://github.com/nicobailon/pi-review-loop#fresh-context)、[配置](https://github.com/nicobailon/pi-review-loop#configuration)。

## 10. 横向设计取舍

### 10.1 三种边不能混成一种

| 关系 | 回答的问题 | 对应参考 |
|---|---|---|
| 控制转移 | 当前执行结束后去哪里？ | osolmaz 的 to/switch；pi-flows 的 decision branches |
| 顺序依赖 | 哪些执行满足条件后才可开始？ | AgwaB after；pi-flows blockedBy；pi-tasks blockedBy |
| 数据依赖 | 输入取谁的哪个输出？ | AgwaB from；pi-flows inputs；pi-subagents 的显式参数构造 |

A→C、B→C 可能表示等待全部、等待其中一条活动路径、或者单游标跳转。必须由模型与调度契约明确，不能靠图形外观猜测。

### 10.2 三种循环路线

| 路线 | 优点 | 代价 | 适用偏向 |
|---|---|---|---|
| JS 程序循环 | 动态、灵活、模型容易生成 | 静态画图与恢复较难，需要稳定调用身份 | 动态代理编排 |
| 显式 Loop 模板 + 轮次实例 | 作用域和历史清楚，便于折叠/展开 | 实例增长；嵌套与跨轮数据要定义 | 视觉化、可审计任务管线 |
| 控制流回边 | 路由直接，修复状态机自然 | 多游标/并行汇合、作用域、最新值语义复杂 | 单路径控制流与交互过程 |

显式 Loop 不是唯一正确方式；如果目标包含依赖并行和画布编辑，它通常更容易控制复杂度。若目标是单游标状态机，回边很自然。

### 10.3 五个“恢复”承诺需要拆开

1. 页面重开后恢复卡片和历史。
2. 重新连接仍存活的执行器。
3. 复用已经完成的结果。
4. 失败后创建新的 attempt。
5. 进程崩溃后继续逻辑流程，并避免重复副作用。

事件落盘、JSON 状态、SQLite、receipt 只能提供实现材料，不自动满足以上全部承诺。尤其副作用成功但记录未落盘的 crash window，必须依靠幂等标识、执行器查询/对账或明确人工恢复。

### 10.4 生命周期与业务结果分离

至少区分：

- 已派发与已完成。
- completed 与验收通过。
- 循环结束与达到目标。
- 已请求取消与确认停止。
- skipped、failed、cancelled。
- 业务下一轮与基础设施重试。

AgwaB 的 loop exhausted、pi-subagents 的 running receipt、pi-tasks 的 stopped/completed 账本语义，都说明只用一个 ok 或 status 很容易误报。

## 11. 面向自建系统的建议模型

本节为设计建议，不是某个项目的现有 schema，也不代表已经适配或实现到 herdr-bot。

### 11.1 主体分层

```text
WorkflowDefinitionVersion       不可变定义版本
  ├─ Graph / Node / Edge        类型化节点、子图与显式关系
  └─ Input / Output Schema      数据契约

CompiledPlan                    可选：展开和规范化后的执行计划

WorkflowRun                     某次运行、锁定定义与输入
  └─ NodeExecution              节点在具体作用域/轮次中的实例
       └─ Attempt               一次实际执行/重试

Artifact                        报告、补丁、大输出
RunEvent                        有序事实记录
ViewProjection / Layout         状态投影和画布布局
```

定义节点 ID、展开实例 ID、attempt ID、外部执行器 run ID 应分别保存。循环第 3 轮的审查重试一次，仍是第 3 轮，不是第 4 轮。

### 11.2 存储建议

- 如果面向多会话/多进程服务器，关系型数据库保存版本、run、execution、attempt、event；JSON 字段保存配置和结构化小输出。
- 大产物独立存文件/对象存储，数据库保存 URI、摘要、大小、内容类型。
- 初期可以整份 JSON 保存不可变定义，不必一开始将每个定义节点拆成数据库表。
- 布局与执行语义分开，拖动节点不能改变依赖或循环规则。
- 只有需要查询复杂图关系时再评估图数据库；“数据是图”不构成必须使用图数据库的理由。
- 哪份状态是权威、哪份是缓存/投影必须明确；事件和快照之间要有一致性、顺序与修订规则，而不是双写后各自为准。

### 11.3 推荐控制流

针对当前需求，建议选“分层依赖图 + 显式 Loop 容器”，而不是任意带环的 prerequisite 图：

```text
主图：调研 → 修复循环 → 汇总
              │
              └ 子图：实现 → 并行审查 → 汇合 → 本轮结果
```

- 每个普通 graph 内部无环。
- Loop 明确 body、initialState、nextState、until、maxIterations、onLimit。
- 子图内部引用限定在当前轮；跨轮读取通过 loop state，不通过隐含“最新输出”。
- 并行 join(all) 与互斥分支 merge 分开，否则未选择分支可能造成死锁。
- 每轮按 scope path 生成可追踪实例，例如 `repair:3 / review.security`。
- 动态 agent 和手工定义节点共享同一执行账本，不能成为观察盲区。

如需脚本扩展，先让脚本通过受控 Interface 创建正式执行实例；不要允许它绕过宿主直接修改权威状态或自授予权限。不要同时实现第二套互不相通的脚本调度器和图调度器。

### 11.4 可视化建议

提供三种视角，而不是只做一张图：

1. **定义视图**：静态节点、关系、Loop/子图容器。
2. **运行视图**：当前执行状态、已展开轮次、动态任务、活动路径。
3. **历史视图**：attempt、事件时间线、某轮输入输出、恢复与重试来源。

布局位置不参与执行指纹；状态更新尽量不重新布局。节点卡片显示短状态和证据摘要，完整 prompt/output/error 在 inspector 中按需加载。回放展示当时事实，不能拿当前最新 output 冒充历史结果。

## 12. 推荐阅读顺序与最终判断

| 阅读顺序 | 项目与重点 | 想解决的问题 |
|---|---|---|
| 1 | AgwaB：types → schema → loop-runtime → store | 依赖、循环模板、执行实例、产物存储怎样分层 |
| 2 | osolmaz：graph → engine → types → render/graph → viewer 文档 | 控制边、attempt 历史、回边布局与回放怎样设计 |
| 3 | pi-flows：authoring → architecture → dashboard-integration | 声明式任务如何调度，如何用事件解耦 UI |
| 4 | 本地 pi-subagents：script runtime → executor → reuse/receipt | 动态脚本、稳定 key、监督、权限与结果交接 |
| 5 | pi-tasks / pi-review-loop | 轻量任务依赖和最小会话循环是否已经足够 |

最终建议不是选一个仓库照搬，而是先确定产品中心：

- 模型临时编排优先：偏向 pi-subagents。
- 依赖/循环/可视化编辑优先：偏向声明式图与显式 Loop，参考 AgwaB。
- 单执行路径、人机交互、修复回路优先：参考 osolmaz 的控制流。
- 固定流程、混合执行器、实时仪表盘优先：参考 pi-flows，但不要继承其不可恢复执行的限制而不自知。
- 只需要任务账本或重复审查：优先考虑更小的 pi-tasks / review-loop 模型，避免过度设计。

对当前提出的“依赖、循环、方便可视化”目标，最值得保留的不变量是：**定义、执行实例、尝试、结果证据和展示投影彼此分离；任何边与状态都具有明确、可验证的执行语义。**
