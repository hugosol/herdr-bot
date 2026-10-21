# herdr-bot 改名剩余步骤

- 日期：2026-10-21
- 当前状态：当前仓库内的代码、测试、当前文档、运行时标识与防火墙已完成改名；手机局域网访问已验收。剩余 `my-agent-toolkit` 独立后续。
- 已完成提交：`6420523 rename project to herdr-bot`、`835e093 docs: add herdr-bot rename remaining steps`
- 当前远程：`git@github.com:hugosol/herdr-bot.git`
- 当前本地目录：`E:\Developer\myhub\herdr-bot`

本文记录改名迁移的执行状态与剩余步骤。已完成事项见 [ADR 0010](./adr/0010-rename-project-to-herdr-bot.md) 和根目录 `README.md` 的「变更记录：项目更名为 herdr-bot」。

术语边界：`docs/omp-*.md`、`.omp/`、`omp` 命令等 OMP 平台词汇按 [ADR 0010](./adr/0010-rename-project-to-herdr-bot.md) 保留；历史 ADR 与 README 历史记录不改写。

## 不可跳过的边界

- 本次是硬切换：旧 `omp-bot.toml`、旧 `OMP_BOT_*` 环境变量、旧 `omp-bot.local` 都不再兼容。
- 不改 `connect-bot` 扩展 ID、`/api/sessions/events` 协议、`zones.json` 或任何 API 路由。
- 不改 `docs/omp-*.md`、`.omp/`、`omp` 命令等 OMP 平台词汇。
- 不重写历史 ADR、README 历史记录或 `my-agent-toolkit/.scratch/` 下的历史证据。
- 外部仓库 `my-agent-toolkit` 的更新是独立后续，不与当前仓库的改名提交混在一起。

---

## 1. GitHub 仓库原地改名（已完成）

### 1.1 操作

1. 打开 GitHub 仓库 `hugosol/omp-bot`。
2. 进入 **Settings → General → Repository name**。
3. 将仓库名改为：

```text
herdr-bot
```

4. 选择原地 rename，保留 issues、PR、提交历史与 fork 关系。
5. 如果 GitHub 提示 `herdr-bot` 已被占用，**停止操作并确认新名字**，不要自动换成其他名字。

### 1.2 更新本地 remote

在仓库根目录执行：

```bat
git remote set-url origin git@github.com:hugosol/herdr-bot.git
git fetch --prune
git remote -v
git status
```

预期：

```text
origin  git@github.com:hugosol/herdr-bot.git (fetch)
origin  git@github.com:hugosol/herdr-bot.git (push)
```

旧 URL 会由 GitHub 自动重定向，但仍应把本地 remote 改成新 URL，避免以后依赖重定向。

### 1.3 验收

- GitHub 页面标题与仓库地址均为 `hugosol/herdr-bot`。
- `git fetch` 成功。
- `git log -1` 仍能看到 `rename project to herdr-bot` 提交。
- issues/PR 仍在原仓库中，没有丢到新仓库。

---

## 2. 本地目录改名（已完成）

> 这一步会改变当前工作目录。请先关闭本 AI 会话、所有终端、编辑器、`herdr-bot.exe`、cargo、node 等占用该目录的进程。

### 2.1 停止占用进程

```bat
tasklist /FI "IMAGENAME eq herdr-bot.exe"
netstat -ano | findstr ":8787"
```

如有监听进程，先正常结束它。不要带着监听中的 `herdr-bot.exe` 改目录。

### 2.2 从父目录改名

在 `E:\Developer\myhub` 下执行：

```powershell
Rename-Item -Path 'E:\Developer\myhub\omp-bot' -NewName 'herdr-bot'
```

改名后重新打开终端：

```bat
cd E:\Developer\myhub\herdr-bot
git status
git remote -v
```

预期：

- `git status` 干净。
- `git remote -v` 指向 `git@github.com:hugosol/herdr-bot.git`。
- 目录内存在 `herdr-bot.toml`，不再有 `omp-bot.toml`。

### 2.3 从新路径重建并冒烟

```bat
cargo build --release --manifest-path backend\Cargo.toml
backend\target\release\herdr-bot.exe --config herdr-bot.toml
```

预期启动日志：

```text
[herdr-bot] 主界面已启动
[herdr-bot] 配置文件：herdr-bot.toml
[herdr-bot] 监听：http://0.0.0.0:8787/
[herdr-bot] mDNS 状态：published host=herdr-bot.local. ...
```

