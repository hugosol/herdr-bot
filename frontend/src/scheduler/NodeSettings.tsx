export type NodeSettingsValue = { zone: string; report_policy: 'auto' | 'manual'; auto_close: boolean }
type Props = { value: NodeSettingsValue; disabled: boolean; onChange: (patch: Partial<NodeSettingsValue>) => void }

/** Settings remain editable without rewriting the node content JSON draft. */
export function NodeSettings({ value, disabled, onChange }: Props) {
  return <div className="scheduler-node-settings">
    <label>zone<input value={value.zone} disabled={disabled} onChange={event => onChange({ zone: event.target.value })} /></label>
    <label className="scheduler-setting-choice"><input type="checkbox" checked={value.auto_close} disabled={disabled} aria-describedby="node-auto-close-help" onChange={event => onChange({ auto_close: event.target.checked })} />auto_close</label>
    <p id="node-auto-close-help">关闭 Tab 会终止 Tab 内终端及相关进程。</p>
    <fieldset disabled={disabled}>
      <legend>report_policy</legend>
      {(['auto', 'manual'] as const).map(policy => <label key={policy} className="scheduler-setting-choice"><input type="radio" name="node-report-policy" value={policy} checked={value.report_policy === policy} onChange={() => onChange({ report_policy: policy })} />{policy}</label>)}
    </fieldset>
  </div>
}
