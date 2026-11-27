import { useId } from 'react'
import type { NodeZone } from '../../controllers/schedulerController'

export type NodeSettingsValue = { zone: NodeZone; auto_approved: boolean; auto_close: boolean; end_node?: boolean }
type Props = { value: NodeSettingsValue; script?: boolean; endNodeProblem?: string } & (
  { readOnly: true; disabled?: never; onChange?: never } |
  { readOnly?: false; disabled: boolean; onChange: (patch: Partial<NodeSettingsValue>) => void }
)

/** One configuration layout in both modes; settings never rewrite the content JSON draft. */
export function NodeSettings(props: Props) {
  const id = useId()
  const { value } = props
  return <section aria-label="node 配置概览" className="scheduler-node-settings">
    <div className="scheduler-status-setting">
      <span className="scheduler-setting-label">end_node</span>
      {props.readOnly ? <strong>{value.end_node ? '结束节点 · 交给用户' : '普通节点'}</strong> : <label className="scheduler-setting-choice"><input type="checkbox" aria-label="end_node" checked={value.end_node === true} disabled={props.disabled || (!value.end_node && !!props.endNodeProblem)} aria-describedby={`${id}-end-help`} onChange={event => props.onChange({ end_node: event.target.checked })} />结束节点 · 交给用户</label>}
      <p id={`${id}-end-help`} className="scheduler-help">每个 Map 最多一个结束节点，只能绑定普通 normal Task 的 Bot（非 Map／脚本），且不能有后继。其他所有节点完成后才可派发；派发成功后由管理者显式结束 Run，不表示业务完成。无需节点报告或 complete，Chat 保持开放且可 Resume，不自动关闭。后端最终校验。</p>
      {props.endNodeProblem && <p role="status">{props.endNodeProblem}；原配置保留，请显式修正。</p>}
      {value.end_node && <p className="scheduler-help">成果批准／auto_close 对结束节点不适用；原值保留，取消结束节点后恢复显示。</p>}
    </div>
    {!value.end_node && <div className="scheduler-status-grid">
      <div className="scheduler-status-setting" data-policy={value.auto_approved === false ? 'manual' : 'auto'}>
        <span className="scheduler-setting-label">成果批准 · auto_approved</span>
        <strong className="scheduler-state-mark">{value.auto_approved === false ? '人工批准' : '自动批准'}<small>{String(value.auto_approved)}</small></strong>
        {!props.readOnly && !props.script && <fieldset className="scheduler-policy-options" aria-label="auto_approved" disabled={props.disabled}>
          {(['auto', 'manual'] as const).map(mode => <label key={mode} className="scheduler-setting-choice"><input type="radio" aria-label={mode} name={`${id}-approval-mode`} value={mode} checked={value.auto_approved === (mode === 'auto')} onChange={() => props.onChange({ auto_approved: mode === 'auto' })} />{mode === 'auto' ? '自动（默认）' : '人工'}</label>)}
        </fieldset>}
        <p className="scheduler-help">模式不是已批准事实。人工模式由用户在节点 Chat 原生确认中明确 Yes；报告发送无需确认，批准不会完成或推进节点。</p>
      </div>
      <div className="scheduler-status-setting" data-auto-close={String(value.auto_close)}>
        <span className="scheduler-setting-label">auto_close</span>
        <strong className="scheduler-state-mark">{value.auto_close ? '已开启' : '已关闭'}<small>{String(value.auto_close)}</small></strong>
        {!props.readOnly && !props.script && <label className="scheduler-setting-choice"><input type="checkbox" aria-label="auto_close" checked={value.auto_close} disabled={props.disabled} aria-describedby={`${id}-close-help`} onChange={event => props.onChange({ auto_close: event.target.checked })} />启用自动关闭</label>}
      </div>
    </div>}
    {props.script && <div className="scheduler-help"><p>脚本节点固定自动批准（auto_approved:true），成功校验后自动报告，无 Chat 或 Tab 可关闭（auto_close:false）。</p>
      {(value.auto_approved === false || value.auto_close) && <><p role="status">当前策略与脚本冲突；原值已保留，请显式修正后保存。</p>
        {!props.readOnly && <button type="button" disabled={props.disabled} onClick={() => props.onChange({ auto_approved: true, auto_close: false })}>改为自动批准且不关闭会话</button>}
      </>}
    </div>}
    <div className="scheduler-zone-setting">
      {props.readOnly ? <><span className="scheduler-setting-label">zone</span><p className="scheduler-node-value scheduler-identifier">{typeof value.zone === 'string' ? value.zone || '尚未绑定 zone' : value.zone.input_id === undefined ? 'dynamic · 每次执行时指定，尚未指定 zone' : `dynamic · 来自节点输入 ${value.zone.input_id}`}</p></> : <>
        <fieldset className="scheduler-policy-options" aria-label="zone 类型" disabled={props.disabled}>
          <label className="scheduler-setting-choice"><input type="radio" name={`${id}-zone-type`} checked={typeof value.zone === 'string'} onChange={() => props.onChange({ zone: '' })} />固定 zone</label>
          <label className="scheduler-setting-choice"><input type="radio" name={`${id}-zone-type`} checked={typeof value.zone !== 'string' && value.zone.input_id === undefined} onChange={() => props.onChange({ zone: { type: 'dynamic' } })} />手动动态 zone</label>
          <label className="scheduler-setting-choice"><input type="radio" name={`${id}-zone-type`} checked={typeof value.zone !== 'string' && value.zone.input_id !== undefined} onChange={() => props.onChange({ zone: { type: 'dynamic', input_id: '' } })} />输入驱动的动态 zone</label>
        </fieldset>
        {typeof value.zone === 'string' ? <>
          <label htmlFor={`${id}-zone`}>zone</label><input id={`${id}-zone`} value={value.zone} placeholder="namespace/name" aria-describedby={`${id}-zone-help`} disabled={props.disabled} onChange={event => props.onChange({ zone: event.target.value })} />
          <p id={`${id}-zone-help`} className="scheduler-help">填写稳定 ID，不是本机目录；保存 Map 无需先登记，启动 Run 时校验固定绑定。</p>
        </> : value.zone.input_id === undefined ? <p>dynamic · 不保存预设；每次执行必须显式指定已登记的 zone。</p> : <>
          <label htmlFor={`${id}-zone-input`}>节点输入 ID</label><input id={`${id}-zone-input`} value={value.zone.input_id} aria-describedby={`${id}-zone-input-help`} disabled={props.disabled} onChange={event => props.onChange({ zone: { type: 'dynamic', input_id: event.target.value } })} />
          <p id={`${id}-zone-input-help`} className="scheduler-help">填写本节点 input_rule 的 ID。派发时读取该输入 JSON 的 zone_id，不接受手动指定。</p>
        </>}
      </>}
    </div>
    {!props.readOnly && <div className="scheduler-setting-help">
      <p id={`${id}-close-help`}>关闭 Tab 会终止 Tab 内终端及相关进程。</p>
    </div>}
  </section>
}
