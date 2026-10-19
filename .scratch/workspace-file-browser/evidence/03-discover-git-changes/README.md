# 03 — 准确定位 Git 待提交变更：真实 Git 与真实浏览器证据

日期：2026-10-19。环境：Windows 10.0.26200，Git 2.53.0.windows.2，omp-bot 后端以隔离配置在
`127.0.0.1:8799` 启动，真实 Chromium（omp 管理的无头 Chromium）驱动页面。未使用或修改用户的业务仓库，
未调用真实关机，未对真实远端推送。

## 隔离场景

- 仓库：`C:\Users\hugos\AppData\Local\Temp\omp-bot-evidence-03\repo`（临时目录，测试结束后可删除）。
- 登记文件：`C:\Users\hugos\AppData\Local\Temp\omp-bot-evidence-03\workspaces.json`。
- 仓库初始提交后，用真实 Git 构造下列事实：
  - `src/conflict.txt`：`side` 与 `main` 双方修改后真实 `git merge` 冲突（`UU`）。
  - `src/modified.txt`：已跟踪修改。
  - `src/deleted.txt`、`src/nested/deep-gone.txt`：工作区删除（未暂存）。
  - `src/old-name.txt -> src/renamed.txt`：`git mv` 暂存改名，Git 识别为重命名。
  - `src/added.txt`：未忽略的未跟踪新增。
  - `ignored.log`：由已提交的 `.gitignore` 忽略的未跟踪文件。
  - `.git/config`：仅追加注释，属于 Git 元数据变化。
- 冲突阶段 `git status --porcelain=v1`（6 项）：

```text
UU src/conflict.txt
 D src/deleted.txt
 M src/modified.txt
 D src/nested/deep-gone.txt
R  src/old-name.txt -> src/renamed.txt
?? src/added.txt
```

- 冲突前 API（`GET /api/workspaces/git-status`）返回 `branch: "main"`、`conflict: true`、
  `pendingCount: 6`，6 项类别依次为 `conflicted / deleted / modified / deleted / renamed / added`；
  响应中没有 `ignored.log`、`.git/config` 或目录项。

## 真实浏览器观察（桌面 1024 宽，Chromium）

浏览树（展开 `src`、`src/nested`、`.git` 后，读取 `getComputedStyle` 与 `data-change`）：

| 条目 | `data-change` | 计算颜色 | 说明 |
|---|---|---|---|
| `src` | `directory` | `rgb(121, 192, 255)` 蓝 | 包含新增、修改、删除、重命名等变更 |
| `src/nested` | `directory` | `rgb(121, 192, 255)` 蓝 | 只有已删除的 `deep-gone.txt`，仍提示变化 |
| `src/added.txt` | `added` | `rgb(126, 231, 135)` 绿 | 未忽略的未跟踪新增 |
| `src/modified.txt` | `modified` | `rgb(121, 192, 255)` 蓝 | 已跟踪修改 |
| `src/renamed.txt` | `renamed` | `rgb(121, 192, 255)` 蓝 | 暂存改名的新路径 |
| `src/conflict.txt` | `conflicted` | `rgb(255, 166, 87)` 橙 | 独立冲突警告，不是普通修改蓝 |
| `ignored.log` | 无 | `rgb(214, 219, 228)` 默认 | 可浏览，但无待提交着色 |
| `.git/HEAD` | 无 | `rgb(214, 219, 228)` 默认 | Git 元数据可浏览，不参与普通变更着色 |
| `docs` | 无 | `rgb(214, 219, 228)` 默认 | 无变更的目录不染色 |

- 删除文件 `src/deleted.txt`、`src/nested/deep-gone.txt` 不在浏览树中出现（`entry:...` 均不存在），
  但它们的现存祖先 `src`、`src/nested` 有蓝色提示。
- 选中 `.git/HEAD` 后页面只读显示 `ref: refs/heads/feature`；`.git` 与 `.git/HEAD` 都没有
  `data-change`，角标也不因 `.git/config` 变化而增加。
- 变更视图（内部子 tab「变更（6）」）逐项类别与颜色：

