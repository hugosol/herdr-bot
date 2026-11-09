# node_guide 表达优化交接

## 目标、确认与保义
- 对应 Bot：bot-task-18e65aed740825f8-0（prompt优化者——Map维度）；唯一绑定 normal Task：task-18e65aed740825f8-0。
- 沿用直接输入 handoff 所载确认：编写 Chat 展示完整精简候选后，用户回复“同意”，批准该正文及仅指导调用方提供本次业务差异、必要选择与授权的意图。此依据为本次显式交接材料，未另行回查原生对话。
- 本次仅调整句序、连接词和条件表述，保留五段全部语义；无新业务决定，无需重新批准。确认覆盖最终 text；不等于最终 Bot 保存授权或报告发送授权。
- 第一段保留目标、来源和仓库／服务二选一；第二段保留优化目标、问题、不变要求、对象／字段范围及共享对象、子 Map、共同契约分支授权与影响范围；第三段保留材料可选、可读性、内容／用途／适用范围／读取授权／特殊格式及既有确认；第四段保留额外交付条件、成果／用途／格式／验收及文件授权、按需指定位置／命名；第五段原样保留缺项补充责任。
- 固定调查、共享影响核实、保存检查、规划安排及运行协议仍由既有承接位置负责，不回填本字段，不取消约束；不要求调用方抄真实配置。额外验收仍受 Task 的仅文本／定义优化、不启动业务或真实模型流程验收边界约束。

## 直接材料与真实配置核对
以下来源仅用于追踪，不作为下游隐式寻址：
- baseline：E:/Developer/bot-playground/.workflow/bc-3ceba29816b4/create-blank-baseline-bc-206fca7fd7e4.md
- 原 text：E:/Developer/myhub/herdr-bot/.workflow/bc-206fca7fd7e4/node-guide-confirmed-text.txt
- 原 handoff：E:/Developer/myhub/herdr-bot/.workflow/bc-206fca7fd7e4/node-guide-confirmed-handoff.md

三份材料均完整读取；以 baseline 自包含 request 为业务依据，不沿来源索引追读历史计划或兄弟定稿。
通过 http://127.0.0.1:18787 的 GET /api/scheduler/bots、POST /api/scheduler/bots/edit（只读编辑查询）及 GET /api/tasks/task-18e65aed740825f8-0，读取完整目录、Bot、真实 revision 与完整 Task。逐字段精确比较通过：Bot 与 Task 全部字段一致，revision.file 及 original 逐字一致，目录无返回的问题项且目标绑定唯一。目标无 map_file。交付前重新核对输入材料和上述真实配置未变。
当前仓库 codebase-memory-mcp 索引已存在且 ready；此事实不证明目标宿主资源加载。

## 文件交付与权限
本次派发授权仅在 E:/Developer/myhub/herdr-bot/.workflow/bc-206fca7fd7e4 新建本分支 text／handoff；父层目录仅为来源背景。
新 text 为唯一最终 UTF-8 纯正文，无标题、围栏或说明；本 handoff 不复制正文或完整配置。保存使用独占新建，写入前检查目录及全部祖先非链接／junction／重解析点、目标不存在；保存后两文件完整读回与内存最终稿逐字比较一致。实际文件路径通过同名输出登记。

## 限制与未验证项
未保存 Bot、修改 Task／Map 或业务运行记录，未执行文本中的业务；最终保存者另行核实共同基线、三组最终材料和合法保存授权。
本次只核实材料、语义及公开配置；未运行开发测试、目标能力或真实模型流程。Skill／Suite／MCP／扩展在目标宿主加载、模型遵从、业务效果及跨进程原子性未验证，不承诺交付后配置不变。
文本／定义优化完成，行为未验证。

## 本次保存前错误及现场核实
首次安全检查因 PowerShell 的 Parent 返回原生 DirectoryInfo、不带 PSIsContainer 扩展属性而误报 Unsafe ancestor；检查在任何文件创建前中止。随后逐级查询确认六级路径均为 Directory、无 ReparsePoint，两个目标文件均不存在。改用 Directory 属性判断后新建；未盲目重试、未覆盖或回滚。
