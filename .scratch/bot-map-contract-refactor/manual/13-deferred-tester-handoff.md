# 13: 延期 tester：成果、嵌套、chief、脚本与恢复验收

**What to build:** 后续 tester 专项手动领取本票，补齐新公开成果、嵌套及 chief 交接、脚本和恢复的原生/黑盒覆盖，分别保留成功、拒绝、部分效果和未覆盖事实。

**Delivers:** P7, P12, P15, P16, P18, P19, P20, P21, P22, P23, P24, P25, P27, P28, P29, P34, P35, P37, P38, P39, P52, P56, P57, P59

**Blocked by:** 11 手动：两仓工具协议与角色接入联调

**Status:** ready-for-human

**执行方式：** 延期至 tester 专项，不纳入本轮 TDD 执行队列；仅手动选择执行。

**Target project:** `E:/Developer/myhub/herdr-bot-tester`（独立专项，不是当前自动仓库清单成员）。

## 权威与范围

来源：[批准契约](../contract.md)、[spec](../spec.md)。实施前读取适用的[结构权威](../lighthouse/02-definition-schema.md)、[调用/模块决定](../lighthouse/03-call-context-design.md)及[最终提示词正文](../lighthouse/04-coherent-prompts.md)，后续 Traverse 修订优先；迁移/切换另读[清单政策](../lighthouse/01-flow-inventory.md)、[复用方法](../lighthouse/05-reusable-migration.md)和[切换边界](../lighthouse/07-cutover-handoff.md)。不从早期候选或本票摘要重新创作业务语义。

测试前遵循所在仓库的测试约定。产品的新增行为采用公开 S1/S2/S3/S4 及独立字面预期/真实文件；不增加测试专用 seam，不用私有算法计算期望。源码路径不作为交付契约。

## Acceptance criteria

- [ ] 延期至 tester 专项，不纳入本轮 TDD 执行队列。目标项目为 herdr-bot-tester；开发票和 11 不等待本票。开始前操作者准备 tester 专项 worktree/分支、安装和逐项授权，不动用户现有修改。
- [ ] 复用嵌套 workflow/材料、成果批准与生命周期入口，拥有 P7/P12/P15/P16/P18–P21 的原生格：公开同名接口、冻结跨动作、finish 参数/拒绝、正式文件取自子 Run、父批准/报告/complete 独立、中途无文件和结束节点角色。
- [ ] 复用 accept-paired-chief-run 及相应 chief 场景，拥有 P22–P25/P28 的原生预览确认、Task/Map 文件交付、无文件失败/受阻和接替报告读取；不新增统一 success-finish 门禁，不以文字 deliverables 推导文件。
- [ ] P27/P29 原生 Run 和 chief 路径的保存/通知失败及部分效果分别取证，保持工具回执与实际文件/记录；不自动重投、重做业务或用新运行覆盖原始失败。
- [ ] P34/P35/P37 本票拥有嵌套双角色、chief 两路径、恢复、approve/report/finish/chief 工具实际加载/消费格。P52/P56/P57 仅适配确实受影响的既有生命周期/恢复格，核对保留历史不延续旧委派或旧 Run 权限；不扩建通用恢复平台。首次发起格归 12。
- [ ] P38/P39 复用 accept-rust-script-bots、check-revise-bot-guides 等受影响 case，验证实际 Bun、HTTP、revision、文件及部分效果；不借此迁移其他流程的业务语义。
- [ ] 两宿主及场景规定的完整组合分别满足原生/真实模型证据要求；受控 provider 只证明机制。缺授权、未跑格、过滤、宿主/清理失败不算通过，业务质量人工签收不能补缺。
- [ ] 场景 case 名、参数及独立预期在实施时核对并补齐，不能仅重跑旧场景。证据记录覆盖到 P 编号、源码/安装/产物来源、原生工具/确认/历史、公开父子/chief 状态、文件字节和退出清理，并回填 P59。

## 检查与证据

从 tester 专项 worktree 使用 `python "$TESTER/run.py" --herdr-bot-worktree "$BOT" <具名场景> --help` 核实真实参数，再按产品测试指引补齐绝对 `$BACKEND`、`$TOOLKIT`、安装路径、获授权模型声明及新证据目录。复用入口包括 `accept-paired-chief-run`、`accept-node-approval-report`、`accept-workflow-nesting-roles`、`accept-workflow-nesting-recovery`、`accept-rust-script-bots`、`check-revise-bot-guides`，按各自实际 coverage 选择并补齐新契约 case，不以名称相近推断已覆盖。

每个受影响场景的设施自测、bot 对应产品回归、toolkit 对应协议/dock 和原生结果分别归档。不得先运行全部旧专项来猜覆盖；按本票 case 分组逐项执行，失败保留、未运行明确列出。只使用拥有的进程/状态和安全壳，不授权真实关机、真实模型网络、用户认证或已有会话。

## 覆盖及测试所有权

[逐 promise 覆盖分区](../tickets.md#coverage-partition)中标为 13 的每个 case 由本票负责编写、维护和取证；不同证据层级不能互相补格。若一个 case 同时证明多个 promise，引用同一份证据，不复制测试。依赖票提供能力，不自动承担本票的验收 case。

本票的 Delivers 仅指所负责 coverage；有延期 tester、人工或操作核验的 promise 不能因本票完成就宣告整体通过。发现批准 seam/promise 不成立先修订契约，不通过缩小本票或弱化断言消除失败。
