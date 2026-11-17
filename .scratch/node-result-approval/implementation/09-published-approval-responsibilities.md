# 09: 按成果职责迁移发布定义，保持 V1／V2 独立

**What to build:** 发布 Map 按每项实际成果确认职责采用批准模式，V1 使用精简后的创建材料，V2 保持原业务协议，两套流程仍能分别发起和交接。

**Delivers:** P34、P35 的发布定义／S1 分区；P17、P31、P38 的发布清理分区

**Test owner for:** P17、P31、P34、P35、P38（仅下列 Coverage ownership 中本票分区；承诺整体由全部分区共同完成）

**Blocked by:** 03：子 manager 独立批准父节点整体成果；08：V1 仅凭执行必需材料保存三份指南

**Repositories:** herdr-bot

**Status:** ready-for-agent

## Acceptance criteria

- [ ] 逐项清点全部发布普通 node，记录业务职责、是否需要用户确认本节点成果、最终 auto_approved 值及理由；旧 manual/write_materials 仅作查找线索，不机械映射。所有脚本自动，结束节点不适用（P34）。
- [ ] 同步更新发布 Map／Task／Bot 的生产者指引、输入要求和显式材料接线；V1 不再索要 authorization、下游许可或三份验权交接书，业务讨论和真正的成果把关保留（P31）。
- [ ] V1／V2 保持各自入口、真实定义和独立 Run；不删除、替换或相互调用以冒充独立，不重写 V2 专用材料协议、不把其旧业务字段当通用 approved（P35）。
- [ ] 使用实际发布定义的独占副本，经公开操作与真实 Bun／文件完成 V1 创建保存闭环；V2 普通节点批准、报告和显式完成单独验证，保留独立输入及完整读回（P35 S1）。
- [ ] 删除发布定义中的旧 node report 审批开关及对应专属断言；保留报告保存、通知失败、材料交接、输出及完成门禁回归（P17、P38）。
- [ ] 配置映射以可读覆盖清单随本票证据输出，列出 V1、V2 和其他发布流程各 node 的配置依据；双宿主实际人工／自动职责及发布配置页面消费分别由 14、15 拥有。同步更新发布与业务说明。
- [ ] 不删除旧产物、历史 Run／Chat，不改写用户历史或自动部署重跑，不为存量旧 Run 增加适配层。

## Coverage ownership

| Promise | 本票唯一拥有的测试覆盖项 | Test owner |
|---|---|---|
| P17 | 发布定义删除旧报告开关 | 09 |
| P31 | 发布材料生产者清理下游授权要求 | 09 |
| P34 | 发布全部节点逐项业务依据、配置映射与 S1 读回 | 09 |
| P35 | S1 V1／V2 独立定义、独立 Run 组合比较及真实脚本闭环 | 09 |
| P38 | 发布定义旧报告策略断言 | 09 |

## 验证与交付边界

来源为已批准 contract 与 spec；按本票 Coverage ownership 认领测试，其余票的同名承诺分区不得重复认领或缩减。独立预期来自契约与字面业务样例，不调用被测实现计算预期。S1 为公开 HTTP／定义与真实业务效果，S2 为实际 OMP／Pi 工具，S3 为真实浏览器；私有模块不升级为新 seam。

先实现本票行为及必要回归，跳过中途完整 build／lint／test／formatter；完成后一次执行本票所需检查。已有测试覆盖的路径更新有效契约；只增加能捕获消费者可见错误的用例，删除旧机制专属断言，保留仍有效的保存、通知失败、身份隔离、输出和门禁职责。同步更新实际受影响的长期文档／变更记录。

资源必须独占：定义、数据目录、原生会话、HOME／APPDATA／XDG、动态数值回环端口、进程和证据目录。不使用用户历史 Chat／Run，不读取未授权凭据、不安装宿主、不执行真实关机。原始失败与清理失败分别保留；确认所属进程退出后才能清理。无旧 Run 兼容／升级、历史改写、批准通知、批准说明／产物前提、自动重试或隐式推进。承诺缺真实依赖／授权时明确未验证，不转交人工、不用受控成功顶替。

## 本仓检查

从本票 herdr-bot worktree 根运行；先设置指向本 worktree 的独立绝对 `CARGO_TARGET_DIR`。构建当前页面 `npm --prefix frontend run build`，以及当前后端与安全夹具 `cargo build --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-fixture --bin herdr-paths --example safe_shutdown_shell`。

完成检查分别运行 `node scripts/test-backend.mjs`、`npm --prefix frontend test`、`npm --prefix frontend run test:browser:build`。本票新增／更新的具体公开场景必须实际执行并记录用例选择器与证据；全局通过不能替代所认领的场景。涉及业务脚本时必须已有 Bun 并执行真实文件场景；页面票必须观察实际打包页面，类型检查不能替代。受控 Herdr／provider 只证明已标明的机制，不证明原生或真实模型业务。
