# 01: 从历史 Chat 定位已有原生对话

**What to build:** 用户可以从每个历史 Chat 发起 Resume；后端根据历史记录中的原生身份查找 Herdr 已有对话，并在匹配时聚焦其实际 Workspace、Tab、Pane，而不是提交输入或另开副本。浏览器呈现后端确认的结果，不以当前会话登记或原有 Herdr 绑定作为入口资格。

**Delivers:** P1, P2, P3, P4, P5, P20, P23, P24；P22 的已有目标定位部分。

**Test owner for:** P2、P3、P4 的自动检查；P22 的已有目标定位自动检查。真实环境部分统一由 07 准备并提交人工确认。

**Blocked by:** None (can start immediately).

**Status:** in-progress

- [x] 真实交付页面对每个历史 Chat 提供 Resume；旧记录、connect-bot 来源记录、登记已移除及服务重启后的记录均可通过页面和独立 HTTP 请求发起操作。信息不足可以明确失败，不能隐藏入口。
- [x] 请求只选择历史 Chat；命令、参数、原生身份、文件引用和工作目录来自后端记录及配置。伪造可执行文件、参数、原生路径或输入等额外内容，不能覆盖后端权威数据或形成任意执行入口。
- [x] 每次操作实际查询 Herdr，不用会话登记在线状态代替查询。原生 ID 与会话路径的不同表达能够判定为同一身份，不混用 agent 名称、Pane 身份和原生 session ID。
- [x] 使用独立预写原生文件头及 ID／路径相等、不相等样例验证身份；同名且同 cwd 的不同对话不得匹配。
- [x] 唯一匹配时聚焦目标实际所在的 Workspace、Tab、Pane；即使与历史偏好位置不同，也不搬迁、不创建 Tab 或 agent。
- [x] 多个匹配时页面给出提示，按明确且稳定的目标排序自动选择；改变查询枚举顺序不改变选择，不增加选择交互或启动副本。
- [x] 目标正在工作或等待输入时，定位均不提交 prompt 或任何会话输入。
- [x] Herdr 不可达、错误响应、无法解析及聚焦失败分别明确反馈；不得作为无匹配继续创建或启动。
- [x] 影响所选 Chat 匹配的身份资料暂不可读时，明确报告核实失败并停止；无关 agent 的资料不可读不得无条件阻止可确定的匹配。两类边界均有独立用例。
- [x] 定位成功仅表示已确认的 Herdr 内部导航，不声称 Windows 桌面窗口已置前。查询成功但无匹配时明确表示尚未恢复，不能将其报告为恢复成功；新进程恢复由后续票交付。
- [x] 保持既有会话事件同机来源限制，独立验证不允许来源的会话事件仍被拒绝且未修改登记；不扩展该边界或重构登记生命周期。
- [x] 自动检查覆盖 HTTP 与可观察 Herdr 副作用；实际操作交付页面验证入口、多目标提示及错误反馈。原生真实焦点检查交由 07，不以受控结果冒充真实环境确认。

## 覆盖分区

本票拥有 P1、P5、P20、P23、P24 的全部合同覆盖，P2、P3、P4 的自动覆盖，以及 P22 的已有目标定位覆盖。P2、P3、P4、P22 引用的真实原生身份、焦点和输入观察复用 07 的 H-Pi／H-OMP，不另设人工轮次；相关承诺须等待其全部分项完成。

## Comments

- 已完成本票实现及自动验收，等待独立集成；按项目规则，合入前保持 `in-progress`，由集成方合入后改为 `closed`。
- S2/S3：`backend/tests/chat_resume.rs` 的 7 项检查覆盖原生 ID／路径（Pi 与 OMP 独立预写文件头）、异身份同名同 cwd 反例、实际位置、工作／等待输入均无输入、多目标稳定排序、执行字段拒绝、查询／聚焦失败与相关／无关身份资料不可读边界。
- S1/P1/P5/P20/P22：`frontend/browser-tests/chatResume.spec.ts` 的 2 项真实交付页面检查覆盖旧格式独立预写记录、connect-bot 协议来源、移除登记与真实进程重启后的 HTTP／页面操作、多目标自动选择、手机错误反馈及未匹配不冒充恢复。
- 最终后端命令：`cargo test --test chat_resume --test session_write_boundary --test session_history --test chat_creation --test pi_zone_history --test omp_zone_history`，55 通过、1 既有真实网卡检查按手动规则忽略；来源拒绝及既有登记／历史行为检查通过。
- 最终交付构建：`cargo build --bins` 与 `npm --prefix ../frontend run build` 通过。`npm --prefix ../frontend run test:browser -- chatResume.spec.ts sessionHistory.spec.ts`：5 通过，含 1280px／390px 历史视图回归；网络护栏通过。浏览器截图在 `frontend/test-results/`（忽略的测试产物）中。
- 红绿证据：最初 Resume 请求为 405；JSON 错误信封被错误分类；页面无 Resume 按钮；校正夹具使用官方 `agent_info` 聚焦响应后暴露旧的 `ok` 响应假设。相应实现后均通过。聚焦响应形状另核对了已安装 Herdr API schema 与外部 Herdr `handle_agent_focus` 源码，未操作用户真实 Chat。
- 未验收：07 所属 H-Pi／H-OMP 原生真实身份、焦点和输入观察；本票的受控结果不替代这些人工确认。新进程恢复不属于本票；成功查询无匹配返回 `not-found` 并明确“尚未恢复”。
