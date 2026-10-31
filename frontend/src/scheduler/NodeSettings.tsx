type Props = { content: string; disabled: boolean; onChange: (content: string) => void }

/** Controls edit the same JSON draft, never a second set of pending values. */
export function NodeSettings({ content, disabled, onChange }: Props) {
  let definition: Record<string, unknown> | null = null
  let jsonError: string | null = null
  try {
    const value: unknown = JSON.parse(content)
    if (typeof value !== 'object' || value === null || Array.isArray(value)) {
      jsonError = '请先将 node 内容修正为 JSON 对象，再修改设置。'
    } else {
      definition = value as Record<string, unknown>
      if (Object.hasOwn(definition, 'auto_close') && typeof definition.auto_close !== 'boolean') {
        jsonError = '请先修正 JSON 中的 auto_close：必须是布尔值或省略。'
      } else if (Object.hasOwn(definition, 'report_policy') && definition.report_policy !== 'auto' && definition.report_policy !== 'manual') {
        jsonError = '请先修正 JSON 中的 report_policy：必须是 auto、manual 或省略。'
      }
    }
  } catch {
    jsonError = '请先修正 node 内容 JSON，再修改设置；原始草稿保持不变。'
  }
  function change(patch: Record<string, unknown>) {
    if (definition && !jsonError) onChange(JSON.stringify({ ...definition, ...patch }, null, 2))
  }
  return <div className="scheduler-node-settings">
    <label className="scheduler-setting-choice"><input type="checkbox" checked={definition?.auto_close === true} disabled={disabled || jsonError !== null} aria-describedby="node-auto-close-help" onChange={event => change({ auto_close: event.target.checked })} />完成后自动关闭</label>
    <p id="node-auto-close-help">关闭 Tab 会终止 Tab 内终端及相关进程。</p>
    <fieldset disabled={disabled || jsonError !== null}>
      <legend>报告策略</legend>
      {(['auto', 'manual'] as const).map(policy => <label key={policy} className="scheduler-setting-choice"><input type="radio" name="node-report-policy" value={policy} checked={(definition?.report_policy ?? 'auto') === policy} onChange={() => change({ report_policy: policy })} />{policy}</label>)}
    </fieldset>
    {jsonError && <p>{jsonError}</p>}
  </div>
}
