# implement

```json
{
  "role": "implement",
  "agent": "tdd",
  "agent_id": "03-map-rules-and-editormd-f5eda6d5-87dc-47b0-97b",
  "heads": {
    "bot": "a9dd09bce0a6ed2f05948a980100c0e10081bbc6"
  },
  "exitCode": 0,
  "aborted": false,
  "truncated": false
}
```

票据 03 已提交 `a9dd09bce0`，未合入集成分支。

- **P5／P29**：Map 规则可分别省略，输入 `required` 必须是显式布尔值，输出声明即必交。旧字段、未知字段和非法字面对象被拒绝；HTTP 读回与实际文件确认拒绝未改写定义。
- **P6／P7**：保留三类材料来源、固定／动态 zone、空 `input`、策略与图校验，材料引用不补 `blocked_by`。父节点保存时只按子 Map 的公开 ID 对接，可选输出子集，无需匹配内部输出 ID。
- **P31**：真实浏览器已验证编辑、保存和重载，并与 HTTP 读回及实际文件核对；仅有输出说明不会生成文件要求。已同步类型、示例、文档和词汇。

定向验证：`workflow_dispatch` 的 `map_rules::` 及相关 node 保存回归共 **10 通过**；`workflow_start` 的 `named_input_declarations_are_local_and_duplicate_save_preserves_map` **1 通过**；指定 `schedulerNodeEditing.spec.ts --workers=1` **11 通过**；前端两文件回归 **26 通过**。

未迁移仍使用旧格式的发布资产，也未验证其可在新解析器下运行；选定资产迁移归 08，现场切换归 14。P7 的运行时成果交付不在本票验证范围内。
