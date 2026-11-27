# 11: 手动：两仓工具协议与角色接入联调

**What to build:** 由同一联调所有者手动启动，使用 bot 与 toolkit 各自 worktree 的实际工具入口，确认新请求、真实后端拒绝/回执、角色恢复和文件交付互相匹配；本票不进入自动开发槽位，也不是后台集成结束后的新最终关卡。

**Delivers:** P7, P9, P15, P16, P18, P19, P20, P21, P22, P23, P24, P25, P26, P27, P28, P29, P33, P34, P35, P37, P59

**Blocked by:** 06 公开成果 finish 与父子 Run 显式交接; 07 chief 委派、结构化报告与接替读取; 09 OMP/Pi 管理者与节点工具适配; 10 OMP/Pi chief 委派与报告工具适配

**Repositories:** bot, toolkit

**Status:** ready-for-human

**执行方式：** 手动启动；不进入本轮 Hyper-Auto-Coding 自动开发队列。

## 权威与范围

来源：[批准契约](../contract.md)、[spec](../spec.md)。实施前读取适用的[结构权威](../lighthouse/02-definition-schema.md)、[调用/模块决定](../lighthouse/03-call-context-design.md)及[最终提示词正文](../lighthouse/04-coherent-prompts.md)，后续 Traverse 修订优先；迁移/切换另读[清单政策](../lighthouse/01-flow-inventory.md)、[复用方法](../lighthouse/05-reusable-migration.md)和[切换边界](../lighthouse/07-cutover-handoff.md)。不从早期候选或本票摘要重新创作业务语义。

测试前遵循所在仓库的测试约定。产品的新增行为采用公开 S1/S2/S3/S4 及独立字面预期/真实文件；不增加测试专用 seam，不用私有算法计算期望。源码路径不作为交付契约。

## Acceptance criteria

- [ ] 本票是显式跨仓 integration/acceptance 所有者，可修改列出的 bot、toolkit 两仓；06/07/09/10 是贡献开发票，按其首次合并满足依赖，不等待背景集成结束。仅在人工选择本票后执行。
- [ ] 唯一拥有两仓接缝 case：实际工具发出的 prepare_inputs/finish/report/chief_delegate/chief_report 请求，进入对应 bot HTTP 后读回状态、持久报告及真实文件；覆盖新参数、无文件中途报告、子集来源和拒绝后的外部事实，不重写产品票的全部内部/HTTP case。
- [ ] 分别观察批准、报告保存、通知、complete、finish；验证双层角色、结束节点与恢复关联。manager 与独立 chief-report 路径、OMP/Pi 均接入批准政策，工具不能绕过真实身份、原生批准机制或后端拒绝。
- [ ] 可复用已有受控 provider/adapter 取得确定性跨仓证据，不新增转发 seam。受控接入结果不证明安装版 agent 实际加载，也不替代 12/13 的原生或真实模型场景。
- [ ] 两仓各自的必需定向检查分别通过且保留独立退出码/日志；任何缺格、权限或清理失败如实保留，不能用一个仓的全局绿替另一仓，也不以安装了工具包或已有旧证据宣称通过。
- [ ] 证据标注每项 P 编号、来源 worktree/revision/修改及锁文件、后端/安全壳哈希、HTTP/工具原始请求回执、角色、文件字节、所拥有进程与退出清理。把本票结果回填 08 准备的交付报告；12/13 仍明确延期。

## 检查与证据

先为本票设置绝对 `$BOT`、`$TOOLKIT`、独占绝对 `CARGO_TARGET_DIR` 和新证据目录；所有命令分别从对应 worktree 根运行，不能误用协调仓旧构建。

- bot：`cargo test --manifest-path backend/Cargo.toml --test chat_creation --test workflow_start --test workflow_dispatch --test chief_delegation`，用各开发票记录的精确选择器运行所涉及的输入、finish、nested_completion、角色恢复、chief 要求/报告 case。覆盖涉及的交付页面时执行 `npm --prefix frontend run test:browser:build -- workflowCurrent.spec.ts workflowRefresh.spec.ts --workers=1`；保留实际页面与 HTTP 读回。开发票拥有的其他检查引用其同版本证据，不假定一个全套退出码代表接缝已验证。
- toolkit：`bun run typecheck`、`bun run check:integrations`、`bun test tests/workflow-chat`，以及受影响具名 dock（至少覆盖实际 node report 和 manager/chief 新协议入口）。原有具名 dock 可用 `bun test tests/harness-dock/native-node-report.test.ts`；新接缝 case 沿用现有 dock 分区并在本票证据中记录精确命令。OMP/Pi 各入口单独取证，不用一个宿主补另一宿主。
- 先从本票 bot worktree 构建安全壳；关机相关 dock 的 `HERDR_BOT_BINARY` 必须指向该独占构建目录的 `debug/examples/safe_shutdown_shell.exe`，`HERDR_BOT_METADATA` 指向 `$BOT` 的交付 Task 选项文件。生产后端只接本票隔离业务实例，不用于真实关机。按既有安装/依赖约定准备 frozen 依赖；缺依赖不补成通过。

以上是各仓检查；另须执行本票新增的跨仓接缝 case，以实际工具入口、公开读回和文件字节证明各条承诺。全仓回归或工具包设施自测不是它们的替代。

## 覆盖及测试所有权

[逐 promise 覆盖分区](../tickets.md#coverage-partition)中标为 11 的每个 case 由本票负责编写、维护和取证；不同证据层级不能互相补格。若一个 case 同时证明多个 promise，引用同一份证据，不复制测试。依赖票提供能力，不自动承担本票的验收 case。

本票的 Delivers 仅指所负责 coverage；有延期 tester、人工或操作核验的 promise 不能因本票完成就宣告整体通过。发现批准 seam/promise 不成立先修订契约，不通过缩小本票或弱化断言消除失败。
