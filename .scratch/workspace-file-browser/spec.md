# workspace 文件浏览器与整仓 Git 提交同步

Status: ready-for-agent

> **2026-10-20 行为修订**：Git workspace 的文件浏览树改用 Git 原生忽略规则，省略被忽略的未跟踪文件和目录，并省略 `.git` 元数据；普通目录仍完整反映磁盘。`.gitignore`、未被忽略的隐藏文件和已跟踪文件继续可见。整棵目录树只在进入 workspace 与手动刷新时读取；文件事件与返回页面只更新 Git 状态。此修订取代下文与该行为冲突的旧描述。

## Problem Statement

用户需要在电脑浏览器与同一可信局域网内的手机上，查看 omp-bot 服务所在电脑的项目文件，定位尚未提交的变更，并完成整个仓库的提交和当前分支同步，而不必切换到桌面 Git GUI 或终端。

目前主界面只有控制面板和会话列表，默认进入控制面板，没有 workspace 登记、文件浏览、变更审查或 Git 提交同步入口。纯浏览普通目录与管理 Git 工作区需要共存；手机屏幕又不适合直接压缩桌面端的左右双栏布局。

用户尤其需要避免误操作和误解：被忽略的未跟踪文件不应误入提交，分支或内容在审查后发生变化时不应静默提交，Git 操作失败后不应继续执行后续步骤，冲突不应被普通提交按钮掩盖，缓存的远端状态不应被当作实时状态。

## Solution

主界面增加文件列表 tab，顺序调整为「会话列表 → 文件列表 → 控制面板」；每次打开或刷新页面仍先展示会话列表，控制面板名称与已有功能不变。

文件列表先展示由后端文件持久保存、电脑和手机共享的 workspace 登记列表。workspace 是服务器上的已登记目录，可以是普通目录，也可以对应一个 Git 工作树。取消登记不删除文件。一个 Git workspace 只管理自身仓库，不递归管理嵌套仓库或子模块中的变更。

进入 workspace 后提供文件浏览视图；Git workspace 另提供变更视图。浏览树按 Git 原生忽略规则省略被忽略的未跟踪路径与 `.git` 元数据，普通目录完整反映磁盘；保留 `.gitignore`、未被忽略的隐藏文件和已跟踪文件。浏览视图展示只读内容，并以 Git 状态颜色及祖先目录提示定位变更；变更视图只展示待提交变更，提供文件 diff、常驻提交信息输入框和状态驱动的 Commit / Sync 主按钮。电脑使用左列表右内容布局，手机使用逐层进入与返回的布局。

Commit 在一句确认消息之后检查审查版本是否仍有效，再暂存全部符合 Git 规则的变更并提交，不自动推送。Sync 仅在工作区干净时按当前分支已有 upstream 与 Git 配置拉取、推送。任一步 Git 操作失败即中断后续步骤，弹窗显示 Git 返回的提示。冲突或未完成 Git 流程阻止普通提交同步，由用户在外部工具处理后返回。

文件系统变化触发有节流的 Git 状态刷新，默认最小自动刷新间隔 10 秒且可配置；目录树只在进入 workspace 和手动刷新时读取。远端状态按需检查。文本自动识别并只读展示，默认单文件预览上限 1 MiB 且可配置；不能预览的文件仍正常参与变更展示与提交。

## User Stories

