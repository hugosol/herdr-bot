# 14: 发布 V1 的双宿主真实创建与保存闭环验收

**What to build:** 发布 V1 使用新材料真实创建保存 Bot，普通节点按成果职责批准，脚本不索要人类凭据；其他非 V2 发布节点也按职责配置和交接。

**Delivers:** P31 的发布生产者／S2 分区；P34 的 V1 及其他非 V2 发布节点 S2／S3 分区；P35 的 V1 双宿主分区

**Test owner for:** P31、P34、P35（仅下列 Coverage ownership 中本票分区；承诺整体由全部分区共同完成）

**Blocked by:** 05：OMP／Pi 成果批准工具与自由报告；06：Map 配置与 Run 批准事实页面；09：按成果职责迁移发布定义，保持 V1／V2 独立

**Repositories:** herdr-bot, toolkit

**Status:** ready-for-agent

## Acceptance criteria

- [ ] 从 09 发布定义的独占副本发起 OMP／Pi 两套真实 V1 Run，保留节点真实 Suite／宿主／业务职责，只隔离位置、传输与存储；脚本仍为真实 Bun，无假 Chat 或模型代执行（P31、P35 V1）。
- [ ] 真实模型生产新 request、基线及明确绑定正文，不填 authorization、create_blank、write_materials、save_guides 或其他人类批准凭据，不索要下游脚本许可；保存不依赖三份验权交接书，不转换旧协议（P31）。
- [ ] 两宿主实际完成创建、三字段保存、报告、批准、manager 验收与显式 complete；读取完整配置／revision、实际文件和 Run 身份，对比独立字面预期，不以成功文字或 Schema 通过代替闭环（P35）。
- [ ] 消费 09 逐节点映射，覆盖 V1 及所有其他非 V2 发布普通节点的批准职责；真实执行覆盖人工／自动，脚本均自动。每项 coverage 具名，不能抽样冒充未覆盖节点（P34 S2）。
- [ ] 真实打包页面读取该分区发布配置，与独立公开定义、冻结模式及批准来源交叉核对；不复制 06 的通用控件用例（P34 S3）。
- [ ] V1 不调用或替换 V2，V2 及无关定义不被改写；独立定义／Run 的 S1 组合比较归 09，V2 原生用例归 15（P35）。
- [ ] 保存真实业务判断、模型身份、原生确认与工具、公开 JSON、文件字节、页面和清理证据，不自动部署用户环境或重跑历史副作用。

## Coverage ownership

| Promise | 本票唯一拥有的测试覆盖项 | Test owner |
|---|---|---|
| P31 | S2 双宿主真实模型 V1 材料生产及脚本执行 | 14 |
| P34 | V1 及其他非 V2 发布节点 S2 职责执行与 S3 配置消费 | 14 |
| P35 | S2 双宿主 V1 真实闭环 | 14 |

## 验证与交付边界

来源为已批准 contract 与 spec；按本票 Coverage ownership 认领测试，其余票的同名承诺分区不得重复认领或缩减。独立预期来自契约与字面业务样例，不调用被测实现计算预期。S1 为公开 HTTP／定义与真实业务效果，S2 为实际 OMP／Pi 工具，S3 为真实浏览器；私有模块不升级为新 seam。

先实现本票行为及必要回归，跳过中途完整 build／lint／test／formatter；完成后一次执行本票所需检查。已有测试覆盖的路径更新有效契约；只增加能捕获消费者可见错误的用例，删除旧机制专属断言，保留仍有效的保存、通知失败、身份隔离、输出和门禁职责。同步更新实际受影响的长期文档／变更记录。

资源必须独占：定义、数据目录、原生会话、HOME／APPDATA／XDG、动态数值回环端口、进程和证据目录。不使用用户历史 Chat／Run，不读取未授权凭据、不安装宿主、不执行真实关机。原始失败与清理失败分别保留；确认所属进程退出后才能清理。无旧 Run 兼容／升级、历史改写、批准通知、批准说明／产物前提、自动重试或隐式推进。承诺缺真实依赖／授权时明确未验证，不转交人工、不用受控成功顶替。

## 跨仓所有权与必需检查

本票为跨仓联调／验收票，owner 可修改所列两个仓库；拥有本票专项入口与覆盖用例，不只是收集其他票的日志。它与其他票共享普通实现槽位，按 blockers 首次合入即可调度；不等待它们的后台复盘，也不是新的统一最终 gate。

1. 在参与的 herdr-bot worktree 设置独立绝对 `CARGO_TARGET_DIR`，依次运行 `npm --prefix frontend run build`、`cargo build --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-fixture --bin herdr-paths --example safe_shutdown_shell`、`node scripts/test-backend.mjs`、`npm --prefix frontend test`、`npm --prefix frontend run test:browser:build`。
2. 在参与的 toolkit worktree 使用已有 Bun 1.4.4／frozen-lock 依赖，依次运行 `bun run typecheck`、`bun run test`、`bun run check:integrations` 及受影响具名原生 dock。配对 dock 的 `HERDR_BOT_BINARY` 是本次安全壳，`HERDR_BOT_METADATA` 是参与 worktree 元数据；生产后端只用于隔离业务专项，不能代安全壳运行关机回归。
3. 本票实现或更新独立专项入口，并交付以下可复跑命令。从参与的 herdr-bot worktree 根运行 `python scripts/accept-node-approval-v1.py --herdr-bot-worktree "$HERDR_BOT_WT" --toolkit-worktree "$TOOLKIT_WT" --backend "$HERDR_BOT_BINARY" --hosts omp,pi --paths-file "$ISOLATED_PATHS" --model-config "$AUTHORIZED_MODEL_CONFIG" --evidence-dir "$NEW_EVIDENCE_DIR"`。这些变量分别为两个实际票据 worktree、该 worktree 生产后端绝对路径、显式隔离依赖配置、显式授权模型配置及不存在的新证据目录；不得绑定协调仓旧产物或正常服务。入口复用已有验收设施，不新建产品 CLI／外部 seam；两仓源码、锁文件、产物及宿主安装身份都进入证据。
4. 专项必须逐项输出本票 coverage 的原始证据、独立预期和通过／未验证结果。任一仓检查失败、任一宿主或 coverage 缺失、实际模型／授权缺失或清理失败均不能标整票完成；退出非零，保留原始错误。受控机械故障明确标注，不混称真实模型业务。全局 suite、CLI fail-closed、单宿主成功或旧源码证据不替代本票专项。

确认与无自动行为观察使用实际产品现有预算，记录配置与单调时钟窗口及理由，不改产品超时或重试策略。真实界面选择可自动驱动本票拥有的独占终端，但必须观察选中值、原生返回和公开事实，不能把按键写入成功当作 Yes。
