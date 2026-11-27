# 创建bot-V2 材料契约

准备或消费材料时读取对应章节。文件位置使用本次命名绑定；需求、约束、已有确认及参考资料路径与用途由本次要求提供。

## request

UTF-8 JSON 对象，包含两个非空字符串字段：

- type：目标类型，task、script 或 map。
- ref：type 为 task 时，提供 Task ID 或可唯一定位的名称；为 script 时，提供脚本路径；为 map 时，提供 Map 文件名或可唯一定位的名称。

目标为既有 normal Task、Map 或 TypeScript 脚本。W1 核实实际对象，歧义由用户澄清；准确身份记录在需求交接中，原始 request 保持不变。

W2 直接消费同一 request，结合需求交接中的准确身份读取当前目标。

## bot_requirements

W1 生成、W2 消费的 UTF-8 Markdown 需求交接文件，内容遵循「Bot 需求对齐者」Task 的“交付需求”要求。

保留已确认结论及必要上下文；完整配置和源码由 W2 从真实目标读取，不另行生成目标快照。

## created_bot

按同目录上一级 create-bot-v2.json 的 map_output_rule 中 created_bot 声明生成身份文件，指向本次实际创建的 Bot。身份文件用于定位，当前配置按该 ID 从服务读取。

## 材料写入授权

调用本流程即允许在本次运行提供的内部材料位置，新建必需交接文件；覆盖已有文件仍须明确授权。