1. As an omp-bot user, I want the top-level tabs ordered as 会话列表, 文件列表 and 控制面板, so that my frequent tasks are placed before fixed operations.
2. As an omp-bot user, I want every page open or reload to start at 会话列表, so that refreshing never unexpectedly takes me into file management.
3. As an omp-bot user, I want 控制面板 to retain its name and existing functionality, so that navigation changes do not change startup, stop or shutdown behavior.
4. As a mobile user, I want three clearly selected, equally sized top-level tabs without horizontal swipe navigation, so that navigation does not conflict with diff scrolling.
5. As a keyboard user, I want clear focus, selected states and keyboard-operable tabs, so that I can navigate without a pointer.
6. As an omp-bot user, I want changing tabs to retain my workspace, selected file, scroll position and commit message within the current page, so that I can check sessions without losing review context.
7. As an omp-bot user, I want changing tabs not to cancel an accepted Git operation, so that navigation is not mistaken for cancellation.
8. As an omp-bot user, I want a page reload not to retry Commit or Sync, so that observing results never repeats a write.
9. As an omp-bot user, I want to register an existing directory on the server as a workspace, so that I can return to the projects I manage.
10. As a mobile user, I want directory paths to refer to the server rather than my phone, so that I browse the intended filesystem.
11. As an omp-bot user, I want the workspace 登记列表 to survive backend restart, so that registrations are not browser-only bookmarks.
12. As a 局域网设备 user, I want computers and phones to share the same registered workspaces, so that each device operates on the same instance.
13. As an omp-bot user, I want removing a workspace registration not to delete its directory or files, so that list maintenance is non-destructive.
14. As an omp-bot user, I want to register a non-Git directory for browsing, so that the file browser is useful without a repository.
15. As an omp-bot user, I want non-Git workspaces to offer browsing without usable Git controls, so that unsupported operations are not misleadingly offered.
16. As an omp-bot user, I want each Git workspace to manage only its own repository, so that whole-repository actions do not recursively operate on nested repositories.
17. As an omp-bot user, I want workspace badges to show the number of pending file changes rather than directories or changed lines, so that I can find repositories needing attention.
18. As an omp-bot user, I want 文件浏览视图 and 变更视图 to be separate internal tabs, so that reading all files and reviewing a submission have distinct purposes.
19. As an omp-bot user, I want the browsing tree to keep hidden files and `.gitignore` while omitting Git-ignored untracked paths and `.git` metadata in Git workspaces, so that the tree matches Git's own semantics without exposing repository internals.
20. As an omp-bot user, I want selecting a file in the browsing tree to show its read-only contents, so that browsing cannot accidentally edit the repository.
21. As an omp-bot user, I want links pointing outside the workspace boundary not to be followed, so that a registered root does not silently expose another directory.
22. As a Git workspace user, I want new non-ignored files displayed in green, so that pending additions are easy to find before staging.
23. As a Git workspace user, I want modified files displayed in blue, so that I can distinguish changes from additions.
24. As a Git workspace user, I want ancestor directories containing pending changes displayed in blue up to the level immediately below the workspace root, so that I can follow changes through a collapsed tree.
25. As a Git workspace user, I want ignored untracked paths omitted from the browsing tree, so that the tree follows Git's ignore rules and does not imply they will be committed.
26. As a Git workspace user, I want the ignore-rules file itself treated like an ordinary changed file, so that edits to repository rules are reviewable and committable.
27. As a Git workspace user, I want deleted files absent from the browsing tree but present in the changes list, so that the tree reflects disk contents without hiding deletions from review.
28. As a Git workspace user, I want deletion-only changes to remain discoverable through existing ancestor directory hints, so that deletions do not disappear from tree-level awareness.
29. As a Git workspace user, I want `.git` metadata omitted from the browsing tree and excluded from ordinary worktree change coloring and counts, so that internal repository activity is neither exposed nor presented as a pending commit.
30. As a Git workspace user, I want the changes list to contain all tracked changes and non-ignored untracked files that the whole-repository commit will include, so that the review matches the operation's scope.
31. As a Git workspace user, I want Git's native ignore rules respected without force-adding ignored files, so that the browser behaves consistently with my repository.
32. As a Git workspace user, I want already tracked files to remain included even if they later match ignore rules, so that the browser does not invent a second ignore policy.
33. As a Git workspace user, I want diffs based on the currently checked-out local branch's latest commit and final local contents, so that staged and unstaged changes are reviewed together.
34. As a Git workspace user, I want the current branch name visible and external branch changes detected, so that I know which branch Commit and Sync will affect.
35. As a Git workspace user, I want deleted items red and new or modified items colored consistently between both views, so that the changes list uses familiar visual meaning.
36. As a Git workspace user, I want Git-detected renames displayed in blue with their old and new paths, so that a recognized rename can be reviewed as one change.
37. As a Git workspace user, I want unrecognized renames to remain a deletion plus an addition, so that the browser does not alter staging or invent file relationships for presentation.
38. As a Git workspace user, I want badges to count a recognized rename once and a separate deletion/addition twice, so that counts match the current Git interpretation.
39. As an omp-bot user, I want ordinary change colors without A/M/D status-letter badges, so that the file lists remain visually simple.
40. As a desktop user, I want a tree or changes list on the left and file contents or diff on the right, so that I can review without leaving the list.
41. As a desktop user, I want side-by-side diffs by default, so that I can compare old and new contents across a wide screen.
42. As a mobile user, I want to enter workspace, file list and file content progressively, so that each step remains usable on a narrow screen.
43. As a mobile user, I want unified diffs by default and a clear return action preserving list state, so that I can review changes without a cramped dual-column layout.
44. As a Git workspace user, I want a persistent commit-message input and main action above the changes list, so that I can review and submit from the same view.
45. As a Git workspace user, I want Commit to require a non-empty message and a short confirmation dialog with confirm/cancel only, so that accidental clicks are gated without duplicating the review screen.
46. As a Git workspace user, I want cancellation of the confirmation dialog to perform no Git write, so that dismissing the gate is safe.
47. As a Git workspace user, I want Commit to reject a branch, HEAD, index or content change since the displayed review version, so that unreviewed changes are not silently included.
48. As a Git workspace user, I want a rejected review-version check to refresh the changes and retain my message, so that I can re-review and confirm again.
49. As a Git workspace user, I want whole-repository Commit to stage all eligible final changes and then commit them, so that I do not need a separate manual staging workflow.
50. As a Git workspace user, I want existing partial staging not to narrow whole-repository Commit, so that the operation consistently includes all eligible final changes.
51. As a Git workspace user, I want Commit not to push automatically, so that local submission and remote synchronization remain distinct intentions.
52. As a Git workspace user, I want Git writes initiated by this service serialized per repository, so that multiple pages do not run overlapping writes on the same repository.
53. As a Git workspace user, I want the main action to show Commit while pending changes exist and Sync only when the workspace is clean and known remote differences exist, so that the next action reflects repository state.
54. As a Git workspace user, I want synchronized, unchecked-remote and missing-upstream states distinguished, so that a disabled action has a meaningful explanation.
55. As a Git workspace user, I want Sync to use the current branch's existing upstream rather than assuming a remote or branch name, so that it respects my Git configuration.
56. As a Git workspace user, I want Sync to check remote state, pull and then push according to existing Git configuration, so that synchronization follows the repository's configured strategy.
57. As a Git workspace user, I want Sync to require a clean worktree and avoid automatic stash or commit, so that synchronization does not hide local edits.
58. As a Git workspace user, I want any failed Git step to stop subsequent steps and show Git's returned message, so that failures remain understandable and are not compounded.
59. As a Git workspace user, I want completed steps retained when a later step fails, so that the UI does not falsely promise transaction rollback.
60. As a Git workspace user, I want Commit and Sync disabled with an explanation during conflicts or unfinished Git workflows, so that ordinary actions do not bypass required recovery.
61. As a Git workspace user, I want external conflict resolution and workflow completion reflected after refresh, so that I can resume normal actions without an in-browser conflict editor.
62. As a Git workspace user, I want detached HEAD and missing-upstream limitations explained rather than implicitly repaired, so that the browser does not change repository configuration for me.
63. As an omp-bot user, I want local file and Git metadata events to trigger Git status updates, so that external tools' changes become visible without constantly rereading the directory tree.
64. As an omp-bot operator, I want a configurable per-workspace automatic status refresh interval defaulting to 10 seconds, so that bursts of filesystem activity have bounded refresh cost.
65. As an omp-bot user, I want events within the refresh window merged with a trailing status refresh, so that the final change is not dropped and continuous writes cannot postpone updates indefinitely.
66. As an omp-bot user, I want workspace entry and manual refresh to read the full tree once, with later deliberate refresh available on demand, so that directory browsing is stable and predictable.
67. As a Git workspace user, I want remote checking explicit, with the last check time visible and a fresh check before Sync, so that cached ahead/behind counts are not mistaken for live remote knowledge.
68. As an omp-bot user, I want text detected from content without a mandatory extension whitelist and rendered as plain text, so that extensionless files are readable without executing their contents.
69. As an omp-bot operator, I want the single-file preview limit configurable with a default of 1 MiB, so that content rendering has an explicit size boundary.
70. As an omp-bot user, I want binary or oversized files to show a clear preview limitation while remaining visible, counted and eligible for Commit, so that rendering constraints do not change repository operations.
71. As a trusted 局域网设备 user, I want to browse registered workspaces and use the agreed Git actions without a new login system, so that the feature fits the existing trusted-LAN deployment.
72. As an omp-bot user, I want the new file feature not to relax session-write restrictions or alter session and shutdown lifecycles, so that existing domain guarantees remain intact.

