# Bot 模板真实浏览器验收（P49）

## 边界与运行环境

2026-10-22 在本票 worktree 使用 Windows x64、本机 Google Chrome 155.0.8059.85（Playwright `channel: chrome`）操作：

| 档位 | CSS 视口 | 输入方式 |
| --- | --- | --- |
| 桌面 | 1280 × 900 | 浏览器点击、键盘输入、下拉选择 |
| 移动视口 | 390 × 844 | 同上，仅缩小视口，不是手机触摸输入 |

入口是构建后的真实 Axum 服务，提供 Vite 生产页面和同源 `/api/`。每档启动独立服务、真实临时持久文件和 zone 目录；没有前端路由拦截、内存原型或合成 API 响应。选项目录复制自本仓库 `meta-data/bot-options.toml`，保留实际 toolkit 入口路径；故障只改隔离副本，不改部署元数据或用户数据。本例不配置受控 Herdr，不启动真实 agent；创建请求在真实文件依赖检查时即被拒绝。

**本证据只证明 P49。** 不证明 P50/H2 的真实手机触摸、软键盘、输入法、浏览器缩放、局域网访问或真实扩展/Skill 执行质量。为 08 准备 H2 时应复用下列步骤，但必须在真实手机上另外操作并记录。

## 可复现执行

在仓库根目录：

```bat
npm ci --prefix frontend
npm run build --prefix frontend
cargo build --manifest-path backend/Cargo.toml --bins
cd frontend
npx playwright test responsiveTemplates.spec.ts
```

完整复用的相关行为检查：

```bat
npx playwright test responsiveTemplates.spec.ts steppedTemplates.spec.ts staleSelectionRecovery.spec.ts
```

`responsiveTemplates.spec.ts` 是 P49 的生产浏览器 seam，不测试私有组件组织。截图、浏览器版本、具体点击顺序与公开 HTTP 响应会重新生成在 `frontend/test-results/responsiveTemplates-*/`。服务随机端口和 Bot 短标识每次不同；重启服务也可能更换端口。用例退出时清理其隔离数据。

## 实际操作与观察

以下操作在两种视口各完整执行一次；具体 HTTP 状态、正文和按钮顺序见 [桌面记录](evidence/bot-template-responsive/desktop-operations.json) 与 [移动视口记录](evidence/bot-template-responsive/mobile-viewport-operations.json)。

1. 进入 `bot`，新增模板。填写 `P49 发布巡检`；在命名编辑器输入“巡检 ”并逐个点击四类字段插入按钮：`zone.name`、`template.name`、`agent`、`bot.id`。预览为 `巡检 alphaP49 发布巡检ompa7f3c2`。
2. 选择 OMP、`gpt-6.1-sol`、`high`、`/code-review`，输入包含两行、中文、引号与 `C:\alpha` 的固定 prompt。进入第二步，勾选 `codebase-tools` 和 `connect-bot`；固定 `model-cost` 仍显示为始终加载。
3. 返回第一步逐项核对具体草稿值，再进入第二步核对两个勾选。进入摘要逐项核对名称、命名规则、Agent、模型、thinking、Skill、prompt、固定/可选扩展和“自动发现关闭”。此时公开 GET 仍是空模板列表。滚动到保存按钮，明确点击后才通过公开 GET 读到完整模板。
4. 编辑同一模板，名称改为 `P49 编辑后巡检`，规则改为 `编辑 {zone.name} / {template.name} / {agent} / {bot.id}`。实际切换 Pi，看到模型和 thinking 联动回到 `default`；再选择 `deepseek-flash`、`off`、`/teach` 和两行新 prompt。确认 Pi 可选扩展没有继承 OMP 勾选，明确勾选 Pi `codebase-tools`。前后往返核对 prompt、thinking 与勾选保留。
5. **持久保存失败：** 在真实存储旁创建同名 `.tmp` 目录阻断临时文件写入，点击保存。页面显示“保存 Bot 模板失败…此前已保存的模板保持不变”，公开 GET 仍返回原模板。移除阻挡，往返检查编辑草稿仍在，重新明确保存后公开 GET 返回全部编辑值。
6. **失效选择及修正：** 从隔离 TOML 删除已勾选的 `pi-codebase-tools` 定义，重新编辑。同一 ID 仍可见、勾选并标注“不可用”；前进摘要点击保存被拒绝，原持久模板未变。返回第二步实际取消失效勾选，摘要显示“无”，保存后公开 GET 确认 `extensions: []`。恢复隔离元数据。
7. **创建前依赖失败：** 将隔离元数据中的 Pi 固定扩展入口指向临时目录内不存在的 `missing-model-cost.ts`。登记真实 zone 目录，从 zone 的 `+` 打开创建弹窗，输入两行 input，点击创建。真实 `/api/bots` 返回 `extension-entrypoint-invalid`；页面明确给出扩展、完整路径、“入口不存在”以及“未创建 Herdr Tab、未启动 agent、未提交输入”，不出现成功。input 保留，滚动后能实际点击关闭。恢复元数据后重新打开，名称预览恢复可用并可关闭；**没有提交一次成功创建，也不将预览视为依赖检查已成功或真实 agent 启动证据。**
8. 重启真实服务，公开 GET 仍读到已修正编辑模板及空扩展列表。

