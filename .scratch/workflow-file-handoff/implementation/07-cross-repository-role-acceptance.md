# 07: 跨仓角色工具、完成门禁与等待闭环验收

**What to build:** 配套 herdr-bot 和 toolkit 在隔离实际宿主中完成真实角色注册、初始化、原生身份注入、report、完成拒绝、collect 等待、新报告以及 manager 显式推进。验收既观察工具和落盘事实，也观察角色实际行为，不以提示词存在、受控 adapter 或 rendered completed 替代证据。

**Delivers:** P7, P8, P17, P20, P21, P22, P26；P4, P5, P6 的真实 Chat 覆盖项。

**Blocked by:** 03: 从当前 execution 登记解析上游文件输入；05: toolkit 新 report 工具与公共角色职责切换。

**Repositories:** herdr-bot, toolkit

**Status:** ready-for-agent

## 跨仓 owner 与范围

依据 [contract](../contract.md) 与 [spec](../spec.md)。本票是 S1/S2/S3 跨仓联调／验收 owner，可修改列出的两个仓库来完成所属场景，拥有这些跨仓测试的编写与维护。不接管 01–05 应完成的调用方迁移，不新增测试专用产品入口或另一套运行记录。

本票为普通实现槽位票，阻塞票首次合并并满足既有调度规则后即可调度，不要求等待它们的后台集成另设终局门禁。

| Promise | 本票唯一拥有的覆盖项 | Test owner | 其他覆盖归属 |
|---|---|---|---|
| P4 | 实际初始化内容进入原生 Chat，包含背景／业务 input／地址，不含敏感正文／聊天 | 07 | 后端初始化：01、03 |
| P5 | 原生 Chat 实际收到当前执行登记地址，不接受误导正文／旧执行地址 | 07 | Run／execution 组合解析：03 |
| P6 | 原生 Chat 实际收到缺失逻辑输入信息且已启动，无伪造地址 | 07 | HTTP 派发行为：03 |
| P7 | 原生 report 可发现、可调用；true/false、缺字段、错类型、省略 outputs，工具与后端一致 | 07 | 05 贡献实现和既有测试迁移，不另认领本覆盖项 |
| P8 | 实际工具公共 Session 注入与报告真实归属一致 | 07 | 后端非法身份及旧执行：02 |
| P17 | report(true) 不自动推进，原生 manager 显式 complete／dispatch 的真实顺序 | 07 | 后端状态／无自动效果：04 |
| P20 | 实际 manager collect 等待及新报告唤醒，保留待办不导致反复即时返回 | 07 | 超时／取消／状态变化矩阵：04 |
| P21 | 实际 node 指引注入和工具行为；外部程序产出；阻塞／部分效果，报告不复制成果全文 | 07 | 无 |
| P22 | 实际 manager 读报告作编排判断，不默认读输出正文；拒绝后等待、不反复 complete | 07 | 无 |
| P26 | 配套 toolkit 新调用成功、旧形状新调用失败且无新增报告 | 07 | 后端新请求与历史读取区分：02 |

## Acceptance criteria

