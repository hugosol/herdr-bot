# 08 跨流程验收与手机实机 H1 证据包

**状态：自动检查与 agent 侧真实浏览器证据已完成；手机实机 H1 未验收，等待用户确认。**

本目录是唯一的手机实机 H1 交接包，覆盖 P4、P26、P27、P28。窄屏截图来自 Chromium 的
`390×844` 视口仿真，**不是手机实机结论**，不能代替真实触摸、软键盘与手感确认。

> **可信局域网提醒（部署前提，不会因本功能改变）**：本服务没有登录、token 或来源认证。任何能访问
> 本服务端口的设备都可以读取已登记 workspace 中的文件内容（包括敏感文件），并可以触发已开放的
> Commit / Sync 写操作。会话事件写入的同机来源校验只限制那一类写入，**不是认证**，也不限制手机
> 使用文件浏览与 Git 操作。不要把本服务暴露到公网、访客网络或不可信 Wi-Fi；本功能没有增加登录、
> 多用户权限或公网部署能力，也没有任意命令、脚本或终端入口。

## 当前可用环境（agent 实测）

| 项 | 值 |
|---|---|
| 主机系统 | Windows 10.0.26200（x64） |
| 自动检查浏览器 | managed Chromium（headless，共享实例） |
| 窄屏证据视口 | `390×844` CSS px（截图按 1.25 设备像素比输出为 488×1055） |
| 宽屏证据视口 | `1280×900` CSS px（截图输出 1600×1125） |
| 交付进程 | `backend/target/debug/omp-bot.exe --config %TEMP%\omp-bot-08-h1\omp-bot.toml` |
| 监听 | `0.0.0.0:8787` |
| 当前 WLAN IPv4 | `192.168.5.3`（接口 `WLAN 2`，网络类别 `Public`） |
| 局域网可达性 | 本机经 `http://192.168.5.3:8787/` 请求页面与 `/api/workspaces` 均返回 HTTP 200 |
| mDNS 入口 | 与启动日志的 `published` 行为准；解析不了时使用上面的 IP |
| 固定操作 | 绑定仓库内 `fixtures/operations/success.bat`，不触发用户真实程序 |

`192.168.5.3` 是本次实测值，DHCP 重新分配、换 Wi-Fi 或重启路由器后可能变化；H1 时以启动日志与
`ipconfig` 为准。

### 待用户补充的真实手机环境

| 项 | 用户填写 |
|---|---|
| 手机型号 | |
| 系统版本（Android/iOS/HarmonyOS 等） | |
| 浏览器与版本 | |
| 连接 Wi-Fi / 网段 | |
| 实际打开地址 | |
| H1 日期 | |

## 可复现的隔离场景

两个脚本都在本目录，均只操作 `%TEMP%\omp-bot-08-h1`，不触碰用户工作仓库或业务远端：

```powershell
# 1. 建立/重置隔离场景（可重复执行）
powershell -NoProfile -ExecutionPolicy Bypass -File prepare-scenario.ps1

# 2. 构建（若尚未构建）
npm --prefix frontend run build
cargo build --manifest-path backend\Cargo.toml

# 3. 启动交付进程（在控制台窗口运行）
powershell -NoProfile -ExecutionPolicy Bypass -File start-server.ps1
```

场景内容：

| 路径 | 内容 |
|---|---|
| `%TEMP%\omp-bot-08-h1\repo` | 分支 `feature`，有 upstream；初始 6 项待提交（修改、删除、已识别重命名、新增、未跟踪） |
| `%TEMP%\omp-bot-08-h1\clean-repo` | 分支 `main`，干净；远端已独立前进 1 个提交，用于检查远端与 Sync |
| `%TEMP%\omp-bot-08-h1\workspaces.json` | 两个 workspace 的隔离登记文件 |
| `%TEMP%\omp-bot-08-h1\omp-bot.toml` | 隔离配置；固定操作绑定安全夹具 |
| `%TEMP%\omp-bot-08-h1\remote.git`、`clean-remote.git` | 两个隔离 bare 远端 |

