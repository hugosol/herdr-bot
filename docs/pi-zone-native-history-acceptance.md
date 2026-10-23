# Pi zone 原生历史收录与诊断验收

## 交付行为与边界

批准 seam 为 S1（真实浏览器）、S2（交付 HTTP 与附属服务端诊断）、S4（Herdr／Pi 原生交互）。
Pi 已可交互且合成初始指示提交获得确认即创建成功；不等待模型完成，不以 connect-bot 为前提。
成功后独立任务只查询该次 `agentName` 的官方 `herdr:pi` 原生引用；精确路径仅用于必要 header 元数据。
header 可不在首个物理行，正文不读取／导入；不存在按 cwd、文件时间或文件名猜 ID 的回退。
身份获取总预算 30 秒，包含 CLI 查询和暂不可读 header；取得身份后仅保存一次。
超时／保存失败不改变创建结果、不重发指示、不关闭 Tab、不停止 agent，也不更改当前登记或关机意图。
阻塞 header 读取在下一次文件读取前检查同一个期限，不声称能强制取消已经进入内核的磁盘调用。

同一完整原生 ID 合并为一条历史，zone 名称优先级持久保留；只收录历史不建立当前登记。
register、注销与两个列表仍各自保持原有语义。历史不保存 keepAlive、当前登记、监听或倒计时。

## 自动覆盖及独立期望

`backend/tests/pi_zone_history.rs` 只通过 HTTP、SSE 与真实受控 Herdr 子进程观察：

| Promise／本票分区 | 具体证据 |
| --- | --- |
| P01、P05、P25 | `pi_without_connect_collects_exact_header_and_leaves_current_and_old_files_untouched`：独立预写 ID `pi-native-full-001`，前置空物理行、header 后无效正文；核对完整 ID／cwd／Bot 名、当前空登记、旧原生主文件及二进制附属记录与空目录不变，旧对话不自动入历史。 |
| P02 | `pi_unconfirmed_start_and_submit_never_collect_but_register_still_can`：启动／提交分别明确失败，不查询身份、不收录；同 ID 随后成功 register 恰收录一条。 |
| P04／P09（维护已交付语义） | `pi_zone_name_wins_both_source_orders_and_survives_restart`：connect→zone→connect→重启→connect，始终唯一且 zone 名优先。 |
| P17、P26 正常收录／等待 | `pi_pending_identity_does_not_hold_creation_or_current_state`：身份 query 被外部 gate 挂起时创建已成功；第二次创建、register、state、monitor 及 SSE 均推进；释放后原 keepAlive、监听与无倒计时状态不变，初始指示没有重发。 |
| P30 期限前成功 | `pi_exact_header_can_arrive_before_the_total_deadline`：精确文件尚不存在，推进生产 Tokio 时钟到 29 秒才写 header，仍正确收录。 |
| P22／P30、P26 身份失败 | `pi_identity_budget_ends_reads_and_never_collects_late_identity`：身份缺失、精确 header 暂不可读、外部 query 尚未完成三分区，推进到 30 秒；之后提供身份仍不新增历史或继续查询，创建结果及当前成员／监听不变。 |
| P22／P30、P26 保存失败 | `pi_save_failure_does_not_retry_or_change_current_intent`：真实临时目录占用原子写入目标，失败后恢复可写也不自主重试；current 在线、working、monitor 不变；重启后历史还在，临时登记和意图不恢复。关机边界用 panic 替身，绝不执行真实关机。 |

`pi_history_outcomes_never_cancel_or_reschedule_an_existing_countdown` 另外覆盖成功／身份超时／真实保存失败：
已有倒计时的 arrangementId、deadline、phase 与监听集合不变；使用 600 秒安全观察窗及 panic 关机边界，测试最后明确取消。

首次 tracer 的 red 是创建成功后公开历史为空，不是 import／fixture 失败；实现后精确 ID 读回 green。
名称优先级 red 实际读回 `Connect later` 而非预写 `Pi Zone Name`，修复并验证跨重启 green。
将身份预算临时改成 60 秒，30 秒期限测试实际失败：晚到身份入历史，实际 `total=2` 而非独立期望 `1`；恢复 30 秒后 green。
最初一次 mutation 检查曾因隔离运行中的 EXE 被 Windows 锁定而无法构建；先收取真实运行证据并结束自己的隔离进程，再执行上述有效 mutation，未把构建失败冒充行为 red。

