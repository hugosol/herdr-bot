# plan_guide 表达优化交接

## 对象、来源与有效确认
所属字段为 Bot.plan_guide；Bot 为 bot-task-18e65aed740825f8-0，唯一绑定 normal Task task-18e65aed740825f8-0（prompt优化者——Map维度）。
直接消费本次命名 baseline、text、handoff：
- baseline：E:/Developer/bot-playground/.workflow/bc-3ceba29816b4/create-blank-baseline-bc-206fca7fd7e4.md
- text：E:/Developer/myhub/herdr-bot/.workflow/bc-206fca7fd7e4/plan-guide-text-confirmed.txt
- handoff：E:/Developer/myhub/herdr-bot/.workflow/bc-206fca7fd7e4/plan-guide-handoff-confirmed.md

原 handoff 记录：用户要求按 writing-for-agents 精简规划无关细节；编写者展示完整三段正文并询问采用全文，用户回复“同意”。本次沿用这一全文及意图确认；依据为直接提供的确认记录，未另查原生对话。委托仅要求保义表达优化，无重新批准要求。本次不改变业务决定，故确认覆盖所交付最终正文；不据此推断最终 Bot 保存授权。

## 表达变更与保义核对
保持三段顺序与全部规划语义，仅压缩措辞、调整句序：
- “范围涵盖关联”简为“涵盖关联”；“独立 Chat 用法”简为“独立 Chat”，范围不变。
- 用户确认环节改为以前置事项起句，业务语义变化、超范围联动及沿用有效确认均保留。
- 非文本变更仍须先另行安排并授权相关工作，不纳入本能力。
- 将交接门槛表述为保存后完整读回一致、定义检查通过再交付，门槛未减。

逐段核对保留：指定既有 Map 的整图优化与获准文本直接保存、各文本字段与明确获准的共同契约／子 Map 内部文本、共享对象一次统筹及全部调用方／独立 Chat；仓库或服务选择且不自动同步、当前执行配置保护及运行禁写；完整交付项目、下游继续判断、仓库修改不等于部署、部署和真实业务／模型验收按需另排，以及“文本／定义优化完成，行为未验证”的界限。
用户精简时删去的通用执行核实、材料读取和上下文传递细节未回填；仍由真实 Task 与适用契约承担。未增加固定图、节点、资源或兄弟指南前提。

## 真实配置核实
使用 http://127.0.0.1:18787，读取完整 Bot 目录、POST /api/scheduler/bots/edit 与 GET /api/tasks/task-18e65aed740825f8-0。
完整 Bot、真实 revision 和完整 Task 均与 baseline 精确结构比较一致；revision.original 逐字一致，revision.file 为 bot-1794222010819446100-10104-0.json。目录无问题，绑定目标 Task 的 Bot 恰好一个；三指南仍为空，无 map_file。未修改 Bot、Task 或运行记录；基线不是配置锁，保存者仍须独立重读核实。

## 文件与交接
本次派发明确授权在 E:/Developer/myhub/herdr-bot/.workflow/bc-206fca7fd7e4 新建本分支材料；未沿用 baseline 中其他阶段目录权限。最终 text 为同批命名输出中的 UTF-8 纯正文，无标题、围栏或说明；handoff 只承载核对依据，不复制正文或配置。
写前已核实目录至 E: 根的全部祖先无链接／junction／重解析点，两个目标名均不存在。两个文件保存后完整读回，与内存最终稿逐字比较；通过后才登记输出。

## 限制与错误
仓库 codebase-memory-mcp 索引已存在且 ready；本次未读源代码或执行开发测试。文档定位命令因 rg 不可用失败（exit 127），后改用 read 读取；首次路径检查回执因 PowerShell 进度流非 UTF-8 引发 UnicodeDecodeError，未执行文件写入，随后只读检查成功并确认无重解析点及目标名冲突。
本次只交付表达材料，不保存业务配置、不执行目标业务或启动 Chat／Run。未验证目标宿主 Skill／Suite／MCP 实际加载、模型遵从、业务效果或交付后配置稳定性；不承诺跨进程锁。最终 Bot 保存授权由保存阶段核实。本次 report_policy 为 auto，报告不等于节点完成。
