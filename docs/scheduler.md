# scheduler 使用与参考

scheduler 管理服务级 Map、node 与共享 Bot 的静态定义；定义保存和浏览不发起 Chat、不调度工作。安装见 [README](../README.md#构建与运行)，可复制样例与隔离操作见 [体验指引](example/README.md)。本页说明定义读写，不把尚未绑定的定义当成可以启动的 Run。

## 定义职责与唯一格式

| 对象 | 字段与职责 |
| --- | --- |
| Map | 必需 `title`、`description` 字符串及 `nodes` 对象；key 是 Map 内 node ID。不含运行状态、管理者配置或重试策略。 |
| node | 必需 `title`、`bot_id`、`input`、`zone` 字符串及 `blocked_by` 字符串数组。引用共享 Bot，提供本项工作的原文输入、zone 路径与同图前置依赖。 |
| Bot | 必需 `id`、`name`、`when`、`plan_guide`、`node_guide` 字符串；可省略 `task_id`。跨 Map 共享，只引用 Task，不复制 Task 配置。 |
| Task | 发起 Chat 的可复用预设，持有 agent、trigger config 与 env config；不嵌入 Map、node 或 Bot。 |
| Chat | 实际对话与原生会话身份，不写进静态定义。执行身份、报告、状态和历史不属于 Map 或 Bot。 |

只接受这一种命名字段格式，没有 `schema_version` 或运行时旧格式兼容。旧 node 的 `parameters`、旧 Bot 的 `use_when`、`description`、`inputs`、`outputs`、`implementation` 都是未知字段，连同版本 1 也明确拒绝。上线前停止服务、备份旧文件，再由规划者改写为新格式：把业务参数整理成 `input` 原文，明确选择 `zone` 路径；将旧能力说明整理成三份文档。不能从旧业务参数猜测 Task／zone 绑定。历史更名工具只处理其原来的命名迁移，不完成这次格式切换。

完整 Map 示例：

```json
{
  "title": "检查计划",
  "description": "先准备，再核对。",
  "nodes": {
    "A": {
      "title": "准备",
      "bot_id": "review-work",
      "input": "准备 alpha 材料；在工作中说明材料位置。",
      "zone": "C:/work/review",
      "blocked_by": []
    },
    "B": {
      "title": "核对",
      "bot_id": "review-work",
      "input": "对照要求核对 beta 材料。",
      "zone": "C:/work/review",
      "blocked_by": ["A"]
    }
  }
}
```

共享 Bot 是独立文件，按 `id` 关联而非文件名；A、B 或另一张 Map 可引用同一份 Bot，不产生副本：

```json
{
  "id": "review-work",
  "name": "核对工作材料",
  "when": "需要对照要求核对材料时。",
  "plan_guide": "安排核对范围、材料位置和交接条件，不修改原始材料。",
  "node_guide": "阅读本项 input，核对差异并说明依据与未确认事项。",
  "task_id": "review-node-task"
}
```

`when` 用于决定是否选用能力，`plan_guide` 给规划者安排工作，`node_guide` 给节点理解业务工作。三份都是独立文档，可按任意顺序读取，可各自为空字符串；不要求先读其他文档。Markdown 只是文档内容，不是必填规则、类型检查或业务质量门禁。文档不会自动补全 node 输入、校验业务参数或读取文中路径。`input` 同样是原文字符串，不做插值或 JSON 解析。

## 保存与校验边界

内部结构合法即可保存：Bot 可没有 `task_id`，可引用不存在的 Task；node 可引用不存在的 Bot 或尚未登记／失效的 zone 路径。不会制造默认绑定或复制默认 Task。`zone` 必须显式存在且为字符串，但可为空表示尚未填写；它是登记目录的路径引用，不是稳定 zone ID，也不是 Bot 文档里的业务键。保存不证明启动前外部资源检查通过。

必需字段不可漏填，不用 `null`、对象、数组代替字符串；Map/node/Bot 都必须是 JSON 命名对象，不能用位置数组代替。未知字段明确拒绝，不静默丢弃。显式 `nodes: {}`、`blocked_by: []`、文本 `""` 合法。`task_id` 可缺省或为 null（读取／保存结果省略无绑定字段）；有值必须是字符串。不额外限制文档长度。

`blocked_by` 只引用同图 node ID。未知前置、自依赖或环拒绝整个保存，不能通过删边／删节点生成成功图。依赖错误指出相关 node；结构错误提供可用字段路径，例如 `nodes.work.input`。**非法保存不覆盖原定义**，失败保留独立 ID 与原始草稿；保存失败和冲突都不是成功预览。

## 目录与定义管理

进入 scheduler 读取服务所在电脑的目录，与当前 zone 无关。配置可独立省略：

```toml
[scheduler]
map_directory = '.data/workflow-maps'
bot_directory = '.data/bots'
```

默认值如上；相对路径按**服务进程 cwd** 而非配置文件目录解析，绝对路径也可用。首次读取创建缺失目录及父目录，保留已有文件，不安装示例。只扫描直接包含的 JSON 文件（后缀不分大小写），不递归。配置改变需重启，文件内容改变可“刷新全部”。目录创建／读取失败与正常空态分开报告。

列表选择 Map 后显示节点清单、可展开的只读图。选择 node 显示本次输入、zone、Bot ID 与依赖，前置／后继可点击定位，关闭详情后焦点回节点清单。依赖图自动布局全部 node，包括独立项；可平移、缩放、适应全部、键盘选择。箭头来自 `blocked_by`，如 B 依赖 A 就显示 A → B，不表示执行、材料传递或 Chat 在线状态。无节点的 Map 可阅读，不等于可以启动。

“Bot 管理”在同一 scheduler Tab 中打开左右分栏列表与详情，按名称／ID 筛选；node 的“Bot 明细”进入同一预览。分别切换“选用说明”“规划指引”“节点指引”，或看只读“原始定义”。Task 显示为引用或“尚未绑定 Task”，不嵌入 Task 配置。原始 JSON 不是编辑器。返回 Map 重查来源并保留仍存在的 node 选择；来源已移除时显示原因。窄屏按目录、节点、详情纵向阅读。

Bot 列表（含空态）可新增，预览可修改；当前 Map 可新增 node，选中 node 可修改。ID 独立输入，新增初始为空，修改时只读，JSON 不含顶层 `id`。node ID 是 `nodes` 的 key，Bot ID 由服务端补回。ID 不是文件路径。新草稿使用打包的 [天气 Bot](example/bots/weather-capability.json)／[上海 A](example/maps/trip-readiness.json)，部署不依赖开发仓库；打开、修改草稿或放弃不写定义，也不自动安装依赖。

显式保存 Bot 使用服务端安全文件名，修改保留原文件名与全局唯一身份；保存 node 只改目标 node，保留 Map 标题、说明、其他 node 和其他文件（当前 Map 排版可变化）。成功立即展示最新内容并更新已展开图，不必刷新。Bot 保存不改 Map，所有引用者仍读取同一共享能力。

node 冲突版本以整个 Map 为单位，即使另一页面只改 B，旧 A 草稿也拒绝覆盖；Bot 按原文件与原始全文检查。冲突提示重新加载时，先复制要保留的草稿，再放弃并重开；不自动合并或重试。切换对象、tab／server、刷新、返回或浏览器重载前，有未保存改动须确认放弃；取消保留草稿。筛选只改变可见列表，不重新请求或丢弃草稿。没有 Map JSON 编辑、ID 重命名、node 跨 Map 搬移或独立 node 库。

## 安全删除与并发

删除 node 先完整检查所属 Map；有其他 node 依赖它时列出标题与 ID，不能强制、级联或清除依赖。Bot 删除先检查整个配置 Map 目录的全部 Map，存在引用则列出文件与 node。损坏／不可读 Map 或目录错误也是阻塞，不能当成零引用。只有完整检查且无引用才最终确认；取消不提交删除。确认后再次检查身份、原始版本、结构和引用，外部变化则拒绝。成功只删除目标，清除其详情并更新当前列表／图；最后一项删除后显示空态。

同一应用实例协调 Bot 和 node 写入、删除，持锁到最终检查与提交。Bot 删除先提交后，仍可保存引用该缺失 Bot 的 node；这不是允许删除已被引用的 Bot。外部编辑器不参与协调，**避免在页面提交期间编辑相关定义**；不承诺与任意外部写入跨文件原子提交。文件写到一半可报错，写完后显式刷新。不提供撤销、引用迁移或快照。

## 共享 Bot 问题排查

合法 Map 的 node、输入和依赖边不因 Bot 问题而消失；Map 页和 Bot 管理页汇总“Bot 问题”，关联问题显示在 node。单个 Bot 出错不隐藏其他健康定义，也不捏造说明或默认实现。

- **缺失 ID**：检查配置目录、JSON 扩展名以及 `id` 与 `bot_id` 是否一致，文件名不决定身份。
- **读取失败**：错误列出文件名；检查服务进程账户的文件读取权限。不能解析时可能无法推断 ID，node 同时显示缺失 ID。
- **结构无效**：按文件名、字段路径核对 JSON、必需字段、类型、未知旧字段；尤其区分 `when` 与旧 `use_when`，空字符串与漏填／null。
- **重复身份**：多个文件声明同一 ID 时列出全部文件，不任选赢家；即使一个文件无效也保留冲突。修正重复声明。

Map 文件加载、结构或依赖失败时清除当前图和详情，不把上次成功内容冒充本次；目录仍可选择其他有效 Map。修复后“刷新全部”同时重读列表、当前 Map 和 Bot，清除 node 选择并收起图。较早请求的迟到结果不替换当前选择。没有自动监听、修复、运行时迁移或跨文件快照。

## HTTP interface 与路径边界

| 方法及路径 | 输入／结果 |
| --- | --- |
| `GET /api/scheduler/maps` | 顶层 Map 文件列表 |
| `GET /api/scheduler/maps/{filename}` | 完整校验后的 Map |
| `GET /api/scheduler/bots` | Bot 目录和局部问题 |
| `GET /api/scheduler/bots/new` | 打包草稿，不含 id |
| `POST /api/scheduler/bots/edit` | `{id}` → content、revision |
| `POST /api/scheduler/bots/save` | `{id, content, revision}`；content 是 JSON 字符串；新增 revision 为 null |
| `POST /api/scheduler/bots/check-delete`、`.../delete` | 检查 `{id}`，最终提交 `{id, revision}` |
| `GET /api/scheduler/maps/{filename}/nodes/new` | 打包 A 草稿、所属 Map revision |
| `POST .../nodes/edit` | `{id}` → 真实 node、Map revision |
| `POST .../nodes/save` | `{id, content, revision, create}`；create 是布尔值 |
| `POST .../nodes/check-delete`、`.../nodes/delete` | 检查 `{id}`，最终提交 `{id, revision}` |

revision 为 `{file, original}`，绑定服务端目录内文件名与原始全文，不接受客户端任选路径。失败包含具体 message；不自动覆盖较新定义。读取不接受 zone、查询参数或请求体。Map filename 只能是顶层 JSON 文件名，拒绝绝对路径、`../`、分隔符和 Windows 文件流名。

目录及各级父路径不跟随符号链接、目录联接或重解析点，目录项也不能越界读取。全部标题、文档、input、zone 仅作文本显示，不按 HTML、CSS、脚本或 Agent 指令执行；文中路径不触发文件读取或 Chat。浏览、筛选、预览、恢复、刷新不改写现存定义；只有显式保存／确认删除才修改目标。

可重复契约检查在 `frontend/browser-tests/schedulerDefinitionCutover.spec.ts`：真实隔离 scheduler 保存未绑定定义、分别读三份文档、提交非法依赖并核对原文件。完整入口、安全约束和人工网络／链接规则见 [测试指引](agents/testing.md)。这些定义检查不证明真实 Herdr／omp 启动或业务执行质量。
