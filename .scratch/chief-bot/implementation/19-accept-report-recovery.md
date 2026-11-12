# 19: 跨仓验收人工重发去重与崩溃恢复

**What to build:** 两宿主在报告投递失败、回执丢失和呈现中断后保留准确事实，仅由用户明确操作恢复，不重复业务或报告处理。

**Delivers:** P23, P24, P25, P26, P27, P34, P35

**Blocked by:** 17：交付持久接收去重与人工恢复接口；18：交付原生人工重发与中断后查看

**Repositories:** herdr-bot, my-agent-toolkit

**Status:** ready-for-agent

来源：[approved contract](../contract.md)、[spec](../spec.md)。

## Acceptance criteria

- [ ] P23 分别观察通知失败后、创建 chief、恢复 chief、重新登记、后端重启及时间流逝；窗口跨越实际维护/启动/确认预算，无重复通知或新业务报告。
- [ ] GPT-6-luna 在发送方收到明确重发请求，原生再次批准后向当前 chief 投递；拒绝不投递，同 ID/内容不变（P24）。
- [ ] 重复/并发重发、丢失回执、更换 chief、实际后端重启后可靠接收状态仍持久，最多触发一次处理，重发反馈已接收而非再次呈现（P25）。
- [ ] 可靠接收后、模型呈现前终止所属原生进程；保留接收但不宣称呈现/已读/验收，不自动重播（P26）。
- [ ] 上述中断后建立当前 chief，用户明确查看，真实读取原报告并呈现，无旧上下文迁移或新业务派发（P27）。
- [ ] 独立 Chat、直接 Map manager 分别新进程恢复后人工重发原报告；同 Task 无关联 Chat 仍拒绝，无新增委派/Run/初始输入重放（P34）。
- [ ] 保存后修改内容并沿用原 ID 被拒绝，或明确新报告取得新 ID/新确认；旧记录不变；拒绝新确认不发送（P35）。

## Coverage partition — 本票唯一验收所有权

|Promise|Coverage item（均分别覆盖 OMP/Pi）|交付票|测试所有者|
|---|---|---|---|
|P23|失败后/创建/恢复/重新登记/后端重启/时间流逝分别无补投|19|19|
|P24|发送方明确要求、再次原生批准/拒绝、当前 chief 与原 ID/内容|19|19|
|P25|重复/并发/回执丢失/更换 chief/实际后端重启后的持久去重|19|19|
|P26|接收后呈现前终止进程，保留接收且不自动重播|19|19|
|P27|新 chief 明确查看原报告，无上下文迁移或重新派发|19|19|
|P34|独立 Chat/直接 manager 各自恢复、原关联、无输入重放；无关联 Chat 拒绝|19|19|
|P35|同 ID 改内容拒绝或新 ID/新确认，原报告不变|19|19|

## 约束与证据

- 继承已批准 contract 的 S1/S2/S3、Implementation Decisions 与全部 Out of Scope，不重新设计 seam。公开操作的具体参数由实现确定，但必须与消费方同步，不留兼容空壳、假工具或假结果。
- 本票只拥有下述 coverage item 的测试；依赖票的永久用例可引用，不复制同一场景重新认领。受控响应仅证明协议/故障，不能证明真实模型职责。
- 所有实际模型调用，包括烟测，固定 GPT-6-luna，记录实际返回身份；不可用或身份不符即阻塞，不回退。测试驱动只提供用户输入/原生批准，不替模型选择业务工具或伪造结果。
- 使用测试独占定义、存储、目录、动态回环端口、HOME/APPDATA/XDG、原生会话和 Herdr 实例；不读取用户凭据/Chat，不操作正常服务或真实关机。明确授权的独立验收凭据只通过现有安全入口使用，不写入证据。
- 时间与无自动行为检查声明实际配置、启动/确认/维护预算及有界观察窗口；不得用即时断言、无限等待或新增产品等待/重试策略冒充证明。保留原始失败和清理失败；退出并确认所属进程结束，不以删除目录代替。
- 功能完成后更新对应仓库现有使用/测试说明或 changelog；记录命令、退出状态、源码/锁文件/产物/安装宿主身份及逐项证据。缺真实依赖或原生能力保持未验证，不能把计划内自动验收交给用户代办。

## 跨仓验收责任与运行入口

本票拥有 `recovery` 专项的实现、维护和证据，可修改上述两个仓库。它是普通可调度票，不是所有工作完成后的新增统一闸门。依赖在 first-merge 后满足，不等待依赖票后台集成结束。

本票须交付以下**新入口**（当前发布的是实现要求，不声称该命令已存在或通过），从 herdr-bot worktree 根运行：

```bash
python scripts/accept-chief-recovery.py --herdr-bot-worktree "$HERDR_BOT_WT" --toolkit-worktree "$TOOLKIT_WT" --backend "$HERDR_BOT_BINARY" --hosts omp,pi --paths-file "$ISOLATED_PATHS" --model-config "$GPT6_LUNA_CONFIG" --evidence-dir "$NEW_EVIDENCE_DIR"
```

- 两个 worktree、后端产物与已有安装依赖均为显式绝对路径；证据目录必须不存在。复用现有原生/浏览器/进程/真实模型传输设施，但不得借用其他专项已通过结论替代本票场景。
- **herdr-bot 分别通过：**在 `$HERDR_BOT_WT` 中运行 `npm --prefix frontend run build`，以独立绝对 CARGO_TARGET_DIR 构建 `cargo build --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-fixture --example safe_shutdown_shell`；再执行 `node scripts/test-backend.mjs`、`npm --prefix frontend test`、`npm --prefix frontend run test:browser:build`。
- **my-agent-toolkit 分别通过：**在 `$TOOLKIT_WT` 使用 Bun 1.4.4 运行 `bun run typecheck`、`bun run test`、`bun run check:integrations`，以及本票受影响的原生 dock 用例。使用参与 worktree 的实际源码/依赖，必要时显式提供既有 baseline 依赖定位，不更改 baseline。
- 配对 dock 的 HERDR_BOT_BINARY 必须指向本次安全关机壳，HERDR_BOT_METADATA 指向本次 worktree 元数据；真实启动专项的 `--backend` 指向本次生产后端。不能用生产程序替代安全壳执行关机回归。
- **专项分别通过：**OMP 与 Pi 各自全部 coverage item 有真实原生历史、工具输入/结果、确认交互、HTTP 与持久事实证据。全部实际模型请求使用 GPT-6-luna，记录返回身份；受控 provider 只能承载明确标注的机械/故障案例。
- 全局套件不能替代本票专项；若引用已有回归，须证明相关源码、夹具、依赖和环境未变并明确对应状态。源码变更后不能沿用旧结果。独占资源、顺序重型检查，不重试失败业务决策。
- 缺 worktree/构建/宿主/模型/确认能力、单宿主缺证据、原始验证或清理失败均非零退出，逐项保持未验证；fail-closed CLI 检查不算业务通过。
