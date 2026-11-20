# 14: 完整资源闭包与分层交付证据

**What to build:** 维护者拿到完整可追溯的交付包及分层证据：全部资源、配套引用和许可证可查，内容审查、生成、装配、原生机制与人工未验项准确区分。

**Delivers:** P4, P11, P45

**Blocked by:** 05: 独立面板席位、并行产物与模型偏离事实, 06: 可维护的项目验证技能与工程核查方法, 07: deslop 与真实 Windows CLI／UI 驱动链路, 08: Windows statechart 的真实图与报告, 09: 决策日志、worktree 审计与安全交接, 10: PR 观察、本地交付与原版授权语义, 11: 真实可解码的视频汇报产物, 13: 跨仓 Suite 装配、恢复与角色消费验收

**Repositories:** herdr-bot, toolkit

**Status:** ready-for-agent

- [ ] 本票为跨仓资源／证据验收 owner，可修改列出的两个仓库；独占 P4/P11 的最终完整包清单、来源、许可证、配套资源闭包测试，不重复 13 的原生装配测试。
- [ ] 交付公开只读检查：在 herdr-bot worktree 根运行 python scripts/check-pstack-delivery.py --herdr-bot-worktree <Bot绝对路径> --toolkit-worktree <Toolkit绝对路径> --evidence-dir <不存在的新证据目录>。这是本票新增命令；提供调用帮助，实际交付资产与声明清单逐项对照，不能调用生成器反算期望或以文件存在证明迁移成功。
- [ ] P4：核对实际交付资产、上游来源版本／许可证、具名映射及未迁移范围说明；内容审查与版本事实相对应，不沿用调查中的技能数量当最终承诺。
- [ ] P11：最终包覆盖技能正文、playbooks、worker/persona、脚本、资料及三个必要外部技能的实际依赖，逐项检查引用并明确读取真实文件；区分包内资产、运行时外部前提及项目产物，不把无法读取判通过。
- [ ] P45：每项 P1–P46 映射到拥有票、测试／审查记录和实际证据，记录版本、输入、命令、退出、产物及副作用；结论区分生成、内容审查、装配、原生机制和人工状态。模型自主效果不得列为已验证。
- [ ] 两个仓库分别运行并通过下列检查，保留独立原始日志；再运行本票最终包检查，并核对 13 专项及各资源票的当前来源证据。不以全仓绿抵消专项失败或环境缺项。
- [ ] 所有可执行实现、内容审查和自动检查先完成；H1 只涉及 P16/P17，H2 只涉及 P38，复核 03／10 的准备材料。用户未确认前明确列为未验证，不凭人工计划中止其他工作，不新增提示词效果人工门禁。
- [ ] 这是一张普通可调度验收票，阻塞边只等待贡献票首合入，不要求等待后台 integration/retro；本票失败如实修复，不引入新的调度器全局门禁。

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

| Promise | Coverage item | Ticket | Test owner |
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
| P36 | Windows statechart 的 Java/TLC 图、报告、退出与清理 | 08 | 08 |
| P36 | Windows CLI/UI 交付代码的明确动作 | 07 | 07 |
| P36 | Windows 日志／审计交付代码的明确动作 | 09 | 09 |
| P36 | Windows forge 辅助代码的明确动作 | 10 | 10 |
| P36 | Windows 视频交付代码的明确动作 | 11 | 11 |

所有未列出的 promise 由其单一 Delivers 票独占测试／内容审查。H1 原始材料与回传复核由 03 负责，H2 由 10 负责，14 汇总状态而不重复测试或代替用户确认。
