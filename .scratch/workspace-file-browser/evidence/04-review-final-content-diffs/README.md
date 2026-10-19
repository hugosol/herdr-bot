# 04 — 审查最终内容与完整 diff：真实 Git、真实浏览器证据

日期：2026-10-19。环境：Windows 10.0.26200，Git 2.53.0.windows.2，omp-bot 后端以隔离配置在
`127.0.0.1:8799` 启动，真实 Chromium（omp 管理的无头 Chromium）驱动页面。未使用或修改用户的业务仓库，
未调用关机，未访问任何真实远端。

## 隔离场景

- 仓库：`C:\Users\hugos\AppData\Local\Temp\omp-bot-evidence-04\repo`（临时目录，可删除）。
- 登记文件：`C:\Users\hugos\AppData\Local\Temp\omp-bot-evidence-04\workspaces.json`。
- 配置：`preview_limit_bytes = 1048576`（默认 1 MiB），mDNS 关闭。
- 仓库初始提交后，用真实 Git 与真实文件构造下列事实：
  - `src/modified.txt`：部分暂存。HEAD 为 `alpha / beta / gamma`，索引为 `alpha / STAGED / gamma`，
    最终本地文件为 `alpha / STAGED / FINAL`。
  - `src/deleted.txt`：工作区删除，未暂存。
  - `src/old-name.txt -> src/new-name.txt`：`git mv` 暂存改名，Git 识别为重命名；新路径随后增加
    `unstaged tail`（状态 `RM`）。
  - `src/added.txt`：未忽略的未跟踪新增。
  - `src/limit-old.txt`：HEAD 为 4 950 000 字节，最终文件为 10 字节。
  - `src/limit-new.txt`：HEAD 为 10 字节，最终文件为 4 950 000 字节。
  - `src/binary.txt`：HEAD 为文本，最终文件含 NUL 字节。
  - `src/script.html`：最终文件含 `<script>window.__pwned = true</script>` 与带 `onerror` 的 `<img>`。
  - 截图阶段额外加入 15 个未跟踪文件 `src/scroll/extra-01.txt … extra-15.txt`，用于让宽屏列表
    真正溢出并验证滚动位置。
- 初始 `git status --porcelain=v1`（8 项）：

```text
 M src/binary.txt
 D src/deleted.txt
 M src/limit-new.txt
 M src/limit-old.txt
MM src/modified.txt
RM src/old-name.txt -> src/new-name.txt
 M src/script.html
?? src/added.txt
```

加入滚动样例后待提交项数为 23。

## 审查范围：HEAD → 最终本地内容

页面显示的 `src/modified.txt` 左右对照 diff：

```text
旧侧（HEAD）        新侧（最终本地内容）
1  alpha            1  alpha
2  beta       →     2  STAGED
3  gamma            3  FINAL
```

- 旧侧显示的是 HEAD 的 `beta`，不是索引里的 `STAGED`；因此不是 index→工作区。
- 新侧同时包含已暂存的 `STAGED` 与未暂存的 `FINAL`；因此不是只看其中一侧。
- 没有远端参与比较；分支显示为 `main`。
- `src/new-name.txt` 的审查使用 Git 给出的旧路径：标题为
  `src/old-name.txt → src/new-name.txt`，旧侧读 HEAD 的旧路径，新侧读最终新路径，并显示未暂存的
  `unstaged tail`。
- 新增 `src/added.txt` 的旧侧为空，删除 `src/deleted.txt` 的新侧为空；两类都能正常渲染。

## 真实浏览器观察（桌面 1024×800，Chromium）

- 变更视图为左列表、右内容：`changes-layout` 为两列，左侧滚动列表，右侧 diff。
- 默认 diff 为左右对照：`data-mode="side-by-side"`；上表旧新行由 `getComputedStyle` 与文本读取确认。
- 列表 23 项时 `scrollHeight=789 > clientHeight=518`；滚动到 `scrollTop=260` 后选中
  `src/modified.txt`，左列表显示 `src/scroll/extra-06` 附近，右侧仍显示该文件的 diff。
- 变更行颜色（`getComputedStyle(...).color`）：
  - 新增 `src/added.txt`：绿 `rgb(126, 231, 135)`。
  - 修改与已识别重命名：蓝 `rgb(121, 192, 255)`。
  - 删除 `src/deleted.txt`：红 `rgb(255, 123, 114)`。
  - 普通 A/M/D 状态字母不出现。
- 已识别重命名显示为一项 `src/old-name.txt → src/new-name.txt`。
- 反复切换「文件浏览 / 变更」内部子 tab 和「会话列表 / 文件列表」顶层 tab 后：所选
  `src/modified.txt` 仍为 `aria-current="true"`，右侧仍是同一个左右对照 diff，左列表
  `scrollTop` 仍为 `260`。

## 预览限制与二进制

- 旧侧超限：`src/limit-old.txt` 显示
  「不可展示完整 diff：旧侧大小 4950000 字节，超过预览上限 1048576 字节；未加载完整内容。」
  页面中没有 `diff-view`（不渲染完整 diff）。
