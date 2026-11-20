# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "06-history-placementmd-02e250a9-70e0-4409-8b43-0",
  "heads": {
    "herdr-bot": "d779f3d6957620734dd0bbe2d85ecc81e074e4ab"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

已提交票据 06：`d779f3d695`。未操作集成分支。

- **P8、P10–P12：** Resume 只在当前宿主按原样查询 `originalWorkspaceId`。缺失或成功查询无匹配才按 `workspace_name` 兜底；命令失败和畸形响应不会触发兜底。测试比较了公开历史与持久文件字节，确认不补写或覆盖原 ID。
- **P13–P15：** 有效历史 cwd 优先。只有可靠且唯一的 Git 归属证据才允许回退主工作树；否则使用配置默认容器，不指定 cwd，并公开降级原因及 App 限制。
- **P41：** App 定位核对关联的 core 终端与实际 Pane 焦点。错误目标或聚焦失败不报告成功，也不承诺 Windows 窗口置前。

在合成 03 共享启动接口的隔离快照中，`chat_resume` **13/13**、`history_workspace` **6/6** 通过。**集成须先合入 03**；共享夹具入口需保留两票各自的分支。未运行浏览器或真实双端宿主验收；真实宿主证据归票据 08。
