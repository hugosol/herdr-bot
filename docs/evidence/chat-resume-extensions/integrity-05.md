# 05：组合降级、权威结果与 Resume 请求边界

日期：2026-10-23。范围仅为 implementation/05，基于已交付的集中降级、最新来源环境、原生默认发现与逐项过滤。

## 交付与测试边界

新增 `frontend/browser-tests/chatResumeIntegrity.spec.ts` 的 **22 个真实浏览器场景**，经过打包页面、公开 HTTP、独立服务进程／重启、真实隔离文件及生产 CLI adapter。只替换系统外部的 Herdr；没有路由拦截或内部 collaborator mock。另扩展既有 `chat_resume::client_can_only_select_history_not_supply_execution_content`，不另造重复的安全测试。

本票的测试切片及独立预期写在各场景前的注释中，批准入口仍为 S1／S2／S3。依赖实现已经满足这些组合行为，按 TDD 的“已有行为先验证”规则保留真实路径并补齐组合覆盖，没有为了产生 red 改坏产品，也没有增设无必要的生产抽象或重试机制。测试首次运行发现两处测试观察错误：历史页面还会独立查询 Workspace 名称，不能把该只读请求当成重复执行；目标 Workspace 文本在历史列和结果中均出现，定位器需限定到有名称的“Resume 目标”。修正观察范围，没有改变合同要求的启动次数、结果或保留目标预期。

### 本票拥有的验收

| 条件 | 具体场景和观察 |
| --- | --- |
| P15：过滤／默认发现分别组合既有回退；确认成功仍明确降级 | `combines exact ID and deleted Workspace`：两 agent × 两环境策略。真实删除原生样例文件、替换受控外部 Workspace 列表，核对 HTTP 恰有 session-file、extension／extensions、workspace 三项，页面逐项呈现原因与策略，并明确“恢复存在降级／已确认恢复同一”。完整 argv 区分精确 ID、显式集合与默认发现，两种环境策略不混用；历史字节不变，不修复原生文件。 |
| P16：已有降级不掩盖权威失败或未确认 | `keeps authoritative result`：两 agent × 启动拒绝、错误原生身份、确认超时、聚焦失败。HTTP 502，分别为 failed/start、failed/confirm、unconfirmed/confirm、failed/focus；页面 alert 保留缺失扩展原因与跳过策略，没有成功 status；外部启动一次，无输入或清理，返回已创建目标。 |
| P16：未到达的选择不描述成已经发生 | `does not invent unreached fallback strategies`：两 agent × 可读文件属于其他身份、原 Workspace 查询失败、默认 Workspace 不存在。分别只允许空降级或已选择的 session-file＋extension，不出现 Workspace 回退、默认发现或成功；没有任何创建／启动。 |
| P19：请求不能指定服务端来源、环境或执行内容 | 原 `client_can_only_select_history_not_supply_execution_content` 现在以 **12 个独立 TCP 请求**尝试 sourceBotId、botId、extensions、entrypoint、envConfig、executable、command、args、sessionFile、nativeId、input、cwd。全部 HTTP 422；Herdr 调用及目标均为空。随后合法请求仍能定位。既有 connect-bot 来源写入限制复用来源合并测试与 `session_write_boundary`，没有放宽事件写入端点。 |
| P20：新环境路径的并发、结构化超时与用户重试 | `rejects overlapping requests`：两 agent × 两策略。外部 start 文件屏障保持 owner 阻塞，独立 fetch 必须在 3 秒内返回 409、不查询不排队；释放后真实 CLI 结构化 timeout 仍为 unconfirmed/start，页面保留降级及全部目标／启动名称。新用户操作遇到查询和相关身份核实失败均不创建副本；原生目标可识别后仅定位。完整外部记录恰为四次用户查找、一次创建、一次启动、一次最终聚焦，无输入／销毁／重启。最后在来源 Bot 已编辑、当前目录已损坏时仍只定位已有目标，覆盖 P11 与新路径的交互。 |
| 真实文件、HTTP、Herdr 与浏览器 smoke | 下述专用真实 session；另保存受控组合的 HTTP、外部调用、目标、文件状态与交付页面截图。 |

