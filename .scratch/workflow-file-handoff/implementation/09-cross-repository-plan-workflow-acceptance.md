# 09: 跨仓 plan_workflow 确认与 Map 生成验收

**What to build:** 精简后的 plan_workflow 在隔离实际宿主中完成用户需求→明确确认的九章节概要方案→新业务 Map。outline 登记命名输出，generate_map 直接消费地址，不解析日志找文件；既有人工确认、业务权限和不自动执行生成 Map 的边界保持不变。

**Delivers:** P27, P28。

**Blocked by:** 06: plan_workflow 改用命名文件交接；07: 跨仓角色工具、完成门禁与等待闭环验收。

**Repositories:** herdr-bot, toolkit

**Status:** ready-for-agent

## 跨仓 owner 与覆盖分区

依据 [contract](../contract.md) 与 [spec](../spec.md)，通过 S2/S3 并结合 S1 观察。该票可修改两个参与仓库，唯一拥有 P27/P28 的完整跨仓测试；06 只贡献迁移及定义可用性，不另认领真实 Chat 验收。无需等待 08，不增加后台集成专用最终门禁。

| Promise | 本票唯一拥有的覆盖项 | Test owner | 贡献票 |
|---|---|---|---|
| P27 | 精简定义在真实宿主完成 outline→generate_map；原生身份、初始化、工具、实际文件及显式推进全链路 | 09 | 06 业务迁移；07 配套角色链路 |
| P28 | 实际 Chat 明确用户确认、Task 九章节、能力权限、不自动执行新业务 Map；记录所有干预 | 09 | 06 保留业务规则；05 公共角色指引 |

## Acceptance criteria

- [ ] 使用参与 worktree 的真实迁移定义、后端及 toolkit，在独占配置和真实 Herdr／实际模型中发起新 Run；不接管用户 Run，不手改持久记录或伪造原生 Session 推进。
- [ ] outline 从实际初始化获得本次需求，产出独立概要方案，通过原生 report 登记命名输出并声明完成情况；generate_map 从解析输入获得并实际读取同一方案，不自行解析运行日志、筛报告、查时间或目录猜地址。
- [ ] 自动场景在实际 outline Chat 中输入明确、可记录的用户方案确认；manual 报告发送也有明确确认依据。确认前不宣称可交接；用户修改要求后需更新确认版本再交接。不能用 node 自述、can_complete=true 或期望文件证明用户批准。
- [ ] 对照启动前独立业务要求和既有 Task 的九章节约定核对实际概要方案，包含必要业务绑定、依赖、材料位置、权限和确认范围；不接受四标题草案替代九章节。
- [ ] 场景覆盖业务能力缺口的边界：明确缺口时不自动创建 Task／Bot 或调度能力创建者；只有实际用户补齐并核实后才继续，或保留真实阻塞而不冒充完成。为可完整交付的主成功场景预置真实合法能力，不能以不可恢复缺口替代整链路成功。
- [ ] manager 实际读取报告并显式 complete outline、dispatch generate_map，报告不自动推进；保留工具顺序、真实身份、执行引用与报告 ID。
- [ ] generate_map 保存新的业务 Map 后读回，并经已有服务 interface 验证可见及结构合法；方案内容、工作编号和实际 node 对应可独立核对。不覆盖同名不同内容，不修改自举 Map、Task／Bot 或运行记录。
- [ ] 只在用户明确提供新业务管理者配置时做只读预检；缺配置时如实报告，不挪用自举 Run 配置。生成的新业务 Map 未被自动启动，以原始工具／HTTP 及持久事实证明，不凭 node 自述。
- [ ] 保留完整原生对话、用户确认输入、实际初始化、工具请求／结果、概要方案及 Map 原始字节、服务读回、报告与完成记录，逐项映射 P27/P28；记录所有用户输入、协调、干预和失败。
- [ ] Full automation 无人工验收豁免。受控用户输入可用于确认，但受控 adapter／provider 预设调用不冒充真实宿主与模型行为；缺依赖、隔离失败或确认未发生时非零失败，不能跳过或降低业务要求。

## Worktree checks — 必须分别通过

1. **herdr-bot worktree：** 运行受影响的 Map／workflow 检查、`node scripts/test-backend.mjs` 和 `npm --prefix frontend test`。实际服务使用该 worktree 的 `cargo build --manifest-path backend/Cargo.toml --bin herdr-bot` 产物及独立 CARGO_TARGET_DIR；既有 workflow 页面数据仍来自本 Run，不新增页面动作。
2. **toolkit worktree：** `HERDR_BOT_REPO` 指向参与的 herdr-bot worktree，运行 `bun test tests/workflow-chat`、受影响的角色指引／node-report／manager dock 及 `bun run typecheck`。本票修改 toolkit 时运行 `bun run test`；未变化的完整回归可引用 07 相同源码／夹具／依赖／环境证据，不将单仓回归当作业务验收。
3. **专用真实业务场景：** 扩展 07 的可重复自动入口，运行 `python scripts/test-workflow-file-handoff.py --backend <参与herdr-bot worktree的后端可执行文件> --toolkit <参与toolkit worktree绝对根> --scenario plan-workflow --paths-file <隔离的依赖根配置>`。场景必须自动驱动真实 Chat 的用户确认、工具执行、保存／读回和证据检查，成功路径跑通后才通过，不能只生成定义或 runner 框架。

两个仓库分别记录命令、退出码、被测源码和环境，证据放本轮独立目录。只读复制运行记录，不重写修补。沿用真实宿主独占 HOME／原生存储／Herdr 实例和动态回环端口约束，持续消费输出并在成功／失败时清理所属进程，保留原错误和清理错误；不接触用户已有会话或擅自使用真实关机、LAN/mDNS。

## 文档与完成标准

同步更新两个仓库受影响的长期职责／业务运行说明及重跑命令；长期文档不依赖临时 evidence。完整 P27/P28 真实业务证据、所有确认及分别通过的仓库检查齐全才关闭本票，不以提示词精简、Map 文件存在、页面 completed 或替身成功替代。
