# Contract — scheduler node 与共享 node_template 管理

Source: [spec](spec.md)
Approval: approved（seam 选择、约束检视及验收模式三关均获用户明确确认）
Acceptance mode: Rapid iteration

## Promises

Coverage 中的“自动”由 agent 负责实现、TDD 及验收。预期结果取自承诺、预先确定的字面量或独立演算，不复用 implementation 计算预期。S1 使用实际交付页面和真实运行的应用；S2 使用页面实际调用的生产 HTTP interface。真实隔离文件、重读结果与现有执行记录是附属证据，不构成第三个 seam。普通运行遵循项目的测试入口、构建、网络护栏及资源隔离约束。

|#|Promise (one observable result)|Coverage|Source|Seam|
|---|---|---|---|---|
|P1|scheduler 的刷新全部旁提供进入同一 Tab 内模板列表的入口。|自动：实际页面进入列表，不产生新浏览器标签；空列表仍可进入新增。|stories 1–3|S1|
|P2|模板预览展示所选模板的可读能力定义。|自动：用预先指定的 ID、标题、适用场景、能力说明、命名输入输出和 chat 类型核对实际预览；区分预览与 JSON 编辑。|story 5|S1|
|P3|node 的模板明细直达其引用模板的同一套预览。|自动：多个 node 引用不同及相同模板，点击后验证准确身份；不再出现旧内联明细。|story 6; Implementation Decisions|S1|
|P4|模板往返保留仍存在的原 Map 与 node 选择并呈现最新模板内容。|自动：从 node 进入、修改模板、返回，核对选择与新内容；来源对象已被移除时明确失效，不展示旧成功数据。|stories 8–9; Implementation Decisions|S1|
|P5|空目录、读取失败、缺失模板及身份歧义均得到可区分的诊断。|自动：真实空目录、读取故障、缺失引用、重复 ID 分别核对结果；其他健康对象仍可查看，不能任选歧义模板作正常预览。|stories 4, 7; inherited template behavior|S1, S2|
|P6|node 与模板的新增及编辑统一分离身份与内容。|自动：新增 ID 独立输入且初始为空，编辑 ID 只读，内容 JSON 不含顶层 id；两类对象均实际保存并重读。|stories 10–13, 17|S1, S2|
|P7|顶层内容中的 id 被拒绝而不是覆盖独立身份。|自动：页面和直接请求均拒绝顶层 id；parameters 内名为 id 的合法参数可保存，避免误伤开放内容。|story 14; Implementation Decisions|S1, S2|
|P8|新增草稿使用约定文档示例内容。|自动：node 为上海出行天气 A 的内容、空 blocked_by 与 destination-weather 引用；模板为天气能力示例去掉 id。核对独立确定的业务值，实际删改后保存；在无开发仓库文档可访问的部署环境中仍可使用。|stories 15–19; Implementation Decisions|S1|
|P9|打开或放弃新增草稿不持久化示例及其依赖。|自动：操作前后检查隔离定义文件集合与内容；保留原有缺失配置目录初始化行为，不安装样例。|story 20; Implementation Decisions|S1|
|P10|编辑已有对象只使用其真实内容。|自动：准备与示例显著不同的合法对象，进入编辑并核对无示例混入，保存后重读。|story 21|S1, S2|
|P11|新模板获得安全且不覆盖现存文件的存储文件名。|自动：已有文件作为哨兵，创建模板后检查新增文件和原文件内容；含路径语义的 ID 不得被当成本地路径，页面不要求文件名。|story 22; Implementation Decisions|S1, S2|
|P12|修改已有模板保留其文件名与 ID 关联。|自动：文件名与 ID 不同的模板更新后重读，确认原文件位置和身份不变。|story 23; Implementation Decisions|S2|
|P13|创建操作拒绝所属身份空间中的重复 ID。|自动：同一 Map 重复 node ID、跨 Map 可复用 node ID、全局重复模板 ID及并发同 ID 创建；失败不覆盖原数据。|story 24; Implementation Decisions|S2|
|P14|保存拒绝不符合既有 JSON 和结构规则的内容。|自动：语法、版本、必需字段、类型、未知字段与未知实现类别的独立反例；检查明确错误及定义文件不被无效输入改写。合法空值沿用原规则。|stories 18, 25|S1, S2|
|P15|node 保存后的完整 Map 满足既有前置依赖规则。|自动：未知前置、自依赖、环均被拒绝；合法并行、汇合和独立节点保存后关系与预先指定结果一致。|story 26|S2|
|P16|管理功能不收紧现有开放参数及缺失模板规则。|自动：包含未声明参数、嵌套 JSON、未登记 zone／Bot 和不存在模板引用的合法 node 可保存；缺失模板仍明确提示，不补默认值。|story 27; Implementation Decisions|S1, S2|
|P17|保存失败时编辑器保留用户输入并显示错误。|自动：校验错误、受控写入失败及冲突后核对独立 ID 与原始草稿仍在；页面不得显示成功或跳转到成功预览。|stories 28, 30|S1|
|P18|保存拒绝覆盖打开编辑后已发生的目标文件变化。|自动：两个客户端编辑同一模板；编辑 node A 期间修改同 Map 的 B；外部编辑器在提交前修改或移除目标文件。提交失败、较新文件不被旧草稿覆盖，页面保留草稿并提示重新加载。外部并发范围见已批准限定。|stories 29–30; boundary review: inferred, required|S1, S2|
|P19|离开有修改的编辑器须确认放弃。|自动：返回、切换页面、应用刷新和浏览器刷新分别观察确认；取消保留草稿，确认后执行原动作；浏览器原生提示只验证确认行为而非固定文案。|story 31; inferred, required|S1|
|P20|被当前 Map 内其他 node 依赖的 node 不可删除。|自动：B 依赖 A 时拒绝删除 A，并指出 B；目标与依赖均未被删改。|stories 32, 34|S1, S2|
|P21|被任意 Map 引用的模板不可删除。|自动：至少两张 Map 引用同一模板，拒绝删除并列出引用位置；不自动删 node、删引用或创建副本。|stories 33–34|S1, S2|
|P22|不能完成引用检查时删除被阻止。|自动：损坏 Map、不可读 Map 及目录读取失败分别验证，显示阻塞检查的文件或目录原因；目标保留，不能当成无引用。|story 35; Implementation Decisions|S1, S2|
|P23|无引用对象也仅在用户最终确认后才删除。|自动：取消确认保留文件与对象；确认后检查实际删除和重读结果。|story 36|S1, S2|
|P24|删除不能使用过期检查结论或歧义目标。|自动：确认框打开后目标变更、新增引用、模板 ID 变为歧义等场景，最终请求拒绝不再安全的删除，不随机选择文件。|Implementation Decisions; boundary review: inferred, required|S1, S2|
|P25|本应用内竞争提交不能穿透冲突或引用检查。|自动：实际并发请求验证同目标更新不会静默双重覆盖；删除模板与新增对它的引用竞争时，提交结果遵循明确先后关系，删除检查到已提交引用必须拒绝。不能把已批准允许的“删除后再创建缺失模板引用”误判为违约。|boundary review: inferred, required; approved concurrency clarification|S2|
|P26|node 新增或修改成功后即时展示被选中的保存结果。|自动：不点击刷新全部，核对当前 Map、node 选择、列表、详情及已展示依赖图。|stories 37, 41|S1|
|P27|node 删除成功后留在当前 Map 并清除被删除节点选择。|自动：删除非被引用节点及最后一个节点，核对详情消失、列表和空态正确，不残留旧图节点。|story 38; inferred last-node symmetry, required|S1|
|P28|模板新增或修改成功后进入其最新预览。|自动：不手动刷新即核对保存后的业务内容与 ID。|stories 39, 41|S1|
|P29|模板删除成功后返回更新后的模板列表。|自动：删除普通模板及最后一个模板，核对列表和空态，不继续展示已删除预览。|story 40; inferred last-template symmetry, required|S1|
|P30|共享模板修改对所有引用者生效且不产生模板副本。|自动：同 Map 多 node、不同 Map 引用同 ID，修改后重新读取均看到新能力；检查存储未产生每 Map 副本。|story 42|S1, S2|
|P31|node 写入保留当前 Map 的其他 node 和元信息。|自动：以独立预期核对其他 node 的嵌套参数、依赖和 Map 标题说明；其他 Map 文件保持原样，不要求被更新 Map 的 JSON 排版逐字节不变。|story 43; Implementation Decisions|S2|
|P32|成功报告对应真实持久化结果。|自动：新增、修改、删除后从实际文件及重新读取核对；代表性成功序列经进程重启后仍成立，不只检查内存或页面提示。|stories 37–43; inferred, required|S2|
|P33|写入能力不开放任意本地文件访问。|自动：直接请求中的越界路径、路径分隔符和符号链接／重解析点场景不能使目录外哨兵被读取或改写；沿用既有配置目录约束。|Implementation Decisions; inferred write/read symmetry, required|S2|
|P34|定义内容始终作为数据处理，不执行工作。|自动：预览、编辑、保存、删除、错误恢复均结合实际执行记录核对不启动 Chat、不执行固定操作；恶意 HTML／脚本和路径参数不执行、不读取对应文件。|story 44; inherited safety guarantees|S1, S2|
|P35|浏览与预览不会改写已有定义。|自动：在正常、缺失、损坏与恢复场景中比对文件字节；明确保存／删除以外不自动修复或安装样例，原有目录初始化除外。|stories 20, 44; Implementation Decisions|S1, S2|
|P36|桌面与窄屏的编辑、诊断和模板往返体验可操作且易于核对。|自动：agent 完成实际页面操作、键盘访问、窄屏编辑与错误恢复检查。人工保留：用户判断阅读舒适度、编辑是否顺手及提示是否易懂。agent 在全部自动工作结束后提供可运行页面、隔离示例数据、桌面与窄屏截图、操作步骤和已知限制；修正后复查。用户确认前人工部分未验收。|stories 5, 8, 12, 28, 31; approved Rapid iteration exception|S1|

