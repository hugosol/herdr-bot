import { useId } from 'react'
import type { NodeZone } from '../../controllers/schedulerController'

export type NodeSettingsValue = { zone: NodeZone; report_policy: 'auto' | 'manual'; auto_close: boolean }
type Props = { value: NodeSettingsValue; script?: boolean } & (
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
        {!props.readOnly && !props.script && <fieldset className="scheduler-policy-options" aria-label="report_policy" disabled={props.disabled}>
          {(['auto', 'manual'] as const).map(policy => <label key={policy} className="scheduler-setting-choice"><input type="radio" name={`${id}-report-policy`} value={policy} checked={value.report_policy === policy} onChange={() => props.onChange({ report_policy: policy })} />{policy}</label>)}
        </fieldset>}
      </div>
      <div className="scheduler-status-setting" data-auto-close={String(value.auto_close)}>
        <span className="scheduler-setting-label">auto_close</span>
        <strong className="scheduler-state-mark">{value.auto_close ? '已开启' : '已关闭'}<small>{String(value.auto_close)}</small></strong>
        {!props.readOnly && !props.script && <label className="scheduler-setting-choice"><input type="checkbox" aria-label="auto_close" checked={value.auto_close} disabled={props.disabled} aria-describedby={`${id}-close-help`} onChange={event => props.onChange({ auto_close: event.target.checked })} />启用自动关闭</label>}
      </div>
    </div>
    {props.script && <div className="scheduler-help"><p>脚本节点固定自动报告（auto），无 Chat 或 Tab 可关闭（auto_close:false）。</p>
      {(value.report_policy !== 'auto' || value.auto_close) && <><p role="status">当前策略与脚本冲突；原值已保留，请显式修正后保存。</p>
        {!props.readOnly && <button type="button" disabled={props.disabled} onClick={() => props.onChange({ report_policy: 'auto', auto_close: false })}>改为自动报告且不关闭会话</button>}
      </>}
    </div>}
    <div className="scheduler-zone-setting">
      {props.readOnly ? <><span className="scheduler-setting-label">zone</span><p className="scheduler-node-value scheduler-identifier">{typeof value.zone === 'string' ? value.zone || '尚未绑定 zone' : 'dynamic · 每次执行时指定，尚未指定 zone'}</p></> : <>
        <fieldset className="scheduler-policy-options" aria-label="zone 类型" disabled={props.disabled}>
          <label className="scheduler-setting-choice"><input type="radio" name={`${id}-zone-type`} checked={typeof value.zone === 'string'} onChange={() => props.onChange({ zone: '' })} />固定 zone</label>
          <label className="scheduler-setting-choice"><input type="radio" name={`${id}-zone-type`} checked={typeof value.zone !== 'string'} onChange={() => props.onChange({ zone: { type: 'dynamic' } })} />动态 zone</label>
        </fieldset>
        {typeof value.zone === 'string' ? <><label htmlFor={`${id}-zone`}>zone</label><input id={`${id}-zone`} value={value.zone} placeholder="namespace/name" aria-describedby={`${id}-zone-help`} disabled={props.disabled} onChange={event => props.onChange({ zone: event.target.value })} /></> : <p>dynamic · 不保存预设；每次执行必须显式指定已登记的 zone。</p>}
      </>}
    </div>
    {!props.readOnly && <div className="scheduler-setting-help">
      <p id={`${id}-zone-help`}>固定 zone 填写稳定 ID，不是本机目录；保存 Map 无需先登记，启动 Run 时校验固定绑定。动态 zone 无预设，每次执行时校验显式选择。</p>
      <p id={`${id}-close-help`}>关闭 Tab 会终止 Tab 内终端及相关进程。</p>
    </div>}
  </section>
}
