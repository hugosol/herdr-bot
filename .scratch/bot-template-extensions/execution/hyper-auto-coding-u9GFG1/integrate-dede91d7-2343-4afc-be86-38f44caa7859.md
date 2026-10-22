# integrate

```json
{
  "role": "integrate",
  "agent": "hyper-integrate",
  "agent_id": "integrate-03-stepped-template-editormd-b0f729e6-",
  "head": "22b3d5311cfc046c8974ecd853c096041a5ee704",
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "result": "当前开发分支已同步集成提交，本轮项目全局测试通过；无冲突、无需修复或新增提交，工作区干净，可由宿主排队合并。",
  "ticket": "03-stepped-template-editor.md",
  "worktree": "C:/Users/hugos/AppData/Local/Temp/hyper-ticket-MQZFDe",
  "branch": "hyper/bot-template-extensions/03-stepped-template-editor-8a299b-8713386ed6b5",
  "head": "22b3d5311cfc046c8974ecd853c096041a5ee704",
  "commands": [
    {
      "command": "git status --short --branch && git log -4 --oneline && git show --stat --oneline HEAD",
      "cwd": "C:/Users/hugos/AppData/Local/Temp/hyper-ticket-MQZFDe",
      "outcome": "确认当前分支 HEAD 为 22b3d53，包含实现提交 fc23613；无未提交变更。"
    },
    {
      "command": "cargo test",
      "cwd": "C:/Users/hugos/AppData/Local/Temp/hyper-ticket-MQZFDe/backend",
      "outcome": "成功完成：361 passed，3 ignored，未出现失败。"
    },
    {
      "command": "npm test",
      "cwd": "C:/Users/hugos/AppData/Local/Temp/hyper-ticket-MQZFDe/frontend",
      "outcome": "通过：26 个测试文件、184 个测试。"
    },
    {
      "command": "git status --short --branch",
      "cwd": "C:/Users/hugos/AppData/Local/Temp/hyper-ticket-MQZFDe",
      "outcome": "测试后仅显示当前分支，无变更，工作区干净。"
    }
  ],
  "scope": "未操作集成工作区，未切换分支、新建 worktree、reset、调用 retro 或直接合入集成分支。按要求未运行复杂脚本、实机 smoke、浏览器跨票据整体验收；这些留给最后独立门禁。",
  "blockers": []
}
