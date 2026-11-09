# Bot 三指南保存交接

## 真实身份与范围
- 服务：http://127.0.0.1:18787；核对时间（UTC）：2026-11-09T11:21:52.929088+00:00。
- Bot：bot-task-18e65aed740825f8-0；名称：prompt优化者——Map维度。
- 唯一绑定 normal Task：task-18e65aed740825f8-0；无 map_file。完整目录核实绑定该 Task 的 Bot 恰好一个。
- 仅修改同一 Bot 的 when、plan_guide、node_guide；正文不作任何再优化、补齐或换行转换。

## 确认与保存授权
- 共同基线：E:/Developer/bot-playground/.workflow/bc-3ceba29816b4/create-blank-baseline-bc-206fca7fd7e4.md。完整读取并解析其中完整 Bot、真实 revision 和完整 Task。
- 三份最终 handoff 分别记录 when 的“采用”、plan_guide 与 node_guide 的“同意”全文确认，并说明保义优化后确认仍覆盖对应最终 text。依本次显式材料沿用；未另行查询原生确认历史。
- 原材料明确不授予最终保存权限，因此此前只报阻塞、未调用 save。当前 Chat 明确列出 Bot 身份、三字段原样一次保存和非目标保持范围后，用户回复“请执行”；本次据此取得最终配置保存授权。报告策略为 auto，报告不等于管理者 complete。

## 三字段唯一来源
| 字段 | 唯一正文 | 确认及限制依据 | 正文原字节 SHA-256 |
| --- | --- | --- | --- |
| when | E:/Developer/myhub/herdr-bot/.workflow/bc-206fca7fd7e4/optimize-when-final-text.txt | E:/Developer/myhub/herdr-bot/.workflow/bc-206fca7fd7e4/optimize-when-final-handoff.md | 7313b970308b6e2e9a6935cf295f33583f8a61a4a123c17f7183a2d5a716f528 |
| plan_guide | E:/Developer/myhub/herdr-bot/.workflow/bc-206fca7fd7e4/plan-guide-text-optimized.txt | E:/Developer/myhub/herdr-bot/.workflow/bc-206fca7fd7e4/plan-guide-handoff-optimized.md | 8f20677131a9dcc94ad03c42a225becd5f6fe287ae0a52ad2c10cdfe125dc58b |
| node_guide | E:/Developer/myhub/herdr-bot/.workflow/bc-206fca7fd7e4/node-guide-optimized-final-text.txt | E:/Developer/myhub/herdr-bot/.workflow/bc-206fca7fd7e4/node-guide-optimized-final-handoff.md | b2a45a5f151b613e67e925649218f624d5d1d5037a68c762b83335973d3248f2 |

七份命名输入均完整读取；保存前再次逐字节比较材料未变，未沿 handoff 来源索引寻找另一版本。

## 保存与完整读回
- 通过 POST /api/scheduler/bots/edit、GET /api/tasks/{id}、GET /api/scheduler/bots 读取公开完整对象、真实 revision 和目录。edit 的独立 id 与 content 组合为完整 Bot 后，与基线逐字段一致；revision.file 及 original 原文精确一致，完整 Task 一致。
- 保存前完整读取全部 5 张 Map 及全部可读 Run，检查 7 个 running/paused Run 的管理者、当前及历史 execution 和递归 Map/Bot/Task 引用，目标未被使用，亦非本执行者配置。不靠停服、暂停、改记录或扩权取得写入资格。
- 同一 Bot 三字段内存汇总为完整 content，排除独立 id，携带真实 revision 一次调用 POST /api/scheduler/bots/save。收到有效成功响应；未重试、回滚或直接修改底层存储。
- 保存后再次 POST edit 完整读回，并核对 revision.original 解析值：when、plan_guide、node_guide 的结构化字符串实际值分别与各 text 完整一致；id、name、task_id 和全部非目标字段不变。
- 重读完整 Task、完整 Bot 目录、Map 目录和全部 5 张 Map；Task、其他 Bot、Map 目录及全部 Map 配置精确结构比较不变。目标唯一绑定保持。
- saved 为公开接口 edit 完整读回的真实 Bot JSON，不是候选、成功响应代替读回或本说明中的配置复制。UTF-8，无 BOM；供管理者解析、核对及正式交接。
- handoff 为 UTF-8 Markdown，仅承载核对依据、效果和限制，不复制配置或指南正文。

## 文件安全及效果
写前核实当前授权资料目录至盘符根的全部祖先为真实目录且无符号链接、junction 或重解析点，目标名不存在。使用同目录独占新建临时文件完整写入并 flush/fsync，再由 Windows os.rename 无覆盖发布；不先删除正式文件。两份正式文件均完整读回，与内存预期逐字节一致；saved 再解析与公开完整读回对象一致。本次无其他文件覆盖。
三字段现已生效；无失败、冲突或无法确认的保存项。此前本地比较一度将不含独立 id 的 edit.content 与完整 Bot 比较，正确组合 id 后基线核对通过；该比较不产生配置写入。

## 限制与未验证项
未启动、调度或执行目标业务，未修改 Task、Map、自身配置、运行记录或其他资源；未安装、扩权、写凭据、停服、关机或删除。未运行开发回归或真实模型业务验收。Skill、Suite、MCP、固定扩展在目标宿主的真实加载、模型遵从与业务效果未验证。
真实 Bot revision 用于服务端并发冲突检查；基线及读回不是跨进程锁、跨对象事务或交付后不变保证。配置保存完成，行为未验证。管理者仍须独立验收并决定 complete。
