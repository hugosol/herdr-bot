# 04: 保留原 Workspace 与历史身份，支持重启后恢复

**What to build:** 正常收录历史 Chat 时保留可选原 Herdr Workspace；重启后可回到该位置恢复，原位置已删除时明确回退默认 Workspace。历史仍独立于当前登记，Resume 不改写原位置，也不因加载同一原生对话增加历史条目。

**Delivers:** P7；P8 的原 Workspace 已删除部分；P9 的原 Workspace 查询失败部分；P21 的旧格式兼容与基础身份去重部分。

**Test owner for:** P8 的原 Workspace 删除检查；P9 的原 Workspace 查询失败检查；P21 的旧格式读取、正常身份收录及基础去重检查。Pi／OMP 恢复后重新接入与重启的交叉用例归 07。

**Blocked by:** 02: 无匹配时恢复 Pi 历史 Chat。

**Status:** in-progress

- [x] 通过正常历史收录途径保存已知原 Workspace，覆盖发起 Chat 和 connect-bot 接入的适用来源；没有原 Workspace 信息时保持可选，不推断任意位置。
- [x] 使用真实临时持久文件并重启服务，确认已收录原 Workspace 可读取；其仍可用时，实际在该位置创建恢复目标，而非默认位置。
- [x] 原 Workspace 已删除时，唯一解析配置默认 Workspace，在该位置恢复，并在页面及操作结果中说明回退；不任意选择或新建替代 Workspace。
- [x] 原 Workspace 查询失败不得当作已删除而静默回退；明确反馈且没有后续创建或启动。默认 Workspace 的缺失、重名及查询失败基础用例由 02 拥有。
- [x] 定位已有对话、在原位置恢复或回退均不改写已收录的原 Workspace；回退后重新读取及重启后仍为原值。已有目标实际在别处也不覆盖原值。
- [x] 以独立预写旧格式记录证明无新增元数据仍可读取并发起 Resume，不要求手工重写历史；缺失信息不要求 Resume 补写。
- [x] 正常发起及 connect-bot 收录以完整原生身份去重，历史不依赖当前登记存在；删除登记不得删除历史或原生内容。
- [x] 用独立字面身份集合验证正常收录及重复收录前后、持久化与重启后的结果；预期值不得调用被测实现重新计算。同名同 cwd 的不同原生对话不能被合并。
- [x] 本票可基于 Pi 路径独立演示持久化恢复；不以等待 OMP 交付阻塞开工。两类 agent 恢复后实际正常接入、读取及重启的历史去重交叉检查由 07 唯一拥有。
- [x] 实际操作交付页面并观察真实创建目标，证明原位置恢复和删除后回退；更新现有历史兼容说明，明确原 Workspace 不随 Resume 更新。

## 覆盖分区

P7 全部覆盖归本票。P8：缺少原 Workspace 归 02，原 Workspace 已删除归本票。P9：默认 Workspace 缺失／重名／查询失败归 02，原 Workspace 查询失败归本票。

P21：旧格式读取、正常身份收录和基础去重归本票；Pi／OMP 恢复后正常接入与重启的组合行为归 07。不得在本票通过基础用例后提前宣称 P21 整体完成。

## Comments

2026-10-23：本票实现与所负责自动验收已完成，等待独立集成；按 triage-labels 规则，合入前保持 `in-progress`。`backend/tests/history_workspace.rs` 的 6 项 HTTP／持久文件／CLI 用例覆盖 P7、P8 删除原位置、P9 原位置查询失败与 P21 基础兼容／身份集合；正常发起与 connect-bot 路径均收录原位置，恢复、定位其他位置及回退后保持原值。前端构建与后端构建通过；两轮后端相关测试分别 25/25、18/18 通过（含重复运行本票 6 项）；真实 Chrome 页面 `historyWorkspace.spec.ts` 与 `chatResumePi.spec.ts` 共 2/2 通过，已观察受控 Herdr 创建于原位置及删除后默认位置，截图位于 `frontend/test-results/historyWorkspace-Resume-re-5ba06-s-deleted-original-fallback/`。受控 Herdr 不替代真实 Pi／OMP 对话验收；P21 跨 agent 恢复后再接入组合仍由 07 负责。
