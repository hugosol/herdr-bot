# 06: 模板问题提示与故障隔离

**What to build:** 用户可以在有效 Map 的依赖图上定位缺失或有问题的共享模板；模板说明不可用不破坏有效图，身份冲突不能靠任选文件掩盖，正常模板仍可查看。

**Delivers:** P19, P20, P21；P22 的模板结构覆盖；P29 的模板文件故障隔离覆盖。

**Test owner for:** P22 的模板固定结构必需字段、类型、未知字段、版本及显式空值与漏填；P29 的坏模板与正常模板共存。05 负责 Map 侧覆盖，各承诺在两票覆盖通过后才整体完成。

**Blocked by:** 04 — 共享模板关联与按需明细。

**Status:** in-progress

## Acceptance criteria

- [x] 模板缺失时保留预先确定的全部 node 与依赖边，在对应节点、详情及页面问题提示三处明确标出缺失模板 ID。
- [x] 分别使用真实不可读模板文件、非法 JSON、非法结构及不支持的 implementation.type 验证问题提示；有效图保持完整，不补造能力文本，不回退为 chat。
- [x] 模板及其固定嵌套结构严格检查必需字段、类型、未知结构字段及版本；分别提供与 Map 同类的独立反例，区分显式空对象、空文本与漏填，遵守已批准格式而不擅自加内容长度限制。
- [x] 多个文件声明同一模板 id 时，该 id 被明确标为冲突并列出相关文件；不按扫描顺序任选说明，也不隐藏冲突节点或删除其边。
- [x] 使用两个同 id 不同文本的模板和一个正常模板，验证冲突详情不选中任一业务说明，正常模板明细仍可查看。
- [x] 坏模板与正常模板同目录共存，分别查看相关 node；单文件错误不导致其余模板全部不可用，不用整个目录失败代替局部故障。
- [x] 通过 S1 观察图结构、三处缺失提示及正常明细；通过主界面实际使用的 S2 核对真实读取和格式错误。不新增测试专用读取 interface。
- [x] 场景使用测试拥有的真实文件与独立预期，保持只读、无执行及网络护栏；同步更新缺失、无效、未知类型及重复身份的排查说明。

## Contract coverage

P22 的模板结构覆盖与 P29 的模板文件隔离覆盖仅由本票编写测试；05 唯一负责 Map 侧对应覆盖。模板目录级创建／读取错误与空态由 04 覆盖，不能替代本票的 P20 单文件反例。

## Comments

- 本票实现及验收完成，等待独立集成；按仓库生命周期，合入前保持 `in-progress`，由集成方合入后设为 `closed`。
- S1/S2 证据：`frontend/browser-tests/schedulerTemplateProblems.spec.ts` 共 42 例，覆盖 P19 的完整 A → B 加独立 C 图及三处缺失提示；P20 的真实 Windows 文件 ACL 拒读、非法 JSON、结构与未知类型；P21 的双文件冲突及无效声明参与冲突；P22 的八个必需字段缺失／类型／null、嵌套 implementation、未知字段、版本及显式空对象／文本与长文本；P29 的坏文件与正常能力共存。场景核对正常明细、定义字节不变及受控 Herdr 无调用。
- TDD：缺失节点标记、非法 JSON 整目录失败、重复身份任选均先观察到对应行为失败，再实现通过。原有严格反序列化的结构检查直接验证既有行为；两例最初过度指定 enum 错误措辞，修正为合同要求的结构错误，保留拒绝／无伪造明细／正常文件可用的预期。
- 实际验证：`npm --prefix frontend run test:browser:build -- schedulerTemplateProblems.spec.ts schedulerTemplates.spec.ts schedulerGraph.spec.ts schedulerDirectory.spec.ts schedulerNavigation.spec.ts --workers=1`：57 passed；包括 TypeScript、前后端构建及网络护栏。`npm --prefix frontend test`：27 files / 205 tests passed。
- 完整后端回归 `node scripts/test-backend.mjs`：失败于未修改的 `zone_sync::configured_timeout_stops_the_running_command_and_keeps_effective_pull_state`（`backend/tests/zone_sync.rs:1181`，预期 200，实际 409 `git-write-in-progress`）；doctest 因此前失败未运行。未重跑或修改票外代码。日志：`C:/Users/hugos/AppData/Local/Temp/herdr-backend-tests-nj7wqU/summary.json`。本票自动验收无未完成项；跨票 P22/P29 仍须结合 05 的 Map 侧证据，合入后需集成验收。
