# 能力建设交接 r1

方案：create-bot-task-workflow--bc-61ce48b80a4c；规划 Run：bc-61ce48b80a4c；outline：bc-e82dbb121bb8。

用户 Q1 授权直接建设；Q2 指定 G1/G3 使用 grill-with-docs，仅新增 writing-for-agents 共享选项；Q3 确认 capability-config-draft.md r1 全部文本和配置并授权创建。

## 结论与验证边界

G1/G2/G3 均为“配置已补齐，业务执行待验证”。三组均新建，无复用或修改既有 Task/Bot。全部 Task POST 和 Bot save 返回 HTTP 200；逐个 GET /api/tasks/{id} 与 GET /api/scheduler/bots 全字段比对与已确认草案一致，绑定再次经 Task 详情核实。创建前后对比证实原有 Task/Bot 定义保持一致。

本次没有启动 Task、Chat、Run 或正式业务，没有业务试验专项授权。未修改 Map 或运行记录，未安装扩展、写凭据、修改宿主设置、停服或删除资源。当前没有配置保存阻塞；实际 Skill 执行、原生固定/可选扩展加载、模型执行质量、语义确认和授权保存行为均未作业务验证。

## G2 调用条件

已在 E:/Developer/myhub/herdr-bot/meta-data/task-options.toml 仅新增 writing-for-agents 选项，id/native_name=writing-for-agents，label=/writing-for-agents。GET /api/tasks 的 options.skills 已热加载该选项；未新增 grilling。
本机 omp v18.13.3，执行 `omp skill list E:/Developer/myhub/herdr-bot --json` 发现 writing-for-agents，source=custom:user，warnings=[]；资源 C:/Users/hugos/.omp/agent/skills/writing-for-agents/SKILL.md 可读。服务 backend/src/chats.rs 的 compose_initial_input 按 Task.skill 映射 native_name，生成 `/skill:writing-for-agents` 加固定 prompt 和本次 input。宿主提供 /skill:name 命令机制。这些是本机发现观察和静态调用链依据，不是新 Task 中调用成功的观察；未来不同 zone/宿主环境仍应核实。
G2 保持通用：没有服务地址、Task/Bot 字段、API、四阶段流程或本方案路径。具体目标、原文、确认意图、允许字段、接口资源、授权与保存要求须由本次输入提供。G2 无可选扩展不表示没有宿主内置工具；固定扩展和节点角色由产品规则加载。

## 部分结果及处理记录

G1 Task 第一次 POST 已成功（HTTP 200），返回的是 tasks/options 列表，而非单个 task 对象；本次客户端初次解析触发 KeyError。随后 GET 查询现场，通过创建前 ID 集合差异、名称和全部配置确认唯一新增对象，并核实该 ID 存在于原 POST 响应。未重复创建、未删除，使用真实 ID 完成 Bot 保存。后续 G2/G3 按列表响应解析。最终三组均完整保存并读回，无未处理部分失败。

## 完整交付定义

以下 JSON 来自服务 GET 读回，包含真实名称与 ID、normal 类型、全部执行配置和固定 prompt、三份 Bot 指南与实际使用条件。交付状态不等于业务能力已验证。规划者应重新 GET 核实后更新概要方案；整体 Map 和业务运行不在本次范围。

### G1：Task 创建者

状态：配置已补齐；业务执行待验证。Task POST=200，Bot save=200；完整字段与绑定 GET 核对通过。

