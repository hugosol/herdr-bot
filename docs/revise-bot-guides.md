# 独立修订既有 Bot 三指南

公开能力为 `revise-bot-guides` Bot，唯一绑定现有文件 `enhance-existing-bot--bc-301b25d9f8d6.json`。该 Map 已原位切换为“只读核实 → 三路修订确认 → 一次保存”，不再先优化 Task.prompt。没有旧输入字段别名或自动兼容分支；已保存的用户部署、运行中定义与历史记录不会自动迁移。旧独立 Task.prompt 优化者、create-bot／create-bot-task 及无关 Map 不变；确需优化目标 Task 时，可在适用外层工作中单独安排，并在修改完成后重新核实修订基线。

## 调用与交付

| 命名输入 | 必填 | 内容 |
| --- | --- | --- |
| `request` | 是 | 可读 UTF-8 委托：真实既有 Bot ID、业务需求、三字段修订范围及保存授权、资源权限、非目标、材料用途与写入范围；可附正式交付位置 |
| `materials` | 否 | 本次参考材料及用途；缺席合法，提供但不可读则拒绝，不制造空地址 |

只需真实 Bot ID，不要求用户抄完整 Bot、Task 或 Map。`userInput` 是本次背景，不替代 request。通过现有 scheduler 定义、预检及发起入口使用，或由父 Map 的一个节点引用此 Bot：填写本次 `manager_task_id` 和管理者 zone，以同名 `input_rule` 显式绑定文件。子管理者和业务文件不保存到 Bot，也不从父 Chat 隐式继承；调用 zone 不覆盖子 Map 内节点 zone。

父节点声明 `saved` 与 `handoff` 输出，要求子管理者在整个子 Run 完成后把 `w5` 的真实同名结果报告父层；父管理者仍须显式接受交接。Map 不声明不存在的全图输出接口。`saved` 是公开接口完整读回的真实 Bot 对象；handoff 只记录身份、绑定、批准依据、字段来源、保存／读回／非目标核对结果、已发生效果及未验证项，不复制正文、基线或 saved，也不承担隐式寻址。若父层确需 baseline，应额外显式要求该交付。

## 五项工作与安全边界

全部业务材料、安全文件写入、授权、真实 revision 冲突、运行禁写与部分效果规则只遵循[共同业务材料契约](business-materials.md)，不引入另一套协议。

1. `w1` 使用 `prepare-bot-revision`：直接读取 request 与实际提供的 materials，自行经公开接口读取目录、完整 Bot 和真实 revision，核实唯一绑定的 normal Task 或 Map 及完整目标配置。未知事实自行调查，未决业务取舍用 grill-with-docs 在当前 Chat 真实澄清；身份、职责、授权或绑定冲突未解决则阻塞。仅保存单份 baseline 和必要 handoff，完整读回；不创建或修改目标或 Bot，不优化 Task.prompt。
2. `w2`／`w3`／`w4` 分别使用既有 when／plan_guide／node_guide 编写者，直接绑定同一 `w1.baseline`，独立修订、整理并确认所属全文，不读取兄弟定稿。修订案例在同一 Chat 整理表达，不新增独立优化阶段。各自产出唯一 text 与必要 handoff，不写 Bot。
3. `w5` 使用既有文本替换者，直接绑定共同 baseline 与三组 text／handoff。真实批准来源必须覆盖整个保存批次；流程到达、报告发送确认或 `can_complete` 不授予保存权。重读完整 Bot、真实 revision 和完整目标，与 baseline 精确比较。任何缺项、冲突或禁写条件都使整批不开始，不自动合并、重试、回滚或替换对象。
4. 合法时一次 edit/revision/save 保存三个字段，原样使用各 text，保持 id、name、唯一绑定及全部非目标；公开完整读回后逐字段核对，再重读完整目标确认未变。失败或结果不明先核实现场并保留原错误与已生效／未生效／无法确认效果，不吞掉失败。

`w1` 和三编写者为 manual 报告：业务决定、全文确认、保存授权及报告发送确认分别成立，管理者仍负责验收。配置交付不授权业务运行、目标补建、安装、扩权或自身运行配置修改，也不证明目标工具／Skill／扩展加载和真实业务质量。

