import { createHash } from "node:crypto";
import { isDeepStrictEqual } from "node:util";
import { Work, bot, fail, guide, inputs, locator, object, request, rules, saveCode, string, version, type Context } from "./bot-materials.ts";

const fields = [
  ["when", "when_text", "when_handoff"],
  ["plan_guide", "plan_guide_text", "plan_guide_handoff"],
  ["node_guide", "node_guide_text", "node_guide_handoff"],
] as const;

export default async function script(context: Context) {
  const work = new Work(context);
  return work.run(async () => {
    const files = inputs(context);
    const required = ["baseline", ...fields.flatMap(([, text, handoff]) => [text, handoff])];
    const ids = new Set<string>();
    for (const file of files) {
      if (!required.includes(file.id) || ids.has(file.id)) fail("validate", "ambiguous-request", "保存只消费唯一 baseline 和三组 text/handoff。");
      ids.add(file.id);
    }
    if (required.some(id => !ids.has(id))) fail("validate", "missing-field", "缺少 baseline 或三组 text/handoff。");
    rules(context, ["saved", "handoff"]);
    const baseline = object(await work.document("baseline"), ["schema_version", "kind", "bot", "revision", "target", "request"]);
    version(baseline.schema_version); string(baseline.kind);
    const specification = request(baseline.request);
    const originalBot = bot(baseline.bot);
    const rawRevision = object(baseline.revision, ["file", "original"]);
    string(rawRevision.file); string(rawRevision.original);
    const revision = { file: rawRevision.file, original: rawRevision.original };
    const snapshot = object(baseline.target, ["type", "configuration"], ["task_id", "map_file"]);
    const { configuration, ...rawTarget } = snapshot;
    const target = locator(rawTarget);
    if (baseline.schema_version !== 1 || baseline.kind !== "baseline" || !revision.file || !revision.original || originalBot.when || originalBot.plan_guide || originalBot.node_guide) fail("validate", "invalid-format", "创建 baseline 必须是三空指南的完整 v1 对象和真实 revision。");
    const original = await work.call("parse_json", { text: revision.original }, "validate", "invalid-format");
    if (original === null || typeof original !== "object" || !("id" in original) || original.id !== originalBot.id || (specification.bot?.id !== undefined && specification.bot.id !== originalBot.id)) fail("validate", "identity-conflict", "baseline Bot、request 与 revision 身份不一致。");
    if (!isDeepStrictEqual(original, originalBot)) fail("validate", "revision-conflict", "baseline 完整 Bot 与 revision.original 不一致。");
    if (originalBot.script !== undefined || originalBot.task_id !== target.task_id || originalBot.map_file !== target.map_file || !isDeepStrictEqual(specification.target, target)) fail("validate", "target-conflict", "baseline/request/唯一 Bot 绑定不一致。");
    const candidate = { ...originalBot };
    const evidence: string[] = [];
    const unverified: string[] = [];
    for (const [field, textId, handoffId] of fields) {
      const input = files.find(file => file.id === textId)!;
      const text = await work.call("read_text", { path: input.path }, "parse", "invalid-format");
      string(text);
      const handoff = guide(await work.document(handoffId));
      const confirmation = handoff.confirmation;
      if (handoff.schema_version !== 1 || handoff.kind !== "handoff" || handoff.stage !== "guide") fail("validate", "invalid-format", "指南 handoff 格式无效。");
      if (handoff.bot_id !== originalBot.id) fail("validate", "identity-conflict", "指南不属于共同 Bot。");
      if (handoff.field !== field || !isDeepStrictEqual(handoff.target, target)) fail("validate", "target-conflict", "指南字段或目标不对应；不交换材料。");
      const sources = new Set<string>();
      let invalidSource = handoff.sources.length === 0;
      for (const source of handoff.sources) {
        const key = JSON.stringify([source.direction, source.binding_id]);
        if (!source.binding_id || !["input", "output"].includes(source.direction) || !["request", "materials", "baseline", "text", "handoff"].includes(source.role) || sources.has(key)) invalidSource = true;
        sources.add(key);
      }
      if (invalidSource || !handoff.sources.some(source => source.direction === "input" && source.role === "baseline") || !handoff.sources.some(source => source.direction === "output" && source.role === "text") || [...handoff.constraints, ...handoff.unverified].some(item => !item)) fail("validate", "invalid-format", "指南来源或限制格式无效；不沿来源索引寻址。");
      const digest = createHash("sha256").update(text, "utf8").digest("hex");
      if (!confirmation.evidence || !confirmation.full_text || !confirmation.semantics_preserved || confirmation.text_sha256 !== digest || !confirmation.save_authorized) fail("validate", "authorization-required", "确认必须覆盖本字段最终原样 bytes、保义及保存授权。");
      candidate[field] = text;
      evidence.push(confirmation.evidence);
      unverified.push(...handoff.unverified);
    }
    const current = await work.call("bot_read", { id: originalBot.id }, "read_current", "identity-conflict");
    if (!isDeepStrictEqual(current.revision, revision)) fail("read_current", "revision-conflict", "真实 revision 已变化；不按相同正文认定成功。");
    const before = await work.call("target", target, "read_current", "target-conflict");
    if (!isDeepStrictEqual(before.configuration, configuration)) fail("read_current", "target-conflict", "完整目标配置与 baseline 不一致。");
    const { id, ...content } = candidate;
    work.write = "failed";
    await work.call("bot_save", { id, content, revision }, "save", error => saveCode(error, "revision-conflict"));
    work.write = "succeeded";
    work.readBack = "failed";
    const read = await work.call("bot_read", { id }, "read_back", "read-back-failed");
    if (!isDeepStrictEqual(read.bot, candidate)) fail("read_back", "read-back-failed", "写后完整 Bot 与三字段候选不一致。");
    const after = await work.call("target", target, "read_back", "read-back-failed");
    if (!isDeepStrictEqual(after.configuration, before.configuration)) fail("read_back", "target-conflict", "保存期间目标变化；保留实际效果，不回滚。");
    work.readBack = "succeeded";
    await work.material("saved", { schema_version: 1, kind: "saved", bot: read.bot, revision: read.revision, target: { ...target, configuration: after.configuration } });
    await work.material("handoff", {
      schema_version: 1, kind: "handoff", stage: "saved", bot_id: id, target,
      sources: files.map(file => ({ binding_id: file.id, direction: "input", role: file.id === "baseline" ? "baseline" : file.id.endsWith("_text") ? "text" : "handoff" })),
      confirmation_evidence: evidence, checks: { bot_read_back: true, target_unchanged: true }, unverified,
    });
  });
}
