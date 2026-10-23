# 05: 同一 Chat 的并发 Resume 不重复执行

**What to build:** 同一 herdr-bot 实例内，同一历史 Chat 同时最多一个 Resume 持有执行资格。来自不同页面或独立请求者的重复操作不能形成后续启动队列，浏览器依据后端权威结果反馈忙碌。

**Delivers:** P16。

**Blocked by:** 02: 无匹配时恢复 Pi 历史 Chat。

**Status:** in-progress

- [x] 执行资格在实例级按所选历史 Chat 约束，不仅依赖单页按钮禁用；并发进入查找及启动阶段时都不能让同一 Chat 同时执行多个 Resume。
- [x] 独立 HTTP 请求者同时操作同一 Chat，配合受控查询及启动延迟，观察最多一次创建和启动；重复请求不等待后排入下一次执行。
- [x] 首个操作完成后，先前被拒绝的请求不会自动执行；不存在延迟启动副本。
- [x] 正常定位、恢复成功和明确失败后正确释放执行资格，后续由用户新发起的请求可以执行；同一 Chat 不会因失败永久忙碌。
- [x] 不同 Chat 不因本票新增不必要的全局串行限制；执行资格不扩大为多实例分布式锁。
- [x] 实际浏览器交互呈现后端接受或拒绝的结果，不乐观宣称成功；以独立请求者和副作用记录证明约束，而不是以按钮状态证明。
- [x] 本票拥有正常和明确失败路径的并发、非排队及资格释放检查；启动超时、后续重试与并发交错用例由 06 唯一拥有。
- [x] 更新现有 Resume 说明中的实例内并发边界及忙碌语义，不改变会话登记生命周期。

## 覆盖分区

本票拥有 P16 的基础实例级执行资格、独立请求并发、拒绝排队、正常与明确失败后释放的自动检查。06 在 P17–P19 的超时场景中验证与该资格的组合行为，不重复认领本票基础用例。

## Comments

- 执行实现与本票验收完成，保持 `in-progress` 等待独立集成；按项目规则，合入之后才可标记 `closed`。
- 已批准 seams：S2 真实独立 TCP HTTP 请求与 S3 生产 CLI adapter／受控外部进程；S1 使用交付页面与真实 Chrome，无浏览器网络 mock。
- P16：`backend/tests/chat_resume_concurrency.rs` 三个用例覆盖查找与启动的受控重叠、立即 409 不排队、定位／恢复完成后及明确 lookup／focus／start 失败后可由用户重新执行、不同 Chat 与独立实例互不阻塞。副作用记录核对只有一次 Tab 创建和 agent 启动。
- 浏览器：`frontend/browser-tests/chatResumeConcurrency.spec.ts` 用独立 HTTP 请求持有资格，交付页面收到真实 `409/resume-in-progress` 后显示后端消息，首个请求成功不把已拒绝请求改为成功；只有新的用户点击才重新查找定位。截图在该 Playwright 用例输出的 `resume-authoritative-busy.png` 与 `resume-after-busy-new-action.png`。
- TDD red：首个受控查询重叠用例在没有执行资格时因重复请求未即时拒绝而失败（3 秒有界等待到期）；增加 guard 后通过。过程中修正了测试选项目录缺少固定 Pi 集成的独立 fixture，未改变验收预期。
- 实际验证：`cargo test --test chat_resume --test chat_resume_pi --test chat_resume_concurrency`：17 通过；`npm run test:browser -- chatResume.spec.ts chatResumePi.spec.ts chatResumeConcurrency.spec.ts`：4 通过；`cargo build --bins` 与 `npm run build` 通过。首次 frontend build 因未安装依赖失败，执行锁定依赖 `npm ci --prefer-offline --no-audit --no-fund` 后构建和浏览器验证成功。
- 说明更新在 `docs/integration-boundaries.md`；未修改会话登记生命周期。本票没有认领或验证 06 的超时交错场景，没有执行 H-Pi／H-OMP 人工真实原生对话验收，也未运行全仓测试。
