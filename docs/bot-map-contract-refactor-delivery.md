# Bot／Map 重构：交付与现场交接

**本报告区分仓库转换、隔离回归、跨仓联调、原生专项和现场事实。** 仓库已交付首条 [创建map 资产与配置清单](workflow-migration-inventory.md)，不是现场迁入或业务验收。其余流程未选择不等于废弃，不以这份 Markdown 决定运行资格。

## 本轮产品自动证据

从对应产品 worktree 运行，使用独占绝对 `CARGO_TARGET_DIR`；先构建交付页（`npm --prefix frontend run build`），所选检查不触发生产操作。见[测试约定](agents/testing.md)。

| 范围 | 精确入口及取证 |
|---|---|
| 发布定义、三阶段、资料及现有界面 | `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch selected_flow:: -- --nocapture` 实际发布文件、HTTP 读回与受控提交；`npm --prefix frontend run test:browser -- selectedFlow.spec.ts --workers=1` Chrome 真实视图、HTTP、截图和没有 Herdr 调用的对照。 |
| 保持代码约束，不重写已有拒绝场景 | `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch rust_save_ -- --nocapture`：6 通过，含真实 Bun 写入、同 revision 冲突拒绝、保存和材料失败的部分效果。`--test workflow_dispatch map_rules:: -- --nocapture`：4 通过，实际拒绝及原定义不变。 |
| P52：暂停、退出、未接管与外部效果 | `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch script_lifecycle_ -- --nocapture`：2 通过；原有真实进程生命周期场景让已接收业务抵达受控文件闩，核对暂停后报告/产物、实际旧进程退出与新进程 `managed=false`、原 Run/产物字节和无重放窗口。首次夹具运行时未构建打包页，`GET /` 返回 404；构建后复跑通过，不将这条失败计为生命周期结论。 |
| P54：只移除显式确认的旧副本 | `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch selected_flow::selected_flow_cutover_removes_only_explicitly_confirmed_old_definition_copies -- --exact --nocapture`：比较独立保留与删除集合及全部实际字节，打印 SHA-256。重复 basename 的正式/离线对象、未处理、废弃、仍引用共享 Bot 的 Map 和 Task 原件是负对照。 |
| P56：仅清理 chief 运行记录 | `cargo test --manifest-path backend/Cargo.toml --test chief_delegation cutover_cleanup::cutover_removes_only_chief_records_and_preserves_chat_zone_and_artifact_bytes -- --exact --nocapture`：1 通过；实际后端和公开委派、报告、接收确认先产生四份 chief 文件，退出后按显式路径清理；重启后公开报告为空，Chat 历史、原生历史、zone、业务文件和近名无关文件的字节保持，测试打印原始大小和摘要。不是 P57 新身份重建证明。 |
| P53/P58：首条稳定身份即能运行 | `cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch selected_flow::selected_flow_executes_by_stable_identity_without_inventory_gate -- --exact --nocapture`：只投放创建map 与关联四 Bot、新旧同名定义分别在正式根和离线 backup；在无效清单内容的条件下用公开操作驱动正式三阶段，比较实际外部提交、内部产物、最终服务配置及没有生成业务 Run 的效果。P53 backup 在线排除和 P55 归档由各自已有产品用例取证，不在本用例替代。 |

通过自动检查只能确认定义、角色资料入口和机械行为：受控宿主不代表真实 agent 读过资料；文件保存、声明结果、批准和完成都不是业务遵从证明。复跑保留标准输出、测试失败及退出/清理异常；不能用一次通过改写旧故障。部署所需的套件、Skill、角色入口和词典按目标安装与配置核查。

### 08 实测记录

