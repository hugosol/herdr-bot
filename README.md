# herdr-bot

Windows 上的 agent 对话主界面，本机浏览器与局域网设备访问同一个服务。

- `chat`：当前会话状态、历史会话及完成后关机监听。
- `bot`：管理可复用的 Bot；Bot 不是实际对话。
- `zone`：管理服务主机上的目录，浏览文件、审查 Git 变更，以及选择 Bot 发起 Chat。
- `scheduler`：浏览服务级 Map 目录，选择真实 JSON 文件查看 node 依赖图与本次参数，区分空目录与读取错误。

顶部 tab 顺序为 `chat`、`bot`、`zone`、`scheduler`；首次打开与整页刷新都回到 `chat`。
右上角的 `server` 按钮在同一页面打开独立内容页，不打开新窗口；点击“返回”回到进入前的 tab。
键盘可用 Tab／Shift+Tab 聚焦入口，Enter／Space 激活；顶部 tab 还支持左右方向键及 Home／End 导航。
进入 server 后焦点移到返回按钮，返回后恢复到此前 tab。

### server 固定操作

在 `server` 内容页点击“启动”或“停止”，调用服务端预先配置的脚本；页面仅反馈脚本调用，
即使退出码为 0，也不代表后台程序已经启动、就绪或彻底停止。请求未结束时两按钮禁用，
结果区分别说明完成、失败、忙碌拒绝或结果无法确认，不自动重试或排队补做。
切回 tab 再进入 server 仍可查看本页最近一次调用的反馈。
手动关机入口也位于 server 内容页，保留确认流程；返回 tab 不是取消关机，
全局关机提醒在各 tab 和 server 页仍可见，取消必须显式操作并等待服务端确认。

## 构建与运行

需要 Rust 1.98+、Node.js 24+；发起 Chat 还需要已运行的 herdr 及已安装的目标 agent。

```bat
npm --prefix frontend ci
npm --prefix frontend run build
cargo build --release --manifest-path backend\Cargo.toml
backend\target\release\herdr-bot.exe --config herdr-bot.toml
```

运行前检查 [`herdr-bot.toml`](herdr-bot.toml) 中的本机路径、监听地址及 Herdr Workspace，
以及 [`meta-data/bot-options.toml`](meta-data/bot-options.toml) 中的模型和扩展入口。
配置相对路径以进程工作目录为基准。请从控制台启动，以便脚本继承可用的控制台。

Bot 的 Codex 模型选项包含 `GPT-6.1-sol`、`GPT-6-luna` 和 `GPT-6-astra`；
思考级别按所选 agent 与模型的能力提供，Pi 的 Luna 支持 `off`，Astra 不支持。
模型目录修改后下次打开编辑、保存或发起 Chat 时重新读取，无需重启。
删除 Bot 时先询问是否删除，取消不发出删除请求；删除不影响已经发起的 Chat。

本机访问 `http://127.0.0.1:<端口>/`；局域网使用启动日志中的
`http://herdr-bot.local:<端口>/`，无法解析时使用服务主机的局域网 IPv4。
手机上的 `127.0.0.1` 指向手机自身。

## scheduler Map 目录

进入 `scheduler` 后读取的是**服务所在电脑**上的 Map 目录；两个浏览器访问同一实例看到同一组文件，
切换 zone 不改变目录归属，也不需要在手机或其他访问设备上选择本地文件。

不配置时使用进程工作目录下的 `.data/workflow-maps/`。首次进入或刷新 Map 列表时会创建缺失目录及父目录，
保留已有文件，不覆盖定义、不自动安装示例。把自己的 Map JSON 文件放到该目录后，点击“刷新 Map 列表”；
列表只包括目录直接包含的 `.json` 文件（后缀大小写均可），不递归读取子目录，不混入文档样例。
已有目录没有 JSON 时显示正常空态；创建、权限或读取失败显示明确错误，恢复目录后可再次刷新。

可在配置文件中独立指定 Map 与共享模板目录：

