# Manual report 发送确认：独立开发需求研究

## 决策与范围

用户明确要求：本问题由后续独立开发需求解决；不修改 oh-my-pi。随后明确授权：凡能通过 report 工具内展示待发送内容并由用户明确选择“是／否”的方案解决的问题，本轮先跳过，等待后续优化。

本轮暂缓：manual 发送批准仅靠 prompt、未获批准外报、业务批准与发送批准的询问语义混淆，以及依赖模型输出特定批准同步标记的观察问题。保留历史失败，不重复运行这些确认流程来寻求偶然通过；状态记为用户授权暂缓／未验证，不记通过。

后续用户扩大暂缓范围：提示词修改留给专项优化，本轮同时暂缓 ticket22 的基线节点职责／阶段混淆、模型自行承担管理职责、误解文件授权和手写错误 HTTP 等模型遵循问题。不再调整 prompt 或重跑同一业务决策寻求偶然通过。三字段实际批准与合法保存的代码约束仍须检查，不能删除批准步骤或放宽门禁。

已知问题单独保留：独立 Chat 在 Pi 启动 handler 就绪前提交，原始输入被保留在编辑器而未执行；本轮不展开修复，不补发 Enter／prompt、不修改 Suite 或安装版宿主，不计通过。

不在暂缓范围：内部 Run 完成资格、父子执行身份与报告归属、报告内容／实际文件、finish 后独立 report、父层显式验收、Tab 精确关闭，以及 ticket22 的保存／revision 冲突／运行禁写／文件安全／非目标配置保持等流程代码机制。优先以现有公开 HTTP、真实文件与安装版宿主机制用例验证。受控 provider 仅证明机制，不证明 Luna 自主业务完成；被暂缓项阻断的真实业务步骤仍记录未到达，不能整体跳过或推定通过。

本文保存研究与设计边界，不代表需求已批准实施，也不代表 ticket14 已通过。当前不实现确认门禁，不放宽原验收断言。

## 核心结论

1. 当前 manual report 是提示词协议，不是程序强制的审批门禁。模型可能未获批准就调用 report，而现有提交路径没有检查真实用户确认。
2. Pi 1.5.0 与 OMP 18.17.0 原生 extension 都支持 await ctx.ui.confirm/select/input，不需要增加模型可调用的 ask 工具，也不需要修改 oh-my-pi。
3. toolkit 当前没有将原生确认能力暴露给通用业务工具。仅修改 report 提示词不能获得强制保证。
4. 真正的门禁应位于 report 的发送副作用之前；未明确确认、取消、超时、abort 或无交互能力时均不得发送。不能把模型正文中的“已确认”作为批准事实。

## 原生 UI 能力与限制

### Pi 1.5.0

安装根：C:/Users/hugos/.pi/agent/install/releases/1.5.0/node_modules/@earendil-works/pi-coding-agent。

- 官方 docs/extensions.md:10-24 明确列出用户交互及工具调用拦截；82-87 的示例在 tool_call 中 await ctx.ui.confirm，拒绝时返回 block。
- dist/core/extensions/types.d.ts:40-44,85-89：confirm(title, message, {signal?, timeout?}) 返回 Promise<boolean>；select/input 返回字符串或 undefined。
- 工具 execute 接收原生 context，可在 execute 内 await 确认，再进行实际发送；不应拆成两个只靠模型判断关联的工具调用。
- 用户拒绝、取消、内置超时与 abort 均不应视为批准；confirm 的 false 本身不区分这些原因。
- TUI 可显示确认选择框，但不是任意长报告的完整 Markdown 预览器，长内容阅读体验仍需设计与实测。

运行模式：

| 模式 | 交互行为 |
| --- | --- |
| TUI | 原生显示确认框，用户操作后 resolve |
| RPC | 发出 extension_ui_request，宿主展示并以同 id 的 extension_ui_response 回传；hasUI=true 不等于客户端已实现展示 |
| print / JSON | 没有交互 UI，confirm 默认 false，不能取得真实人工批准 |

RPC 官方依据：docs/rpc-extension-ui.md:1-24,48-65。没有 response、timeout 或 abort 时，对话可能持续等待；custom TUI 组件在 RPC 不支持。

### OMP 18.17.0

安装根：D:/nvm/v22.23.1/node_modules/@oh-my-pi/pi-coding-agent。

- src/extensibility/extensions/types.ts:321-351,1027-1034：原生 confirm API 和工具 execute context。
- src/modes/controllers/extension-ui-controller.ts:1149-1201,1216-1222：TUI 确认选择，取消不构成批准。
- src/modes/rpc/rpc-mode.ts:1517-1556：请求/响应式 confirm，宿主须展示并回传。
- src/extensibility/extensions/runner.ts:476-499：无 UI 的默认 confirm 返回 false。
- tool_call hook 可在底层工具执行前 await 并阻止调用，但不能撤销已经提交的 HTTP 或覆盖任意其他代码路径。

