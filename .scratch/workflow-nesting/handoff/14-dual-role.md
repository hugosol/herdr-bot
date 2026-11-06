# 14票交接：双角色完成边界与报告策略

## 结论

代码已合入两仓本地 `dev`，不等于本票验收通过。14票仍缺必需的真实 OMP／Pi 双宿主专项通过证据；不要关闭票据或把它改写成成功。

本次交接只归档已有报告，没有运行测试、启动模型请求或修改编排内部状态。

## 从哪里继续

- [原票及完整验收条件](../implementation/14-cross-repo-dual-role-lifecycle.md)
- [批准契约](../contract.md)
- [原始集成报告归档](evidence/14-integration.md)：以下结果均来自这份历史报告，不是对当前 dev 的新验证。
- 普通 Git 收尾后的源码基线（不含后续交接文档提交）：herdr-bot `0393c41df31bfb17b19bc299234671c521607d42`；toolkit `367cc4c5cbb7479b7d2878e63620350bae9f042c`。

## 尚未解决的实际失败

`accept-workflow-nesting-roles.py --hosts omp,pi` 退出1。报告记录 OMP 完整通过；Pi 的公开 completion 第0258次响应返回普通文字，而适配器要求 JSON，`workflow_completion_transport.py:55` 抛出：

```text
json.decoder.JSONDecodeError: Expecting value: line 1 column 1 (char 0)
```

这是传输／输出格式契约失败的已观察事实，不足以单独判定产品逻辑有缺陷，也不足以把根因完全归给模型服务。未用重试、放宽解析或文字兜底覆盖失败。

同轮 `--hosts declared` 退出0、混合宿主场景通过，但不能替代必需的双宿主矩阵。

## 历史上通过的范围

报告对应 herdr-bot `0145e49dac669e37dcb5bb390b7e0cf879560e41`、toolkit `5c9ebc66ad58f52799be114dae33f07fb36a87d3`：

| 范围 | 报告记录 |
|---|---|
| 前端构建、后端构建、完整后端入口 | 退出0 |
| workflow_dispatch | 164/164 |
| 前端测试 | 244/244 |
| roles runner及completion transport测试 | 退出0 |
| toolkit workflow-chat | 25/25 |
| toolkit typecheck | 退出0 |
| toolkit serial dock | 179/179 |
| 修复后的 toolkit canonical | 1819/1819 |

这不是对后续合并树的全绿承诺。报告保留此前失败：Pi初始化观测曾用固定次数 immediate 提前耗尽，已改为真实请求到达观察；不能抹去原日志。Pi dock 当时显式使用已安装 Node 22.19.0，不表示已修复 Node 24 的原生退出问题。

## 下轮处理边界

1. 先检查失败响应、适配器请求／输出格式约束及当时源码身份，定位格式失败来源。原报告提到 `native-omp-pi-completion-wire/0258.json`；本次没有把该原始响应文件归档，不能假设它仍可访问。
2. 若旧原始响应已不可达，明确记录证据缺口，不把已有摘要当成完整响应。不要为了补证据擅自启动新的付费模型矩阵。
3. 真正关闭本票仍需原票全部覆盖项及所属检查的实际通过证据；单宿主、declared或全局通过不能补足双宿主失败。
4. 必须重新执行真实模型验收时，先获得新的明确授权并使用独占隔离环境；不安装依赖宿主、不读取用户认证、不执行真实关机，不重试业务决策掩盖失败。

## 与编排死锁的区别

后续原工作树 Git 元数据丢失，使 `retry_integration` 在读取分支前失败。这是独立的编排／工作树生命周期问题，不是上述 Pi 验收的根因。此前工具仍把13／14／23标为 failed，`finish` 未成功；普通 Git 收尾没有伪造 integrated 状态。

原证据根曾为 `C:/Users/hugos/AppData/Local/Temp/hyper-ticket-fIEuJT/.scratch/workflow-nesting/evidence/integration14-4fb24d307a/`。这里只保留定位信息；临时工作树可能已被部分删除。已归档的集成报告无需依赖该临时目录即可阅读。
