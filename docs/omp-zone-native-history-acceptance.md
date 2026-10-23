# OMP zone 原生历史收录与诊断

## 交付行为

创建成功仍仅表示 Herdr 已确认 OMP 可交互、且一次合成初始指示的提交获确认；不等待模型轮次。
在成功状态发布后，服务端独立执行原生身份收录，不要求模板勾选 connect-bot。
页面关闭、停止观察或创建弹窗关闭均不取消收录，也不增加历史专用页面轮询。
启动失败或提交未获确认不产生 zone 来源历史；同一真实原生 ID 以后成功 register 仍可收录。

身份查询使用本次返回的独立 agent 控制名执行只读 `herdr agent get <agentName>`。
只接受官方 `agent_session` 中 `source=herdr:omp`、`agent=omp` 的精确引用，不能从 Tab、Pane、
创建短 ID、别名、cwd 或最近文件推断原生 ID。精确 `id` 引用使用该 ID 与 Herdr 上报的 cwd；
精确绝对 `path` 引用只读取该文件的必要 metadata prefix。OMP 的物理首行可以是 `type:title`，
随后 `type:session` header 才提供完整 `id` 与 `cwd`。不根据相似 JSON 格式猜 agent 类型，
不扫描目录、不解析或导入正文、不改写原生文件。读取前缀有 64 KiB 上限。

身份获取共用一个最多 30 秒的期限，包含外部查询和暂不可读 header 的等待；
期限内只做有界只读身份获取。成功取得身份后只尝试保存一次，失败即结束，不后台重试写入或自动 connect。
期限后不再发起获取；已经进入的内核文件 I/O 不能保证强制中断，但返回后不得继续后续读取。
这个预算既不是模型轮次时限，也不强制取消任意内核磁盘操作。

收录只操作持久历史，不建立当前登记、不改变 keepAlive、工作状态、完成后关机监听或倒计时。
同一完整原生 ID 与 register 合并，Bot 名称优先且跨重启保留；注销当前登记不删除历史。
历史记录不保存临时登记、监听或关机意图。

## 服务端诊断

身份缺失、引用 agent/source 不符、30 秒到期、header 不可读与保存失败均仅记录服务端 warning。
日志含 `zone-history`、`creation=<本次创建ID>`、`agent=<控制名>`、`kind=omp` 和原因；
已取得原生 ID 的保存失败还包含 `session=<完整原生ID>`。
创建结果及页面仍只表达创建本身，不显示历史状态或历史失败提示。
期限边界保留最后一次期限内的具体失败原因，不让最终 ready future 的通用 deadline 错误覆盖
header 不可读等根因。实际交付进程 warning 原文见
[`evidence/omp-zone-history-diagnostics.json`](evidence/omp-zone-history-diagnostics.json)。
用创建响应的 `id` / `agentName` 关联 warning，不能把日志中的控制名当作原生 sessionId。
如果失败后该对话另行成功 connect，register 会按普通接入规则尝试持久化；zone 独立任务本身不重试保存。

## 已覆盖的公开接缝

批准 seam：S1 实际 Chrome 页面、S2 HTTP / SSE / 服务端诊断、S4 真实或受控 Herdr / OMP 交互。
受控 Herdr 是真实外部进程，不替代 Herdr / OMP 真实安装运行证据。

`backend/tests/omp_zone_history.rs`：

| 票据承诺 | 具体测试与观察 |
| --- | --- |
| P01、P05、P25 | `omp_without_connect_records_exact_header_after_title_without_changing_native_files`：title 在 header 前，精确 ID / cwd / Bot 名称独立读回，当前 SSE 为空，原生主文件与较新诱饵旧文件字节不变，正文故意不是 JSON。 |
| P02 | `failed_omp_start_and_submission_do_not_record_zone_but_register_can_record_same_identity`：分别拒绝启动及提交，历史为空且无身份查询；相同 ID 独立 register 后可读。 |
| P17、P26 | `pending_omp_identity_does_not_hold_creation_registration_or_current_state`：未释放真实外部查询屏障时两个创建已成功、register / 当前观察 / 监听仍推进；释放后历史收录不改变当前语义。 |
| P30 | `omp_header_at_29_seconds_is_recorded_before_the_total_budget_expires`：受控生产时钟推进至 29 秒，随后写入精确 header 仍收录。 |
| P22、P26、P30 | `omp_total_deadline_ends_missing_unreadable_and_pending_identity_without_changing_current_intent`：缺身份、header 暂不可读及外部查询未结束均在 30 秒结束；期限后恢复身份不再查询或保存，当前登记 / keepAlive / 监听不变。 |
| P22、P26、P30 | `omp_save_failure_is_one_shot_and_does_not_persist_current_shutdown_intent`：实际隔离文件系统保存失败，恢复可写后无自主重试；原历史可读，重启后历史存在而当前及监听为空。 |
| P26 | `omp_history_outcomes_do_not_retime_or_persist_an_existing_countdown`：正常收录、身份到期、保存失败分别保持既有倒计时的 arrangementId / deadline；隔离重启不恢复倒计时。使用一小时期限及 panic-only power adapter，绝不执行真实关机。 |

TDD 证据：首次 OMP tracer 在创建成功后因历史仍为空而 red；首次真实浏览器注册后将 Bot 名称覆盖为
connect 名称而 red，共享持久名称优先级修复后对应观察 green。定向临时 mutation 将 OMP title 行当作
首行即终止，原 tracer 再次因历史缺失而失败；恢复正确 prefix 读取后全部 7 个 OMP HTTP 测试通过。
这验证 assertion 能辨别“只读首个物理行”的实际违规，而不只是增加覆盖标签。

