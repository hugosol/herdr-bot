/**
 * chat-name 命名规则的字段与示例预览。
 *
 * 只承诺四个字段：`{zone.name}`、`{task.name}`、`{agent}`、`{chat.id}`。示例值用于编辑时观察
 * 规则效果；创建弹窗中的真实预览由创建流程负责。这里不对未知占位符做额外诊断。
 */

export const INITIAL_CHAT_NAME = '{zone.name} · {task.name} · {chat.id}'

/** 编辑时可插入的四个已承诺字段。 */
export const NAMING_FIELDS = [
  { key: 'zone-name', token: '{zone.name}' },
  { key: 'task-name', token: '{task.name}' },
  { key: 'agent', token: '{agent}' },
  { key: 'chat-id', token: '{chat.id}' },
] as const

/** 用示例值或创建流程提供的真实上下文渲染命名规则；未知占位符保持原样。 */
export function renderChatNameExample(
  rule: string,
  taskName: string,
  agent: string,
  context?: { zoneName: string; chatId: string },
): string {
  const values: Record<string, string> = {
    '{zone.name}': context?.zoneName ?? 'alpha',
    '{task.name}': context === undefined
      ? taskName.trim() === '' ? '审查' : taskName.trim()
      : taskName,
    '{agent}': agent,
    '{chat.id}': context?.chatId ?? 'a7f3c2',
  }
  return rule.replace(/\{(zone\.name|task\.name|agent|chat\.id)\}/g, (token) => values[token])
}