## Implementation Decisions

### 约束性质与未选择的接缝

- 本节记录对话已经确定的行为与技术约束，不指定新增模块、接口、协议、数据结构或测试接缝。
- workspace 登记列表由后端以文件持久保存，是已确认事实；保存格式、字段、配置键名、存储位置及内部边界尚未决定。
- 文件系统变化监听、Git 状态查询和页面刷新是已确认机制方向；监听库、事件传输方式、是否复用既有观察连接及模块归属留待契约阶段。
- 配置项明确包含自动刷新最小间隔和单文件预览大小上限；不在此决定具体配置结构。

### 导航与状态保留

- 顶层顺序为会话列表、文件列表、控制面板；每次打开或整页刷新默认会话列表，不通过恢复上次 tab 改写此默认。
- 三个顶层入口等宽，有明确选中态与焦点；支持键盘操作，包括方向键切换。不以左右滑动手势切换顶层 tab。
- 文件浏览视图和变更视图是 workspace 内部的子 tab，不把文件做成顶层 tab。
- 当前页面内切换视图或顶层 tab，保留对应浏览位置、展开状态、文件选择、滚动和输入中的提交信息；整页刷新不承诺恢复这些临时状态。
- 页面切换不取消后端已接受的 Git 操作；返回后可观察实际状态与结果。刷新不重试 Git 写操作。
- 控制面板的固定操作、会话列表的业务行为以及全局关机提醒保持不变。