- Rust 定向共 21 项通过：发布及隔离 3、chief 清理 1、脚本生命周期 2、保存 6、Map 规则 4、既有 `plan_workflow::` 3 和 `create_migration_` 2。创建回归的旧归档首次因为选定 Map 的批准文字差异失败 2 项；仅对明确批准的字段差异归一、保留原历史归档和其他字段，独立复跑通过（`local://177.log`、`local://181.log`）。发布红灯先观察到原 Map 的 `input_requirements` 被公开读取拒绝，落地后通过。Chrome 1 项通过；首次依赖链接模式无法解析 `picocolors`，保留原失败，改用 `bun install --cwd=frontend --frozen-lockfile --linker hoisted --backend copyfile --force` 后构建成功（仍有 Vite chunk 体积提醒）；浏览器夹具构建路径与初版断言错误在复跑前已修正。
- S4 独立对照：按配置解析并核对 9/9 Map（均无迁移完成/废弃确认）、33 个 Bot 定义和 36 个 Task 条目；对照选定流程四 Bot 的全部 16 个字段正文、三 Task 的完整批准 prompt、唯一业务文档正文、系统章节标题与链接。原资料路径未被发布定义或通用运行文档引用。未把资料正确作为实际模型遵从。
- 字节证据：P54 在 9 个隔离对象中仅删除 2 个已授权旧副本，7 个保留对象前后逐字节相等；正式 Map/外层 Bot 的 SHA-256 分别为 `42eeba120edf35b6334542b41044516cca33b7130543687642ac66844568f6f8` 与 `2ed4708a6a1192f9a673567780d5f975767740959500df5a146b8d62c1f30bd4`（此 worktree 的磁盘换行）。P56 删除 4 个、保留 6 个隔离文件；业务文件 19 字节，SHA-256 `f13bc979e6715a636f4dca7db6c69bbfece506a9e369ba6ea937e9d6b75e3a47` 前后相同，两个所属后端均记录退出且清理错误为空。P53/P58 最终内部 Map 与实际文件及完整 HTTP 读回一致，SHA-256 `37c46f661b78e90d9c7e5b906e9e6ea8057a0a7b43cd617ee0025bf0bbf5ec3f`；7 次请求对应 3 个独立 Chat、1 个管理者和 3 个节点，未发起生成后的业务。原始命令输出及全部摘要见本轮归档 `local://177.log` 前段（最终发布、P54、P58）、`local://152.log`（P56）、`local://164.log`（P52）；Chrome 截图已归档到 `local://ticket-08-selected-creation-map.png`，用例还对照了公开 HTTP。

### 07 后续集成复验

