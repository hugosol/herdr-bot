# 停止 Map 与 graceful shutdown 生命周期：Lighthouse

> Question and discussion: [停止 Map 与 graceful shutdown 生命周期](../decision/06-stop-lifecycle.md)

## Question

独立停止入口与 graceful shutdown 生命周期是否仍是首版所需？本文记录范围处置，不声称回答了原票所有停止设计问题。

## Discussion

### 暂停承接停止派发需求

确认来源：运行状态机票据会话 Q63 用户要求初版不要添加过多特性；Q66 澄清后用户明确“Q66 同意”（2026-10-28）。旧地图要求独立 stop 工具与 graceful shutdown 设计；现修订为只保留 pause／resume，不增加 stop、pausing、stopping、stopped。暂停生效即禁止新派发，已有执行可继续并自然收尾，不等待全员收尾才返回暂停成功。暂停可以恢复，也可以一直保持；不承诺活动 Chat 退出、资源释放或统一终止。全部节点完成后仍可由管理者显式成功结束。

独立停止票据因此不再需要，而非原停止终态问题已经被逐项解决。暂停操作契约由[运行状态机、管理者工具与决策 seam](03-runtime-state-operations.md)提供。

### 依赖审查与追踪

原依赖本票的运行事实与内存文件一致性、omp extension 接入与可信执行身份、运行启动与人工观察、跨模块交接，不再需要独立停止生命周期输出。其暂停／恢复与成功结束输入由运行状态机票据提供；故障恢复由故障票据提供，通知／等待由协作票据提供。这些提供者尚未全部解决，因此不据本票 resolved 宣称下游已解锁。

追踪绑定与字段契约：未发现要求活动 Chat 必须退出的上游已发布承诺；管理者一贯通过工具操作，静态 Map 不承载运行状态。旧地图独立 stop 要求由本次明确修订取代。未发现需新增决策票据的缺口。外部 Chat 生命周期和持久化实现仍未验证。

## Decision

独立停止生命周期退出本轮范围，停止新派发的需求由可恢复暂停承接；不新增 stop 工具或停止过渡／终态。

## User stories

1. As a 管理者, I want 暂停新派发并允许已有节点收尾, so that 我能停止推进且保留恢复自由度，无需独立停止机制。

## Preconditions

- 暂停、恢复与正常结束消费运行状态机契约；消息反馈与接入机制由对应未完成票据提供。

## Postconditions

- 下游不再要求独立停止工具或停止终态；原停止依赖以本处置及运行状态机的替代输入解释。
- 本票 resolved 仅表示确认不再需要，不表示 graceful shutdown 已实现。

## Invariants

- 暂停不等于外部 Chat 全部退出；允许既有执行反馈。
- Map 是静态定义，Run 是执行；运行操作仍受管理者权限和串行入口约束。
