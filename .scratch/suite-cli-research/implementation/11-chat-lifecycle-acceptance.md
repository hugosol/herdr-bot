# 11: 发起、workflow、恢复及并发启动跨仓验收

**What to build:** 独立 Chat、manager、node 与恢复使用最新环境而不改变固定角色、trigger 或原生身份；并发启动只消费一致产物。

**Delivers:** P12, P13, P28, P29, P30, P31, P32, P35

**Blocked by:** 05: 装配清理、真链接与一致发布; 08: OMP 原生 MCP 完整增量能力; 09: Pi 资源与原生 MCP 跨仓验收; 10: Pi 模式、模板与无 Suite 基线跨仓验收

**Repositories:** herdr-bot, toolkit

**Status:** ready-for-agent

## Acceptance criteria

- [ ] 两宿主有/无 Suite 的固定扩展实际行为保持，独立普通 Chat 不获得 node 角色，manager/node 与恢复执行关联遵守既有规则。
- [ ] 用测试拥有的认证数据与隔离存储观察并发协调，禁止读取真实凭据、复制/链接认证或泄漏密钥；不能以未传 agentDir 参数代替证据。
- [ ] 独立读取原生模型配置与历史位置/身份；模型、思考、初始 Skill、初始消息和命名按字面预期保持。
- [ ] 修改来源 Task 和 Suite、重启后恢复使用最新环境，保留原身份且不重放 trigger；无来源及已删除来源按既有原生恢复边界，不猜测 Suite。
- [ ] 保存 Suite 后，已有 Chat 无产品主动热重配事件，后续启动使用新定义；不扩展为冻结第三方资源或禁用宿主原生重载。
- [ ] 同 Suite 与不同 Suite 并发真实启动均观察独立完整哨兵，不混合产物；文件协调不跨外部 Herdr 启动。

## Coverage ownership

P12/P13/P28/P29/P30/P31/P32 全部及 P35（并发真实启动消费）测试归本票；P35（装配发布）归 05。

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
| P35 | 同/不同 Suite 装配发布一致性 | 03/04/05 | 05 |
| P35 | 同/不同 Suite 并发真实启动消费 | 05/07/08 | 11 |

## 专项入口交付与 worktree 调用

以下是本票必须实现的入口契约，不声称该脚本当前已存在。由 herdr-bot worktree 根执行，参数均指向本票拥有的 worktree/隔离数据：

```powershell
python scripts/accept-suite-lifecycle.py --herdr-bot-worktree "$HERDR_BOT_WT" --toolkit-worktree "$TOOLKIT_WT" --backend "$BACKEND_BINARY" --paths-file "$ISOLATED_PATHS" --evidence-dir "$EVIDENCE_DIR"
```

入口覆盖本票全部 acceptance criteria，缺依赖或清理失败非零；证据按 coverage item 关联，不以两仓全局套件替代。参数应验证源码和二进制对应关系，不静默回退其他部署。路径仅用于明确可执行检查命令，不限定内部实现布局。
