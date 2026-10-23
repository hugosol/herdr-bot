# 02: 无匹配时恢复 Pi 历史 Chat

**What to build:** 用户从历史列表恢复 Pi Chat 时，只有成功查找且无已有匹配，才在唯一解析的默认 Herdr Workspace 新建目标，使用历史工作目录加载同一 Pi 原生对话，确认加载后聚焦并反馈结果。不把恢复当作按 Bot 预设重新发起 Chat。

**Delivers:** P6、P10、P11、P12、P13、P14、P15 的 Pi 部分；P8 的缺少原 Workspace 部分；P9 的默认 Workspace 解析部分；P22 的 Pi 恢复确认部分。

**Test owner for:** P6、P10、P11、P13、P15、P22 的 Pi 自动检查；P8 的缺少原 Workspace 检查；P9 的默认 Workspace 检查；P12 的类型无效及 Pi 原生目标不可用检查；P14 的 Pi 设置检查。

**Blocked by:** 01: 从历史 Chat 定位已有原生对话。

**Status:** in-progress

- [x] 实际页面和独立 HTTP 请求均能触发 Pi 恢复；必须先成功查询且无匹配，已有目标或查询失败不得进入新建分支。
- [x] 记录缺少原 Workspace 时，按现有配置规则唯一解析默认 Workspace，新建恢复目标并在页面与操作结果中说明回退；无需 Resume 补写原 Workspace。
- [x] 默认 Workspace 不存在、存在同名歧义或查询失败时明确报错，无后续创建或启动；不任意选择、不自动创建替代 Workspace。
- [x] 优先使用可用会话文件。以独立文件和 ID 样例分别覆盖两者均存在、仅 ID、文件不可用；ID 回退只能加载同一原生对话，不能接受另一对话或空白对话作为替代。
- [x] 使用当前配置的 Pi 命令及所需集成，通过 Pi 原生 session 能力加载历史，不猜 agent 类型。
- [x] agent 类型缺失、未知或不支持时给出可见原因且不启动；Pi 原生对话不存在或不可用时明确失败，不把空白会话当作成功。
- [x] 使用记录中的 cwd，不静默替换目录；真实临时目录覆盖包含空格的合法路径，观察传入和实际目标 cwd；目录缺失时明确失败。
- [x] 用可辨认的 Bot 初始指示及与原生设置不同的 Bot 模型／思考级别作反例背景，证明不提交初始指示、不提交新输入、不施加 Bot 预设覆盖。agent 自身原生恢复规则不被宣称为本项目保证。
- [x] 恢复仅加载历史，不主动继续中断任务；自动观察 Pi session 加载与无输入提交，真实活动由 07 的 H-Pi 确认。
- [x] 只有确认正确对话恢复并完成聚焦才反馈成功；单纯创建 Tab 或提交启动命令不够。成功只描述 Herdr 内部导航，不声称 Windows 窗口已置前。
- [x] 使用真实请求驱动受控 Herdr，观察查找、创建、启动、确认及聚焦的阶段结果和无替代行为；实际操作交付页面观察恢复、回退和失败。受控参数不能代替真实 Pi 加载证据。
- [x] 更新现有用户或集成说明中的 Resume 行为及限制；不引入新的测试专用恢复接口，不复制一套与正常交付路径不同的恢复流程。

## 覆盖分区

本票拥有 P8 的缺少原 Workspace 场景、P9 的默认 Workspace 三种错误场景，以及 P12 的通用类型无效场景。原 Workspace 删除与查询失败归 04；OMP 对应原生行为归 03。

P6、P10、P11、P13、P14、P15、P22 的 Pi 自动分项归本票。P12 的 Pi 目标不可用分项也归本票。H-Pi 的原生身份、加载内容、焦点和输入／任务观察统一归 07 准备、用户确认；这些共享承诺不能因本票自动检查通过就整体标为完成。

## Comments

- 本票自动分项已实现并验证，待独立集成；按项目生命周期，合入前保留 `in-progress`，不把本次 invocation 当作已合入。
- S2/S3：`backend/tests/chat_resume_pi.rs` 的 7 个真实 HTTP／受控 CLI 检查覆盖文件优先、仅 ID、不可用文件回退、错误文件、缺失固定集成、含空格 cwd／实际 cwd 不符、默认 Workspace 三种错误、类型无效、目录缺失、启动／确认／聚焦失败，以及无 Bot 指示／模型／思考覆盖和无输入提交。原生设置反例为 `native-provider/native-model`、`low`，已保存 Bot 为 `bot-provider/bot-model`、`high` 和可辨认固定初始指示。
- TDD 红证据：首个 Pi 恢复检查实际得到 `not-found` 而非 `restored`；实际 cwd 反例实际得到错误成功；浏览器在 HTTP 已 `restored` 时未显示成功状态。分别修复后转绿；不是用 setup/import 失败充当行为红。
- `cargo test --test chat_resume_pi --test chat_resume --test chat_identity -- --nocapture`：19 passed；`cargo test --test chat_creation --test chat_creation_detached -- --nocapture`：36 passed。
- `npm run build`：通过（TypeScript + Vite）；`npm run test:browser -- chatResume.spec.ts chatResumePi.spec.ts --workers=1`：3 passed，真实交付页面观察确认恢复、默认位置回退、失败与不另开目标。截图由新浏览器用例保存为 `pi-resume-confirmed-fallback.png` 和 `pi-resume-failed.png`（Playwright test-results）。
- 未验证：真实 Pi 加载内容／原生身份／真实 Herdr 焦点及无任务活动仍为 07 的 H-Pi 和用户确认。本票的受控 `agent_get` 身份与实际工作目录观察不替代 H-Pi，也不将共享 P6 等整体承诺标为完成。
