# Suite 启动能力核查

日期：2026-11-03。仅研究，不构成已确认设计，不修改实现或领域术语。

## 结论

herdr 已支持 agent argv 透传；完整 suite 不能仅用两种运行时共同的原生 CLI 参数表达。应区分资源选择与严格来源白名单。下述结论针对本地 checkout，不套用上游历史版本印象。

## 现有链路与术语

- Task 是预设，Chat 是实际对话；env config 用于启动及新进程恢复，trigger config 只用于发起。参见 [GLOSSARY](../../GLOSSARY.md)。
- herdr-bot 目前组装模型/thinking 和 `--no-extensions --extension <entry>...`：[实现](../../backend/src/chats/mod.rs#L248)。这包含固定扩展和用户选择，不是仅用户勾选。
- herdr CLI 接收 `--` 后的 AGENT_ARG：[绑定](E:/Developer/myhub/herdr/src/cli/agent/bindings.rs#L103)。服务端将 args 加到 executable 后：[实现](E:/Developer/myhub/herdr/src/app/agents.rs#L227)。

## OMP

- `--skills` 是逗号分隔的技能名称 glob，不是路径。`--no-skills` 将 skills 设为空，不能与 `--skills` 组合成 Pi 式显式路径白名单。[main.ts](E:/Developer/myhub/oh-my-pi/packages/coding-agent/src/main.ts#L1751)
- `--config <file>` 是可重复 YAML overlay；可以由系统生成运行级配置，用 `skills.customDirectories` 指定技能发现目录。因此“OMP 无法指定 skill 路径”表述过宽：缺少的是 Pi 式直接路径参数，并非缺少配置途径。应优先核查该现有途径，而非直接认定必须由 extension 补齐。[skills.ts](E:/Developer/myhub/oh-my-pi/packages/coding-agent/src/extensibility/skills.ts#L485)
- 当前 customDirectories 调用扫描器未传 `includeSelf`：扫描的是目录下直接子目录中的 `SKILL.md`，不是目录自身的 `SKILL.md`。适合传 skills 集合目录；精确挑选可结合 includeSkills 名称过滤，或另行评估生成仅含所选目录引用的集合目录。[扫描实现](E:/Developer/myhub/oh-my-pi/packages/coding-agent/src/discovery/helpers.ts#L467)
- 来源开关可以排除常见 native/agents/Claude 来源，但不应当作完整来源白名单：managed skills 另行加入，插件来源也需单独检查；最终 includeSkills 是名称过滤，不是来源身份锁定。严格只含 suite 资源的组合尚未完成运行验证。[来源过滤](E:/Developer/myhub/oh-my-pi/packages/coding-agent/src/extensibility/skills.ts#L305)
- `--system-prompt`、`--append-system-prompt` 接收文本或文件；system 不会取消生成的 skills/rules/context。[说明](E:/Developer/myhub/oh-my-pi/docs/system-prompt-customization.md#L48)
- `--system-prompt-template` 是 Handlebars system 模板，不是 slash-command prompt template。后者由 agentDir/prompts 和 cwd/.omp/prompts 加载，未发现原生指定/关闭该目录发现的 CLI 参数。[loader](E:/Developer/myhub/oh-my-pi/packages/coding-agent/src/config/prompt-templates.ts#L165)
- `--no-extensions -e ...` 保留显式扩展；不同时关闭 skills/MCP/rules/context 等发现。[说明](E:/Developer/myhub/oh-my-pi/docs/extension-loading.md#L109)
- MCP 有原生支持，但未发现 `--mcp-config`、`--mcp-server`、`--no-mcp` 启动参数。使用 profile/user/project MCP 文件、其他工具配置及 package discovery。[MCP 来源](E:/Developer/myhub/oh-my-pi/docs/mcp-config.md#L14)
- `mcp.enableProjectConfig:false` 排除项目来源，不排除外部用户来源；`enabledServers` 是强制启用而非 allowlist。`--config` 不是任意 MCP server map 注入接口。[config](E:/Developer/myhub/oh-my-pi/packages/coding-agent/src/mcp/config.ts#L105)
- `--profile` 隔离原生用户配置及 auth/sessions/caches；不隔离 cwd 和 foreign 配置。不能直接等同 suite。[profile](E:/Developer/myhub/oh-my-pi/docs/config-usage.md#L76)
- 扩展可 registerFlag/getFlag，支持 string/boolean；没有通用 `--extension-config <json>`。[解析](E:/Developer/myhub/oh-my-pi/packages/coding-agent/src/cli/args.ts#L278)

## Pi

- `--skill <path>` 和 `--prompt-template <path>` 可重复；分别与 `--no-skills` / `--no-prompt-templates` 组合，保留显式资源。扩展 package 和 resources_discover 仍可贡献额外资源，非安全沙箱。[loader](E:/Developer/myhub/pi/packages/coding-agent/src/core/resource-loader.ts#L609)
- `--system-prompt` / `--append-system-prompt` 支持文本或现存文件；不存在的路径会作为文本，而非严格报错。最终 system 仍可含 context、skills、cwd 等。[解析](E:/Developer/myhub/pi/packages/coding-agent/src/core/resource-loader.ts#L195)、[组装](E:/Developer/myhub/pi/packages/coding-agent/src/core/system-prompt.ts#L152)
- 本地 Pi **有原生 MCP**，为默认内置 `builtin:mcp`。`--no-extensions` 也关闭内置扩展；需要 MCP 时显式 `-e builtin:mcp`。默认 codemode exposure 还需 codemode，或配置 direct exposure。[内置列表](E:/Developer/myhub/pi/packages/coding-agent/src/extensions/index.ts#L7)
- 有 `--no-mcp`，没有原生运行级 `--mcp-config` / `--mcp-server`。`pi mcp add` 等管理命令不是临时启动参数。
- MCP 从 agentDir/mcp.json 和可信 cwd/.pi/mcp.json 合并；项目遗漏的用户服务器仍保留，非白名单。[MCP config](E:/Developer/myhub/pi/packages/coding-agent/src/extensions/mcp/config.ts#L146)
- `PI_CODING_AGENT_DIR` 可以切换配置目录，但同时影响 auth/models/settings/默认存储等；不是只替换 suite 资源。`--no-approve` 排除项目配置，`--no-context-files` 单独控制上下文文件。[配置说明](E:/Developer/myhub/pi/docs/configuration.md#L7)
- 未发现内置 `--suite` / `--profile`。扩展配置需遵循各扩展的 registerFlag、文件或环境变量契约，没有统一任意 JSON 注入口。[flag API](E:/Developer/myhub/pi/packages/coding-agent/src/core/extensions/types.ts#L1715)

## 验证

实际运行并成功返回：`herdr agent start --help`、`pi --help`、`omp --help`。OMP 报 v18.16.0；Pi 帮助列出 MCP 管理、builtin extension、资源路径和隔离参数；herdr 帮助确认 `[-- [AGENT_ARG]...]`。

帮助来自 PATH 中已安装命令；源码分析来自指定仓库，未断言两者 revision 完全一致。未启动实际 suite 会话，未连接 MCP 或调用模型；组合隔离结论来自源码，不是端到端执行证明。

## 已确认的设计边界

- 最新决策：Suite 是场景工具集合；Task 至多引用一个 suite，不接管 trigger config。不实现跨 suite 配置合并、优先级或继承；同一 suite 仍可被多个 Task 复用。下方历史研究中涉及多 suite 的技术示例不再表示当前产品范围，以 spec.md 为准。
- 排除默认 skills/MCP；prompts 保持 Pi/OMP 原生模板语义。
- 同一资源相同配置去重，冲突不隐式覆盖。优先复用宿主现有去重，不重写加载器。
- MCP 定义集中登记，suite 引用；本地路径允许引用配置参数。只管理 Chat 初始化加载，不托管额外长期进程。
- 不得修改 herdr、OMP、Pi 的源代码；此前提出的修改原生启动接口方案已排除。
- 优先复用原生 CLI；系统解析配置后生成传给 herdr 的启动参数。遇到原生能力缺口，先核查 OMP/Pi 的公开 extension 能力能否补齐；不能仅因 SDK 或内部实现存在同名能力就认定 extension 可用。
- 若 extension 仍无法满足要求，先明确具体限制，再与用户讨论调整方案，不静默放宽资源白名单或改变资源语义。Suite 外部模型优先不按 harness 区分。
- 始终使用当前配置，不做历史配置版本管理。
- 保持现有固定扩展默认加载逻辑；suite 不替代固定扩展。
- 用户已确认按 suite 物理目录设计推进：系统生成 skills/extensions 软链接，启动输入指向相应目录。
- Suite 配置保持统一；Pi 当前不支持原生 LSP，加载时跳过 LSP 部分，不新增 Pi LSP 实现，不拆两套用户配置。

## 去重补充核查

- Pi `DefaultResourceLoader.mergePaths` 将路径解析后按 `getResourcePathKey` 去重，保留第一次。直接调用当前源码方法的 smoke：三个相同输入返回一个绝对路径，成功。该 smoke 只证明路径合并，不证明扩展工厂执行次数。[源码](E:/Developer/myhub/pi/packages/coding-agent/src/core/resource-loader.ts#L1056)
- OMP `discoverExtensionPaths` 当前源码用 realpath（失败退回绝对路径）作为 key，同一文件保留第一次。[源码](E:/Developer/myhub/oh-my-pi/packages/coding-agent/src/extensibility/extensions/loader.ts#L640)。其 extension-loading 文档仍说不使用 realpath，与当前实现不一致，以源码为准。直接导入源码运行 smoke 被 checkout 缺失 pi_natives Windows 原生插件阻塞，未宣称运行通过。
- 原生路径去重不等于 suite 配置冲突检查：Pi MCP 按名字 Map 合并，合法项目同名项可覆盖用户项，不会普遍按“不同配置即冲突”拒绝。[源码](E:/Developer/myhub/pi/packages/coding-agent/src/extensions/mcp/config.ts#L110)
- 现有 `${roots.name}` 是路径开头专用引用，不是通用字符串插值；MCP command/args/env 中哪些字段消费路径引用仍待确定。[约定](../../README.md#本机根目录与解析规则)

## 技能链接目录候选方案

用户提出每个 suite 建立 skills 目录，目录内用软链接引用实际技能。此方案可复用双方原生扫描器：OMP 接受直接子目录的符号链接并读取其 SKILL.md（discovery/helpers.ts:515-535）；Pi 扫描器对符号链接 stat 后跟随目录（core/skills.ts:220-247）。每个 suite 的 skills 目录直接放技能目录链接，不放总根目录→suite→skills 的多层结构让 OMP 递归扫描。多 suite 可向 OMP customDirectories 传多个目录，Pi 则重复 --skill 并配 --no-skills。

此方案解决所选技能的目录呈现，不自动排除 OMP 的默认来源；隔离结果见最新加载契约。不得把技能目录方案等同于切换整个 agent 配置目录。未执行真实软链接验收（本仓要求显式 HERDR_BOT_MANUAL_SYMLINKS=1）。用户已确认由系统根据 suite 配置生成链接，不再将人工维护链接目录视为另一套选择来源。

## 最新加载契约核查

详见 [loading-contracts.md](loading-contracts.md) 与 [spec.md](spec.md)。关键更新：Pi 实际 CLI 目录解析需要显式 manifest，已用临时样例运行验证；Pi MCP 可通过公开 createMcpExtension 自定义 loadConfig 复用原生实现，不能再把它归为无解接口缺口。OMP 可用全局 disabledProviders 限制 skills/MCP，但存在跨资源副作用和用户 MCP 名单影响；OMP prompts 公开资源事件未接线，严格目录注入仍未成立。Pi 跳过 LSP 已获用户明确批准。

## Suite 物理目录扩展提案

用户提出 suite 具有物理目录，skills/extensions 通过生成软链接表示实际组件。组织形式可统一，但不能假设一个原生目录参数覆盖全部资源。OMP 扩展目录支持一级文件/包入口及链接（docs/extension-loading.md:210-237）；Pi 同样有一级扩展目录与包入口解析（core/extensions/loader.ts:740-823），CLI 临时来源还经过 package-manager，具体生成布局应以该入口验收。

MCP/LSP 是配置声明，不是可执行扩展目录：OMP 的 extension package 根可发现 mcp.json/.mcp.json（discovery/omp-plugins.ts:276-318），LSP 可来自 plugin roots 的 lsp 配置；两者都不能据此保证排除其他来源或默认服务器。Pi 内置 MCP 固定读取 agentDir/mcp.json 和可信项目配置，并非任意 suite 目录；公开 registerMcpServer 可以从扩展注册，但文件同名配置优先且其他默认服务器仍可能连接（docs/mcp.md 的 Extensions and SDK）。Pi 核心无原生 LSP，需另选实际扩展实现。物理目录可统一，MCP/LSP 的加载与隔离入口仍待核实，不把软链接本身当作新加载接口。

## 尚待确定

资源登记最小范围、配置参数引用的精确消费规则，以及每个 CLI 缺口的具体解决方式。保留项目上下文的精确边界仍需在加载策略中明确。恢复使用当前配置及保留固定扩展已确认，不再作为开放问题。