H1 后若要重来，重新运行 `prepare-scenario.ps1` 即可。它会删除并重建这个临时目录；不会删除仓库内的
任何文件。

## 局域网访问步骤（当前实测可用）

1. 在电脑上启动交付进程，确认控制台打印监听地址与 mDNS 状态。
2. 电脑与手机连接**同一个普通 Wi-Fi**（不要访客网络、热点或启用了 AP 隔离的网络）。
3. 手机上打开 `http://192.168.5.3:8787/`；解析得到时也可用 `http://omp-bot.local:8787/`。
   **不要在手机上用 `127.0.0.1`**（那指向手机自己）。
4. 预期看到标题 “omp-bot”、三个等宽入口（会话列表 / 文件列表 / 控制面板），默认停在
   “会话列表”。
5. 打不开时：
   - 确认手机与电脑同一网段，电脑防火墙允许 `omp-bot.exe`（本机当前规则覆盖公用网络，但只放行
     旧路径的 exe；换 worktree 或改路径后需重新放行）；
   - 路由器“AP 隔离/客户端隔离”、访客网络、VPN/代理会阻断；
   - 用 IP 兜底，不要只依赖 mDNS。

## 手机实机 H1 检查清单（待用户逐项确认）

以下项目全部是 **H1 待确认**，当前没有一项被标为通过。请在隔离场景中操作，不需要向业务远端推送，
也不要求人工验证 Git 语义正确性（那部分已由自动检查与 CLI 独立核对）。

### P4：顶层入口触摸与选中态

- [ ] 打开页面默认停在“会话列表”，三个入口等宽、排列为“会话列表 → 文件列表 → 控制面板”。
- [ ] 依次触摸三个入口：选中态清晰，当前内容随之切换。
- [ ] 在文件列表/变更列表中左右滑动滚动时，不误切换顶层 tab。
- [ ] 触摸目标大小与间距可接受；不会因为入口太窄而点错。

### P26：逐层进入、返回与 diff 滚动

- [ ] 文件列表中点“浏览”进入 `repo`，看到文件/目录列表；分支显示 `feature`。
- [ ] 进入 `src` 目录，再打开 `long.txt`；内容为只读文本，页面没有编辑入口。
- [ ] 点“返回文件列表”回到原列表，之前的列表滚动位置与进入层级符合预期。
- [ ] 切到“变更”，点 `src/long.txt`：显示统一 diff（不是左右双栏）；上下滚动顺畅。
- [ ] 点“返回变更列表”回到列表，列表滚动位置保持；再进入其他变更仍可用。
- [ ] 变更列表较长时（场景中可重跑脚本或外部新增文件制造更多项）滚动、选择与返回不丢位置。

### P27：软键盘出现后的输入框与主按钮可达性

- [ ] 在“变更”视图中，提交信息输入框与 `Commit` 主按钮位于变更列表上方。
- [ ] 点按提交信息输入框，软键盘弹出后输入框仍可见、可输入；页面没有被键盘遮住无法操作。
- [ ] 键盘弹出时 `Commit` 按钮仍可触摸（必要时页面可滚动到它）；空信息时按钮不可用，输入后可用。
- [ ] 收起键盘后，输入内容仍保留；切换 tab 或内部视图再回来仍保留。

### P28：确认/取消触摸目标

- [ ] 点 `Commit` 只出现一句确认消息与“取消 / 确认提交”两个按钮，不重复展示 workspace、分支、
      文件数或清单。
