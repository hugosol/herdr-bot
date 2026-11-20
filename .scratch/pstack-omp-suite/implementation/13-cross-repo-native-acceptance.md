# 13: 跨仓 Suite 装配、恢复与角色消费验收

**What to build:** 从公开 Task／Chat 发起和恢复进入安装的真实 OMP，观察选定资源、固定环境、保存状态和生成角色的实际消费，失败明确且启动不隐式发布资源。

**Delivers:** P2, P3, P14, P17, P20, P24, P28, P31, P46

**Blocked by:** 04: 首次 setup、确认绑定与原生执行角色, 12: 公开入口中的 OMP-only pstack Suite

**Repositories:** herdr-bot, toolkit

**Status:** ready-for-agent

- [ ] 本票为跨仓联调／验收 owner，可修改列出的两个仓库，独占下列跨仓用例；复用 S2/S3 及既有验收设施，不新建 transport、技能加载器或角色运行时。
- [ ] 交付公开 runner：在 herdr-bot worktree 根运行 python scripts/accept-pstack-suite.py --herdr-bot-worktree <Bot绝对路径> --toolkit-worktree <Toolkit绝对路径> --backend <本次生产后端绝对路径> --isolated-config <owned安装配置绝对路径> --cases all --evidence-dir <不存在的新证据目录>。这是本票新增交付命令，须附可复制配置样例与帮助；命令路径仅作为稳定验收入口，不固定内部实现布局。
- [ ] 配置显式选择实际安装的 OMP、Herdr 宿主、core、Bun/Node 与官方固定扩展；使用真实公开 HTTP／页面发起和恢复、自有状态根、受控回环模型 adapter。受控模型必须驱动真实原生工具，不代写原生历史／状态或用替身宿主冒充真实消费。
- [ ] P2：准备未选集合及同名异源技能，观察实际 OMP 最终目录并明确读取选定源正文，核对实体来源；名称白名单或 argv 不足以证明通过。
- [ ] P3：真实发起和恢复都保留适用 Task 固定扩展及项目指令，以扩展实际运行、原生请求与读取事实验证，不推导文件／凭据沙箱。
- [ ] P14：真实发起／恢复时观察源资源、进程调用及必要网络记录，并用可读不可写的交付资源作对照；不得同步上游或构建套件。
- [ ] P46：声明资源缺失／不可读时，经公开发起与恢复均明确失败，独立检查实际启动副作用，不回退空环境；不扩大成所有外部依赖的预检。
- [ ] P17 跨仓分区：公开恢复原生 on/off Chat，检查首轮贡献和保存值；不重复 03 的原生开关／跨进程基础用例。P20 跨仓分区：Suite 装配后明确主／子读取的实际来源基准正确，不依赖父聊天继承。
- [ ] P24/P31 跨仓分区：无绑定经公开 Suite 进入 setup，明确确认生成后重新发起 Chat 消费角色；停止同包会话后 A→B 更新再公开发起也消费 B。P28 跨仓分区：经 herdr 发起其他 Suite 的对照会话，核对配置及实际模型未受影响。
- [ ] 两个仓库检查分别通过，命令、源码身份、退出码与原始日志分开留档；全仓绿不能替代本票跨仓场景。过滤／缺格／依赖失败保持非成功或明确未验证，不改断言补绿。
- [ ] 保留真实原生请求、历史、资源来源、文件、HTTP／页面观察及所属进程退出证据，只清理 owned 资源，不操作用户已有会话或认证。

## 覆盖归属

P2、P3、P14、P46 全部由本票写测试。P17、P20、P24、P28、P31 仅认领上列跨仓分区；03／04 拥有其原生分区。05 的面板方法和并行机制不是本票启动前提。

## 契约与执行边界

依据：[已批准 contract](../contract.md)（S1 套件维护命令、S2 Suite 选择及 Chat 发起／恢复、S3 OMP 原生会话交互）。以 P 编号和本票 coverage 为验收依据，不改 spec／contract 的承诺。

只保证迁移内容语义和代码机制，不测试模型自主路由、评分、综合、授权服从或输出质量；内容审查由 agent 完成，不转成人工必验。受控 adapter 只证明请求和真实工具交互。不得用源码字符串、参数正确或产物存在替代消费者可见效果；期望来自独立指定输入、批准上游方法或独立字面结果。

