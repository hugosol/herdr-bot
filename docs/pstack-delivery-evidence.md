# pstack 交付包与分层证据

黑盒验收代码已迁至独立项目 `herdr-bot-tester`。本页验收命令使用显式设置的 `$TESTER` 与 `$BOT`，不要求两个项目相邻。产品构建与自身回归仍从产品根运行。

```powershell
$TESTER = 'E:/Developer/myhub/herdr-bot-tester'
$BOT = 'E:/Developer/myhub/herdr-bot'
$env:HERDR_BOT_WORKTREE = $BOT
```

验收设施的 `scripts/` 路径现属于验证项目。参数、授权、覆盖及非零退出约束不变。完整入口见验证项目 `README.md`。

从本次 herdr-bot worktree 根运行（Python 3.11+；参数必须为绝对路径，证据目录必须不存在）：

```powershell
python "$TESTER/run.py" --herdr-bot-worktree "$BOT" check-pstack-delivery `
  --toolkit-worktree $Toolkit --evidence-dir $NewPackageEvidence
python "$TESTER/scripts/check-pstack-delivery.test.py" $Toolkit
```

检查器不调用生成器、不联网、不启动宿主或读取用户配置；只创建指定证据目录和 `summary.json`。以 toolkit 的 `pstack-suite/manifest.json`、所有权账本和独立审查的 `delivery-closure.json` 对照实际文件；读取各文件、计算摘要，并再次从引用目标读取内容（重复目标共用本次读取）。缺失、不可读、未发布、字节漂移、来源错配和非清单路径不能靠文件名通过。`0` 仅代表包检查；`1` 保留失败明细；`2` 是用法或无法创建新证据目录。读到语义规则不等于执行过规则。确认后的本地模型输入、角色清单和 persona 是 setup 产物，不是未配置包的固定内容；Java/TLC、渲染、CLI/UI、forge 等外部前提不由离线检查认证。

Suite 可增加或重新组合 extensions、skills、MCP、LSP 等资源。包检查只记录当前 `pstack-omp` 声明，不要求字段内容、资源路径、顺序或数量等于某份固定配置；包检查通过也不认证 Suite 功能完整。功能判据由[原生专项](pstack-suite-native-acceptance.md#场景和证据)承担，包括实际技能读取及来源、固定工具调用、模式切换与恢复、角色执行等。缺少相关能力时，由对应场景失败或保留未验证，不用配置相等代替行为证据。发布文件缺失、引用断裂与来源漂移仍按包完整性要求拒绝。

维护者应分开记录：**生成**（S1 实际维护命令与字节/副作用）、**内容审查**（固定来源、具名适配、审查结论）、**装配**（S2 业务界面和本次实际源码的宿主结果）、**原生机制**（S3 会话、工具、端口、文件、解码和清理）以及**人工**（仅 H1/P16–P17 与 H2/P38）。两个仓库的 canonical 命令、隔离选项、安装版本、生产后端/安全壳之分和 13 专项见[跨仓原生验收](pstack-suite-native-acceptance.md)。新报告需保留每次命令、cwd、实际源和产物身份、输入及版本、退出、原始日志和已发生副作用；缺格/旧源码/过滤/外部环境缺失分别列出，不能由另一层的绿项补足。模型自主选择、评分、综合、授权遵从和输出质量不在通过范围内。

## P4/P11 内容审查：固定来源与交付边界

对照 `cursor/plugins` 提交 [`aaeb0ea3a0584995aa8f55ce44c6c5259414b896`](https://github.com/cursor/plugins/tree/aaeb0ea3a0584995aa8f55ce44c6c5259414b896) 中两份 `.cursor-plugin/plugin.json` 与两份 MIT 许可：pstack 0.15.46（Lauren Tan），Cursor Team Kit 1.2.0（Cursor）。按清单逐文件与该修订的原文比对；版本与资产选择由发布清单确定，**不是调查中的技能数量承诺**。其中 pstack LICENSE 和 `in-your-own-words` 的旧快照仅有 CRLF 差异；本次恢复发布原字节，生成文本及映射不变。`delivery-closure.json` 的 `selected_tree_sha256` 用发布快照（不使用生成器）的选定路径排序、按 `path<TAB>sha256<LF>` 串接，对 UTF-8 串取 SHA-256；基线变更时从新的独立发布源更新，不从本地生成目录推导。各资产在离线报告中含源/交付摘要及具名映射和理由。

- pstack 核心路由、隐藏入口、原则、playbooks、worker 和 persona template 保留阅读路径；同包的 TSV 模板、Notes 示例子页、TS 模式资料、Bugbot triage、日志和 worktree audit、watcher 和 statechart 的 TS 依赖随交付读取。角色绑定需要显式 setup；缺绑定不应以占位 persona 顶替。发起时的真实 URI、相对路径和角色消费以原生专项为准。
- Cursor Team Kit 只交付三项。`deslop` 原文不变，仅用目标项目 Git diff，无单独 helper；`control-cli` 和 `control-ui` 的具名映射保留使用真实本地驱动、先后取证与清理要求，提供技能正文内的 ConPTY / Playwright 探针。需要的已安装工具、项目命令/页面和临时脚本/产物没有被误列为缺失的随包文件。
- `statechart` 的运行脚本与报告资料在包内；TLC/JVM 在包外，项目的 TLA+ 输入及所生成图/报告也不在包内。视频方法在包内，FFmpeg/FFprobe/字体和项目的幻灯片、字幕、视频在包外；没有捆绑渲染器。PR watcher 是包内只读工具，forge 操作和权限在包外；worktree audit 可能 fetch，不能写成“无副作用”。
- 待迁移说明区分 Benny、云 VM/云接管、Slack/Grok 事件与 recall/学习增强；文档里引用它们的上游路径不是运行时路由。当前安全暂停、WIP、日志和明确本地材料续接仍保留，不声称可代替云恢复或历史检索。更新前先停用正在使用同一包的会话，更新后重新发起；此检查器不是实时装配或权限沙箱。

## Promise → 所有者及证据入口

以下是取证路由，不是通过清单。路径 `tests/pstack/...` 与 `pstack-suite/...` 在 Toolkit；Bot 命令和测试在 herdr-bot。`13` 指[原生专项及独立场景](pstack-suite-native-acceptance.md#场景和证据)。按这张表将本轮实际原始证据附到承诺；若源或环境改变，不以旧观察替代本轮证据。人工记录必须来自操作者本人。

| Promise | Owner | 测试／审查入口与该层应保留的实际证据 |
|---|---|---|
| P1 | 12 | Task 选择/保存/启动的 Bot 前后端兼容性回归；公开状态与 Herdr 启动对照。 |
| P2 | 13 | `assembly`：同名异源对照、原生目录及真实读到的技能正文。 |
| P3 | 13 | `assembly`：原生提示、项目指令、固定 `/budget` 与状态上报。 |
| P4 | 14 | `check-pstack-delivery.py` 实际清单/许可证/映射摘要；本页与上游归档比对。 |
| P5 | 01 | `maintenance.test.ts`：有变换的构建前后快照和 stderr/退出。 |
| P6 | 01 | `maintenance.test.ts`：具名 preview、差异、失败诊断；本次包报告带操作理由。 |
| P7 | 01 | `maintenance.test.ts`：独立锚点/冲突输入、退出、已完成的文件效果。 |
| P8 | 01 | `maintenance.test.ts`：独立输出、两次生成和手改对照。 |
| P9 | 01 | `maintenance.test.ts`：一致/不一致检查前后实际文件及目录。 |
| P10 | 01 | `maintenance.test.ts`：独立 upstream 比较和 overlay preview。 |
| P11 | 14 | `check-pstack-delivery.py`：实际引用目标、嵌入探针摘要；本页分类与外部前提审查。 |
| P12 | 01 | `maintenance.test.ts`：独立文本/二进制读取及换行边界。 |
| P13 | 01 | `maintenance.test.ts`：成功和失败时非归属文件仍可读。 |
| P14 | 13 | `assembly`：只读资源、调用记录、源前后指纹及本地网络对照。 |
| P15 | 03 | `persistent-mode-omp-dock.test.ts`：首轮真实请求/贡献。 |
| P16 | 03 | 同上，开关事件与各轮请求；H1 还须用户确认实际 TUI。 |
| P17 | 03、13 | 同上及 `mode-resume-on/off`：保存值、跨进程历史和公共 Resume 首轮；H1 由用户确认。 |
| P18 | 03 | `persistent-mode-omp-dock.test.ts`：显式单次 Skill、下一轮/恢复时的状态。 |
| P19 | 02 | `core-routing.test.ts` 及上游/映射/交付正文审查，保留方法步骤。 |
| P20 | 03、13 | `persistent-mode-omp-dock.test.ts` 与 `source-base`：含空格源、不同 cwd、主/子真实读取。 |
| P21 | 03 | `persistent-mode-omp-dock.test.ts`：默认目录、手工入口/URI、文件读取。 |
| P22 | 05 | `swarm`/`arena`/`interrogate` 上游、mapping、输出的内容审查；不是自主评分检验。 |
| P23 | 05 | `parallel-panels-omp-dock.test.ts`：真实 task 结果、独立 checkout/文件与 Git 回读。 |
| P24 | 04、13 | `setup-personas-omp-dock.test.ts` 与 `role-lifecycle`：空绑定真实入口/读取。 |
| P25 | 04 | `setup-personas.test.ts` 及 setup 内容审查：明确输入、取消、生成、错误和读回。 |
| P26 | 04 | `setup-personas-omp-dock.test.ts`：实际父/子模型、推理等级。 |
| P27 | 04 | 同上：确认生成、原生角色名与实际 task 路由。 |
| P28 | 04、13 | 同上及 `role-lifecycle`：配置前后、其他 Suite 模型对照。 |
| P29 | 05 | `parallel-panels-omp-dock.test.ts`：独立席位及对应实际效果。 |
| P30 | 05 | 同上及面板内容审查：明显覆盖、不可用、未知和实际身份事实。 |
| P31 | 04、13 | `setup-personas-omp-dock.test.ts` 与 `role-lifecycle`：确认、停用更新、重启并消费 A/B。 |
| P32 | 04 | 同上及 persona 源/映射/正文审查：执行与审查职责和原生工具结果。 |
| P33 | 07 | `external-drivers.test.ts` 和外部技能内容审查：实际 CLI transcript、UI 截图、进程退出。 |
| P34 | 06 | `verification-methods.test.ts`、`verification-methods-omp-dock.test.ts`：真实资源和项目文件读取、步骤审查。 |
| P35 | 06 | `verification-methods.test.ts` 及上游/映射审查：方法/失败口径。 |
| P36 | 07、08、09、10、11 | `external-drivers.test.ts`、`windows-statechart.test.ts`、`logs-audit.test.ts`、`forge-delivery.test.ts`、`video-render-omp-dock.test.ts`：真实 Windows 动作、TLC 图/报告、退出及清理分别附录。 |
| P37 | 09 | `logs-audit.test.ts`：TSV 字节、真实 Git refs、fetch 副作用与缓冲区。 |
| P38 | 10 | `forge-delivery.test.ts`：受控 forge 请求、真实自有 Git 状态；H2 按自有仓库方案由用户确认。 |
| P39 | 10 | 主/子角色与 PR/Autopilot 具名映射内容审查，比较实际交付与原版许可语义。 |
| P40 | 03 | `persistent-mode-omp-dock.test.ts`：模式开关前后无业务写入/外部动作的对照及授权文本审查。 |
| P41 | 09 | `logs-audit-omp-dock.test.ts` 与安全暂停/续接原文审查：真实读到的日志/WIP/交接指引。 |
| P42 | 11 | `video-report.test.ts`、`video-render-omp-dock.test.ts`：上游方法审查和真实 MP4/信息/解码帧。 |
| P43 | 02 | `core-routing.test.ts` 和 `MIGRATION-BACKLOG.md` 范围审查。 |
| P44 | 02 | `core-routing.test.ts`、`core-routing-omp-dock.test.ts`：候选/隐藏/暂缓技能区别及真实可达性。 |
| P45 | 14 | 本轮包报告、原始运行记录和逐项证据索引；失败、未运行、H1/H2 独立标记。 |
| P46 | 13 | `resource-failure`：公开发起/恢复错误与实际宿主未启动、无读写/请求的对照。 |

H1 的可复现 TUI 准备材料在 Toolkit `pstack-suite/README.md` 的 H1 小节，包括 marker 显示和实际退出/恢复观察。H2 由 10 提供自有测试仓库、受控结果、真实 forge 指令、预期副作用和采集/回传步骤；只运行明确获授权的测试仓，不接触生产仓库或以机械结果代签。两项在用户确认前均为未验证。
