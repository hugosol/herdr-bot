# 10: Pi 模式、模板与无 Suite 基线跨仓验收

**What to build:** 经真实 Herdr 启动 Pi，验证固定 pi-suite 的无 Suite 基线、明确模式覆盖及模板按调用展开，不因共享声明中的 LSP 阻止资源使用。

**Delivers:** P14, P16, P23, P24, P26, P27

**Test owner for:** P14, P16, P23, P24, P26, P27；实现贡献票为 06、07。

**Blocked by:** 07: Pi Task 启动接入共享 Suite

**Repositories:** herdr-bot, toolkit

**Status:** ready-for-agent

## Acceptance criteria

- [ ] 实际固定扩展加载且消费受控配置，不依赖可选 Suite 声明加载 pi-suite。
- [ ] 无 Suite 时，安装适配扩展与原生基线的 MCP 和 codemode 行为一致，不追加或接管。
- [ ] 全局 only→Suite on 与全局 on→Suite only 双向对照，核对模型实际收到的工具声明；不写回原全局模式。
- [ ] only 模式经 codemode 读取真实受控文件，独立核验内容，证明底层工具未丢失。
- [ ] 模板哨兵在初始化和调用前不展开，显式调用后按原生文本模板语义展开；不作为系统指令或初始消息。
- [ ] Pi 未启动声明的 LSP，但其他适用资源实际可用。

## Coverage ownership

P14、P16、P23、P24、P26、P27 的全部契约测试归本票。06 的扩展侧回归只负责局部行为，不重复声明这些承诺已完成；P25 的浏览器模式控件归 01。

## 契约与交付约束

- 依据同一功能目录内已批准的 contract 与 spec，优先级为已批准契约；沿用 S1 主界面、S2 公开 HTTP、S3 部署配置/启动、S4 经 Herdr 启动的原生 agent 接缝，不新增测试专用公开入口。
- Acceptance mode: Full automation；人工验收例外 none。测试采用独立声明、字面结果或独立计算，不由被测实现重算预期。缺依赖、权限或真实证据明确记为阻塞/未验证，不以 skip、非空日志、零退出码或替身冒充通过。
- 复用既有文件协调、路径根和执行角色边界。不得修改 herdr/OMP/Pi、使用内部 monkey patch、重定向整个 agent 配置目录、复制认证或引入独立工具 manager。
- 测试拥有独占配置、可变目录、动态回环端口和进程；持续消费日志，正常及失败都确认所属进程退出，原始失败与清理失败分别保留。不读用户凭据，不操作用户 Chat/daemon，不真实关机，不自动安装、提权或修改系统 PATH。
- 开发与检查在本票分配的 repository worktree 内执行，遵循该仓测试和作者规范；更新受影响调用者、契约测试和用户文档，删除切换后失效的业务路径，不建立长期兼容双轨。
- P44 为所有票的交付不变量：各票提供自己的变更来源及公开入口证据；14 拥有完整交付清单一致性测试。原型只作输入，不作交付通过证据。

## 跨仓执行、检查与证据

本票是普通跨仓集成/验收票，owner 可修改列出的两个仓库。按 blocker 首次合并后的 frontier 执行，不等待其后台集成结束，不建立额外最终调度门禁。

- 明确取得本票的 herdr-bot 与 toolkit worktree 绝对根；herdr-bot 的构建、后端进程、前端产物和 toolkit 扩展必须来自这些 worktree，不得静默加载用户安装副本或其他 worktree。
- 从 herdr-bot worktree 根分别运行 `node scripts/test-backend.mjs`、`npm --prefix frontend test`、`npm --prefix frontend run test:browser:build`；该 worktree 使用独立 CARGO_TARGET_DIR。三项分别记录结果，前端构建产物必须对应当前源码。
- 从 toolkit worktree 根分别运行 `bun run typecheck`、`bun run check:integrations`、`bun run test`。工具链和依赖按该仓规范准备，各命令独立记录，不把另一仓通过算作本仓通过。
- 本票 owner 编写本票场景的专项自动验收，复用已有隔离与进程设施，不依赖尚未完成的兄弟验收票或另建通用框架。09 与 10 可并行，11 可复用已完成的两票设施；共享文件修改须明确单一整合 owner。
- 专项入口必须显式接受两个 worktree 根、当前后端二进制、隔离部署配置、场景选择与证据目录。从 herdr-bot worktree 启动，加载所指 toolkit 源码；隔离配置中指定只读真实 Herdr/Pi/OMP 依赖，不读取用户业务根或真实 auth。
- 交付时在验收文档记录已经实际运行的完整命令与参数来源，使其他 agent 能对同一组 worktree 复跑。新专项入口是本票交付项，不声称当前已存在。全局套件不能代替本票每条承诺的专项检查。
- 每条验收保留 Promise/coverage 标识、独立预期、实际请求与结果、源码和二进制身份、宿主版本、原生工具/资源/模式/历史证据，以及所属进程退出确认；密钥不进入命令、日志或证据。失败返回非零且保留证据，不自动重试业务行为。

## 专项入口交付与 worktree 调用

以下是本票必须实现的入口契约，不声称该脚本当前已存在。由 herdr-bot worktree 根执行，参数均指向本票拥有的 worktree/隔离数据：

```powershell
python scripts/accept-suite-pi-mode.py --herdr-bot-worktree "$HERDR_BOT_WT" --toolkit-worktree "$TOOLKIT_WT" --backend "$BACKEND_BINARY" --paths-file "$ISOLATED_PATHS" --evidence-dir "$EVIDENCE_DIR"
```

入口覆盖本票全部 acceptance criteria，缺依赖或清理失败非零；证据按 coverage item 关联，不以两仓全局套件替代。参数应验证源码和二进制对应关系，不静默回退其他部署。路径仅用于明确可执行检查命令，不限定内部实现布局。