```toml
[scheduler]
map_directory = '.data/workflow-maps'
node_template_directory = '.data/node-templates'
```

两个字段均可独立省略，分别使用上述默认值。共享模板默认目录为 `.data/node-templates/`，进入 scheduler 时同样自动创建缺失目录及父目录，保留用户文件且不安装样例；真实空目录与创建／读取失败分别显示正常空态和错误。
两个目录只扫描直接包含的 JSON 文件，不递归读取。**相对路径按进程 cwd 解析，不按配置文件所在目录解析**；
也可填写服务主机上的绝对路径。配置修改需要重启服务；目录内容变化只需刷新列表，当前 Map 也会重新读取。
点击文件名读取真实 Map，自动排列全部 node（包括独立 node）。卡片显示标题与 Map 内 ID；
点击卡片或用 Tab 聚焦后按 Enter／Space，在下方核对**当前 node** 的本次参数与前置依赖。
依赖箭头仅来自 `blocked_by`：B 中写 `"blocked_by": ["A"]` 就显示 **A → B**，不是执行路由、材料传递或运行状态。

支持的 Map JSON 格式（所有结构字段均必需，`schema_version` 为整数 `1`；未知结构字段不受支持）：

```json
{
  "schema_version": 1,
  "title": "检查计划",
  "description": "先准备，再核对；仅预览，不执行。",
  "nodes": {
    "A": {
      "title": "准备",
      "node_template_id": "review-work",
      "parameters": { "target": "alpha", "options": { "items": [1, true, null] } },
      "blocked_by": []
    },
    "B": {
      "title": "核对",
      "node_template_id": "review-work",
      "parameters": { "target": "beta", "zone": "尚未登记的名称" },
      "blocked_by": ["A"]
    }
  }
}
```

Map 是服务级静态工作定义，`nodes` 对象的 key 是 Map 内 ID；node 是具体工作，不是执行实例。
`node_template_id` 引用跨 Map 共享的业务能力模板，而不是 Bot，也不在 Map 内嵌入模板副本。
选择 node 后首先只显示具体工作；点击“模板明细”才展开共享能力说明，切换 node 后重新收起，避免把上一项说明误认为当前工作。
模板按 JSON 中的 `id` 与 `node_template_id` 关联，**不按文件名**关联；同一 Map 多个 node、不同 Map 均可复用一个模板，文件重命名不改变身份。
明细逐项显示标题、适用场景、能力说明、命名输入输出的文本说明和 `chat` 实现类型；这不表示可以执行或发起 Chat。依赖图和本次参数展示不依赖模板内容。

共享模板 JSON 格式（`schema_version` 为整数 `1`；`inputs`／`outputs` 是名称到说明文本的对象）：

```json
{
  "schema_version": 1,
  "id": "review-work",
  "title": "核对工作材料",
  "use_when": "需要对照要求核对材料时。",
  "description": "整理遗漏与差异，不修改原始材料。",
  "inputs": { "target": "待核对的材料说明。" },
  "outputs": { "review": "包含差异与依据的核对说明；不是已生成结果。" },
  "implementation": { "type": "chat" }
}
```

`implementation.type` 目前仅支持 `chat`（后端为仅含 `Chat` 的 Rust 枚举），不接受未知类型并静默回退。
没有模型、Bot、启动指令等实际执行配置；输入输出说明不是参数验证规则，也不会按文字或参数去读取文件。
模板顶层的八个字段及 `implementation.type` 都必须显式填写，类型必须符合示例；固定结构不接受未知字段，版本只接受整数 `1`。
`inputs`／`outputs` 的业务名称可以自由定义，但每项说明必须是文本。显式 `{}` 表示没有命名输入／输出，显式 `""` 表示空文本；
它们不等于漏填或 `null`。`implementation: {}` 仍缺少必需的 `type`；数组不能代替命名对象。不额外限制说明文本长度。

### 共享模板问题排查

