# 创建task-V2 材料契约

生成或消费材料时读取对应章节。

## created_task

按同目录上一级 create-task-v2.json 的 map_output_rule 中 created_task 声明生成身份文件，指向本次实际创建的 Task。身份文件用于定位，当前配置按该 ID 从服务读取。

## request

生成前，读取 ../create-bot-v2/materials.md「request」，按该接口生成目标定位文件。type 使用 task，ref 使用本次创建并读回的真实 Task ID，与 created_task 指向同一对象。

此文件只定位目标；创建bot-V2 自行读取真实 Task 及其引用资料。

## 材料写入授权

调用本流程即允许在本次运行提供的内部材料位置，新建必需交接文件；覆盖已有文件仍须明确授权。
