# 01: 同 Tab 模板浏览、准确预览与诊断

**What to build:** 在 scheduler 的“刷新全部”旁提供模板管理入口，在同一 Tab 查看共享 node_template 列表和可读预览。node 的“模板明细”进入同一套预览，不再使用旧内联展开。缺失、损坏或身份歧义得到真实诊断，浏览不改写定义。

**Delivers:** P2, P3, P5, P35；P1 的入口与同 Tab 导航部分。

**Test owner for:** P1 的入口位置、同 Tab 列表和不新开浏览器标签用例。P1 的空列表进入新增草稿由 02 实现并测试；两部分通过后 P1 才完成。

**Blocked by:** None (can start immediately).

**Status:** in-progress

## 依据与验收方式

依据本 feature 已批准的 contract 与 spec。S1 是实际交付页面与真实运行应用，S2 是页面实际调用的生产 scheduler HTTP interface；真实隔离文件是副作用证据，不新增测试专用 seam。复用已有模板身份、目录边界和诊断规则。

- [x] 模板管理入口位于“刷新全部”旁；进入同一 Tab 的模板列表，不创建浏览器标签。空目录显示明确空态；可用新增流程由 02 完成，不把本票完成声明为完整 P1 已交付。
- [x] 用预先指定的模板 ID、标题、适用场景、能力说明、命名输入输出及 chat 实现类型核对可读预览。预览不是 JSON 编辑器，不运行能力。
- [x] 多个 node 引用相同及不同模板时，“模板明细”进入准确身份的同一套预览；移除旧内联明细及与其绑定的过时交互预期，不保留两套入口行为。
- [x] 通过真实空目录、读取故障、缺失引用和重复 ID，验证可区分诊断；健康对象仍可查看，不任选歧义模板作成功预览。S1 与 S2 均验证对应可观察结果。
- [x] 在正常、缺失、损坏及恢复场景中，比较浏览前后的已有定义文件字节；不自动修复、安装示例或改写定义。保留原有缺失配置目录初始化行为。
- [x] 更新当前模板浏览使用说明；不更改历史 ADR 或扩大到调度、执行工作及 Map 管理。

## 验证与资源约束

本票拥有 P2、P3、P5、P35 的 contract 用例及上列 P1 分区。按项目测试指引执行 TDD 和实际页面验收，页面使用当前构建产物与规定 npm 入口，不以模拟 HTTP 页面代替真实服务。预期值独立指定，不调用实现计算预期。测试独占真实临时定义目录、服务工作目录、持久化路径、进程及数值回环端口；不触碰用户数据、不发起 Chat 或执行真实固定操作。跨操作执行安全的统一 contract 验证由 06 负责，当前浏览路径仍必须遵守安全约束。

## Comments

- 首次执行：以获批 S1（实际页面）及 S2（生产 scheduler HTTP）验收，无新增内部测试 seam。入口、可读预览、管理页真实目录错误三个纵向切片均先观察行为红灯，再实现并确认绿灯；初次入口测试另修正了 `aside` 的可访问角色（complementary 而非 region），未改变验收含义。
- P1 本票分区：`schedulerTemplateBrowsing.spec.ts` 的入口用例核对相邻按钮、同 Tab 列表、无新浏览器页、真实空目录和返回 Map。空列表新增仍由 02 负责，未声明完整 P1。
- P2／P3：`schedulerTemplates.spec.ts` 的 `P2 P3` 用例逐项核对独立业务字面量和 ID，通过列表、同模板两个 node、不同模板 node 进入同一可读预览；确认没有 JSON 编辑器、旧内联区域或 HTML 执行。所有受影响旧浏览器交互已迁移为显式返回导航。
- P5／P35：`schedulerTemplateBrowsing.spec.ts` 的恢复用例在同进程中外部移走、损坏、恢复文件，对比每阶段完整定义文件集合与字节，并通过 S1／S2 确认缺失、损坏、健康对象及恢复结果。`schedulerTemplates.spec.ts` 验证真实 Windows 目录 ACL 读取失败与恢复；`schedulerTemplateProblems.spec.ts` 验证单文件 ACL 错误、重复 ID、缺失引用及健康对象隔离。既有安全用例继续核对无执行、目录初始化、坏文件保留和真实 HTTP 读取。
- 实际运行：`npm --prefix frontend run test:browser -- scheduler --workers=4` 为 86 passed（加入最后一个恢复用例前）；随后 `npm --prefix frontend run test:browser:build -- --workers=4` 构建当前前后端与安全壳、网络护栏通过，完整浏览器 **246 passed**。桌面 `template-preview.png` 与窄屏 `narrow-template-details.png` 位于对应 `frontend/test-results/`，已实际查看。README 与样例使用说明已更新；截图与自动可操作性不代替后续 P36 主观验收。
- 其他实际验证：`npm --prefix frontend test` **27 files／205 tests passed**；`npm --prefix frontend run test:browser:partition` 通过，完整 246 = fast 222 + time-contract 24，且两分区不相交。后端生产代码未修改，未另跑完整后端回归（浏览器入口已构建并运行真实后端）。
- 本票实现与自动验收已完成，交独立集成代理；依项目状态规则，尚未合入集成分支前保留 `in-progress`，不把 agent 完成等同于已合并。未操作集成分支工作区，未运行 retro。
