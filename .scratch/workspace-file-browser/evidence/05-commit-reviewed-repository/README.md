# 05 经审查复核的整仓 Commit — 浏览器证据

## 环境

- 真实交付后端进程：`backend/target/debug/omp-bot.exe`，`127.0.0.1:8791`，使用本票构建的
  `backend/static` 打包页面。
- 隔离仓库：`C:\Users\hugos\AppData\Local\Temp\omp-bot-05-smoke\repo`，`main` 分支，另有
  `remote.git` bare 远端；初始提交 `4a04214e943dc7c6a6fb432b30fb8adb979fed15`。
- 登记文件与配置均在该临时目录；`[git] command_timeout_seconds = 20`。
- 浏览器：真实 Chromium；桌面 `1024×720`，窄屏为 `390×844` 视口仿真（不是实机）。
- 初始待提交状态：`tracked.txt` 修改、`deleted.txt` 删除、`added.txt` 未跟踪且已被外部部分暂存、
  `ignored.log` 被 `*.log` 忽略。

## 场景与截图

| 截图 | 场景 |
|---|---|
| `desktop-commit-bar.png` | 变更列表上方常驻提交信息与 Commit 主按钮；空信息时按钮禁用。 |
| `desktop-commit-message.png` | 输入提交信息后主按钮可用。 |
| `desktop-commit-confirm.png` | 点击 Commit 只弹出“确认提交当前审查版本中的全部待提交变更？”与取消/确认提交；不重复 workspace、分支、文件数、信息或清单。 |
| `desktop-hook-rejection.png` | 真实 `pre-commit` hook 拒绝后，页面显示 Git 返回的纯文本诊断（含 `<script>` 字面量），输入仍保留。 |
| `desktop-review-invalid.png` | 确认前外部把同一路径、同一数量的 `tracked.txt` 内容从 A 改成 B：拒绝当次提交，提示审查失效，提交信息保留并刷新实际内容。 |
| `desktop-commit-success.png` | 重新审查并再次确认后提交成功；变更列表变为空。 |
| `desktop-tab-switch-commit.png` | Commit 执行中切到会话列表再返回文件列表：执行未取消，返回后显示真实成功结果。 |
| `desktop-reload-during-commit.png` | Commit 执行中整页重载：默认回到会话列表，没有重放写入。 |
| `desktop-reload-actual-state.png` | 重载后重新进入变更视图：按实际仓库状态显示干净，没有假成功或自动重试。 |
| `narrow-commit-bar.png` | 窄屏 `390×844`：提交信息与 Commit 在变更列表上方，输入框与按钮高 44px。 |
| `narrow-commit-input.png` | 窄屏输入提交信息。 |
| `narrow-commit-confirm.png` | 窄屏确认框：取消与确认提交按钮均高 44px、宽 112px。 |
| `narrow-commit-success.png` | 窄屏提交成功，显示实际结果与空变更列表。 |

## 取消、失败与失效的独立副作用核对

所有核对通过 CLI 直接读取仓库与 bare 远端，不依赖页面显示：

- **取消确认**：HEAD 仍为 `4a04214…`，远端 `main` 仍为 `4a04214…`，索引仍只有先前外部暂存的
  `added.txt`，工作区状态不变；页面没有发送提交请求。
- **hook 拒绝**：HEAD 与远端不变；`git diff --cached --name-only` 为 `added.txt`、`deleted.txt`、
  `tracked.txt`，证明 `git add --all` 已成功而 `git commit` 失败后没有自动回滚。页面反馈文本完整显示
  `<script>window.__pwned = 1</script>`；`commit-feedback` 内 `script`/`img` 数量为 0，`window.__pwned`
  未出现。
- **审查失效**：把 `tracked.txt` 从 `review version A` 改成 `review version B`（同路径同数量）后确认，
  页面提示审查失效；HEAD、远端与索引均未变化，提交信息仍为 `invalidation evidence`。
- **真正提交**：提交后 HEAD `98dc4b34…`，提交树为 `.gitignore`、`added.txt`、`tracked.txt`；
  `deleted.txt` 已删除，`ignored.log` 仍不在树中；远端 `main` 保持 `4a04214…`（未 push）。
  外部部分暂存没有缩小范围：删除与未暂存修改同样进入提交。
- **窄屏提交**：HEAD `9df969ca…`，提交树包含 `mobile-added.txt`；远端仍为 `4a04214…`。
- **切换 tab**：提交前后 `rev-list --count HEAD` 从 3 增加到 4，页面返回后显示
  “已提交 605f3c20：commit survives tab switch”；远端不变。
- **整页重载**：提交执行期间重载，`rev-list --count HEAD` 从 4 增加到 5 且只增加一次；提交信息为
  `reload does not replay`；远端不变。重载后默认会话列表，再进入变更视图显示实际干净状态。

## 自动化覆盖与限制

- `backend/tests/workspace_commit.rs` 的 19 个集成用例覆盖：完整待提交集合与忽略/已跟踪例外、最终提交树、
  不 push、同路径同数量内容变化、外部暂存、HEAD 变化、切分支、超限/二进制内容变化、空信息与封闭请求面、
  hook 拒绝保留索引、无 HEAD 首提交、普通目录/detached/冲突/未完成流程、嵌套仓库与子模块 gitlink 范围、
  并发与断线、路径别名、超时与派生进程终止、资格独立、缺失目录、无 upstream、编辑器/凭据/签名条件、
  预览限制不改变提交资格、干净仓库拒绝空提交。
- `frontend/tests/commit.test.tsx` 的 13 个交互用例覆盖常驻输入与按钮、空信息、tab/视图往返保留、一句确认、
  取消无写、成功、审查失效后保留信息并再次确认、响应中断未确认、执行中切 tab、重载不重放、HTML/script
  纯文本、超时文字、普通目录/detached/冲突阻塞。
- **未验证（human-led H1）**：真实手机触摸、软键盘弹出后的可访问性与返回手感。本目录的窄屏截图与上述
  触摸目标尺寸只是 agent 准备的证据，不能冒充实机通过。
