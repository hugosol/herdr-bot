# Suite 加载契约核查

当前决策（Q29）：用户已放弃整体替换 Pi agentDir；使用原生资源参数及单个 suite 适配扩展，复用公开 MCP/codemode 工厂。以下配置目录替换试验仅保留为研究证据，不代表当前方案。认证继续使用原路径，不需要 auth.json 链接或锁映射。

范围：指定本地 OMP/Pi checkout。除 Pi 目录解析及下述原生 MCP 配置读取 smoke 外，以下为源码证据，不是端到端验收。未改宿主源码。

## 原生配置目录替换补充验证

用户要求优先用本次配置文件替代而非 extension 工厂。已实际调用当前 Pi 的 loadMcpConfig，创建两个独立临时 agentDir 和一个带 .pi/mcp.json 的临时项目：

- agent-a + projectTrusted:false → 仅 suite-a，errors=[]。
- agent-b + projectTrusted:false → 仅 suite-b，errors=[]。
- agent-a + projectTrusted:true → suite-a 与 project-only，errors=[]。

三组精确断言通过，临时目录已删除；没有运行任何配置中的程序或连接 MCP。证明配置读取层可通过独立 agentDir 与项目来源开关隔离，不证明完整 CLI/env 传入、认证/会话保持或实际连接。PI_CODING_AGENT_DIR 指向整套 agent 配置；--no-approve 排除多类项目配置而非仅 MCP。原生配置目录方案优先，下面的 extension 工厂保留为备选。

## Pi

### 目录入口需要 manifest

实际 CLI 在 `packages/coding-agent/src/core/resource-loader.ts:557-560` 调用 package-manager 的 `resolveExtensionSources`，不能只依据旧 discoverAndLoadExtensions 判断。

父会话运行了当前源码的 `DefaultPackageManager.resolveExtensionSources`，使用临时 cwd/agentDir、SettingsManager.inMemory({})、builtinExtensions:[]，不调用 resolve、不执行 extension、不联网：

1. 裸 extensions/ 包含 alpha.ts、beta/index.ts，结果仅保留 extensions 目录本身，未展开两个入口。
2. 写入 `{"pi":{"extensions":["./alpha.ts","./beta"]}}` 后，结果是 alpha.ts 与 beta/index.ts，精确断言通过。
3. 临时目录已删除。未使用真实软链接，未证明 Windows 链接权限、模块导入或完整 CLI 启动。

生成目录必须有明确入口清单。它应只声明 extensions，不附带未选择的 skills/prompts。Skill 使用 --no-skills + 重复 --skill；模板使用 --no-prompt-templates + 重复 --prompt-template。模板目录只取直接子级 .md，不递归。扩展主动通过 resources_discover 或 package manifest 贡献资源不受这些 no-* 沙箱式阻止，所选扩展契约须审查。

### MCP 可复用公开工厂

公开导出：`packages/coding-agent/src/index.ts:414-415`。CLI extension 的 virtual module 暴露包根（`core/extensions/virtual-modules.ts:9-14,33-41`）。

`createMcpExtension(options)` 的 `loadConfig?: (ctx: ExtensionContext) => LoadedMcpConfig` 是公开选项（`extensions/mcp/index.ts:69-91,279`）。可由一个 suite adapter：

- 在工厂初始化时通过公开 registerMcpServer 注册合并、校验后的 suite 项；
- 调用 createMcpExtension，传入只返回空文件来源的 loadConfig；
- 复用工厂的 /mcp、连接、工具及事件实现，不重写协议；
- --no-extensions --no-mcp，并显式选择固定扩展、该 adapter、suite 扩展；不再启用 builtin:mcp。

--no-mcp 只过滤 builtin:mcp，不过滤普通 adapter（main.ts:792-799；resource-loader.ts:609-615；docs/cli.md:198-199）。工厂加载时注册，session_start 时合并注册表再连接（extensions/mcp/index.ts:1114-1150；extensions/types.ts:1905-1924）。需要 codemode/deferred exposure 时显式保留对应 builtin，不能让配置存在但工具不可用。

