# 02: 持久记录来源 Bot，按最新 env config 恢复

**What to build:** 正常由 Bot 发起的 Chat 在确认原生身份后保留可信来源。用户重启服务、编辑该 Bot 后，Resume 使用其最新 env config 与当前 Bot 选项目录，而不重放 trigger config。收录与 register 使用统一的持久信息补齐入口，安全合并同一原生身份的一致互补信息。

**Delivers:** P1, P2, P3, P9, P10；P4（自动覆盖）, P18（有效发起覆盖）。P 编号引用本功能已批准合同。

**Test owner for:** P4 的最新选择／映射、固定与可选显式集合、关闭自动发现及 ambient 排除自动覆盖；P18 的有效发起仍应用 trigger config 与 env config。P4 真实加载由 07 准备并由用户确认，P18 无效扩展发起／恢复对照归 04。

**Blocked by:** None (can start immediately).

**Status:** in-progress

- [x] 在结构上区分 trigger config（初始消息、初始 Skill 调用、model、thinking）与 env config（当前扩展加载配置），迁移全部受影响调用者。若已交付持久格式需要改变，提供显式迁移，不保留隐式旧字段或路径；不实现未来加载型 skills 或系统提示词注入。
- [x] 正常发起并确认原生身份后记录可选来源 Bot 标识；只接受发起流程的明确来源，不按显示名、cwd 或短 chatId 猜测，不为测试新增公开来源查询或写历史入口。
- [x] 发起收录与 register 共用持久信息合并规则，按完整原生身份去重、安全串行写入；缺省值不擦除已知值，来源首次有效后不覆盖，重复相同信息不改变结果、首次时间或备忘。统一补齐不承担查询或降级修复。
- [x] 分别制造 register 先到、发起收录先到及并发写入，覆盖缺省与明确来源的一致互补信息；使用真实隔离文件与进程重启核对独立身份集合、已知首次时间和备忘。不要求互相冲突的普通可更新字段满足交换律。
- [x] 以扩展 A／B 的独立预写样例，通过后续 Resume 证明正常发起来源跨重启保留；已知来源 A 后又收录来源 B、无来源 register 及 Resume，结果仍使用 A；当前缺失来源只可由明确发起来源填入。
- [x] 对两个现有支持的 agent（Pi、OMP），创建后将 Bot 选择由 A 改为 B，并修改当前目录映射；恢复使用最新可用固定项与 B，关闭自动发现且排除 A 和未选 ambient。此票使用有效扩展样例，失效项容错归 04。
- [x] Resume 不提交初始消息、不调用初始 Skill、不传 Bot model／thinking 覆盖；观察实际输入活动，不能仅检查内部字段。原生会话自身设置沿用既有恢复规则。
- [x] 先保存有效 Bot，再使 model 或初始 Skill 目录选择失效，同时保留有效扩展与原生对话；恢复仍使用有效 env config，不重放或要求有效 trigger config。
- [x] 有效普通发起仍应用 trigger config 与 env config；不得因恢复配置解耦放宽普通发起的扩展校验策略。
- [x] 通过公开 HTTP、真实隔离持久化／重启及 Herdr 控制观察闭环，不以私有补齐函数断言替代正常收录。不改动用户现有 Chat。
- [x] 完成行为回归与真实请求驱动的恢复 smoke，更新受影响说明并记录证据；受控 adapter 参数仅证明提交内容，不能宣称 P4 人工真实加载覆盖通过。与 01 的相近代码修改需要协调，但不存在行为前置依赖。

## Comments

### 2026-10-23 — 实现与本票自动验收完成，待独立集成

- `Bot.trigger`／`Bot.env` 与已解析启动配置分离；serde flatten 保持已有 Bot HTTP／磁盘形状，无新格式迁移或旧字段别名。历史新增可省略、仅内部可读的 `sourceBotId`，仅明确发起在确认原生身份后可写，沿用收录／register 的同一串行事务入口。
- 新增 `backend/tests/chat_resume_environment.rs`，接缝为真实 HTTP、真实隔离文件、交付服务独立进程重启及生产 Herdr CLI adapter；受控 Herdr 在独立进程记录实际提交的 argv／prompt，不替代内部补齐实现。
- P1/P2/P3/P4（自动）/P9/P10/P18（有效发起）：`source_survives_restart_and_resume_uses_latest_environment_without_trigger_replay` 对 Pi、OMP 分别正常发起、编辑 A→B、更新映射、重启，再从公开 HTTP 恢复；逐项核对最新固定项与 B、关闭发现、排除 A／ambient、初始 model／thinking／Skill／prompt 只用于发起。最初 red 实际失败在恢复参数缺少 B，随后 green。
- P10：`invalid_model_or_initial_skill_does_not_block_environment_restoration` 分别使已保存 Bot 的 model 或初始 Skill ID 失效，Pi／OMP 都仍恢复有效 env；普通发起保持明确拒绝。恢复记录中没有输入活动。初次用例的错误码期望曾误写为 unknown-model，按已有公开错误类别修正为 bot-model-invalid／bot-skill-invalid，没有放宽行为期望。
- P2/P3/P9：`intake_orders_and_concurrent_writes_preserve_first_source_time_memo_and_identity_set` 覆盖 register 先到、发起收录先到，以及外部原生身份屏障配合同时 HTTP 写入；通过后续 Resume 证明首次来源 A 不被 B、缺省 register 或伪造来源字段替换；无来源的独立身份先仅用固定项，明确发起后才能补齐 A。完整独立身份集合、已知首次时间 123、真实备忘、空白不擦除、跨进程重启及原生文件不变均核对。
- 行为回归实际命令：`cargo test --manifest-path backend/Cargo.toml --lib --test bots --test dynamic_catalog --test chat_creation --test chat_identity --test omp_zone_history --test pi_zone_history --test session_history --test history_memo --test history_workspace --test chat_resume --test chat_resume_pi --test chat_resume_omp --test chat_resume_concurrency --test chat_resume_timeout --test chat_resume_environment --test network_policy`：176 passed，17 suites。
- 最终配置去除重复扩展校验、补上无来源 register 场景后的命令：`cargo test --manifest-path backend/Cargo.toml --lib --test bots --test dynamic_catalog --test chat_creation --test chat_resume_environment --test network_policy`：119 passed，6 suites。新 smoke 三个测试全部通过；普通发起扩展校验由现有 dynamic_catalog／chat_creation 回归保持。所有服务 smoke 只监听数值回环，mDNS 禁用，原生文件和持久数据均在隔离临时目录，不接触用户现有 Chat。
- README 与集成边界已更新；与 01 已协调 resume.rs 和文档相邻修改。01 的反馈包装合入时需为本票新增的 restore_failure 调用机械补入 feedback 参数；Bot 访问需使用 `bot.env.extensions`。
- 未宣称 P4 真实扩展加载或真实 Pi／OMP 原生加载人工验收通过；该项由 07 准备并由用户确认，P18 无效扩展对照归 04。本票自动验收已完成。按项目规则 closed 须已合入，因此保持 in-progress，交由独立集成代理处理，不自行合入。