| 变更项 | 类别 | 计算颜色 | 页面文字 |
|---|---|---|---|
| `src/conflict.txt` | `conflicted` | 橙 `rgb(255, 166, 87)` | `src/conflict.txt` + 冲突标记 |
| `src/deleted.txt` | `deleted` | 红 `rgb(255, 123, 114)` | `src/deleted.txt` |
| `src/modified.txt` | `modified` | 蓝 `rgb(121, 192, 255)` | `src/modified.txt` |
| `src/nested/deep-gone.txt` | `deleted` | 红 `rgb(255, 123, 114)` | `src/nested/deep-gone.txt` |
| `src/renamed.txt` | `renamed` | 蓝 `rgb(121, 192, 255)` | `src/old-name.txt → src/renamed.txt`（一项） |
| `src/added.txt` | `added` | 绿 `rgb(126, 231, 135)` | `src/added.txt` |

- 冲突提示条文字为「存在冲突文件：冲突不是普通修改，请先在外部工具中解决。」；普通修改行没有这条提示。
- 页面不显示普通 `A/M/D` 状态字母；变更行文字只含路径、重命名箭头/文字与冲突标记。
- 变更视图没有 Commit / Sync 按钮（本票尚不开放写操作）。
- 浏览树与变更列表对同一变更使用同一类别颜色（上表两处一致）。

## 外部切分支与主动刷新

在页面打开期间，外部执行 `git merge --abort` 后 `git switch -c feature`（随后重新暂存同一个改名）：

```text
 D src/deleted.txt
 M src/modified.txt
 D src/nested/deep-gone.txt
R  src/old-name.txt -> src/renamed.txt
?? src/added.txt
```

再次进入 workspace / 点按「刷新」后：

- 分支显示从 `分支：main` 变为 `分支：feature`；API 同时返回 `branch: "feature"`、`conflict: false`、
  `pendingCount: 5`。
- 变更子 tab 从 `变更（6）` 变为 `变更（5）`；冲突行与冲突提示条消失。
- 返回 workspace 列表后角标显示 `5 项待提交`（固定期望：修改 1 + 删除 2 + 识别重命名 1 + 新增 1）。
- 窄屏 `390×844` 下变更列表与浏览树仍显示相同颜色；`src` 目录为蓝色 `rgb(121, 192, 255)`。

## 只读核对

在同一外部改名与暂存状态上，先记录索引语义，再连续调用两次状态/概览接口，然后重新读取：

```text
git ls-files --stage | sort | sha256sum
before: b8e4e35cce57d2663710af260df1190695a2175ee855c94c583ade1e507f8702
after:  b8e4e35cce57d2663710af260df1190695a2175ee855c94c583ade1e507f8702
git diff --cached --binary | sha256sum
before: 87fa9fb57a7adb8f6c894c9f5ddd52ae8f53418e0236b54dba9e784bd3777be6
after:  87fa9fb57a7adb8f6c894c9f5ddd52ae8f53418e0236b54dba9e784bd3777be6
```

状态查询没有为识别重命名而修改真实索引，也没有改变已暂存语义。

## 截图

- `desktop-browse-colors.png`：桌面浏览树（解决冲突后的 `feature` 状态重拍，便于看到彩色树）：
  绿色新增、蓝色修改/重命名/祖先目录、无色的忽略文件与已跟踪普通文件；删除项不在树中。
  冲突阶段的橙色文件行由 `getComputedStyle` 读取为 `rgb(255, 166, 87)`，并见
  `desktop-changes-colors.png`。
- `desktop-changes-colors.png`：桌面变更视图，含冲突提示条、删除红、识别重命名一项。
- `desktop-branch-refresh.png`：外部切到 `feature` 后的分支显示与 5 项变更。
- `desktop-workspace-badge.png`：workspace 列表上的 `5 项待提交` 角标。
- `narrow-changes-colors.png`、`narrow-browse-colors.png`：`390×844` 下两个视图的颜色一致性。

## 限制

- 证据来自真实 Git 与真实 Chromium，但 Chromium 为无头/仿真窄屏；手机实机触摸与软键盘体验仍属于
  human-led H1，不由本票宣称通过。
- 本票不提供 diff、Commit 或 Sync；页面只回答「哪些变更待提交」，未观察任何 Git 写操作。
