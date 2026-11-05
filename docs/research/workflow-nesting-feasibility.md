# Workflow 嵌套与两个 Bot 案例：事实验证

日期：2026-11-05。范围：四路只读 subagent 源码核查，以及主代理执行的本地 JSON 定义检查。不是嵌套实现验收；未启动业务服务、用户 Chat 或真实工作流，未运行构建或测试套件，未修改业务配置。

## 结论

现有单 Run 命名文件交接和 execution 隔离可复用，但目前不支持 Map 型 Bot、Run 命名输入、可选输入、双角色执行或父子 Run 关联。不能通过仅修改定义与提示词实现嵌套。未发现推翻已确认业务方向的静态证据；实际可行性仍需实现后的隔离运行验证。

## 已执行检查

使用 Python 标准库只读解析 meta-data/workflow-maps/*.json、meta-data/bots/*.json、meta-data/tasks.json：

|Map|节点|node 文件引用|无效输出引用|
|---|---:|---:|---:|
|create-bot-task-workflow--bc-61ce48b80a4c.json|10|18|0|
|enhance-existing-bot--bc-301b25d9f8d6.json|5|10|0|
|plan_workflow.json|2|1|0|
|tibo-monitor-then-c-drive-inspection.json|2|0|0|

共 16 Bot、22 Task，四张 Map 引用 12 个不同 Task；检查未发现重复 Bot ID、Bot 指向不存在 Task、Map 节点缺失 Bot/非 normal Task、blocked_by 依赖环。四张 Map 均没有 input_schema。这些结果只证明静态定义，不证明提示词能完成业务。

## 1. 双角色：扩展存在，授权模型明确冲突

- 固定扩展配置指向实际 my-agent-toolkit checkout 的 OMP/Pi manager-chat、node-chat，四入口存在，不是空实现。配置来源：meta-data/task-options.toml；实际入口为该外仓 entrypoints/{omp,pi}-auto/{manager,node}-chat.ts。
- backend/src/tasks/catalog.rs:76-83,377-387,421-434：ExecutionRole 是单值 Standalone/Manager/Node，固定扩展精确匹配角色，不能加载受限双角色。
- backend/src/runs.rs:549-602：manager_run 拒绝同时关联任意节点执行的身份。
- backend/src/runs/reports.rs:58-85：node_run 发现身份是任意 active Run manager 就立即拒绝 report；即使子 Run 已 finish 也不会绕过。
- backend/src/runs.rs:241-267、backend/src/chats/resume.rs:178-208：持久角色识别与恢复也拒绝 manager+node，不能只修改即时授权。
- 外仓 extensions/workflow-chat/{manager,node}-chat/shared-logic/tools/index.ts：工具分别为 run/collect 与 report，无静态重名；framework/behaviors/injection.ts:43-50 使用 definition.id:tag 区分 Prompt 身份。
- 外仓两个角色的 shared-logic/prompts/init.md 没有说明“内部 run/collect、外部 report”的双层语义，需要调整初始化上下文及指引。

必须以明确持久父 Run/node/execution ↔ child Run 关系证明合法复合身份，不能整体取消身份唯一性校验。工具仍不接受模型自报执行归属。

finish 不结束原生 Session（backend/src/runs/completion.rs:138-161）；外仓公共请求仅随 session_end/cleanup 取消。因此 [INFERENCE] 修正合法身份关联后，finish 后外报无需另造扩展重启机制。两宿主实际同载仍未验证。

## 2. 输入和输出：可复用解析骨架，但跨 Run 链路缺失

- backend/src/scheduler/mod.rs:8-15,54-86,282-291：Map 只有 title/description/nodes；InputRule 仅 file/node；根启动请求只有 managerTaskId/zone/userInput。未知字段被拒绝。
- 无输入规则或 [] 合法；已声明规则都必须解析成功。可选缺席没有现成表示，必须贯穿声明、解析、执行保存和初始化，不得吞掉不可读错误。
- backend/src/runs/startup.rs:4-25：node 来源只读取当前 Run 指定节点当前 execution 的最新同 ID 输出；迟到旧执行报告不作为新输入。
- backend/src/runs/startup.rs:110-116：派发时冻结规则和实际地址，不冻结文件内容。Run 启动只冻结 zone 绑定等，不是整图快照。
- backend/src/runs/reports.rs:102-120：输出 ID 必须属于冻结 output_rule；同 ID 后报更新、省略保留。报告只是登记，不验证实际文件内容。
- frontend/src/scheduler/RunLaunchForm.tsx:17-22,48-73 与 schedulerApi.ts:82-98,130-138：启动 UI 和前端严格形状校验也必须同步更新。
- docs/planning-deliverables.md:25-35、meta-data/tasks.json 的规划/转换者：当前生成契约只覆盖 Task 型 Bot，必须同步 Map 型目标、manager 配置、Run 输入和可选项。

### 修正此前设计说明中的过度陈述

现有直接 file 输入使用 zone 内相对路径，通过 checked_file 检查；省略 zone_id 时使用 manager zone。node 来源则只取得已登记的绝对地址，不再次检查文件存在或 zone 边界（startup.rs:8-20）。因此不能声称“已有机制会统一验证所有下游文件可读”。新根 Run 输入应明确沿用哪种直接文件边界；转交 report 输出若新增存在性/安全校验，属于新增行为，需与兼容要求分别说明。

不增加 Map 输出声明仍可成立：[INFERENCE] 子 manager 将结果报告到父 execution 后，普通下游可以沿用现有 resolver；Map 型下游再把解析结果绑定到自身新子 Run。现状缺少这座桥，尚无运行证据。

## 3. 生命周期与故障：不能直接把根启动成功等同节点成功

- backend/src/runs.rs:45-118：Run/NodeExecution 没有父子关联。
- runs.rs:414-495、runs/startup.rs:51-97：根 Run 接受后异步启动；manager 启动失败只保留进程内诊断。子 Run 已创建、manager Chat 已启动、首次输入已提交是不同阶段。
- runs/startup.rs:100-160、runs/recovery.rs:84-163：节点已有持久 effects、核实与恢复边界，但嵌套时需核对子 Run 事实，不能只看 Chat 是否存在就判未创建。
- runs/retry.rs:31-55：retry 新 execution、旧执行保留可复用；child ID 必须与 execution 一起保留。
- runs/reports.rs:31-43,58-87 与 runs/completion.rs:88-99：旧报告可追加/通知，但不能控制当前 execution；不要声称旧报告完全不投递。
- runs/completion.rs:72-111,133-154：当前 complete 只核对当前最新严格 true，finish 检查所有内部节点完成；没有子 Run finish 门禁，需要新增后端校验。
- runs/operation.rs:55-86、runs/reports.rs:121-148：现有按 Run 串行。父子持锁互等是新增风险 [INFERENCE]，须避免父锁等待子操作、子锁又等待父操作的 ABBA。
- scheduler/mod.rs:990-1026、runs.rs:163-228：当前 Map/history 删除不检查 Bot.map_id 或父子运行引用。新增引用保护不可省略。

## 4. 两个案例及提示词：有实际清理对象，也有不能删除的规则

创建图：create Task → optimize Task → create blank Bot → 三路编写/优化 → save Bot。抽取边界从创建空白 Bot 开始。修订图当前先修改 normal Task.prompt，再三路指南修订；缺少公共 Bot 基线准备节点，不能直接用于 Map 型目标。

相关共享 Task 位于 meta-data/tasks.json：空白 Bot 创建者、when/plan_guide/node_guide 编写者、通用提示词优化者、通用文本替换者、既有 Task.prompt 优化者；它们被两图共用，独立 Chat 分支不能被角色注入替代。

- 创建图 write/optimize 节点和修订图分支明确要求 handoff 复制完整正文及配置；去重需同步生产者和消费者，不是仅删冗长文本。
- 公共 node-chat prompt 已承担命名 inputs、report_policy、简短 report、报告不等于完成等协议。
- 公共 node-chat prompt **没有承担文件安全写入、防链接越界、完整落盘读回**。这些约束在 docs/workflow-runtime.md:196-210 以及业务 Task 中；集中前不能直接删除。
- revision、目标配置不变、运行禁写、共同基线核对、业务确认与授权不是通用报告协议，必须保留。
- 当前 Task prompt 还有真实旧契约：meta-data/tasks.json 中创建 Task/优化 Task 的 prompt 提到旧 extensions 字段，并引用旧 backend/src/tasks.rs；当前 docs/agents/testing.md 明确旧 extensions 字段被拒绝。必须纳入相关调用链清理，不能假设旧流程如今一定可运行。
- 另有独立 create-bot-task、create-bot 能力，并非拟议 Map 包装；概要规划者引用的是前者。后续应明确修改范围，不能误把同名业务能力视为已经调用新子 Map。

共同基线单份、text 单份、handoff 仅确认/来源/限制的设计可沿用已确认方案。消费者直接绑定所有必要文件，不通过 handoff 再自行寻找未声明材料。

## 后续运行证明（本轮未执行）

1. OMP/Pi 实际双扩展同载：三工具可发现、Prompt 不覆盖、真实 Session 正确，finish 后 report 仍发出；受控 observer 只证明扩展机制，不冒充后端嵌套通过。
2. 隔离后端父 Map→子 Map→普通/Map 型下游：空输入、可选缺席、缺必需项、同名 ID、多层绑定、输出 ID 连接。
3. child 未 finish 不能外报可完成；finish 后父仍须验收 complete。
4. child 部分启动失败与核实；retry 新 child、旧报告隔离、并发不互等。
5. 两个实际案例使用隔离对象运行，核对目标不变、公共基线、正文唯一来源、一次合法保存与读回。

现有测试源码提供可复用隔离入口：backend/tests/workflow_dispatch/{named_inputs,input_resolution,report_outputs,retry,completion_gate}.rs；外仓 tests/harness-dock/node-report-contract.test.ts。未运行这些入口，不能记作通过；单 Run 通过也不能替代嵌套验收。

## 调查证据来源与限制

四个子任务：RoleVerification、FileFlowVerification、LifecycleVerification、CasePromptVerification。主代理复核了 catalog 装配、node_run 拒绝条件和实际 resolve_inputs 实现，并实际执行两次只读 JSON 检查。图谱部分过期/缺覆盖，结论以当前文件读取为准。真实模型遵循、原生混载、文件安全效果和嵌套端到端均保持未验证。
