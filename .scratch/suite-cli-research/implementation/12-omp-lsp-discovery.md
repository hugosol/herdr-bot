# 12: OMP 四语言 LSP 与缺失状态

**What to build:** 声明的四种语言服务按 OMP 原生项目标记、可执行发现与优先级运行，缺少程序不会误报已运行。

**Delivers:** P37, P38

**Blocked by:** 03: OMP 精确技能环境与路径消费

**Repositories:** herdr-bot

**Status:** ready-for-agent

## Acceptance criteria

- [ ] 使用已具备运行条件的隔离 Rust、TypeScript、Java、Python 项目，经 Herdr 启动实际 OMP 并观察四种原生语言服务启动。
- [ ] 使用批准的 rust-analyzer、typescript-language-server、jdtls、pyright 对应定义，保留默认/用户/项目配置和原生优先级，不承诺白名单排除其他服务。
- [ ] 构造缺可执行程序负例，独立观察未运行与产品反馈；声明存在不等于服务已启用或运行。
- [ ] 缺少程序明确记录阻塞与未验证，不自动安装、不修改系统 PATH，不用 Pi 跳过 LSP 证明 OMP 已通过。

## Coverage ownership

P37、P38 全部测试归本票。Pi 跳过 LSP 且资源仍可用的 P14 测试归 10。

## 契约与交付约束

- 依据同一功能目录内已批准的 contract 与 spec，优先级为已批准契约；沿用 S1 主界面、S2 公开 HTTP、S3 部署配置/启动、S4 经 Herdr 启动的原生 agent 接缝，不新增测试专用公开入口。
- Acceptance mode: Full automation；人工验收例外 none。测试采用独立声明、字面结果或独立计算，不由被测实现重算预期。缺依赖、权限或真实证据明确记为阻塞/未验证，不以 skip、非空日志、零退出码或替身冒充通过。
- 复用既有文件协调、路径根和执行角色边界。不得修改 herdr/OMP/Pi、使用内部 monkey patch、重定向整个 agent 配置目录、复制认证或引入独立工具 manager。
- 测试拥有独占配置、可变目录、动态回环端口和进程；持续消费日志，正常及失败都确认所属进程退出，原始失败与清理失败分别保留。不读用户凭据，不操作用户 Chat/daemon，不真实关机，不自动安装、提权或修改系统 PATH。
- 开发与检查在本票分配的 repository worktree 内执行，遵循该仓测试和作者规范；更新受影响调用者、契约测试和用户文档，删除切换后失效的业务路径，不建立长期兼容双轨。
- P44 为所有票的交付不变量：各票提供自己的变更来源及公开入口证据；14 拥有完整交付清单一致性测试。原型只作输入，不作交付通过证据。

## 检查与证据

- 遵循协调仓测试指引：开发反馈选择受影响后端目标或前端 npm 测试；交付按受影响面运行 `node scripts/test-backend.mjs`、`npm --prefix frontend test`，涉及页面行为时运行 `npm --prefix frontend run test:browser:build` 并观察真实浏览器。
- S4 承诺必须经真实 Herdr 与实际交付宿主入口运行专项场景。受控 MCP/回环模型可证明机制，不证明真实模型自主任务遵循。新入口须随交付记录可复跑命令、隔离配置来源及证据位置。
- 不能用普通套件、参数断言、历史原型或静态配置代替本票明确的行为证据。仅修改票据不表示任何产品检查已经运行或通过。
