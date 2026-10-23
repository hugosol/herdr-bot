# 04: 逐项过滤失效扩展，保留空显式集合策略

**What to build:** 来源 Bot 存在时，一项扩展不可用只跳过该项，其他有效扩展继续用于恢复。用户在页面看到每个失效项与已知原因；即使全部被过滤，也保持有 Bot 的空显式集合，不切换默认发现。普通发起继续严格拒绝相同的无效扩展。

**Delivers:** P6；P5（自动覆盖）, P15（扩展过滤降级覆盖）, P18（无效扩展发起／恢复对照）。P 编号引用本功能已批准合同。

**Test owner for:** P5 的固定／可选扩展逐项过滤自动覆盖；P15 的扩展缺失及空集合单类反馈；P18 的同一无效扩展样例在发起和恢复之间的对照。P5 真实加载归 07 准备并由用户确认，P15 组合覆盖归 05，P18 有效发起覆盖归 02。

**Blocked by:** 01 — 集中既有恢复降级，保留精确恢复与定位规则；02 — 持久记录来源 Bot，按最新 env config 恢复。

**Status:** in-progress

- [x] 来源 Bot 存在时，固定与所选可选扩展均逐项解析；映射缺失、入口不存在或不可读时只跳过对应项，其余可用项继续提交。Pi 与 OMP 均遵守同一策略。
- [x] 为固定／可选项分别使用独立预写样例覆盖三类失败，保留另一有效项，核对实际提交集合和具体缺失原因。不可读场景必须实际制造读取失败，不用普通缺失替代。
- [x] 所有扩展都被过滤时显式集合为空，自动发现仍关闭；不得因空集合切换无来源分支或重新追加默认固定扩展。
- [x] 降级通过集中机制积累，交付页面列出每个扩展名称或标识、已知原因和跳过策略，已确认恢复时明确表示有降级，不只显示普通成功。
- [x] 空集合后缺少身份集成而无法确认时，只能如实报告失败或未确认，不假定启动或恢复成功；不新增空集合的人工验收项。
- [x] 相同无效扩展样例分别用于普通发起与 Resume：发起保持原有明确拒绝且不启动 agent，Resume 逐项过滤；不能把容错选择泄漏给普通发起。有效发起整体配置覆盖由 02 拥有。
- [x] 不安装、下载、修复、导入探测扩展，不增加运行时异常隔离、自动重试或重启修复；入口可读不等于实际加载成功。
- [x] 通过公开请求、真实隔离入口与 Herdr 控制观察验证提交行为，并由 agent 操作交付页面观察结果；完成受影响行为测试、恢复 smoke、说明更新与证据记录。不改动用户现有 Chat，不以参数记录宣告 P5 人工覆盖通过。

## Comments

### 2026-10-23 — 首次实现与验收

- P5／P18：`backend/tests/chat_resume_environment.rs` 新增 5 个公开 HTTP／真实文件／Herdr CLI 行为测试。Pi、OMP 均覆盖固定／可选项缺少入口映射、入口不存在、实际不可读（Windows 独占句柄，另一次真实读取必须失败且不是 NotFound）；保留另一有效项并核对完整提交参数。同一失效 Bot 普通发起明确拒绝且无新 Herdr 调用。额外覆盖已选可选标识整条目录映射删除。
- 固定项“映射缺失”样例保留当前 id/kind，删除 `entrypoint` 字段，经实际公开配置读取路径触发；不把删除整条固定项冒充历史固定项仍应存在。整体 TOML 无法解析、无法读取等结构错误继续失败；`dynamic_catalog` 现有 3 项回归全部通过。
- P6／P15：空集合保留 `--no-extensions`，逐项原因保留；移除身份上报后真实确认等待最终返回 502／unconfirmed，不假定成功。`frontend/browser-tests/chatResumeExtensions.spec.ts` 实际操作交付页面，Pi、OMP 各验证部分集合的已确认降级及空集合的未确认反馈，2 项通过；截图与受控提交记录保存在 `docs/evidence/chat-history-resume/extension-filter-04-*`。
- TDD：可选映射删除的 red 为 Resume 502／invalid-agent-settings；文件缺失的 red 为没有降级提示；固定入口映射删除的 red 为整份配置因缺少 entrypoint 被拒绝。对应最小实现后各自 green。不可读及空集合使用现有已成立的单项过滤／确认机制补足覆盖，未制造假 red。
- 实际验证：`cargo test --manifest-path backend/Cargo.toml --test chat_resume_environment --test chat_creation --test dynamic_catalog --test bots`：44 passed（8 + 26 + 3 + 7）；`npm run build --prefix frontend`、`cargo build --manifest-path backend/Cargo.toml --bins` 成功；`npm run test:browser --prefix frontend -- chatResumeExtensions.spec.ts`：2 passed，网络护栏通过。
- 实际 native smoke：专用 Herdr 0.9.5 session + Pi 1.4.0／OMP 18.12.6，有效官方身份项与缺失可选项并存时均确认同一原生身份、复制文件、历史 cwd 和聚焦，终端显示已知旧历史且 idle；复制文件哈希不变，隔离模型端点请求为零，后续 Resume 均只定位。只使用既有专用种子历史的副本，未修改用户 Chat；专用测试 session 与服务已停止。记录：`docs/evidence/chat-history-resume/extension-filter-04-native-smoke.json`。smoke 来源字段是隔离预写样例，正常来源收录由上面的公开发起测试覆盖。
- 未宣告 H-Pi／H-OMP 人工加载验收通过；完整显式／默认／ambient 集合证据与用户确认仍按票 07 分工，不新增空集合人工项。
- 实现和本票自动验收完成，状态保留 in-progress，待独立集成合入后按项目规则关闭。与票 03 的共享文件集成约定：保留其 `Feedback::environment_args` 来源选择／默认发现／读取错误策略，在有来源尾部用本票 `self.environment(&catalog, agent, &bot.env.extensions)` 替代严格 `resolve_env`＋`check_entrypoints`，再调用 `agent_launch_args`。