### workspace 与浏览范围

- 文件系统始终指服务所在电脑的文件系统，不调用浏览器选择手机或客户端目录来代替服务器目录。
- 登记入口采用服务器路径输入与有效性校验；登记列表跨设备共享、跨后端重启保留；移除仅取消登记。
- 普通目录与 Git 工作区均可登记。Git workspace 的仓库操作根为该工作树根；一个 workspace 不聚合多个仓库。
- 普通目录只有文件浏览视图；不出现可执行的 Commit / Sync 或 Git 变更视图。
- Git workspace 的浏览树使用 Git 原生忽略规则，省略被忽略的未跟踪文件和目录，并省略 `.git` 元数据；普通目录不过滤。两种 workspace 都保留隐藏文件与忽略规则文件；`.git` 元数据不参与普通待提交变更、着色与角标。
- 已跟踪文件即使匹配忽略规则也仍显示；忽略查询使用仓库及其嵌套 `.gitignore`、`.git/info/exclude` 和全局 excludes，不自行解析规则。嵌套仓库与子模块内部不递归应用父仓库的忽略集合。
- 读取范围限制在所选已登记 workspace 内；不跟随指向根外的符号链接或 Windows 目录联接，显示目标与不可进入原因。
- 不递归提交、同步或管理嵌套仓库、子模块的内部变更；不把它们的内部文件作为所属仓库的普通新增内容提交。

### 待提交变更、着色与计数

- 待提交变更遵循 Git 原生规则：已跟踪文件变更，以及未被 Git 忽略的未跟踪文件。已经跟踪的文件不会因后来匹配忽略规则而被排除；不强制添加被忽略的未跟踪文件。
- 浏览视图只展示磁盘实际存在的文件；变更视图包含删除项。仍存在的祖先目录应反映其范围内的删除变更。
- 新增绿色、修改蓝色、删除红色、Git 已识别的重命名蓝色。两个视图对同一变更使用一致颜色，不增加普通状态字母标记。
- 祖先目录只要包含待提交变更就显示蓝色，向上传播到 workspace 根目录下一级；不按子文件的绿色或红色染整个目录。
- Git 已识别重命名在变更列表以旧路径到新路径的一项展示；浏览树只展示实际存在的新路径。未识别则保持删除与新增两项，不自创相似度配对，也不为展示而修改真实暂存区。
- 角标统计变更项，不统计目录或 diff 行数。同一文件同时有已暂存与未暂存修改只计一次；识别为重命名计一项，未识别的删除加新增计两项。Git 识别结果变化可导致显示项数变化。
- 冲突使用明确警告区别于普通修改；不把普通 Git 失败或单纯分支分叉标成文件冲突。

