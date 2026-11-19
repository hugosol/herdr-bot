# 15: 跨仓验收：精确自动关闭与原生提醒

**What to build:** 提供可重复运行的配对专项入口，使用真实宿主独立验收节点完成后的目标关闭与 agent 原生提醒，留下唯一人工听音项所需的证据。依据[合同](../contract.md)的 S1/S2/S3 与[规格](../spec.md)。

**Delivers:** P43, P44, P45, P47, P48, P54, P55

**Blocked by:** 01 — framework 发布会话级 ready；11 — 节点完成后只关闭准确关联的终端；12 — 将 agent 状态接入两端原生提醒

**Repositories:** herdr-bot, toolkit

**Status:** ready-for-agent

- [ ] 从公开节点完成操作触发两端真实关闭；独立记录目标 Tab 与进程、关闭回执、布局移除、实际退出。最后一个 Tab 走 App 显式确认（不是 park/settle），并核对实际空容器结果；额外 Tab、无关进程及需要扩大 worktree group 的场景核对无关目标存活（P43–P45）。
- [ ] 使用真实 OMP/Pi × TUI/App 状态转换核对正确 Pane/terminal 与原生提醒状态；验证各端原生开关、资源、适用焦点抑制、触发时序及无 bot 重复播放。不用设置页试听代替状态链路（P47/P48）。逐端归档可重现触发步骤、配置、时间线及可取得的播放／录音证据；实际可听且符合场景只在所有自动工作后由用户确认，未确认不算通过，也不阻塞本票的自动交付。
- [ ] 从本票 `herdr-bot` worktree 用独占绝对 `CARGO_TARGET_DIR` 构建本次页面和后端 binaries/安全关机壳，分别通过 `node scripts/test-backend.mjs`、`npm --prefix frontend test`、`npm --prefix frontend run test:browser:build`；从本票 `toolkit` worktree 用冻结依赖分别通过 `bun run typecheck`、`bun run test:framework`、`bun run check:integrations`、`bun run test`，另跑受影响的 native lifecycle/state docks，配对 dock 使用本次安全壳。分别归档原始日志。
- [ ] 从 `herdr-bot` worktree 根运行本票专项，文档提供可复制命令，显式传入两仓绝对路径、本次后端、隔离配置、关闭与提醒场景和新证据目录；保存安装身份、公开状态、原生观察及原始失败。只操作自有资源，实际确认各所属进程退出再清理，无真实关机，不碰用户认证或现存 Chat（P54/P55）。任一自动覆盖缺失、检查失败或清理失败不能以全仓绿补齐。