自动场景的 [26 份结果／副作用收据](integrity-05/controlled-page-http.json) 对应 22 个场景；四个交互场景分别额外保存最终用户重试。代表截图：[过滤＋精确 ID＋Workspace](integrity-05/pi-filtered-combined.png)、[默认发现＋两种回退](integrity-05/omp-discovery-combined.png)、[启动拒绝](integrity-05/pi-filtered-start.png)、[身份不符](integrity-05/pi-filtered-mismatch.png)、[确认超时](integrity-05/pi-filtered-confirm.png)、[聚焦失败](integrity-05/pi-filtered-focus.png)、[结构化超时](integrity-05/omp-filtered-structured-timeout.png)、[用户重试定位](integrity-05/omp-filtered-retry-located.png)。受控收据不证明真实扩展已加载。

## P1–P21 自动覆盖闭环

只列现有覆盖及本票组合增量，不重复 01／03／04 的单类测试，不修改其他票据。下表是行为到证据的映射，不把 P 编号写进测试名当作通过证明。

| Promise | 对应自动证据／交付审阅 |
| --- | --- |
| P1、P4、P9 | `chat_resume_environment::source_survives_restart_and_resume_uses_latest_environment_without_trigger_replay`：正常发起、公开修改来源 Bot、真实重启、独立 A／B 集合与新映射 argv、无初始输入重放。 |
| P2、P3 | `intake_orders_and_concurrent_writes_preserve_first_source_time_memo_and_identity_set`：register-first、creation-first、并发写入、后续来源 B／稀疏 register／伪造来源／Resume、跨重启的首次时间与备忘及完整身份集合；通过恢复结果观察首次来源 A，只有明确发起才能补齐另一条无来源记录。 |
| P5、P18 | `missing_optional_mapping_is_filtered_only_on_resume`、`missing_entrypoints_preserve_other_extensions_on_resume_only`、`missing_fixed_and_optional_entry_mappings_are_item_local`、`unreadable_fixed_and_optional_entries_keep_the_other_valid_item`；Pi／OMP、固定／可选、映射／不存在／不可读均有独立样例，相同无效选择普通发起拒绝。有效发起仍由 P1 的 argv 和真实初始输入观察覆盖。 |
| P6 | `empty_explicit_environment_retains_each_warning_without_assuming_confirmation` 及 `chatResumeExtensions.spec.ts`：空显式集合仍关闭发现、两项原因保留、缺少身份不能确认成功。 |
| P7 | `missing_source_and_deleted_bot_use_native_discovery_without_guessing` 及 `chatResumeDiscovery.spec.ts`：旧记录、外部接入、正常发起后删除来源，两 agent 完整 argv 无 Bot 追加；不猜同名同 cwd Bot。 |
| P8 | `history_reads_only_resolve_workspace_names_without_any_persistent_enrichment` 及 Discovery 页只读场景：独立旧记录，多次读取、重启、改名和查询失败，比较文件字节／mtime，不创建缺失持久文件。 |
| P10 | `invalid_model_or_initial_skill_does_not_block_environment_restoration`：分别让 model、初始 Skill 失效，Resume 仍加载环境且不重放；普通发起仍拒绝。 |
| P11 | `chat_resume`／`chat_resume_omp` 仅定位覆盖及本票交互场景最后一步：来源已编辑且新启动配置损坏仍定位现有目标，不执行环境加载／重启／输入。 |
| P12 | `chat_resume_pi`、`chat_resume_omp`、`chatResumeContract.spec.ts`：完整原生身份、真实 cwd、同名同 cwd 反例、无新输入；本票另有下述真实原生 smoke。不以受控参数证明真实旧历史或任务未继续。 |
| P13 | `history_workspace`、`historyWorkspace.spec.ts`、`default_workspace_must_resolve_uniquely_before_creating_any_target`；缺少／删除、原查询失败、默认缺少／重名、跨重启保留原位置；本票组合旧位置删除。 |
| P14 | `chat_resume_pi`／`chat_resume_omp` 的 file／id-only／unavailable-file／wrong-file 场景，以及本票精确 ID 组合、提前原生身份失败场景。 |
| P15、P16 | 单类反馈复用 `chatResumeDiscovery`／`chatResumeExtensions`／`chatResumePi`／`chatResumeTimeout`；组合、权威失败与未到达选择由本票 18 个组合／失败／提前停止场景补齐。 |
| P17 | `unreadable_bot_storage_or_catalog_fails_without_start_or_repair` 及 Discovery 页八个真实解析／OS 读取失败场景；不默认发现、不修复损坏文件。 |
| P19 | 上述 12 个独立请求、来源合并防伪造及 `session_write_boundary` 的同机接受／非同机拒绝且不建行自动项。真实网卡人工项保持 ignored，不记为通过。 |
| P20 | 既有 `chat_resume_concurrency`、`chat_resume_timeout`、查找／身份失败先例；本票四个新策略交互场景补齐。 |
| P21 | 审阅 README、`docs/integration-boundaries.md` 与 GLOSSARY 的 trigger config／env config、最新环境、初始 Skill 调用／加载型 Skill、初始消息／系统提示词、原生存储与不传 session-dir 说明；不添加源码／文案逐字匹配测试。 |

