# 创建task-script type 材料契约

## created_script

按同目录上一级 create-task-script-type.json 的公开输出声明生成身份文件。

生成前，确认正式脚本已保存并完整读回，与最终实现一致。script 路径遵循产品仓库 docs/scheduler.md「TypeScript 脚本目标的表示契约」。

身份文件不表示脚本业务运行已验证。

## request

生成前，读取 ../create-bot-v2/materials.md「request」。

type 使用 script，ref 使用 created_script 中的相对脚本路径，指向同一正式脚本。下游自行读取真实脚本及必要引用资料。

## 材料写入授权

调用本流程即允许在本次运行提供的内部材料位置，新建必需交接文件；覆盖已有文件仍须明确授权。