Complete implementation and all assigned automated checks before the human-led acceptance round. Planned human-led checks do not pause otherwise executable work. At the final handoff, report the remaining checks together with their prepared evidence; they remain unverified until the user confirms them. Report genuine blockers when encountered.

交付关联由 /to-tickets 为 P 编号补充，不在此预先切片。完成状态由交付工作与所需检查结果派生，不维护手写状态列；P36 包括用户确认。

## Seam decisions

|Seam|Exposes|Hides|Alternatives considered|Reversibility|
|---|---|---|---|---|
|S1：实际交付页面的用户操作 interface|真实导航、列表与预览、编辑、草稿保护、确认和反馈；通过实际运行应用观察结果。|页面内部 module 组织、编辑复用方式、状态管理和展示实现。|只测内部 module 无法证明真实交互；不作为本 contract 的替代。用户批准复用现有页面 seam。|内部重构自由；用户可观察行为改变需先修订承诺。|
|S2：页面实际使用的 scheduler HTTP interface|实际增删改请求的持久化、校验、冲突、引用检查和访问限制；真实隔离文件及执行记录提供副作用证据。|具体冲突标识、同步机制、文件名生成、内部 module 拆分；不固定尚未选择的路由形式。|只用 S1 无法独立排除绕过页面、仅更新内存或文件副作用违规；新增测试专用 interface 没有必要。用户批准扩展既有生产 interface。|内部机制可替换；持久化格式、ID 语义和生产调用约定变化需迁移调用方并重验 contract。|