### 内容与 diff

- 默认展示当前分支最新本地提交到本地最终内容的整体差异，合并观察已暂存和未暂存内容；未跟踪且未被忽略的文件按新增处理。
- 当前分支名可见；用户在外部工具切换分支后，刷新应反映新分支，后续动作针对重新确认后的当前分支。
- 电脑默认左右对照 diff，手机默认统一 diff；手机通过列表进入内容并明确返回，保留列表上下文。
- 自动识别文本，不以强制后缀白名单限制预览；内容按纯文本展示，不执行 HTML 或脚本。
- 单文件文本预览上限默认 1 MiB，后端可配置。超限时不加载完整内容，明确显示限制；不将受限展示伪装为完整内容或完整 diff。
- 二进制文件显示基本信息及不支持文本预览或 diff 的说明。预览限制不影响可见性、着色、变更数量与提交范围。

### Commit 与审查版本复核

- 变更视图上方常驻提交信息输入框与状态驱动的主按钮；提交信息须非空。
- 点击 Commit 仅弹一句确认消息与确认、取消，不重复展示 workspace、分支、文件数、提交信息或文件清单；这些信息由变更界面承载。
- 取消确认不产生 Git 写操作。确认后以已展示的审查版本为基准，重新核对分支、HEAD、暂存状态、文件集合与内容，不能只比较文件名或数量。
- 与审查版本不一致时阻止当次提交，提示并刷新视图，保留提交信息，要求重新审查确认；不得默默纳入新变化。
- 检查通过后暂存整个仓库全部符合 Git 规则的最终变更，再 commit。已有部分暂存不缩小本次范围；Commit 不自动 push。
- 本服务发起的同仓库 Git 写操作串行执行，不把仓库级资格混同为固定操作的既有执行资格。
- 外部编辑器和其他 Git 进程不受本服务完全控制。提交前检查不等于能够原子锁住所有外部修改；契约应保留这一事实，不承诺不存在竞争窗口。

### 主按钮与同步

| 状态 | 主操作与反馈 |
| --- | --- |
| 存在普通待提交变更且没有阻塞 | Commit；提交信息有效后可执行 |
| 工作区干净、有 upstream、已知 ahead 或 behind 非零 | Sync，并展示待拉取、待推送数量 |
| 工作区干净、已知已同步 | 禁用的已同步状态；仍可主动检查远端 |
| 工作区干净、远端尚未检查 | 未检查提示与检查入口，不宣称已同步 |
| 工作区干净、没有 upstream | 未配置跟踪分支提示，不提供 Publish Branch |
| 冲突、merge/rebase 等流程未完成 | 禁用 Commit / Sync，显示需要外部处理的原因 |
| detached HEAD | 禁用相应普通 Commit / Sync 操作并解释原因 |

- Sync 同样经过一句确认消息的确认、取消门禁。
- Sync 要求工作区没有未提交变更，使用当前分支既有 upstream；不假设远端或分支名称固定。
- 同步逻辑为检查远端、pull、push；是否需要实际推送由同步后的提交状态决定。只有待拉取提交时，不要求产生无意义的推送副作用。
- pull 策略遵循既有 Git 配置，包括配置明确要求的 merge、rebase 或 fast-forward 限制。不得擅自覆盖配置来确保成功；配置要求 rebase 不等于本功能自行添加自动 rebase 策略。
- 使用服务器现有 Git 身份与认证配置，不提供网页凭据输入、upstream 配置或首次分支发布。
- 不自动 stash、提交脏工作区、强推、修改配置或恢复失败流程。
- 任意 Git 步骤失败或被拒绝都立即中断后续步骤，弹窗显示 Git 返回的诊断；不把失败仅简化为通用成功或无原因提示。
- 已完成步骤不会因后续失败被自动撤销；暂存成功后提交失败、拉取成功后推送失败，都按实际仓库状态展示，不宣称整个流程有事务回滚。

