# 06: 启动超时保留不确定性与目标，重试重新查找

**What to build:** Pi 或 OMP 恢复启动等待超时后，页面与操作结果明确表示“启动结果未确认”，保留已创建目标供检查，不自动清理或重启。用户再次 Resume 时重新查找，能定位首次启动后才出现的对话，而非盲目重放启动。

**Delivers:** P17, P18, P19；P22 的启动超时及不充分成功条件部分。

**Test owner for:** P22 的超时未确认、仅创建 Tab 或提交命令不足以成功的检查。

**Blocked by:** 03: 无匹配时恢复 OMP 历史 Chat；05: 同一 Chat 的并发 Resume 不重复执行。

**Status:** in-progress

- [x] 分别对 Pi、OMP 施加可控启动确认超时，浏览器及 HTTP 结果均保留不确定性，不声称恢复成功或确定未启动。
- [x] 已取得的 Workspace、Tab、Pane 或 agent 目标信息在操作结果和页面中可识别，供用户检查；只表达实际已取得的信息，不编造确认。
- [x] 超时后目标仍存在；没有自动关闭、终止、再次创建、再次启动或自动提交输入。
- [x] 明确区分创建 Tab、提交命令、确认正确对话加载和完成聚焦；前两者不足以报告完整恢复成功，超时不能冒充安全失败。
- [x] 首次超时后使同一原生目标变得可识别，用户再发起 Resume 时观察重新查询并聚焦该目标，没有第二次启动。
- [x] 超时后的新操作仍遵守查询失败、身份核实失败和聚焦失败不得创建副本的规则，不通过重放上次启动规避查询。
- [x] 覆盖启动等待期间独立请求者重复操作、超时结果返回后并发重试，以及执行资格释放；不排队执行旧重复请求，不让超时永久占用资格。
- [x] 使用真实 HTTP 请求观察完整控制副作用，并在实际交付页面操作超时及重试路径；检查保留的真实目标状态，不只检查返回字段。
- [x] 更新现有用户说明中的“启动结果未确认”、目标保留和重试语义，不提供会自动再次启动的隐藏重试机制。

## 覆盖分区

本票唯一拥有 P17–P19 全部覆盖，分别覆盖 Pi 和 OMP，并拥有其与 P16 执行资格的超时交错用例。05 保留基础并发用例责任。

P22 的已有目标定位归 01，Pi 恢复确认归 02，OMP 恢复确认归 03，超时与仅创建／提交不得报成功归本票；跨路径结果一致性及真实环境确认归 07。相关承诺须全部分项完成。

## Comments

- 本票实现及自动验收已完成，等待独立集成；按状态规则，未合入前保留 `in-progress`，不提前标记 `closed`。
- 批准 seams：S1 交付页面、S2 真实 TCP HTTP、S3 生产 CLI adapter 连接独立受控 Herdr 进程。未 mock 项目内部恢复、锁、轮询或前端请求。
- P17／P22：`backend/tests/chat_resume_timeout.rs::pi_and_omp_mutating_command_timeouts_expose_only_observed_destination` 覆盖 Pi、OMP 的 Tab 创建与启动命令等待超时，分别保留实际已知位置；未知 Tab 创建结果为 `null`，不编造目标 ID。`pi_and_omp_timeout_retain_target_release_eligibility_and_retry_by_lookup_not_replay` 覆盖提交命令之后仍缺少原生身份的确认超时，必须是 `unconfirmed`，不能是 `failed` 或 `restored`。原 Pi／OMP 测试中的空白进程超时样例迁入该更完整覆盖；显式错误身份、启动失败和聚焦失败覆盖保留。
- P18／P19／P16 超时交错：同一测试检查实际外部 Tab 与 started 记录保留、完整控制命令清单、等待时独立请求立即 409、不排队、超时后释放资格；原目标迟到后重新查询并聚焦同一目标。超时后查询失败、相关身份不可读和聚焦失败均不得重复启动；并发重试由外部查询 gate 控制，只有一个执行。
- S1：`frontend/browser-tests/chatResumeTimeout.spec.ts` 的 Pi／OMP 两个用例，在交付打包页面完成等待、未确认提示、实际目标标识展示、查找失败后用户重试及定位原目标；真实 HTTP 与外部创建／焦点记录共同验证无二次启动、清理或输入。截图：`frontend/test-results/chatResumeTimeout-pi-start-2fa27-s-it-without-another-launch/` 与 `frontend/test-results/chatResumeTimeout-omp-star-f7e56-s-it-without-another-launch/` 下各自的 `*-startup-unconfirmed.png`、`*-retry-located-retained-target.png`（本次运行生成，未跟踪产物）。
- TDD red：确认超时原先返回 `outcome: failed`，Tab 创建超时丢失已知 Workspace，启动超时同样返回失败；浏览器未展示目标标识。分别观察行为断言失败后实现，再运行 green。
- 实际验证：`cargo test --test chat_resume --test chat_resume_pi --test chat_resume_omp --test chat_resume_concurrency --test chat_resume_timeout --test history_workspace`：6 suites／30 tests 全通过。`npm test`：27 files／205 tests 全通过。`npm run build`、`cargo build --bins` 均成功。`npm run test:browser -- chatResume.spec.ts chatResumePi.spec.ts chatResumeOmp.spec.ts chatResumeConcurrency.spec.ts chatResumeTimeout.spec.ts historyWorkspace.spec.ts`：8 tests 全通过；网络策略检查通过。
- 用户说明已更新 `README.md`、`docs/integration-boundaries.md`。无本票未完成实现或自动验收项。受控进程证据不冒充真实 Pi／OMP 原生加载及桌面焦点证明；合同分配给 07 的 H-Pi／H-OMP 人工确认未在本票执行。