每个截图状态检查页面无横向溢出、弹窗边界位于视口内。长内容在弹窗内部滚动；保存、上一步、下一步、取消/关闭均通过真实浏览器操作，不仅检查元素存在。没有发现需要修复的阻断布局问题，因此没有修改生产界面，也未隐藏字段、改只读或改动命名编辑/模型联动/错误语义。

## 截图索引

截图保留实际视口，不拼接为原型图；移动预设和摘要需要内部纵向滚动，所以另附底部操作区截图。

| 状态 | 桌面 | 移动视口 |
| --- | --- | --- |
| 命名编辑及四类插入 | [预设](evidence/bot-template-responsive/desktop-preset.png) | [预设](evidence/bot-template-responsive/mobile-viewport-preset.png) |
| 固定扩展及真实勾选 | [扩展](evidence/bot-template-responsive/desktop-extensions.png) | [扩展](evidence/bot-template-responsive/mobile-viewport-extensions.png) |
| 摘要内容 | [摘要](evidence/bot-template-responsive/desktop-summary.png) | [摘要](evidence/bot-template-responsive/mobile-viewport-summary.png) |
| 摘要底部保存入口 | [操作区](evidence/bot-template-responsive/desktop-summary-actions.png) | [操作区](evidence/bot-template-responsive/mobile-viewport-summary-actions.png) |
| 磁盘写入失败 | [错误](evidence/bot-template-responsive/desktop-persistence-error.png) | [错误](evidence/bot-template-responsive/mobile-viewport-persistence-error.png) |
| 失效勾选与修正入口 | [失效选择](evidence/bot-template-responsive/desktop-unavailable-selection.png) | [失效选择](evidence/bot-template-responsive/mobile-viewport-unavailable-selection.png) |
| 创建前真实依赖失败 | [错误](evidence/bot-template-responsive/desktop-dependency-error.png) | [错误](evidence/bot-template-responsive/mobile-viewport-dependency-error.png) |
| 失败后的关闭/继续入口 | [操作区](evidence/bot-template-responsive/desktop-dependency-actions.png) | [操作区](evidence/bot-template-responsive/mobile-viewport-dependency-actions.png) |

## TDD 与验收说明

本票的批准 seam 是生产浏览器操作及公开 HTTP，而非私有 DOM 结构。既有三步、保存与失效修正行为已实现，因此没有人为制造生产代码 red；新验收先执行实际页面，观察到的唯一失败是验收场景的元数据删除表达式未兼容 Windows CRLF（两档均在同一 fixture 前置检查失败）。仅修正表达式，不更改预期或产品行为后，两档工作流通过。

实际验证：`npm ci --prefix frontend` 成功；`npm run build --prefix frontend && cargo build --manifest-path backend/Cargo.toml --bins` 成功；单独 P49 工作流 **2 passed**；最终运行 `npx playwright test responsiveTemplates.spec.ts steppedTemplates.spec.ts staleSelectionRecovery.spec.ts` 为 **17 passed（19.2s）**。仅有 Node 的 `NO_COLOR` / `FORCE_COLOR` 环境提示，不影响结果。后两份既有用例包含受控 Herdr，其通过不转换为真实 agent 证据。本票未运行全仓测试。

判定准则来自本票：可操作性、具体草稿往返、明确保存、错误可理解并可继续修正、无阻断横向溢出。独立预期使用上述固定字面值，公开读取核对持久结果；真实文件故障保留了负责拒绝与错误解释的生产机制。