- interface is the test surface：调用方与验收穿过同一 seam，不把私有 helper 或内部 module 当作 contract 承诺。
- 现有只读 interface 增加显式写入能力，不为每个操作另建 seam。不以“一份生产 adapter”包装没有真实变化需求的假想 seam。
- 依赖分类：校验和页面状态属于 In-process；文件系统属于 Local-substitutable，本 contract 选真实隔离目录而非内存文件系统；浏览器到自有后端属于 Remote but owned，验收复用真实传输而不需要新增测试 port；既有外部执行 adapter 的记录能力只用于验证不执行，不触发真实第三方动作。
- 不采用只有模拟响应的页面验证代替真实持久化结果；内部测试和 adapter 选择保持 implementation 自由，但不能取代约定证据。

### 已批准的并发限定与取舍

- 同一应用实例内，所有经生产 interface 的写入必须协调提交，保护冲突检查、引用检查与实际变更。node 保存以所属 Map 文件为冲突单位，不自动合并其他 node 的并发修改。
- 提交前重新检查外部文件变化；已观察到的目标变化、新引用或身份歧义必须阻止不安全操作。
- 用户明确同意：不参与协调的任意外部编辑器，可能在最后检查与文件修改之间再次写入；不承诺与其具有跨文件原子性。使用说明须提示避免在页面提交期间直接修改相关定义。
- 保留直接文件维护，不采用“所有写入只能通过受控 interface”的替代方案，因为后者改变已确认使用方式。
- 未来成本：若要求任意外部编辑与页面提交竞争时仍具备严格原子保证，需要重新审视存储和写入所有权，并修订 contract；不能靠补测宣称现有保证成立。本次不引入数据库迁移或外部编辑器协议。
- 缺失模板引用仍允许保存，因此“任何时候都不存在失效引用”不是承诺。承诺是删除不能绕过它提交时应观察的引用检查，不能暗中改变现有缺失模板规则。

