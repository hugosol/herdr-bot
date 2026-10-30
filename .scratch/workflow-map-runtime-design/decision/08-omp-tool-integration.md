# omp extension 接入与可信执行身份

Type: grilling
Status: resolved
Assignee: none
Map: ../map.md
Blocked by: 01, 03, 04, 05, 06
Modules: Bot 执行；流程控制

## Handoff

- 2026-10-28 用户在全分支收口后明确“请发布”；[接入 Lighthouse](../lighthouse/08-omp-tool-integration.md)已完整发布，本票无剩余待访谈问题。resolved 仅表示设计决定完成，不授权产品实现或声称真实运行通过。
- 原部分发布已并入完整权威。上游修订各归其原处：[绑定与 Workspace](../lighthouse/01-execution-binding.md)、[Task 类型与字段](../lighthouse/02-map-definition-contract.md)、[状态工具](../lighthouse/03-runtime-state-operations.md)、[首版协作范围](../lighthouse/04-collaboration-protocol.md)、[人工恢复的信息责任](../lighthouse/05-fault-pause-retry.md)、[保存与报告通知](../lighthouse/07-persistence-consistency.md)。本票链接它们，不另维护相反政策。
- 当前无 unanswered resume question。下一张 frontier 是[运行启动、只读图与人工观察路径](09-observation-experience.md)，本轮未认领。之后由[跨模块设计收敛与实施交接契约](10-spec-acceptance.md)整理精确 schema/路由拼写、文档交付和验收契约，不将这些整理工作等同重新开放已定设计。
- 用户强调曳光弹与复用：撤回顶层共享目录、独立凭据、身份/联系人查询、SSE/长轮询收信、原生 IRC 接收与强制续轮、启动时额外追加扩展、--session-dir 和 autoResume 边界调查。Q16 后又以 Q19 保留报告→manager 单向通知，最终选择 Herdr 输入；这是范围修订，不得恢复早期双向候选。
- 初始身份/安全候选已处置：原生 session_id 由 adapter 自动取得，归属只在后端 Run 关联；用户接受同机信任风险。用户明确 omp 未开启 autoResume；正常启动沿用现有链，先保存关联再提交指示。会话切换不扩展首版。具体确认来源见 Lighthouse，不能把已撤回提案当作遗漏。
- 证据限制：静态核查 herdr-bot、my-agent-toolkit、oh-my-pi、Herdr 与 hyper-auto-coding；未修改产品、运行测试或真实 Agent。公共 HTTP 取消、双声明同目录的检查工具适配、启动时序、Herdr 实际输入与可读路径必须由后续实施验证，不是本次运行证明。
- Backtracer：已对本轮发布的 intent、pattern、dependency 做追踪。独立 Chat/统一 Workspace/Task 类型由绑定与字段提供；动作合法性与故障责任由状态/故障提供；报告保存与 collect 依据由通信/持久化提供；toolkit 分层与 Herdr 接入由本票提供，最终可操作性由交接票验收。新类型的界面管理、启动入口与 Chat 导航已在观察票的配置/观察范围，文档及 HTTP 取消、输入失败等实施检查已在交接票覆盖；开放提供者未视为已经实现。
- 追踪中的旧双向通信、原生 IRC 保证、单独 Workspace 选择、通用发送补交接依赖，均有本轮明确缩减/替代确认，已修订原权威。停止票仍只是范围处置，暂停/恢复/finish 由状态机提供。没有未裁决冲突或需要新增票据的 gap；不修改下游票据的恢复上下文或 claim。下游旧问答/独立放行/停止等历史候选须按现行 Lighthouse 消费，不复活已否定范围。

## Question

如何在产品启动的 omp Chat 中加载真实工具，并通过后端运行关联校验角色与执行身份？

原调查范围包含工具注册、凭据/鉴权、会话隔离、消息继续、等待与生命周期；本轮按用户确认收窄为 Task 类型驱动的孪生自动扩展、同机 session_id 校验、run/collect/report 和 Herdr 单向通知。普通双向通信、强认证、原生 IRC 接入及会话切换恢复不再是本票未决要求。

## Inputs

- [Bot、Task、Chat 与管理角色的绑定](01-execution-binding.md)
- [Map、Bot 与运行配置的契约边界](02-map-definition-contract.md)
- [运行状态机、管理者工具与决策 seam](03-runtime-state-operations.md)
- [通用协作消息、交付与决策会话](04-collaboration-protocol.md)
- [节点故障、等待人工处理与重试](05-fault-pause-retry.md)
- [停止 Map 与 graceful shutdown 生命周期](06-stop-lifecycle.md)
- [运行事实与内存文件一致性](07-persistence-consistency.md)
- [第二阶段交接计划](../../workflow-map-runtime-tracer-implementation-plan.md)：历史输入，以本地图发布修订为准。
- [领域词汇](../../../GLOSSARY.md)

## Comments

- 本轮 Q8–Q37 与后续启动澄清为确认记录，逐主题来源和源码证据见 Lighthouse。原建图“尚无 Lighthouse”的恢复入口已退休。
- 本次只发布需求/设计；用户未授权执行实现票据，不创建产品代码或测试脚手架。