复用现有原生机制和测试归属，已有覆盖不重复新增。修改前读取目标仓的适用指引。使用 owned 临时配置、Git 仓库、端口、页面和可追踪进程，保留证据并只清理所属资源。缺依赖、错误或未运行必须如实报告。不要安装／修改宿主、全局模型或生产配置来伪造通过。

更新生成资源／模型前停止使用同包的 pstack 会话，完成后重新发起；不引入热加载、在线快照或跨进程强制停用检测。正常并行会话不被禁止。先完成代码、内容审查和全部可执行自动检查，再进行 H1/H2 人工验收；人工未确认项继续标为未验证，不阻止其他工作。

## 跨仓检查与 worktree 绑定

先把 `<Bot>`、`<Toolkit>`、`<CargoTarget>`、`<Evidence>` 替换为本轮绝对路径。两个 worktree 均为本票实际来源；Cargo 目录独占且新建／为空，证据目录不得已存在。安装选择和测试状态根均为 owned，不复制用户认证。

- **herdr-bot**：在 `<Bot>/frontend` 执行 `bun install --frozen-lockfile`，然后在 `<Bot>` 根执行 `npm --prefix frontend run build`；设置独占绝对 `CARGO_TARGET_DIR=<CargoTarget>` 后执行 `cargo build --manifest-path backend/Cargo.toml --locked --bins --example safe_shutdown_shell`。随后分别运行 `node scripts/test-backend.mjs`、`npm --prefix frontend test`、`npm --prefix frontend run test:browser:build`。关机相关测试和配对 dock 只接同次构建的安全壳，不允许系统关机；生产后端用于 owned 业务 HTTP。
- **toolkit**：在 `<Toolkit>` 根用 Bun 1.4.4 执行 `bun install --frozen-lockfile`，分别执行 `bun run typecheck`、`bun run test:framework`、`bun run check:integrations`、`bun run test`。需要配对后端的 dock 将 `HERDR_BOT_BINARY` 明确指向上述同次构建的 `safe_shutdown_shell.exe`。隔离 worktree 缺少历史 sibling 布局时，按既有测试指引设置 `OMP_BASELINE_RECORD_ROOT` 指向原 toolkit 根，仅解析基线依赖，不能替代本 worktree 源码。
- **专项**：从 `<Bot>` 根执行本票验收条件中的公开 runner，两个 worktree 参数必须显式绝对路径；13 的 `--backend` 指向 `<CargoTarget>` 中本次生产 `herdr-bot.exe`。配置声明实际安装与版本，使用独占真实宿主／状态、回环受控 provider 和新证据目录；14 的只读包检查不替代 13 的原生专项。
- 两仓分别保存命令、源码／产物身份、安装版本、原始日志和退出码，任一仓失败不得称跨仓通过。全局 suite 不能替代 contract 指定专项；专项中的筛选、跳过和缺格不能宣称全量通过。H1/H2 不得由自动结果代签。

## 跨票 coverage partition

每个 coverage item 仅有一个测试 owner；同一 promise 的所有分区满足后才完成。P4/P11 的最终完整包测试统一由 14 编写，其他票贡献资源与业务证据，不重复认领完整包闭包。

|---|---|---|---|
| P17 | 原生 on/off 跨进程恢复、首轮贡献及 H1 材料 | 03 | 03 |
| P17 | 公开 Chat 恢复后的保存值与贡献 | 13 | 13 |
| P20 | 不同 cwd、含空格源目录的主／子原生明确读取 | 03 | 03 |
| P20 | Suite 装配后主／子读取的真实来源基准 | 13 | 13 |
| P24 | 无角色绑定的原生会话及 setup 读取 | 04 | 04 |
| P24 | 无绑定经公开 Suite 发起可达 setup | 13 | 13 |
| P28 | 生成前后配置、非 pstack 原生会话的解析对照 | 04 | 04 |
| P28 | 经 herdr 发起其他 Suite 的模型行为对照 | 13 | 13 |
| P31 | 原生首次确认生成与停用更新 A→B 后新会话 | 04 | 04 |
| P31 | 公开 Chat 首次 setup／生成／重启及更新消费 | 13 | 13 |

所有未列出的 promise 由其单一 Delivers 票独占测试／内容审查。H1 原始材料与回传复核由 03 负责，H2 由 10 负责，14 汇总状态而不重复测试或代替用户确认。