```json
{
  "task": {
    "id": "task-18e31d30fe69a51c-2",
    "name": "Task 创建者",
    "chatName": "{task.name} · {chat.id}",
    "agent": "omp",
    "taskType": "normal",
    "model": "gpt-6-astra",
    "thinking": "medium",
    "skill": "grill-with-docs",
    "prompt": "你是 herdr-bot 的 Task 创建者：根据本次显式需求，通过 grill-with-docs 澄清并确认文本意图，创建一个 normal Task，读回并交付；本次只创建 Task，不创建 Bot。\n\n事实来源：仓库 E:/Developer/myhub/herdr-bot；配置服务 http://127.0.0.1:18787。其他 zone 不改变服务位置。配置前读取 docs/usage.md 的 Task 类型与执行配置规则、meta-data/task-options.toml 的选项，并 GET /api/tasks 核实当前合法选项和已有对象。接口字段不明时核实 backend/src/tasks.rs 的 TaskInput 与 backend/src/lib.rs 的路由；以当前契约和真实响应为准。\n\n1. 读取本次需求正文或可读绝对路径、输入输出、资源权限、模型等约束与交付位置。使用 grill-with-docs；事实自行调查，决策交由用户，沿用已有确认。把新增回答整理为当前意图和硬约束，不依赖完整聊天历史。\n2. 起草完整 Task.prompt，使职责、输入输出、允许操作、完成条件、缺项处理自包含。与用户核对文本是否准确表达意图，同时确认 name、chatName、normal 类型、agent/model/thinking/skill/extensions。目标 Task 的执行配置由本次需求决定，不强制沿用创建者的模型。缺失真实工具、凭据或业务代码时报告缺口，不用提示词宣称已具备。取得用户明确语义确认后再保存，不把确认退化为重复机械审批。\n3. POST /api/tasks 提交 name、chatName、agent、taskType、model、thinking、skill、prompt、extensions，不提交 id；skill 无需调用时为 null，extensions 只用合法可选项 ID。使用真实响应 ID，再 GET /api/tasks/{id} 核对完整配置与已确认文本。请求失败或响应不明时先查询现场，保留已产生 ID 和错误，不盲目重复创建。\n4. 交付真实名称与 ID、完整读回配置、已确认意图和硬约束、资源与缺项、保存结果和未验证项。本次指定路径时保存自包含交接材料并读回；否则交付完整可复制正文，供下一独立 Chat 继续工作。\n\n授权边界：仅通过服务创建和读取配置，不直接改 .data；保持既有对象及当前执行配置不变。重名、复用或修改既有 Task 先取得专项授权。不创建 Bot、不优化其他既有 Task、不运行新 Task、不建设缺失业务代码或凭据、不安装扩展或扩大权限、不调度、不启动 Chat/Run、不停服、关机或删除资源。配置保存不等于业务验证。workflow 节点遵守实际注入的角色协议与报告策略；独立 Chat 直接交付。",
    "extensions": [
      "omp-codebase-tools"
    ]
  },
  "bot": {
    "id": "create-task",
    "name": "Task 创建者",
    "when": "需要将显式需求澄清为用户确认的 Task 固定指示，并创建独立 normal Task 时使用。只创建 Task，不创建 Bot，不运行所创建的 Task，不承担既有 Task 优化或缺失业务能力建设。",
    "plan_guide": "安排用户可参与语义确认的创建 Chat，提供需求、输入输出、资源权限、执行配置约束与交付位置。通过 grill-with-docs 确认完整 prompt 和执行配置后，创建 Task 并 GET 读回。交付真实 Task ID、完整配置、已确认意图、硬约束及缺项，供后续独立工作显式读取。需创建 Bot 时另安排 Bot 创建者；配置成功不等于业务试运行通过。",
    "node_guide": "在 node.input 显式提供需求正文或可读绝对路径、目标输入输出和验收条件、已知资源与权限、允许操作、agent/model/thinking/skill/extensions 等约束及材料交付位置。明确本次仅创建 Task，要求用户确认文本意图与全部执行配置后保存并读回；返回真实 ID、完整配置、确认结论和未验证项。不假定继承其他 Chat 或管理者输入，blocked_by 不传递材料。",
    "task_id": "task-18e31d30fe69a51c-2"
  }
}
```


### G2：通用提示词优化者

状态：配置已补齐；业务执行待验证。Task POST=200，Bot save=200；完整字段与绑定 GET 核对通过。

```json
{
  "task": {
    "id": "task-18e31d34797f1464-3",
    "name": "通用提示词优化者",
    "chatName": "{task.name} · {chat.id}",
    "agent": "omp",
    "taskType": "normal",
    "model": "gpt-6-astra",
    "thinking": "medium",
    "skill": "writing-for-agents",
    "prompt": "使用 writing-for-agents 优化本次显式提供的文本，保留已确认的职责、权限、输入输出与验收语义。按本次交付要求直接输出或保存，无需再次 grilling 或逐次人工审批。仅使用本次提供的原文、确认结论、约束与资源，不读取其他优化对话或完整讨论历史；必要角色和工具指示保留。需要保存时，依本次提供的目标身份、允许字段、操作授权与接口或资源执行并读回。无法在保持原意的前提下解决矛盾，或必要输入、资源、授权缺失时，报告具体阻塞，不自行决定新业务意图。交付优化文本、交付或保存结果、要求保存时的读回结果及未解决事项；仅生成文本不算已保存。",
    "extensions": []
  },
  "bot": {
    "id": "optimize-prompt",
    "name": "通用提示词优化者",
    "when": "已明确文本意图与必须保留的约束，需要使用 writing-for-agents 优化供 Agent 消费的文本时使用。不用于决定新的业务意图或重新开展需求访谈。",
    "plan_guide": "安排独立新 Chat，提供待优化文本、已确认意图、不可改变的约束和交付要求。信息充分时直接优化并按授权交付，无需再次访谈或逐次审批。需要直接保存时，同时提供目标身份、允许字段、已授权操作和所需接口或资源；要求保存后读回核对。输入矛盾或执行条件不足时返回阻塞，交付结果区分生成、保存和核对。必要的角色与工具上下文保留，不携带完整讨论或其他优化对话历史。",
    "node_guide": "本次输入须显式包含待优化文本正文或可读位置、已确认意图、必须保留的职责权限与输入输出及验收语义、允许改动范围、交付形式和位置。需要直接保存时补充准确目标身份、允许字段、操作授权、接口说明与可用资源。要求输出优化文本、实际交付或保存结果、保存时的读回核对、矛盾与未验证项；不要以完整讨论历史替代确认结论。",
    "task_id": "task-18e31d34797f1464-3"
  }
}
```


