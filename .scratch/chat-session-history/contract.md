# Contract — chat 历史会话列表与原生会话存储回归

Source: [spec](spec.md)
Acceptance mode: Rapid iteration
Approval: approved

批准依据：用户明确批准 S1–S4；确认普通分页规则；确认 zone 收录独立执行、身份获取预算 30 秒、失败仅记服务端日志且不进入创建结果，connect-bot 保留操作者警告；随后选择 Rapid iteration，仅保留 H1。

本 contract 约定可观察承诺和 seam，不规定实现顺序或内部构造。批准不等于实现或验收完成。承诺的完成情况由交付工作、自动检查与人工确认共同推导，不维护承诺状态列；交付工作由 `/to-tickets` 建立关联。

## Promises

Coverage 中的 A 表示代理负责实现、TDD 及所列自动／运行检查；H1 是唯一预留的人工验收项。H1 只保留用户实际部署环境中的最终链路确认，不替代 A，不把实现或排错交给用户。期望值来自本 contract、spec、独立预写样例及外部真实观察，不能用被测实现重新计算期望。

|#|Promise (one observable result)|Coverage|Source|Seam|
|---|---|---|---|---|
|P01|zone Bot 确认创建成功后，以本次实际原生对话身份收录；不依赖勾选 connect-bot，也不等待模型任务完成。|A：分别驱动 Pi／OMP 成功创建和身份返回，独立读取历史，核对预先指定的原生 ID、cwd 与名称；受控 adapter 不冒充真实运行。H1：用户在实际 Herdr 中分别创建不带 connect-bot 的 Pi／OMP，确认历史 ID 与本次原生对话相同。代理准备两种 agent 的创建对象定位、原生 ID／文件头部核对证据、历史读取结果及最小验收步骤，不提交用户模型任务。|stories 1, 3, 14, 15；Implementation Decisions §2、§4|S2、S4|
|P02|未确认成功的 zone 创建不以 zone 来源入历史；同一对话若独立成功 register，仍可按接入规则入历史。|A：启动／初始输入提交失败分别观察历史不新增 zone 记录；再以同一原生 ID 成功注册并观察恰一条记录。|stories 14, 16；Implementation Decisions §2|S2|
|P03|只有服务端接受的 register 触发接入收录；可信 sessionId 不因缺少可选元数据而被拒绝收录。|A：有效注册、缺可选值、未知非注册写入及拒绝来源分别驱动；观察历史和接入回执，不放宽现有 register 必填字段规则。|stories 2, 16, 17；Implementation Decisions §2、§7|S2、S3|
|P04|相同完整 sessionId 的重复及跨来源收录最终只有一条记录，不同 ID 不因同名或同 cwd 被误合并。|A：独立字面样例覆盖 connect→zone、zone→connect、重复注册及同时到达；与同名同 cwd 的不同 ID 对照，独立读回精确集合。真实身份的部署核对归 P01 的 H1。|stories 4, 5；Implementation Decisions §3；inferred-required：并发仍须满足同一身份唯一性|S2、S4|
|P05|历史成员与当前会话登记独立，运行中的对话可同时出现；单独收录历史不建立当前登记。|A：zone 不接入时只出现在历史，接入后可同时出现，随后注销当前登记不影响历史；浏览器核对两个视图。|stories 6, 7, 41；Implementation Decisions §1|S1、S2|
|P06|已成功保存的历史跨后端重启和浏览器生命周期保留，独立设备读取同一实例的持久数据。|A：写入已知记录，重启隔离后端，再用独立客户端／浏览器上下文读取；关闭和刷新页面不改变该集合。|stories 8–10；Implementation Decisions §4|S1、S2|
|P07|注销、当前登记过期、zone 取消登记或历史年龄／数量增长均不自动删除已存历史。|A：沿现有操作推进各事件，受控时间跨过当前登记保留窗口，以超过一页的已知集合核对历史完整保留；不把列表分页截断当作存储淘汰。|stories 11–13；Implementation Decisions §5|S2|
|P08|后续有效元数据更新同一记录，缺失或无有效值的字段不清空已知值。|A：依次写入完整值、部分更新和缺失值，独立读回具体字段；精确原生绝对路径可更新并作为内部引用保存，但不进入页面展示。|stories 18, 19；Implementation Decisions §3|S2|
|P09|zone Bot 显示名优先于 connect-bot 会话名；connect-only 使用最近一次有效上报名称，且来源先后不改变优先级。|A：使用不同字面名称验证两个来源顺序及后续改名，读回并在页面核对最终名称；不能按当前到达顺序覆盖 zone 名称。|stories 20, 21；Implementation Decisions §3|S1、S2|
|P10|页面无名称时以 sessionId 识别记录，其余缺失展示值显示 `—`。|A：实际浏览器呈现缺失名称、类型、cwd 的历史样例；核对具体回退内容，不能用空白、猜测类型或“离线”代替未知元数据。|stories 22–24；Implementation Decisions §5|S1|
|P11|首次收录时间是首次成功持久化的服务端时间，更新及此前未保存的尝试不改写或伪造该时间。|A：用独立预定时刻进行失败后首次成功保存、重复注册及元数据更新，跨重启读回同一首次时间；不以当前登记的注册顺序代替。|stories 25, 26, 37；Implementation Decisions §3、§4|S2|
|P12|历史按首次收录时间倒序，心跳、重复注册和元数据更新不把旧记录提到前面。|A：使用不同预写时刻建立可判定顺序，执行后续更新并比较精确顺序；首次时间相同的记录也须能稳定翻页，但具体次级排序不冻结。|stories 26, 27；Implementation Decisions §5；inferred-required：静止数据集分页不能因同时间记录漂移|S1、S2|
|P13|历史每页最多 50 条并提供正确的前后页导航。|A：空集合、恰 50 条和 51 条等边界使用独立 ID／时间样例，核对每页具体记录与页间关系、首页／末页导航；静止集合中全部记录可被访问。|story 28；Implementation Decisions §5|S1、S2|
|P14|历史只展示名称、agent 类型、cwd、首次收录时间，桌面紧凑表格与手机紧凑条目均可读。|A：实际桌面和手机尺寸浏览器检查四项内容、长名称／路径、分页可用性与布局；不出现模型、来源、keepAlive、运行状态或关机监听操作。代理准备实际页面证据；本项不预留人工验收。|stories 23–25, 29, 30；Implementation Decisions §5|S1|
|P15|chat 右上角三角按钮在当前／历史两个互斥视图间切换，面板底色不同且有明确文字标签。|A：实际点击并观察两个方向的切换及标签、不同底色；不能仅依赖三角方向或颜色传达模式。|stories 31, 32；Implementation Decisions §5|S1|
|P16|打开或整页刷新默认当前；进入历史或翻页读取当时最新持久结果，停留时不自动刷新。|A：独立客户端在浏览器停留期间新增／更新记录，页面不实时重排，退出再进入或翻页后取得最新结果；刷新回当前。跨页冻结快照不属于承诺。|stories 33, 34；Implementation Decisions §5；已批准的分页取舍|S1、S2|
|P17|zone 收录不在创建成功响应关键路径中，不改变创建结果或增加历史专用轮询；关闭页面不取消本次收录。|A：用可控但尚未完成的身份读取，观察创建已经成功，同时其他创建、注册及当前观察仍可推进；关闭页面后提供身份，独立读回历史。验证行为推进，不仅测一个宽松耗时阈值。|story 35；Implementation Decisions §4；已批准的独立执行约束|S1、S2、S4|
|P18|connect-bot 历史保存失败不使已被接受的接入失败或 disarm，也不停止原有上报。|A：对隔离存储制造实际写入失败，执行接入，观察当前登记与后续上报仍成立；接入成功与历史失败必须分开表达。|story 36；Implementation Decisions §4|S2、S3|
|P19|zone 收录失败仅产生可关联创建及原因的服务端警告；connect-bot 保存失败向实际操作者明确警告。|A：分别触发身份缺失／超时和保存失败，收取交付运行的诊断输出，关联对应创建及原因，确认创建页面不增加历史提示；在实际 connect 命令反馈中观察接入成功和历史失败，不能只核对 HTTP 字段或模型上下文。具体日志／提示文案不固定。|story 37；Implementation Decisions §4；已批准的反馈渠道修订|S2（附属诊断观察）、S3|
|P20|后续成功 register 会重新尝试历史写入，已存则更新、未存则新建，不产生重复或重置已有首次时间。|A：先令历史保存失败，再恢复可写条件并重新注册，读回唯一完整记录；另覆盖旧记录更新失败后的重新注册，核对原首次时间保留。|story 38；Implementation Decisions §4|S2、S3|
|P21|历史读取／更新失败不伪装成空列表或保存成功，也不损坏此前已存记录。|A：真实隔离存储中的不可读、无效内容和写入失败分别驱动，观察明确读取错误及原持久数据不被覆盖；故障解除后仍能读取此前已存值。|story 37；Implementation Decisions §4|S1、S2|
|P22|本功能不增加后台写入重试、自动 connect 或新的接入失败预算。|A：触发保存失败后不再发起事件，观察无自主重连或重复写入；后续明确成功 register 可触发新的保存尝试。30 秒内有界只读身份获取不是无限重试或后台保存重试。|stories 36, 38, 41；Implementation Decisions §4|S2、S3|
|P23|新 Bot 不覆盖或预分配原生会话目录，Pi／OMP 实际遵循自身正常存储配置。|A：受控 adapter 验证启动选择，隔离真实 agent 验证实际文件位置及原生发现能力，不以 argv 回显替代落盘证据。H1：实际 Herdr 中两种 agent 创建后核对正常存储位置及其原生会话发现机制。代理准备有效配置说明、实际原生引用、目录／头部证据与发现核对步骤；此前自定义目录迁移的结果不能冒充本项通过。|story 39；Implementation Decisions §6|S4|
|P24|本功能不修改用户 autoResume、原生目录或其他 agent 全局配置，也不加入替代性强制新会话策略。|A：用隔离配置运行真实 agent，独立比较运行前后配置；覆盖原生配置生效而不是假定固定默认目录，不改真实用户全局文件。|stories 39, 40；Implementation Decisions §6|S4|
|P25|本功能不批量导入旧会话，不移动、复制、转换、删除或改写既有原生主文件与附属文件。|A：在隔离环境预放旧原生文件及附属记录，运行新收录流程，比较字节及目录清单，确认未自动入历史；该旧对话之后成功 register 可按普通规则收录。精确引用读取只用于必要身份元数据，不导入正文。|stories 40, 42；Implementation Decisions §3、§6；Out of Scope|S2、S4|
|P26|当前 keepAlive、登记生命周期、完成后关机监听和倒计时保持原有语义，历史不令临时状态持久化。|A：复用并更新必要的现有行为覆盖，核对收录、更新及失败不改变当前状态；隔离重启后历史存在而临时意图不恢复。绝不执行真实关机。|story 41；Implementation Decisions §1、§7；ADR-0006|S1、S2、S3|
|P27|保持同机写入和局域网读取规则，历史能力不放宽会话写入来源或新增认证。|A：从受允许与被拒绝的来源驱动公开请求，分别观察接入和历史结果；被拒绝写入不建历史，局域网读取仍按既有权限可用。|Implementation Decisions §7；继承 ADR-0004|S2|
|P28|本次不暴露搜索、筛选、正文、恢复、删除或原生文件存活性扫描行为。|A：通过交付页面与已文档化的公开 interface 核对仅有约定的读取和收录能力，不调用恢复命令或扫描旧存储来填充列表；不做源码文本断言。|story 42；Out of Scope|S1、S2|
|P29|并发成功写入的不同记录及同记录合法合并结果不会互相覆盖丢失，重启后仍能读回承诺持久的数据。|A：独立客户端同时收录多个预写 ID、交错更新同一 ID 后读取精确结果，再重启隔离后端核对；不通过调用实现合并函数生成期望，不要求超出 spec 的任意硬件故障保证。|inferred-required：stories 4, 5, 8, 18 与 Implementation Decisions §3、§4 的并发推论|S2|
|P30|zone 成功后的独立任务只为获取精确原生身份等待最多 30 秒，取得身份后尝试保存一次，超时或失败后结束。|A：受控时间和外部 adapter 延迟覆盖期限前成功、到期仍不可得、文件 header 暂不可读、保存失败及页面关闭；核对无猜测收录、无延迟创建成功、无到期后继续获取／写入重试，警告归 P19。身份获取预算不是模型轮次时限，也不声称能强制取消任意内核磁盘操作。|Implementation Decisions §2、§4；用户明确批准的 30 秒与独立执行规则|S2、S4|

