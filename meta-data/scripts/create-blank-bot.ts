import { isDeepStrictEqual } from "node:util";
import { Work, fail, inputs, request, rules, saveCode, type Bot, type Context } from "./bot-materials.ts";

export default async function script(context: Context) {
  const work = new Work(context);
  return work.run(async () => {
    const files = inputs(context);
    const seen = new Set<string>();
    if (files.some(file => !["request", "materials"].includes(file.id) || seen.has(file.id) || !seen.add(file.id))) fail("validate", "ambiguous-request", "创建仅消费唯一 request 和可选 materials 命名绑定。");
    const value = await work.document("request");
    const specification = request(value);
    rules(context, ["baseline", "handoff"]);
    const target = await work.call("target", specification.target, "read_current", "target-conflict");
    const identity = specification.bot ?? {};
    const id = identity.id ?? target.default_id;
    const expected: Bot = {
      id, name: identity.name ?? target.name,
      ...(identity.group === undefined ? {} : { group: identity.group }),
      when: "", description: "", input_rule: "", output_rule: "",
      ...(specification.target.type === "task" ? { task_id: specification.target.task_id } : { map_file: specification.target.map_file }),
    };
    const { id: _, ...content } = expected;
    work.write = "failed";
    await work.call("bot_save", { id, content, revision: null }, "save", error => saveCode(error, "identity-conflict"));
    work.write = "succeeded";
    work.readBack = "failed";
    const read = await work.call("bot_read", { id }, "read_back", "read-back-failed");
    if (!isDeepStrictEqual(read.bot, expected)) fail("read_back", "read-back-failed", "创建后完整 Bot 与预期不一致。");
    const after = await work.call("target", specification.target, "read_back", "read-back-failed");
    if (!isDeepStrictEqual(after.configuration, target.configuration)) fail("read_back", "target-conflict", "创建期间目标配置发生变化；不回滚已创建 Bot。");
    work.readBack = "succeeded";
    await work.material("baseline", {
      schema_version: 1, kind: "baseline", bot: read.bot, revision: read.revision,
      target: { ...specification.target, configuration: target.configuration }, request: value,
    });
    await work.material("handoff", {
      schema_version: 1, kind: "handoff", stage: "created", bot_id: id, target: specification.target,
      sources: files.map(file => ({ binding_id: file.id, direction: "input", role: file.id })),
      checks: { bot_read_back: true, target_unchanged: true },
      unverified: ["仅创建定义；未执行目标业务，也未保存四字段正文。"],
    });
  });
}
