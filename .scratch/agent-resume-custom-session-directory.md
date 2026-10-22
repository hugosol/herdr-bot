# 自定义会话目录与 agent 内 /resume 的发现范围

调查日期：2026-10-22。只调查，不改变会话存放方式、不移动或删除原生文件。

## 结论

herdr-bot 创建 Bot 时传入 `--session-dir <数据目录>/bot-sessions/<creation-id>`。这里存放的是 agent 原生会话文件，不是默认目录之外额外备份的一份。使用同一自定义目录的 agent 可以通过 `/resume` 发现其范围内的会话；普通终端按默认配置启动的 agent 不会自动发现这些外置目录。cwd 相同也不会自动扩大搜索范围。

显式 CLI 文件路径可以绕过发现范围：Pi 使用 `pi --session <绝对路径>`，OMP 使用 `omp --resume=<绝对路径>`。文件存在不等于一定可恢复，仍需格式、权限及运行环境有效。

## 原有决策的原因

`docs/bot-template-agent-inputs.md:127–142` 已记录：创建 Bot 必须得到独立新对话；OMP 无显式会话标志或目录时可能由 `autoResume` 续接同 cwd 最近的会话，显式 `--session-dir` 则进入新建分支。Pi 默认新建，但采用相同的独立目录方式。新 Tab 本身不保证 agent 新建对话。

这不是 Herdr 创建 Tab 的要求：`backend/src/herdr.rs:178–218` 的 `tab create` 只指定 Workspace、cwd、label 和焦点选项；`:224–260` 的 `agent start` 才透传 herdr-bot 生成的原生 agent 参数。独立 session-dir 是 herdr-bot 的启动策略，不是 Tab 的存储目录。

## herdr-bot 来源

- `backend/src/bots.rs:174–186`：为每个创建分配独立原生会话目录，以避免意外自动续接。
- `backend/src/bots.rs:249–268`：实际把目录通过 `--session-dir` 传给 agent。
- `backend/src/lib.rs:1102–1132`：创建目录并传入后台创建任务；不是复制既有对话文件。

## Pi 源码

以下相对 `E:/Developer/myhub/pi/`：

- `packages/coding-agent/src/modes/interactive/interactive-mode.ts:3337–3340,5726–5742`：`/resume` 打开选择器；当前列表使用 SessionManager 的 cwd/sessionDir；非默认目录的 All loader 也显式使用同一 sessionDir。
- `packages/coding-agent/src/core/session-manager.ts:1900–1966`：自定义目录下当前列表按会话 cwd 过滤，All 去除 cwd 过滤但仍只扫描该自定义目录。默认 All 扫描默认 sessions root 的项目目录，不自动合并外置目录。
- 同文件 `:941–954`：目录内 `.jsonl` 发现为直属文件，不递归扫描 creation 子目录。
- `packages/coding-agent/src/main.ts:252–281`：CLI `--session` 的路径参数直接解析；ID/前缀受 list/listAll 的范围约束。

## OMP 源码

以下相对 `E:/Developer/myhub/oh-my-pi/`：

- `packages/coding-agent/src/modes/controllers/selector-controller.ts:1773–1805`：当前选择器使用当前 sessionDir；All loader 使用无自定义目录参数的 `listAllForPicker()`。
- `packages/coding-agent/src/session/session-manager.ts:4389–4426`：当前列表使用显式 sessionDir；All 走全局默认存储。
- `packages/coding-agent/src/session/session-listing.ts:613–624,660–695`：本地扫描目录直属 `.jsonl`；全局扫描默认 sessionsRoot 的 `*/*.jsonl`；选择器会过滤部分空会话。因此自定义目录中的记录切换到 All 后可能不再显示。
- `packages/coding-agent/src/slash-commands/builtin-lifecycle.ts:371–406` 及 `packages/coding-agent/src/session/session-listing.ts:916–972`：agent 内 `/resume <参数>` 按 ID/文件名前缀匹配，允许回退到默认全局范围，但没有任意绝对路径的专门解析分支。
- `packages/coding-agent/src/main.ts:1257–1275`：CLI `--resume` 有显式路径分支，区别于 slash 命令。

## 原生 ID 检查

本机只读取目录 `*/*.jsonl` 的 session header（OMP 可先有一行 title 元数据），未读取消息正文，结果如下：

|范围|原生文件数|唯一 ID 数|
|---|---:|---:|
|`C:/Users/hugos/.pi/agent/sessions`|55|55|
|`C:/Users/hugos/.omp/agent/sessions`|1183|1183|
|`E:/Developer/myhub/herdr-bot/.data/bot-sessions`|17|17|

三组均完整解析所扫描文件的 header。Pi 默认目录与 OMP 默认目录的 ID 交集为 0；herdr-bot 自定义目录与两者的交集也均为 0。此结果仅适用于扫描当时、上述目录及直属项目/创建子目录中的原生文件；不代表其他自定义目录、profile 或未来会话绝不重复。

当前 Pi `packages/coding-agent/src/core/session-manager.ts:264–266` 使用 UUIDv7；当前 OMP `packages/coding-agent/src/session/session-manager.ts:137–140` 使用 `Bun.randomUUIDv7()`。两者没有产品 namespace 前缀。Pi 允许显式指定 ID（`:1057–1061`），两者打开已有有效原生文件时均保留 header ID（Pi `:1085–1098`；OMP `:2044–2047,2370–2390`）。因此 ID 适用于本需求定义的逻辑对话去重，不是物理文件一对一的绝对保证；复制文件不会自动变成不同对话身份。

## 仅凭文件格式识别 agent 的边界

再次检查 `.data/bot-sessions/*/*.jsonl` 的头部：17 个主文件均为 session version 3、header 字段均为 `type/version/id/timestamp/cwd`。其中 11 个有 OMP 当前格式的前置 title slot（`type=title,v=1,title,updatedAt,pad`），6 个没有。未读取消息正文。

Pi `packages/coding-agent/src/core/session-manager.ts:41–49` 与 OMP `packages/coding-agent/src/session/session-entries.ts:16,38` 均使用 version 3，核心 header 结构重叠。OMP `packages/coding-agent/src/session/session-title-slot.ts:66–99` 明确定义上述 title slot；OMP 也支持无 slot 的旧文件。因此有 slot 可作为 OMP 格式的强证据，缺少 slot 不能直接推定为 Pi，更不能仅凭相同 header version 区分产品或证明原始创建者。

## 验证边界

实际运行 `omp --help`，本机输出 v18.12.6，确认 `--session-dir` 描述为 “Directory for session storage and lookup”，`--resume` 支持 ID 前缀、路径及选择器。选择器发现范围依据当前相邻仓库源码调查；未启动交互式 agent、未实际执行 `/resume`、未读取或恢复用户对话，也不把仓库源码等同于所有安装版本行为。
