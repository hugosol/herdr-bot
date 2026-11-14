# 11: 跨仓验证两能力闭环并准备真实 manager H1/H2 验收

**What to build:** 使用参与 worktree 构建的隔离后端、真实交付 Map 与 toolkit 原生工具完成两种 Rust 节点的组合验收；完成全部自动化后，将可重复环境、步骤和完整证据交给用户确认 H1/H2，不将计划人工项冒充已通过。

**Delivers:** P2, P3, P5, P10, P12, P13, P14, P23, P27, P29, P34, P35（只拥有本票列明的集成/人工分区）

**Blocked by:** 04 — 保持脚本执行的并发、断连、暂停和重启语义；06 — 保持脚本报告保存与通知失败隔离；07 — 迁移创建 Bot 流程并保护修订分支；08 — 在既有 Bot 和 scheduler 页面维护及预检脚本目标；09 — 在 workflow 展示真实脚本执行与待交接状态；10 — 适配 manager 原生工具指引与脚本结果呈现。

**Repositories:** herdr-bot, toolkit

**Status:** ready-for-agent

## Acceptance criteria

- [ ] 作为跨仓 integration/acceptance owner，可修改本票列出的两仓，保留一个全局 ticket 序列及 first-merge 依赖调度。本票是普通实施槽位，不等待依赖票后台集成全部结束、不增加全局最终 gate。不得将依赖票的自动化或未实现行为转交人工。
- [ ] 提供并执行可重复跨仓自动化验收入口，从显式 herdr-bot worktree 根运行，明确接收 herdr-bot/toolkit worktree、当前后端/静态页/原生入口、隔离配置和新证据目录；拒绝不匹配产物或覆盖旧证据。完成时文档记录准确调用命令、参数、环境和各覆盖项结果，不留下待实现入口。
- [ ] 实际运行本次构建后端中的两个 Rust 业务能力，保留源码/锁文件/页面/二进制/安装入口身份与实际业务执行证据，证明无需运行时解释器或独立业务 exe。不能以源码声明、函数可调用或 stub 宿主业务写入代替 P2。
- [ ] 自动化通过真实隔离后端、真实交付 Map 节点完成创建、三路正文材料消费、保存和下游消费，独立读回真实完整 Bot/目标、revision、baseline/saved/handoff、execution、outputs、报告及 complete 记录。不将直接调用业务函数视为 P29。
- [ ] 跨仓专项用生产 adapter/原有 manager 工具验证实际脚本结果可透传、通知与 collect 兼容、complete 门禁及失败结果可消费。受控外部响应只证明协议/机械故障，不证明真实 manager 理解；不新增脚本接收工具或伪造脚本 Chat。
- [ ] 在实际浏览器打开迁移后的空白创建 Bot 和新专用保存 Bot，核对空指南、Rust 目标及创建/修订 Map 各自引用，并以公开完整定义读回对照，拥有 P23 的迁移页面分区。
- [ ] 执行 03 拥有的 P15 用例，并在 05 通路上补充 H2 组合演示：保存后材料失败、原效果读回、显式 fail/retry、原 baseline 冲突。该演示不复制认领 03 的业务测试或 05 的通用门禁测试。
- [ ] H1/P34：准备独占真实宿主/manager 会话、明确授权边界、操作步骤、预期派发返回/真实报告接收/collect/显式 complete 结果，以及构建身份、配置/文件/报告证据和待收集原生工具记录。覆盖两能力，并观察无脚本 Chat/Tab；自动化先完成，最终由用户实际操作或指导真实 manager 确认。
- [ ] H2/P35：准备可重复隔离业务失败、修复及显式 fail/retry 操作，预期新旧 execution/报告/输出和旧报告不得完成新执行的对照，保留真实工具结果。用户最终确认真实 manager 对失败结果和重跑的理解；不能用受控 adapter 成功代替。
- [ ] 不把脚本自动报告的确定性检查重新转为模型语义验收。缺少真实宿主/授权时明确列出阻塞、已完成自动化及已准备证据；H1/H2 保持未验证，不能阻断仍可执行的实现、自动化和证据准备。
- [ ] 更新两仓已有运行/验收文档，记录精确入口、每仓独立结果、清理结果及 remaining human checks。保留原始失败，不用重跑成功掩盖或以全局 suite 替代专项。

## Required checks per repository

所有命令从对应参与 worktree 根执行，依赖按各自 frozen lock 准备。每个仓单独记录命令、退出状态、来源/构建身份；任一仓失败不能宣称跨仓自动化通过。

### herdr-bot worktree

1. 设置当前 worktree 独立绝对 CARGO_TARGET_DIR。
2. `npm --prefix frontend run build`。
3. `cargo build --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-fixture --bin herdr-paths --example safe_shutdown_shell`。
4. `node scripts/test-backend.mjs`。
5. `npm --prefix frontend test`。
6. `npm --prefix frontend run test:browser:build`。
7. 运行本票实际交付的跨仓专项入口，显式绑定两个 worktree 和上述产物，逐项输出 P2/P23/P27/P29 自动化证据、H1/H2 待确认材料；依赖票专项按其已有所有权执行，不重复编写同一测试。

