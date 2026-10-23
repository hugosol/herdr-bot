# 待人工确认的验收边界

下列事项在原交付记录中尚未获得用户确认；文档精简和 Bot／Chat 改名不将它们改记为通过。

- **真实 agent 加载集合**：在隔离 zone 核对 OMP／Pi 实际加载的固定与可选扩展，并以独立 ambient 对照确认自动发现关闭。受控 Herdr 的参数记录不能替代实际加载结果；Pi 信任提示需由用户处理。
- **实际部署的原生存储与历史**：保留当前 agent 的存储和 autoResume 设置，以本次 Chat 对应的精确 Herdr 原生引用核对完整 ID、cwd 和历史记录；不得用最近文件或同名对话代替身份。核对原生发现时只查看列表，不恢复或向已有对话提交输入。
- **真实手机操作**：需要真实手机确认触摸、中文输入法、软键盘、滚动及错误修正，不以桌面窄屏截图代签。
- **真实断电**：自动检查不证明整机实际关机；只有用户确认可接受影响、保存好工作后才能进行。

此前项目改名材料已记录手机局域网访问验收完成；这不等于 Bot 编辑与发起 Chat 的真实手机操作已验收。

历史截图、原始日志和 JSON 保留在 [`evidence/`](evidence/)；它们沿用当时的命名，不是新接口的导入数据或当前使用说明。此次整理不重写历史票据或原始证据，也不复用过去的通过数宣称当前版本已验证。

## 历史 Chat Resume：07 验收交接（2026-10-23）

### 自动验证与边界

批准的 seams 仍是 S1 交付页面、S2 公开 HTTP、S3 Herdr 公开控制。新增 `frontend/browser-tests/chatResumeContract.spec.ts` 的 Pi／OMP 组合用例使用独立预写的三个历史身份（含同名同 cwd 反例）、真实原生样例文件、真实服务进程及重启。恢复前后、正常 register、unregister 和重启后都从公开历史接口核对固定身份集合与首次时间；不以当前登记存在为资格。已有正确行为直接验证，没有为了制造 red 改坏产品。

同一组合用例将页面与独立 HTTP 请求的定位／失败结果对照 Herdr 的真实夹具记录及 `focused.json`，核对新进程恢复的真实启动目标、最终聚焦与无输入。只替换外部 Herdr，不替换生产 adapter、页面、HTTP、文件或重启。它们不是原生加载证明。

实机还发现 Herdr 0.9.5 的结构化启动超时被误标为确定失败：真实返回 `error.code: "timeout"`、`message: "timed out waiting for agent startup"`。新增跨路径回归先失败（预期 `unconfirmed`、实际 `failed`），修正 adapter 保留结构化超时语义后通过；不通过搜索错误文字来分类，也不改变普通拒绝／查找／聚焦的失败语义。

本次实际运行：

- `npm ci`、`npm run build`：成功，构建交付页面。
- `cargo build --bins --example safe_shutdown_shell`：成功，构建交付进程及外部夹具。
- `npm test`：27 文件、205 测试通过；网络策略检查通过。
- `cargo test --tests`：439 通过、5 个既有人工网络检查忽略；不把忽略项记为通过。
- `npx playwright test browser-tests/chatResumeContract.spec.ts browser-tests/chatResume.spec.ts browser-tests/chatResumePi.spec.ts browser-tests/chatResumeOmp.spec.ts browser-tests/chatResumeConcurrency.spec.ts browser-tests/chatResumeTimeout.spec.ts browser-tests/historyWorkspace.spec.ts`：12 通过（交付页面、公开 HTTP、真实重启与生产 CLI adapter）。

复用覆盖，不重写其他票同一路径测试：

