/**
 * bot-name 命名规则的字段与示例预览。
 *
 * 只承诺四个字段：`{zone.name}`、`{template.name}`、`{agent}`、`{bot.id}`。示例值用于编辑时观察
 * 规则效果；创建弹窗中的真实预览由创建流程负责。这里不对未知占位符做额外诊断。
 */

export const INITIAL_BOT_NAME = '{zone.name} · {template.name} · {bot.id}'

/** 编辑时可插入的四个已承诺字段。 */
export const NAMING_FIELDS = [
  { key: 'zone-name', token: '{zone.name}' },
  { key: 'template-name', token: '{template.name}' },
  { key: 'agent', token: '{agent}' },
  { key: 'bot-id', token: '{bot.id}' },
] as const

/** 用示例值或创建流程提供的真实上下文渲染命名规则；未知占位符保持原样。 */
export function renderBotNameExample(
  rule: string,
  templateName: string,
  agent: string,
  context?: { zoneName: string; botId: string },
): string {
  const values: Record<string, string> = {
    '{zone.name}': context?.zoneName ?? 'alpha',
    '{template.name}': context === undefined
      ? templateName.trim() === '' ? '审查' : templateName.trim()
      : templateName,
    '{agent}': agent,
    '{bot.id}': context?.botId ?? 'a7f3c2',
  }
  return rule.replace(/\{(zone\.name|template\.name|agent|bot\.id)\}/g, (token) => values[token])
}
