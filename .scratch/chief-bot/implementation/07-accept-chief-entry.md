# 07: 跨仓验收 chief 入口与生命周期

**What to build:** 通过交付页面与真实 OMP/Pi 证明 chief 固定角色启动/恢复可用，且所有入口服从同一后端唯一身份和离线保留规则。

**Delivers:** P7, P29, P30

**Blocked by:** 04：约束唯一 chief 的创建恢复与登记；05：交付 chief 入口与单次发起设置；06：交付两宿主 chief 固定角色入口

**Repositories:** herdr-bot, my-agent-toolkit

**Status:** ready-for-agent

来源：[approved contract](../contract.md)、[spec](../spec.md)。

## Acceptance criteria

- [ ] 两宿主分别经公开发起和新进程恢复实际加载原生工具、角色指引并取得调用结果；未选 Suite 仍工作。
- [ ] 多浏览器及并发公开调用覆盖注册前占位、普通/专用入口、不同 zone/Task，权威状态与有效操作只有一个胜者。
- [ ] 以公开生命周期操作和可控时间验证离线阈值前后、保留期恢复、明确注销；真实页面反馈不可创建/可创建，不推断旧进程退出。
- [ ] 时间窗口引用实际 online/offline 配置与维护扫描预算，记录阈值与实际观察，不新增产品等待策略。

## Coverage partition — 本票唯一验收所有权

|Promise|Coverage item（均分别覆盖 OMP/Pi）|交付票|测试所有者|
|---|---|---|---|
|P7|OMP/Pi × 无 Suite 发起/恢复 × 实际工具及指引消费|07|07|
|P29|注册前占位、普通/专用入口、跨浏览器/zone/Task 竞争|07|07|
|P30|离线保留、阈值后移除、保留期恢复、注销与页面反馈|07|07|

## 约束与证据

- 继承已批准 contract 的 S1/S2/S3、Implementation Decisions 与全部 Out of Scope，不重新设计 seam。公开操作的具体参数由实现确定，但必须与消费方同步，不留兼容空壳、假工具或假结果。
- 本票只拥有下述 coverage item 的测试；依赖票的永久用例可引用，不复制同一场景重新认领。受控响应仅证明协议/故障，不能证明真实模型职责。
- 所有实际模型调用，包括烟测，固定 GPT-6-luna，记录实际返回身份；不可用或身份不符即阻塞，不回退。测试驱动只提供用户输入/原生批准，不替模型选择业务工具或伪造结果。
- 使用测试独占定义、存储、目录、动态回环端口、HOME/APPDATA/XDG、原生会话和 Herdr 实例；不读取用户凭据/Chat，不操作正常服务或真实关机。明确授权的独立验收凭据只通过现有安全入口使用，不写入证据。
- 时间与无自动行为检查声明实际配置、启动/确认/维护预算及有界观察窗口；不得用即时断言、无限等待或新增产品等待/重试策略冒充证明。保留原始失败和清理失败；退出并确认所属进程结束，不以删除目录代替。
- 功能完成后更新对应仓库现有使用/测试说明或 changelog；记录命令、退出状态、源码/锁文件/产物/安装宿主身份及逐项证据。缺真实依赖或原生能力保持未验证，不能把计划内自动验收交给用户代办。

## 跨仓验收责任与运行入口

本票拥有 `entry` 专项的实现、维护和证据，可修改上述两个仓库。它是普通可调度票，不是所有工作完成后的新增统一闸门。依赖在 first-merge 后满足，不等待依赖票后台集成结束。

本票须交付以下**新入口**（当前发布的是实现要求，不声称该命令已存在或通过），从 herdr-bot worktree 根运行：

```bash
python scripts/accept-chief-entry.py --herdr-bot-worktree "$HERDR_BOT_WT" --toolkit-worktree "$TOOLKIT_WT" --backend "$HERDR_BOT_BINARY" --hosts omp,pi --paths-file "$ISOLATED_PATHS" --model-config "$GPT6_LUNA_CONFIG" --evidence-dir "$NEW_EVIDENCE_DIR"
```

- 两个 worktree、后端产物与已有安装依赖均为显式绝对路径；证据目录必须不存在。复用现有原生/浏览器/进程/真实模型传输设施，但不得借用其他专项已通过结论替代本票场景。
- **herdr-bot 分别通过：**在 `$HERDR_BOT_WT` 中运行 `npm --prefix frontend run build`，以独立绝对 CARGO_TARGET_DIR 构建 `cargo build --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-fixture --example safe_shutdown_shell`；再执行 `node scripts/test-backend.mjs`、`npm --prefix frontend test`、`npm --prefix frontend run test:browser:build`。
- **my-agent-toolkit 分别通过：**在 `$TOOLKIT_WT` 使用 Bun 1.4.4 运行 `bun run typecheck`、`bun run test`、`bun run check:integrations`，以及本票受影响的原生 dock 用例。使用参与 worktree 的实际源码/依赖，必要时显式提供既有 baseline 依赖定位，不更改 baseline。
- 配对 dock 的 HERDR_BOT_BINARY 必须指向本次安全关机壳，HERDR_BOT_METADATA 指向本次 worktree 元数据；真实启动专项的 `--backend` 指向本次生产后端。不能用生产程序替代安全壳执行关机回归。
- **专项分别通过：**OMP 与 Pi 各自全部 coverage item 有真实原生历史、工具输入/结果、确认交互、HTTP 与持久事实证据。全部实际模型请求使用 GPT-6-luna，记录返回身份；受控 provider 只能承载明确标注的机械/故障案例。
- 全局套件不能替代本票专项；若引用已有回归，须证明相关源码、夹具、依赖和环境未变并明确对应状态。源码变更后不能沿用旧结果。独占资源、顺序重型检查，不重试失败业务决策。
- 缺 worktree/构建/宿主/模型/确认能力、单宿主缺证据、原始验证或清理失败均非零退出，逐项保持未验证；fail-closed CLI 检查不算业务通过。