- 在 bot 原开发 worktree 的同步提交 `e171f13b` 上运行 `cargo test --manifest-path backend/Cargo.toml --test chief_delegation --test chief_authority -- --nocapture`：委派 32/32，权限 2/2。P22–P26、P29、P34 的预览、Task 文件校验、Map 公开成果子集及角色恢复通过；P57 的隔离清理后 chief 恢复、旧职责拒绝与保留文件字节对照通过。
- P27/P28：只读报告日志保存失败后观察 46,008 ms，未尝试通知；TUI 通知失败并重启后观察 46,010 ms，保留原报告且无重投或重新派发；App／OMP 与 App／Pi 的通知失败分别观察 46,167 ms 和 46,252 ms，各仅一次通知尝试。接替者读回及显式重发去重通过；后端记录的 `cleanup` 均为空。见[原始后端日志](local://ticket-07-chief-integration.log)。
- 定向回归：`workflow_start` 的 `app_delegated_map_binds_manager_before_approved_input`、`app_root_run_waits_for_native_readiness_and_persists_before_business_input`、`root_runs_preserve_named_bindings_separately_from_business_text` 各 1/1；`npm --prefix frontend test -- tests/chats/chatCreation.test.tsx` 18/18；页面与安全壳构建成功，`npm --prefix frontend run test:browser -- chiefEntry.spec.ts --workers=1` 4/4。此处不是全仓结果；11 跨仓工具联调、12／13 延期 tester 和 14 现场／用户验收未由 07 执行。

### 其他自动票的已取得来源

下列是编排器已提供的回执，不是 08 重新执行的检查。原报告位于该轮归档 `%TEMP%/hyper-auto-coding-2swvrlva/`；相对的执行日志/构建哈希按各原报告定位。临时路径未能留存的原始证据以报告为限，不从合并状态推断覆盖。

| 票 | 实际记录、失败与边界 | 来源（归档文件名） |
|---|---|---|
| 01 | `cargo check --manifest-path backend/Cargo.toml --all-targets`；5 提示词单元、11 公开回归；集成另列 3 角色恢复，全数通过。无原生／模型证据。 | `implement-ca55d174-6ae7-46b2-a17d-aee5a59ed830.md`、`integrate-037d239c-8551-44cb-b910-fd2d9da6e629.md` |
| 02 | `workflow_dispatch` 实现首轮 33/35，夹具修正后仅 `create_migration_` 2/2；集成首轮 33/36，旧字段夹具修正后定向 12/12；`runtime_data` 启动/backup 2/2、`chief_delegation -- map::` 5/5、前端 scheduler 32/32、四真实页面 spec 20/20、`python scripts/test-script-contracts.py` 成功。早期页面构建有 `picocolors` 失败。不同范围与轮次不合并成全绿。 | `implement-3f2bf1f7-25a1-4fdd-8ae3-81c9a2a36265.md`、`integrate-59ccc253-b743-47ff-8aa6-9e645a9e3b63.md` |
| 03 | `workflow_dispatch`/`workflow_start` Map、节点和缺省/拒绝回归；集成定向 7 通过、初次命令分隔符漏写未执行用例；`mapDefinitionContract.test.ts` 与 `mapInputsEditor.test.tsx` 26 通过，`schedulerNodeEditing.spec.ts --workers=1` 11 通过。 | `implement-7b62cf02-8647-4759-99d5-becbc24eb3a0.md`、`integrate-0e114f5a-87d4-4b98-a3b4-2aadc2adbc36.md` |
| 04 | `workflow_start`/`workflow_dispatch` 定向 13，workflow 页面 15、Map 删除及相邻页面 9，通过。保留原 Windows exe 被占用（os error 5）；后续使用独占构建目录。集成回执另记录当时运行的浏览器结果 394 通过／1 跳过（原始 `local://115.log`），不是集成总门禁。P30 已明确不新增 Run 删除 API/UI，仅人工磁盘归档。 | `implement-77261c10-2d31-43ba-8a71-959dcba16490.md`、`integrate-946dd4c5-4c1f-419a-8c6a-6045b8ee3ccb.md` |
| 05 | 公开输入、发送、角色恢复：Cargo 主轮 38、补测 12 及最终角色对照 1 通过（有重叠）；Chrome 13、补测 9；Vitest 15。保留编排器报告的全量中暴露 13 项；集成修订和独立定向结果另由 05 回填。 | `implement-652bdb52-8381-479b-874c-28d5d46e43ca.md` |
| 06／07 | 08 未取得本轮原始报告，分别由对应交付回填显式 finish/父交接及 chief 委派、报告与接替证据；不据依赖关系写为通过。 | 未随本次快照提供 |
| 09 | `bun run typecheck`、`bun run check:integrations` 成功；workflow-chat 71、受控 dock 22、安全壳 dock 8；集成另外记录 workflow-chat 71/71、6 个具名 OMP/Pi dock 18/18。 | `implement-8c7d180b-07bd-4537-99b9-8241e98f718d.md`、`integrate-e1b6c500-29e4-41c0-9096-a6ba52c2fbf9.md` |
| 10 | `bun run typecheck`、`bun run check:integrations` 及本地用例。集成 `bun test tests/workflow-chat` 73/73；另行具名 OMP/Pi dock（`--parallel=4`）14 文件 67/67。首轮 360 秒外层超时：10 文件有日志未获判定、`node-report-contract` 在途未知、3 文件未见启动；2 夹具残留已归档，首轮退出未经证实。最终集成报告只执行记录恢复后的类型/静态检查，沿用同提交结果，不是重跑原生。 | `implement-6b1c7b2a-9f72-42c1-a200-1bf5432620f6.md`、`integrate-38ecd3ee-789b-47ab-896e-b09e9053141e.md`、`integrate-3a360a1a-6d47-4649-8632-2b9750951ae0.md` |

**清理独立事实：** 编排器记录 01、02、04 worktree 删除被拒绝，03 目录非空、09 Invalid argument、10 Filename too long，均保留；有清理阶段缺文件／进程工具，以及字面 `placeholder` 警告，不能视为有效清理证明。01 的独占目标目录未获清理或仍存在的确认；共享缓存和归属不明路径未处理。专项进程退出与这些 worktree 清理失败不是同一事实。

## 跨仓、延期、人工证据回填

- **11 跨仓联调：未在本轮 08 执行。** 由[跨仓联调](../.scratch/bot-map-contract-refactor/manual/11-cross-repository-acceptance.md)记录同版本 bot/toolkit、公开工具请求与动态结果、失败和清理证据。独立产品 HTTP 成功不填这一格。
- **12／13 原生 tester：明确延期，未执行。** [定义、发起、输入与角色](../.scratch/bot-map-contract-refactor/manual/12-deferred-tester-context.md)与[成果、嵌套、chief、脚本和恢复](../.scratch/bot-map-contract-refactor/manual/13-deferred-tester-handoff.md)分别回填已适配命令、宿主/场景事实、未跑格和清理结果；不以旧专项、单宿主、受控模型或用户签收代替。
- **14 用户业务验收及迁移完成确认：未验。** 以[选定流程的证据材料](workflow-migration-inventory.md#创建map已交付的转换与共享影响)和[实际切换手动票](../.scratch/bot-map-contract-refactor/manual/14-controlled-cutover-and-user-acceptance.md)为入口。在自动工作和联调结束后，提供输入、当前概要方案及确认/缺口、配置差异/授权/共享影响、最终配置和内部文件核对、已做/省略的检查和未知结果。用户分别确认真实业务效果和定义迁移完成；未经确认清单仍为未开始。
- **14 现场切换：未执行。** 尚无实际进程停止、线上导入路径、移动/删除授权、归档字节或现场持久关联证据。下节只定义现场步骤；隔离演练不能补记为现场完成，也未启动用户的业务或使用其认证。

## 现场停机、清理和首条验证（仅获授权后）

1. 用服务实际启动参数解析 `paths_file`、Map/Bot/Run 根、历史 `sessions.history_file`、zone 路径及原生会话根；列出旧布局迁移可能读取的所有历史导入源，选在上述在线根、目录连接/重解析点及所有启动导入源之外的备用和归档位置。Git 只保存定义历史，不代替这个一次性离线暂存。旧布局升级和新格式迁入是两个动作。
2. 禁止新的派发与业务请求，逐个登记关联 Run/Chat/script、进程/终端、未完成的文件或其他外部写入；逐一核实在途工作真正结束或已处理到用户允许的边界。`pause` 不取消在途业务，`managed=false` 不判定成功、失败或停止，后端退出不回滚外部效果。保留时间、归属、终止/回执、外部效果和未知项；无法核实的效果不得因停止服务而被登记为未发生。再停止并确认对应服务和进程退出。
3. 停机后逐项登记旧 Map/Bot 的源、去向和字节长度/SHA-256，移至离线 backup。完整移动旧 Run 目录（含 `run.json`、`history.jsonl`、`messages.jsonl` 及实际其余记录）到离线归档；保留原样，不直接删除，不用现有 Map 补造历史，不把旧 Run 接回新服务。不得整目录删除数据根。
4. 以实际 `sessions.history_file` 为基准，仅清理相同文件主名的 `.chief.json`、`.delegations.json`、`.chief-reports.jsonl`、`.chief-reports.receipts.jsonl` 四个文件；例如 `session-history.json` 的报告为 `session-history.chief-reports.jsonl`。先独立登记待删路径、确认来源及应保留的全部 Chat、原生会话、zone 和实际产物；逐个使用字面路径（例如 PowerShell `Remove-Item -LiteralPath $confirmedChiefRecord`，不加通配、不递归）。保存前后清单、长度/SHA-256、请求及结果，验证保留集合的真实字节，不只检查目录存在。chief 的旧原生 Chat 不因此删除；之后只能用现有恢复/登记建立新当前身份，不继续旧委派和 Run 权限。
5. 更新经联调的后端、页面、Toolkit 与本次获准资产。只把已选 Map 和必要关联 Bot 迁入正式根，保留 Map 文件名、Bot ID 和引用；共享 Task、Suite、script、Skill 和未改格式资料原位维护。先查在线目录、正式配置读回、引用和隔离状态，再按用户授权单独测试首条流程；不启动生成的业务，不等待其他流程，不把清单当门禁。新旧服务不混跑，不接管旧 Run；需继续业务时先查明旧效果，再另行发起。
6. 用户明确确认某对象已迁移且另授权删除后，才删除该对象在 **offline backup** 的旧副本，逐个核实路径与摘要并使用 `Remove-Item -LiteralPath $confirmedOldCopy`。不把正式新定义、待处理项、废弃项、共享原件或业务文件列为清理对象；共享 Bot 的旧副本不必等待它的所有消费者一起迁移。再次读取在线清单和保留集合字节，保留原始失败和清理错误；不得把备份清理的授权外推到其他对象。