- 新侧超限：`src/limit-new.txt` 显示同样结构的新侧说明；页面中没有 `diff-view`。
- 二进制：`src/binary.txt` 显示
  「不可展示完整 diff：新侧不可文本 diff：文件包含 NUL 字节，不是可文本预览的文本。」
  页面中没有 `diff-view`。
- 上述三项仍出现在变更列表中，`data-change` 与蓝色着色不变；变更角标保持 `变更（23）`，
  没有因为预览不可用而移除或缩小审查集合。

## HTML / script 按纯文本展示

选中 `src/script.html` 后：

- 页面上可见的 diff 文本是字面量
  `<script>window.__pwned = true</script>` 与
  `<img src="x" onerror="window.__pwned = true">`。
- diff 容器的 `innerHTML` 不包含 `<script`（内容被转义为文本）。
- `window.__pwned` 为 `undefined`；页面中 `img` 元素数量为 0。
- 页面唯一 `script` 元素是应用自身的打包脚本，不是 diff 内容。
- 可预览性由内容决定，不使用后缀白名单：无后缀的 `src/modified.txt`、`src/deleted.txt` 等同样可审查。

## 窄屏 390×844（Chromium 视口仿真）

- 变更视图默认停留在列表：`changes-mobile` 可见，没有 `diff-view`。
- 选择 `src/modified.txt` 后进入 diff，`data-mode="unified"`；统一行显示
  `2 - beta`、`3 - gamma`、`2 + STAGED`、`3 + FINAL`。
- 明确返回：`返回变更列表` 按钮可见；返回后列表重新可见，没有残留 diff。
- 滚动恢复：列表 `scrollTop=260`（`scrollHeight=789 > clientHeight=504`）后进入
  `src/scroll/extra-10.txt` 的统一 diff，再返回；列表 `scrollTop` 精确恢复为 `260`。

## 只读核对（独立于页面）

在连续请求目录、文件与 diff（修改、重命名、旧侧超限、二进制）前后，独立记录：

```text
.git/index SHA-256         2db1354b7f0341a19f3eb538dabcbeeff16365667a0a00f085321d9804bc6d86
git ls-files --stage       （整体快照哈希）
git diff --cached --binary （整体快照哈希）
git diff --binary          （整体快照哈希）
src/ 下所有文件字节的 SHA-256
```

请求前后全部相同（`unchanged: true`）。审查扫描没有暂存、刷新索引或改动文件；没有为了识别重命名
而操纵真实暂存区。

## 自动检查对应关系

- `backend/tests/workspace_diff.rs`（4 例）：真实部分暂存仓库（另有一个与 HEAD 内容不同的远端跟踪引用）
  的 HEAD→最终内容、无首次提交的空树比较、两侧各自限额（32 字节精确边界、旧超限/新很小、旧很小/新超限）、
  两侧二进制、根外路径/遍历/绝对路径/符号链接/目录联接/根替换、旧路径遍历、未登记 workspace、封闭查询面，
  以及索引与文件内容不变的独立核对。
- `frontend/tests/diffReview.test.tsx`（8 例）：宽屏左右对照、重命名旧路径、增删侧为空、HTML 纯文本、
  超限/二进制不渲染完整 diff、内部视图与顶层 tab 上下文、窄屏统一 diff、明确返回与精确滚动恢复。
- 运行结果：`cargo test --test workspace_diff` → `4 passed`；`cargo test --no-fail-fast` →
  `222 passed / 30 suites`；`npm test` → `16 files / 113 tests passed`；`npm run build` 成功。
- 突变验证：把旧侧临时改成索引对象，HEAD→最终内容用例因 `STAGED` 代替 `beta` 失败；把 `complete:false`
  也渲染成 diff 行，超限/二进制用例失败；两处还原后分别恢复 `4 passed` 与 `8 passed`。

## 截图

- `desktop-side-by-side-modified.png`：桌面左列表右内容、默认左右对照；旧侧 `beta`、新侧
  `STAGED`/`FINAL`。
- `desktop-side-by-side-renamed.png`：旧路径 → 新路径的左右对照。
- `desktop-list-scrolled-selection.png`：23 项列表滚动到 `scrollTop=260` 并选中修改项。
- `desktop-context-preserved.png`：内部子 tab 与顶层 tab 往返后选择、diff 与滚动位置保持。
- `desktop-limit-old-side.png`、`desktop-limit-new-side.png`：两侧各自超限的明确说明。
- `desktop-binary-unavailable.png`：二进制不可文本 diff 的原因。
- `desktop-html-script-as-text.png`：HTML/script 字面量作为纯文本显示。
- `narrow-unified-diff.png`：窄屏统一 diff 与「返回变更列表」。
- `narrow-list-scroll-restored.png`：返回后列表仍处于原滚动位置。

## 限制

- 宽屏与窄屏证据来自无头 Chromium；窄屏为视口仿真，触摸与软键盘手感仍属于 human-led H1，
  由 08 汇总后交用户确认。
- 未实现 Commit / Sync / 文件事件自动刷新（属于后续票）；本票只审查 HEAD→最终本地内容。
