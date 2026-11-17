# 07: V1 无授权凭据创建空白 Bot

**What to build:** V1 创建脚本只消费执行必需的新协议材料，真实创建空白 Bot 并交付可读结果，不再向上游索要人类批准凭据。

**Delivers:** P31 的 S1 创建分区；P33 的创建正确性分区

**Test owner for:** P31、P33（仅下列 Coverage ownership 中本票分区；承诺整体由全部分区共同完成）

**Blocked by:** 04：脚本成功后批准，失败不批准

**Repositories:** herdr-bot

**Status:** ready-for-agent

## Acceptance criteria

- [ ] 使用独立字面新 request 经真实 Map／公开 dispatch、Bun 和实际文件完成创建；完整公开读回目标和 Bot 配置，真实读取基线及交付材料，不以 Schema 通过代替执行（P31）。
- [ ] 删除整个 authorization 对象及创建端生产／消费要求，不用 approved、自动 true 或其他凭据改名复活旧授权；不再要求 create_blank/write_materials/save_guides 或供验权的确认材料。拒绝旧协议，不加别名、双解析或自动转换。
- [ ] 明确共享新 request／基线／目标接口及字段归属，记录给 08、09 使用；业务协议不包含通用执行批准事实，不修改 V2 专用材料协议。
- [ ] 独立覆盖非法形状、重复／未知字段、目标歧义、字段归属、实际冲突及合法保存约束；比较完整对象和真实文件，无误写或假成功（P33 创建分区）。
- [ ] 真实制造保存、读回或材料失败，记录原错误和已生效效果，部分生效不回滚；不刷新基线、删除已建 Bot 或自动重跑来掩盖失败（P33）。
- [ ] 同步替换创建脚本旧授权测试、独立字面材料和长期协议文档；保留与执行正确性有关的校验，不因旧摘要与授权混合而整段删除有效保护。

## Coverage ownership

| Promise | 本票唯一拥有的测试覆盖项 | Test owner |
|---|---|---|
| P31 | S1 新字面协议真实创建无凭据、无旧解析 | 07 |
| P33 | S1 创建形状／目标／归属／真实写入与材料故障及部分效果 | 07 |

## 验证与交付边界

来源为已批准 contract 与 spec；按本票 Coverage ownership 认领测试，其余票的同名承诺分区不得重复认领或缩减。独立预期来自契约与字面业务样例，不调用被测实现计算预期。S1 为公开 HTTP／定义与真实业务效果，S2 为实际 OMP／Pi 工具，S3 为真实浏览器；私有模块不升级为新 seam。

先实现本票行为及必要回归，跳过中途完整 build／lint／test／formatter；完成后一次执行本票所需检查。已有测试覆盖的路径更新有效契约；只增加能捕获消费者可见错误的用例，删除旧机制专属断言，保留仍有效的保存、通知失败、身份隔离、输出和门禁职责。同步更新实际受影响的长期文档／变更记录。

资源必须独占：定义、数据目录、原生会话、HOME／APPDATA／XDG、动态数值回环端口、进程和证据目录。不使用用户历史 Chat／Run，不读取未授权凭据、不安装宿主、不执行真实关机。原始失败与清理失败分别保留；确认所属进程退出后才能清理。无旧 Run 兼容／升级、历史改写、批准通知、批准说明／产物前提、自动重试或隐式推进。承诺缺真实依赖／授权时明确未验证，不转交人工、不用受控成功顶替。

## 本仓检查

从本票 herdr-bot worktree 根运行；先设置指向本 worktree 的独立绝对 `CARGO_TARGET_DIR`。构建当前页面 `npm --prefix frontend run build`，以及当前后端与安全夹具 `cargo build --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-fixture --bin herdr-paths --example safe_shutdown_shell`。

完成检查分别运行 `node scripts/test-backend.mjs`、`npm --prefix frontend test`、`npm --prefix frontend run test:browser:build`。本票新增／更新的具体公开场景必须实际执行并记录用例选择器与证据；全局通过不能替代所认领的场景。涉及业务脚本时必须已有 Bun 并执行真实文件场景；页面票必须观察实际打包页面，类型检查不能替代。受控 Herdr／provider 只证明已标明的机制，不证明原生或真实模型业务。
