# 06 按现有 upstream 安全 Sync — 浏览器证据

## 环境

- 真实交付后端进程：`backend/target/debug/omp-bot.exe`，`127.0.0.1:8795`，使用本票构建的
  `backend/static` 打包页面。
- 隔离仓库：`C:\Users\hugos\AppData\Local\Temp\omp-bot-06-smoke\repo`，分支 `feature`，远端名为
  非默认的 `upstream`，bare 远端为 `C:\Users\hugos\AppData\Local\Temp\omp-bot-06-smoke\remote.git`。
- 登记文件与配置均在该临时目录；`[git] command_timeout_seconds = 20`。
- 浏览器：真实 Chromium；桌面 `1280×900` 请求视口（截图按 1024×720 输出），窄屏 `390×844` 视口仿真
  （不是实机）。
- 初始：本地 `21cf7d0`、远端 `1ea7ad1`，工作区干净；本地从未检查过远端。

## 场景与截图

| 截图 | 场景 |
|---|---|
| `desktop-sync-unchecked.png` | 干净、有 upstream、从未检查：显示 `upstream：upstream/feature`、`最近检查：尚未检查`、Sync 禁用与检查入口。 |
| `desktop-sync-checked.png` | 主动检查后：真实 fetch 更新引用，显示最近检查时间、`待拉取 1 个提交，待推送 0 个提交`，Sync 可用。 |
| `desktop-sync-confirm.png` | 点按 Sync 只出现一句确认消息与“取消 / 确认同步”；不重复 workspace、分支、upstream、数量或清单。 |
| `desktop-sync-success.png` | 确认后 fetch → pull（fast-forward）→ 无 push；页面显示 `已同步（upstream/feature）：拉取 1 个提交，推送 0 个提交`，主按钮禁用为“已同步”。 |
| `desktop-sync-push-rejected.png` | 远端独立前进、本地也有提交；pull 成功后 push 被远端 `pre-receive` 拒绝：页面保留 Git 原始诊断与“已完成的步骤（fetch → pull）不会被自动撤销”，刷新后显示仍需推送 2 个提交。 |
| `desktop-sync-recovered.png` | 移除拒绝 hook 后再次确认 Sync：fetch/pull 无新提交，按需 push 2 个提交成功，远端与本地一致。 |
| `narrow-sync-controls.png` | 窄屏 `390×844`：upstream、最近检查时间、检查远端与“已同步”主按钮均可见，按钮高 44px。 |
| `narrow-sync-checked.png` | 窄屏主动检查后的反馈与数量。 |
| `narrow-sync-confirm.png` | 窄屏确认框：取消 / 确认同步两个触摸目标均高 44px。 |
| `narrow-sync-error.png` | 窄屏 push 拒绝：`<script>`/`<img onerror>` 作为纯文本显示，没有被执行。 |

## 取消、check 与真实副作用核对

所有核对通过 CLI 直接读取本地仓库与 bare 远端，不依赖页面显示：

- **主动检查前**：本地 HEAD `21cf7d0`、远端 `1ea7ad1`，但本地远端跟踪引用仍是 `21cf7d0`；页面显示
  “尚未检查”，证明状态读取不会自动 fetch。
- **主动检查后**：`refs/remotes/upstream/feature` 更新为 `1ea7ad1`，本地 HEAD 与工作区不变；页面显示
  最近检查时间与 `待拉取 1 个提交`。
- **取消确认**：确认框关闭后没有发送 Sync 请求；随后确认成功时本地才从 `21cf7d0` fast-forward 到
  `1ea7ad1`，证明取消没有 fetch/pull/push 或本地提交。
- **首次 Sync**：本地与远端均为 `1ea7ad1`，远端跟踪引用相同，提交数 2，工作区干净，`remote.txt` 已存在；
  `branch.feature.remote=upstream` 未被改动。
- **push 拒绝**：本地由 pull 生成合并提交 `b9dfd2d`，父提交为 `dd82961`（本地）与 `889bfde`（远端）；
  远端仍停在 `889bfde`。本地提交树包含 `local.txt`、`remote.txt`、`remote2.txt`、`tracked.txt`，工作区
  干净，提交数 5、合并提交 1；页面刷新后显示 `待拉取 0 个提交，待推送 2 个提交`。没有自动撤销 pull
  或本地合并提交，也没有宣称整体回滚。
- **恢复**：移除拒绝 hook 后再次 Sync，本地与远端均为 `b9dfd2d`，远端跟踪引用同步更新。
- **诊断不执行**：在真实浏览器中读取 `sync-feedback`：文本完整包含
  `<script>window.__pwned = 1</script><img src=x onerror="window.__pwned = 2">`；
  `feedback.querySelectorAll('script')` 为 0，`img` 为 0，`'__pwned' in window` 为 false。
- **窄屏取消**：窄屏确认框取消后本地仍领先远端 1 个提交（`a5ea20c` vs `b9dfd2d`），没有产生推送。

## 自动化覆盖与限制

- `cd backend && cargo test --test workspace_sync`：15 passed。覆盖真实检查更新引用与检查时间、状态读取
  不 fetch、Sync 前重新检查、非默认 remote/分支、merge 与 rebase 配置、`pull.ff=only` 策略拒绝、
  只有 behind 时不产生 push（真实 pre-push hook 未触发）、push 拒绝后保留 pull 结果、fetch/pull 失败
  短路、普通目录/脏工作区/detached/无 upstream 拒绝、冲突与外部完成/撤销后恢复、嵌套仓库与子模块内部
  状态不变、Sync/Sync 与 Commit/Sync 并发及路径别名、断线不取消、超时终止派生执行与资格保持、非交互
  网络失败、分支切换目标、固定操作资格独立，以及封闭请求面。
- `cd backend && cargo test --lib sync`：2 passed。覆盖 Sync 终止未确认时保持仓库资格、确认终止或普通
  结果释放资格。
- `cd frontend && npm test`：146 passed（本文件新增 10 个交互用例）。覆盖主按钮逐态、主动检查只发一次并
  更新页面、一句确认与取消无请求、成功后刷新为已同步、执行中切 tab、整页重载不重放、响应中断未确认与
  不自动重试、Git 诊断纯文本、外部切分支后的显示、普通目录/状态读取失败不显示可执行 Sync。
- **未验证（human-led H1）**：真实手机触摸、软键盘与返回手感。本目录的窄屏截图只是 agent 准备的证据，
  不能冒充实机通过；H1 由 08 汇总并由用户确认。