- [ ] 从两个参与 worktree 的源码构建／加载实际后端与角色入口，分别记录根目录、源码状态、依赖和宿主版本，不误用默认同级旧仓或已安装旧后端。
- [ ] 对 OMP/Pi 受影响工具参数保留实际宿主 dock 覆盖，防止类型强转／删字段后执行。隔离真实 Herdr 与实际模型完成角色行为链路；明确记录真实运行使用的宿主及版本，不把一个宿主结果外推为所有宿主通过。
- [ ] 不使用手填 Session 身份或手改运行记录完成验收。记录原生 Session、Run、node、execution，核对公共 Session 注入、实际 HTTP 和保存报告的归属。
- [ ] 用独立背景、业务 input、地址及敏感正文标记核对实际原生 Chat 输入；同时覆盖已解析上游地址和依赖满足但输出缺失仍启动的场景。
- [ ] 原生工具可发现并实际调用新 report；严格覆盖 true/false、缺 can_complete、错误类型、省略 outputs 和旧形状新请求。非法调用无报告落盘；不能仅断言 schema 或请求转发。
- [ ] 完整观察先 false 后 true 的实际角色场景；另独立触发一次 complete 门禁拒绝，manager 看到明确原因后用已有 collect 等待，不对同一报告反复尝试。不得用受控 provider 预设调用冒充模型遵从职责。
- [ ] collect 已检查既存通知且仍有 running node 时真正进入等待，同一 pendingReports 保留不造成重复立即唤醒；新报告到达后唤醒，保留调用起止与报告 ID 关联。
- [ ] node 使用外部业务程序生成独立文件，详细成果仅在文件中；报告包含必要结论、阻塞、部分效果或干预信息。实际公共指引注入及报告内容联合证明职责，不固定措辞。
- [ ] manager 阅读报告而不默认读取输出正文验收；true 后仍显式决定 complete 和 dispatch，无自动推进。保留完整对话和工具次序，不用最终状态推断过程。
- [ ] 每个 Promise 覆盖项关联具体原始证据；缺真实依赖、无法隔离或模型访问失败时明确阻塞，不跳过、不降低承诺、不以替身结果标通过。实际业务确认由隔离场景明确输入，不依赖用户手工验收代替本票自动验证。

## Worktree checks — 必须分别通过

命令从对应仓库 worktree 根执行；以 HERDR_BOT_WT、TOOLKIT_WT 表示调度器分配的绝对 worktree 根。所有定位必须显式关联参与源码。

1. **herdr-bot：** `cargo test --manifest-path backend/Cargo.toml --test workflow_start`、`cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch -- --nocapture`、`node scripts/test-backend.mjs`。为 dock 构建 `cargo build --manifest-path backend/Cargo.toml --example safe_shutdown_shell --bin herdr-fixture`；真实宿主后端构建 `cargo build --manifest-path backend/Cargo.toml --bin herdr-bot`，使用该 worktree 且尊重独立 CARGO_TARGET_DIR 的产物。
2. **toolkit：** 在检查进程中设置 `HERDR_BOT_REPO=HERDR_BOT_WT`，运行 `bun test tests/workflow-chat`，受影响角色／collect／取消 dock，`bun run typecheck` 与 `bun run test`。依赖、Bun 版本及运行协调遵循该仓测试规范，不能用裸全仓 bun test 替代脚本入口。
3. **本票专用真实场景：** 在 herdr-bot 建立可重复的自动验收入口并运行 `python scripts/test-workflow-file-handoff.py --backend <HERDR_BOT_WT的本轮后端可执行文件> --toolkit <TOOLKIT_WT> --scenario roles --paths-file <隔离的依赖根配置>`。该命令是本票要实现并文档化的交付入口，不是假称当前已存在；复用既有真实宿主隔离设施，必须执行上述场景、检查预期、保留证据并在失败／缺依赖时非零退出，不得只输出准备信息。

两个仓库分别记录命令、退出码和所测源码／环境；全局回归不能替代专用场景。专用入口支持显式实际依赖路径，隔离配置不读取／继承用户业务数据和 Session。不同 worktree 的构建目标目录独立。保留原始输入、工具参数／结果、HTTP、原生历史、实际文件、只读记录副本和过程证据；说明任何有界无重试窗口。正常和失败均清理所属进程并保留原错误／清理错误，不能关闭用户服务、真实关机或擅自启用 LAN/mDNS。

## 文档与完成标准

更新两个仓库所属长期运行／验证说明，使入口可脱离临时 ticket 重跑；不得将临时证据链接变成长久文档依赖。实际通过 S2 的上述覆盖项和各仓检查后才能关闭本票，不以搭建 runner、提示词检查或受控 adapter 冒烟代替完成。
