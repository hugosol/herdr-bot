# 05: toolkit 新 report 工具与公共角色职责切换

**What to build:** OMP/Pi node-chat 通过既有 report 工具提交 body、必填 boolean can_complete 和可选 outputs，真实 Session 身份由公共能力自动传递。公共 node／manager 指引统一解释简短报告、阻塞、部分效果、显式完成及拒绝后等待，不再要求模型复制文件正文或默认由 manager 做内容验收。

**Delivers:** enabling: unblocks P7, P8, P17, P20, P21, P22, P26。

**Blocked by:** 04: 最新报告完成门禁与等待语义。

**Repositories:** toolkit

**Status:** ready-for-agent

## 契约与测试责任

依据协调仓已批准的 [contract](../contract.md) 与 [spec](../spec.md)，保持 S2 的既有角色工具、公共 Session／HTTP／Prompt interface。不增加第二套身份、私有 HTTP 客户端或工具名称。

本票必须迁移并维护受影响的既有参数、角色、公共请求及 dock 检查；新契约跨仓验收覆盖项的唯一 owner 为 07：P7 工具发现和参数矩阵、P8 原生 Session 注入、P17 显式工具顺序、P20 实际等待、P21/P22 指引运行行为、P26 配套实际调用。本票通过不表示这些跨仓承诺已完成，不把测试维护或破损调用方推给 07。

## Acceptance criteria

- [ ] OMP/Pi 的既有 report 参数和执行接入同步支持 body、严格 boolean can_complete、可选命名 outputs，模型不得提供运行身份；真实身份仍取公共 Session，后端仍决定实际归属。
- [ ] 缺 can_complete、字符串 false、错误 body／outputs 类型、额外身份字段明确拒绝，不能被宿主删字段或类型强转后意外执行；省略 outputs、true 和 false 均正确处理。保留既有 body 基本限制，不自行发明空正文拒绝。
- [ ] 工具结果保留后端保存／通知／诊断的分阶段信息；保存成功通知失败不被描述成回滚，不自主重报。沿用公共请求预算、工具取消及 Session 结束隔离。
- [ ] node 公共指引要求如实声明可交接性，报告结论、阻塞、部分效果、限制和必要干预；详细成果留在文件中，允许业务程序生成结果，不要求正文重复完整成果或框架已知身份。
- [ ] manager 公共指引要求阅读报告进行编排；true 只允许而不强迫 complete。拒绝后使用现有 collect 等待新报告，不反复对同一报告 complete，不默认读取成果正文做业务验收。
- [ ] 删除“节点不获得本次 userInput”“报告业务参数只有 body”“无报告人工完成”及默认 manager 内容验收等冲突说明；保留明确的 report_policy、真实用户确认、恢复／重试、部分效果和不自动派发边界。
- [ ] 同步工具示例、受影响消费者、既有测试／dock 和长期部署文档。按实际语义更新调用，不统一补 true、不保留旧形状新请求放行，不扩大到其他 Map 的业务迁移。
- [ ] 通过配套后端观察新 report 和门禁行为，不能只用 HTTP mock echo、schema 字段存在或指引文本断言证明工具可用；真实宿主契约验收归 07。

## Verification

在本票 toolkit worktree 使用仓库指定 Bun 版本。先在配套 herdr-bot worktree 构建其已有 dock 依赖：`cargo build --manifest-path backend/Cargo.toml --example safe_shutdown_shell --bin herdr-fixture`；toolkit 检查进程的 `HERDR_BOT_REPO` 必须指向该配套 worktree，不可使用另一个旧 checkout。

在 toolkit worktree 根执行 `bun test tests/workflow-chat`、受影响的 node-report、manager 状态动作、collect、角色指引及取消 dock，并运行 `bun run typecheck`。按该仓规范在完成时覆盖受影响消费者；独立跨仓验收 07 另运行完整回归。实际加载角色并观察新工具提交／拒绝作为冒烟，不以单纯类型检查代替运行。复用 dock 的隔离配置和安全壳，不操作用户会话。