原来的“只调用 registerMcpServer 就能隔离”不成立：默认实例仍读取全局/项目 mcp.json，且文件同名项优先。应替换默认实例的配置读取，但复用公开实现。

Pi LSP：按用户已批准规则跳过，不要求二套 suite 定义或额外实现。

## OMP

### Skills/MCP 存在来源过滤路径，但会影响其他资源

`disabledProviders` 是正式设置（config/model-settings.ts:77-82），在 capability/index.ts:306-342 扫描前过滤整个 provider，不是单一资源类型的开关。

- managed skills 的 provider 是 omp-managed；可以在此层禁用，不必用 --no-skills。
- customDirectories 独立扫描，不受 provider 列表枚举禁用（extensibility/skills.ts:490-510）。
- 单独 skills 隔离可禁用当前 skill providers：native、omp-managed、agents、claude、claude-plugins、codex、github、opencode、skillshare、agent-plugins、omp-plugins。
- 若用 OMP package mcp.json，则保留 omp-plugins，--no-extensions 将其根限制为显式 package roots。这些根必须由系统控制；固定扩展使用精确入口，避免无意携带兄弟资源（discovery/omp-extension-roots.ts:274-340）。
- MCP 排除其他来源还需禁用 native、mcp-json、claude、claude-plugins、codex、cursor、gemini、opencode、vscode、windsurf、agent-plugins；与技能集合合并。

这条路径可以阻止其他 provider 贡献服务器，但不是“只改变 skills/MCP”。同 provider 的其他能力发现也会受影响。是否接受此副作用尚须明确，不能承诺其余上下文资源全保持原样。

### MCP 仍受用户 enabled/disabled 名单影响

`mcp/config.ts:110-152` 在 capability discovery 之外直接读取原生用户 mcp.json 的 disabledServers/enabledServers。即使禁用 native/mcp-json provider，同名 suite server 仍可能被用户 denylist 压制。默认来源不连接与配置完全自包含是两个不同承诺。

公开 extension settings override 可覆盖已有设置（docs/extensions.md:289-312），但不是任意 MCP 列表替换接口。session_start 不能作为阻止默认连接的可靠钩子：非 UI 连接在 runner 绑定之前，UI deferred discovery 在 session 返回前开始（sdk.ts:2888-2894,5886）。

### Prompts 有确定的原生接口缺口

实际 prompt-template loader 固定读取 agentDir/prompts 与 cwd 项目 prompts，不经过 provider 禁用（config/prompt-templates.ts:153-181；sdk.ts:2074-2077）。package 的 prompts capability 不等于当前会话的 templates 数组。

resources_discover 虽在类型和 runner 中定义，当前没有 AgentSession 调用点，官方文档也明确说明（docs/extensions.md:658-661）。不能依赖它注入 suite promptPaths。

没有已证明的公开 extension 接口能替换当前会话的原生 templates 数组。SDK options 属于宿主，不属于 extension 权限。注册 slash command 可以模拟部分体验，但不等于已满足用户要求的原生 prompt template 语义；未经同意不能作为等价替代。

### LSP 为增量配置，不是白名单

--plugin-dir 可将 suite 根加入 LSP 配置发现；lsp.json 可从这些根读取（main.ts:1884-1890；utils/config-file-sources.ts:41-69）。但根与已有 registry 合并，且 bundled defaults、home/user/project 配置仍参与。项目和用户配置优先级高于 plugin root。disabledProviders 不控制这条直接文件扫描路径。

所以 suite 可贡献 LSP 配置，但无法仅凭该方式排除所有外部/默认 LSP。用户已明确要求 skills/MCP 默认隔离；LSP 是否也要求同等强度尚未明确，不自行扩大或缩减。

## 运行验证边界

- Pi 裸目录与 manifest 解析实际运行成功，临时文件已清理。
- 前次 OMP 源码导入被缺失 pi_natives Windows addon 阻塞，未通过源码运行；本次未构建、修改或重装宿主。
- 未连接真实或受控 MCP；Pi 工厂方案目前是源码链路证明。
- 未运行真实软链接加载，后续必须按 testing.md 开关在隔离目录验证。