### 冲突与未完成流程

- 最简处理为禁用普通 Commit / Sync，提供明确仓库级原因提示；未解决冲突文件可显示醒目警告。
- 冲突指 Git 未解决文件状态，可来自 merge、rebase、cherry-pick 或 revert；普通非零退出、认证失败、网络失败、策略拒绝不自动等同于冲突。
- 即使文件冲突已经解决，流程尚未结束仍阻塞普通主操作。用户在外部工具完成或撤销流程后，刷新重新判定可执行操作。
- 不自动选择冲突内容、不自动 abort、不提供网页 continue/skip/abort 或冲突编辑工作流。

### 本地刷新与远端检查

- Git 不是通用文件变化推送源；监听工作区和相关 Git 元数据变化后，再查询本地 Git 状态并更新展示；此过程不重读目录树。
- 自动刷新按 workspace 合并事件，最小刷新间隔默认 10 秒、后端可配置。没有等待窗口时尽快刷新；窗口内事件记为待刷新，窗口结束补一次，连续写入不能使刷新无限延期。自动刷新只更新 Git 状态。
- 进入 workspace 读取一次完整目录树；之后只有手动刷新会重新读取目录树。回到页面、Git 操作完成后的刷新以及提交前复核都不重读目录树；提交前仍不使用尚未过期的缓存代替查询。
- 普通目录也只在进入 workspace 和手动刷新时更新目录树，不依赖文件事件自动重读。
- 本地文件事件不触发持续自动 fetch。远端提供明确检查入口，Sync 前必定检查，展示最近检查时间；ahead/behind 基于最近已知远端状态，不承诺实时远端通知。

### 信任边界与既有决策

- 延续仅可信局域网、无新增认证的部署前提；本功能允许可信局域网浏览器读取已登记范围并发起确定的 Git 操作，不开放任意终端命令执行。
- 既有会话事件仅接受同机来源的限制保持不变；它不应被扩大应用为禁止手机发起本功能 Git 操作，也不应因本功能而被放宽。
- 既有会话与关机状态的权威来源、生命周期及全局提醒不变，文件浏览的临时界面状态不成为会话或关机业务事实。

## Testing Decisions

- 当前前端已有 Vitest、React Testing Library、user-event 与 jsdom 的交互验证先例；导航验证覆盖 tab 切换、可见性和键盘交互。既有“两 tab、默认控制面板、方向键不切换”的约束与本规格不兼容，应随行为迁移，不保留旧约束作为兼容要求。
- 当前后端已有 Rust 异步验证、HTTP 交互与临时文件夹/进程夹具的先例，可作为后续契约阶段评估验证方式的既有实践；本规格不指定必须新增哪个接口或采用哪个验证接缝。
- 对话已经用临时真实 Git 仓库验证：普通文件系统改名、目标仍未跟踪时可显示为删除加新增；暂存后、尚未提交时可被识别为重命名。不应编写“所有未提交改名必定识别为重命名”的断言，也不应为展示检测而改变真实暂存区。
- 提交、忽略、当前分支、重命名和同步行为必须符合真实 Git 语义，不能以固定模拟输出替代其正确性证据；精确接缝与覆盖分配留待下一阶段。
- 桌面与手机的实际浏览器表面均需检查布局、导航、确认门禁和 diff 可用性；仅组件测试通过不能作为响应式界面可用的证明。视口与执行工具尚未选定。
- 验证不得使用或修改用户正在工作的真实仓库，不向真实业务远端推送；需要写操作的验证应隔离文件、仓库及远端状态。
- 正式验证应针对用户可见行为、边界、状态转换及失败后实际仓库状态，不验证源码字面量、纯转发或实现细节。既有与本次新行为冲突的检查不得以兼容 shim 保留。
- 本次仅生成规格，不运行应用测试、不宣称文件浏览或 Git 功能已实现。接缝、每个接缝上的承诺、覆盖表及验收模式由 `/to-contract` 选择和批准。