`frontend/browser-tests/piZoneHistory.spec.ts` 的 **5 个真实 Chrome 场景全部通过**（含测试进程树 cleanup 回归）：

- 未 connect 创建先成功，页面没有历史请求或收录提示；关闭页面后释放身份，独立新页面读回历史。
- 未接入仅在历史；明确 register 后当前及历史都能看到同 ID；注销当前登记仍保留历史。
- 身份缺失、精确 header 不可读、真实保存失败三种场景收取交付 EXE 的 stdout／stderr，页面创建结果保持成功且没有历史失败提示；故障解除但没有新事件时不继续获取／写入。
- 受控 Herdr 只替代外部环境，生产 adapter、HTTP、持久化、页面、SSE、浏览器生命周期均真实运行；这些结果不冒充真实原生运行。

实际页面截图：[未 connect 的 Pi 历史](evidence/pi-zone-native-history/pi-history-without-connect.png)。
原始警告留存：[交付进程诊断](evidence/pi-zone-native-history/delivered-diagnostics.txt)。
最后的具体原因保留修复后另运行 `npx playwright test piZoneHistory.spec.ts --grep "unreadable-header"`，
实际 **1 passed**，日志仍包含具体 header 不可用原因，而不是被期限检查的泛化错误覆盖。

### 已收取的交付运行警告

以下来自真实交付进程，不是日志函数替身：

```text
warning: zone-history creation=bc-1588717a4464 agent=bot-836ab7c4c05b kind=pi identity-timeout: Herdr 的 agent get native identity 返回了无法理解的响应：native reference agent/source does not match this creation
warning: zone-history creation=bc-823eb6b20878 agent=bot-0c8eaf548c47 kind=pi identity-timeout: Herdr 的 agent get native identity 返回了无法理解的响应：native header unavailable: 系统找不到指定的文件。 (os error 2)
warning: zone-history creation=bc-f579ec724ff0 agent=bot-63fc637ccbcf kind=pi session=pi-browser-save-failure save-failed: 拒绝访问。 (os error 5)
```

后续 adapter 诊断修正把 `agent_session: null` 精确描述为 `native identity is not available`，不再误称 source 不匹配；两种日志都来自对应版本真实运行，其他关联字段及超时语义不变。

## 已执行验证命令

```powershell
cd backend
cargo test --test pi_zone_history --test session_history --test bot_creation --test bot_creation_detached --test bot_identity --test countdown_independence --test monitoring_recovery -- --nocapture
# 7 suites，68 passed，0 failed；包含恢复 30 秒 mutation 后的最终 green。
cargo build --bins
cd ../frontend
npm ci
npm run build
npx playwright test piZoneHistory.spec.ts templateExtensionLaunch.spec.ts
# 实际 Chrome：26 passed（Pi 5 + 扩展回归 21）。
# 在最后的诊断原因保留修复后：
cd ../backend
cargo test --test pi_zone_history -- --nocapture
# 最新 Pi HTTP 8 passed；新增已有倒计时三分区，前述 68 项回归结果仍单独保留。
```

浏览器扩展回归曾有两例在 cleanup 报 Windows EPERM：原支撑设施只终止 backend 父进程，
尚未结束的独立 Herdr query 子进程继续持有临时 EXE。新增受控 gate 的公开创建场景稳定复现同一 red；
测试支撑设施现仅对自己启动的 backend 使用 `taskkill /PID … /T /F`，等待整棵测试进程树退出再删除临时目录，
不靠忽略错误或文件删除重试掩盖。此调整不影响生产 agent 生命周期。

原创建回归测试将外部总调用列表误当成控制序列，新增只读 `agent_get` 后原断言失败；
现仅从这些“不能重复启动／提交／关闭／改配”断言中排除已批准的只读观察，
保留完整创建控制序列和启动参数的字面断言；身份读取次数／期限由本票专项测试直接约束。

## 隔离真实 Pi／Herdr 链路