- [ ] 点“取消”：确认框关闭，没有产生提交（可让 agent 用 CLI 核对 HEAD 不变，或直接相信页面行为）。
- [ ] 再次点 `Commit` 并点“确认提交”：显示成功结果；提交信息与主按钮状态随之更新。
- [ ] `clean-repo` 中点“检查远端”，再点 `Sync`：确认框同样只有一句消息与“取消 / 确认同步”。
- [ ] 点“取消”：没有拉取/推送；再次确认后显示已同步，且不产生无意义推送。
- [ ] 两个确认框的按钮都易于触摸，不会误触；取消与确认边界清楚。

### 记录方式

在每项后填写“通过 / 不通过 / 无法完成 + 现象”，并附手机截图（确认/取消弹框至少各一张）。若某
尺寸或布局与预期不同，记录手机型号、系统、浏览器版本与当时地址。

## Agent 侧已完成的自动与真实浏览器证据

### 自动检查（本票新增）

| 证据 | 结果 | 覆盖 |
|---|---|---|
| `cargo test --test workspace_integration` | `3 passed` | P42 本地事件不 fetch；P46 局域网调用者可浏览/Commit、会话写入仍被拒、无命令入口；P3 监听期间固定操作/会话/关机命令不变 |
| `npx vitest run tests/integration.test.tsx`（在 `frontend/` 执行） | `4 passed` | P3 全局关机提醒跨三个 tab 可见；固定操作仍可用；P41 节流窗口内进入/返回/手动刷新立即读到当前事实；读取失败不被伪装成空目录或干净仓库 |
| `cargo test --no-fail-fast` | `286 passed / 34 suites`（0 failed；本票新增 3 个集成用例，既有固定操作、会话、关机、mDNS、浏览、Git、Commit、Sync、监听套件全部运行） | 01–07 的自动检查回归 |
| `npm --prefix frontend run test` | `21 files / 150 tests passed` | 现有前端全部交互与新增 4 例 |
| `npm --prefix frontend run build` | 成功（`tsc --noEmit && vite build`） | 交付页面构建 |
| `cargo test --test mdns_conflict --test mdns_failure` | `3 passed` | 首次全量运行与前端构建并发时出现 404，构建完成后单独重跑通过；失败原因是静态页面目录正在被替换，不是产品回归 |

### 真实 Chromium 窄屏（390×844 视口仿真）观测

| 观测 | 结果 |
|---|---|
| 三个顶层入口宽度 | `116.66 / 116.67 / 116.67` CSS px；默认选中“会话列表” |
| 文件浏览 | `repo` 根目录列出 `.git`、`notes`、`src`、`.gitignore`、`added.txt`、`ignored.log`、`tracked.txt`；`deleted.txt` 不在树中 |
| 文件内容 | 打开 `tracked.txt` 显示只读文本 `tracked changed`，有“返回文件列表” |
| 变更视图 | 6 项：删除红 `rgb(255,123,114)`、修改/识别重命名蓝 `rgb(121,192,255)`、新增绿 `rgb(126,231,135)`；重命名显示“旧路径 → 新路径” |
| 统一 diff | `data-mode="unified"`；长文件 92 行；滚动页面 900px 后仍可继续查看 |
| Commit 门禁 | 输入框 218×44、Commit 112×44；确认框按钮均 112×44；只有一句消息与取消/确认 |
| 取消 | 关闭确认框，未发送写请求；HEAD 保持 `4eda6ef…` |
| 审查失效 | 外部改 `tracked.txt` 后确认：页面显示 `审查版本已失效…`，提交信息保留，HEAD 仍为 `4eda6ef…` |
| 提交成功 | 新 HEAD `f55983e9…`；提交树含 `.gitignore`、`added.txt`、`notes/untracked.txt`、`src/long.txt`、`src/renamed.txt`、`tracked.txt`；`ignored.log` 与被删除文件不在树中 |
| Sync 检查 | 检查前本地 HEAD `a4849b8…`、远端跟踪 `a4849b8…`、远端 `a86e48d…`；检查后跟踪引用变为 `a86e48d…`，HEAD 不变 |
| Sync 取消 | 未拉取；HEAD 仍 `a4849b8…` |
| Sync 成功 | HEAD 与远端均为 `a86e48d…`，页面显示“拉取 1 个提交，推送 0 个提交”，无无意义 push |
| 回到页面绕过节流 | 第一个本地事件自动刷新 23ms；窗口内第二个事件 1.5s 后仍未自动显示；切到会话列表再返回后 81ms 显示 |
| 手动刷新绕过节流 | 窗口内新文件 500ms 后仍未自动显示；点“刷新”后 56ms 显示 |
| 进入 workspace 绕过节流 | 外部新文件在重新进入 workspace 后 14ms 显示 |
| 长列表返回 | 变更列表 `scrollHeight=1182 / clientHeight=504`；`scrollTop=300` 进入 diff 再返回后仍为 300 |
| 本地事件不 fetch | 4 次本地事件后远端跟踪引用仍为 `4eda6ef…`，`remoteCheckedAt=null`；远端实际为 `1d8c37c…`；显式“检查远端”后引用才变为 `1d8c37c…` |
| 宽屏布局 | 左树右内容；树宽 352 CSS px、内容从 701.5 CSS px 开始；左右对照 diff 180/180 行 |