## 专用真实 Herdr／Pi／OMP smoke

证据：[页面与 HTTP](integrity-05/native-page-http.json)、[身份、原生输出、保留目标及文件 hash](integrity-05/native-side-effects.json)、[组合成功截图](integrity-05/native-combined.png)、[缺少必要身份集成截图](integrity-05/native-missing-identity.png)。

- 独立目录 `C:\Users\hugos\AppData\Local\Temp\resume-integrity05-native-avblp0zh`，专用 Herdr session `integrity05-avblp0zh`，唯一默认 Workspace `w1`／`integrity05-default`；所有 CLI 均显式指定此 session，未控制默认 session 或用户 Chat。
- 使用先前**专用验收 seed 的副本**，不修改原始 seed；只在新目录预写隔离历史记录与来源 fixture。可信来源的正常收录仍由公开发起自动测试证明，不把 smoke 的预写来源冒充收录证明。隔离 HOME／USERPROFILE／PI_CODING_AGENT_DIR，禁止 server 重启自动恢复，不提交任何 agent 输入。
- Pi 1.4.0：原生 ID `01a1672d-b70b-72d3-b25d-c96f455256c7`，新目录 `pi.jsonl`；历史 cwd 为原专用 seed 的 `pi\zone`。有效官方身份扩展＋缺失可选项，同时原位置不存在；页面 HTTP 200／restored，到 `w1:t2`／`w1:p2`，两项降级均保留。Herdr 报告该精确文件与 cwd、idle，真实输出含 `isolated storage acknowledgement`。
- OMP 18.12.6：ID `01a16737-5b3b-76f7-ab1b-05324809594b`，新目录 `omp.jsonl`；历史 cwd 为原专用 seed 的 `omp\zone`。无来源、原位置不存在，使用隔离原生默认发现；页面 HTTP 200／restored，到 `w1:t3`／`w1:p3`，两项降级均保留。Herdr 报告精确文件与 cwd、idle，真实输出保留旧中断任务及 Read，没有继续任务的新增原生记录；两个副本和原始专用 seed 的 SHA-256 均未变化。
- 必要集成失败：只关闭本次创建的 Pi `w1:t2`，使当前映射中的 Pi 必要身份入口不可用，再从页面 Resume；空显式集合仍关闭发现，HTTP 502／unconfirmed／confirm 到 `w1:t4`／`w1:p4`，两项扩展缺失＋Workspace 回退全部显示。实际已启动但无可核实原生身份，未冒充成功；独立用户重试返回 failed／lookup／identity-verification-failed，未创建副本。该未确认目标与 OMP 目标保留在专用 session，未自动关闭或重启。
- 捕获时 HTTP 为 `http://127.0.0.1:4738/`，smoke 后已停止本 worktree 的临时 HTTP 进程以交出 worktree；专用 Herdr 与目标保留供检查。若需要重开，仅对该隔离目录的配置和专用 session 操作，勿对用户 Chat 重试。