| 自动覆盖 | 现有检查／本票组合检查 |
| --- | --- |
| P1–P5、P20、P23、P24 | `chat_resume.rs`、`chatResume.spec.ts`，及 `session_write_boundary.rs` 的默认自动来源拒绝检查 |
| P6、P8–P15、P22 的 Pi／OMP 自动分项 | `chat_resume_pi.rs`、`chat_resume_omp.rs`、`chatResumePi.spec.ts`、`chatResumeOmp.spec.ts` |
| P7、P8 删除位置、P9 原位置查询失败、P21 基础收录／旧格式 | `history_workspace.rs`、`historyWorkspace.spec.ts` |
| P16 | `chat_resume_concurrency.rs`、`chatResumeConcurrency.spec.ts` |
| P17–P19、超时并发、P22 超时呈现 | `chat_resume_timeout.rs`、`chatResumeTimeout.spec.ts`；本票另覆盖实机发现的结构化启动超时 |
| P21 Pi／OMP 恢复→接入→读取→移除→真实重启；P22 跨路径一致性 | `chatResumeContract.spec.ts` 的两个身份集合组合用例及两个结构化超时用例 |

这些结果完成自动覆盖（包括 P21 的本票组合分项），不把 P6、P22 或引用人工证据的承诺整体标为通过。下述 H-Pi／H-OMP 仍必须由用户明确确认；它们是同一轮的两个人工项，不追加手机／断电等其他验收范围。

### 两项专用真实环境材料

原始证据：[`native-resume.json`](evidence/chat-history-resume/native-resume.json)；截图：[新进程恢复](evidence/chat-history-resume/native-new-processes.webp)、[OMP 已有目标定位](evidence/chat-history-resume/native-omp-located.webp)、[Pi 已有目标定位](evidence/chat-history-resume/native-pi-located.webp)。JSON 保留实际启动命令行、原生内容、前后 Herdr 状态、页面文本及 HTTP 收据，不只是参数形状或非空结果。

隔离目录：`C:\Users\hugos\AppData\Local\Temp\herdr-native-storage-vysibcs5`。专用 Herdr session：`resume07-20364`；唯一验收 Workspace：`w1`／`resume07-dedicated`。捕获时专用页面是 `http://127.0.0.1:1444/`；端口重启可能变化，按该专用服务日志为准。仅此目录及专用 session 被操作；未关闭、重启、提交输入或编辑用户现有 Chat。初始 PowerShell 无法通过 `Start-Process omp` 启动的失败目标保留供检查；随后只为此专用 session 选择 `cmd.exe`，没有改用户 Herdr 配置。

| 人工项 | 原生身份与已知内容 | 已准备的目标与状态 | 用户确认记录 |
| --- | --- | --- | --- |
| H-Pi | Pi 1.4.0；`01a1672d-b70b-72d3-b25d-c96f455256c7`；文件在 `pi/nondefault-native-sessions/2026-10-23T05-13-31-659Z_01a1672d-b70b-72d3-b25d-c96f455256c7.jsonl`；cwd 为隔离目录的 `pi/zone`；唯一旧输入 `Acknowledge this isolated storage test.`，已知回复 `isolated storage acknowledgement` | 无匹配后页面 `restored` 到 `w1:t6`／`w1:p6`；再从另一个专用目标定位到同一目标，页面 `located`，Herdr 报告 focused；实参使用 `--session` 精确文件 | **未验收；尚无用户确认** |
| H-OMP | OMP 18.12.6；`01a16737-5b3b-76f7-ab1b-05324809594b`；文件在 `shared-agent/sessions/-tmp-herdr-native-storage-vysibcs5-omp-zone/2026-10-23T05-24-03-515Z_01a16737-5b3b-76f7-ab1b-05324809594b.jsonl`；cwd 为隔离目录的 `omp/zone`；旧输入 `H-OMP dedicated interrupted task: reply only RESUME07-PENDING-ACK.`，已完成 Read 工具读取 `RESUME07-INTERRUPTED-TOOL-READ` 后，在下一次模型请求期间中断专用 seed 进程 | 无匹配后页面 `restored` 到 `w1:t5`／`w1:p5`；再定位到同一目标，页面 `located`，Herdr 报告 focused；实际命令是 `--resume`，无 `--restore` 或新输入；恢复和定位后原生文件 SHA-256 未变，专用 loopback 模型请求仍是 seed 的 2 次 | **未验收；尚无用户确认** |

