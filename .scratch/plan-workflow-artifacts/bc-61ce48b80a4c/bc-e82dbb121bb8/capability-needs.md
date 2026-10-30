# 能力建设需求

## 1. 方案标识、修订与确认范围

- 方案：create-bot-task-workflow--bc-61ce48b80a4c；本文 r1，首次交付。
- 规划 Run：bc-61ce48b80a4c；outline 执行：bc-e82dbb121bb8。
- 用户已在本节点 Chat 确认 Q1–Q10 的业务决策：创建 Task 与 Bot 时通过 grilling 确认文本意图；优化自动保存；优化使用通用单 Skill writing-for-agents，专用操作信息放在 workflow 交接材料，不固化进优化 Bot/Task。三个能力由用户人工补齐。
- 本文是能力建设交接，不是已完成配置、业务验证或最终概要方案确认。关联缺口 G1、G2、G3，均待补齐。
- 接收者无需读取其他文档或本次聊天即可按本文建设。用户手动发起建设 Chat，本规划节点不调用创建者。

## 2. 各项能力目标、输入输出与完成条件

### G1：Task 创建者

职责：根据本次显式输入，通过 grilling 澄清需求并起草 Task.prompt，向用户核对意图是否清晰准确；达成明确确认后，通过服务创建 normal Task 并读回。确认是语义确认，不是重复机械审批。仅创建 Task，不提前创建 Bot。

输入：目标能力的需求原文或可读绝对路径、输入输出、资源及权限、模型等约束、材料交付位置。信息不足在当前 Chat 询问；把新增回答和已确认结论写入交接材料。创建前确认全部执行配置；所创建 Task 的模型由本次需求决定，不强制沿用创建者模型。

输出：真实 Task 名称及 ID、完整读回配置、已确认意图与硬约束、已知资源和缺项、保存结果及未验证项。完成条件：用户已确认意图，服务读回与本次确认一致，后续 Chat 不依赖完整聊天即可继续工作。

非目标：不创建 Bot、不优化其他既有 Task、不运行新 Task、不建设缺失业务代码或凭据、不承担调度。

### G2：通用单 Skill 提示词优化者

职责：在新的独立 Chat 中，使用 /writing-for-agents 自动优化显式提供的文本，保留已确认的职责、权限、输入输出和验收语义；按本次输入提供的交付要求输出或保存结果。无需再次 grilling 或逐次人工审批。存在无法保持原意解决的矛盾时，报告阻塞而不是自行决定新业务意图。

Bot 与 Task 必须保持通用：不得固化 herdr-bot 服务地址、Task/Bot 字段、配置 API、四阶段流程或本方案路径。Task 为单 Skill 入口，固定 prompt 不塞入业务专用流程；Bot 指南只描述通用选用条件、必要输入和交付约定。具体对象身份、原文、已确认意图、允许改动范围、接口文档、保存要求与材料地址由使用者的 workflow input/交接材料提供。

输入：待优化文本、已确认意图及必须保留的约束、交付要求；需要直接保存时，由调用方同时给出目标身份、允许字段、已授权操作与所需接口/资源。独立新 Chat 不读取完整 grilling 历史、放弃方案或其他优化 Chat 历史；必要角色和工具指示不视为应清除的上下文。

输出：优化文本、交付或保存结果、读回核对结果（本次要求保存时）、未解决矛盾与未验证项。完成条件：实际使用 writing-for-agents，保留已确认语义，完成本次明确交付要求；不得将仅生成文本冒充已保存。

必须解决的调用条件：当前服务 options.skills 只有 grill-with-docs、teach、code-review、wayfinder，没有 writing-for-agents。本规划 Chat 已能通过 skill://writing-for-agents 读取 Skill，绝对位置为 C:/Users/hugos/.omp/agent/skills/writing-for-agents/SKILL.md；这不证明未来执行宿主的 /writing-for-agents 调用已生效。建设者应核实合法的单 Skill 调用方式及宿主发现规则，明确交付实际配置；不可提交非法 skill 值或用复制 Skill 正文冒充调用。若需修改共享选项目录、安装或调整宿主配置，先向用户取得专项授权。仅配置读回不能标为调用或业务试运行通过。

### G3：Bot 创建者

职责：读取已交付 Task 的真实 ID、最新固定指示与已确认需求，通过 grilling 起草并与用户确认 when、plan_guide、node_guide 的意图；确认后通过服务创建 Bot 并绑定给定的 normal Task，读回核对。

输入：需求与澄清结论、目标 Task ID 和材料路径、资源及权限、交付位置。when 描述适用性；plan_guide 描述规划安排；node_guide 指导显式填写节点输入。三份指南不是执行提示词，不承诺其自动注入 Chat。能力描述须符合给定 Task 的真实职责。

