# 11: 完整批次串行与不同 Run 独立推进

**What to build:** 同 Run 的状态操作等待整个派发批次完成，pause 不中途撤回目标；不同 Run 和已启动 Chat 继续工作。

**Delivers:** P17, P21, P36

**Test owner for:** P17.03, P36.02（仅本票所列分区；整个 promise 须全部分区完成）。

**Blocked by:** 09 — 暂停新执行与恢复派发资格

**Status:** in-progress

## 契约与范围

依据：[approved contract](../contract.md) 与其继承的 [spec](../spec.md)。P 编号与 S1–S4 的含义以 contract 为准；本票不重新定义 promise 或新增观察 seam。

完善并验证业务串行边界，不以文件锁代替。动作结果和快照在同一边界内生成，后续动作重检现场。不得用任意 sleep 推断并发次序；以受控事件门闩与可观察外部调用证明顺序。

仅交付本票的覆盖分区；共享 promise 不因某一分区通过就算整体完成。实现前沿是所有列出的阻塞票均完成的票，不按编号强制全串行。

## 验收标准

- [x] **P17.03**（本票编写并执行的契约检查）：并发变化后后续动作重新检查当前事实，动作结果及快照来自同一业务串行边界，不用旧快照授权。
- [x] **P21.01**（本票编写并执行的契约检查）：受控延迟的完整派发批次与同 Run pause、另一 Run 调用并发；以事件顺序证明同 Run 串行、独立 Run 与既有 Chat 不受阻，分别观察业务串行与文件等待，不用任意 sleep 推断。
- [x] **P36.02**（本票编写并执行的契约检查）：pause 排在完整在途批次后生效，不中途撤回已启动目标。
- [x] 完成上述能力所需调用者、错误反馈和角色/用户指引；不留占位实现、假成功、运行时旧格式兼容或未接线入口。
- [x] 保留本票真实入口的可重复验证证据及检查结果；相关长期说明随能力更新，不只更新临时票据。

## 验证与证据

延迟一个真实 HTTP 派发批次，同时提交 pause 和另一 Run 的请求；证明完整批次先结束、pause 后生效，另一个 Run 及既有 Chat 未被全局串行阻塞。

遵守仓库测试指引：复用真实 HTTP TestServer、现有前端/npm 浏览器入口及 toolkit 公共 interface；页面能力观察实际交付页面，宿主能力需要真实 Herdr/omp 证据。受控 adapter 只证明确定性故障/时序，不证明模型或原生宿主行为。

普通检查只用数值回环、独占临时数据/cwd/动态端口与必要 Herdr 副本，持续消费并清理所属子进程；不操作用户对话或产物，不绕过 npm 网络护栏。LAN/mDNS、真实符号链接和关机仍遵守显式人工授权规则。

自动化预期取自契约、独立字面样例或预先算定结果，不用被测实现重算预期。负向自动行为检查注明有界观察窗口并保留外部调用证据；取消/错误不得伪造快照或自动重试。真实环境缺失如实报告阻塞，不以替身或跳过宣称通过。

## 共享 promise 的 coverage partition

下面列出本票涉及的共享 promise 的完整分区。每个分区只有一个交付票和测试 owner；其他票的行是依赖说明，不是本票重复认领的验收项。各分区最终均通过后 promise 才完成。

| Promise / coverage item | Coverage | Ticket | Test owner |
| --- | --- | --- | --- |
| P17.01 | 基础 status 经真实 manager 工具返回与持久事实一致的节点状态及快照，不持久化第二套 ready 状态。 | 04 | 04 |
| P17.02 | 独立小图列出完成前后可派发项、阻塞原因、结束条件的字面预期，查询和动作快照一致。 | 07 | 07 |
| P17.03 | 并发变化后后续动作重新检查当前事实，动作结果及快照来自同一业务串行边界，不用旧快照授权。 | 11 | 11 |
| P36.01 | pause 生效后 dispatch 拒绝，但既有 Chat 可 report/complete，不要求退出。 | 09 | 09 |
| P36.02 | pause 排在完整在途批次后生效，不中途撤回已启动目标。 | 11 | 11 |
| P36.03 | paused 时 retry 明确拒绝且无新 Chat；必须先显式 resume。 | 13 | 13 |

## Comments

本票实现与所拥有的确定性检查已完成，待独立集成；按仓库规则合入后才能标记 closed。已有运行实现已持有完整批次业务锁、在锁内重读事实并生成动作快照，因此保留该实现，不为本票重复引入锁或新的运行 seam。

- P17.03：新增 `serialized_dispatch_rechecks_pending_snapshot_after_in_flight_launch`；外部初始输入门闩期间 pending/ready 的同目标请求，在释放后必须拒绝为 running，动作快照一致且没有第二次宿主启动。定向命令 `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch serialized_dispatch_rechecks_pending_snapshot_after_in_flight_launch -- --exact --nocapture` 通过。临时注入“使用排队前 Run、不在锁内重读”后在字面拒绝断言失败；恢复后通过，故障代码未保留。
- P21.01/P36.02：新增 `serialized_batch_finishes_before_pause_without_blocking_other_run_or_chat`；A/B 两道外部门闩下 pause 分别无响应，同 Run 文件查询均成功，独立 Run 实际启动 B、既有 Chat 的 working 上报经真实 SSE 可见；历史严格为 dispatch A → dispatch B → pause，批次 running 与后续 paused 快照均保留两项执行。定向同名 `--exact --nocapture` 命令通过。临时把业务锁改为逐项目标持有，在“pause must wait for the FULL batch”断言失败；恢复后全量 workflow suite 通过。负向 HTTP 窗口每道门闩 200ms，不用 sleep 定序。
- 更新长期 `docs/workflow-runtime.md`：完整批次/只读文件观察/不同 Run/Chat 活动/业务写入排队的区别、角色/用户处置与复验命令；新增测试 helper 仅为已发出真实 TCP 请求读取结果，不新增产品接口。
- `npm --prefix frontend ci --no-audit --no-fund`、`npm --prefix frontend run build` 通过（第一次 build 因尚未安装 tsc 失败，安装依赖后解决）。全量后端首次因未构建静态页在 herdr_config 失败；补齐前端构建后重跑 `node scripts/test-backend.mjs --binary-jobs 2`：60 个 binary 通过、1 个失败，0 未派发；`workflow_dispatch` **21/21 通过**。唯一失败是范围外 `zone_sync::configured_timeout_stops_the_running_command_and_keeps_effective_pull_state`，预期 200 实得 409 git-write-in-progress；doctest 未运行，不宣称全后端通过，也未修改/重跑该范围外失败。
- 实际日志：`C:/Users/hugos/AppData/Local/Temp/herdr-backend-tests-mcVKEA/summary.json`；本票 suite `binary-050.stdout.log`；范围外失败 `binary-060.stdout.log`。定向 `--nocapture` 输出含实际调用、查询、历史和动作结果，可用长期说明中的 `serialized_` 命令重新生成。
- 证据边界：HTTP/SSE、文件和生产 Herdr 调用包装层真实；外部 Herdr 是隔离记录型副本，Chat 事件为受控输入。本票不声称原生 omp/真实角色工具或 Prompt 注入已验证；共享 promise 的其他分区不由本票宣称完成。