**Complete implementation and all assigned automated checks before the human-led acceptance round. Planned human-led checks do not pause otherwise executable work. At the final handoff, report the remaining checks together with their prepared evidence; they remain unverified until the user confirms them. Report genuine blockers when encountered.**

## Seam decisions

|Seam|Exposes|Hides|Alternatives considered|Reversibility|
|---|---|---|---|---|
|S1：主界面浏览器交互|交付页面的当前／历史切换、内容、分页、响应式样式及历史读取错误；创建结果仍只表达创建本身。|浏览器内部状态、布局实现及数据获取组织。|只检查返回数据无法证明可读布局或实际切换；只检查离屏渲染不足以证明实际桌面／手机表面。复用现有浏览器交互 seam，不新增展示 module 的外部 interface。|较高；内部页面实现可更换，但已批准的可见行为必须保持。|
|S2：herdr-bot 交付运行的公开 HTTP interface，附属服务端诊断观察|真实请求的创建／注册结果、历史分页与持久读回、权限、失败结果及进程重启后的事实；zone 记录失败由对应请求引发的服务端警告观察。|元数据存储格式、原子更新方式、线程／任务组织、锁安排及内部错误传递。|仅在浏览器观察无法可靠区分持久化、并发和故障原因；直接测试私有存储把实现冻结且不证明 caller 真正经历正确语义。沿用既有公开请求 seam，增加历史只读读取，不把存储内部提升成新外部 seam。日志是用户批准反馈修订后的附属观察，不另设内部 module seam。|中等；公开读取与回执的语义是对 caller 的承诺。内部持久实现可替换；严格跨页快照若以后成为需求，会影响读取 interface、分页导航及相关覆盖。|
|S3：connect-bot 命令与操作者反馈|实际接入、保存失败警告、再次注册重新收录，以及原有上报继续工作的可观察结果。|receipt 解析与反馈组合的内部组织。|只返回新增 HTTP 字段会被现有 caller 忽略；只测试消息选择函数无法证明警告到达操作者。复用真实命令及反馈 seam，覆盖受影响 caller，不以模型是否看到通知作为判断。|中等；反馈语义与调用兼容性需要保持，具体文案不固定。|
|S4：Herdr／原生 agent 运行交互|指定创建对应的原生身份、真实会话引用、Pi／OMP 存储位置及原生发现能力，不依赖可选 connect；既有原生文件及设置不受改写。|Herdr 传输 adapter、引用查询时序、必要 header 元数据读取及原生启动的内部组织。|只用受控 adapter 会漏掉真实原生身份与存储差异；只看到官方扩展文件存在不能证明上报发生；按 cwd 最近文件猜身份违反 P01。受控 adapter 与真实运行提供不同证据，必须区分。|中等；外部产品升级可能改变引用可获得性或原生发现规则，应先修订 contract 并重新核对受影响承诺，不能静默降级为只收录 connect 来源。|