输出：真实 Bot 名称及 ID、绑定 Task ID、三份指南的确认文本与意图约束、完整读回定义、保存结果及缺项。完成条件：用户确认意图、Bot 读回一致、绑定经 Task 详情核实；不存在未处理的职责冲突。

非目标：不另建或修改目标 Task、不运行它、不将工具或权限缺口写成已具备、不承担调度。

## 3. 已调查 Bot 与复用不足

2026-10-30 通过 http://127.0.0.1:18787 的 GET /api/scheduler/bots、GET /api/tasks 与创建者 Task 详情再次核实：

- create-bot-task（能力创建者 · Bot＋Task）绑定 task-18e315be1cf0ef10-0，normal / omp / gpt-6-astra / medium，初始 Skill 为 grill-with-docs。可作为用户人工建设入口；其现有职责是在同一个 Chat 创建 Bot＋Task，不能原样充当仅创建 Task 或仅创建 Bot 的独立能力。
- outline-planning 和 plan-to-map 分别处理规划与 Map 翻译，不承担上述三项业务；其余目录项为临时文件排查和额度观察，不适用。
- 当前完整 Bot 目录未发现符合 G1/G2/G3 的定义；响应未返回局部目录错误。
- 已有名为 grill-with-docs 的普通 Task，但仅 Skill 入口不足以证明具备 G1/G3 的配置职责和交付契约。复用须核对职责，不按名称认定能力。

## 4. 缺口分类、资源与证据

| 缺口 | 当前分类与状态 | 已有条件 | 尚缺条件 |
| --- | --- | --- | --- |
| G1 | 配置缺口，待补齐；业务执行未验证 | Task 创建接口的公开契约、已有创建者配置、可读产品文档 | 独立 normal Task 与 Bot、确认和交接行为的固定职责 |
| G2 | 配置缺口并有未验证的 Skill 调用条件，待补齐 | 本机可读 writing-for-agents Skill；OMP 可选 | 通用 normal Task/Bot、合法调用配置；若宿主实际无法调用则为真实能力阻塞，不能靠文案解决 |
| G3 | 配置缺口，待补齐；业务执行未验证 | Bot 保存接口契约、Task 查询、已有创建者配置 | 独立 normal Task 与 Bot、给定 Task 绑定与语义确认职责 |

产品仓库及已登记业务 zone：E:/Developer/myhub/herdr-bot。服务：http://127.0.0.1:18787；其他 zone 不改变配置服务位置。

配置前读 docs/usage.md 的 Task 类型/执行配置规则、meta-data/task-options.toml 的合法选项；Bot 指南与保存契约见 docs/scheduler.md。创建 Task 使用 POST /api/tasks 的真实响应 ID；Bot 使用文档中的 save 契约（id 与 content 分离，新增 revision=null），不直接改 .data。详情通过 GET /api/tasks/{id} 与 GET /api/scheduler/bots 核实。接口文档与实际服务有差异时报告，不猜测保存成功。

现有资料证明配置路径存在，不证明新能力运行有效；本次没有启动 Chat、Run 或进行业务试验。

## 5. 允许操作与需另行授权事项

用户将本文带入人工建设 Chat 后，在该 Chat 的实际授权内创建三个 normal Task 及对应 Bot，读取真实选项、保存新配置并读回。执行者采用用户接受的 omp / gpt-6-astra / medium；G1/G3 使用 grilling，G2 使用 writing-for-agents。允许必要的纯配置调查；不把选择模型等同于验证宿主运行效果。

保持既有 Task/Bot/Map、当前运行记录和执行者配置不变；名称冲突、需复用并修改既有对象时先请求授权。不安装扩展、变更共享 Skill 选项、写凭据、扩大权限、试运行新 Task 或正式业务，除非用户在建设 Chat 另行明确授权。不自动派发、不启动 Run、不停服/关机/删除资源。

Task 创建成功而 Bot 保存失败时，保留真实 Task ID 和错误，先核实部分结果，不重复创建、不擅自删除或声称整体回滚。

## 6. 返回契约

请给用户一份可直接带回规划 Chat 的交接正文或可读绝对路径，逐项列出 G1/G2/G3：

1. 状态：已补齐、待验证或阻塞；真实 Bot/Task 名称、ID 和绑定。
2. Task 的 normal 类型、agent/model/thinking/skill/extensions、固定 prompt；Bot 三份指南和实际使用条件。
3. 配置保存及 GET 读回的真实结果；如复用，注明复用理由与是否修改。
4. G2 实际调用 writing-for-agents 的配置方式、可读资源位置、宿主支持依据；清楚区分静态核实与实际调用观察。保持通用能力，不加入本方案专用逻辑。
5. 已验证项、未验证项、权限或环境阻塞、部分失败及已产生的对象 ID；如做试验，说明专项授权、范围、结果和限制。

规划者收到结果后将重新查询服务，不仅凭返回文字认定补齐。无需建设整体 Map，也无需执行本方案正式业务。