浏览器检查：

```text
http://127.0.0.1:8787/
```

预期页面标题与 H1 均为 `herdr-bot`。

---

## 3. Windows 防火墙与局域网入口（防火墙与手机验收已完成）

旧规则仍指向旧 exe 路径，改名后需要重建。

### 3.1 查看并删除旧规则

以管理员身份打开命令提示符：

```bat
netsh advfirewall firewall show rule name="omp-bot"
netsh advfirewall firewall delete rule name="omp-bot"
```

### 3.2 添加新规则

```bat
netsh advfirewall firewall add rule name="herdr-bot-tcp" dir=in action=allow program="E:\Developer\myhub\herdr-bot\backend\target\release\herdr-bot.exe" protocol=TCP enable=yes profile=any
netsh advfirewall firewall add rule name="herdr-bot-udp" dir=in action=allow program="E:\Developer\myhub\herdr-bot\backend\target\release\herdr-bot.exe" protocol=UDP enable=yes profile=any
```

### 3.3 更新客户端入口

- 手机书签/主屏幕快捷方式改为：

```text
http://herdr-bot.local:8787/
```

- 旧 `http://omp-bot.local:8787/` 不再发布；旧书签失效是硬切换的预期结果。
- 如果局域网解析失败，继续使用 `http://<本机局域网 IPv4>:8787/` 兜底。

### 3.4 验收

- `netsh advfirewall firewall show rule name="herdr-bot-tcp"` 能看到新路径。
- 旧 `omp-bot` 规则不存在。
- 手机与电脑在同一 Wi-Fi 下能打开新地址。
- 启动日志出现 `published host=herdr-bot.local.`。

---

## 4. `my-agent-toolkit` 独立后续（未完成）

本步骤在 `my-agent-toolkit` 仓库中单独提交，不属于当前仓库改名提交。

仓库：`E:\Developer\myhub\my-agent-toolkit`

### 4.1 影响面

- 受 Git 跟踪文件：678 个。
- 含旧名变体的文件：91 个，共 564 处。
- 活动代码/测试/文档：24 个文件，144 处。
- 历史 `.scratch`：67 个文件，420 处；**保持原文**。

### 4.2 必改的功能耦合

`tests/connect-bot/support/omp-bot.ts`：

- 默认 sibling 路径：

```text
../omp-bot/backend/target/release/omp-bot.exe
```

改为：

```text
../herdr-bot/backend/target/release/herdr-bot.exe
```

- 二进制名：`omp-bot.exe` / `omp-bot` → `herdr-bot.exe` / `herdr-bot`。
- 环境变量：`OMP_BOT_BINARY` → `HERDR_BOT_BINARY`。
- 错误提示中的构建路径：`../omp-bot/backend/Cargo.toml` → `../herdr-bot/backend/Cargo.toml`。
- 临时目录前缀：`omp-bot-interop-` → `herdr-bot-interop-`。
- 配置文件名：`omp-bot.toml` → `herdr-bot.toml`（该文件通过 `--config` 显式传入，文件名本身不是功能耦合，但应同步）。

如果暂时不改这个 helper，至少要在运行时显式设置：

```bat
set OMP_BOT_BINARY=E:\Developer\myhub\herdr-bot\backend\target\release\herdr-bot.exe
```

否则真实面板 interop 用例会找不到 binary。

### 4.3 用户可见文案与测试

同步更新：

- `extensions/connect-bot/definition.ts` 的命令描述。
- `extensions/connect-bot/shared-logic/feedback.ts` 的用户消息。
- `extensions/connect-bot/README.md`、`GLOSSARY.md`、`config.yaml` 注释。
- `tests/connect-bot/*.test.ts` 中逐字断言的用户消息。
- `tests/framework/config.test.ts` 中的 `omp-bot.local` 配置样例。
- `tests/harness-dock/consumer-regression.test.ts` 中的消息常量。
- `GLOSSARY-MAP.md` 的上下文描述。
- `framework/docs/harness-support.md`、`extensions/model-cost/README.md` 中涉及 `OMP_BOT_BINARY` 的说明；先确认它指的是面板 binary 还是其他 OMP binary，再决定是否改名。

不要修改：

- `.scratch/` 下的历史记录。
- 与本面板无关的 OMP 平台词汇。
- `connect-bot` 扩展 ID、`/connect-bot` 命令名、`/api/sessions/events` 协议。

### 4.4 建议执行顺序