## Out of Scope

- 在线编辑、新建、删除、重命名、上传、下载文件，以及丢弃、回退变更。
- 手动选择提交文件、逐块暂存、独立暂存区管理、amend、提交历史编辑。
- clone、初始化仓库、登记时配置远端、首次 Publish Branch、配置 upstream。
- 创建、切换、删除分支；跨多个仓库或分支批量提交同步。
- 递归管理嵌套仓库、聚合仓库或子模块内部工作区。
- 网页冲突编辑器、冲突自动选择、网页 continue/skip/abort、自动恢复失败流程。
- 自动 stash、强制推送、隐式提交并同步、为同步覆盖 Git 配置。
- 浏览器通用终端、任意 shell 命令执行和网页凭据管理。
- 持续自动 fetch、实时远端变化订阅、把缓存远端信息宣称为实时状态。
- 强制文件后缀白名单、完整二进制预览、超限文件的完整文本或 diff 渲染。
- 新增登录、token、多用户权限系统或公网部署能力。
- 更改固定操作、会话上报来源限制、会话生命周期、关机倒计时及监听语义。
- 本阶段选择模块边界、API、事件协议、持久化 schema、监听库、测试接缝或实现票拆分。

## Further Notes

- 本文综合已完成的逐轮设计质询，用户要求直接生成并发布规格；不再开启需求采访。`ready-for-agent` 表示规格可进入后续流程，不表示契约已经批准或功能已经实现。
- 领域用语遵循根领域词汇表：workspace、workspace 登记列表、文件浏览视图、变更视图、待提交变更、会话列表与控制面板。
- 相关既有决策：ADR-0002 的可信局域网与无认证前提；ADR-0004 的同机会话事件写入边界；ADR-0005 的后端权威关机安排；ADR-0006 的会话与监听生命周期。本规格不以文件操作权限重写这些边界。
- 当前已核对的实现仍为两个顶层 tab，默认控制面板，原生按钮通过 Tab/Enter/Space 操作；方向键切换、第三个 tab 和默认会话列表均属于本次目标变更，而非现有能力。
- Git 的重命名是比较时的识别结果，不是文件系统永久身份；暂存前后展示项数可能变化，用户已接受遵循 Git 结果。
- 非 Git 普通目录支持浏览不等于支持初始化 Git；预览不可用不等于文件不参与提交；页面关闭不等于后端 Git 操作取消。
- 可信局域网是假设，不是认证：能访问服务的设备可能读取已登记范围的敏感文件，也可触发已开放 Git 写操作。用户明确接受暂不增加认证，部署说明必须保留此提醒。
- 服务器既有 Git 配置、身份、凭据与 hooks 会影响操作结果；本功能不承诺任意仓库配置均能无人值守成功。任何拒绝或失败按约定展示并停止后续步骤。
- 预览上限与自动刷新间隔已经明确可配置；配置名称、存储格式、具体接口和接缝仍未决定，应在 `/to-contract` 中沿用现有惯例并审批。
- 本规格发布后，`/to-contract` 已完成三个明确审批门；批准的承诺、接缝与验收分配见 [contract.md](contract.md)。上文有关“留待契约阶段”的文字记录规格生成时尚未选择的事项，不代表已批准契约仍在等待审批。
- 下一步：运行 `/to-tickets .scratch/workspace-file-browser/contract.md`。契约批准不表示功能已经实现或验收通过。

## Comments

### 契约审批完成

- 用户依次批准两个接缝、约束审视结果，以及 **Rapid iteration** 验收模式。
- 约束审视确认支持已初始化但无首次提交仓库的界面 Commit、有限且可配置的 Git 命令执行时长，以及 diff 两侧分别应用预览限额；精确承诺统一记录于 [contract.md](contract.md)，不在规格中维护第二份承诺表。
- 唯一 human-led 例外是手机实机交互体验；覆盖范围、agent 应准备的证据和先完成自动工作再交接的要求统一记录于契约 Coverage 单元格。未获用户实机确认前不得视为该项通过。
- 本次仅发布规格与契约决策文档，未实现文件浏览器，也未运行功能验收。
