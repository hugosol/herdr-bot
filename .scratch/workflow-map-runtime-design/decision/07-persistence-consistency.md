# 运行事实与内存文件一致性

Type: grilling
Status: resolved
Assignee: none
Map: ../map.md
Blocked by: 02, 03, 04, 05, 06
Modules: 运行状态与恢复

## Handoff

- 已确认结果与确认来源统一见[运行事实与内存文件一致性：Lighthouse](../lighthouse/07-persistence-consistency.md)。2026-10-28 本票 Q26 用户“同意”授权完整发布；本票没有剩余待访谈问题，不授权实现产品。
- 关键取舍与撤回原因见 Lighthouse：用户要求首版简单，撤回多文件事务／启动回滚方案；错误只中止当前调用，不持续封锁 Run。接受的部分提交、外部副作用与重复启动风险均已明确，不将早期候选恢复为要求。
- 原范围收口：当前状态内容和派生快照、文件布局、单写入主体、文件级协调、提交顺序、错误与重启语义已确定；不增加额外版本号或配置快照。精确可信身份字段由接入票据提供，字段拼写及跨模块接口在交接票据收敛，不是未完成的持久化策略。
- Backtracer 已追踪本轮发布：当前事实／历史观察由[运行启动、只读图与人工观察路径](09-observation-experience.md)覆盖；可信 Chat 关联、实际投递及错误响应由[omp extension 接入与可信执行身份](08-omp-tool-integration.md)覆盖；文件层与业务职责、部分提交及重启只读验收由[跨模块设计收敛与实施交接契约](10-spec-acceptance.md)覆盖。现有问题已包含这些消费范围，不新增票据或修改下游 Handoff／claim。
- 上游核对：配置不变与无配置快照保持不变；Run 业务串行不等于文件锁；正常已记录派发去重不扩大为异常窗口幂等；collect 待办与检查位置分开；后端重启只读不否定后端仍存活时的人工节点恢复。停止生命周期仅消费既有处置及暂停替代输入。未发现新增未覆盖问题或需用户裁决的上游冲突；接入开放前置不等于已经实现。
- 下一张按 frontier 为[omp extension 接入与可信执行身份](08-omp-tool-integration.md)，先核查 Task 扩展加载、connect-bot 复用和 omp 实际会话接口，再落实可信身份；本次不认领该票。[运行启动、只读图与人工观察路径](09-observation-experience.md)也可独立推进。地图其余 fog 仍依赖接入事实，尚无新增可单独命题的缺口。
- 证据限制：静态读取 tasks／history 文件保存代码并定位 zones／scheduler 同类模式，阅读 Rust 官方 File／rename 文档；未运行产品、测试、Windows 故障或真实 Agent 场景。安全替换、错误响应复用及实际接入仍须实施验证，源码惯例不是目标能力已实现的证明。

## Question

运行态如何在内存与运行文件间形成一份逻辑真相，并可靠提交操作？

确定数据 schema、快照、身份引用、修订号、单写入者、串行化粒度、可靠替换、提交／通知顺序、写盘失败语义、外部 Chat 动作与落盘非原子窗口、重启后中断表达。旧执行与交付不能因重试隐式丢失；不承诺自动恢复。验证 Windows 现有存储惯例，但本票不实现产品。

## Inputs

- [Map、Bot 与运行配置的契约边界](02-map-definition-contract.md)
- [运行状态机、管理者工具与决策 seam](03-runtime-state-operations.md)
- [通用协作消息、交付与决策会话](04-collaboration-protocol.md)
- [节点故障、等待人工处理与重试](05-fault-pause-retry.md)
- [停止 Map 与 graceful shutdown 生命周期](06-stop-lifecycle.md)

- [第二阶段交接计划](../../workflow-map-runtime-tracer-implementation-plan.md)。其中暂停／重试等排除项须与本轮范围修订合并阅读。
- [领域词汇](../../../GLOSSARY.md)。

## Comments

- 本票 live grilling 已完成。Q3–Q5、Q8–Q11、Q14–Q21、Q25 的选择及 Q22–Q24 错误处理澄清已发布至 Lighthouse；Q26 用户确认整体设计并授权发布。
- Q6／Q7 曾接受的强事务要求被用户“不要在第一版设计太复杂的机制去解决低概率边界情况”明确替代；Q12 回滚实现未采用。Q17 持久投递结果候选撤回，改用诊断日志；Q23 持续封锁 Run 候选撤回，改为通用错误传播中止当前调用。完整现行政策与确认原话见 Lighthouse，不在票据维护第二套答案。
- 本次只发布设计、追踪依赖并完成决策票，不实现通用文件层、产品接口或真实验收。
