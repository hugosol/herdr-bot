# 09: Pi 资源与原生 MCP 跨仓验收

**What to build:** 使用两个参与 worktree 经真实 Herdr 启动 Pi，证明共享 Suite 资源和原生 MCP 增量能够同时成立。

**Delivers:** P6, P7, P9, P10, P17, P19, P20, P21, P46

**Test owner for:** P6/P7/P9/P10/P17/P19/P20/P21 的 Pi 场景；P46 的 Pi 配置、装配与运行期场景（缺 MCP 引用除外）。实现贡献票为 03、04、06、07。

**Blocked by:** 07: Pi Task 启动接入共享 Suite

**Repositories:** herdr-bot, toolkit

**Status:** ready-for-agent

## Acceptance criteria

- [ ] 同一逻辑声明分别通过已交付 OMP 路径和 Pi 路径消费；本票独占 Pi 的集中根、声明相对路径、含空格及不同 cwd 证据。
- [ ] Pi 实际读取技能脚本、资产和扩展相对依赖；生成入口可消费，已选/未选哨兵与新增源技能的独立集合一致。
- [ ] 技能过滤的同一次启动必须保留 builtin MCP，实际调用原生与 Suite 服务器，不能关闭来源取得过滤结果。
- [ ] 空项目调用集中 MCP 且项目未写复制文件；覆盖文件同名优先、异名等价、禁用、空清单，并独立核对胜者、连接次数与文件结果。
- [ ] 分别制造 Pi 不可读技能、扩展、生成失败和不兼容 Suite，启动拒绝且不以空环境继续；缺 MCP 引用引用 02 的专属测试证据。
- [ ] 有效但不可达 MCP 使用原生连接状态，不能新增全部就绪门禁；保留原始错误与状态证据。

## Coverage ownership

本票拥有 P6/P7/P10/P17/P19/P20/P21 的全部 Pi 契约场景，以及 P46 的 Pi 配置/装配失败（缺 MCP 引用除外）和 Pi MCP 运行期场景。OMP 对应场景归 03/04/08；P9 的 OMP 基础测试归 04，本票拥有独立 Pi 依赖消费场景。

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

## 跨票 coverage partition

表中每个场景只有一个契约测试 owner；同一 Promise 的全部场景完成才算整体交付。其他贡献票不得重复认领此场景。

| Promise | Coverage item | 贡献票 | Test owner |
| --- | --- | --- | --- |
| P6 | OMP 根引用、声明相对路径、空格及不同 cwd | 03 | 03 |
| P6 | Pi 根引用、声明相对路径、空格及不同 cwd | 03/07 | 09 |
| P7 | OMP 技能产物实际消费 | 03 | 03 |
| P7 | OMP 扩展产物实际消费 | 04 | 04 |
| P7 | Pi 共享声明产物实际消费 | 03/04/06/07 | 09 |
| P9 | OMP 扩展工厂、相对依赖与资产 | 04 | 04 |
| P9 | Pi 扩展工厂、相对依赖与资产 | 06/07 | 09 |
| P10 | OMP 精确技能与原生 MCP 同次成立 | 03 | 03 |
| P10 | Pi 精确技能与原生 MCP 同次成立 | 06/07 | 09 |
| P17 | 集中引用选择/校验 | 01/02 | 02 |
| P17 | OMP 空项目消费集中定义且未复制项目配置 | 08 | 08 |
| P17 | Pi 空项目消费集中定义且未复制项目配置 | 06/07 | 09 |
| P19 | OMP 原生来源保留与增量调用 | 08 | 08 |
| P19 | Pi builtin 保留与追加注册 | 06/07 | 09 |
| P20 | OMP 同名、等价异名、禁用与连接次数 | 08 | 08 |
| P20 | Pi 文件同名、等价异名、禁用与连接次数 | 06/07 | 09 |
| P21 | OMP 空清单下原生调用 | 08 | 08 |
| P21 | Pi 空清单下原生调用 | 06/07 | 09 |
| P46 | 缺集中 MCP 引用的保存/启动拒绝（复用 P18） | 02 | 02 |
| P46 | OMP 不兼容引用启动拒绝（复用 P1） | 02 | 02 |
| P46 | OMP 技能不可读与装配生成失败 | 03 | 03 |
| P46 | OMP 扩展不可读与装配生成失败 | 04 | 04 |
| P46 | Pi 不兼容引用/不可读资源/装配生成失败 | 06/07 | 09 |
| P46 | OMP 有效但不可达 MCP 的原生运行期状态 | 08 | 08 |
| P46 | Pi 有效但不可达 MCP 的原生运行期状态 | 06/07 | 09 |

## 专项入口交付与 worktree 调用

以下是本票必须实现的入口契约，不声称该脚本当前已存在。由 herdr-bot worktree 根执行，参数均指向本票拥有的 worktree/隔离数据：

```powershell
python scripts/accept-suite-pi-resources.py --herdr-bot-worktree "$HERDR_BOT_WT" --toolkit-worktree "$TOOLKIT_WT" --backend "$BACKEND_BINARY" --paths-file "$ISOLATED_PATHS" --evidence-dir "$EVIDENCE_DIR"
```

入口覆盖本票全部 acceptance criteria，缺依赖或清理失败非零；证据按 coverage item 关联，不以两仓全局套件替代。参数应验证源码和二进制对应关系，不静默回退其他部署。路径仅用于明确可执行检查命令，不限定内部实现布局。