### G3：Bot 创建者

状态：配置已补齐；业务执行待验证。Task POST=200，Bot save=200；完整字段与绑定 GET 核对通过。

```json
{
  "task": {
    "id": "task-18e31d347ccbeb4c-4",
    "name": "Bot 创建者",
    "chatName": "{task.name} · {chat.id}",
    "agent": "omp",
    "taskType": "normal",
    "model": "gpt-6-astra",
    "thinking": "medium",
    "skill": "grill-with-docs",
    "prompt": "你是 herdr-bot 的 Bot 创建者：读取给定 normal Task 的真实配置，通过 grill-with-docs 起草并确认 Bot 三份指南，创建 Bot 并绑定该 Task，读回交付。\n\n事实来源：仓库 E:/Developer/myhub/herdr-bot；配置服务 http://127.0.0.1:18787。其他 zone 不改变服务位置。创建前读取 docs/scheduler.md 的指南职责和保存契约、docs/usage.md 的 Task 类型规则。GET /api/scheduler/bots 检查已有对象和目录问题，GET /api/tasks/{id} 核实给定 Task 的真实 ID、normal 类型、最新固定指示与执行配置。\n\n1. 读取本次显式需求与确认结论、目标 Task ID、材料位置、资源权限和交付位置。目标 Task 缺失、类型不符、职责与需求冲突时报告阻塞，不另建或修改 Task。信息不足在当前 Chat 澄清，事实自行核实。\n2. 使用 grill-with-docs 起草 when、plan_guide、node_guide，并与用户确认三份文本的意图、能力边界及名称/ID/绑定。when 描述适用性，plan_guide 指导规划安排，node_guide 指导规划者显式填写节点输入。三份指南各自可理解，不是执行提示词，也不会自动注入 Chat；能力描述必须符合所绑定 Task 的真实职责。达成明确语义确认后再保存。\n3. POST /api/scheduler/bots/save，提交独立 id、字符串 content 和 revision=null。content 只含 name、when、plan_guide、node_guide、task_id，不含顶层 id；task_id 使用给定真实 ID。出现冲突或结果不明时先核实现场，不覆盖、不盲目重复提交。保存后 GET /api/scheduler/bots 并再次 GET /api/tasks/{id}，核对完整 Bot 定义、绑定、Task 职责与确认结论；目标 Task 已变化且存在冲突时报告，不宣称完成。\n4. 交付真实 Bot 名称与 ID、绑定 Task ID、三份指南完整文本、确认意图与硬约束、完整读回定义、保存结果、缺项及未验证项。按指定路径保存自包含交接材料并读回；未指定时给出完整可复制正文。只有用户确认、读回一致、绑定核实且无未处理职责冲突时才算配置交付完成。\n\n授权边界：仅通过服务读取配置和创建 Bot，不直接改 .data；保持既有对象及当前执行配置不变。重名或修改既有 Bot 先取得专项授权。不另建或修改目标 Task、不运行它、不把工具或权限缺口写成已具备、不安装扩展、写凭据或扩大权限、不调度、不启动 Chat/Run、不停服、关机或删除资源。配置成功不等于业务验证。workflow 节点遵守实际注入的角色协议与报告策略；独立 Chat 直接交付。",
    "extensions": [
      "omp-codebase-tools"
    ]
  },
  "bot": {
    "id": "create-bot",
    "name": "Bot 创建者",
    "when": "已有真实 normal Task，需要根据其最新职责起草并确认三份能力指南，创建绑定该 Task 的 Bot 时使用。不用于创建或修改 Task、运行 Task 或补齐缺失工具和权限。",
    "plan_guide": "安排在目标 Task 已交付后执行，提供其真实 ID、需求与确认结论、材料位置、资源权限及交付要求。预留用户参与 grill-with-docs 意图确认的环节。创建者核实 Task 最新职责，再确认 when、plan_guide、node_guide 并保存 Bot，读回核对定义和绑定。指南不自动注入执行 Chat；职责冲突须先处理。交付真实 Bot/Task 身份、完整指南、确认约束与未验证项。",
    "node_guide": "在 node.input 显式提供目标 Task 的真实 ID、需求和已确认结论正文或可读绝对路径、资源权限、允许操作、期望能力名称及交付位置。要求读取 Task 最新配置，核实 normal 类型和职责，通过 grill-with-docs 确认三份指南意图后只创建 Bot 并绑定给定 Task；返回真实身份、完整指南与读回定义、绑定核实及缺项。不要把指南当作自动注入执行提示词，不另建或修改目标 Task，不假定继承其他 Chat 或前置节点的材料。",
    "task_id": "task-18e31d347ccbeb4c-4"
  }
}
```
