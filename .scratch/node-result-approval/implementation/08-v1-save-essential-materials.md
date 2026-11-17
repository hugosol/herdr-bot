# 08: V1 仅凭执行必需材料保存三份指南

**What to build:** V1 保存脚本使用真实基线、目标与明确绑定正文完成三个指南字段的合法保存，不再要求用于验权的三份交接书。

**Delivers:** P32；P31 的 S1 保存分区；P33 的保存正确性分区

**Test owner for:** P31、P33（仅下列 Coverage ownership 中本票分区；承诺整体由全部分区共同完成）

**Blocked by:** 07：V1 无授权凭据创建空白 Bot

**Repositories:** herdr-bot

**Status:** ready-for-agent

## Acceptance criteria

- [ ] 沿用 07 的新材料契约，仅提供执行必需材料即可经真实 Map／Bun 保存三个字段；完整公开读回配置、真实 revision 和实际 saved／交付文件（P31、P32）。
- [ ] 删除 authorization 和三份供验权 handoff 的强制消费、对应确认凭据和生产要求；不以 approved 或正文批准 digest 保留人类授权门禁，不加旧输入兼容。仍可保留独立于验权的必要一致性校验。
- [ ] 使用三段独立字面正文，覆盖换行、空白及不同字段值，逐字段核对最终完整对象；目标配置、非目标字段和真实绑定不变（P33 保存分区）。
- [ ] 独立验证 revision 冲突、基线变化、正文／字段归属错误、目标变化和现有运行禁写规则；公开拒绝与实际文件共同证明没有误写。
- [ ] 用真实文件或存储故障验证写入失败、写后读回失败及材料失败；保留已写入／已读回事实及部分输出，不回滚或假报成功，不刷新旧基线以使重跑通过（P33）。
- [ ] 同步修订 ADR 0019 中旧授权检查决定，保留无 Chat、自动报告、显式 complete 及合法保存职责；更新材料协议、Schema／字面例和旧授权测试，不改通用文本替换者或 V2 专用协议。

## Coverage ownership

| Promise | 本票唯一拥有的测试覆盖项 | Test owner |
|---|---|---|
| P31 | S1 新协议真实保存无凭据 | 08 |
| P32 | S1 仅必需材料真实三字段保存、完整读回与交付 | 08 |
| P33 | S1 保存正文／基线／revision／字段归属／真实故障及部分效果 | 08 |

## 验证与交付边界

来源为已批准 contract 与 spec；按本票 Coverage ownership 认领测试，其余票的同名承诺分区不得重复认领或缩减。独立预期来自契约与字面业务样例，不调用被测实现计算预期。S1 为公开 HTTP／定义与真实业务效果，S2 为实际 OMP／Pi 工具，S3 为真实浏览器；私有模块不升级为新 seam。

先实现本票行为及必要回归，跳过中途完整 build／lint／test／formatter；完成后一次执行本票所需检查。已有测试覆盖的路径更新有效契约；只增加能捕获消费者可见错误的用例，删除旧机制专属断言，保留仍有效的保存、通知失败、身份隔离、输出和门禁职责。同步更新实际受影响的长期文档／变更记录。

资源必须独占：定义、数据目录、原生会话、HOME／APPDATA／XDG、动态数值回环端口、进程和证据目录。不使用用户历史 Chat／Run，不读取未授权凭据、不安装宿主、不执行真实关机。原始失败与清理失败分别保留；确认所属进程退出后才能清理。无旧 Run 兼容／升级、历史改写、批准通知、批准说明／产物前提、自动重试或隐式推进。承诺缺真实依赖／授权时明确未验证，不转交人工、不用受控成功顶替。

## 本仓检查

从本票 herdr-bot worktree 根运行；先设置指向本 worktree 的独立绝对 `CARGO_TARGET_DIR`。构建当前页面 `npm --prefix frontend run build`，以及当前后端与安全夹具 `cargo build --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-fixture --bin herdr-paths --example safe_shutdown_shell`。

完成检查分别运行 `node scripts/test-backend.mjs`、`npm --prefix frontend test`、`npm --prefix frontend run test:browser:build`。本票新增／更新的具体公开场景必须实际执行并记录用例选择器与证据；全局通过不能替代所认领的场景。涉及业务脚本时必须已有 Bun 并执行真实文件场景；页面票必须观察实际打包页面，类型检查不能替代。受控 Herdr／provider 只证明已标明的机制，不证明原生或真实模型业务。