有效 Map 的全部 node 与 blocked_by 依赖边不因模板问题而删除，本次参数也不会被模板说明替换。
页面“模板问题”汇总当前文件问题；关联问题同时标在 node 卡片与 node 详情中，展开“模板明细”不会补造业务说明或回退为 `chat`。

- **缺失 ID**：节点、详情及页面提示“缺失共享模板：ID”。检查文件是否直接位于配置的共享模板目录、扩展名是否为 JSON、JSON 内 `id` 是否与 `node_template_id` 一致；文件名本身不决定身份。
- **文件读取失败**：问题中列出实际文件名。检查服务进程账户对该文件的读取权限；不要只检查目录可以列出。单个不可读文件不会阻止正常模板查看。无法读取或解析的文件不能可靠推断其 ID，因此相关节点可能同时显示缺失 ID；依据页面文件名诊断排查。
- **JSON／结构无效**：依据文件名和错误定位检查 JSON 语法、必需字段、类型、未知字段和版本；尤其区分 `use_when` 与误写的字段，以及空对象／空文本与漏填／`null`。其他合法模板仍可查看。
- **未知实现类型**：`implementation.type` 只支持 `chat`；未知值明确报告结构问题，不作为 chat 说明展示。
- **重复身份**：同一个 `id` 被多个文件声明时，明确报告 ID 冲突并列出全部相关文件，不按扫描顺序任选其一。即使其中一个文件结构无效，也不能把另一个文件当作无歧义说明。修改重复声明，使每个 ID 只对应一个文件；正常 ID 不受影响。

修复文件后点击“刷新 Map 列表”重新读取当前 Map 和模板，无需重启；目录级创建／读取失败仍作为目录错误单独显示，而不是伪装为空目录。
`parameters` 是命名参数对象，允许任意直接 JSON 值及任意参数名，不检查是否声明于模板 inputs，
不校验 zone、Bot 或其他资源是否存在，不补入模板默认值。字符串按文本显示，对象及数组使用缩进 JSON；
选择另一个 node 后，参数与前置关系全部替换为新 node 的内容。

服务运行期间可在服务主机上修改 Map 或共享模板，无需重启。点击“刷新 Map 列表”重新扫描当前文件集合，
同时重新读取已选 Map 与共享模板；新增或移除顶层 JSON 后使用此操作更新列表。
点击“重新加载当前 Map”重新读取当前 Map 和共享模板，清除旧图与 node／模板详情，随后可重新选择 node 查看新内容。
快速切换或再次重载时，较早请求的迟到结果不会替换当前选择的图或详情。
不自动监听文件，不保存模板快照或锁定版本，也不提供跨文件原子快照；外部写入尚未结束时，
不完整 JSON 可以显示为读取／格式错误，待写完后再重新加载。所有这些操作均只读定义，不执行工作。

### Map 错误定位与恢复

Map 和 node 必须是使用上述命名字段的 JSON 对象，不能用位置数组代替。每个结构字段都必须显式填写；
缺失 `blocked_by`、误写 `blockedBy`、用字符串代替依赖数组或使用未知版本均报错，不补默认值或忽略未知字段。
合法的空内容与漏填不同：`nodes: {}` 表示没有 node，`parameters: {}` 表示没有参数，`blocked_by: []` 表示无前置依赖；
标题、说明及其他文本字段可显式为空字符串。结构字段不能用 `null` 代替应有类型；参数**值**则可为 `null`、
空文本、空对象、空数组或其他直接 JSON 值，参数名也不会被当作结构字段校验。

错误提示包含 Map 文件名；结构错误附带可用的 node／字段路径（例如 `nodes.work`、`nodes.work.blocked_by`）及 JSON 行列。
依赖错误会分别指出未知前置 ID、自依赖的 node，或依赖环导致无法解除前置依赖的 node（也可能包含被环阻塞的下游 node）。
请核对 `blocked_by` 中的 ID 是否存在、是否引用自身，以及是否构成环；不会通过删边或删节点生成伪成功图。