依赖分类与证据约束：合并和受控时间属于 **in-process**；隔离文件系统属于 **local-substitutable**；独立部署的自有 connect-bot caller 与 herdr-bot 交互属于 **remote but owned**；Herdr／Pi／OMP 对本功能属于 **true external**。复用既有生产与受控 adapter，不因新增测试凭空增加外部 interface。受控 adapter 可证明时序、故障与调用语义，不能替代 S4 真实原生运行或 H1 的部署确认。

所选取舍：zone 失败不进入创建结果，不要求页面为历史专门继续观察，代价是创建者不会立即知道未收录；由可关联的服务端警告及以后成功 connect 重新收录承担可诊断与恢复入口。恢复创建页面警告将影响结果观察与展示承诺，届时应先修订 contract。普通分页不冻结跨页集合，实时新增期间可能跨页移动；未来严格遍历不重不漏需重审读取 interface 与导航，本次不提前承担快照生命周期。

约束审视已走过全部六个表面：entry、data and state、external dependencies、errors and failure、output and observability、resources and concurrency；全部适用，没有不适用项。与已批准承诺相容的推论已写入 P04、P12、P17、P29；它们是完成原承诺所必需，而不是额外功能。线程、存储内部及具体提示文本仍是 implementation 自由。

## Not yet specified

- 没有阻止本 contract 批准的产品决定。S1–S4、独立 30 秒身份获取、两种失败反馈渠道、普通分页和 Rapid iteration／H1 均已明确批准。
- 公开读取与回执的具体编码、兼容字段组织、原生引用取得机制、内部存储与任务组织尚未固定；它们必须满足上述承诺，不能被用作延后 zone 收录、吞掉 connect 警告或减少覆盖的理由。不在这里预切实施工作。
- H1 的实际部署验收证据尚待实现后准备与用户确认；这是已分配的覆盖，不是未决定的需求，也不能在未确认时标为通过。
- 未来搜索、会话恢复、删除、实时刷新及严格跨页快照均没有在本 contract 中定义 interface；沿用 spec 的范围排除，不为未来功能预切工作。
- 实施若证明某个 seam 或承诺不成立，先修订本 contract，并重新打开受影响实施工作；不能先改实现，再让 contract 默默追随。

## Out of scope

完整继承 [spec 的 Out of Scope](spec.md#out-of-scope)，本 contract 不新增排除项。H1 仅改变验收责任，不缩减任何功能或兼容承诺；受控 adapter／隔离管道结果不能被用于把真实部署验收改成自动通过。
