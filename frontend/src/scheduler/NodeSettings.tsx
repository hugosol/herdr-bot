import { useId } from 'react'

export type NodeSettingsValue = { zone: string; report_policy: 'auto' | 'manual'; auto_close: boolean }
type Props = { value: NodeSettingsValue } & (
  { readOnly: true; disabled?: never; onChange?: never } |
  { readOnly?: false; disabled: boolean; onChange: (patch: Partial<NodeSettingsValue>) => void }
)

/** One configuration layout in both modes; settings never rewrite the content JSON draft. */
export function NodeSettings(props: Props) {
  const id = useId()
  const { value } = props
  return <section aria-label="node 配置概览" className="scheduler-node-settings">
    <div className="scheduler-status-grid">
      <div className="scheduler-status-setting" data-policy={value.report_policy}>
        <span className="scheduler-setting-label">report_policy</span>
        <strong className="scheduler-state-mark">{value.report_policy === 'manual' ? '手动报告' : '自动报告'}<small>{value.report_policy}</small></strong>
        {!props.readOnly && <fieldset className="scheduler-policy-options" aria-label="report_policy" disabled={props.disabled}>
          {(['auto', 'manual'] as const).map(policy => <label key={policy} className="scheduler-setting-choice"><input type="radio" name={`${id}-report-policy`} value={policy} checked={value.report_policy === policy} onChange={() => props.onChange({ report_policy: policy })} />{policy}</label>)}
        </fieldset>}
      </div>
      <div className="scheduler-status-setting" data-auto-close={String(value.auto_close)}>
        <span className="scheduler-setting-label">auto_close</span>
        <strong className="scheduler-state-mark">{value.auto_close ? '已开启' : '已关闭'}<small>{String(value.auto_close)}</small></strong>
        {!props.readOnly && <label className="scheduler-setting-choice"><input type="checkbox" aria-label="auto_close" checked={value.auto_close} disabled={props.disabled} aria-describedby={`${id}-close-help`} onChange={event => props.onChange({ auto_close: event.target.checked })} />启用自动关闭</label>}
      </div>
    </div>
    <div className="scheduler-zone-setting">
      <label htmlFor={`${id}-zone`}>zone</label>
      {props.readOnly ? <p className="scheduler-node-value">{value.zone || '尚未绑定 zone'}</p> : <input id={`${id}-zone`} value={value.zone} placeholder="namespace/name" aria-describedby={`${id}-zone-help`} disabled={props.disabled} onChange={event => props.onChange({ zone: event.target.value })} />}
    </div>
    {!props.readOnly && <div className="scheduler-setting-help">
      <p id={`${id}-zone-help`}>填写稳定 zone ID，不是本机目录；保存 Map 无需先登记，启动 Run 时校验绑定。</p>
      <p id={`${id}-close-help`}>关闭 Tab 会终止 Tab 内终端及相关进程。</p>
    </div>}
  </section>
}
