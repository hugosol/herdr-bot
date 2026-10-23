# 03：原生默认发现、历史只读性与真实读取错误

日期：2026-10-23。范围仅为 implementation/03；基于 01 集中降级与 02 来源 Bot 环境恢复。

## 交付与批准边界

测试入口为公开 HTTP、交付页面、真实隔离持久文件／服务进程重启，以及生产 Herdr CLI adapter。仅外部 Herdr 被受控程序替代，不 mock 本项目的 Bot／目录／历史读取或恢复选择。

`resume.rs` 的集中 `degradation::Feedback::environment_args` 区分两种策略：

- 没有来源关联，或成功读取 Bot 存储后确认来源 ID 不存在：参数只含所选原生文件／精确 ID 的 `--session`（Pi）或 `--resume`（OMP）。不传 `--no-extensions`，不追加固定／可选入口，不按名称、cwd 或其他 Bot 猜测；反馈分别保留缺少关联／已删除原因与原生默认发现策略。
- 来源可读且存在：仍读取最新环境并显式加载。存储损坏／读取失败、配置解析／读取失败明确返回 settings 失败，不新建目标、不启动、不修复。

历史查询实现本来已经是只读，本票没有改动 `history.rs`，以真实进程与文件证据保留该行为，而不是另建补齐路径。

## 验收映射

| 本票条件 | 实际证据 |
| --- | --- |
| P7：旧记录、外部接入、正常发起后删除来源 Bot；Pi／OMP 均原生发现且选择同一对话 | `chat_resume_environment::missing_source_and_deleted_bot_use_native_discovery_without_guessing` 六个独立场景；`chatResumeDiscovery.spec.ts` 六个实际浏览器场景。删除样例先正常发起并确认收录，再通过公开 DELETE 删除来源，创建同名干扰 Bot，重启后 Resume。完整外部 argv 精确断言为两项，不含任何 Bot 扩展／trigger 参数。 |
| P7：不根据名称、cwd、当前 Bot、伪造来源补绑 | 同名同 cwd 的现有 Bot；外部 register 提供自称 `sourceBotId`，仍显示缺少来源并使用原生策略。原有 intake 竞态测试继续验证完整身份隔离与可信来源首次补齐。 |
| P15：默认发现的原因与策略交付到页面 | 每个浏览器场景核对 HTTP 的唯一 `extensions` 降级、页面 status 中同一原因和“原生默认扩展发现／不显式追加”策略；正常原 Workspace／可读文件排除了其他降级干扰。 |
| P8：公开读取、重启不初始化／补齐／修复 | `history_reads_only_resolve_workspace_names_without_any_persistent_enrichment` 独立预写缺少来源及旧字段的记录；三次进程启动、每次三次 GET，比较历史字节及修改时间、目录配置字节，确认 Bot／zone 文件仍不存在；缺少历史文件时 GET 空列表也不创建文件。 |
| P8：Workspace 临时按 ID 解析、改名不写回、查询失败仍可读取 | 同一测试及浏览器 read-only 场景：`w-stable` 的 Original name → Renamed name → 查询失败；保留同名其他 ID 干扰；页面失败时显示稳定 ID，历史字节／mtime 不变。 |
| P17：真实 Bot／配置错误不冒充删除，不启动或修复 | `unreadable_bot_storage_or_catalog_fails_without_start_or_repair` 两 agent × 四独立机制；Bot JSON 损坏、Bot 文件被目录替换造成 OS 读取失败、TOML 解析失败、TOML 被目录替换造成 OS 读取失败。每种均 HTTP 502、failed/settings、准确 code、空降级，外部记录没有新 Tab／start／input，坏文档／目录哨兵及历史保留。浏览器八个场景核对同样错误及 alert，无成功 status。 |
| 公开请求 smoke 与实际浏览器操作 | Playwright 驱动已打包页面、真实 HTTP、独立进程及 Chrome；无路由拦截。17 个场景通过，截图见下。 |

## TDD 与实际命令

首个 tracer test 先运行失败：`missing_source_and_deleted_bot_use_native_discovery_without_guessing` 在 legacy/Pi 的 HTTP 结果中得到空 `degradations`，与独立要求的一项原生默认策略不符。实现后同一测试通过六个场景；完整 argv 断言同时排除了原固定-only／关闭发现的实现。已满足的历史只读／错误行为增加真实边界覆盖，没有伪造 red。

首次只读测试 fixture 未提供启动必需的 metadata 文件，进程启动失败；随后补上合法启动目录，并仍保留独立历史输入及字节／mtime 预期，没有改变只读验收含义。回归中旧 Pi／OMP 的无来源 fixed-only argv／缺失固定项立即失败断言与新批准策略冲突，已迁移为原生 argv；单项扩展失效归 04，不沿用“无来源必须具备 Bot 固定项”的旧前提。

实际执行：

```text
cd backend
cargo test --test chat_resume_environment missing_source_and_deleted_bot_use_native_discovery_without_guessing -- --nocapture
# red: 0 passed, 1 failed；实现后 green: 1 passed
cargo test --test chat_resume_environment history_reads_only_resolve_workspace_names_without_any_persistent_enrichment -- --nocapture
# 合法启动 fixture 后：1 passed
cargo test --test chat_resume_environment unreadable_bot_storage_or_catalog_fails_without_start_or_repair -- --nocapture
# 1 passed（内部两 agent × 四机制）
cargo test --test chat_resume_environment --test chat_resume_pi --test chat_resume_omp --test chat_resume_timeout --test history_workspace -- --nocapture
# 最终：27 passed / 0 failed；6 + 8 + 5 + 2 + 6
cargo build --bins
# passed

cd frontend
npm ci
npm run build
# TypeScript 与生产打包通过
npx playwright test browser-tests/chatResumeDiscovery.spec.ts browser-tests/chatResumePi.spec.ts --output test-results/native-discovery-03
# 17 passed（20.4s）
```

## 交付页面截图

- [Pi 旧记录：缺少来源与原生默认发现](native-discovery-03/pi-legacy-native-discovery.png)
- [OMP 正常发起后删除来源 Bot：原生默认发现](native-discovery-03/omp-deleted-native-discovery.png)
- [OMP Bot 存储损坏：明确失败](native-discovery-03/omp-bots-corrupt.png)
- [Workspace 名称查询失败：稳定 ID 仍可显示](native-discovery-03/read-only-workspace-id-fallback.png)

## 未验证边界

本票只证明策略选择、实际提交参数、受控响应处理、交付页面及文件只读性。受控 Herdr 的相同原生身份确认不是真实 agent 加载证明。**真实原生默认扩展集合及 ambient 对照仍由 07 的 H-Pi／H-OMP 准备并由用户确认；本票未执行或代签该人工验收。** 04 的扩展逐项过滤及 05 的组合降级不在本票覆盖所有权内。当前 worktree 完成并测试后交独立集成代理；尚未由本代理合入集成分支。
