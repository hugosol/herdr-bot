# Contract — Suite 场景工具集与 Task 会话环境配置

Source: [Suite 规格](spec.md)
Acceptance mode: Full automation
Approval: approved — 接缝选择、约束审视和验收模式均已获用户明确确认。

## Promises

Coverage 全部由 agent 承担，包括实现后的 TDD、自动检查和约定的真实依赖验收；人工验收例外为 **none**。本轮批准契约不代表任何产品承诺已交付。

期望来自规格、独立字面样例或独立计算，不从被测实现重算。真实依赖检查缺权限、缺程序或缺证据时报告阻塞与未验证，不以替身、跳过、日志非空或零退出码替代通过。先完成所有仍可执行的工作，不擅自降低承诺。

Delivering work 由 /to-tickets 后续按 P 编号绑定；此处不维护完成状态列。完成状态从实施票完成情况及所有必需检查结果推导。

| # | Promise (one observable result) | Coverage | Source | Seam |
| --- | --- | --- | --- | --- |
| P1 | Task 只接受一个兼容 suite 或不选 | 自动：直接请求与浏览器选择的独立有效/无效样例。 | [story 1, 2, 3, 18, 19](spec.md#user-stories) | S1, S2 |
| P2 | 多个 Task 可引用同一 suite | 自动：不同稳定 Task ID 读取同一引用。 | [story 4](spec.md#user-stories) | S2 |
| P3 | Suite 图形管理可完成增删改查 | 自动：真实浏览器操作及再次读取。 | [story 5](spec.md#user-stories) | S1 |
| P4 | 界面保存与文件编辑使用同一权威定义 | 自动：外部独立文件内容、重读与重启。 | [story 6](spec.md#user-stories) | S1, S2, S3 |
| P5 | 资源声明复用已有实体而非通用注册平台 | 自动：独立声明与真实资源消费。 | [story 7](spec.md#user-stories) | S2, S3, S4 |
| P6 | 集中根引用及声明相对路径不依赖 Chat cwd | 自动：含空格及不同 cwd 的实际启动。 | [story 8, 48](spec.md#user-stories) | S3, S4 |
| P7 | 指定生成位置包含可消费的装配产物 | 自动：同一声明在两宿主实际加载。 | [story 9, 10, 20](spec.md#user-stories) | S3, S4 |
| P8 | Skill 配套脚本和资产可用 | 自动：实际读取/执行受控资产。 | [story 11](spec.md#user-stories) | S4 |
| P9 | Extension 相对依赖与资产可用 | 自动：真实工厂及工具调用。 | [story 12](spec.md#user-stories) | S4 |
| P10 | Suite 技能集合不被常规默认发现或新增源技能扩大 | 自动：已选/未选哨兵的独立预期集合。 同一次启动必须保留原生 MCP，不能用关闭 MCP 来源取得技能过滤结果。 | [story 13, 15](spec.md#user-stories) | S4 |
| P11 | 首批依赖技能按批准清单可加载 | 自动：独立18/11清单与宿主发现。 | [story 14](spec.md#user-stories) | S3, S4 |
| P12 | 固定扩展不由 suite 可选声明取消 | 自动：有/无suite及固定扩展实际行为。 | [story 16](spec.md#user-stories) | S2, S4 |
| P13 | 执行角色与恢复关联保持既有规则 | 自动：独立Chat、manager、node及恢复对照。 | [story 17](spec.md#user-stories) | S2, S4 |
| P14 | Pi 跳过 LSP 不阻止适用资源 | 自动：不启动LSP且其他资源实际使用。 | [story 21](spec.md#user-stories) | S4 |
| P15 | OMP 跳过 suite 本地模板而不改写其语义 | 自动：模板哨兵与实际原生指令观察。 | [story 22](spec.md#user-stories) | S4 |
| P16 | Pi 本地模板仅按调用展开 | 自动：初始化及显式调用前后对照。 | [story 23](spec.md#user-stories) | S4 |
| P17 | 集中 MCP 定义由 suite 引用而非每项目复制 | 自动：空项目使用集中定义并核对项目文件未写入。 | [story 24, 25](spec.md#user-stories) | S1, S2, S3 |
| P18 | 失效 MCP 引用在读取显示及保存启动检查中明确反馈 | 自动：外部删除后直接保存/启动失败样例。 | [story 26, 27](spec.md#user-stories) | S1, S2 |
| P19 | Suite MCP 增量保留原生来源 | 自动：真实服务器启动/调用及来源。 Pi 保留 builtin MCP，验证追加注册及文件同名覆盖；不能沿用旧隔离原型预期。 | [story 28, 32](spec.md#user-stories) | S4 |
| P20 | MCP 去重、覆盖及禁用遵循各宿主原生规则 | 自动：独立定义胜者、连接次数与实际文件结果。 包含同名覆盖、异名等价、原生禁用；原生覆盖/禁用不属于集中引用失效，不由产品强制重启服务器。 | [story 29](spec.md#user-stories) | S4 |
| P21 | 空 MCP 清单不关闭原生 MCP | 自动：空清单下已有服务器实际调用。 | [story 30](spec.md#user-stories) | S4 |
| P22 | OMP suite MCP 保留完整原生能力 | 自动：工具、管理交互、资源、prompts、通知及instructions分别观察。 | [story 31](spec.md#user-stories) | S4 |
| P23 | pi-suite 作为固定扩展加载 | 自动：实际固定扩展加载与受控配置消费。 | [story 33](spec.md#user-stories) | S4 |
| P24 | 无 suite 时 pi-suite 不改变 MCP/codemode | 自动：有适配扩展与原生基线对照。 | [story 34](spec.md#user-stories) | S4 |
| P25 | Pi 模式控件保存明确 on/only 且不出现在OMP-only编辑中 | 自动：真实控件操作、保存重读及宿主切换。 | [story 35, 38](spec.md#user-stories) | S1, S2 |
| P26 | Suite 模式双向覆盖原模式 | 自动：模型收到的工具声明与模式交叉样例。 | [story 36](spec.md#user-stories) | S4 |
| P27 | only 模式底层工具仍可调用 | 自动：通过codemode读取真实文件并独立核验。 | [story 37](spec.md#user-stories) | S4 |
| P28 | 认证状态不因 suite 改址复制或分裂协调 | 自动：隔离认证存储及并发运行证据，不用用户凭据。 使用测试拥有的认证数据；不得读取真实凭据或在日志中泄露凭据。 | [story 39](spec.md#user-stories) | S3, S4 |
| P29 | 模型配置与原生历史位置身份不因 suite 改变 | 自动：隔离部署前后原生读取。 | [story 40](spec.md#user-stories) | S3, S4 |
| P30 | Task trigger config 与命名行为不被 suite 替换 | 自动：独立字面初始指示与原生历史。 | [story 41](spec.md#user-stories) | S2, S4 |
| P31 | 恢复读取来源 Task 最新环境且不重放trigger | 自动：修改定义、重启、恢复原身份与消息核对。 同时覆盖无来源/已删除来源 Task 的既有恢复行为，不猜测 suite。 | [story 42](spec.md#user-stories) | S2, S4 |
| P32 | 保存 suite 不主动热重配已有 Chat | 自动：已有Chat与后续启动的对照事件。 不扩展为禁用宿主原生重载或冻结第三方资源内容的承诺。 | [story 43](spec.md#user-stories) | S2, S4 |
| P33 | 被引用 suite 删除被拒绝并列出引用Task | 自动：直接删除及页面反馈，引用保持。 | [story 44, 45](spec.md#user-stories) | S1, S2 |
| P34 | 装配清理不删除真实资源目标 | 自动：独立目标内容与链接实体前后对照。 目标文件独立哈希及未拥有资源不变；受控生成不得放宽 zone 和通用持久化的任意链接拒绝。 | [story 46](spec.md#user-stories) | S3 |
| P35 | 并发启动不观察半套资源 | 自动：独立两套资源哨兵及并发真实启动。 覆盖同suite与不同suite；文件协调不得跨越 Herdr 外部效果，不承诺跨文件事务或多进程写入协议。 | [story 47](spec.md#user-stories) | S2, S3, S4 |
| P36 | 链接失败明确反馈且不自动提权或复制替代 | 自动：可控权限/路径失败与副作用证据。 Windows 文件/目录真符号链接及跨磁盘必须有真实证据，junction 不能代替；显式手动开关不等于自动提权。 | [story 48](spec.md#user-stories) | S2, S3 |
| P37 | 四种声明语言服务按OMP原生条件可发现 | 自动：符合项目标记的实际原生语言服务启动。 使用已具备运行条件的隔离部署验证四种语言；缺少程序时报告阻塞，不自动安装或改系统 PATH。 | [story 49](spec.md#user-stories) | S4 |
| P38 | 缺少语言服务不被报告为已运行 | 自动：缺可执行程序的独立负例。 | [story 50](spec.md#user-stories) | S1, S4 |
| P39 | 22个Task按已批准稳定ID映射迁移 | 自动：独立17/1/4预期与重启读回。 对照获批稳定ID映射而非显示名；不改已存在的新编辑。 | [story 51](spec.md#user-stories) | S2, S3 |
| P40 | 迁移保留Task及其既有引用身份和trigger | 自动：独立迁移前后记录及历史关联。 | [story 52](spec.md#user-stories) | S2, S3 |
| P41 | 迁移重复执行不重复生成且部分失败明确 | 自动：中断失败、重启和独立文件证据。 | [story 53](spec.md#user-stories) | S3 |
| P42 | 实现suite保留hyper-auto-coding命令能力 | 自动：真实扩展注册命令可调用，不执行用户任务。 | [story 54](spec.md#user-stories) | S4 |
| P43 | Tibo-monitor本阶段不选suite且不导入web-access | 自动：批准映射与真实持久定义。 | [story 55](spec.md#user-stories) | S2, S3 |
| P44 | 交付不修改herdr/OMP/Pi或使用内部补丁 | 自动：交付变更清单及原生公开入口证据。 不以 SDK 重组或源码 CLI 局部成功替代实际交付入口；不得使用内部 monkey patch。 | [story 56](spec.md#user-stories) | S3, S4 |
| P45 | 并发的 Suite 删除与 Task 引用新增不能均成功并留下悬空引用 | 自动：受控并发请求；核对成功/拒绝结果及独立持久读回。外部编辑不纳入进程内互斥保证，重读后须检查。 | inferred：履行既有承诺所必需；[story 19, 44, 45](spec.md#user-stories)；约束审视获批 | S2 |
| P46 | 配置与装配失败阻止该次 Suite 启动；有效配置的 MCP 运行期连接状态交由宿主原生处理 | 自动：分别制造缺引用、不可读资源、生成失败及有效但不可达的 MCP；前者明确拒绝且不以空环境启动，后者遵循原生失败/等待状态，不新增全部就绪门禁，不将 Chat 已启动宣称为 MCP 已可用。 | 约束审视明确获批的失败语义；[story 26, 27, 28, 31, 32, 50](spec.md#user-stories) | S1, S2, S3, S4 |

## Seam decisions

| Seam | Exposes | Hides | Alternatives considered | Reversibility |
| --- | --- | --- | --- | --- |
| S1 — 主界面交互 interface | 用户实际可完成的 Suite 管理、Task 选择、宿主过滤、模式切换及错误反馈 | 页面组织、内部状态管理与渲染实现 | 仅 HTTP 检查不能证明用户能完成操作；既有组件级检查也不能替代真实浏览器。复用现有主界面方式，不额外增加测试专用公开入口。 | 页面布局可调整；用户动作、选择语义与错误承诺保持。 |
| S2 — 公开 HTTP interface | Task/Suite 保存与引用校验、删除保护、发起/恢复/workflow 的结果及并发可见行为 | 解析、业务协调、内部调用与存储实现 | 仅页面校验可被直接请求绕过；私有 helper 测试不能承担完整业务承诺。扩展既有公开 interface，不为新用例另造内部公开接缝。 | 内部实现易替换；外部请求语义变化须修订契约并迁移调用者，不保持隐式双轨。 |
| S3 — 部署配置与程序启动 interface | 权威配置、路径语义、迁移重启结果、生成产物可消费性、失败与资源所有权 | 文件操作协议、装配算法、协调机制和内部布局 | 内存替身无法证明真实重启、权限及链接行为；逐一暴露内部文件 helper 降低 depth。复用真实程序入口和独立磁盘证据。 | 实现可改；用户维护的配置语义及稳定身份变化需要显式迁移。生成产物可重建，不作为历史版本。 |
| S4 — 经 Herdr 启动后的原生 agent interface | 实际资源集合、工具声明/调用、MCP 全部原生能力、模式、模板、LSP 与原生 Chat 身份 | 宿主 adapter 的配置翻译、公开扩展注册接线、宿主内部模块 | argv 检查与受控 Herdr adapter 可检查控制流程，但不能证明真实宿主消费。独立工具 manager、整套 agentDir 替换与内部补丁均被上游否决，不能作为通过的替代。 | 宿主升级可能要求调整 adapter 和验收；用户层承诺不因此自动变化。不能维持承诺时先修订契约并取得必要批准。 |

### 已批准的决定与推论

- 接缝选择：用户明确批准 S1–S4。四者各自识别不同违约，不能仅用页面值、HTTP 成功或启动参数替代更深的外部效果；不据此要求四个内部 module。
- 约束审视：用户明确确认全部发现及 P46 的失败语义。未选“全 MCP 就绪才放行 Chat”：它会增加等待时限、部分连接成功处理和原生异步启动改变，不符合本次沿用原生机制的选择。
- 必要推论：P45 覆盖进程内删除/新引用竞争；外部文件编辑通过重新读取检查，不新增跨进程协议。受控链接装配不放宽既有任意重解析点规则；一致发布不等于历史快照，也不得持有文件协调锁跨外部启动。
- 原生 MCP 覆盖和禁用是合法结果，不视作缺失引用；空清单只是不追加。技能过滤与 MCP 增量必须组合成立，而不是分别验收后假定兼容。
- 验收模式：用户明确选择 Full automation。未选 Rapid iteration；本轮没有保留给用户确认的验收项，Windows 真链接及原生交互证据仍由 agent 负责取得。环境授权或缺依赖属于真实阻塞，不转为通过。
- 不新增独立 ADR：本次复用既有接缝及上游约束，未决定新的、难以逆转且有真实取舍的 module interface 架构；既有 ADR 仍按规格继承。

### 依赖分类与证据规则

- **In-process**：选择与校验计算通过上述 interface 观察，不为其增设公开测试接缝。
- **Local-substitutable**：文件系统一般行为用隔离真实临时目录观察；Windows 链接、权限和跨磁盘不能用内存替身证明。
- **True external**：herdr、OMP、Pi 及外部 MCP/LSP 对本功能均为不能随意修改的依赖。控制与失败场景可以使用注入的 mock adapter，但 S4 的约定验收必须实际运行目标宿主。测试拥有的 MCP、回环模型只证明协议与执行机制，不证明真实模型自主任务遵循。
- 本契约不新增 **Remote but owned** 的网络 module 或 port。受控依赖用于隔离验证，不成为额外产品能力。
- 所属进程、临时配置和动态端口必须独占；清理正常及失败路径均须确认所属进程退出，不操作用户现有 Chat、凭据、daemon 或真实关机。原始失败与清理失败分别保留。

## Not yet specified

- **阻塞契约批准的决定：无。** 四个接缝、失败语义和验收责任均已确认。
- **递延的契约项：无。** 资源路径存在不等于运行验收通过；缺少真实执行证据属于后续工作未完成，不是递延承诺。
- 内部 module 分割、装配算法和私有协调协议保持自由，不在本契约预切实施票。公开 interface 的具体字段与错误编码须遵循既有约定并承载全部承诺；若实现证明当前 seam 无法承载承诺，先修订契约并重新取得必要批准，再重开受影响实施票。

## Out of scope

完整继承 [规格的 Out of Scope](spec.md#out-of-scope)，不增加排除项；已批准的接缝和全自动验收不改变产品范围。

## Coverage report

- 每条承诺至少映射一个 seam：P1–P46 全部覆盖，例外 **none**。
- 每个 seam 至少承担一条承诺：S1–S4 全部有覆盖，例外 **none**。
- User Stories 1–56 全部进入承诺映射；P45 标记为必要 inferred，P46 记录明确获批的失败语义。
- 六面审视全部完成且适用：entry → S1/S2/S4；data and state → S2/S3/S4；external dependencies → S3/S4；errors and failure → S1/S2/S3/S4；output and observability → S1/S2/S3/S4；resources and concurrency → S2/S3/S4。不适用项 **none**。
- Not yet specified：递延契约项 **none**，批准阻塞 **none**；未执行的产品检查不得据此报告通过。
- Acceptance mode：**Full automation**；human-led coverage：**none**。

下一步：对本契约运行 `/to-tickets`。实施若推翻 seam 或承诺，应先修订契约，再重开受影响实施票。
