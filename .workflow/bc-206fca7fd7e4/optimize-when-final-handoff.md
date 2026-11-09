# Bot.when 表达优化交接

## 对象与来源
- Bot：bot-task-18e65aed740825f8-0（prompt优化者——Map维度）；唯一绑定 normal Task：task-18e65aed740825f8-0。
- 直接消费本次 baseline：E:/Developer/bot-playground/.workflow/bc-3ceba29816b4/create-blank-baseline-bc-206fca7fd7e4.md；用于完整配置、真实 revision、需求及授权核实。
- 直接消费本次 text：E:/Developer/myhub/herdr-bot/.workflow/bc-206fca7fd7e4/write-when-confirmed-text.txt，以及 handoff：E:/Developer/myhub/herdr-bot/.workflow/bc-206fca7fd7e4/write-when-confirmed-handoff.md；分别为已确认正文和确认依据、语义边界。以上索引仅供追踪，不替代命名材料交付。
- 输出 text 是所属字段的唯一最终 UTF-8 纯正文；本说明不复制全文。

## 确认与保义
上游 handoff 记载：用户要求发现入口覆盖 Map、Bot、Task 提示词优化，when 精简，具体修改形式和能力限制由 plan 部分承担。编写者展示完整精简候选及“Map、Bot、Task 提示词优化作为发现入口，保留整图统筹这一能力特点”的意图后，用户回复“采用”，批准对应完整 text。

本次仅删除句首“需要”，使触发动作前置；其余字符不变。发现范围、整图视角、职责分层、约束归属、上下游承接以及减少重复、错位和遗漏的目标全部保留。没有新增业务决定，按本次保义优化授权沿用该全文确认，确认覆盖本次最终 text；未另行要求用户重复批准。

## 语义限制与权限
- 广泛发现入口不取消 Task 的目标 Map 输入契约，不扩展为与 Map 无关的任意独立文本润色。
- 用户已删除的修改形式与执行限制不回填 when；省略不取消真实 Task 的输入、权限、保存、验收和运行边界，也不表示选用前提已满足。执行指示及规划材料承担相关信息；本分支不修改或验收兄弟字段。
- 本次派发仅授权在初始化中间资料目录新建本分支 text／handoff。没有 Bot 保存授权；全文批准不代替最终配置保存或报告发送授权。最终保存者独立核实保存资格。

## 真实核实与文件交付
通过 http://127.0.0.1:18787 读取完整 Bot 目录、POST /api/scheduler/bots/edit 和 GET /api/tasks/task-18e65aed740825f8-0。完整 Bot、revision 的 file／original 及完整 Task 与 baseline 精确比较一致（对象键顺序不影响比较）；目录无问题，绑定目标的 Bot 恰好一个，三指南仍为空字符串。
首次本地比较误将 edit.content 对象作为 JSON 字符串解析，报 TypeError: the JSON object must be str, bytes or bytearray, not dict。仅本地比较失败，无配置写入；按真实对象结构重新只读查询并比较通过。

写入前核实授权目录及全部祖先均非重解析点，目标文件不存在。两份新文件按 UTF-8 保存，完整读回并与各自最终稿逐字比较一致；未覆盖上游或兄弟产物。text 仅有上述一处保义删词。

## 未验证项与后续责任
未保存 Bot，未修改 Task、Map 或运行记录，未执行目标能力或业务试运行；未验证目标宿主 Skill／Suite／MCP／扩展实际加载、模型遵从及业务效果。文本优化完成，行为未验证。
基线核实不是配置锁或交付后不变保证；最终保存者须依契约重新比较真实配置并核实三字段及保存授权。报告仅交接成果，管理者独立验收并 complete。
