# 08: 跨仓共享文件输入与业务汇总验收

**What to build:** 在隔离实际宿主运行中，一个来源登记同一文件供两个消费者读取，各自产出后由专门业务汇总 node 消费。manager 只进行编排，框架只解析登记地址；manager zone、node zone 与业务产物目录分离，产物不被搬运到框架规定位置。

**Delivers:** P23, P24 的真实业务覆盖项。

**Blocked by:** 07: 跨仓角色工具、完成门禁与等待闭环验收。

**Repositories:** herdr-bot, toolkit

**Status:** ready-for-agent

## 跨仓 owner 与覆盖分区

依据 [contract](../contract.md) 与 [spec](../spec.md)，沿用 S1/S2/S3。该票可修改两个参与仓库，唯一拥有本场景的跨仓业务测试。无需等待 06 或 09，不引入额外最终验收槽位或后台集成门禁。

| Promise | 本票覆盖项 | Test owner | 其他覆盖归属 |
|---|---|---|---|
| P23 | 两个消费者实际读取同一登记输入、独立产出后由业务 node 实际消费／汇总；独立预期字节证明正确结果 | 08 | HTTP 解析及不解释内容：03 |
| P24 | 实际 manager/node/产物目录互不相同、不同 Run 资料位置区分、外部登记原地址被实际使用 | 08 | Run 位置初始化：01；登记原地址传递：03 |

## Acceptance criteria

- [ ] 真实 Herdr、角色入口和实际模型均来自本轮隔离部署；source→两个消费者→汇总的 Map 在运行前保存，全部派发由真实 manager 显式执行，不中途改运行记录补流程。
- [ ] 两消费者初始化引用同一个来源当前 execution 的命名输出。源文件正文仅存在于业务文件中，不通过初始化／报告把答案或完整正文传给消费者。
- [ ] 原生工具轨迹证明两个消费者都实际读取指定源地址，分别产出独立结果；汇总 node 实际读取两份登记结果后生成汇总文件。最终文件正确本身不足以证明读取过程。
- [ ] 启动前由验收方固定可区分的字面输入和独立预期字节；预期不由实际产物、模型报告或被测解析逻辑重算，不给消费者注入期望答案。
- [ ] 框架没有新增合并／转换／业务内容验收动作；业务需要的合并和必要内容检查由 Map 明确安排的 node 完成，manager 不默认读取结果正文验收。
- [ ] manager zone、各 node zone 和业务产物目录分离，至少两个隔离 Run 的中间资料位置可区分；zone 外文件直接登记，下游实际读取原地址，无复制／移动或强制落入 zone。
- [ ] 保留真实 Session／Run／execution、输入、report 登记、manager complete／dispatch 顺序、实际文件原始字节和读取轨迹，证据分别关联 P23 与 P24。
- [ ] 缺真实依赖或无法证明隔离时明确失败／阻塞，不以受控 Herdr、受控 provider、手填身份或准备文件冒充真实运行；所有人工／协调输入均记录。

## Worktree checks — 必须分别通过

1. **herdr-bot worktree：** `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch -- --nocapture`；`node scripts/test-backend.mjs`；`npm --prefix frontend test` 复用既有 workflow 数据回归，不新增页面动作或浏览器 seam。真实后端使用本 worktree 的 `cargo build --manifest-path backend/Cargo.toml --bin herdr-bot` 产物及独立 CARGO_TARGET_DIR。
2. **toolkit worktree：** `HERDR_BOT_REPO` 指向参与的 herdr-bot worktree，运行 `bun test tests/workflow-chat`、受影响 node-report／manager 派发与角色指引 dock，以及 `bun run typecheck`。若本票修改 toolkit，按其最终集成规范运行 `bun run test`；未改源码／夹具／依赖／环境时可引用 07 的有效完整回归证据，不能引用不同 checkout 的结果。
3. **专用跨仓业务场景：** 扩展 07 交付的自动入口，运行 `python scripts/test-workflow-file-handoff.py --backend <参与herdr-bot worktree的后端可执行文件> --toolkit <参与toolkit worktree绝对根> --scenario fan-in --paths-file <隔离的依赖根配置>`。该场景必须自动建立隔离定义、运行真实 Chat、验证读取和字节并输出证据；只跑全局套件不算通过。

各仓分别记录命令、退出码、源码状态及证据范围；已有结果仅在相应源码、夹具、依赖和环境未变化时复用。共享源码不边改边验收。沿用独占存储、动态数值回环端口、持续消费进程输出及所属资源清理；不访问用户 Chat、Run 或业务文件，不擅自启用真实关机、LAN/mDNS。

## 文档与完成标准

更新本场景的长期可重复运行说明，不把旧 fan-in 示例中“manager 默认读材料验收”的规则带回新职责，不把旧场景的成功直接冒充本次新登记协议通过。P23/P24 对应真实业务证据及分别通过的仓库检查齐全才关闭本票；09 的 plan_workflow 不在本票范围。
