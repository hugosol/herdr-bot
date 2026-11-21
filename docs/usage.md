# 使用说明

黑盒验收代码已迁至独立项目 `herdr-bot-tester`。本页验收命令使用显式设置的 `$TESTER` 与 `$BOT`，不要求两个项目相邻。产品构建与自身回归仍从产品根运行。

```powershell
$TESTER = 'E:/Developer/myhub/herdr-bot-tester'
$BOT = 'E:/Developer/myhub/herdr-bot'
$env:HERDR_BOT_WORKTREE = $BOT
```

验收设施的 `scripts/` 路径现属于验证项目。参数、授权、覆盖及非零退出约束不变。完整入口见验证项目 `README.md`。

本文记录日常操作细节；安装与启动见 [README](../README.md#构建与运行)。

## 动态 TypeScript 脚本

脚本功能要求后端进程的 PATH 中已有 Bun（开发验证版本 1.4.4）；不下载、不安装、不捆绑运行时。脚本放在配置 Bot 目录的同级 scripts 目录，默认是 meta-data/scripts。用户手工编辑文件和 Bot 的 script 字段，不新增脚本管理 UI。

例如在 meta-data/scripts/export-result.ts 中写入：

```ts
import { join } from "node:path";

export default async function (context) {
  const path = join(context.zone, `result-${context.executionId}.txt`);
  await Bun.write(path, context.input);
  return { status: "success", outputs: [{ id: "result", path }] };
}
```

Bot 绑定 `"script": "export-result.ts"`，node 声明 `"output_rule": [{"id":"result"}]`。脚本默认导出函数，可同步或异步返回；最小结果是 `{status:"success"}` 或 `{status:"failure"}`。有文件交付时返回 `outputs:[{id,path}]`；path 为本机绝对路径。Rust 校验状态、ID 唯一且已声明、文件实际可读；成功必须交齐 output_rule，失败可以登记有效的部分输出。非零退出、异常或结果格式错误均不能报告成功。handoff、说明正文和 canComplete 不是通用脚本必填字段。

context 提供 runId、executionId、zone（节点工作目录）、input（node.input 原文）、inputs（已解析命名地址）、outputRule 和 outputDirectory（本次 Run/execution 的材料目录）。输入绑定冻结不等于文件内容冻结。每次派发新建 Bun 进程，入口源码按接受时读取的内容执行；相对 import 仍从脚本原目录解析，依赖文件不作历史快照。修改用于之后的派发，不提供恢复旧脚本进程。

脚本是可信代码，不是沙箱：可以使用 Bun 的文件、网络和进程能力，拥有后端账户权限；外部程序和 npm 依赖需预先准备，执行时禁止 Bun 自动安装包。console 日志与结果协议分离。修改 herdr-bot 系统定义应通过 context.host，而不是直接覆盖内部 JSON。

两个发布业务脚本通过 host(operation,args) 复用宿主持久化：read_json({path})／parse_json({text}) 严格拒绝重复 JSON key，read_text({path}) 严格读取 UTF-8，target(唯一 Task／Map 定位) 读取完整目标，bot_read({id}) 返回 bot 与 revision，bot_save({id,content,revision}) 执行唯一绑定、活动定义保护和 revision 检查，material({id,document}) 安全新建并完整读回 JSON 材料。此处 content 不含顶层 id，revision 创建时为 null、修改时使用真实读回值。宿主操作失败会抛出带 code/message 的错误；它不是权限限制，也不是通用脚本的必经步骤。

脚本 node 仅允许 `auto_approved:true`（省略时默认 true），不能配置人工批准；`auto_close` 只能 false，不能指定 manager Task。派发时本次 execution 的 `approved:false`、`approvalSource:null`；只有 Bun 正常退出、结果为合法 success 且全部输出校验通过后，才保存 `approved:true`、`approvalSource:"auto"` 并自动报告，不创建 Task／Chat／Session 或人工确认。失败保留真实部分效果和有效部分输出，不批准、不回滚、不自动重试。合法 fail／retry 保留旧执行，但新执行不继承旧批准或报告，须等待本次成功。

批准与报告不自动完成节点、派发下游或结束 Run，仍需 manager 显式 complete。报告保存或通知失败不撤销真实业务效果及已保存的执行事实，也不自动重投。不要在脚本返回后继续后台写入，返回值应代表本次业务已结束。

## 脚本空白 Bot 创建

发布脚本位于 `meta-data/scripts/create-blank-bot.ts` 与 `save-bot-guides.ts`。Bot 定义使用 `script` 路径，与 `task_id`、`map_file` 互斥，例如 `{"name":"空白创建","when":"","plan_guide":"","node_guide":"","script":"create-blank-bot.ts"}`。现有创建图已绑定两份 TS 脚本，保留原 Bot 身份及修订 replace-text。V1 保存干净切换为仅 baseline、when_text、plan_guide_text、node_guide_text 四项输入，三份正文按命名原样写入字段；不消费指南 handoff 或批准凭据，不兼容旧七项输入。saved／handoff 输出身份不变。旧部署需手工把 `rust_script: "create_blank_bot"` 改为 `script: "create-blank-bot.ts"`，把 `rust_script: "save_bot_guides"` 改为 `script: "save-bot-guides.ts"`，并部署 scripts 目录；没有旧能力执行回退。历史 Run 不自动重写或重跑。

在真实 Map 的 node 中引用它，声明命名 `request` 输入及 `baseline`、`handoff` 输出；`node.input` 可为空，不是业务参数，成果批准模式 `auto_approved` 只能 true，自动关闭只能 false。request 的严格 JSON、普通 Task／Map 目标范围和执行数据约束见[业务材料](business-materials.md#request-与共同-baseline)。从现有 manager 发起 Run、准备输入并 dispatch 后，后台真实创建唯一绑定的三指南空字符串 Bot，保存真实 revision 与完整配置材料，再按原报告／collect 流程交接。manager 显式 complete 前下游仍不准入，报告或执行结束都不自动派发。

真实 Task／Map 的成功与拒绝实例、在途 HTTP 返回和实际文件消费者证据可用[后端隔离回归](agents/testing.md#rust-空白创建公开-http文件回归)重现。记录型外部宿主只证明协议；不等于真实 manager 会话或页面验收。

## Chief 报告保存与通知

两宿主的 `omp-chief-report`／`pi-chief-report` 固定入口只装配到 chief 实际委派的独立普通 Chat；普通 Task 的其他独立 Chat 不因此获得报告角色。预览、批准后启动及新进程恢复均沿用该真实关联，不把独立发送者伪装成 workflow node。直接 Map manager 由 manager-chat 提供同名 `chief_report`；内部 node 继续使用自己的 `report`。原生工具每次展示完整报告并单独确认，固定入口存在不授予后端报告资格。Suite 固定扩展区将该适用角色显示为“chief委派独立Chat”，它不是新的 Task 类型。

`POST /api/chief/reports` 是同机工具的报告提交入口。请求只接受 `sessionId` 和 `body`；后端按真实委派记录授权独立 Chat 或直接 Map manager，不按 Task 名称、扩展是否加载或调用者提供的 Bot／Run／收件人参数授权。Map 内部节点仍用 `/api/workflow/node/report` 向自己的 manager 报告。

`body` 必须包含 `background`（需求背景）、`execution`（执行对象）、`result`（`success`／`failure`／`blocked`）、`summary`（结果说明）、`deliverables`（交付物字符串数组）和 `pendingDecisions`（待用户判断事项字符串数组）。三个文字说明字段（background／execution／summary）不得为空；无交付物或待判断项时明确传空数组。后端逐字保存内容，自动生成不可变的 `id`、时间 `at`、真实 `delegationId` 和发送方 `from`，不接受指定报告 ID 或覆盖原报告。

后端先将报告保存至历史目录旁的 `*.chief-reports.jsonl`，释放文件协调后才按当前有效 chief 的原生会话身份定位，并只在 `[herdr].backend` 选定的宿主尝试一次通知。TUI 使用匹配的 Pane，App 使用已核对归属的 App Pane 与 core 终端；不会按标题、原分派者或另一个宿主补选目标。没有 chief、定位失败或通知失败都不撤销报告。响应分别包含 `saved`、`report`、`notification.status`／`reason` 和诊断写入状态；通知状态为 `submitted`（Herdr 确认输入提交）、`failed`（明确未提交）、`uncertain`（外部效果不可确认）或 `not-attempted`（未发起通知）。`submitted` 不等于可靠接收、已呈现、用户已读或业务验收。真实保存失败返回非成功状态、`saved:false` 和 `not-attempted`，不发送通知；HTTP 回执丢失时先查询持久事实，不应再次提交而制造另一份报告。

`GET /api/chief/reports` 返回全部已存报告；`?sessionId=<发送方原生身份>` 过滤发送方。`GET /api/chief/reports/{id}` 读取同一稳定报告，包括新 chief 没有原始委派上下文时。原报告只读；没有更新／删除入口。初始化、后端重启、会话恢复和时间流逝均不扫描或补投；本入口没有后台重试。接收确认与用户明确批准的人工重发使用原报告 ID／内容，不应重新提交业务报告。

### 持久接收、人工重发与查看

以下 POST 只接受同机工具来源，JSON **仅**为 `{ "sessionId": "当前调用者的原生 Chat 身份" }`；未知字段（包括 `body`、`from`、指定收件人）拒绝。原生扩展仍负责绑定真实 Session、向用户取得明确的本次请求／重发内容批准，不能把这些接口当作自然语言同意的替代。

| 操作 | 资格与公开结果 |
| --- | --- |
| `GET /api/chief/reports/{id}/receipt` | 读取 `{received: boolean, receipt: null或记录}`。不存在的报告返回 404；通知 `submitted` 本身不会创建接收记录。 |
| `POST /api/chief/reports/{id}/receive` | 仅当前 `registered` chief 可可靠接收；返回 `{received:true, process:boolean, receipt, report, guidance}`。只有首次持久提交返回 `process:true`；重复／并发调用、更换 chief、回执丢失或后端重启后均返回 `process:false`，不得据此再次触发模型处理／呈现。 |
| `POST /api/chief/reports/{id}/resend` | 仅原被委派 Chat 可在用户明确要求并再次确认后发送。返回 `{saved:true, report, received, receipt, notification, diagnostic, guidance}`；不新建报告或接受替换内容。已经可靠接收则 `received:true`、`notification.status:"not-attempted"`，不再次通知。否则只尝试通知当前 chief；无 chief／失败／不确定仍保留原报告，不补投。 |
| `POST /api/chief/reports/{id}/view` | 当前 chief 在用户明确要求查看时读取原报告；返回 `{report, received, receipt, process:false, guidance}`，不会创建／修改接收记录、自动通知或重新派发业务。未接收的报告也可明确查看，但查看本身不是接收授权。 |

接收记录为 `{reportId, sessionId, claimId, at}`，其中 `at` 为 Unix 毫秒；保存在原报告 journal 旁的 `*.chief-reports.receipts.jsonl`，原报告内容不变。认领及去重持久提交与 chief 授权／撤销串行化；旧 chief 失权后不能取得新接收授权。外部定位等待后再次核实当前 claim 与接收事实；已经授权并进入外部提交的输入不会被 chief 更换物理撤回，返回值保留真实 `submitted`／`failed`／`uncertain`。

可靠接收不是模型已呈现、用户已读或业务验收。接收后、呈现前崩溃仍保留接收事实，不自动重播；重复 `receive` 的 `process:false` 不是新呈现授权，人工恢复须由用户在当前 chief 明确要求 `view`。存储不可确认返回非成功状态，不能把失败解释为已获处理授权或安全地重新提交业务报告。

实际恢复独立执行者或直接 Map manager 沿用原生 Chat 和持久委派／原报告关联；恢复不新增委派、Run 或重放初始输入，同 Task 的其他 Chat 不获得重发资格。只有修订内容经新的明确批准后，才可通过原提交入口建立新报告 ID。以上 HTTP 行为不代替 OMP／Pi 原生确认与真实模型呈现验收。

## 界面外观

全站保留原有 NieR 米灰底色、浅骨色面板、深棕文字与方角控件，不切换为白底主题。名称、对象 ID、引用、标题、主要正文和输入内容维持原有中性色层级，不按对象类型整体换色。三种辅助色仅分配给部分小字号信息：孔雀青（`--nier-detail-label: #15534f`）用于 12–13px 配置字段标签、表头；深蓝（`--nier-resource: #254c76`）用于工作目录、扩展入口等小字号路径副行；棕灰（`--nier-description: #564a40`）用于小字号帮助、来源／执行说明和元数据。普通次要文字仍用原有 `--nier-muted`，不整体染色；字号、字重和字体不因配色调整而改变。颜色集中定义在 `frontend/src/app/styles.css` 的 `--nier-*` 变量中；原有链接／焦点／运行状态蓝色、成功绿、等待琥珀和失败红保持不变，选项扫动、反色选中与悬浮保留原高对比文字。状态仍由文字、符号和节点形状共同表达，不仅依赖颜色。Scheduler 的成果批准模式标识中，人工模式使用蓝色，自动模式使用琥珀色；`auto_close` 配色不受此互换影响。

Zone 的 Git 变更文件名保留状态色例外：新增绿（`#0c5423`）、编辑／重命名蓝（`#10468a`）、删除红（`#8f1526`），变更文件行使用 600 字重；选中后仍保留类别色。变更祖先目录仍为蓝色，冲突仍用原警告色。diff 新增／删除内容分别使用浅绿／浅红行底及对应深色文字，手机端的 `+`／`−` 标记随行着色；未变更正文不改色。这套高辨识配色仅作用于文件浏览器，不修改全站成功、运行、失败等状态色。

Chat、Zone、Workflow、Task、Suite、Bot、Scheduler 保留原有标题偏移阴影、背景网格、逐字轻跳显现与切页淡入。标题字符依次错开，最长约 580ms 完成；屏幕阅读器直接读取完整标题。Scheduler 的 Map／节点列表与 Bot 列表保留 200ms 从左向右的背景扫动、上下边线与选中反色，旁边的分组等操作不触发选项动画。明细内部仍通过表面、边框与文字层级区分信息；主操作采用深色反白，次要操作保持中性。Chat 区分会话名、Agent 类型、状态、模型与工作目录，离线卡片保持文字可读。Task 展开配置、Suite 资源声明、Bot 文档和 Scheduler 节点明细区分名称、ID、路径、参数与说明；Workflow 分开呈现状态、当前报告、执行关联和旧执行历史。明细辅助文字不小于 12px；Task 操作固定分成两排：上排为 Bot 跳转与“发起 Chat”，下排为分组、编辑、删除。Task 与 Zone 的“发起 Chat”共用深色实心、方角、左侧方块标记及悬停反色样式，字号、内边距、最小高度与禁用反馈保持一致；Bot 与编辑使用方角描边，Task、Bot 与 Scheduler 的分组操作统一使用叠层图标，不显示“分组”或“group · 名称”按钮文字；保留 44px 点击区域，悬停提示当前分组，并提供含对象名称与当前分组的无障碍名称。分组标题与筛选仍显示名称，删除使用危险色文字；悬停提供反色或浅红反馈。平板与手机端操作区移到概要下方，保持两排，不挤压名称。

展开导航中，Task 使用带播放键的方形执行模块图标，Scheduler 使用三个方形节点与分支连线图标，以轮廓区分单个任务和流程编排；Tab 名称与导航行为保持不变。

正文与自然语言报告使用系统中文字体，代码、JSON、ID 和路径使用等宽字体，不请求外部字体。日期与数值使用等宽数字便于比较。表单、文件浏览、diff、图节点和原有确认流程保持不变；图坐标不随视觉反馈改变。“减少动态效果”关闭非必要过渡，不延迟操作或逐字显示业务状态；不提供旧主题切换。

## 页面导航

外侧导航提供两种布局，内部页面与编辑方式相同：

- **紧凑布局**（默认）：顶部只显示一组入口。`chat / zone / workflow` 用于日常工作，`task / suite / bot / scheduler` 用于配置；不额外显示分类标题。两组在横向轨道上按日常工作在左、配置在右排列。仅在导航条内左滑进入配置组，右滑回到日常工作组；整条导航跟随手指移动，松手时拖动距离达到导航可见宽度的三分之一即可切换，否则回弹，不按速度判定，也不循环切换。内容区滑动仍交给当前页面。
- **展开布局**：点击左上角布局按钮，以窄侧栏同时显示两组七个入口。电脑也可直接展开后点击目标，无需拖动。所有入口（包括 Bot 与 Scheduler）共用同一导航样式；页面内部的响应式布局不改变外侧导航的字号、图标、选中指示线或侧栏布局。

布局按钮只改变外侧导航，不重新加载页面，不清空当前编辑或筛选状态。地址中的 `?layout=expanded` 指定展开布局，`?layout=compact` 或无参数指定紧凑布局；切换会原地更新地址，不增加浏览器历史记录。返回紧凑布局时显示当前页面所属组；同一页面期间切换分组会记住每组上次访问的入口。

整页刷新保留 URL 指定的布局，但内容回到 `chat`，即使刷新前正在查看 Run。首次打开显式 `#workflow/<Run ID>` 链接仍可读取对应 Run 详情，不重新启动。Run 列表的 Map 分组目录仅在 workflow 列表可见时读取，停留在其他入口或 Run 详情时不预读；切回列表仍保留已有筛选。scheduler 发起与角色 Task 前提、三份记录及启动失败处置见[Map Run 运行说明](workflow-runtime.md)。

`bot` 是共享 Bot 管理功能的独立入口。Task 列表右侧与 Scheduler 的 Map 明细标题下方显示关联 Bot：唯一关联时点击即切换到 Bot 入口并打开明细；无关联时显示不可点击的“无关联 Bot”，多个关联时显示不可点击的“关联冲突”，不任意选择，也不新增保存限制。Map 此处只查找执行目标为该 Map 的封装 Bot，不列出其节点使用的 Bot。

Bot 与 Scheduler 列表只保留打开明细的入口，不显示分组操作或关联跳转按钮；分组标题与筛选保持不变，分组操作移至明细页。Task、Bot 与 Scheduler 的关联按钮统一显示“目标类型图标＋目标名称”，不再显示“Bot：”或“执行目标：Task / Map”前缀。图标复用对应 Tab：Bot 为机器人、Task 为带播放键的执行模块、Map 为 Scheduler 节点流程图；悬停提示与无障碍名称明确类型和名称。Bot 明细点击目标按钮即切换到对应入口并打开已保存明细；目标未绑定、不存在或读取失败时不能跳转。关联跳转直接丢弃受影响的未保存修改，不自动保存、不弹离开确认；其他原有离开确认保持不变，单纯切换紧凑／展开布局也不会离开编辑页。

Task 列表名称、Bot 明细名称和 Scheduler 的 Map 明细标题旁均提供铅笔按钮，可直接修改对应 Task 名称、Bot 名称或 Map 的 `title`。点击后在原位输入，只有按 Enter 才保存去除首尾空白后的名称；输入法选词时的 Enter 不保存。失焦、Escape、切换页面／明细或打开其他编辑／操作弹窗均取消未提交的修改；空名称或与原名相同不发送请求。保存失败会显示错误并退出输入，保留原名称；只有后端成功后才更新显示。Task 原有“编辑”三步流程和 Bot 原有完整编辑入口保留，Scheduler 原“修改显示名称／保存名称”入口移除，统一使用标题旁铅笔。

改名允许重名，不改变稳定 ID、文件名、引用关系或既有 Chat／Run 名称；修改 Map 标题也不修改 Bot 自身名称，Bot 的目标名称从 Map 读取，不另存副本。保存后以服务端返回值更新列表与明细。Map 并发修改造成版本冲突时需重新加载后编辑，不覆盖他人的修改。Bot 与 Scheduler 保留各自的刷新操作。

右上角的 `server` 按钮在同一页面打开独立内容页，不打开新窗口；右上角原按钮变为“返回”，点击后回到进入前的入口并恢复为 `server`，内容区不另设返回按钮。键盘可用 Tab／Shift+Tab 聚焦可见入口，Enter／Space 激活，方向键及 Home／End 在可见入口之间导航。进入 server 后焦点移到返回按钮，返回后恢复到此前入口。紧凑导航切组使用横向滑动及回弹过渡，布局展开／收起保留淡入效果；减少动态效果的系统设置会关闭自动过渡，但保留手指直接拖动反馈。

### Zone 目录浏览与管理

Zone 列表显示目录名、稳定 ID 和服务器完整路径，可按名称、ID 或路径搜索；标题旁的数量为当前匹配数／登记总数。点击目录行进入只读文件浏览，点击“发起 Chat”仍以该行稳定 ID 打开创建窗口。Git 待提交数量使用蓝色纯数字角标，与领先 upstream 的数量分别显示；冲突保留警示色，悬停或辅助技术可读取完整状态，不把未知状态当作零。目录行不再另显示“浏览 →”文字，点击入口保持不变。

“登记已有目录”绑定服务器现有路径；“新建 Zone”仍受服务器创建父目录配置限制。取消登记需要二次确认，弹窗标明目标 ID 与路径；取消或按 Escape 不发送移除请求，确认后仅移除登记，不删除文件。登记与新建弹窗同样支持 Escape 关闭及焦点返回，请求进行中不能关闭。

电脑端文件浏览保留目录树／文件预览分栏，手机端保留目录与内容的单窗切换及返回按钮。进入目录从页首显示，返回列表恢复原滚动位置；搜索条件保持。文件阅读、变更审查、Commit、远端检查和 Sync 仍使用原有后端流程。

### 跨 server 使用 zone 与定义

在 zone 入口登记稳定 ID 与本机目录。推荐 ID 使用 `namespace/name`，例如 `hugosol/herdr-bot`；没有仓库来源时使用 `default/bot-playground`。命名由用户决定，系统不根据 Git remote 自动改名或克隆目录。一个 ID 在当前 server 上只绑定一个目录，不同 ID 可以共享目录。zone 登记属于各 server 的本机数据，不随 Git 共享；界面不提供重新绑定按钮，需要换目录时取消登记后重新登记。

另一台 server 使用同一张 Map 时，先准备本地目录并登记相同 ID；Map 不保存该机器的绝对执行目录。修改绑定不改变已经启动的 Run，本次 Run 继续使用启动时确定的路径；Run 历史不保证可跨 server 恢复。

直接文件输入保存 zone 内相对路径，`zone_id` 可省略，省略时使用本次 Run 的 **manager zone**，不是 node zone。需要其他目录中的文件时显式指定那个 zone ID，不通过 `..` 绕出根目录；文件必须在派发时存在且可读，系统不自动同步。Run 来源使用 `{id:"本地名",type:"run",input_id:"Map 声明 ID"}`，只取所属 Run 的绑定；未提供可选项不生成地址，已提供但缺失／不可读则拒绝。报告产物与 Run 输入可在所有 zone 外，不复制、不锁定文件；检查仅证明当时可读，之后删改不自动重试或修复。

配置提示词用 `${key}` 引用[本机提示词词典](../README.md#本机提示词词典)。保存与编辑保留占位符；系统发送配置字段时替换一次；仅当替换前的 Task 固定 prompt 含未转义且完整闭合的 `${key}` 时，才追加词典读取说明，便于 agent 直接阅读原始 Map/Bot 文件。用户输入、其他消息段、替换值、`$${key}` 和未闭合写法不触发说明注入。用户临时输入和 agent 产物中的同形字符串不替换。

### chief 委派独立 Task Chat

Task 型 Bot 委派使用公开 HTTP：

1. 当前有效 chief 调用 `POST /api/chief/delegations/preview`，明确提供 `sessionId`、`botId`、`zoneId`、本次业务 `input`，以及 `materials: [{id, zoneId, path}]`。材料的 `path` 为对应已登记 zone 内的相对文件路径；即使与执行 zone 相同，也要显式指定。空业务输入、重复材料 ID、不可读／越界材料、冲突 Bot、没有 Task 目标或目标非普通 Task 均拒绝。
2. 返回 `previewId` 与完整 `facts`：原始请求、真实 Bot／Task 定义、解析后的执行 cwd、材料地址、Chat 名称和将提交的完整 `prompt`。工具应完整呈现这些事实并取得本次原生批准；此 HTTP 预览不是批准本身，也不会创建 Herdr Tab。原生确认的接线与两宿主真实效果属于 toolkit 和跨仓验收。
3. 只在批准后调用 `POST /api/chief/delegations`，请求仅含 `{sessionId, previewId}`。预览单次消费，不能附带替换业务输入。提交时重新核对 chief 权威及 Bot／Task、zone／材料绑定、提示词词典产生的完整文本和模型／扩展选择；批准期间含义发生变化则拒绝并要求重新预览确认，不会执行旧目标或替代工作。预览仅在本次后端进程内有效，重启后须重新确认。
4. `202` 回执沿用普通 Chat 创建的形状与 `id`，通过 `GET /api/chats/{id}` 观察启动。只有 Herdr 就绪、真实原生身份及来源历史／委派关联保存、批准文本投递已确认后才是 `succeeded`；它只表示发起成功，**不表示业务完成**，不等待模型完成业务。失败保留现有阶段、类别、Tab／Pane／agent 与投递结果不确定说明，不自动清理或重试。

`GET /api/chief/delegations/{id}` 读取独立于 chief 上下文保存的委派事实、实际 `sessionId` 和最后观察到的创建状态；`GET /api/chief/delegations?sessionId=<真实原生身份>` 精确查询发送方关联。委派记录保存在历史文件旁的 `.delegations.json` 文件，经过统一文件持久化边界。业务完成情况不由启动回执判断；后端意外中断时最后记录的 `starting` 也不是确定未启动，须核查外部现场，不要重新派发冒充恢复。来源 chief、Bot、Task、cwd、业务输入、材料及执行 Chat 不依靠 Task 名称推断，不产生父 Run／node／execution。材料只绑定地址，不复制或冻结文件内容；之后更改内容由实际执行者观察。

委派按所选宿主装配 Task 环境；App 不要求 TUI 专用状态扩展。业务输入不预填到启动参数，须等所选终端就绪、实际原生身份与批准目录核对、历史与委派关联保存后才提交。`Resume` 原 Chat 继续使用原关联与既有来源 Task 环境恢复规则，不重放初始业务输入。其他使用相同 Task 发起的 Chat 不获得该关联。当前 chief 已更替时，旧进程即使还在线或握有旧预览，也不能再委派；已经提交外部的输入则不能声称被物理撤回。

### chief 按 Bot 委派 Map Run

Map 型 Bot 使用同一组委派 HTTP 操作，不把它转换成没有 Run 的 manager Chat：

- 预览请求明确提供 `sessionId`、Map 型 `botId`、本次业务 `input`、manager 的 `zoneId`、`managerTaskId` 和 `materials: [{id, zoneId, path}]`。manager 必须是 manager 类型 Task，不能省略或从名称猜测；Task 型 Bot 则不接受 `managerTaskId`。材料 ID 对应目标 Map 的 `input_files`，必需项必须齐备，可选项可以不提供，不接受未声明的材料 ID。文件解析及预检复用既有 zone／Run 输入规则。
- `facts.task` 是明确选择的 manager，`facts.map.file` 是 Bot 实际引用的 Map，`facts.map.definitions` 包含所选 Map／嵌套 Map／Bot 定义，`tasks`、`zoneBindings`、`directMaterials` 展示内部执行预设、位置和直接材料。`facts.prompt` 是完整、已组装的 manager 初始输入。预览不创建 Run 或 Chat；必须完整展示后取得本次原生批准。
- 批准后提交仍只含 `{sessionId, previewId}`。重新检查所选定义闭包及提示词词典解析后的业务含义、manager、内部 Task／zone、材料绑定及材料当前字节；变化或失效返回 `409 delegation-preview-stale`，保留原因并要求重新预览确认，既不悄悄执行更新后的工作，也不重试启动。未关联的定义改动不要求重新确认。这个检查不冻结执行后的文件，之后的改动仍由实际执行者观察。
- Map 提交复用既有 Run 发起协议，完成启动返回 `201` 的 Run detail（`id`、`run` 等），不是 Task 的 `202` Chat 轮询回执。通过 `GET /api/workflow/runs/{id}` 读取运行事实；启动失败保持既有 `500`、`startupError`／`readError` 和真实外部效果，不声称回滚。Run 记录初始化失败时不启动 manager。
- 委派记录增加 `runId`，实际 manager 原生身份在业务输入投递前保存为 `sessionId`；`creation` 是最后可靠保存的启动结果，Run detail 保留具体阶段与投递结果。记录、Run manager 和原生 Chat 可互相核对，报告／恢复消费者使用真实 `sessionId` 查询关联。内部 node 不因此获得 chief 委派关联。

该 Run 是根 Run，没有虚构的父 Run／node／execution；manager zone 不覆盖内部 node 各自的 zone。业务输入保持原文，材料作为命名 `fileInputs` 保存。发起成功不代表业务完成，不会替 manager 确认 `inputsReady`、派发内部节点、通过节点完成门禁或代替既有创建流程的阶段审批。恢复与后续报告仍使用原本的管理链；独立原生宿主／模型效果由对应跨仓验收证明。

### Task、Bot 与 Map 分组

三类定义各自使用可选 `group` 字符串归类，没有独立 Group 文件或 ID。同类型对象的分组名称相同即属于同组；名称区分大小写，不允许空字符串或首尾空白。省略 `group` 或使用 `null` 表示未分组。新建对象默认未分组；分组不改变用途、引用、执行或权限，也不会随 Map 引用自动更新。

管理列表按组名排列为可折叠区块，“未分组”最后，组内保留原有顺序。Task、Bot、Map 选择控件对符合原有筛选条件的候选项按组排列。对象的 **group** 按钮可选择同类型已有组、输入新组名或移回“未分组”，保存后更新展示；选择对象的控件不提供归属编辑。没有成员的组自然消失，不提供独立分组管理界面。归属专用接口为 `PATCH /api/tasks/{id}/group`、`PATCH /api/scheduler/bots/{id}/group`、`PATCH /api/scheduler/maps/{filename}/group`，请求必须包含 `{ "group": "组名" }` 或 `{ "group": null }`，只修改最新定义的归属。Task 接口返回最新 Task 列表，Bot／Map 接口返回定义字段及最新 revision。普通 Task 更新和 Bot 保存省略 `group` 时保留原归属，显式 `null` 才清除；node／Map 文件输入编辑保留 Map 归属。直接编辑定义文件或让 Agent 修改 `group` 也可归类；改名须同步修改相关成员，拼写不同会形成不同组。

自举配置已做一次性归类：以“创建task”“创建bot”“创建map”为根，沿 node 的 Bot 引用、Bot 的 Task／Map 目标递归去重，并纳入节点明确引用的管理者 Task；Map、Bot、Task 分别标注 `meta-map`、`meta-bot`、`meta-task`。共享定义只归类一次，不复制对象，也不将反向引用这些能力的其他 Map 纳入。本次归类用于梳理自举定义，不提供初始化打包或安装机制，不意味着 Suite、技能、zone 和本机资源已构成完整发布包。

Bot 页面沿用公共导航样式，不覆盖外侧 Tab 的字号、选中标记或阴影；列表样式只作用于 Bot 内容区域。Bot 列表支持组合使用“分组”筛选与名称／ID／组名关键词搜索；显示计数来自当前筛选结果，可一键清除筛选。组标题提供折叠与成员数量，行末“分组”按钮保留移动到已有组、新建组和移回未分组的操作。桌面未选中时使用完整目录，打开 Bot 后并排显示列表与详情；手机和平板一次显示列表或详情，通过“返回 Bot 列表”保留筛选并回到原浏览位置。分组没有成员后仍保留当前筛选的空结果提示，不会自动切到其他组。

### 删除 Map

在 scheduler 选中 Map 后点击“删除 Map”，在同一个弹窗中选择“删除历史”“保留历史”或“取消”。被任意 Bot 的 `map_file` 引用时，两种删除均拒绝并定位引用，不会自动解绑；Bot 定义损坏、不可读或越过链接安全边界时也拒绝，不能视为没有引用。没有 Bot 引用且检查完整时，“保留历史”可只删除 Map 定义；“删除历史”还会检查 Run 父子执行引用，任一方向（含旧 execution 或单向残留）仍有关联就拒绝并定位父 Run／node／execution／child Run，不级联删除或自动解绑。无此关联且检查可靠时，“删除历史”才同时删除 Map 定义和关联 Run 记录，不受 Run 状态或 Chat 在线状态限制，也不会停止 Chat 或删除对话与工作文件。保留的 Run 仍可在 workflow 查看。完整边界与失败处理见[删除 Map 与历史 Run](scheduler.md#删除-map-与历史-run)。

Map 的空 `input_requirements` 与缺省值等价；节点编辑或删除不会为了空输入要求新增该字段，有内容时仍保留。输入声明编辑接口始终返回可编辑的输入要求文本。

### 成果批准配置与运行事实

在 scheduler 选中 Map 的普通 node，修改“成果批准 · auto_approved”：省略时默认自动，显式自动为 `true`，人工为 `false`。保存后可重开核对；设置与内容 JSON 分开，保存冲突或无效 JSON 保留草稿，不静默覆盖。图中自动模式为圆角矩形、人工为切角矩形；形状表达模式，不表示已经批准。脚本只能自动批准，冲突草稿须显式修正；结束节点显示批准不适用，保留原配置以便取消结束节点后恢复。

workflow 的“当前定义（非执行快照）”显示当前 Map 配置；“当前执行事实”显示本次派发冻结的 `autoApproved`、`approved` 与 `approvalSource`，旧执行历史分别显示各自保存的事实，不从当前 Map 补造。修改 Map 不改变在途执行；合法 retry 采用当前模式，新 execution 从未批准开始，不继承旧批准或报告。自动来源 `auto` 不是用户 Yes，人工来源 `manual` 表示人工批准；模式本身不等于批准。结束节点不显示伪批准或批准等待。

人工模式由用户在节点 Chat 的宿主原生确认中明确 Yes；不是另设浏览器审批页面。报告发送不再要求确认，可随时报告进展、问题或失败。成果批准独立于报告和产物，不通知管理者、不自动完成节点或派发下游；manager 仍须核实最新报告的严格 canComplete:true 与约定交付后显式 complete。打开、轮询、刷新或查看历史均只读，不重派业务。不提供旧运行兼容或批量改写。

节点报告先由后端保存，再向该 Run 自己的 manager 尝试一次通知；嵌套流程内部报告不会改投外层 manager。保存失败时不通知，保存成功后即使通知失败或请求断开，原报告与输出登记仍可从 Run 读回；通知不代表用户已读或节点已完成。外部通知不占用文件协调或 Run 操作锁，也不会触发后台重试、队列或补投。

### 结束节点：交给用户继续

在 scheduler 的 node 设置中标记 `end_node`，可将一个普通 Task 型末端节点设为结束节点。每张 Map 至多一个，不能是脚本型或 Map 型节点，也不能有后继。其余节点全部完成后才可发起；发起成功后，由 manager 显式结束 Run，不等待该 Chat 工作结束。

workflow 显示“已交给用户”，而非把结束节点伪标为 completed。用户可打开或恢复该 Chat 继续任务；启动与恢复均不加载 node-chat，无需向 manager 报告，也不会因 Run 完成而自动关闭。节点业务输入和实际文件材料仍会传入。Run 的完成不是最终业务交付完成的证明。

该 Map 作为子 Run 时仍遵守原有嵌套上报和父节点验收规则，不自动完成父节点。未标记结束节点的 Map 保持全部节点交接完成后才能 finish 的原规则。

### workflow 中的脚本执行

脚本节点从已保存 execution 显示 `kind: typescript`、脚本路径 `script`、执行 ID、业务生命周期、冻结规则与实际文件 inputs，不从当前 Bot 重建历史身份。旧 `rust_script` 执行与来源只供历史读取；不能重新执行旧固定能力。node.input 是否消费由用户脚本决定。

业务在途、业务成功／失败结束、报告 `canComplete`、节点状态与下游依赖资格分别显示。自动报告的严格 `canComplete: true` 只表示可交接；manager 仍须显式 `complete`，此前节点仍为 `running`、依赖仍阻塞。失败保留原错误及可得实际效果，不回滚、不自动 retry。manager 核实后显式 fail／retry 产生新的 execution；旧报告与 outputs 只属于旧 execution，不授予新执行完成资格。

当前／旧脚本执行及脚本来源报告没有 Chat Resume，也没有 Chat 关闭动作；manager、真实 Task／Map Chat 的原入口保留。查看、轮询、刷新和导航只读取运行事实，不派发、不重跑、不完成节点，也不调用外部 Chat。资源结束不表示等待启动 Chat；脚本没有人工批准等待。

### 完成后的 Tab 清理与诊断

Map node 可选择“完成后自动关闭”；关闭会终止该 Tab 内的终端及相关进程，不删除 Chat 历史。此设置固定在每次派发的执行中，只有管理者合法完成该节点后才尽力清理。workflow 仍显示既有的 `completed` 状态，不显示关闭成功或失败，也不会为关闭弹出通知。

Tab 仍存在不代表工作未完成。缺少可靠 Tab 关联时系统直接跳过，不寻找替代目标；Herdr 命令失败、执行不可用或要求额外确认时，完成事实仍有效，后续合法业务照常推进。系统不会自动重试、强制确认或新增确认页面。请按 Run/node/执行 ID（目标已知时还含 Tab ID）在后端输出中查找关闭尝试与结果；只看到尝试不代表已关闭，失败或跳过不能当作成功。确认提示需要在 Herdr 中人工检查，不要通过重复 complete 期待补发关闭。

### 暂停或恢复 Map Run

workflow 是只读观察入口，没有暂停/恢复按钮。要暂停新派发，请进入当前 Run 已关联的管理者 Chat 表达意图，由管理者的 pause 工具落实；暂停不退出已有节点 Chat，它们仍可报告及完成交接。处理后由管理者显式 resume 恢复派发资格，再单独选择 dispatch；resume 本身不启动工作。满足完成条件（普通工作节点全部 completed，结束节点如有则已成功发起）时，paused Run 也可直接 finish。普通 Chat 的 **Resume** 仅定位/恢复原生对话，不等于恢复 Run 派发或接管重启前的 Run。

角色工具须先实际部署；没有 manager-chat/node-chat 入口时不要手填身份或编辑运行记录冒充执行。拒绝、重复调用与部分生效的解释见[暂停新执行与恢复派发资格](workflow-runtime.md#暂停新执行与恢复派发资格)。

### 选择 Herdr 宿主

在 `herdr-bot.toml` 的 `[herdr]` 设置 `backend = 'tui'` 或 `backend = 'app'`；省略等同 `tui`，非法值拒绝启动。TUI CLI 路径为 `executable`，App 窗口控制与配套 core 分别为 `app_executable`、`core_executable`。三条路径可一次填好；也可省略，分别按 PATH 查找 `herdr`、`herdr-app`、`hc`。切换前先结束旧 agent、停止旧宿主，再改 `backend` 并重启 herdr-bot；不迁移正在运行的 Chat 或 Run。

App 模式下，`HERDR_APP_HOME` 指向所选窗口，App 实际连接的 core 和 herdr-bot 调用的 `hc` 必须使用相同 core 环境（非默认实例可通过 `HC_SESSION` 或 `HC_SOCKET` 选择）。仅关闭 App 窗口不会退出 core。herdr-bot 用 App 中的实际 Space／Tab 及 core 中记录的终端归属核对目标；短终端地址相同不能证明是同一实例。App Tab 回执本身既不能证明归属，也不能证明 agent 已可输入。

`workspace_name` 是默认容器名称的唯一来源：TUI 匹配 Workspace label，App 匹配 Space name。按字面精确匹配，零个或多个都报错，不任取一个或新建容器。它不是 zone 路径，也不是原生 Chat 身份。

### 原生 agent 提醒

TUI 的 Task 固定扩展向对应 Pane 报告 agent 状态；App 的 OMP／Pi 将原生 OSC 7501 写入所选 App Tab 的 core 终端，不加载 TUI 状态扩展。App 需要支持 OSC 7501 的 agent 与 core；OMP 保持 `terminal.programStatus` 启用（默认），Pi 默认与终端协商（不要设置 `PI_PROGRAM_STATUS=0`）。

声音只由当前宿主决定。TUI 使用自己的 `[ui.sound]`，App 使用自己的 `settings.json` 中的 `sounds`；两端各自处理自定义声音和焦点抑制。herdr-bot 不播放声音；初始化 ready、connect-bot 会话状态和报告投递不是播音指令。复现和取证步骤见[原生提醒专项](agents/testing.md#原生提醒专项p47p48)。

代码导航统一从 [`backend/src/herdr/native_status/mod.rs`](../backend/src/herdr/native_status/mod.rs) 开始：这里负责宿主状态扩展选择，并标明两端原生状态路径、声音配置与验收入口；`tui.rs` 集中识别官方 TUI 状态扩展。扩展路径与执行角色适用性仍归 Task 元数据，实际播放仍归外部宿主。各发起／恢复入口在原有位置执行选择，保留原校验顺序及 Resume 降级规则。

### server 固定操作

在 `server` 内容页点击“启动”或“停止”，调用服务端预先配置的脚本。`[operations.start]`／`[operations.stop]` 可作为两端共享的固定绑定；如果两端需要不同脚本，另外成对声明 `[operations.app.start]`／`[operations.app.stop]`（两段均指定 `script` 与 `working_directory`），仅 App 模式使用该对绑定。

页面仅反馈脚本调用，
即使退出码为 0，也不代表后台程序已经启动、就绪或彻底停止。请求未结束时两按钮禁用，
结果区分别说明完成、失败、忙碌拒绝或结果无法确认，不自动重试或排队补做。
切回 tab 再进入 server 仍可查看本页最近一次调用的反馈。
手动关机入口也位于 server 内容页，保留确认流程；返回 tab 不是取消关机，
全局关机提醒在各 tab 和 server 页仍可见，取消必须显式操作并等待服务端确认。

## Suite 管理与权威定义

Suite 页面统一展示全局只读的 **Task 固定扩展**，各 Suite 的扩展则按宿主逐行配置入口路径，不提供可选扩展勾选目录。固定区按 OMP／Pi 分组，展示名称、当前解析后的入口路径和适用执行角色；无需先选择某个 Suite 即可查看。Task 编辑页不再重复展示固定列表，仍保留 Suite 选择器。这只是展示位置迁移：未选择 Suite 的 Task 仍按既有宿主／角色规则加载固定扩展，固定区不是某个 Suite 的专属配置，也不表示扩展已运行或连接成功。

`connect-bot` 已加入两宿主的固定扩展，分别使用 `${roots.toolkit}/entrypoints/omp-auto/connect-bot.ts` 和 `${roots.toolkit}/entrypoints/pi-auto/connect-bot.ts`；不再是 Suite 可选项。它与其他无角色限定的固定扩展一样适用于普通独立 Chat、manager 与 workflow node，不新增角色例外。auto 入口在新进程第一次主会话启动时异步尝试连接登记，收到有效回执才是已连接；可用 `/connect-bot off` 关闭，失败不会自动重试，新进程恢复不继承旧连接状态。不要同时加载 manual 和 auto 两个入口。

桌面采用工具集列表与编辑区双栏，列表显示名称、稳定 ID、适用宿主以及实际 Skills／MCP 数量；手机采用紧凑列表与单栏表单。编辑内容按基本信息、资源路径、语言服务和集中 MCP 分组，保存与关闭操作在编辑区顶部并随滚动保持可见；删除位于表单末尾，仍需确认。手机可通过左上角布局按钮收起侧栏，为路径和 JSON 编辑留出宽度。未选择工具集时显示编辑指引，读取失败时保留重新读取入口。

在 `suite` 入口创建、列表查看、编辑或删除场景工具集。Suite ID 由作者指定，使用英文字母、数字、`-`、`_`；保存后不能改 ID。harness 只能选择 Pi 或 OMP，保存后不可切换；需要另一宿主时新建 Suite。ID 以 `-pi`／`-omp` 结尾，显示名称以 `· Pi`／`· OMP` 结尾。Task 只能选择匹配其 agent 的 Suite；更换 agent 后原选择不兼容时，必须显式重选或取消后才能保存，不自动配对或清空。Skills 和 Prompt templates 按行声明具体资源位置；Extensions 按 OMP／Pi 在文本框逐行填写入口路径，不扫描整个来源目录或导入默认集合；LSP 使用服务名到原生配置对象的 JSON 声明。页面显示声明不表示语言服务或 MCP 已经运行，资源装配与原生消费是另外的启动步骤。

OMP 发起或以新进程恢复有 Suite 的 Chat 时，会将非空 `lsp` 声明写入本次私有生成目录的原生 `lsp.json`，通过 `--plugin-dir` 增量发现，不覆盖项目或用户的 LSP 文件。保留 OMP bundled defaults、HOME 根配置、插件、用户及项目的原生合并规则；用户／项目配置可覆盖 Suite，Suite 不是排除其他服务的白名单。Pi Suite 不接受 LSP 字段，不使用 Pi 结果证明 OMP 已通过。

批准的四种服务如下（字段名采用 OMP 原生 `command`、`args`、`fileTypes`、`rootMarkers`）：

| 服务名 | command / args | fileTypes | 项目标记（任一） |
| --- | --- | --- | --- |
| `rust-analyzer` | `rust-analyzer` / `[]` | `.rs` | `Cargo.toml`, `rust-analyzer.toml` |
| `typescript-language-server` | `typescript-language-server` / `["--stdio"]` | `.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs` | `package.json`, `tsconfig.json`, `jsconfig.json` |
| `jdtls` | `jdtls` / `[]` | `.java` | `pom.xml`, `build.gradle`, `build.gradle.kts`, `settings.gradle`, `.project` |
| `pyright` | `pyright-langserver` / `["--stdio"]` | `.py`, `.pyi` | `pyproject.toml`, `pyrightconfig.json`, `setup.py`, `setup.cfg`, `requirements.txt`, `Pipfile` |

声明本身不是已启用或运行的状态。OMP 根据项目标记、本地／PATH 可执行程序和原生优先级决定可用集合；程序缺失时不会启动。TypeScript 服务还需要兼容的 TypeScript 运行环境，Java 需要兼容 JDK 和完整 JDT LS 安装，Rustup 代理需要既有工具链组件，Python 服务需要相应 Node/Python 环境。Herdr-bot 不安装这些依赖、不修改系统 PATH、不把缺失或初始化失败显示为运行成功。在 Chat 中调用原生 `xd://lsp` 的 `capabilities` 和 `status` 观察初始化及 `ready`；`configured, not started` 仅表示可发现，不能当作运行证据。缺依赖时保留阻塞和未验证结论，专项入口见[测试指引](agents/testing.md#omp-suite-四语言-lsp-真实验收)。

部署可在 `herdr-bot.toml` 指定以下位置；普通相对路径相对该配置文件，支持既有 `${roots.name}` 路径根，改路径后重启后端：

```toml
[suites]
definition_file = 'meta-data/suites.json'
mcp_file = 'meta-data/mcp.json'
generated_directory = '.data/suites'
```

`definition_file` 是唯一权威文件，界面直接读写它，不读取生成目录中的同名文件，不建立数据库副本。缺少文件表示没有 Suite，首次保存会创建父目录；文件损坏或不可读明确报错，不覆盖成空清单。外部编辑后点击“重新读取 Suite”（有未保存编辑时先确认丢弃），或重启程序；两种方式都从该文件取得定义。保存不会主动重配已有 Chat。

两种类型共用一份清单，各自声明资源，例如：

```json
{
  "suites": [
    {
      "id": "review-tools-omp",
      "name": "评审工具 · OMP",
      "harness": "omp",
      "skills": ["${roots.skills}"],
      "extensions": ["${roots.toolkit}/entrypoints/omp-auto/codebase-tools.ts"],
      "mcp": ["catalog-search"],
      "lsp": {}
    },
    {
      "id": "review-tools-pi",
      "name": "评审工具 · Pi",
      "harness": "pi",
      "skills": ["${roots.skills}/code-review"],
      "extensions": ["${roots.toolkit}/entrypoints/pi-auto/codebase-tools.ts"],
      "mcp": ["catalog-search"],
      "prompts": ["../prompts/review.md"],
      "codemode": "on"
    }
  ]
}
```

资源字符串保留根引用与声明相对路径，其基准是 **Suite 声明文件** 所在目录，不是 Chat 或后端的 cwd。资源实体继续在原位置维护，本编辑器不复制源码、认证、模型或历史配置。

Suite 扩展不维护可选目录，也不提供勾选项。OMP Extensions／PI Extensions 每行填写一个入口，保存到该 Suite 的 `extensions` 数组。`tasks.metadata_file` 中的扩展声明只用于固定环境；固定项仍在 Suite 页只读展示，不写入 Suite。

新增扩展时，将源码放在稳定目录，按该项目要求安装依赖（例如在包根运行 `npm install`），必要时先构建，再把兼容宿主的入口加入 Suite 并保存。不需要修改 herdr-bot 代码或新增可选 TOML 条目；启动不自动安装或更新依赖。独立 Skills、Prompt templates 仍需分别声明，扩展自身注入的提示随扩展运行。

每次发起或需要新进程的来源 Task 会话恢复，重新读取 Suite 中当前宿主的入口路径。`${roots.*}` 使用根配置；普通相对入口以 **Suite 声明文件** 所在目录为基准，不随 Chat cwd 改变。路径修改只影响后续新进程；已有进程不热切换。不同 Suite 的路径列表独立保存；保存时去重字面路径，启动时对同一实际目标再去重。

Suite 的 `extensions` 是单一 harness 的入口路径数组（例如 `["entry.ts"]`），不接受旧宿主键对象或 `extensionOptions` ID 字段，也不把字符串猜作目录项 ID。启动只加载当前宿主声明的入口，直接把原始绝对路径传给宿主 `--extension`，与 Task 固定扩展方式一致。Pi 必须声明具体文件；OMP 仍可使用原生支持的文件或目录。herdr-bot 不展开扩展的 `package.json`，不生成扩展链接、包副本或扩展清单，也不递归预检 `node_modules`。模块导入与资产读取由原生宿主在源目录完成，固定扩展仍按既有执行角色规则单独加载。

发起 Chat 或需要新进程的 Resume 都检查已解析 Suite 的声明资源。资源缺失或不可读时在启动前明确失败，不改选或回退原生默认发现；来源 Task 或 Suite 权威配置不可用时仍适用原有的配置降级规则，而单个 Task 固定扩展不可用也仍按既有规则报告。包内依赖与资产不在启动前递归预检，缺失模块或运行期资产错误由原生宿主报告。扩展与技能均从原路径加载，不需要创建目录符号链接；程序不会提权、修改开发者模式或以 junction／复制替代资源。生成目录只保存仍需要生成的宿主配置，不保存资源源码副本。

### 按宿主选择技能源路径

`skills` 是当前 harness 的路径数组，不接受旧宿主键对象。OMP Suite 的每项是技能**集合目录**：仅发现该目录直接子目录中的 `SKILL.md`，不递归，也不加载集合目录自身的 `SKILL.md`。多个集合目录共同构成所选范围，后续发现会包含其中新增的技能。Pi Suite 的每项则是一个包含 `SKILL.md` 的技能目录。路径均相对 Suite 声明文件解析，并支持根引用。

`SKILL.md` 使用原生 YAML frontmatter；`description` 必须非空，省略 `name` 时使用来源目录名。同一真实资源重复引用去重；不同实体声明同名时明确报错并指出来源，不按目录顺序覆盖或自动改名。指定目录缺失或不可读时失败，不回退到其他来源。

发起前检查所选技能及配套资源可读，不生成技能链接目录。每次启动或需要新进程的来源 Task 会话恢复，重新读取当前集合，按 canonical 路径去重，并由本次有效直接子技能的最终名称生成 `includeSkills` 白名单；不同实体同名时拒绝启动，不能因其中一个未启用而绕过冲突检查。集合新增或移除有效技能会改变下一次启动的白名单。

OMP 的 `omp-skills.json` 使用现有全局安装已支持的原生 `--config`，只对 `skills` 做高优先级局部覆盖：直接传入多个源集合目录，设置 `includeSkills`、`ignoredSkills: []`，将已有七个来源开关 `enableCodexUser`、`enableClaudeUser`、`enableClaudeProject`、`enablePiUser`、`enablePiProject`、`enableAgentsUser`、`enableAgentsProject` 均设为 `false`，并按有效名称是否非空设置 `enabled`。不修改 OMP 源码、全局安装或用户／项目配置，也不要求部署新的宿主构建。该覆盖约束的是**原生最终可用技能列表**，不承诺禁止默认目录扫描；默认来源中不在白名单的技能不得成为可用技能，实际名称及来源仍需原生验收核对。原本属于默认来源的目录若被显式指定，仍可加载。

技能列表隔离不改变原生扩展、agent、MCP、LSP 或项目指令，不是文件访问沙箱，也不复制认证或改变模型／会话位置。明确选择空集合或没有有效技能的 Suite 使用空白名单与 `enabled: false`，不回退；**不选 Suite** 则不传技能覆盖，保留原生技能行为。正文及资源按原路径读取；已有会话中的历史正文不会被追溯替换，运行中的进程不保证自动重发现元数据。

生成位置默认 `.data/suites`，相对服务 TOML，可使用 `${roots.name}`；交付配置使用 `${roots.suite_resources}`。每次入口只有在资源检查和配置生成完整后才交给 Herdr，不原地改写已经交出的清单，不持有文件协调锁执行 Herdr。生成目录不是 Suite 权威副本、资源源码或认证目录，不要把它作为手工维护入口。源技能、资源或生成位置不可读／不可写时，公开启动返回 `400 task-suite-invalid` 及具体 Suite／路径原因；不创建 Tab，不以空环境启动，不自动提权、启用开发者模式或复制／junction 替代。清理失败另行保留在错误说明中。

同一次启动的技能及适用宿主的 LSP／MCP／Pi 配置共用独占新建的 `<suite-id>/<本次装配>` 目录，全部写好才发布启动参数。同 Suite 或不同 Suite 的并发启动各用自己的完整目录，不会改写或清理已经交付给另一启动的产物；协调范围在进入 Herdr 前结束。资源检查失败不创建半套输出；创建后失败仅删除本次确实新建的配置目录，不删除技能源路径，也不扫描删除生成根中的未拥有文件。清理失败会与原始失败一起反馈，不新增跨文件事务或跨进程写入协议。

成功交付的装配目录可能仍被已有进程消费，因此不会在下次装配时自动回收。需要手工重建生成目录时，先停止所有使用它的 Chat 进程，且只移除确认属于程序的装配子目录，不删除 Suite 权威定义或资源源目录。受控装配不会放宽 zone 浏览与普通持久化对任意重解析点的拒绝。普通权限下的原路径资源加载和失败清理入口见[装配安全验收](agents/testing.md#suite-装配发布与链接安全验收)。

真实消费专项入口见[测试指引](agents/testing.md#omp-suite-技能真实消费验收)。它使用真实 Herdr 与已安装 OMP 的交付启动入口；常规后端替身检查不能代替真实技能消费证据。

### Pi 原生启动资源

Pi Suite 使用原生 `--no-skills` 和逐项 `--skill`，不关闭 builtin MCP。技能参数直接使用 `skills` 中的原目录，不创建链接；明确空 Suite 与不选 Suite 不同，后者不添加技能／模板覆盖。Pi Suite 的 `extensions` 必须填写具体入口文件；后端逐个传原始源码绝对路径 `--extension`，不解析包的 `pi.extensions`，也不创建包链接。依赖和资产留在原目录，由 Pi 原生加载。Pi 本地模板通过原生 `--prompt-template` 逐项选择，初始化不展开；LSP 不传给 Pi。

交付目录的 `pi-suite` 是固定扩展，无须 Suite 重复声明。`--no-extensions` 同时关闭 builtin 自动发现，因此发起／恢复路径显式保留 `builtin:mcp` 与 `builtin:tool-search`；无 Suite 时还保留 `builtin:codemode`，有 Suite 时由 pi-suite 使用本次明确模式，避免两个 codemode 实例。配置产物为 `generated_directory/<suite-id>/<本次装配>/pi-suite.json`，通过 toolkit **公开工厂期 `--pi-suite-config <绝对路径>`** 通道交付；部署必须包含支持该 flag 的 toolkit，旧的仅环境变量通道不能由 Herdr 的 `agent start` 参数自动替代。配置只含版本、模式和本次引用解析出的 `mcpServers`，不会写全局模式、项目 `mcp.json` 或移动认证、模型、会话存储。MCP 的 `cwd`、带根引用或 `./`／`../` 的 command 以集中 MCP 文件目录解析，裸 command 保持 PATH 查找，args/env/url 保持原生字面语义。

开发真实烟测见[测试指引](agents/testing.md#pi-suite-启动开发烟测)。它不替代 Pi 完整资源／MCP／模式／模板契约矩阵；缺宿主能力或尚未交付的 toolkit flag 时应明确记为未验证，不能以参数或产物存在宣称原生消费通过。

### 宿主适用配置与集中 MCP

Pi Suite 必须明确保存顶层 `codemode: "on"` 或 `"only"`。勾选 **Codemode only** 保存 `only`，取消保存明确的 `on`，不表示关闭 codemode 或继承全局 only。Pi 独有 `prompts` 与 `codemode`，OMP 独有 `lsp` 与 `dap`；另一宿主的字段即使为空也拒绝，不静默跳过。编辑器只显示当前 harness 的配置；不再接受旧 `agents`、宿主键资源对象或嵌套 `pi` 字段。

MCP 连接仍由维护者在集中文件编辑，界面只选择 ID，不新增 MCP 编辑平台或进程 manager。例如 `mcp_file` 内容：

```json
{"mcpServers":{"catalog-search":{"command":"search-server","args":["--stdio"]}}}
```

Suite 的 `mcp` 只保存引用，不保存连接配置副本；空数组表示不追加，不禁用宿主原生 MCP。定义页只读此集中目录，不执行其中的命令。保存 Suite 会重新读取集中目录并拒绝不存在的 MCP ID。外部删除 MCP 定义后，重新读取的 Suite 编辑器仍保留已选 ID，并标出“失效”；不会静默丢弃引用。只有显式取消该失效项或恢复集中定义后才能保存。

### OMP 原生增量 MCP

选择 Suite 发起或新进程恢复 Chat 时，后端把引用的集中定义发布为只含 `.mcp.json` 的独占资源包，通过原生 `--extension` 追加给 OMP。不向项目写入配置，不替换整个 agent 配置目录，也不建立另一套 MCP manager；连接、工具、资源列举／读取、服务器 prompts、通知刷新、instructions 与 `/mcp` 管理交互均由原生 OMP 提供。Suite 本地 prompt templates 的省略不影响 MCP 服务器 prompts。

MCP 的 `command` 支持 `${roots.name}`；`./`、`../` 等路径式命令及 `cwd` 相对 **集中 MCP 文件** 所在目录解析，不随 Chat cwd 或生成位置改变。裸命令仍由 PATH 查找，其余原生连接字段原样交给 OMP；命令参数仍遵循服务器／宿主语义，不逐个猜测为文件路径。

不读取或合并用户／项目 MCP 配置；同名胜者、异名等价连接去重、不同参数的独立连接、项目 `enabled: false` 及用户 `disabledServers` 都遵循 OMP 原生规则。合法的覆盖或禁用不是失效引用，不强制 Suite 胜出或重启服务器。空 MCP 清单不生成额外包，用户与项目原生服务器照常可用。

配置引用或资源发布失败拒绝本次启动；有效但不可达的服务器由 OMP 显示连接中／未连接等原生状态。Herdr 接受 Chat 不表示 MCP 已可用，不增加“所有服务器就绪”门禁。实际交付入口的专项自动验收见[原生 MCP 测试指引](agents/testing.md#omp-suite-原生-mcp-真实消费验收)。

### OMP Suite 调试器（DAP）

集中定义由 `[suites].dap_file` 指定，默认 `meta-data/dap.json`。维护者编辑文件，OMP Suite 仅保存 `dap: ["codelldb"]` 引用；Pi 不接受 `dap`。列表只显示调试器名称，展开详情查看完整语言、说明和原生配置。语言来自 `config.languages`，是适配器声明，不是逐语言实测保证。旧 OMP Suite 省略 `dap` 等同空数组；交付的「代码实现 · OMP」（`code-implementation-omp`）选择 CodeLLDB，使用前需配置本机 `roots.codelldb`。其他 Suite 不自动勾选。

```json
{"adapters":{"codelldb":{"name":"CodeLLDB","description":"Windows x64 Rust；PDB 下 enum 显示有限。","config":{"command":"${roots.codelldb}/extension/adapter/codelldb.exe","args":[],"connectMode":"stdio","languages":["c","cpp","rust","zig"],"launchDefaults":{"stopOnEntry":true,"sourceLanguages":["rust"]}}}}}
```

`name`、`description` 是目录展示信息，只有 `config` 进入 OMP 原生 `dap.json`。适配器 `command` 支持 `${roots.*}`，带 `./`、`../`、`.\\`、`..\\` 的路径以**集中 DAP 文件父目录**解析；绝对路径保持原意，裸命令由 PATH 查找。参数及 launch/attach defaults 保持原生语义，不猜测哪些字符串是路径，也不展开原生 `${port}`。使用绝对路径或明确根引用可避免 Chat cwd 改变带来的歧义。

每次发起或需要新进程的来源 Task 恢复重新装配所选定义，在本次独占目录生成 `dap.json`，通过 `--plugin-dir` 注入；没有 LSP 声明时也生效。有 LSP 时共用同一插件目录。不会写入项目或用户配置，不修改原生覆盖优先级，也不是调试器白名单。集中定义删除后保留失效引用供显式修正；保存、选用与装配重新校验，不静默丢弃。

**已配置不等于调试可用。** OMP 默认开放 `debug`（`debug.enabled: true`），Suite 不负责开启或关闭该工具。文件存在不能证明运行库完整、协议兼容或目标含调试符号；运行期失败由 OMP 报告，不自动下载安装或修改 PATH。不选调试器只是不追加配置，原生调试能力仍保留。

#### Windows Rust / CodeLLDB 安装与使用

1. 从 [CodeLLDB v1.12.3 发布页](https://github.com/vadimcn/codelldb/releases/tag/v1.12.3) 下载完整 `codelldb-win32-x64.vsix`，不是 bootstrap 包。按 ZIP 解压，保留整个 `extension` 目录及其 LLDB、DLL 和脚本，不要只复制 exe。
2. 在本机根配置 `[roots]` 中设置 `codelldb = 'E:/实际解压目录'`，使 `${roots.codelldb}/extension/adapter/codelldb.exe` 指向真实程序。`paths.example.toml` 仅给出注释示例，不代表已安装。修改根配置后重启服务。
3. 打开 OMP Suite，选择 CodeLLDB 并保存。使用该 Suite 发起新的 Chat（已有进程不热更新）。
4. 用带调试符号的 Rust 构建产物，例如 `cargo build` 后的 `target/debug/app.exe`。明确让 OMP 使用 `adapter: "codelldb"`，以该 exe 为 `program`，在源码设置断点，再继续、单步和读取变量；不是以 `.rs` 文件充当可执行程序。

配置链路的定向检查为 `cargo test --manifest-path backend/Cargo.toml --test suites --test suite_assembly dap_`；真实页面选择、详情、保存刷新和失效引用修复由 `npm --prefix frontend run test:browser:build -- suiteDap.spec.ts --workers=1` 验证。两者不启动 CodeLLDB，不能代替下面的原生调试验收。

交付配置覆盖 `args: []` 与 `connectMode: "stdio"`：CodeLLDB 不传端口参数时使用 stdio，不能保留 OMP 默认 `--port 0` 却按 stdio 连接。`launchDefaults.terminal: "console"` 使用内部控制台，避免无交互终端的 OMP 调试会话尝试附着 Windows 控制台而失败。Rust MSVC 的 PDB 可用，但 enum 解码存在限制。本次交付支持范围为 Windows x64 CodeLLDB/Rust；目录虽可声明其他适配器，未因此验证其他语言，也未接入 Java。验收需实际命中源码断点、单步并读取预期变量；目录显示、路径存在和启动受理均不替代该证据。

### Task 单 Suite 选择与引用保护

Task 编辑第二步提供 **Suite（至多一个）** 单选，只提供当前 agent 兼容的 Suite 和“不选 Suite”。多个 Task 可以引用同一个 Suite；引用保存为 Task 的可选 `suite` 字符串（稳定 Suite ID），不复制 Suite 内容。`null` 或省略表示明确不选，数组、多选、不存在的 ID、空字符串或不兼容的选择会被直接 HTTP 拒绝。

- Task 列表与编辑器重新读取当前权威定义和集中 MCP 目录；失效引用仍显示原 ID 及原因。切换 agent 不会偷偷清空或改选 Suite，不兼容的原选择保留为不可用项，必须显式修正。
- Task 保存及发起 Chat 前均重新验证 Suite 存在、宿主兼容和集中 MCP 引用。发起后新进程恢复与 workflow manager/node 启动也验证来源 Task 的 Suite 引用。验证失败会明确拒绝，不以空环境继续，不删除引用；原生 MCP 的运行期连接状态仍由宿主处理，不新增“所有 MCP 已就绪”门禁。
- Suite 不替换 Task 的初始 Skill、固定 prompt、模型、思考级别或命名规则。修改定义不会主动重配已运行的 Chat。
- 删除仍被引用的 Suite 返回 `409 suite-in-use`，界面列出引用它的 Task 名称与稳定 ID。先显式编辑或删除这些 Task，再删除 Suite；拒绝后 Suite 和所有 Task 引用保持不变。读取 Task 失败时也不会冒险删除 Suite。
- 同一进程内新增／修改 Task 引用与删除 Suite 共用既有文件协调范围，不能同时成功留下悬空引用。手工编辑在下一次读取／保存／启动时重新检查；不承诺跨文件事务或多进程写入协议。

`GET /api/tasks` 的 `options.suites` 提供当前 Suite 定义，`options.mcp` 提供集中 ID 清单；Task POST/PUT 的 `suite` 字段与读取字段一致。引用校验失败返回 `400 task-suite-invalid` 和具体原因。读取失效 Task 本身仍成功，以便保留并显式修正引用。

公开 HTTP 入口：`GET /api/suites` 返回 `{suites, mcp, dap, fixedExtensions}`（`mcp` 为集中 ID 清单，`dap` 为调试器详情数组，每项含 `id`、`name`、`description`、原生 `config`；`fixedExtensions` 为只读固定目录，条目含 `id`、`label`、`agent`、`kind: "fixed"`、解析后的 `entrypoint` 和 `roles`，角色值为 `standalone`／`manager`／`node`）；`POST /api/suites` 创建；`GET /api/suites/{id}` 返回 `{suite}`；`PUT /api/suites/{id}` 修改同一 ID；`DELETE /api/suites/{id}` 删除。创建、更新与删除成功均返回相同的 `{suites, mcp, dap, fixedExtensions}` 列表形状。保存请求体使用上例的单个 Suite 对象，不接受查询参数或未知字段，删除不接受请求体。错误以 JSON `code` / `message` 返回；只有成功写入权威文件才显示已保存。

定义管理的可复跑隔离验收为 `cargo test --manifest-path backend/Cargo.toml --test suites` 与 `npm --prefix frontend run test:browser:build -- suites.spec.ts --workers=1`。Task 引用完整性验收为 `cargo test --manifest-path backend/Cargo.toml --test suite_references` 与 `npm --prefix frontend run test:browser:build -- suiteReferences.spec.ts --workers=1`，覆盖单选及复用、集中引用失效后的读回／保存／启动拒绝、删除保护及受控并发请求；启动负例使用独占 Herdr 记录夹具确认没有外部启动调用，不宣称原生资源消费通过。浏览器用例独占临时配置、声明、动态回环端口和后端进程，真实操作页面并独立读取文件；包含外部修改及实际进程重启，不需要用户凭据或原生 agent 启动。它们证明定义管理和模式持久化，不证明原生 agent 已消费资源。

### 当前交付定义与旧格式更新

交付的 `meta-data/suites.json` 和 `meta-data/mcp.json` 是当前默认权威定义，Task 通过稳定 ID 引用 Suite。当前清单为：

| Suite ID | harness | 资源职责 |
| --- | --- | --- |
| `codebase-exploration-omp` | OMP | 探索技能集合、扩展、四语言 LSP 与集中 MCP |
| `codebase-exploration-pi` | Pi | 逐技能探索、扩展、集中 MCP，codemode 为 on |
| `code-implementation-omp` | OMP | 实现技能集合、扩展、四语言 LSP 与集中 MCP |
| `web-access-pi` | Pi | 独立网页访问扩展，codemode 为 on |
| `pstack-omp` | OMP | pstack 生成技能集合、持续模式入口及 persona 生成目录 |
| `pi-codemode-pi` | Pi | codemode only |

OMP 集合新增有效直接子技能会参与后续发现，不承诺固定技能数量。探索的两种 Suite 独立选择同一集中 `codebase-memory-mcp` 定义；Pi 不含 LSP。实际资源发现与命令消费仍须对应宿主验收，不能从声明或迁移成功推断。

`pstack-omp` 不叠加探索／实现 Suite，也不替代 Task 固定扩展或项目指令。将 `[roots].toolkit` 指向含 `pstack-suite` 交付资源的 toolkit，Task 选择该 Suite 后发起 Chat。更新生成资源前先结束使用同一包的 pstack 会话，按 toolkit 的 `pstack-suite/README.md` 操作；发起不负责生成或更新资源。

`web-access-pi` 与 `codebase-exploration-pi` 的 `extensions` 均直接声明 `${roots.pi_extensions}/pi-web-access/index.ts`，能力不是某个 Suite 专属的开关，不从包 manifest 推导入口。依赖须事先准备，启动不执行 npm 安装或更新。网页访问 Suite 的技能、模板、集中 MCP 选择为空，仍保留 Task 固定扩展与原生 MCP 合并行为，不改变 Task 模型、思考级别或初始提示词。`pi-web-access` 源码包 `0.41.0` 的工具指引和内部模型提示由扩展提供；注册不证明工具全部 active、搜索服务或凭据可用。源码入口与发布包 `./dist` 不可直接互换。

### 从多宿主 Suite 一次性切换

旧格式不兼容，启动不会自动转换。先停止目标部署全部后端写入并备份 Suite 与 Task 权威文件，不从源码默认定义覆盖自定义部署：

1. 按旧 `agents` 将每个 Suite 拆为单 harness 实体。ID 添加 `-omp`／`-pi` 后缀，名称添加 `· OMP`／`· Pi`；先检查新 ID 冲突。
2. 提取旧 `skills[宿主]`、`extensions[宿主]` 为数组，保留原路径语义；独立复制 MCP 选择。OMP 保留 `lsp`；Pi 保留 `prompts`，将 `pi.codemode` 提升为顶层 `codemode`。移除旧 `agents`、资源宿主映射、嵌套 `pi` 及另一宿主的专属字段。不要把 Pi 单技能目录当成 OMP 集合目录。
3. 按每个 Task 原有 agent 与旧 Suite ID 精确映射新 Suite ID，保持 Task ID、agent、名称、类型、模型、初始技能与提示词原样。对无效引用或冲突先明确修复，不猜测映射。
4. 检查其他持久数据是否存在仍被执行逻辑解析的 Suite 引用。Chat 历史登记仅关联来源 Task，无需改写；旧委派预览的冻结事实、聊天正文、提示词及已有启动产物不是当前配置，不做全文替换。原生恢复读取来源 Task 的最新环境。
5. 完成两份权威文件及引用校验后启动新版本，不保留旧 ID 别名；不热更新已有 agent 进程，新配置用于后续发起或新进程恢复。

更早的 `extensionOptions` ID 必须先按其真实来源解析为入口文件／目录，不能猜测或丢弃。Pi 扩展必须是具体源文件；普通相对路径以 Suite 声明文件为基准。已属于固定环境的 `connect-bot` 不重复加入 Suite。

定义管理与引用验收沿用上节隔离入口；测试本身不授权修改其他部署。

## Task 模型与删除

Task 页以紧凑清单展示名称、用途、Agent、模型／思考级别与 Suite。搜索支持名称、Agent、类型、模型、Skill 和 Suite 名称／ID，可与 Agent 筛选组合；计数显示当前匹配数与总数，清除筛选不会修改任何 Task。展开“查看配置与提示词”可读取命名示例、Task 设置与完整 prompt；固定扩展统一在 Suite 页面查看，失效 Suite 的错误不必展开即可看到。

新增与编辑仍经过“Task 预设 → 扩展配置 → 确认保存”三步，只有明确保存并收到后端成功响应才更新共享定义。编辑器自动聚焦显示名、限制键盘焦点在弹窗内，未保存时按 Escape 等同取消，关闭后焦点回到原入口；保存期间不能取消。底部操作保持可达。Task、Suite 与其他入口共用统一导航：`layout=expanded` 保持侧栏，切换页面不改变其他 Tab 的字号、图标边框或导航布局；手机上也可使用全局布局按钮切换至紧凑导航。

Task 的 Codex 模型选项包含 `GPT-6.1-sol`、`GPT-6-luna` 和 `GPT-6-astra`；
思考级别按所选 agent 与模型的能力提供，Pi 的 Luna 支持 `off`，Astra 不支持。
模型目录修改后下次打开编辑、保存或发起 Chat 时重新读取，无需重启。
删除 Task 时先询问是否删除，取消不发出删除请求；删除不影响已经发起的 Chat。

## 从 zone 或 Task 发起 Chat

Zone 行与 Task 行的“发起 Chat”打开同一个弹窗，zone 与 Task 均可重新选择。从 zone 打开时预选该 zone；从 Task 打开时预选该 Task，zone 显示“请选择已登记zone”，必须手动选择，即使只有一个已登记 zone。

首次选择和每次切换 Task 都将 model、thinking 刷新为该 Task 的设置，之后仍可手动修改。选项及联动与新增 Task 共用：切换模型时保留兼容的思考级别，不兼容则重置为“沿用 Agent 默认值”并提示。未指定的参数沿用 Agent 默认值；agent 类型由 Task 决定。

只切换 zone 不重置 model、thinking 或初始 input。重复选择当前 Task 或 zone 不会清除已有名称预览。发起时先校验已保存 Task 的选项，再校验本次覆盖值；损坏的预设须先修正，不能通过本次覆盖值绕过。模型、思考级别及本次名称规则的修改仅用于本次发起，不保存到 Task 或其他配置；关闭后重新打开不保留临时修改。初始 input、名称预览、Herdr Workspace 选择及创建进度仍在同一弹窗中完成。

Task 发起时，当前容器按 zone 根目录名匹配，默认容器按 `[herdr].workspace_name` 匹配；App 使用 Space，TUI 使用 Workspace。所选宿主启动目标后，页面会显示“正在初始化输入”，直到后端确认本次原生会话的输入管线已就绪。初始指示只经该目标终端提交，沿用命令、Skill 和模板的原生交互语义。`inputSubmitted: true` 只表示后端已确认终端提交，不表示模型已读或完成；初始化也不改变会话的在线状态或关机监听。

只有宿主明确证明在写入前因 blocked 拒绝初始指示时，后端才会重试。每次从收到该拒绝起等待至少 1 秒，并在重试前核对原 Tab、进程和原生会话仍匹配；最多重试 5 次（含首次提交共 6 次尝试）。页面在等待时显示本次拒绝的原因，刷新不会重新发起 Chat；目标退出或更换会话／进程时取消，不创建替代 Tab。若第 6 次仍明确被拒绝，则保留现场与最后一次拒绝的原因，不再发送。超时、断线、部分输入、响应不明确或 `started:false` 都可能已经产生输入效果，只显示投递结果未确认，不自动重发。恢复与报告通知不使用此重试规则。

## chief bot 入口

页头 `server` 左侧的 `chief bot` 按钮每次点击都读取后端当前 chief，不使用浏览器本地选择。有当前 chief 的原生身份时，经现有 Resume 路径定位或恢复该 Chat；仍在启动、结果不确定或查询失败时显示反馈，不另建 Chat。

后端没有当前 chief 时，打开既有 Chat 发起窗口，只列出 chief 类型 Task。初始选择为 `chief-bot`、`default/bot-playground`，本次名称为 `chat with chief bot`；不因默认 Task 的命名规则被编辑而更改该入口的默认名称。可以改选 chief Task、已登记 zone、模型、思考级别及本次名称，修改仅用于本次启动，不保存 Task，也不为每次 Chat 复制 chief-bot 定义。Task 与 zone 的原有发起入口保持不变。

默认 zone 未登记时，显示“请选择已登记 zone”，创建按钮保持禁用；必须手动选择有效登记，不自动选用唯一项，也不创建目录或绑定。Herdr Workspace、初始 input 与成功／失败反馈继续使用同一窗口。启动与其他浏览器竞争时以服务端接受／拒绝为准，不自动重试。

交付选项目录为两宿主声明 `omp-chief-chat`／`pi-chief-chat` 固定入口，按 chief 执行角色装配；普通与 manager Task 不加载该角色。chief 未选 Suite 时也适用，公开发起及新进程恢复都从当前来源 Task 和选项目录取得环境；固定加载本身不授予当前 chief 权限。原生只读 `chief_definitions` 查询及角色指引是否实际消费，由跨仓入口专项分别验证，不由扩展清单或启动成功推断。

默认 chief-bot 没有固定 prompt 或初始 Skill，可直接留空 input 发起。对任何 Task，最终初始指示为空时，agent 就绪后显示“创建成功，等待用户输入”，公开创建结果为 `inputSubmitted: false`；不向 Herdr 提交空消息、不补造用户指令。存在固定 prompt、Skill 或本次 input 时仍按原规则提交并确认，之后才显示初始指示已提交。

## 当前 chief 的后端权威

chief Task 经既有 `POST /api/chats` 发起，也经既有 `POST /api/chats/resume` 定位或恢复；所有入口共享同一占位，不按浏览器、zone 或 Task 分区。占位先持久保存再调用 Herdr；chief 启动须在提交初始输入前确认真实原生身份并保存来源。普通会话的名称、客户端声明角色或同名 Task 不授予 chief 资格。

- `GET /api/chief` 返回 `{current: null}` 或 `{current: {claimId, creationId?, sessionId?, phase}}`。`claimId` 是本次授权身份，不是密码；`creationId` 对应既有创建进度，`sessionId` 仅在原生身份已确认或用户明确恢复该历史 Chat 时出现。
- `phase` 为 `starting`（新发起）、`resuming`（恢复占位）、`awaiting-registration`（身份已确认，等待登记）、`registered`（当前登记保留）或 `uncertain`（已尝试的外部效果未确认）。不能把无 `sessionId` 的占位或 `uncertain` 当作“没有 chief”再次发起。
- `POST /api/chief/qualify` 接受 `{sessionId}`，只有当前 `registered` 身份返回 `{current: {...}}`；其余返回 `409 chief-not-current`。委派与报告接收消费者在实际授权点重新判断，不缓存先前的成功回执，不以工具存在或 Task 类型代替当前资格。
- 已有 `POST /api/sessions/events` 的 register / unregister 与维护扫描共同维护资格。离线但仍保留的登记继续占位；超过在线与离线窗口并被扫描清除，或明确 unregister，才释放其授权。旧进程不被强制终止，迟到 register 不会恢复已撤销的 chief 身份；可由用户显式 Resume 重新竞争空位。
- `POST /api/chief/release` 接受当前 `{claimId}`，明确注销该授权，返回 `{current:null, released:{...}}`；陈旧 claim 返回 `409 chief-not-current`。这也用于用户检查不确定的外部现场后明确解除尚未绑定的占位。不会终止进程、删除原生 Chat 或把不确定效果改称未发生；已解除的事实仍保存在权威记录中。

冲突的发起／恢复／登记返回 `409 chief-occupied`，不排队或自动重试。明确的外部启动拒绝释放未绑定占位；超时、不可理解响应或无法观察外部命令结束保留占位与不确定事实。权威存储失败返回 `chief-storage-failed`，不当作空状态继续启动。

权威记录位于 `sessions.history_file` 同目录、以 `chief.json` 替换其扩展名的独立文件（如 `history.json` → `history.chief.json`），使用既有原子持久化。后端重启仍按 ADR-0006 清空会话登记，原当前 chief 可重新登记竞争；已被其他 chief 取代的旧身份不会因重启复权。尚未完成登记的发起／恢复占位跨重启保留，避免把外部结果不明当作未创建。这里只约束后端认可的有效职责，不承诺唯一存活进程，也不迁移对话上下文。

## Task 类型与固定角色扩展

在 task 入口新增或编辑 Task 时，第一步选择普通（`normal`，新建默认）、流程管理者（`manager`）或需求分流者（`chief`）。三种用途均可选择 omp 或 pi；切换用途不改变已选择的合法 agent、模型、思考级别及 Suite 选择。普通 Task 既可从 zone 或 Task 列表独立发起 Chat，也可经 node → Bot → Task 用于节点执行，无需复制成节点专用 Task。manager Task 仍专用于管理者配置，不能绑定给节点执行；chief 不可作为节点执行目标或 Run 管理者，角色不由名称推断。

随仓 `meta-data/tasks.json` 提供唯一默认预设：稳定 ID 与初始名称均为 `chief-bot`，`taskType: "chief"`，Chat 名称规则为 `chat with chief bot`，默认 omp / GPT-6-luna / medium，不选择 Suite 或初始 Skill，固定 prompt 为空。通过既有 `GET /api/tasks`、`GET /api/tasks/{id}`、`POST /api/tasks` 和 `PUT /api/tasks/{id}` 管理，沿用当前模型、思考级别、Suite 与持久化校验。编辑名称不改变其 ID 或用途。该预设是可编辑的定义数据，不是在发起 Chat 时生成的新 Task；自定义或空的存储文件也不会被运行时隐式补入默认预设。定义保存成功不表示 chief 原生扩展、唯一会话权威或模型职责已验收。

角色扩展的适用规则由代码固定：独立发起普通 Task 不加载 manager-chat/node-chat，manager Task 加载 manager-chat；workflow 派发普通工作节点时强制加载对应宿主的 node-chat；显式结束节点例外，启动和恢复均使用普通 Chat 环境，不加载 node-chat。角色扩展不能勾选、取消或混装；Task 编辑器不展示节点角色固定项。其他固定项仍沿用 Task 选项目录；场景资源只由 `suite` 引用提供。Task 不再接受、保存或运行旧 `extensions` 选择，空数组同样被拒绝。

扩展入口位置仍位于 `meta-data/task-options.toml`；代码按稳定 ID `omp-manager-chat`、`pi-manager-chat`、`omp-node-chat`、`pi-node-chat` 识别固定角色，元数据不再支持 `taskType` 适用规则。实际工具、Prompt 和原生扩展运行能力须由 my-agent-toolkit 及宿主环境提供；配置保存或页面展示成功不代表原生加载已验证。缺失或不可用入口在发起时明确报错；Resume 沿既有规则报告扩展降级，不伪称已加载。

需要新进程的 Resume 仍读取来源 Task 的最新会话环境，但由原 Chat 的持久 Run／node／chief 委派关联决定执行角色，包含 retry 前保留的旧执行。子 Run 管理者保留管理者与父节点双角色；结束节点保留执行关联，但与独立普通 Chat 一样不加载节点角色。不能装配会话环境而改用原生默认发现时，反馈会列出未显式装配的原角色扩展；默认发现不代表这些扩展已加载，也不改变原执行归属。Resume 不新建 Run、执行或委派，不重放 trigger config，也不接管或跨宿主迁移 Run。本次不处理运行期间动态修改来源 Task 配置的冲突，也不引入配置快照或热更新协调。

### 共享 Bot 指南业务

空白 Bot 创建、三份指南编写、通用提示词优化和通用文本替换共用[共同业务材料契约](business-materials.md)。目标可为真实 normal Task 或 Map；共同基线由执行者自行通过公开接口核实，用户只需明确目标、业务要求、授权和可选材料。baseline 单份直接交给每个必要消费者，text 是唯一完整正文，handoff 仅记录确认、来源、语义限制和未验证项。独立 Chat 不加载 node 角色也负责授权、配置冲突、真实 revision、运行禁写、非目标保持、安全落盘与完整读回。旧独立 create-bot／create-bot-task 不改变职责；源码交付不自动覆盖用户部署。

既有 Task 型或 Map 型 Bot 可直接使用 `revise-bot-guides`，不必先优化 Task.prompt：只读核实共同基线，三路独立修订并确认，在同一 Chat 整理表达，最终汇合一次合法保存和完整读回。请求文件、可选材料、子管理者及 saved／handoff 交接见[独立修订指南](revise-bot-guides.md)。该能力原位替换原 enhance-existing-bot Map 的强制 Task 优化前置，不修改旧独立入口、真实目标或无关 Map；配置交付不是目标能力已验证。

### 数据格式要求

Suite 专项离线迁移不会补齐更早的数据类型升级；请先使用当前 Task 类型格式。Task 必须显式包含 `taskType: "normal"`、`"manager"` 或 `"chief"`；不接受旧 `node` 类型，也不在运行时自动转换或补类型，缺少类型的 Task 文件会报 `tasks-corrupt`。

使用自定义 `tasks.metadata_file` 时，扩展条目不应包含 `taskType` 适用配置；固定角色保留上述稳定角色 ID、对应 agent、`kind = 'fixed'` 及真实入口路径。节点执行身份与既有 Chat 的恢复语义不因 Task 分类而改变。

## Task 配置与原生存储

Task 预设默认保存于 `meta-data/tasks.json`，选项目录仍为 `meta-data/task-options.toml`；均可通过配置指定其他路径。Task、Bot、Map 作为定义数据允许纳入 Git，页面编辑会修改这些文件；旧数据切换与冲突处理见 [定义布局升级](../README.md#从旧定义布局升级)。

Task 配置分为两组：

- **trigger config（发起配置）**：初始消息、初始 Skill 调用、model 与 thinking，仅在发起 Chat 时应用。
- **env config（会话环境配置）**：Suite 引用及固定扩展；发起 Chat 时加载，Resume 需要新进程时也加载。

初始 Skill 调用是随初始消息提交的动作，不是加载型 Skill；Task 固定 prompt 与本次 input 都属于用户输入，不是系统提示词注入。Task 自身不单独选择加载型 Skill 或系统提示词；加载型资源由所选 Suite 声明提供。

原生会话文件和附属记录由 agent 管理，herdr-bot 不搬动文件或修改 agent 原生配置。发起 Chat 不保证新建原生对话；agent 自身的自动续接设置不属于 herdr-bot 的功能。

### 仅修改 Task.prompt

仅修改既有 Task.prompt 时，使用调用方明确提供的配置服务，结合本文“Task 类型与固定角色扩展”“Task 单 Suite 选择与引用保护”核实：

1. `GET /api/tasks/{URL编码ID}` 取得完整 `task`，与本次修改前的完整配置比较。查询 Bot 目录核实全部绑定及共享影响；目录问题妨碍判断则阻塞，新增共享影响须确认。目标身份不得从名称或 cwd 推断。
2. 按 `docs/workflow-runtime.md` 的“公开接口与持久字段参考”检查真实执行关联及运行使用：目标不是执行者或本流程执行者的 Task，不受 running/paused Run 的配置不变约束。覆盖管理者 Task、保存的执行 taskId 和关联 Map/Bot 的 Task 引用；记录或定义不可读则阻塞。暂停、未接管、节点完成或 Chat 空闲不解除禁写，不操作运行状态绕过。
3. 核实本次仅改 prompt 的保存授权，保存前重读完整配置、绑定与运行使用，变化则停止。Task 接口不提供 Bot 式 revision；使用原配置比较，不自造版本号或宣称跨进程锁。
4. `PUT /api/tasks/{URL编码ID}` 完整提交 name、chatName、agent、taskType、model、thinking、skill、prompt、suite，仅替换 prompt。ID 只用于 URL，suite 原样保留稳定 ID 或 null；不发送已退役 extensions。请求／响应以当前契约为准，不明时核实 `backend/src/tasks/mod.rs` 的 TaskInput 和 `backend/src/lib.rs` 路由；字段不能无损保留则停止。PUT 返回 tasks/options 列表。
5. 再次 GET，核对 prompt 与已确认的目标文本逐字一致、全部非目标字段及绑定不变。失败或效果不明时先查询实际效果，保留错误和部分生效事实，不盲重试或回滚。按本次交付要求提供完整读回配置与核对结论。

适用于独立 Chat 与 workflow 中获准的 Task.prompt 修改；只读核实与保存读回不能证明实际模型或业务效果。

## 历史 Chat Retro

历史会话操作栏的 `Retro` 打开既有发起 Chat 弹窗，不恢复原会话，也不自动提交。Task 默认选择 ID 为 `task-retro` 的预设；缺失时留空提示，允许改选，不自动创建。zone 仅在历史 cwd 与当前登记目录唯一匹配时默认选择；缺失或多个 zone 共用该目录时由用户选择。

初始 input 预填历史 agent 类型、session ID、cwd 与默认 session 根目录，均可编辑。根目录取启动期 `paths_file` 中的 `roots.pi_agent` 或 `roots.omp_agent` 并拼接 `sessions`，按会话保存在默认位置的约定供 Retro 查找；不扫描或读取聊天正文，不要求具体文件地址，不经过提示词词典替换。对应根未配置或 agent 类型未知时，输入标注需补充。修改根配置后需重启后端；本次选择和 input 不修改 Task 预设或历史记录。

## 历史 Chat Resume

在 `chat` 的历史视图中使用 `Resume` 返回 Pi／OMP 原生对话，不要求该 Chat 仍在当前登记中。已有目标只做 Herdr 内部导航；需要新进程时，加载同一原生对话，不自动继续中断任务，也不重放 Task 的发起配置或发送新 prompt。Herdr 内部导航不保证 Windows 窗口置前。

新启动的 omp / pi 会话可能尚未写入原生文件；pi 在首次用户或助手消息出现后才创建文件。herdr-bot 可按[受限原生路径规则](adr/0014-unpersisted-omp-startup-identity.md)保存身份及预期路径，但这不代表文件已落盘或当前可从磁盘恢复。历史文件中的内部 `startupIdentityPath` 仅由可信发起流程写入，普通 register 上报不能设置或覆盖；历史列表不公开该字段。Resume 只对保存的 ID、启动路径与当前官方引用完全吻合的未落盘候选允许定位，不把任意缺失文件当作在线 Chat。实际读取对应文件时若发现身份冲突，明确拒绝，不跳过冲突后启动替代 Chat、不改绑历史；不新增后台落盘检查。原进程已不存在时仍按既有精确 ID 加载及恢复确认流程处理，未确认同一会话不报告成功。

新进程优先使用有效的历史绝对工作目录。目录不可用时，只有 TUI 的 worktree 元数据能把历史 checkout 精确关联到 Git 元数据目录，并核实唯一、有效的主工作树，才回退到该目录；App 不提供这项归属证据，也不会按 Space 或目录名称猜测 Git 归属。无法可靠找回目录时，在配置的默认容器（TUI Workspace／App Space）中新建 Tab，不指定工作目录，由当前宿主选择并显示实际目录，两端的结果可能不同。

使用明确的工作目录时，原容器 ID 只在当前宿主中原样查找；另一端恰好有同值 ID 时也会使用这个匹配。只有 ID 缺失或成功查询确认无匹配时才回退，命令失败或畸形响应不会触发兜底。默认容器必须唯一、精确匹配 `[herdr].workspace_name`，不会使用当前焦点、任取第一项或自动创建容器。选择容器不改变 zone 绑定或历史原位置。

恢复成功前必须核实同一原生会话身份。显式指定目录时，还必须确认实际目录与指定目录相同；交给 Herdr 选目录时，必须观察到有效的实际目录。位置回退与实际工作目录会在页面显示，身份或目录核实失败不会报告成功。Resume 不执行代码 checkout，不弹出目录回退确认，也不改写 herdr-bot 历史记录（包括旧 cwd、Workspace、备忘与来源信息）；回退只影响本次恢复。herdr-bot 不直接改写原生聊天文件，agent 自身恢复时的原生存储行为仍由 agent 管理。

新进程使用来源 Task 的最新环境配置，而不是创建时的快照；没有来源 Task 或来源已删除时，使用 agent 的原生默认扩展配置。修改 Task 不会重新配置已经运行的 Chat。恢复中无法加载的扩展或位置回退会在页面提示。

历史记录支持多行纯文本备忘，电脑与手机共享；备忘不会发送给 agent 或修改原生聊天记录。

若页面显示“启动结果未确认”，外部目标仍可能已经启动。先检查页面提供的目标信息，不要把超时当作未执行；服务不会自动清理或重试。

## Rust script cross repository acceptance

从显式参与 herdr-bot worktree 根运行；这是普通实施票的专项，不增加全局最终 gate，不改变全局票序或 first-merge 依赖调度。两仓的完整检查仍分别执行，不能用专项或人工项冲抵失败回归。

```powershell
$env:CARGO_TARGET_DIR = Join-Path $HERDR_BOT_WT '.scratch/cargo-target'
$env:CARGO_BUILD_JOBS = '1'
# 依赖先各自在所属根按 frozen lock 安装；Bun 1.4.4。
npm --prefix frontend run build
cargo build --locked --manifest-path backend/Cargo.toml --bin herdr-bot --bin herdr-fixture --bin herdr-paths --example safe_shutdown_shell
python "$TESTER/run.py" --herdr-bot-worktree "$BOT" accept-rust-script-bots `
  --herdr-bot-worktree $HERDR_BOT_WT --toolkit-worktree $TOOLKIT_WT `
  --backend (Join-Path $env:CARGO_TARGET_DIR 'debug/herdr-bot.exe') `
  --static-dir (Join-Path $HERDR_BOT_WT 'backend/static') `
  --native-entry (Join-Path $TOOLKIT_WT 'scripts/rust-script-acceptance.ts') `
  --isolation-config $ISOLATION_JSON --evidence-dir $NEW_EVIDENCE_DIR
```

所有参数为绝对路径；证据目录必须不存在。`ISOLATION_JSON` 必须包含 `externalMode:"controlled-protocol-only"`，以及绝对安装路径 `bun`、`node`、`piCli`、`piNode`、`ompBaselineRecordRoot`、`chrome`。`piCli` 指实际安装 Pi 的 CLI JS，不是自动发现认证的启动器；`piNode` 为满足该安装 engines 的已安装 Node（Windows 已有 Node 22.19.0 可用于规避高版本退出竞态）。`ompBaselineRecordRoot` 只解析既有 baseline 的只读源码锚点，不替换本 worktree 扩展源码。先为本 toolkit worktree 准备其既有精确 OMP dock 安装，不能使用另一 worktree 的遗留产物。配置不含模型密钥，不发现用户认证、不联系正常服务；受控 provider 不是实际模型理解证据。

入口拒绝错误 worktree、原生入口、静态页、缺失或过期产物及 Cargo depfile 来源不匹配；保留源码／锁文件、页面、后端／fixture／安全壳／resolver、安装宿主身份和完整参数。依次执行原 owner 03 的 P15、P23 真实页面专项、10 的 `bun test tests/harness-dock/manager-script-protocol.test.ts`，再执行两宿主生产后端业务组合。每阶段独立记录命令、cwd、stdout、stderr、退出码，不因前一阶段失败略去其余可运行阶段；旧证据不覆盖。`RUST_SCRIPT_TOOLKIT=$TOOLKIT_WT python "$TESTER/scripts/accept-rust-script-bots.test.py"` 只验证入口安全，不证明业务。

业务组合保持交付八节点 Map 的节点、依赖和材料规则，只隔离 zone 和外部编写者的 Task 绑定。实际 Rust 创建和保存均由生产 `herdr-bot` 执行；没有运行时业务解释器或独立业务 exe。manager 使用正常 OMP/Pi auto 入口、实际 run／collect／read，外部编写者只提供明确标记的受控 `abc`／`hello`／空串材料；真实 Rust 保存、完整 Bot／目标、revision、baseline／saved／handoff、execution、报告、完成记录和原生读取结果均独立读回。它不证明三编写者真实模型遵从，不能替代其既有原生验收。

部分失败组合先真实保存成功，再用独占文件碰撞阻止 handoff，观察 saved 仍可读、false 报告、write/readBack=succeeded、complete 拒绝。只解除碰撞，实际 manager 原生工具显式 fail 和 confirmed retry；新 execution 仍使用原 baseline，必须 revision-conflict，旧报告、旧 outputs、原保存效果保持。此为 05 通路上的组合演示，不复制 03 或 05 的业务／通用门禁测试。原 owner 03 的 P15 另行直接运行。

每例独占 HOME/APPDATA/XDG、原生目录、定义、Run、文件和数值回环服务，日志持续消费。保留原始错误与 cleanupErrors，先等待原生宿主和后端／fixture 的退出确认再清理；证据副本保留，临时活目录删除。脚本没有关机调用，生产后端不运行关机回归；两仓全局 dock 的 `HERDR_BOT_BINARY` 始终指本次 `safe_shutdown_shell.exe`，`HERDR_BOT_METADATA` 指本次 `meta-data/task-options.toml`。

### H1／H2：只由用户确认

自动化全部完成后才能开始最终人工判断；缺少真实宿主／模型授权不妨碍实现、机械验收及证据准备，但 H1/P34、H2/P35 始终保持 unverified。自动化入口返回 0 只表示 automatedPassed，不表示这两项通过。不要把 Rust 确定性检查转成模型重新审核三段文本的任务。

授权前准备独占真实 Herdr Workspace、manager Task／会话、隔离后端配置、文件根、HOME/APPDATA/XDG 和数字回环 endpoint；用户明确提供真实 Herdr 安装、只读 paths、manager 的模型配置与许可边界。不得沿用自动化的 `herdr-fixture`、controlled provider 或删除过的临时路径；不得自动发现用户凭据／正常 Chat。使用参与 worktree 的生产后端和完整交付定义，按既有配置将所有可写路径放入新根，`static_directory` 指本次静态页，manager-chat 配置只指该 endpoint。保存实际配置／文件和构建哈希；工具预算沿用交付配置，记录观察预算，不延长产品门禁。没有明确授权时，将这些具体缺项写入人工记录，不伪造会话已建立。

H1 操作顺序：

1. 用户在真实 manager 中授权只在隔离定义创建一个既有 normal Task／Map 的 Bot，不授权执行目标业务；由交付创建 Map 正常发起真实 Run。保留完整 request、目标、manager 身份及真实原生工具入参与结果。
2. 真实 manager 调用原 `run action=dispatch nodes=["create_blank"]`。观察返回只表示派发，未把脚本成功说成已 complete。保存派发返回、执行在途／结束读回、Herdr Tab 清单；无脚本 Chat／Tab，manager Chat 正常存在。
3. 观察该 execution 的后端真实报告到达 manager；保留原消息、报告 ID／来源／canComplete／outputs、collect 返回以及原生 read 对命名 baseline／handoff 的读取。缺通知时 collect 读回事实不能冒充已收到通知。
4. 对当前 true 报告显式 complete；按原流程完成真实三路正文确认和表达优化，最终显式派发 save_bot。对第二能力重复派发返回、报告接收、collect、命名 saved／handoff 读取和显式 complete。以公开完整 Bot／revision／目标和 Run 完成记录交叉核对；不运行创建的目标业务、不新增脚本接收工具。
5. 用户评价真实 manager 是否准确解释派发、报告、部分限制和交接；保留原生历史、截图、工具结果和用户结论。自动化机械事实不要求模型二次语义验收。

H2 操作顺序：

1. 在另一个独占 Run 重复创建和三路材料准备。用已有 `scriptFileGate` 的真实文件门闩暂停保存前读取；取得当前 save_bot execution ID 后，在 `<run.zone>/.manager-zone/.workflow/<run.id>/<execution.id>/handoff.json` 写入本例拥有的碰撞文件，然后释放门闩。不得损坏用户文件。自动化同名 partial-save-retry 场景提供可重复机制、完整请求和结果对照。
2. 真实 manager 接收 false 报告，公开完整读回确认 Bot 已保存、revision 已变化、saved output 可消费；材料失败不是未写入，也不能 complete。保存真实工具拒绝，观察没有自动 fail、retry、回滚或刷新 baseline。
3. 用户只删除本例碰撞文件；真实 manager 显式 `run action=fail nodeId="save_bot" reason=...`，再经用户确认 `run action=retry nodeId="save_bot" confirmed=true reason=...`。保留新旧 execution、报告、outputs、完整 Bot 和 baseline 字节。相同正文不豁免旧 baseline：新执行必须 revision-conflict，旧报告不能完成新执行。
4. 若要继续真正修复，在用户另行授权的新流程建立新的有效基线，不覆盖原失败证据、不自动推断回滚许可；保留真实修复后的独立执行与报告、显式 complete 结果。用户最终确认真实 manager 对失败及重跑的理解。

人工证据最少包含：实际宿主／模型／构建身份、完整隔离配置与授权边界、原始工具输入／结果和通知、完整公开定义及 revision、真实文件内容／绑定、当前和旧 execution／报告／outputs、完成／拒绝记录、无脚本 Chat／Tab 的实际观察、截图、预算、所属进程退出确认与独立清理错误。不能以受控原生协议或全局回归代签 H1／H2。