### toolkit worktree

1. 使用 Bun 1.4.4，分别执行 `bun run typecheck`、`bun run test`、`bun run check:integrations`。
2. 运行受影响的 OMP/Pi manager 原生协议专项，精确命令及其覆盖由 10 交付并在本票验收记录中展开，不能仅记录“dock 通过”。
3. 配对 dock 的 HERDR_BOT_BINARY 指向本次 herdr-bot worktree 构建的 safe_shutdown_shell，HERDR_BOT_METADATA 指向该 worktree 元数据；真实业务验收入口使用生产 herdr-bot 后端，不以生产后端执行关机回归。

## Cross-ticket coverage partition

下表的“owner”指该覆盖项的测试编写/证据准备责任。H1/H2 的最终确认人始终是用户；同一 promise 分区全部满足后才完整完成，不用其他票通过推断本分区通过。

| Promise | Coverage item | Delivery ticket | Test/evidence owner |
| --- | --- | --- | --- |
| P3 | 受控业务在途时派发已返回 | 02 | 02 |
| P3 | 真实 manager 派发 H1 | 11 | 11 准备；用户确认 |
| P5 | 后端身份、外部无节点 Chat/Tab 调用 | 02 | 02 |
| P5 | workflow 无虚假身份/Chat 恢复入口 | 09 | 09 |
| P5 | 真实宿主无脚本 Chat/Tab H1 | 11 | 11 准备；用户确认 |
| P7 | 创建请求非法格式/缺项/冲突/歧义 | 02 | 02 |
| P7 | 合并保存材料非法格式/缺项/冲突/歧义 | 03 | 03 |
| P8 | 创建成功与写入/读回失败 | 02 | 02 |
| P8 | 合并保存成功与写入/读回失败 | 03 | 03 |
| P10 | 保存、通知及 collect 自动化 | 02 | 02 |
| P10 | 真实 manager 接收 H1 | 11 | 11 准备；用户确认 |
| P12 | 当前 true 报告和显式 complete 门禁 | 02 | 02 |
| P12 | 真实 manager 交接 H1 | 11 | 11 准备；用户确认 |
| P13 | 失败报告和 fail/retry 门禁 | 05 | 05 |
| P13 | 真实 manager 失败处理 H2 | 11 | 11 准备；用户确认 |
| P14 | recover 拒绝、新旧执行/输出保留 | 05 | 05 |
| P14 | 真实修复和独立重跑 H2 | 11 | 11 准备；用户确认 |
| P23 | 专用 Bot、空指南和两个流程引用 | 07 | 07 |
| P23 | 迁移定义在实际页面读回 | 11 | 11 |
| P27 | 创建分支格式/拒绝/显式接线和 S1 材料闭环 | 07 | 07 |
| P27 | 跨仓原生工具组合场景 | 11 | 11 |
| P29 | 隔离后端实际 Map/文件/报告/完成记录自动化 | 11 | 11 |
| P29 | 真实 manager 工具端 H1 | 11 | 11 准备；用户确认 |
| P32 | 后端自动报告、无 manual/关闭调用 | 02 | 02 |
| P32 | 定义维护页面约束 | 08 | 08 |
| P32 | 运行页面自动报告/无关闭与等待入口 | 09 | 09 |
| P33 | 创建能力运行禁写 | 02 | 02 |
| P33 | 合并保存能力运行禁写 | 03 | 03 |

其余 promise 单票所有权：P1/P6/P9/P20/P21 → 02；P15/P24/P25/P26 → 03；P4/P16/P17/P18 → 04；P19 → 05；P11 → 06；P22/P28 → 07；P30 → 08；P31 → 09；P2 → 11；P34/H1、P35/H2 → 11 准备、用户确认。01 与 10 只 enabling，不宣称 promise 验收通过。

## Isolation and evidence requirements

各场景独占定义、Run、zone、材料、HOME/APPDATA/XDG、原生状态和数值回环服务；不访问正常服务、用户 Chat 或自动发现认证。只消费显式授权的真实宿主/模型配置，不新增模型、平台或宿主矩阵要求。安全终止并持有/等待所属进程确认退出后再清理，持续消费日志，原始错误和清理错误分别保留。

证据至少包含每项 promise 的用例/步骤、独立字面预期、原始 HTTP/原生工具结果、实际完整配置与真实 revision、输入/输出文件内容和绑定、当前/旧执行及报告、完成记录、页面截图、构建与宿主身份、观察预算和进程退出确认。控制型 adapter、全局回归、单个函数或配置声明均不能替代真实 S2 H1/H2。