上述 smoke 证明这轮组合及必要身份缺失的实际恢复结果，不等于完整显式／部分／默认／ambient 扩展集合人工验收。**H-Pi／H-OMP 仍待用户确认**，本票不代签，也未因其未确认停止自动工作。

## 实际 TDD 命令与结果

在 `frontend` 下分切片执行：

```text
npx playwright test browser-tests/chatResumeIntegrity.spec.ts --grep "combines exact ID" --output test-results/integrity-05-combined
# 修正页面附带只读 Workspace 查询的观察后：4 passed
npx playwright test browser-tests/chatResumeIntegrity.spec.ts --grep "keeps authoritative" --output test-results/integrity-05-failures
# 8 passed
npx playwright test browser-tests/chatResumeIntegrity.spec.ts --grep "does not invent" --output test-results/integrity-05-unreached
# 6 passed
npx playwright test browser-tests/chatResumeIntegrity.spec.ts --grep "rejects overlapping" --output test-results/integrity-05-interactions
# 修正同一 Workspace 文本的定位器范围后：4 passed
```

在 `backend` 下：

```text
cargo test --test chat_resume client_can_only_select_history_not_supply_execution_content -- --nocapture
# 1 passed；包含 12 个独立拒绝请求及随后合法定位
```

准备交付产物实际执行 `npm ci && npm run build`、`cargo build --bins --example safe_shutdown_shell`，均成功。真实浏览器 smoke 通过 Eval 驱动实际交付页面及 HTTP，配合真实 `herdr --session integrity05-avblp0zh` 控制 interface；收据与副作用见上，不以空结果或参数形状替代实际观察。

最终回归实际执行：

```text
# frontend
npm test
# 网络策略通过；27 files、205 tests passed
npm run test:browser -- browser-tests/chatResume browser-tests/historyWorkspace.spec.ts browser-tests/botExtensionLaunch.spec.ts --output test-results/integrity-05-full
# 网络策略通过；73 passed，包含新 22 个组合场景和既有 Resume／普通发起边界
npx playwright test browser-tests/chatResumeIntegrity.spec.ts --grep "rejects overlapping" --output test-results/integrity-05-interactions-final
# 最后加入“编辑来源 Bot＋损坏当前目录仍定位已有目标”的交互后：4 passed

# backend
cargo test --tests
# 446 passed、1 failed、5 ignored；退出 101，不宣称全量通过
```

后端 [完整命令输出](integrity-05/backend-tests.txt) 中，本功能直接相关的 `chat_resume` 7、`chat_resume_concurrency` 3、`chat_resume_environment` 11、`chat_resume_omp` 5、`chat_resume_pi` 8、`chat_resume_timeout` 2 均通过，合计 36；`history_workspace` 6 与 `session_write_boundary` 3 个自动项也均通过。五个既有真实网卡／mDNS 人工项 ignored，不记为通过。

全量唯一失败为 **`zone_sync::configured_timeout_stops_the_running_command_and_keeps_effective_pull_state`**（`backend/tests/zone_sync.rs:1181`）：其终止后的 Git 写资格请求预期 HTTP 200，实际 HTTP 409／`git-write-in-progress`。该用例不是 Resume／扩展路径，本票没有改动其生产代码或测试；未运行基线比较，因此不把它断言为“已知既有 flaky”，也不通过重跑掩盖失败。已将原始结果交给主代理，本票保持隔离、不越票修复。**这使项目全量检查仍非全绿，但本票拥有的自动验收均完成。**

本 worktree 尚未合入集成分支；按票据状态规则，合入前保留 in-progress，由独立集成代理完成整合后关闭。H-Pi／H-OMP 仍为上述未验证的人工边界。
