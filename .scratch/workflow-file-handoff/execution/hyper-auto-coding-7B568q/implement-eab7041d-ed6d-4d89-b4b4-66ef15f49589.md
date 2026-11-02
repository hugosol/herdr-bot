# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "06-plan-workflow-migrationmd-9e2740a3-3a7a-4288-",
  "heads": {
    "herdr-bot": "e9619f8912db9b4ad7b3599addf6c4e8a1b175f5"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

{
  "ticket": "06-plan-workflow-migration",
  "result": "implemented-and-verified",
  "commit": "e9619f8",
  "branch": "hyper/workflow-file-handoff/06-plan-workflow-migration-fe2c85-9497b79afe0a",
  "worktree": "C:/Users/hugos/AppData/Local/Temp/hyper-ticket-ppMKuj",
  "handoff": "实现已提交，worktree 可交独立集成代理接管；未操作集成分支、其他票据或工作流报告。",
  "changes": [
    "outline 使用初始化注入的 userInput、业务 input 和按 Run 区分的中间资料位置；删除解析运行身份、固定机器材料目录和运行日志背景读取协议。",
    "outline 保留九章节概要方案、人工补齐能力、当前方案确认及独立 manual 报告发送确认；通过 can_complete 和命名 outputs.outline 交接。",
    "generate_map 声明 inputs.outline 来源为 outline 节点的 outline 输出，直接读取解析地址；保留必要业务内容核验、真实绑定和 zone、冲突不覆盖、完整读回与服务校验、条件只读预检及绝不自动启动业务 Run。",
    "新增公开 Map interface 保存／读回测试并更新 workflow-runtime 迁移说明。"
  ],
  "acceptance_evidence": [
    {
      "criteria": "AC1–AC2：初始化背景及命名概要方案交接",
      "evidence": "Map 已移除背景、身份与报告筛选协议；S1 测试独立断言来源声明，并经公开 edit/save/read 检查不丢失。TDD red 在旧定义 inputs 为 Null 时失败，green 通过。"
    },
    {
      "criteria": "AC3–AC5：九章节、真实确认、能力边界",
      "evidence": "按既有概要规划 Task 九章节契约保留业务要求，明确方案确认与报告发送确认分离、阻塞 false、用户手动补齐能力及禁止自动创建／派发；属定义内容审阅，不声称已证明真实用户确认行为。"
    },
    {
      "criteria": "AC6–AC7：忠实翻译、安全交付、不自动启动",
      "evidence": "Map 指示保留字段核对、服务结构校验、冲突不覆盖、仅明确配置时只读预检、缺输入／不可读／部分落盘报告与缺启动配置合法定义交付；S1 测试实际证明定义操作后无 Run、无宿主调用，不冒充生成业务的真实执行验收。"
    },
    {
      "criteria": "AC8：说明、受影响检查、服务保存读回",
      "evidence": "planning_definition_round_trips_named_handoff_without_starting_work 在隔离服务通过，保护两节点、绑定、依赖、报告策略、命名引用和完整读回；文档链接检查及完整后端回归通过。"
    }
  ],
  "verification": [
    {
      "command": "cargo test --manifest-path backend/Cargo.toml --test workflow_dispatch plan_workflow:: -- --nocapture",
      "result": "先 red（缺少命名来源断言失败），迁移后 green：1 passed。"
    },
    {
      "command": "node scripts/check-workflow-docs.mjs",
      "result": "通过：17 documents、159 localLinks、0 errors。"
    },
    {
      "command": "node scripts/test-backend.mjs",
      "result": "首次因未生成 backend/static/index.html 的构建前提失败，40 个 binaries 未派发、doctest 未运行；未修改测试规避。"
    },
    {
      "command": "npm --prefix frontend ci --no-audit --no-fund && npm --prefix frontend run build",
      "result": "依赖安装及真实前端静态资源构建成功，补齐后端进程测试前提。"
    },
    {
      "command": "node scripts/test-backend.mjs",
      "result": "补齐前提后全量通过：build、全部 binaries、doctest passed；0 binaries not dispatched。日志与 summary：C:/Users/hugos/AppData/Local/Temp/herdr-backend-tests-pN7l7F/summary.json。"
    }
  ],
  "unverified": [
    "P27/P28 完整真实宿主 outline → generate_map、实际业务文件翻译质量、真实用户确认与 manual 报告发送确认凭据，按票据责任由 09 独占验证，本票未声称通过。"
  ],
  "ticket_status": "未改票据状态；依项目规则 closed 还要求完成集成。"
}
