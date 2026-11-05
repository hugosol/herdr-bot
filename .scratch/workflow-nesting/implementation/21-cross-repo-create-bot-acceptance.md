# 21: 跨仓新建 Bot 包装业务验收

**What to build:** 真实模型在隔离部署中为已有 normal Task 及 Map 分别创建完整 Bot，三份指南与实际批准来源一致，目标不变且新能力未被自动执行。

**Delivers:** P39, P40, P41, P43, P44, P45（只认领下表覆盖项；完整 promise 等全部所属项通过）

**Test owner for:** P40, P43, P44, P45 的本票覆盖项；其他覆盖项 owner 见下表。

**Blocked by:** 12: OMP 与 Pi 双角色工具及指引共存；17: 发布为已有 Task 或 Map 包装 Bot 的能力

**Repositories:** herdr-bot, toolkit

**Status:** ready-for-agent

依据：[批准契约](../contract.md)；[规格](../spec.md)。契约 Approval 已获批准，本轮 24 票修订方案由用户授权发布。发布不代表实现、自动验收或用户部署操作已完成。

## Acceptance criteria

- [ ] 两种真实隔离目标分别完成成功闭环，组合覆盖有／无可选材料；用户只给定位、要求和材料，由执行者实际读取目标形成共同基线，必要歧义实际澄清。
- [ ] 实际创建全新 Bot，核对真实 ID、目标全部前后配置、绑定与最终三个指南，目标未修改、未执行；创建成功不等于新能力业务已验证。
- [ ] 三路各自确认及表达优化后汇合，保留确认版本和来源；各分支不直接覆盖 Bot，三字段一次合法保存，读回与批准文本逐字段一致。
- [ ] 真实读取证明必要消费者直接取得单份共同基线、唯一 text 和必要 handoff；禁止用 handoff 复制正文／配置或猜材料来源，不做提示词全文快照。
- [ ] P44 创建成功、基线／目标冲突、真实 revision 冲突、未授权、运行禁写、非目标字段保持、完整读回及已生效／未生效／未知结果分别有独立可观察检查；失败不盲重试或自动回滚。
- [ ] P44 文件安全覆盖真实隔离安全写入、链接／越界拒绝及完整字节读回；保护有实际可达承接，不因去重删除，不把原地改写概括为全面禁止。
- [ ] 下表六个既有共享 Task，以及 17 实际新增的创建专属 Task，逐个执行同一 Task 的独立 Chat 与 workflow 配对交付，独立场景不注入 node 角色且交付／授权仍正确。
- [ ] 使用实际授权业务模型及真实宿主，不用受控 provider 预设业务步骤；实际用户批准由隔离场景自动送入真实 Chat 并保留记录，不能把报告 true 当授权。
- [ ] 所有受影响调用方与既有行为检查同步切换，删除被本次替代的死路径，不引入旧字段别名或以兜底吞掉失败。
- [ ] 更新所属长期使用／运行／验证说明，记录真实执行的入口及证据边界；临时 ticket／证据不作为长期文档唯一依赖。

## 覆盖分区与测试所有权

| Promise | 本票覆盖项／实现贡献 | Contract 测试 owner | 其余归属 |
|---|---|---|---|
| P39 | normal Task／Map 包装、真实身份、目标不变和不执行 | 21 | 完整 |
| P40 | 创建有／无材料、真实目标读取、基线及澄清 | 21 | 修订：22 |
| P41 | 创建三分支确认／优化／一次保存与来源一致 | 21 | 完整 |
| P43 | 创建链实际基线／text／handoff 交付及消费 | 21 | 修订链：22 |
| P44 | 创建链成功、授权／冲突／revision／运行禁写／保持／读回／部分生效／文件安全 | 21 | 修订链相应项目：22 |
| P45 | 六个既有共享 Task 与创建专属 Task 的独立／workflow 配对 | 21 | 修订专属：22；外层修改：24 |

表中 owner 是相应覆盖项的唯一测试编写与维护者。一个用例可以支持多个 promise，但不能由两票重复认领；promise 必须在全部覆盖项通过后才完整交付。enabling 票的单仓机制／公开接线检查不替代上述 contract 覆盖。

## P45 既有共享 Task 唯一归属

| 稳定 Task ID | 业务名称 | P45 配对测试 owner |
|---|---|---|
| `task-18e33fdc55177d1c-1` | 空白 Bot 创建者 | 21 |
| `task-18e3404478db1ef8-2` | Bot.when 编写者 | 21 |
| `task-18e340545eb481bc-3` | Bot.plan_guide 编写者 | 21 |
| `task-18e340f5287462ec-4` | Bot.node_guide 编写者 | 21 |
| `task-18e31d34797f1464-3` | 通用提示词优化者 | 21 |
| `task-18e3415003d8cd00-5` | 通用文本替换者 | 21 |

21 逐个拥有上述同一 Task 的独立 Chat／workflow 配对测试。17 的创建专属新增项归 21；18 的修订专属新增项归 22；19 的外层修改项归 24。新增／删除受影响项必须同步清单并确认唯一 owner，不得以“共享／专属”泛称省略实际稳定 ID。

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
python scripts/accept-workflow-nesting-create.py --backend <本票后端绝对路径> --toolkit <TOOLKIT_WT> --hosts declared --paths-file <独占隔离依赖配置> --model-config <专用授权模型声明> --evidence-dir <本次独占证据目录>
```

`--hosts declared` 必须执行实际业务定义声明的宿主及真实角色入口，逐项记录，不能为迁就 runner 把所有 Task 改为单一宿主或改变业务配置。`--hosts omp,pi` 必须两宿主分别通过，单宿主成功不抵消另一宿主缺失。支持从显式配置读取已安装依赖，不安装、不提权、不修改全局 PATH，不读取用户认证。

证据须包含两仓根及 revision／修改身份、构建产物与锁文件身份、已安装宿主／实际 OS 进程、模型服务身份（不记录密钥）、原始用户输入及确认、模型／工具／HTTP 请求结果、原生历史、实际文件字节、只读运行事实和逐覆盖项结论。独立预期来自 contract、固定字面样例和实际输入，不能由被测实现重算。

独占 HOME／APPDATA／XDG／原生存储／命名 Herdr／业务定义与运行目录；使用动态数值回环端口，持续消费 stdout／stderr，不启用 LAN／mDNS，不连接用户 daemon／Chat，不执行真实关机。不重试业务或模型决策掩盖失败。成功和失败都停止、等待并确认所属进程退出，分别保留原始错误与清理错误。

缺宿主、明确授权模型、必要能力或隔离／清理失败均非零且保持未验证；不 skip，不以准备完成、observer、受控 provider、单仓回归或最终状态冒充专项通过。

## 完成标准

上表本票覆盖项、全部验收条件和所属仓库检查均有实际通过证据后才能关闭。Full automation 无人工验收豁免；真实依赖缺失如实保留阻塞，不以提示词存在、配置文件、可编译 scaffold、受控成功或单次最终状态宣称完整交付。