`frontend/browser-tests/ompZoneHistory.spec.ts`：实际交付进程与 Chrome 页面核对页面关闭后的独立收录、
不接入时只在历史、接入后当前和历史同时具有该对话、注销后历史保留，以及 zone 名称优先。
另覆盖期限前 header 恢复、缺身份 / 错 agent / header 不可读的实际 30 秒到期、真实保存失败；
收取创建关联的后端 warning 为测试附件，确认页面无历史失败提示、到期或保存失败后不再自主查询 / 写入。

## 验证命令

- `cargo test --test omp_zone_history -- --nocapture`：7 个 HTTP / SSE / 受控时间用例通过。
- `npm run build` 与 `cargo build`：已实际完成，用于交付二进制和打包页面的浏览器验收。
- `npx playwright test ompZoneHistory.spec.ts`：6 项全部通过（真实 Chrome，隔离交付进程，2.1 分钟）。
  注销后仍可读的实际历史页面截图见
  [`evidence/omp-zone-history-after-unregister.png`](evidence/omp-zone-history-after-unregister.png)。
- `cargo test --test session_history -- --nocapture`：既有持久历史 4 项通过。
  首次与浏览器同时编译曾被 Windows 正在运行的 exe 锁阻止；关闭浏览器进程后独立运行成功，
  不是被测历史行为失败。
- 定向 mutation 命令为
  `cargo test --test omp_zone_history omp_without_connect_records_exact_header_after_title_without_changing_native_files -- --nocapture`；
  注入首行即终止错误后失败，恢复后重跑全部 7 项通过。没有保留 mutation 或测试专用生产 interface。

## 隔离真实 Herdr / OMP 证据

[`evidence/omp-zone-native-history.json`](evidence/omp-zone-native-history.json) 保留本次真实运行的精确对象和读回：

- 真实 Herdr **0.9.5** 的显式命名隔离服务器 `history-omp-31316`，真实 OMP **18.12.6**；
  HOME、USERPROFILE、APPDATA、LOCALAPPDATA、agent 目录及设置均隔离，未读取真实用户 auth。
- 公开 zone 创建对象 `bc-0a317d78f343`，Tab `w1:t5`，Pane `w1:p5`，控制名 `bot-f56418837f76`；
  Bot 名称 `OMP isolated history 96772c`，未加载 connect-bot。
- 官方 Herdr `agent get` 返回的精确原生路径及该文件前两行保存在 JSON；第二行 header 的真实完整 ID 为
  `01a1648d-9a3a-7422-969b-1d2ee0a984e7`，独立 HTTP 历史读回同一 ID / cwd / Bot 名称，当前 SSE 为空。
- 仅模型端点为 loopback 合成响应，原生 CLI、Herdr PTY、官方扩展、启动、提交、文件持久化和历史读取均为真实实现。
  使用隔离 dummy key，没有真实用户模型任务、账号或远端 provider 请求；合成响应不证明模型能力。
- 初次全新配置会进入 OMP onboarding；Herdr 的输入提交确认不等于初始化界面已处理任务。
  隔离设置显式关闭启动引导并标记 setup 完成后进行有效运行，未修改真实用户设置。
  `/rename <字面标题>` 虽不需用户模型任务，但在新对话上也可能只有内存身份而没有持久 header，
  因而不能用它保证新会话文件落盘；这些早期运行正确产生可关联超时 warning，没有猜 ID 收录。
- 隔离服务已停止，仅保留证据与隔离目录。此项不是用户实际部署 H1。

原生存储选择 / 原生发现的 P23-H1 证据继续见
[`native-session-storage-acceptance.md`](native-session-storage-acceptance.md)，不以本票的收录代替原生发现核对。

## P01-H1：实际部署最小核对（仍待用户确认）

1. 正常构建交付页面与后端，启动目标实例。保留现有 OMP autoResume、模型及原生存储设置；
   不迁移旧目录、不批量导入、不为了验收写入用户全局配置。确认固定官方 Herdr OMP 扩展入口可读。
2. 在用户认可的隔离 zone，用不勾选 connect-bot 的 OMP 模板创建。由用户选择本就认可的初始指示；
   代理不向用户模型提交任务。如果不允许任何模型任务，应使用已准备好的隔离 loopback 模型环境，
   或用户认可且已正常落盘的对话；不能假定 `/settings`、`/rename` 等无模型命令必定让全新 header 落盘。
3. 保存本次创建成功响应中的 `id`、`name`、`tabId`、`paneId`、`agentName`。
   执行 `herdr agent get <该 agentName>`，核对 `agent_session.source=herdr:omp`、`agent=omp`。
   只对返回的精确 path 只读核对必要 metadata：允许 title 行在 session header 前，记录完整 `id` 与 `cwd`。
   不按文件修改时间选最新文件，也不把创建 ID、控制名或文件名片段当作原生 ID。
4. 独立请求 `GET /api/sessions/history` 或打开 chat 历史，核对相同完整 ID / cwd / Bot 名称。
   未 connect 时当前没有该登记。之后明确 connect 则两视图可同时具有该对话；注销当前登记后历史仍在。
5. 若没有收录，以创建 `id` / `agentName` 收取服务端 warning。
   文件在 30 秒后才落盘不会令原 zone 任务重新启动；不要重发初始指示或重建 Tab来掩盖超时。
   用户之后明确成功 connect 可按普通 register 再尝试收录。记录真实通过 / 失败 / 无法判断及原始证据。

H1 是最终部署确认，尚未标为通过。它不替代上面的受控自动验收或真实隔离运行。
