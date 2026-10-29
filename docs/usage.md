# 使用说明

本文记录日常操作细节；安装与启动见 [README](../README.md#构建与运行)。

## 页面导航

顶部 tab 顺序为 `chat`、`task`、`zone`、`scheduler`、`workflow`；普通地址打开或刷新回到 `chat`，显式 `#workflow/<Run ID>` 链接则读取对应 Run 详情，不重新启动。scheduler 发起与角色 Task 前提、三份记录及启动失败处置见[Map Run 运行说明](workflow-runtime.md)。
右上角的 `server` 按钮在同一页面打开独立内容页，不打开新窗口；点击“返回”回到进入前的 tab。
键盘可用 Tab／Shift+Tab 聚焦入口，Enter／Space 激活；顶部 tab 还支持左右方向键及 Home／End 导航。
进入 server 后焦点移到返回按钮，返回后恢复到此前 tab。

### 暂停或恢复 Map Run

workflow 是只读观察入口，没有暂停/恢复按钮。要暂停新派发，请进入当前 Run 已关联的管理者 Chat 表达意图，由管理者的 pause 工具落实；暂停不退出已有节点 Chat，它们仍可报告及完成交接。处理后由管理者显式 resume 恢复派发资格，再单独选择 dispatch；resume 本身不启动工作。全部节点 completed 时 paused Run 也可直接 finish。普通 Chat 的 **Resume** 仅定位/恢复原生对话，不等于恢复 Run 派发或接管重启前的 Run。

角色工具须先实际部署；没有 manager-chat/node-chat 入口时不要手填身份或编辑运行记录冒充执行。拒绝、重复调用与部分生效的解释见[暂停新执行与恢复派发资格](workflow-runtime.md#暂停新执行与恢复派发资格)。

### server 固定操作

在 `server` 内容页点击“启动”或“停止”，调用服务端预先配置的脚本；页面仅反馈脚本调用，
即使退出码为 0，也不代表后台程序已经启动、就绪或彻底停止。请求未结束时两按钮禁用，
结果区分别说明完成、失败、忙碌拒绝或结果无法确认，不自动重试或排队补做。
切回 tab 再进入 server 仍可查看本页最近一次调用的反馈。
手动关机入口也位于 server 内容页，保留确认流程；返回 tab 不是取消关机，
全局关机提醒在各 tab 和 server 页仍可见，取消必须显式操作并等待服务端确认。

## Task 模型与删除

Task 的 Codex 模型选项包含 `GPT-6.1-sol`、`GPT-6-luna` 和 `GPT-6-astra`；
思考级别按所选 agent 与模型的能力提供，Pi 的 Luna 支持 `off`，Astra 不支持。
模型目录修改后下次打开编辑、保存或发起 Chat 时重新读取，无需重启。
删除 Task 时先询问是否删除，取消不发出删除请求；删除不影响已经发起的 Chat。

## Task 类型与固定角色扩展

在现有 task 入口新增或编辑 Task 时，第一步选择用途：普通（`normal`，新建默认）、流程管理者（`manager`）或节点执行者（`node`）。用途不等于 agent 类型，也不绑定某次 Run 身份。manager/node 仅允许 omp；从 pi 切换为角色类型时页面切到 omp，并重置不适用的模型、思考级别及可选扩展。普通类型仍可选 omp 或 pi。直接 HTTP 保存也执行同样的类型/agent 约束。

第二步的固定扩展随用途变化：manager 包含 manager-chat，node 包含 node-chat，普通类型不加载两者。角色扩展不出现在可选复选框中，不能取消或混装；其他固定项与可选项仍沿用当前 Task 选项目录。保存的 `extensions` 仅是可选项稳定 ID，不应手工放入角色扩展 ID。发起 Chat 和需要新进程的 Resume 都根据来源 Task 的最新类型选择环境；不会偷偷为错误配置补齐角色。修改类型不改变已运行 Chat 的身份、工具或状态。

角色入口映射位于 `meta-data/task-options.toml`（`taskType` 限定角色固定项）。实际工具、Prompt 和原生扩展运行能力须由 my-agent-toolkit 对应交付及宿主环境提供；配置保存或页面展示成功不代表这些能力已运行验证。缺失或不可用入口在发起时明确报错；Resume 沿既有规则报告扩展降级，不伪称已加载。

### 旧 Task 显式迁移

升级前停止 herdr-bot，备份实际配置与数据，再在仓库根目录执行（路径替换为服务的真实工作目录与配置文件）：

```powershell
python scripts/migrate-task-types.py --root 'E:\path\to\server-cwd' --config 'E:\path\to\herdr-bot.toml'
```

工具按配置解析 Task 与历史路径，为每个未分类旧 Task 显式写入 `taskType: "normal"`，不根据名称、prompt 或扩展猜测角色。ID、显示名、chat-name、agent、model、thinking、初始 Skill、prompt 和全部可选扩展保持原值；历史 Chat 的来源 Task、备忘、cwd、Workspace 及原生文件完全不改写。原 Task 文件保留为同路径 `.pre-task-types` 备份；重复执行已迁移文件不写入。备份冲突、残留 staging、路径重叠、非法结构或链接路径会拒绝迁移，不覆盖正式文件。更早的 Bot→Task 更名必须先完成。

升级后未迁移的 Task 文件会明确报 `tasks-corrupt`，不会在运行时自动补类型。迁移成功后重启服务，从 task 列表核对完整预设，再在历史 Chat 使用 Resume；Resume 读取来源 Task 最新环境，不重放任何 trigger config。若要恢复备份，先停服并保留失败现场，再由操作者将备份还原；旧文件只能由旧版本读取，不能作为新版本的缺省兼容路径。

## Task 配置与原生存储

Task 配置分为两组：

- **trigger config（发起配置）**：初始消息、初始 Skill 调用、model 与 thinking，仅在发起 Chat 时应用。
- **env config（会话环境配置）**：扩展配置；发起 Chat 时加载，Resume 需要新进程时也加载。

初始 Skill 调用是随初始消息提交的动作，不是加载型 Skill；Task 固定 prompt 与本次 input 都属于用户输入，不是系统提示词注入。Task 不提供加载型 Skill 或系统提示词注入。

原生会话文件和附属记录由 agent 管理，herdr-bot 不搬动文件或修改 agent 原生配置。发起 Chat 不保证新建原生对话；agent 自身的自动续接设置不属于 herdr-bot 的功能。

## 历史 Chat Resume

在 `chat` 的历史视图中使用 `Resume` 返回 Pi／OMP 原生对话，不要求该 Chat 仍在当前登记中。已有目标只做 Herdr 内部导航；需要新进程时，加载同一原生对话，不自动继续中断任务，也不重放 Task 的发起配置或发送新 prompt。Herdr 内部导航不保证 Windows 窗口置前。

新启动的 omp 会话可能尚未写入原生文件。herdr-bot 可按[受限原生路径规则](adr/0014-unpersisted-omp-startup-identity.md)保存身份及预期路径，但这不代表文件已落盘或当前可从磁盘恢复。历史文件中的内部 `startupIdentityPath` 仅由可信发起流程写入，普通 register 上报不能设置或覆盖；历史列表不公开该字段。Resume 只对保存的 ID、启动路径与当前官方引用完全吻合的未落盘候选允许定位，不把任意缺失文件当作在线 Chat。实际读取对应文件时若发现身份冲突，明确拒绝，不跳过冲突后启动替代 Chat、不改绑历史；不新增后台落盘检查。原进程已不存在时仍按既有精确 ID 加载及恢复确认流程处理，未确认同一会话不报告成功。

新进程优先使用仍然有效的历史工作目录。目录缺失、已删除或不可用时，只有 Herdr Workspace 的 worktree 元数据能够精确关联历史 checkout 路径，且通过元数据中的共享 Git 目录查询 worktree 信息，能够核实唯一且可用的主仓库根目录，才回退到该主目录；不会从目录名称推测关联。无法可靠证明主目录时，在配置的默认 Herdr Workspace 中新建 Tab，不传 `--cwd`，由 Herdr 选择实际工作目录。默认 Workspace 按配置名称唯一匹配，不采用当前焦点或列表中的第一个 Workspace；无法唯一找到时明确失败。主目录回退仍优先使用原 Workspace，原 Workspace 不可用时才使用默认 Workspace；交给 Herdr 选目录时则直接使用配置的默认 Workspace。

恢复成功前必须核实同一原生会话身份。显式指定目录时，还必须确认实际目录与指定目录相同；交给 Herdr 选目录时，必须观察到有效的实际目录。位置回退与实际工作目录会在页面显示，身份或目录核实失败不会报告成功。Resume 不执行代码 checkout，不弹出目录回退确认，也不改写 herdr-bot 历史记录（包括旧 cwd、Workspace、备忘与来源信息）；回退只影响本次恢复。herdr-bot 不直接改写原生聊天文件，agent 自身恢复时的原生存储行为仍由 agent 管理。

新进程使用来源 Task 的最新环境配置，而不是创建时的快照；没有来源 Task 或来源已删除时，使用 agent 的原生默认扩展配置。修改 Task 不会重新配置已经运行的 Chat。恢复中无法加载的扩展或位置回退会在页面提示。

历史记录支持多行纯文本备忘，电脑与手机共享；备忘不会发送给 agent 或修改原生聊天记录。

若页面显示“启动结果未确认”，外部目标仍可能已经启动。先检查页面提供的目标信息，不要把超时当作未执行；服务不会自动清理或重试。
