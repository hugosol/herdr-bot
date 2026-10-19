# 07 自动刷新与监听健康：真实浏览器证据

环境：Windows 10.0.26200，本仓库构建的 Axum 交付进程（`backend/target/debug/omp-bot.exe`），
`refresh_interval_seconds = 2` 的隔离配置；真实 Chromium 页面（headless managed Chromium），
workspace 与 Git 仓库都在系统临时目录，不触碰用户工作仓库。

## 已观察结果

1. **普通目录外部新增自动出现**（`desktop-auto-refresh.png`）
   - 页面停在 workspace 浏览视图，未点击“刷新”。
   - 通过 `std::fs` 在外部创建 `auto.txt` 后，12ms 内浏览树出现该文件；截图时树中为 `auto.txt`、`README`。
2. **Git 外部切分支自动更新**（`desktop-git-branch-refresh.png`）
   - 页面显示 `分支：main`。
   - 外部执行 `git switch -c browser-feature-evidence`（只写 `.git` 元数据）后，20ms 内页面显示
     `分支：browser-feature-evidence`，未点击“刷新”。
3. **真实监听失效明确显示错误**（`desktop-watch-error.png`）
   - 删除被监听的 workspace 目录后，页面出现：
     `自动更新已停止：workspace 目录已不存在或无法访问，自动更新已停止：系统找不到指定的文件。 (os error 2)`
     并明确说明“当前内容可能已过期”。
   - 页面没有显示“已恢复”或继续宣称自动更新正常。
4. **监听失效后手动读取仍取得当前事实**（`desktop-manual-refresh-after-failure.png`）
   - 重建同名目录并写入 `current.txt`，点击“刷新”后浏览树显示 `current.txt`；
     “自动更新已停止”提示保持可见，正确表示监听仍未恢复。

## 自动化交叉证据

- `cd backend && cargo test --test workspace_watch`：7 passed。覆盖真实文件系统增删改、真实 Git 切分支与
  待提交状态、链接工作树外部 Git 元数据、两个观察者共享刷新代数、缺省 10 秒窗口、窗口内手动读取
  立即取得当前事实，以及删除/重建目录后的监听错误与手动读取。
- `cd frontend && npm test`：115 passed。覆盖真实 SSE 数据源接线、自动刷新触发目录与角标重读、
  监听错误/连接中断提示、手动刷新绕过监听状态，以及连接恢复只由完整 `snapshot` 完成。
- 故障注入核对：关闭健康检查时监听失效用例超时；移除外部 Git 元数据监听时链接工作树用例超时；
  让窗口到点不补刷新时多观察者用例超时；均恢复正确实现后重新通过。
