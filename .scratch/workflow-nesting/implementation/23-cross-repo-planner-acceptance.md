# 23: 跨仓规划者发现与选择两类 Bot 验收

**What to build:** 真实规划者按业务要求读取两类 Bot 的三份指南并选择适合能力，生成通过公开校验的工作定义，不依赖指南自动注入。

**Delivers:** P03

**Blocked by:** 12: OMP 与 Pi 双角色工具及指引共存；20: 规划者发现并生成两类 Bot 节点

**Repositories:** herdr-bot, toolkit

**Status:** ready-for-agent

依据：[批准契约](../contract.md)；[规格](../spec.md)。契约 Approval 已获批准，本轮 24 票修订方案由用户授权发布。发布不代表实现、自动验收或用户部署操作已完成。

## Acceptance criteria

- [ ] 隔离目录预置 Task 型和 Map 型能力，以及相似名称但不同职责的干扰项；选择预期由独立业务要求决定，不从被测输出重算。
- [ ] 实际原生读取轨迹证明 when／plan_guide／node_guide 与真实目标被核实；两个规划场景分别合理选用两类能力，不能只验证目录包含名称。
- [ ] 生成定义通过公开严格校验及预检：Map 型节点 manager、input、zone、输入接口及文件映射正确，子图不展开。
- [ ] 不把指南自动注入当作前提，不把能力声明当已实现证据；缺能力／授权／业务决定时保留明确阻塞，不自行补建。
- [ ] 保留方案确认与不自动发起生成 Map 的边界，以实际工具／HTTP／运行事实证明无启动；不只看模型自述。
- [ ] 本票仅拥有 P03，不等待包装案例或旧外层调用；测试清单与证据独立于 24。
- [ ] 所有受影响调用方与既有行为检查同步切换，删除被本次替代的死路径，不引入旧字段别名或以兜底吞掉失败。
- [ ] 更新所属长期使用／运行／验证说明，记录真实执行的入口及证据边界；临时 ticket／证据不作为长期文档唯一依赖。

## 覆盖分区与测试所有权

| Promise | 本票覆盖项／实现贡献 | Contract 测试 owner | 其余归属 |
|---|---|---|---|
| P03 | 真实三份指南发现、双类型选择及合法生成 | 23 | 完整 |

表中 owner 是相应覆盖项的唯一测试编写与维护者。一个用例可以支持多个 promise，但不能由两票重复认领；promise 必须在全部覆盖项通过后才完整交付。enabling 票的单仓机制／公开接线检查不替代上述 contract 覆盖。

## 跨仓 owner、调度与 Worktree checks

本票是普通跨仓联调／验收票，可修改两个列明仓库，拥有上表中跨仓测试的实现、运行与维护。阻塞开发票首次合并后按既有 frontier 调度；不等待其后台集成，不依赖其他验收票，不新增最终门禁。不得用全局套件代替本票专项。

`HERDR_BOT_WT`、`TOOLKIT_WT` 为本票分配的两个绝对 worktree 根；以下命令在相应根执行。各仓分别保存源码身份、命令、退出码及原始日志，不能使用默认同级旧仓或全局旧后端。

### herdr-bot — 必须分别通过

- 在本 worktree 使用独立、绝对 `CARGO_TARGET_DIR`；先 `npm --prefix frontend ci`（仅缺依赖时）和 `npm --prefix frontend run build`，生成本 worktree 的静态页面。
- `cargo build --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-fixture --example safe_shutdown_shell`。
- `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch` 及实际受影响的定向目标；`node scripts/test-backend.mjs`；`npm --prefix frontend test`。
- 如修改页面或页面验收，运行 `npm --prefix frontend run test:browser:build`；时间承诺标签变化后核对 `npm --prefix frontend run test:browser:partition`。页面行为必须在实际浏览器观察，不仅组件测试。

### toolkit — 必须分别通过

- 遵循该仓测试规范及 Bun 1.4.4；仅缺依赖时 `bun install --frozen-lockfile`。昂贵检查按该仓规范协调运行。
- 检查进程设置 `HERDR_BOT_REPO=HERDR_BOT_WT`、`HERDR_BOT_METADATA=<HERDR_BOT_WT 的 meta-data/task-options.toml>`、`HERDR_BOT_BINARY=<本票 CARGO_TARGET_DIR 中的 debug/examples/safe_shutdown_shell.exe>`；不能用生产关机程序代替安全壳。真实业务 runner 另用本轮 herdr-bot 二进制。
- `bun test tests/workflow-chat`，以及本票受影响的身份／双角色／collect／恢复原生 dock；`bun run typecheck`；`bun run test`，分别记录结果。
- 如 worktree 的 frozen baseline 需要既有 sibling 定位，显式设置 `OMP_BASELINE_RECORD_ROOT` 为原 toolkit 根，仅用于只读依赖／oracle，不替换被测 worktree 源码。

### 本票专项 — 必须实现并实际运行

下面是**本票要交付的测试入口与参数契约，不表示该命令目前已经存在或通过**。复用既有真实宿主、模型传输、隔离和证据设施；不得新增测试专用产品 API。入口从 `HERDR_BOT_WT` 运行：

```text
python scripts/accept-workflow-nesting-planning.py --backend <本票后端绝对路径> --toolkit <TOOLKIT_WT> --hosts declared --paths-file <独占隔离依赖配置> --model-config <专用授权模型声明> --evidence-dir <本次独占证据目录>
```

`--hosts declared` 必须执行实际业务定义声明的宿主及真实角色入口，逐项记录，不能为迁就 runner 把所有 Task 改为单一宿主或改变业务配置。`--hosts omp,pi` 必须两宿主分别通过，单宿主成功不抵消另一宿主缺失。支持从显式配置读取已安装依赖，不安装、不提权、不修改全局 PATH，不读取用户认证。

证据须包含两仓根及 revision／修改身份、构建产物与锁文件身份、已安装宿主／实际 OS 进程、模型服务身份（不记录密钥）、原始用户输入及确认、模型／工具／HTTP 请求结果、原生历史、实际文件字节、只读运行事实和逐覆盖项结论。独立预期来自 contract、固定字面样例和实际输入，不能由被测实现重算。

独占 HOME／APPDATA／XDG／原生存储／命名 Herdr／业务定义与运行目录；使用动态数值回环端口，持续消费 stdout／stderr，不启用 LAN／mDNS，不连接用户 daemon／Chat，不执行真实关机。不重试业务或模型决策掩盖失败。成功和失败都停止、等待并确认所属进程退出，分别保留原始错误与清理错误。

缺宿主、明确授权模型、必要能力或隔离／清理失败均非零且保持未验证；不 skip，不以准备完成、observer、受控 provider、单仓回归或最终状态冒充专项通过。

## 完成标准

上表本票覆盖项、全部验收条件和所属仓库检查均有实际通过证据后才能关闭。Full automation 无人工验收豁免；真实依赖缺失如实保留阻塞，不以提示词存在、配置文件、可编译 scaffold、受控成功或单次最终状态宣称完整交付。