## 截图索引

| 文件 | 内容 |
|---|---|
| `narrow-01-sessions-tabs.png` | 窄屏默认会话列表与三个等宽入口 |
| `narrow-02-workspaces.png` | 文件列表与两个隔离 workspace |
| `narrow-03-browse-tree.png` | 进入 `repo` 的浏览树、分支 `feature` |
| `narrow-04-file-content.png` | 只读文件内容与明确返回 |
| `narrow-05-changes.png` | 变更列表、颜色、重命名、常驻提交栏 |
| `narrow-06-diff-top.png` | 统一 diff 顶部 |
| `narrow-07-diff-scrolled.png` | 统一 diff 滚动后 |
| `narrow-08-commit-message.png` | 提交信息已输入、Commit 可用 |
| `narrow-09-commit-confirm.png` | Commit 一句确认与取消/确认 |
| `narrow-10-review-invalid.png` | 审查失效、提交信息保留、无提交 |
| `narrow-11-commit-success.png` | 提交成功后的实际干净状态 |
| `narrow-12-sync-checked.png` | 检查远端后的时间与 ahead/behind |
| `narrow-13-sync-confirm.png` | Sync 一句确认与取消/确认 |
| `narrow-14-sync-success.png` | 同步成功的实际状态 |
| `narrow-15-return-current.png` | 回到页面后显示窗口内新文件 |
| `narrow-16-manual-refresh-current.png` | 手动刷新后显示窗口内新文件 |
| `narrow-17-enter-current.png` | 重新进入 workspace 后显示新文件 |
| `narrow-18-explicit-check-remote.png` | 本地事件后显式检查远端 |
| `narrow-19-changes-scrolled.png` | 变更列表滚动到 300 |
| `narrow-20-changes-scroll-restored.png` | 从 diff 返回后滚动位置恢复 |
| `desktop-01-browse-layout.png` | 宽屏左树右内容 |
| `desktop-02-side-by-side-diff.png` | 宽屏左右对照 diff |

## 限制与未验证事项

- **手机实机全部未验证**：没有真实手机的触摸、软键盘、浏览器地址栏/安全区、滚动与返回手感证据。
  本目录的窄屏截图是视口仿真，不能冒充实机通过。
- 手机型号、系统、浏览器与局域网访问地址待用户填写。
- 没有执行真实关机；关机提醒的跨 tab 可见性由前端测试中的权威观察快照验证，未在 H1 场景中启动
  真实倒计时。
- 没有向任何业务远端推送；Git 结果只针对 `%TEMP%\omp-bot-08-h1` 下的隔离仓库与 bare 远端。
- 真实浏览器为 headless managed Chromium；真实手机上的字体、键盘、系统浏览器工具栏与安全区可能
  改变可用高度。