```bat
cd E:\Developer\myhub\my-agent-toolkit
git status
git switch -c rename/connect-bot-panel-herdr-bot
```

完成编辑后：

```bat
HERDR_BOT_BINARY=E:/Developer/myhub/herdr-bot/backend/target/release/herdr-bot.exe bun test tests/connect-bot
bun run typecheck
bun test tests/harness-dock/consumer-regression.test.ts
```

提交为独立 commit，例如：

```text
rename connect-bot panel references to herdr-bot
```

### 4.5 验收

- `bun test tests/connect-bot` 通过，包含真实面板 interop 用例。
- `bun run typecheck` 无诊断。
- `connect-bot` 运行时仍使用 `http://127.0.0.1:8787` 的 loopback 端点，协议未变。
- 用户可见消息显示 `herdr-bot 面板`，不再显示 `omp-bot`。

---

## 5. 最终审计（仓库内已完成；外部仓库待做）

在当前仓库根目录执行：

```bat
git remote -v
git status
```

预期：

- remote 指向 `git@github.com:hugosol/herdr-bot.git`。
- 工作树干净。

搜索旧标识时，允许保留的只有：

- 历史 ADR 正文与术语注记。
- README 历史记录。
- `docs/omp-*.md` 中的 OMP 平台词汇。
- 说明「旧名是 omp-bot」的迁移文档。
- `.scratch/` 历史证据（外部仓库）。

以下位置必须全部是新名：

- 可执行文件：`herdr-bot.exe`
- 配置：`herdr-bot.toml`
- mDNS：`herdr-bot.local`
- 环境变量：`HERDR_BOT_*`
- 日志：`[herdr-bot]`
- 前端标题/H1：`herdr-bot`
- 仓库 remote：`hugosol/herdr-bot`
- Windows 防火墙规则：`herdr-bot-tcp` / `herdr-bot-udp`

最终冒烟：

1. 启动 `herdr-bot.exe --config herdr-bot.toml`。
2. 打开 `http://127.0.0.1:8787/`，确认标题为 `herdr-bot`。
3. 确认日志出现 `published host=herdr-bot.local.`。
4. 用手机通过 `http://herdr-bot.local:8787/` 打开页面。
5. 在 agent 侧执行 `/connect-bot`，确认会话仍能登记到面板。
6. 不触发真实关机；关机路径如需验证，使用 `backend/examples/safe_shutdown_shell.rs` 的记录型替身。

---

## 6. 回滚

### 6.1 代码回滚

```bat
git revert 6420523
```

或在 GitHub 上 revert 改名提交。旧仓库 URL 在 GitHub rename 后仍会重定向。

### 6.2 GitHub 仓库名回滚

GitHub Settings → Repository name 改回 `omp-bot`。issues/PR/历史仍在。

### 6.3 本地目录回滚

关闭占用进程后：

```powershell
Rename-Item -Path 'E:\Developer\myhub\herdr-bot' -NewName 'omp-bot'
```

### 6.4 防火墙回滚

删除 `herdr-bot-tcp` / `herdr-bot-udp`，按旧 exe 路径恢复 `omp-bot` 规则。

### 6.5 外部仓库回滚

在 `my-agent-toolkit` 中 revert 独立改名 commit；不要动 `.scratch` 历史。

---

## 7. 完成清单（截至 2026-10-21）

- [x] GitHub 仓库已原地改名为 `hugosol/herdr-bot`
- [x] 本地 remote 已更新并 `git fetch --prune` 成功
- [x] 本地目录已改名为 `E:\Developer\myhub\herdr-bot`
- [x] 新路径下 `cargo build --release` 成功
- [x] `herdr-bot.exe --config herdr-bot.toml` 启动成功
- [x] 页面标题/H1 为 `herdr-bot`
- [x] 日志出现 `[herdr-bot]` 与 `published host=herdr-bot.local.`（手机 mDNS 访问已验收）
- [x] 手机可通过 `http://herdr-bot.local:8787/` 打开
- [x] 旧 `omp-bot` 防火墙规则已删除，新 `herdr-bot-*` 规则已添加
- [ ] `my-agent-toolkit` 的 connect-bot interop helper 与文案更新完成
- [ ] `HERDR_BOT_BINARY=... bun test tests/connect-bot` 通过
- [ ] `bun run typecheck` 通过
- [x] 当前仓库最终审计通过，旧项目标识只出现在历史说明中；OMP 平台词汇按 ADR 0010 保留