## Not yet specified

- 无阻塞批准的决策。
- 路由名称、请求编码、冲突标识、锁或同步策略、安全文件名算法、示例打包方式及内部 module 划分由 implementation 决定，不是遗漏的产品承诺，不预先切片。
- 没有已承诺但延后的新 seam。若 implementation 证明所选 seam 无法承载某项证据，先修订 contract，再重开受影响票据，不静默降低覆盖。
- 不承诺任意外部编辑器之间的跨文件原子性；这属于已批准范围限定，不是待完成工作。

## Out of scope

继承 [spec 的 Out of Scope](spec.md#out-of-scope)，不另行扩大排除项。已批准的并发保证范围见上节及 spec 的同步澄清。

## Approval and coverage review

- Seam selection：用户“批准”S1 与 S2，真实隔离文件和执行记录作为附属证据，不增加内部测试专用 seam。
- Boundary review：用户“同意”本应用内提交协调与外部编辑器保证限定；服务端最终检查、Map 级冲突与身份歧义拒绝作为既有承诺的必要含义纳入。
- Acceptance mode：用户选择 Rapid iteration；唯一人工保留项为 P36 的主观体验判断，其实现与所有有用自动检查仍归 agent。
- 每个 promise 至少映射一个 seam；例外：无。每个 seam 均承载 promise；例外：无。
- Entry：S1 页面操作与 S2 直接请求均覆盖，防止页面校验被绕过；没有新增 CLI 或公共 library interface。
- Data and state：格式、ID、文件持久化、示例、草稿与选择状态已覆盖。
- External dependencies：真实隔离文件、外部编辑及现有执行记录已覆盖；没有需要成功调用的第三方功能。时间与随机值仅为内部机制时不形成独立承诺。
- Errors and failure：读取、解析、校验、写入、身份歧义、冲突及不完整引用检查均覆盖；无自动重试或自动修复承诺。
- Output and observability：页面诊断、请求结果、文件和执行记录共同区分违规；不新增日志、指标或独立报告格式。
- Resources and concurrency：测试拥有独立目录、进程和回环端口；同实例竞争请求、确认后变化与外部编辑范围已覆盖。未承诺独立性能时限、容量指标或分布式多实例一致性，不凭此新增产品能力。
- 本文是已批准的决策 artifact，不是已通过验收的报告；P1–P36 的交付与检查尚待后续工作。
