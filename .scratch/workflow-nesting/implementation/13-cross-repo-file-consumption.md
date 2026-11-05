# 13: 跨仓命名输入与真实文件消费验收

**What to build:** 在隔离真实 Chat 中消费根及多层命名输入，并将子流程交付交给普通节点和另一个子流程，观察真正读取的地址、内容及文件变化。

**Delivers:** P05, P09, P10, P12, P14, P16, P18（只认领下表覆盖项；完整 promise 等全部所属项通过）

**Test owner for:** P05, P09, P10, P12, P14, P16, P18 的本票覆盖项；其他覆盖项 owner 见下表。

**Blocked by:** 05: 完成子流程交付与父层显式验收；12: OMP 与 Pi 双角色工具及指引共存

**Repositories:** herdr-bot, toolkit

**Status:** ready-for-agent

依据：[批准契约](../contract.md)；[规格](../spec.md)。契约 Approval 已获批准，本轮 24 票修订方案由用户授权发布。发布不代表实现、自动验收或用户部署操作已完成。

## Acceptance criteria

- [ ] 编写并运行本票专用自动场景，显式加载参与 worktree 的后端、toolkit 角色入口与隔离定义，复用已有隔离／证据设施，不使用旧场景冒充新覆盖。
- [ ] 真实页面发起所得文字与文件绑定进入实际 Chat；采用独立文字标记和文件字节，证明文字与文件分别传入。
- [ ] 内外 zone 使用不同目录，各自真实启动及读取来源与配置一致；多层上游 ID／本地 ID 不同、跨 Run 同名均按本次绑定消费。
- [ ] 根与嵌套空输入、可选缺席在真实执行者中明确缺席，不读取占位路径或自行寻找文件。
- [ ] 子流程登记真实文件，经子流程→普通节点、子流程→另一子流程实际读取并核对内容与接口映射，不能只比地址。
- [ ] 派发后原地修改隔离文件，真实消费者观察变化，证明无内容快照；另在检查后删除文件，执行者如实处理失败，系统无自动修复／重试。
- [ ] 实际模型自主选择必要工具并交付，受控 provider 只可用于补充机制检查；保留工具读取轨迹、字节与运行关联，不能把最终正确文件当作完整读取证据。
- [ ] 仅将实际验证的宿主记录为通过，不把 Pi 文件消费结果外推 OMP；双宿主同载义务由 14 单独拥有。
- [ ] 所有受影响调用方与既有行为检查同步切换，删除被本次替代的死路径，不引入旧字段别名或以兜底吞掉失败。
- [ ] 更新所属长期使用／运行／验证说明，记录真实执行的入口及证据边界；临时 ticket／证据不作为长期文档唯一依赖。

## 覆盖分区与测试所有权

| Promise | 本票覆盖项／实现贡献 | Contract 测试 owner | 其余归属 |
|---|---|---|---|
| P05 | 内外真实目录和文件来源 | 13 | 运行绑定：04 |
| P09 | 原生收到独立业务文字与实际文件绑定 | 13 | 页面与读回：01 |
| P10 | 空输入／可选缺席的实际消费，无猜路径 | 13 | 入口／解析：01、02、04 |
| P12 | 多层重命名后的真实材料读取 | 13 | 本层：02；跨层绑定：04 |
| P14 | 原地改写后的真实可变内容消费 | 13 | 无快照机制与说明：02 |
| P16 | 检查后删改的执行者真实处理 | 13 | 无系统修复：02 |
| P18 | 子输出进入普通／Map 下游的实际内容 | 13 | 公开解析：05 |

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
python scripts/accept-workflow-nesting-files.py --backend <本票后端绝对路径> --toolkit <TOOLKIT_WT> --hosts declared --paths-file <独占隔离依赖配置> --model-config <专用授权模型声明> --evidence-dir <本次独占证据目录>
```

`--hosts declared` 必须执行实际业务定义声明的宿主及真实角色入口，逐项记录，不能为迁就 runner 把所有 Task 改为单一宿主或改变业务配置。`--hosts omp,pi` 必须两宿主分别通过，单宿主成功不抵消另一宿主缺失。支持从显式配置读取已安装依赖，不安装、不提权、不修改全局 PATH，不读取用户认证。

证据须包含两仓根及 revision／修改身份、构建产物与锁文件身份、已安装宿主／实际 OS 进程、模型服务身份（不记录密钥）、原始用户输入及确认、模型／工具／HTTP 请求结果、原生历史、实际文件字节、只读运行事实和逐覆盖项结论。独立预期来自 contract、固定字面样例和实际输入，不能由被测实现重算。

独占 HOME／APPDATA／XDG／原生存储／命名 Herdr／业务定义与运行目录；使用动态数值回环端口，持续消费 stdout／stderr，不启用 LAN／mDNS，不连接用户 daemon／Chat，不执行真实关机。不重试业务或模型决策掩盖失败。成功和失败都停止、等待并确认所属进程退出，分别保留原始错误与清理错误。

缺宿主、明确授权模型、必要能力或隔离／清理失败均非零且保持未验证；不 skip，不以准备完成、observer、受控 provider、单仓回归或最终状态冒充专项通过。

## 完成标准

上表本票覆盖项、全部验收条件和所属仓库检查均有实际通过证据后才能关闭。Full automation 无人工验收豁免；真实依赖缺失如实保留阻塞，不以提示词存在、配置文件、可编译 scaffold、受控成功或单次最终状态宣称完整交付。