OMP 的文件首行是原生 title 元数据，第二行才是 session header；证据记录的是完整原生 ID，不按最近文件猜测。JSON 中早先 `H-omp dedicated old Chat` 是完成状态的准备样例，**不是** H-OMP 的中断任务验收目标。Pi／OMP 的真正历史内容来自安装的原生 CLI；loopback 模型只生成专用样例，并不证明真实外部模型质量。初次未隔离 native 配置时观察到模型回退警告，最终捕获使用专用 `shared-agent` 原生配置；不把任何 agent 自身的模型恢复／autoResume 规则扩大为本项目保证。

### 一次性交付用户的重复步骤（两项均执行 A、B）

1. 只附着专用 session：`herdr session attach resume07-20364`。在另一个浏览器打开上述专用页面，进入历史列表。先核对原生 ID、精确文件 header、cwd 和已知历史内容。不要操作平时使用的 Chat、默认 session 或其他 Workspace。
2. **A：已有目标定位。** 先在该专用 Herdr 客户端切到另一个专用目标，然后对表中对应历史行点 Resume。核对页面 `located`、实际 Workspace／Tab／Pane 与 `herdr --session resume07-20364 agent list`／`agent get <返回的 Pane>` 一致；客户端确实切到该位置，未另开 Tab。通过 `agent read <Pane> --source recent-unwrapped --lines 120` 和原生文件确认旧内容与输入没有增加。不得把创建 Tab／提交命令或 Windows 置前当作通过。
3. **B：无匹配后新进程恢复。** 重新执行专用 `agent list`，确认表中 ID 仅在本次专用目标；只关闭该专用历史对应的 Tab（`herdr --session resume07-20364 tab close <刚核对的 Tab ID>`，不要照抄过期 ID），不关闭其他 Tab。再次点该历史 Resume。核对新返回的 Workspace／Tab／Pane、原生 ID／文件、cwd、实际进程参数与焦点，看到相同旧历史且没有新输入，页面只有确认身份及聚焦后才显示 `restored`。Pi 必须使用 session；OMP 必须使用 resume 而非 restore。
4. **H-OMP 额外观察属于同一项。** 该专用任务在 Read 后中断；Resume 后应保留这段旧历史、没有新 assistant／tool 任务活动，仍等待输入。核对唯一旧 user message 与文件未新增内容；不能以“参数里没有 restore”代替看到任务未继续。H-Pi 同样核对没有新输入或自动任务活动。
5. 两项分别反馈“原生对话正确／实际焦点正确／无额外输入／新目标加载旧历史”；H-OMP 再明确反馈“未自动继续中断任务”。如有不同，保留目标和实际输出，不重发输入或自动重试。用户明确确认前，两行始终为未验收；P6 及其引用的真实分项、P22 的真实分项也不据准备材料关闭。

若专用服务已退出，先确认 **仅此命名 session** 不在运行，再在新的专用终端设置 `HERDR_CONFIG_PATH=<隔离目录>\herdr.toml`、`PI_CODING_AGENT_DIR=<隔离目录>\shared-agent`，清空继承的 `OMPCODE`，将 `HOME`／`USERPROFILE` 指向 `<隔离目录>\home`，启动 `herdr --session resume07-20364 server`；另一专用终端运行隔离目录中的 `herdr-bot.exe --config <隔离目录>\herdr-bot.toml`，使用日志的新端口。专用配置禁止 Herdr server 重启时自动恢复 agent，未匹配路径仍由页面 Resume 显式发起。seed loopback 模型不是普通模型服务且不要求用户向它提交任务；不再运行时不能把请求数继续不变当作新的观察证据。如果隔离目录已被清理，缺少的就是这两个专用原生样例及配置，须重新准备，不借用用户现有 Chat 替代。