## 当前 toolkit / Herdr 接入缺口

以下 toolkit 路径相对 D:/developer/myhub/my-agent-toolkit：

- adapters/pi/native/core/extensions/types.ts:192-197 与 adapters/omp/native/extensibility/extensions/ui.ts:1-17：只声明 notify / widget，没有确认能力。
- adapters/pi/native/core/extensions/tools.ts:22-30 与 adapters/omp/native/extensibility/extensions/tools.ts:22-31：原生 ctx 没有作为确认能力传给业务工具；delivery 仅提供会话、调用身份和取消信号。
- framework/interface/tool.ts:13-18,29-35：公共工具交付接口没有确认 transport。
- extensions/workflow-chat/node-chat/shared-logic/tools/index.ts:27-28 和 shared-logic/report.ts:9-33：report 校验参数后直接提交 /api/workflow/node/report，没有 UI 批准门禁。
- extensions/workflow-chat/node-chat/shared-logic/prompts/init.md:19-21：manual 要求展示摘要、地址、限制，询问并结束本轮，等待自然语言确认。OPERATIONS.md:123 明确这不是审批接口。

Herdr-bot 的 backend/src/chats/mod.rs:246-267,295-315 和 backend/src/chats/herdr.rs:287-321 表明：当前通过 Herdr pane 启动 agent，不是本仓自行托管的 RPC UI 客户端。代码未强制 RPC/print；实际能否显示确认框仍须通过真实 Herdr 终端/PTY 行为验证。本研究没有实测 UI，不把原生 API 存在当成端到端交互通过。

## 后续独立需求应明确的事项（未实施）

- 公共确认能力、两宿主 adapter 和 report 的接入归属。
- 确认展示范围：报告正文/摘要、输出地址、限制及实际发送对象；批准须绑定待发送内容，修改后重新确认。
- auto 模式保持现有语义；manual 未明确批准不得触发发送副作用。
- 取消、超时、abort、无 UI、重复操作、并发或过期对话的行为和证据。
- TUI 的真实可操作性与长报告预览；若支持 RPC，客户端需实现对应 UI 协议。
- 若要求防止任何直接 HTTP 绕过，不能只做 extension UI 门禁，还须另行定义后端批准凭证及信任边界；本研究未决定该架构。
- 测试应观察真实用户选择与 HTTP 是否实际发生，不能以模型遵循 prompt、最终 completed 或伪造批准代替。

## ticket14 当前证据与其他未关闭项

证据根：.scratch/workflow-nesting/evidence/。

### 已观察到的 manual 越界

resume-20261106-210400/native-roles-omp-pi/business/omp：批准问题与 todo/run 工具调用同回，未形成合规等待停轮；runner 发送批准输入为 0，但 backend-wire/0009.json 记录实际外报 HTTP 200。离线 replay14-approval.py 退出 1 重现这些旧事实，不是一次新真实模型通过。

OMP 系统模板“completion only stopping condition / 不得在仍有 actionable 工作时停轮”与 manual 等待批准存在解释冲突线索，但未通过对照实验证明唯一因果；不应把它断言为 oh-my-pi 缺陷，亦不修改该仓。

### 最新 Pi：另有确认观察/语义问题，非网络失败

continue-20261107-090904/roles-pi：真实模型固定 gpt-6-luna，5 次调用，businessExit=1、transportFailed=false、cleanupErrors=[]。

- evidence.json 记录等待 REQUEST_SEND_APPROVAL_1 超时。
- completion-wire/0004.json 中模型确实以无工具调用的普通文字停轮，但只要求用户回复 SEND_APPROVAL_1，未输出 runner 期待的 REQUEST_SEND_APPROVAL_1。
- 同一回复将该确认表述为授权继续内部执行和诊断；而场景已授权业务执行，待确认应是外报发送。存在确认语义混淆，不能仅补 marker 就认领通过。
- 因此本轮不能笼统归为“Pi无法停轮”或“网络超时”。仍需区分 runner 对同步 marker 的依赖、模型是否完整展示发送内容，以及业务批准/发送批准的职责边界。

### 尚未获得通过证据（不等于已发现产品缺陷）

两组样例、重复 Map、多 execution 干扰下的身份归属；未 finish 拒绝与 finish 后独立 report；父 manager 验收最新报告并显式 complete；真实 Tab 的存活、精确关闭与内部保留等后续步骤，均不能从当前第一处确认边界失败推定通过。旧 handoff/14-dual-role.md 曾记录历史 OMP 通过与 Pi 输出格式失败，不是当前源码/环境全矩阵通过证据。

结论：将强制确认门禁列为独立需求，不会自动使 ticket14 其余验收通过。14 仍保持未关闭；其他确定性业务缺陷尚不能据这些未到达步骤下结论。
