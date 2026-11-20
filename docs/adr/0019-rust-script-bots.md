# 脚本型 Bot 使用用户维护的 TypeScript 与已有 Bun，复用节点交接流程

已确认的设计；本文不表示实现或实际运行验收已完成。

2026-11-16 修订：原先将两个 Rust 能力编入后端的选择，无法满足用户自行添加脚本并绑定 Bot、无需重建后端的需求。改为手工维护 TypeScript 脚本，Bot 使用 `script` 绑定配置 Bot 目录旁 `scripts` 目录内的相对 `.ts` 路径（默认 `meta-data/scripts`），与普通 Task、Map 互斥。运行环境由部署者预先提供 Bun；不捆绑、下载或自动安装运行时，也不依赖 Pi／OMP 的内嵌 Bun。Rust 通过独立 Bun 进程动态加载脚本，不新增编辑器或脚本管理 UI。

脚本是可信本机代码，拥有执行账户的文件、网络和进程权限；路径检查仅规范绑定，不构成沙箱。进程分离也不是安全隔离。第三方依赖须由部署者提前准备，不在执行时自动安装。执行 module 的 interface 为默认异步函数与结构化上下文／结果；两个业务脚本是实际 adapter，形成可替换 seam。运行与交接机制集中在 Rust，业务规则集中在脚本，以 depth 提供 leverage 与 locality，而非增加固定能力注册表。系统定义修改仍经过宿主的 `files` 协调、revision 和合法保存，不能用直接覆盖内部 JSON 替代。

manager 通过现有派发入口启动工作后立即返回，后端异步启动 Bun 执行 TS 业务并接入共享报告处理，保存结果并通知 manager；不增加等待批次机制，业务执行不得长期占用 Run 操作锁。复用文件输入解析、输出绑定、报告及完成规则，不伪造节点 Chat 或 session 身份。脚本自动报告，结束时回收执行资源，没有 Chat 或 Tab 需要关闭。manager 使用现有等待与报告读取机制，并显式 complete 后继续安排下游；不增加自动完成、自动派发、自动重试、回滚或跳过功能。异常保留错误与实际效果，由用户或 agent 修复并显式重跑。

脚本复用 input_rule、output_rule 及命名文件绑定；上下文同时提供 node.input 原文，由脚本决定是否消费。最小结果为 `{status:"success"|"failure",outputs?:[{id,path}]}`：状态必填，没有输出时可省略 outputs；路径必须是本机绝对文件路径。正常退出不等于成功，Rust 还需验证结果形状、输出 ID 唯一且已声明，以及文件实际存在且可读；success 必须覆盖全部 output_rule。failure 可保留有效的部分输出，不回滚实际效果。通用脚本不强制 handoff、说明正文或 canComplete 字段；业务诊断可附带 failure。宿主从有效成功结果生成可交接报告，但仍须 manager 显式 complete。

迁移的两个创建流程脚本继续只消费严格命名材料，不解释 node.input。它们保留执行数据所需的严格请求、目标、revision、正文正确性与完整读回检查，只有合法写入与必要材料交付成功才返回 success。V1 空白创建 request 与三指南保存均不生产或消费 authorization／confirmation、approved 或正文批准 digest 等人类批准凭据；成果批准由通用执行层持有，不嵌入业务材料。保存只消费同一真实 baseline 与明确命名的 when_text／plan_guide_text／node_guide_text，字段归属由这些绑定确定，不强制三份指南 handoff。指南 handoff 仅记录来源、语义限制及未验证项，不再是保存验权凭据；保存结果 handoff 仅交付实际核对事实。保留严格字段、真实 revision、完整目标、运行禁写与写后完整读回，不从缺失凭据默认批准。V2 专用材料不随 V1 改写。这些业务约定不强加给其他用户脚本。

## 两个发布脚本与迁移范围

- 空白 Bot 创建者：保留 create-blank-bot 的身份和现有 Map 引用，改为 `script: "create-blank-bot.ts"` 执行目标；替代验证后删除原 Task task-18e33fdc55177d1c-1，不兼容该 Task 的历史来源恢复需求，不迁移或删除历史 Chat 数据。
- 通用文本替换者：保留 replace-text 的名称、能力、Task 绑定及修订流程用途，不收窄其语义。另建三份指南为空的创建流程专用脚本 Bot，负责三路指南合并、合法保存和完整读回；仅将 create-bot-wrapper.json 的 save_bot 改为引用新 Bot。不增加 Map 私有 Bot 或禁止复用机制。
- 两个 TS 实现只覆盖当前创建流程的固定输入输出，不建设通用替换引擎。保留现有输入输出 ID 和材料接线，仅在创建流程收紧 request、baseline 及必要交接材料格式，同步调整生产与消费要求，不改变原通用替换 Bot 或修订流程契约。

## 验收边界

在隔离数据上通过真实 Map 节点执行验证创建空白 Bot、三路指南保存、完整读回、文件交接、报告通知及 manager 完成流程，不以直接调用脚本函数替代端到端验证。验证目标冲突、原文或 revision 不匹配时不误写、不报告成功。生产数据不用于破坏性验收。