## 专属 Task 与验收归属

| 稳定 Task ID | Bot | 用途 | 独立 Chat／workflow 配对 owner |
| --- | --- | --- | --- |
| `task-revise-bot-baseline-v1` | `prepare-bot-revision` | 只读核实既有 Bot、真实目标、需求与授权，交付共同基线 | 22 |

三个编写者与保存者沿用[共同契约的既有稳定 Task](business-materials.md#稳定-task-清单与验证边界)，本票不再修改共享 Task 协议。专属 Task 同样可独立使用：显式提供委托和材料用途，缺少必要业务决定就澄清；要求文件但无位置／授权时先澄清，不因缺少 workflow 注入丢失业务安全职责。

## 运行与验证

使用本 worktree 的独立 `CARGO_TARGET_DIR`，先生成当前静态页面并构建安全宿主：

```powershell
npm --prefix frontend ci
npm --prefix frontend run build
cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch revise_bot_guides -- --nocapture
cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch shared_business -- --nocapture
cargo build --manifest-path backend/Cargo.toml --bin herdr-fixture --example safe_shutdown_shell
python scripts/check-revise-bot-guides.py --target-dir $env:CARGO_TARGET_DIR
node scripts/test-backend.mjs
```

`revise_bot_guides` 是启用接线检查：通过公开定义保存／读取及预检消费五节点、同一直接 baseline、三路独立确认与七材料汇合，验证必填缺失、可选缺席／可读及不可读路径拒绝，不固定提示词措辞。`shared_business` 维护 Task／Map 两种绑定的一次保存、真实 stale revision、完整读回及非目标保持边界，本能力不复制 contract 测试所有权。

独立安全后端烟测 `check-revise-bot-guides.py` 实际经 HTTP 保存、读回专属 Task、Map 输入／五节点与 Map 型包装 Bot，用真实临时 request／materials 验证缺席、存在、缺失及 Windows 独占读锁拒绝；核对拒绝不改变定义，无 Run 或宿主调用。每次保留独占目录下的 evidence.json、HTTP 请求响应及持续消费的进程日志，正常和失败均终止并等待所属安全后端及日志线程。使用受控资源只证明公开接线，不证明部署 Suite 或原生角色加载。

最小共享 Task 原生调用沿用[共享材料启用检查](agents/testing.md#共享-bot-指南材料的启用检查)：安装版 Pi、公开 CompletionTransport、隔离 HOME／APPDATA／XDG／sessions，加载实际共享 Task.prompt，真实读取源材料、写入唯一 text／handoff 并完整读回，核对原文和未授权哨兵不变。保留实际授权、原生 read／write 及准确文件字符；进程零退出、模型自称成功或静态定义存在均不充分。该窄烟测不是完整五节点自主执行。发布时实际用安装版 Pi 与 `task-18e31d34797f1464-3` 原始 prompt 执行独立 Chat：真实读取 writing-for-agents 与源文件，真实执行安全文件写入，再直接完整读取 text／handoff；独立核对来源准确、正文不重复、源文件与未授权哨兵不变。首次传输的真实模型输出混入 JSON 外文字，记录了 HTTP 502 与原生 stopReason:error，因此该次不判通过，即使 Pi 自动继续并生成了材料；保留原始失败材料。第二次在全新隔离目录仅对公开 completion 使用 JSON schema 约束序列化，不给定工具选择或业务答案，五次实际模型响应、原生真实工具与完整读回均成功，无原生或传输错误。两次均终止并等待所属原生进程、关闭传输并移除隔离模型配置。此结果不证明专属基线核实 Task 的自主质量或 22 的配对／完整修订验收。

完整 P40／P42／P43／P44／P45 的 Task 型与 Map 型目标、有／无材料、真实澄清、全部负例与专属 Task 配对由 22 唯一负责；本票启用检查不认领这些 contract 场景。外层真实调用归 24。缺真实宿主、模型权限或其他依赖时保留未验证，不能以受控成功代替真实模型完成。