[原始 JSON 证据](evidence/pi-zone-native-history/isolated-real-herdr-pi.json) 使用真实安装的 **Herdr 0.9.5、Pi 1.4.0**。
新建命名隔离 Herdr server `ticket05-pi-27344`，HOME／USERPROFILE／APPDATA／LOCALAPPDATA／
`PI_CODING_AGENT_DIR` 和配置均指向临时目录；生产 herdr-bot 通过精确该 server 的 socket 控制对象。
没有连接或控制用户 focused session，没有读取用户 auth，没有向用户已有对话或外部模型提交任务。
模型唯一替身是仅监听 loopback 的固定 acknowledgement endpoint，复用 `scripts/test-native-session-storage.py` 的 Model；
提交的是固定隔离存储探针，不是用户模型任务。原生 CLI、官方 Herdr Pi 扩展、Herdr readiness／prompt、
官方精确引用、原生落盘和独立历史读取都是真实实现。

| 对照项 | 实际值 |
| --- | --- |
| 创建对象 | `bc-a3b76fd9de31`，Tab `w1:t3`，Pane `w1:p3`，agent `bot-2ac19bdc420c` |
| 创建结果 | `succeeded`、`inputSubmitted: true`，未选择 connect-bot |
| 官方引用 | `herdr:pi`／`pi`／`path`，完整绝对路径在原始 JSON |
| 原生 header ID | `01a1648d-82a2-7385-b71b-ffaf6d3c6c28` |
| header cwd | `C:\Users\hugos\AppData\Local\Temp\pi-zone-real-pcvo1p99\zone` |
| 独立 HTTP 历史 | 同一完整 ID／cwd，名称 `Isolated Pi Loopback History`，agent `pi`，恰一条 |
| 有效原生位置 | 隔离 settings.json 的 `sessionDir` 指向 `native-pi-sessions`；创建没有 CLI 目录覆盖 |

真实读取精确 header 后，独立断言其 ID／cwd 与 `GET /api/sessions/history` 一致。
取证后只停止本次自己创建的隔离 backend 与命名 Herdr server；不执行关机，临时目录保留供核对。

另一个真实无模型探针只提交 `/settings`：Pi 立即报告精确文件路径，但尚未形成原生消息时文件不存在。
服务端正确遵守有界等待／不猜身份；因此该无消息探针**不是成功收录证据**，不能把路径文件名当 header ID。
成功收录证据为上表的固定 loopback acknowledgement 创建。

## P01-H1：实际部署最小核对（仍待用户确认）

1. 保持部署当前 Pi 原生配置及官方 Herdr 扩展，不修改 autoResume／用户全局目录。
   使用用户认可的隔离 zone、命名测试 Herdr session 与独立 herdr-bot 数据文件，避免用户现有对话。
   确认 backend 与 Herdr 指向同一 socket，固定扩展包含 `pi-herdr-agent-state`。
2. 在 zone 创建不勾选 connect-bot 的 Pi，记录创建结果的 `id`、`tabId`、`paneId`、`agentName`、`name` 与 zone。
   若不允许任何模型请求，可仅用已确认的无模型命令验证“创建成功、身份等待可超时且页面不报历史失败”；
   **没有实际 header 时不能将该探针记为 P01 收录通过**。要核对完整闭环，可在隔离配置复用上述 loopback 固定响应模型，
   只提交固定存储探针；代理不向用户真实模型或已有任务提交输入。
3. 用 `herdr agent get <本次 agentName>`（始终显式选择本次命名 session／socket）取
   `/result/agent/agent_session`，确认 `source=herdr:pi`、`agent=pi`。只读此精确引用的 session header，
   不遍历旧目录、不选最近文件、不从文件名或 Pane 推断原生 ID。
4. 用独立 HTTP 客户端读取 `GET /api/sessions/history`，核对完整 header `id`／`cwd` 及 Bot 名；
   浏览器切换历史核对同一对象，当前视图应无该 ID。之后若用户主动 connect，两视图可同时存在，注销当前不删除历史。
5. 若到期／保存失败，按创建结果的 `id`／`agentName` 查服务端 `zone-history` 警告；不要为补历史而重发初始指示、
   强制 connect、重启 agent、扫描目录或写原生文件。P23-H1 的存储／发现核对见 `native-session-storage-acceptance.md`。

以上准备不是用户实际部署确认；P01 的 H1 尚未验证。OMP 真实分区与最终 H1 确认不由本票代签。