选择 Map 后若 JSON 解析、结构检查、依赖检查或单文件加载失败，当前图、node 详情与已展开的模板明细均清除，
不会把上一次的成功结果当作本次结果。列表仍保留其他文件，可直接选择有效 Map 恢复预览；修好文件后点击“刷新 Map 列表”重新读取。
例如文件在列表加载后被移走，会显示该文件读取错误，而不是让整个目录内的其他 Map 不可访问。
浏览与恢复不会改写坏文件、自动补字段或执行任何 node。

生产读取接口为 `GET /api/scheduler/maps`（列表）、`GET /api/scheduler/maps/{filename}`（读取顶层 JSON）与 `GET /api/scheduler/templates`（当前共享模板目录）；
文件名由页面编码，不接受目录路径、zone、查询参数或请求体。
目录路径各级不跟随符号链接、Windows 目录联接或其他重解析点，列表与读取均拒绝链接项。
所有业务文本仅作文本显示；参数中的路径不会被读取，HTML／脚本样式文字不会执行。
浏览只允许准备必要目录，不修改 Map 或模板，不启动 Chat，不执行定义。

## Bot 配置与原生存储

Bot 配置分为两组：

- **trigger config（发起配置）**：初始消息、初始 Skill 调用、model 与 thinking，仅在发起 Chat 时应用。
- **env config（会话环境配置）**：扩展配置；发起 Chat 时加载，Resume 需要新进程时也加载。

初始 Skill 调用是随初始消息提交的动作，不是加载型 Skill；Bot 固定 prompt 与本次 input 都属于用户输入，不是系统提示词注入。Bot 不提供加载型 Skill 或系统提示词注入。

原生会话文件和附属记录由 agent 管理，herdr-bot 不搬动文件或修改 agent 原生配置。发起 Chat 不保证新建原生对话；agent 自身的自动续接设置不属于 herdr-bot 的功能。

## 安全边界

**没有账号或身份验证，只应在可信局域网使用，不要暴露到公网。**
能访问服务的设备可以管理 Bot、启动 agent、执行配置脚本、操作 Git 和关闭整台 Windows 电脑。
connect-bot 会话写入另有限制：只接受同机来源，见 [ADR 0004](docs/adr/0004-session-write-boundary-same-host.md)。
页面确认不是服务端认证。

## 历史 Chat Resume

在 `chat` 的历史视图中使用 `Resume` 返回 Pi／OMP 原生对话，不要求该 Chat 仍在当前登记中。已有目标只做 Herdr 内部导航；需要新进程时，在历史工作目录加载旧对话，不自动继续中断任务，也不重放 Bot 的发起配置。Herdr 内部导航不保证 Windows 窗口置前。

新进程使用来源 Bot 的最新环境配置，而不是创建时的快照；没有来源 Bot 或来源已删除时，使用 agent 的原生默认扩展配置。修改 Bot 不会重新配置已经运行的 Chat。恢复中无法加载的扩展或位置回退会在页面提示。

历史记录支持多行纯文本备忘，电脑与手机共享；备忘不会发送给 agent 或修改原生聊天记录。

若页面显示“启动结果未确认”，外部目标仍可能已经启动。先检查页面提供的目标信息，不要把超时当作未执行；服务不会自动清理或重试。

## 旧数据一次性切换

Bot／Chat 术语切换同时改变内部路由、字段、配置与存储格式，不提供旧格式兼容。
迁移需要 Python 3.11+。升级前停止旧服务，在仓库根目录运行：

```bat
python scripts\migrate-bot-chat-terminology.py --config herdr-bot.toml
```

迁移保留已有身份标识；不修改 zone、历史会话或 agent 原生对话文件。
完成后重新构建并启动服务。命名与迁移决策见 [ADR 0011](docs/adr/0011-bot-chat-terminology.md)。

## 文档

- [领域术语](GLOSSARY.md)
- [架构决策](docs/adr/)
- [测试指引](docs/agents/testing.md)：运行入口、并行诊断、夹具隔离与手动网络验收。

源码在 `backend/`（Rust/Axum）和 `frontend/`（React/Vite）。
