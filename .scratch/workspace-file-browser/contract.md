# Contract — workspace 文件浏览器与整仓 Git 提交同步

Source: [spec](spec.md)
Acceptance mode: Rapid iteration
Approval: 接缝选择、约束审视、验收模式三个审批门均已由用户明确确认。

本文是已批准的决策契约，不表示功能已实现或验收已完成。承诺的完成状态由交付工作、自动检查结果及必要的用户确认推导，不手工维护状态列；交付归属由 `/to-tickets` 回填到实现票并引用承诺编号。

> **2026-10-20 修订**：Git workspace 的浏览树改为按 Git 原生忽略规则省略 ignored 未跟踪路径，并省略 `.git` 元数据；普通目录仍完整反映磁盘。`.gitignore`、未被忽略的隐藏文件和 tracked 文件继续可见。目录树只在进入 workspace 与手动刷新时读取；文件事件和返回页面只更新 Git 状态。P12、P17、P19、P39–P41 已按此行为改写。

## Promises

Coverage 中的“自动”由 agent 负责，包括真实浏览器与隔离环境验证；不是仅运行模拟检查。唯一人工例外为手机实机交互体验，在对应 Coverage 单元格标记为 **human-led H1**。同一 H1 的检查和证据按相关单元格合并交付，不扩展成人工验证 Git 正确性或人工修复实现的责任。

**Complete implementation and all assigned automated checks before the human-led acceptance round. Planned human-led checks do not pause otherwise executable work. At the final handoff, report the remaining checks together with their prepared evidence; they remain unverified until the user confirms them. Report genuine blockers when encountered.**

