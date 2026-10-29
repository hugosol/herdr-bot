# Workflow：运行实例列表交互参考

调研日期：2026-10-29。仅设计调研，不实施 UI，不修改产品代码。沿用本目录既有的「来源事实 → 适配建议 → 风险与取舍」结构。下文所有适配建议均为**待讨论建议，不是已批准决策**。

## 范围与术语

- **本项目边界（来自任务说明，未核验本地应用）：** Workflow 入口读取 Run，也就是 Map 的一次运行事实；Scheduler 编辑静态 Map 并发起 Run；Bot 是共享能力定义，不是 Task 或 Chat。本笔记不重新定义这些职责。
- **参考对象：** GitHub Actions 的 workflow runs、n8n 的 executions、Temporal Web 的 Workflow Executions，均比较运行实例，而非流程定义目录。GitHub 明确以某次 run 进入 summary；n8n 明确区别 execution history（运行）与 workflow history（定义版本）；Temporal 的 Workflows 主表虽然叫 Workflows，内容实际是保留期内的 Workflow Executions。[GitHub 运行历史](https://docs.github.com/en/actions/how-tos/monitor-workflows/view-workflow-run-history)、[n8n 单流程运行](https://docs.n8n.io/build/understand-workflows/understand-executions/view-executions-for-a-single-workflow)、[Temporal Web UI](https://docs.temporal.io/web-ui)
- **证据范围：** 以官方现行文档为依据，不把 CLI/API 能力当成已经确认的 Web UI 功能；不声称复刻了实际屏幕的像素、列宽、默认排序或交互细节。三者已覆盖「CI 运行诊断」「节点式自动化执行」「长运行状态观测」，本轮不再扩展 Prefect。

## 一、简明比较

| 维度 | GitHub Actions workflow runs | n8n executions | Temporal Web Workflow Executions |
| --- | --- | --- | --- |
| 对象与范围 | 选择 workflow，再从其 runs 点入单次 summary；不是选择 YAML 后编辑。[运行历史](https://docs.github.com/en/actions/how-tos/monitor-workflows/view-workflow-run-history) | Overview 可看有权访问的 workflows 的 executions，project 下限定为该 project，单 workflow 的 Executions 又限定为该流程。[全局运行](https://docs.n8n.io/build/understand-workflows/understand-executions/view-all-executions)、[单流程运行](https://docs.n8n.io/build/understand-workflows/understand-executions/view-executions-for-a-single-workflow) | 当前 Namespace、保留期内的 execution 表，选择一条进入对应执行历史及元数据。[Web UI](https://docs.temporal.io/web-ui) |
| 信息层级 | Run summary → job（列表或图）→ step 日志；运行状态、终态结果与 job/step 诊断分层。[运行日志](https://docs.github.com/en/actions/how-tos/monitor-workflows/use-workflow-run-logs) | 全局/项目运行列表 → 单次 execution；调试时显式选择 Debug in editor 或 Copy to editor，才把历史数据载入当前流程。[全局运行](https://docs.n8n.io/build/understand-workflows/understand-executions/view-all-executions)、[调试运行](https://docs.n8n.io/build/understand-workflows/understand-executions/debug-executions) | Execution → History / Workers / Relationships / pending Activities / Queries / Metadata；详情包含开始、结束、耗时、Run ID、Workflow Type、Task Queue 等。[Web UI](https://docs.temporal.io/web-ui) |
| 筛选与状态 | Web 文档确认存在 Filter workflow runs 字段，但本轮来源未完整列出其筛选项；官方 CLI 明确支持 workflow、branch、commit、event、created、user、status。不能把 CLI 清单直接当作 Web 清单。[状态徽章页](https://docs.github.com/en/actions/how-tos/monitor-workflows/add-a-status-badge)、[CLI run list](https://cli.github.com/manual/gh_run_list) | 文档列出 workflow（全局列表）、Failed / Running / Success / Waiting、开始时间、保存的自定义数据；自定义数据有套餐条件。[全局运行](https://docs.n8n.io/build/understand-workflows/understand-executions/view-all-executions) | Status、Workflow ID、Workflow Type、开始/结束时间、Search Attributes；可用表单或原始查询，Saved Views 可保存常用条件。[Web UI](https://docs.temporal.io/web-ui) |
| 异常定位 | 失败 step 自动展开，显示该 step 的日志及耗时；日志行可复制永久链接。日志搜索只覆盖已展开 steps。[运行日志](https://docs.github.com/en/actions/how-tos/monitor-workflows/use-workflow-run-logs) | 失败 execution 可进入 Debug in editor；这会复制运行数据到当前 workflow 并固定首节点数据，属于调试动作，不是纯查看。[调试运行](https://docs.n8n.io/build/understand-workflows/understand-executions/debug-executions) | History 有 Timeline / All / Compact / JSON；Task Failures 视图能找出仍 Running、但 Workflow Task 持续失败或超时的实例。[Web UI](https://docs.temporal.io/web-ui) |
| 操作边界 | 重跑全部、失败 jobs 或单个 job 是不同动作；可选看之前的 run attempt，重跑使用原事件的 SHA / REF。[重跑](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/re-run-workflows-and-jobs) | Retry 可选原 workflow 或当前已保存 workflow，使用历史数据；从保存状态恢复，每次 retry 是新的 execution，失败节点可能重复外部副作用。[全局运行：What a retry runs](https://docs.n8n.io/build/understand-workflows/understand-executions/view-all-executions#what-a-retry-runs) | Request Cancellation、Terminate、Reset、Signal/Update，以及 Start Workflow Like This One 是不同能力；取消是协作请求，强制终止不运行清理，Reset 则终止当前 Run 并从历史点启动新 Run。[Web UI](https://docs.temporal.io/web-ui)、[取消与终止](https://docs.temporal.io/encyclopedia/workflow/cancellation-and-termination) |

## 二、三个参考各自最值得借鉴什么

### 1. GitHub Actions：从「哪次运行有问题」收敛到「哪一步、哪行证据」

**来源事实：** 官方路径是 Actions → workflow → run summary → job → step 日志；失败步骤自动展开，支持日志行链接与步骤耗时。重跑还区分全部 jobs、失败 jobs 和指定 job，历史 attempts 可单独查看。[运行历史](https://docs.github.com/en/actions/how-tos/monitor-workflows/view-workflow-run-history)、[运行日志](https://docs.github.com/en/actions/how-tos/monitor-workflows/use-workflow-run-logs)、[重跑与 attempts](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/re-run-workflows-and-jobs)

**适配建议：** Workflow 列表只承担运行定位；进入 Run 后先展示结果摘要，再定位异常 Task/节点及错误证据。列表不展开完整日志或把所有节点都塞进一行。若项目已有可寻址的运行详情，可考虑从列表直接进入异常位置；这不是要求新增日志存储或永久链接能力。

**风险：** GitHub 的 CI commit/branch 语境不等于本项目 Map 语境；不要照搬代码提交列。也不要把「重跑」「刷新运行事实」「编辑 Map」做成同一种循环箭头。仅展示已有事实，不根据 UI 参考虚构 Task 耗时或失败原因。

### 2. n8n：全局运行与单流程运行两种范围，历史查看与编辑调试显式分开

**来源事实：** 全局/项目 Executions 与单 workflow Executions 是不同查询范围；执行历史不等于定义版本历史。失败执行的 Debug in editor、成功执行的 Copy to editor 会把历史数据复制到当前 workflow，固定在首节点，而不是保持只读历史视图。[全局运行](https://docs.n8n.io/build/understand-workflows/understand-executions/view-all-executions)、[单流程运行](https://docs.n8n.io/build/understand-workflows/understand-executions/view-executions-for-a-single-workflow)、[调试运行](https://docs.n8n.io/build/understand-workflows/understand-executions/debug-executions)

**适配建议：** 运行列表中的 Map 是「这次 Run 属于哪个 Map」的归属信息，不是把列表变成 Map 编辑目录。若保留跨入口导航，「查看这次运行」与「到 Scheduler 编辑 Map」应明确命名；不能点击历史 Run 就静默切换到当前定义，冒充当时运行事实。

**风险：** n8n 的重试可选择原定义或当前已保存定义，而且不是统一从 trigger 全量重跑；每次 retry 生成新 execution，失败节点可能重做已经完成的部分外部动作。这些是 n8n 的执行语义，不是本项目现有能力的证明。[重试语义与副作用](https://docs.n8n.io/build/understand-workflows/understand-executions/view-all-executions#what-a-retry-runs)

### 3. Temporal Web：运行状态不等于健康程度，先暴露事实，再提供有边界的操作

**来源事实：** Temporal 以 Namespace 限定 execution 范围；时间可使用 UTC、Local 或 Relative 格式；详情提供事件历史、输入/结果、等待活动、Worker 和关系信息。Task Failures 视图刻意展示仍 Running 但出现持续 Workflow Task 失败/超时的实例，不能简单把 Running 等同于健康。[Web UI：Workflows、History、Task Failures](https://docs.temporal.io/web-ui)

**来源事实：** Saved Views 保存常用查询，存于当前浏览器且属用户私有；临时修改条件不会自动覆盖已保存视图。分享相对时间视图时，URL 使用绝对时间。[Web UI：Saved Views](https://docs.temporal.io/web-ui#saved-views)

**适配建议：** 已有运行状态与已有异常证据分层显示；例如 Run 仍在运行但某 node 失败，不应为了一个红点改写 Run 总状态。先选简单状态/Map 筛选，只有真实查询规模需要时再讨论保存视图；不把 Temporal 的查询语言、Namespace、Task Queue 等术语搬进本项目。

**风险：** 「请求取消」不等于「已取消」；Terminate 不做清理；Reset 会创建新 Run，不能包装成普通刷新。只有后端明确支持且产品确认后，才讨论这些按钮。[取消、终止与 Reset](https://docs.temporal.io/encyclopedia/workflow/cancellation-and-termination)

## 三、适用于本项目的候选信息层级（未批准）

以下是由上述参考归纳的设计建议，不是本地应用现状，也不是新增后端字段的需求。字段必须先确认数据可得。

1. **行的主身份：这一次 Run。** 主标题可用 Map 标识，紧邻短 Run ID；若展示当前 Map 名称，须明确它来自当前定义而非历史快照。本项目不新增运行名称；同一 Map 多次运行必须可区分。完整 ID 可复制或在详情查看，不能只靠相同 Map 名选运行。
2. **第一扫描层：状态、归属 Map、开始时间。** 状态用文字与标记共同表达，不只用颜色；按产品实际状态词汇呈现，不直接照搬其他产品的 Failed / Waiting。时间的相对值要有明确绝对值查看途径，统一时区。
3. **第二扫描层：结束/耗时与已有异常摘要。** 只有已有可靠运行事实时才显示；不以进度百分比、预计完成时间或自造「健康」分数补空白。进行中与已结束的时间文案要不同。
4. **详情首屏：当前 Run 身份与结果。** 其后才是 Task/节点事实和异常证据。Map 定义编辑属于 Scheduler；Bot 能力定义另有归属，不把它们混成运行列表行。
5. **交互优先级：打开详情优先，管理动作后置。** 行主链接只导航；复制 ID、已有刷新可为次级动作。取消、重试、删除不因参考产品有就新增；未来若批准，先定义作用对象、状态条件、影响范围与结果反馈。

推导依据分别是 GitHub 的 run → job → step 诊断层次、n8n 的 execution / workflow history 分离、Temporal 的 ID/时间/状态/历史证据分离。[GitHub](https://docs.github.com/en/actions/how-tos/monitor-workflows/use-workflow-run-logs)、[n8n](https://docs.n8n.io/build/understand-workflows/understand-executions/view-executions-for-a-single-workflow)、[Temporal](https://docs.temporal.io/web-ui)

## 四、筛选、返回与空态的候选规则（未批准）

- **先对齐范围，再做筛选：** 标题明确「运行记录」或「Runs」，Map 只是筛选维度。状态与 Map 可作为优先讨论项；时间范围按真实记录规模决定，不默认引入复杂查询编辑器。
- **筛选状态可见且可清除：** 显示当前条件与匹配数量；从 Run 详情返回后保留筛选、选中 Run 与滚动位置是候选行为，不声称三家文档都已证明这种实现。
- **筛选与选中分离：** 用户改筛选时不隐式运行 Map、不重试 Run，也不篡改当前详情身份。若当前 Run 被过滤掉，明确仍在查看哪个 Run，而不是偷偷把详情切到第一条。
- **空态分开：** 「尚无运行」「无符合条件的运行」「读取失败」不能共用一个空列表；详情不存在、已不可访问或历史不再保留也不应冒充「从未运行」。参考中 Temporal 列表受保留期影响，n8n 可见记录受保存设置影响，删除 workflow 还会删除其 execution 历史，但这些并非本项目的已确认保留策略。[Temporal 保留期范围](https://docs.temporal.io/web-ui#workflows)、[n8n 保存与重试可用性](https://docs.n8n.io/build/understand-workflows/understand-executions/view-all-executions)
- **异常入口不改写状态：** 若数据能证明某 node 有问题，可给「查看异常」次级入口；没有异常结构化数据则保留正常详情路径，不靠猜测解析出一个假失败定位。

## 取舍结论

**最接近的参考不是流程编辑器首页，而是三者的 execution/run 视图。** 推荐讨论方向为：用 GitHub 的「运行 → 异常步骤」定位路径、n8n 的「历史实例与当前定义分离」、Temporal 的「状态与异常证据分层」组合成轻量运行记录界面。参考的是职责与交互边界，不是引入第三方组件库或照搬全部管理功能。

样式仍应复用本项目 Scheduler / Bot 管理的既有视觉语言；本笔记没有浏览本地应用，因此不对现有组件、间距、颜色或布局作核验结论。下一步访谈应在「紧凑运行列表」「需要横向比较时的轻表格」「列表—详情是否同屏」之间结合实际字段和使用场景取舍，不能仅凭市场参考直接宣布方案已定。
