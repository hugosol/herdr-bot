# 08: 跨仓验收：同一历史 Chat 在两端恢复

**What to build:** 提供可重复运行的配对专项入口，在单端运行前提下证明跨端实际启动、定位与原生身份核验，并记录未确认和硬失败而不是给出泛化成功。依据[合同](../contract.md)基础 seam 与 B1、[规格](../spec.md)。

**Delivers:** P8, P10, P11, P12, P13, P14, P15, P29, P30, P31, P32, P33, P35, P41, P54, P55

**Blocked by:** 01 — framework 发布会话级 ready；07 — 恢复历史 Chat 并如实显示环境与身份结果

**Repositories:** herdr-bot, toolkit

**Status:** ready-for-agent

- [ ] OMP/Pi 分别实跑 TUI→App、App→TUI：先确认旧 agent 与旧端退出，公开 Resume 后核对真实新进程、原生 ID/file、实际 cwd 与宿主焦点；不能用启动参数、同名容器或终端存在代替。保留原历史记录、既有持久格式和原始容器 ID（P12/P29/P41）。
- [ ] 真实宿主观察同端原 ID 命中、跨端无匹配与人为同值命中；单独构造缺字段、成功空结果、错误和畸形响应，需注入时区分受控边界。实际核对配置新名称、有效 cwd、可靠唯一 Git 归属，及缺失/歧义时未指定 cwd、由默认容器选实际目录的证据，包含 App 限制（P8/P10–P15）。
- [ ] 发起后改来源环境，恢复核对实际最新 Suite/env config 与无 trigger 重放；分别覆盖无来源、已删除、环境读取失败和 Suite 配置错误，验证原生默认发现、不附加强制 connect-bot。真实缺证据只记已请求并定位／未确认，不改绑或补启动；已知 ID／文件头／agent 冲突及真实启动/定位失败不能被降级掩盖（P30–P33/P35）。
- [ ] 从票据 `herdr-bot` worktree 用独占绝对 `CARGO_TARGET_DIR` 构建本次页面和后端 binaries/安全关机壳，分别通过 `node scripts/test-backend.mjs`、`npm --prefix frontend test`、`npm --prefix frontend run test:browser:build`；从票据 `toolkit` worktree 用冻结依赖分别通过 `bun run typecheck`、`bun run test:framework`、`bun run check:integrations`、`bun run test`，另跑受影响的 native session/connect-bot docks，配对 dock 使用本次安全壳。分别保留原始日志。
- [ ] 从 `herdr-bot` worktree 根运行本票专项，文档提供可复制命令，显式传入两仓绝对路径、本次后端、隔离宿主配置、两种切换方向和新证据目录。记录安装身份、公开结果、原生历史及原始失败；只管理自有实例／状态目录，不读用户认证，实际观察清理退出（P54/P55）。全仓通过不代替单格证据；缺授权、依赖、覆盖或退出确认保持未验证。
