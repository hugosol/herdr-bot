# 01: Suite 权威定义与图形管理

**What to build:** 用户通过主界面管理 Suite；界面保存、手工编辑和程序重读使用同一权威文件，Pi 模式保存为明确的 on/only。

**Delivers:** P3, P4, P25

**Blocked by:** None (can start immediately)

**Repositories:** herdr-bot

**Status:** ready-for-agent

## Acceptance criteria

- [ ] 通过真实浏览器完成未被引用 Suite 的创建、列表、查看、修改、删除，并独立读取文件核对结果。
- [ ] 外部写入独立定义后重读与重启均呈现该定义；不得建立数据库副本或把生成目录作为权威源。
- [ ] 支持资源声明、宿主适用范围和集中 MCP 选择；只读已有 MCP 目录，不新增 MCP 编辑平台。
- [ ] 真实操作 Pi on/only、保存重读和宿主切换；OMP-only 编辑不显示 Pi 控件，不把关闭 only 解释为继承或禁用。

## Coverage ownership

P3、P4、P25 的全部测试归本票。Task 引用校验、删除保护及失效 MCP 反馈归 02。

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

## 跨票 coverage partition

表中每个场景只有一个契约测试 owner；同一 Promise 的全部场景完成才算整体交付。其他贡献票不得重复认领此场景。

| Promise | Coverage item | 贡献票 | Test owner |
| --- | --- | --- | --- |
| P17 | 集中引用选择/校验 | 01/02 | 02 |
| P17 | OMP 空项目消费集中定义且未复制项目配置 | 08 | 08 |
| P17 | Pi 空项目消费集中定义且未复制项目配置 | 06/07 | 09 |
