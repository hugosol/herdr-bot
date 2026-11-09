# plan_guide 交接

## 范围与确认

所属字段：Bot.plan_guide。目标 Bot：bot-task-18e65aed740825f8-0；唯一绑定 normal Task：task-18e65aed740825f8-0，名称为“prompt优化者——Map维度”。本次为创建分支，仅交付字段正文，不保存 Bot、不修改或运行目标 Task。后续表达优化已由本次委托另行安排。

当前 Chat 先展示五段候选。用户指出中间三段啰嗦，并要求按 writing-for-agents 判断规划相关性、精简。编写者随后展示完整三段候选，明确删去通用执行核实、材料可读性与上下文传递细节，保留影响范围、先后顺序和用户参与的条件，并询问是否采用全文。用户紧接回复“同意”。此确认覆盖所交付三段完整正文及其业务意图，不是最终 Bot 保存授权，也不是 manual 报告发送授权。

## 材料与核实依据

直接输入 baseline：E:/Developer/bot-playground/.workflow/bc-3ceba29816b4/create-blank-baseline-bc-206fca7fd7e4.md，用于核实完整配置、需求、授权与边界；不是正文回填清单。后续消费者须直接取得该 baseline 和所属 text／handoff，不沿本说明隐式查找材料。

从 http://127.0.0.1:18787 读取完整 Bot 目录、POST /api/scheduler/bots/edit 的 content 与真实 revision、GET /api/tasks/task-18e65aed740825f8-0 的完整 task。完整 Bot 和完整 Task 与 baseline 精确结构比较一致；目录中绑定该 Task 的 Bot 恰好一个。edit 返回的 revision.file 为 bot-1794222010819446100-10104-0.json，revision.original 与 baseline 原文一致；三指南仍为空字符串，无 map_file。未发现影响目标判断的目录问题。

已读取本机 prompts.local.toml 快照；本次实际使用的目标 Task.prompt 和空指南均无词典占位符。未对 baseline、Chat 或交付材料插值。仓库 codebase-memory-mcp 已有索引。本次未新增术语或修改领域文档。

## 必须保留的语义

- 整图统筹并直接保存获准文本，不改成逐份独立润色或仅交建议；共享对象统一处理，兼顾全部调用方与独立 Chat。
- 仓库定义与服务配置明确选择，不自动同步；共同契约和子 Map 内部文本按授权纳入。
- 业务语义变化和超范围联动须用户确认；有效确认沿用。运行禁写与当前执行配置保护仍有效。
- 拓扑、绑定、结构化输入输出和执行配置变化另行安排获授权工作，不扩展本能力范围。
- 交接以保存、完整读回及定义检查为依据；仓库修改不代表部署，行为验收另行安排。
- 用户要求精简后删去的执行核实、材料读取与传递细节不再回填 plan_guide；它们仍由真实 Task 和适用输入契约承担，删除未改变能力或输入契约。正文不依赖先读兄弟指南，不预设某次业务图、固定节点或资源。

## 文件与验证边界

text 为同批交付的 plan-guide-text-confirmed.txt，UTF-8 纯正文，无标题、围栏或身份说明；handoff 不复制正文或完整配置。文件新建于本次初始化明确授权的 E:/Developer/myhub/herdr-bot/.workflow/bc-206fca7fd7e4，不沿用上游 request 的其他阶段目录权限。写入前已检查目录及全部祖先无链接／junction／重解析点，两个目标名均不存在。

路径检查首次 PowerShell 命令因外层 shell 展开变量而解析失败，未发生写入；改用编码命令后检查成功。文件保存后须完整读回并核对确认稿，登记仅在该检查完成后进行。

本次仅业务材料编写及只读配置核实，未保存 Bot、修改 Task、启动 Chat／Run、执行目标业务或开发测试。未验证目标宿主的 Skill／Suite／MCP 实际加载、模型遵从或业务效果；不承诺交付后配置不变或跨进程锁。后续优化者和保存者仍须按共同基线消费规则独立重读核实。最终 Bot 保存授权留待所属阶段；报告发送另经本次 manual 原生确认，报告不完成节点。