|#|Promise (one observable result)|Coverage|Source|Seam|
|---|---|---|---|---|
|P1|顶层顺序为会话列表、文件列表、控制面板。|自动：真实页面入口顺序与交互。|[story 1](spec.md#user-stories)|S1|
|P2|每次打开和整页刷新默认会话列表，不重放 Git 写操作。|自动：打开、刷新及操作中重载；独立核对提交与远端引用，不能以显示一次成功提示代替副作用次数。|[stories 2,8](spec.md#user-stories)|S1,S2|
|P3|原有固定操作、会话与全局关机行为不因文件功能而改变。|自动：已有行为回归与跨 tab 全局提醒；不执行真实关机。|[stories 3,72](spec.md#user-stories)；ADR-0004/0005/0006|S1,S2|
|P4|三个顶层入口等宽，具有明确选中态、焦点与键盘交互，不以左右滑动切换。|自动：真实桌面与窄屏浏览器、方向键及 Tab/Enter/Space；human-led H1：真实手机触摸目标是否可用；agent 准备窄屏截图、访问步骤与触摸检查项。|[stories 4,5](spec.md#user-stories)；[Implementation Decisions](spec.md#implementation-decisions)|S1|
|P5|当前页面内往返 tab 或视图保留 workspace、展开状态、文件选择、滚动及提交信息。|自动：顶层与内部视图往返，对独立指定的原位置与输入检查；整页刷新不要求恢复这些临时状态。|[story 6](spec.md#user-stories)；[Implementation Decisions](spec.md#implementation-decisions)|S1|
|P6|切换 tab 不取消后端已接受的 Git 操作，返回可观察实际状态与结果。|自动：操作执行与视图切换交错；核对真实仓库结果及返回页面反馈。|[story 7](spec.md#user-stories)|S1,S2|
|P7|workspace 登记指向服务器上的现有普通目录或 Git 工作树根目录。|自动：隔离服务器目录登记、路径输入校验与后续读取；不用客户端目录选择代替服务器目录。|[stories 9,10,14,16](spec.md#user-stories)|S2|
|P8|workspace 登记列表跨设备共享且跨后端重启保留。|自动：两个独立调用者读取同一列表，真实进程重启后再次读取；不能以同页缓存证明持久化。|[stories 11,12](spec.md#user-stories)|S2|
|P9|取消登记不删除目录或文件。|自动：取消前后独立读取原目录与文件内容。|[story 13](spec.md#user-stories)|S2|
|P10|普通目录只有可用的浏览能力，没有可执行的 Git 提交同步。|自动：页面无可用 Git 操作；直接请求不能绕过资格检查产生 Git 写入。|[stories 15,18](spec.md#user-stories)；inferred-required：不能仅依赖按钮限制|S1,S2|
|P11|整仓操作不递归管理嵌套仓库或子模块内部变更。|自动：真实隔离嵌套仓库与子模块前后状态；子模块内部文件不作为父仓库普通文件提交，父仓库自身已跟踪的 gitlink 引用变化仍正常纳入。|[story 16](spec.md#user-stories)；[Implementation Decisions](spec.md#implementation-decisions)；inferred-required：区分父仓库引用与子仓库内容|S2|
|P12|Git workspace 浏览树按 Git 原生忽略规则省略 ignored 未跟踪路径，并省略 `.git` 元数据；普通目录完整反映磁盘。两种 workspace 都保留隐藏文件、忽略规则文件、tracked 文件和空目录。|自动：真实 Git 仓库核对 Git 忽略集合与页面；普通目录与独立创建的真实目录集合核对；`.gitignore`、隐藏文件、tracked 文件和空目录保留，`.git` 不出现。|[stories 19,20](spec.md#user-stories)|S1,S2|
|P13|浏览与审查不改变文件内容或真实暂存状态。|自动：读取、diff 和重命名展示前后，独立核对文件与索引状态；允许 Git 正常只读查询所需的非语义缓存活动，不把它当作暂存操作。|[stories 20,37,46](spec.md#user-stories)；inferred-required：只读含暂存语义不变|S2|
|P14|任何公开读取都不能越出所选已登记 workspace。|自动：直接请求根外路径、遍历路径、符号链接、Windows 目录联接和路径替换反例；根外链接显示目标与不可进入原因，不读取其内容。|[story 21](spec.md#user-stories)；inferred-required：约束作用于实际读取入口|S1,S2|
|P15|文件新增绿色、修改及已识别重命名蓝色、删除红色，不显示普通 A/M/D 状态字母。|自动：实际浏览器的明确样例，颜色与独立指定状态对应；两个视图一致。|[stories 22,23,35,36,39](spec.md#user-stories)|S1|
|P16|包含待提交变更的现存祖先目录显示蓝色，传播到 workspace 根下一级。|自动：多层新增及删除样例；即使文件已删除，存在的祖先仍提示变化；不将所有目录染成子文件颜色。|[stories 24,28](spec.md#user-stories)|S1,S2|
|P17|被忽略的未跟踪路径不出现在浏览树；忽略规则文件自身按普通文件与普通变更处理。|自动：真实忽略规则、嵌套规则、tracked 文件和 ignored 目录样例，独立核对 Git 集合及浏览树。|[stories 25,26](spec.md#user-stories)|S1,S2|
|P18|删除文件只出现在变更列表，不在浏览树伪造仍存在的文件。|自动：真实删除后的两视图对照。|[story 27](spec.md#user-stories)|S1,S2|
|P19|`.git` 元数据不出现在 Git workspace 浏览树，也不计入普通待提交着色、目录变更提示及角标。|自动：仅元数据变化的明确样例，确认 `.git` 不出现且元数据变化不改变浏览树、着色或角标；普通目录中同名文件不受影响。|[story 29](spec.md#user-stories)|S1,S2|
|P20|待提交集合准确遵循 Git 跟踪与忽略规则。|自动：真实 Git 样例覆盖 tracked 修改/删除、未忽略 untracked、ignored untracked、后来匹配忽略的 tracked；最终提交树与独立预期核对，不强添 ignored。|[stories 30,31,32](spec.md#user-stories)|S2|
|P21|通常的 diff 展示当前本地分支最新提交到本地最终内容的整体差异。|自动：独立写定旧新文本与部分暂存仓库，区分 HEAD→最终内容和 index→工作区、远端→本地；无首次提交时适用 P47。|[story 33](spec.md#user-stories)|S1,S2|
|P22|外部切换分支后，刷新所见分支与重新确认的操作目标一致。|自动：真实外部切换分支，页面显示更新，旧审查不能向另一分支提交。|[story 34](spec.md#user-stories)|S1,S2|
|P23|重命名按 Git 当前识别结果显示，不为展示修改索引或自创配对。|自动：直接改名未暂存与已暂存改名的真实样例；识别时显示旧路径→新路径蓝色项，否则保持删除与新增。|[stories 36,37](spec.md#user-stories)|S1,S2|
|P24|workspace 角标按待提交变更项计数，而非目录或 diff 行数。|自动：独立固定期望——同文件暂存加未暂存一次、已识别重命名一次、未识别删加两次；接受 Git 检测变化造成项数变化。|[stories 17,38](spec.md#user-stories)|S1,S2|
|P25|电脑使用左树或列表、右内容布局，diff 默认左右对照。|自动：真实宽屏浏览器截图及文件选择、滚动交互，不仅验证 DOM 存在。|[stories 40,41](spec.md#user-stories)|S1|
|P26|手机使用逐层进入与明确返回，diff 默认统一显示并保留列表上下文。|自动：真实窄屏浏览器验证导航、diff 和滚动恢复；human-led H1：手机实机检查触摸、diff 滚动与返回是否顺手；agent 准备窄屏截图、局域网访问步骤与复现清单，不能以尺寸模拟冒充实机通过。|[stories 42,43](spec.md#user-stories)|S1|
|P27|变更列表上方常驻提交信息和主按钮，Commit 要求非空信息。|自动：页面与直接请求均拒绝空信息；human-led H1：真实手机软键盘弹出后输入框、按钮是否仍可访问；agent 准备输入及键盘检查步骤、窄屏布局证据。|[stories 44,45](spec.md#user-stories)；inferred-required：后端也拒绝无效提交意图|S1,S2|
|P28|Commit 与 Sync 仅使用一句消息的确认/取消门禁，取消不产生 Git 写入。|自动：确认与取消的用户交互，真实仓库前后状态；human-led H1：手机实机确认/取消触摸目标可用；agent 准备操作步骤及弹框截图，不把人工范围扩大为 Git 语义验收。|[stories 45,46](spec.md#user-stories)；[Implementation Decisions](spec.md#implementation-decisions)|S1,S2|
|P29|确认后的提交重新核对已展示审查版本，任何分支、HEAD、索引、文件集合或内容差异阻止当次提交。|自动：同路径同数量内容变化、外部暂存、切分支、超限与二进制内容变化等反例；预览限额不得截断复核。保留规格已接受的外部并发竞争窗口，不承诺原子锁住所有外部进程。|[story 47](spec.md#user-stories)；inferred-required：完整内容检查不限于可预览内容|S2|
|P30|失效审查有提示并刷新，提交信息保留，必须重新确认。|自动：失效响应后的实际页面状态及再次点击流程。|[story 48](spec.md#user-stories)|S1,S2|
|P31|Commit 暂存全部符合规则的最终变更后提交，不受既有部分暂存缩小范围，不自动推送。|自动：真实提交树、本地历史与隔离远端引用独立检查；不能只断言命令返回成功。|[stories 49,50,51](spec.md#user-stories)|S2|
|P32|本实例对同一实际仓库的 Git 写操作串行。|自动：不同调用者及路径别名并发，验证无重叠写入；断开或超时但执行尚未确认停止时，不释放资格允许重叠。资格不混同固定操作已有执行资格。|[story 52](spec.md#user-stories)；inferred-required：别名与进程所有权不能绕过串行|S2|
|P33|主按钮准确区分 Commit、Sync、已同步、远端未检查、缺 upstream 和阻塞状态。|自动：逐态真实 Git 事实到页面映射；工作区干净、有 upstream 且已知有 ahead/behind 时才给可执行 Sync，已同步仍可检查远端。|[stories 53,54](spec.md#user-stories)；[Implementation Decisions](spec.md#implementation-decisions)|S1,S2|
|P34|Sync 只处理干净工作区并遵循当前分支现有 upstream 与 pull 配置。|自动：非默认远端/跟踪分支、merge/rebase/ff-only 策略、脏目录直接请求；不 stash、不隐式 commit、不改配置、不强推。|[stories 55,56,57](spec.md#user-stories)|S2|
|P35|Sync 检查远端后 pull、再按需 push，任一步失败停止后续步骤并显示真实诊断。|自动：隔离 bare 远端和配置拒绝、冲突、推送拒绝样例，独立核对引用与调用后果，检查页面纯文本错误；只有 behind 时不要求无意义推送。|[stories 56,58](spec.md#user-stories)|S1,S2|
|P36|后续失败不自动撤销已成功步骤，也不宣称流程整体回滚。|自动：暂存后 hook 拒绝 commit、pull 成功后远端拒绝 push，核对实际索引、提交和引用与页面反馈。|[story 59](spec.md#user-stories)|S1,S2|
|P37|冲突或未完成 Git 流程阻止普通 Commit/Sync，外部完成后刷新恢复。|自动：真实 merge/rebase 等冲突及已解决但流程未完样例，直接请求同样拒绝；普通命令失败不误标为文件冲突。|[stories 60,61](spec.md#user-stories)；inferred-required：后端资格检查|S1,S2|
|P38|detached HEAD 与无 upstream 明确提示限制，不隐式修复配置或发布分支。|自动：真实仓库状态、按钮反馈与操作前后配置不变；无 upstream 不妨碍正常本地 Commit。|[story 62](spec.md#user-stories)；[Implementation Decisions](spec.md#implementation-decisions)|S1,S2|
|P39|工作区与相关 Git 元数据的文件事件触发 Git 状态更新；目录树快照只在进入 workspace 与手动刷新时重读。|自动：真实进程、真实目录写入与 Git 外部操作，观察 Git 状态更新且目录树请求数不增加；模拟通知不代替监听实际生效证据。|[story 63](spec.md#user-stories)；[Implementation Decisions](spec.md#implementation-decisions)|S2|
|P40|每 workspace 的 Git 状态自动刷新默认最小间隔 10 秒且可配置，事件合并后仍最终刷新；目录树不参与自动刷新。|自动：独立事件时序覆盖窗口首尾、连续写入、多个观察者；可控时间证明节流，真实进程证明监听与目录树请求数不变；不把最小间隔错误承诺为 Git 查询必在 10 秒内完成。|[stories 64,65](spec.md#user-stories)|S2|
|P41|进入 workspace 读取一次完整目录树；进入或回到页面、操作后及提交前检查不受自动刷新缓存节流限制，但只有手动刷新会重读目录树。|自动：窗口内交错操作，观察 Git 状态与预提交检查实际结果，并核对只有手动刷新增加整树请求；而非只断言刷新函数被调用。|[story 66](spec.md#user-stories)；[Implementation Decisions](spec.md#implementation-decisions)|S1,S2|
|P42|远端按需检查，Sync 前必查，并明确最近检查时间而非宣称实时。|自动：隔离远端独立前进、本地引用和页面检查时间；本地文件事件不造成持续自动 fetch。|[story 67](spec.md#user-stories)|S1,S2|
|P43|按内容识别文本且只作纯文本展示，不强制后缀白名单或执行内容。|自动：无后缀文本、伪文本后缀二进制、含 HTML/script 的文件及 Git 诊断，实际浏览器确认不执行。|[story 68](spec.md#user-stories)|S1,S2|
|P44|文本与 diff 预览使用可配置大小上限，默认 1 MiB；diff 两侧各自适用。|自动：精确边界及越界字节大小，旧侧超限/新侧很小及相反情况；任意一侧超限不渲染完整 diff，明确说明，二进制给出不可文本预览原因。|[stories 69,70](spec.md#user-stories)；已批准 B3|S1,S2|
|P45|预览不可用不改变文件可见性、着色、变更计数或提交资格。|自动：超限与二进制的页面、集合和真实提交树，预览失败不静默遗漏文件。|[story 70](spec.md#user-stories)|S1,S2|
|P46|可信局域网可以使用约定浏览与 Git 操作，但没有任意命令入口且会话同机写入限制不变。|自动：公开交互的操作范围、原来源拒绝回归与跨调用者访问；不向用户业务远端推送。|[stories 71,72](spec.md#user-stories)；ADR-0004|S2|
|P47|已初始化但没有首次提交的 Git workspace 可在变更视图执行首次 Commit。|自动：真实无 HEAD 仓库，以空树比较，符合待提交规则的已暂存和未忽略未跟踪文件均绿色；确认 Commit 后独立验证首个提交树；不添加 git init 功能。|已批准 B1；[stories 22,30,33,49](spec.md#user-stories)|S1,S2|
|P48|Git 操作不新开终端交互流程或网页凭据流程。|自动：沿用无需输入的现有身份与凭据；需交互的编辑器、凭据或签名条件明确失败或按 P49 超时，不覆盖 Git 策略以伪造成功。|已批准 B2；[Implementation Decisions](spec.md#implementation-decisions)|S1,S2|
|P49|每条 Git 命令默认最多执行 300 秒且可配置，超时停止本次执行并中断后续步骤。|自动：隔离的阻塞执行和派生执行、较短配置与默认加载，验证终止结果、不回滚已生效状态；终止未确认时按 P32 保持资格。页面标明本功能超时及已有 Git 输出，不伪造 Git 退出消息。|已批准 B2|S1,S2|
|P50|登记成功意味着持久化成功，登记读取或写入失败不被伪装为成功或空列表覆盖。|自动：隔离存储写失败、损坏内容、再次读取与重启，核对原内容未被静默替换。|inferred-required；[stories 9,11,12](spec.md#user-stories)|S1,S2|
|P51|目录消失、不可访问或读取失败不被当作空目录或干净仓库。|自动：外部删除或改动可访问条件后的读取、按钮反馈与写入拒绝；错误明确可见。|inferred-required；[stories 19,20,47,57](spec.md#user-stories)|S1,S2|
|P52|监听失效不会继续被显示为自动更新正常。|自动：可复现的监听失败条件与真实进程结果，明确错误或过期提示，仍允许实际可用的手动查询；不偷偷增加周期 fetch 或宣称恢复成功。|inferred-required；[stories 63,65,66](spec.md#user-stories)|S1,S2|
|P53|失去操作响应不被当作操作未执行或成功的证据。|自动：请求观察中断后的实际状态查询、无自动重放；页面给出未确认反馈而不是假成功，仍执行 P6/P32。|inferred-required；[stories 7,8,58,59](spec.md#user-stories)|S1,S2|
|P54|缺省新配置保留已批准默认值，非法配置明确拒绝而非静默回退。|自动：旧配置缺段仍可启动，10 秒、1 MiB、300 秒的默认与合法覆盖；无效类型或不合法值明确失败。具体字段名与配置结构不作为固定承诺。|inferred-required：配置承诺沿用既有启动配置惯例；[stories 64,69](spec.md#user-stories)；已批准 B2|S2|

## Seam decisions

|Seam|Exposes|Hides|Alternatives considered|Reversibility|
|---|---|---|---|---|
|S1 — 主界面公开交互 interface|真实页面的导航、内容、颜色、可访问性状态、响应式布局、确认与错误反馈；复用已有主界面交互观察，扩展 workspace 功能。|私有视图拆分、状态管理、目录计算及传输装配；真实传输与确定性场景 adapter 可替换，不把它们再列为契约接缝。|只看 S2 无法证明移动布局、颜色或触摸体验；每个私有视图各立接缝会增加覆盖重叠并约束内部重构，均不采用。|内部重构不改变契约；未来新增独立客户端时应重新评估其交互承诺，不要求复用浏览器私有实现。|
|S2 — 交付进程公开交互 interface|启动配置、公开读取/操作/更新及实际文件与 Git 副作用；复用已有公开请求、真实进程和观察验证能力，并补充 workspace interface。|登记格式、Git 封装、监听/节流、仓库资格与进程管理的内部结构；只读查询与副作用通过同一外部观察串联。|只看 S1 的“成功”文案不能证明提交树或远端引用；给每条 Git 命令增加契约接缝会绑定实现步骤；单独立持久化/监听接缝则降低观察高度，均不采用。|具体公开消息形状可按项目惯例确定；一旦存在真实调用者，变更需要协调 adapter，但本契约不钉死内部模块或传输协议。|

### 依赖分类与证据

- **In-process**：颜色、状态选择和计数等留在 implementation 中，不为私有计算新增契约接缝。
- **Local-substitutable**：文件系统、真实 Git、本地隔离 bare 远端与时间输入。真实隔离资源验证外部语义，可控时间验证节流时序；不能以文件或事件替身代替 Windows 链接/联接与真实监听证据。
- **Remote but owned**：浏览器到本后端的传输。真实传输 adapter 与确定性场景 adapter 有不同实际用途；无需提前为单一 Git 包装创造 hypothetical seam。
- **True external**：托管远端、凭据及用户第三方配置。同步语义在隔离远端验证，既有凭据在可用环境中使用；本契约不要求访问某家托管平台，也不允许为验收向真实业务远端推送。真实环境不可用时如实说明证据范围。
- 独立预期来自本契约、固定例子与真实文件/引用的独立检查，不调用 implementation 的计算路径生成期望。

### 审批决定与实质取舍

- **接缝门**：用户明确选择 S1、S2；未选择每个私有模块单独验收，也未选择仅凭页面结果证明 Git 正确性，理由见表。
- **约束门 B1**：支持无首次提交的 Git workspace，空树比较、待提交文件全按新增绿色、通过 Commit 完成首提交。拒绝将它降级为普通目录或要求先外部提交，因为会使本功能的首提交入口失效；不扩展至初始化 Git。
- **约束门 B2**：接受非交互和每命令默认 300 秒可配置时限。未采用无限等待或本版另建终端/取消工作流，避免无解释地长期占用仓库资格或扩大范围。合法慢操作可能被打断，用户接受通过调大时限适配；已成功状态不自动撤销。若未来需要网页凭据、交互式 hook 或可恢复取消，须重审 S2 的执行与结果 interface，而不是静默增加交互。
- **约束门 B3**：大小上限对 diff 两侧分别生效。未采用仅检查工作区当前大小，因为小新文件仍可能迫使加载很大旧内容；两侧合计限额也未采用，以保持“每份内容上限”的明确含义。
- **验收门**：用户选择 Rapid iteration，接受仅手机实机交互体验由人确认。自动化、实现和真实 Git 正确性不转交人工。Full automation 未选，因为用户选择保留实际触摸与软键盘判断；不是减少承诺。
- 上述选择没有满足“难以逆转、无上下文令人意外、真实取舍”三条件的新增架构决定，不单独新建 ADR。沿用既有 ADR 的领域保证，不把其历史说明当作本功能交付证据。

### 六表面约束审视记录

|Surface|适用保证与推论|承载接缝|
|---|---|---|
|entry|页面门禁之外，公开操作仍检查目录/Git 资格、审查版本、脏工作区、冲突等；没有通用命令执行入口。启动只沿用配置输入，不新增面向用户的通用命令集合。|S1,S2|
|data and state|登记文件跨重启、跨设备可读；持久失败不假成功；实际路径不越根；缺 HEAD 用 P47；配置及实际索引/提交语义一致。|S2，并由 S1 显示结果|
|external dependencies|真实 Git、文件系统、隔离远端、时间与 Windows 路径特性；已有 Git 配置不被覆盖；真实监听有证据；阻塞输入受 B2 限制。随机性没有本功能新增承诺。|S1,S2|
|errors and failure|失败短路、无隐式回滚、响应不确定性、普通失败与冲突分开、超时与派生执行终止、持久及读取失败。|S1,S2|
|output and observability|明确区分未检查、过期/不可用、冲突、失效审查、超时、无法预览；Git 原提示作为纯文本显示，实际副作用有独立证据。无新增日志、metrics 或 telemetry 产品承诺。|S1,S2|
|resources and concurrency|真实仓库级串行、路径别名一致、进程未确认终止不提前释放、跨观察者合并事件、有限命令时长、两侧预览限制与完整审查复核并存；不得为成本默默漏掉待提交项。按需读取等内部方法自由。|附着 S1,S2，不新增接缝|

推论分类：P10/P13/P14/P27/P29/P32/P37 及 P50–P54 中标记项是既有保证成立的必要推论（inferred-required）；持久文件格式、监听库、审查指纹、队列、锁及读取批次是内部选择，不升格为用户保证；P47–P49 与 P44 的 B3 补充属于已经明确批准的新行为或取舍。

## Not yet specified

- **审批阻塞：无。** 三个审批门已完成。
- 具体配置键名、登记数据格式和保存位置、公开消息形状、更新传输与已有观察能力如何组合：未钉死为契约承诺，依照现有惯例在交付中确定，不据此提前拆票或增设接缝。
- 精确响应式断点、视觉 token、内部读取批次、文本识别细节等 implementation 选择保持自由，不能改变承诺或隐藏不支持条件；若实际选择改变承诺，先修订契约。
- 手机型号、系统、浏览器及局域网访问地址是最终 H1 证据的环境信息，届时填写，不阻塞自动实现和检查。agent 必须为 H1 准备访问步骤与可复现清单；用户确认前不视为实机已验收。
- 自动工作已完成仍不等于 H1 完成。票据交付归属由 `/to-tickets` 指定，不在本契约存储手工状态。

## Out of scope

继承 [spec 的 Out of Scope](spec.md#out-of-scope)，不复制或扩大范围。审批补充 B1 仅支持已有 Git 仓库的首提交，不增加初始化能力；B2 仅界定命令执行，不增加终端、凭据或冲突恢复工作流；B3 只界定既有预览限额，不缩小提交范围。

规格中“本阶段不选接缝”约束属于 `/to-spec` 的阶段职责；本契约承担下一阶段已批准的接缝与覆盖决定，不据此推导实现票或构建次序。
