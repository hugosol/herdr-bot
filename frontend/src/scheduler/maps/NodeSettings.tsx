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
      <span className="scheduler-setting-label">节点用途</span>
      {props.readOnly ? <strong>{value.end_node ? '结束节点 · 交给用户' : '普通节点'}</strong> : <label className="scheduler-setting-choice"><input type="checkbox" aria-label="end_node" checked={value.end_node === true} disabled={props.disabled || (!value.end_node && !!props.endNodeProblem)} aria-describedby={`${id}-end-help`} onChange={event => props.onChange({ end_node: event.target.checked })} />结束节点 · 交给用户</label>}
      {(!props.readOnly || value.end_node) && <p id={`${id}-end-help`} className="scheduler-help">结束节点在其他节点完成后，将后续工作交给用户。Chat 保持开放；Run 结束不代表业务已完成。</p>}
      {props.endNodeProblem && <p role="status">{props.endNodeProblem}。</p>}
      {!props.readOnly && <details className="scheduler-format-help"><summary>结束节点使用条件</summary><p>每个 Map 最多一个，只能绑定普通 Task 的 Bot，且不能有后继节点。结束节点无需成果批准或节点报告，也不会自动关闭；取消后恢复原有批准与关闭设置。</p></details>}
    </div>
    {!value.end_node && <div className="scheduler-status-grid">
      <div className="scheduler-status-setting" data-policy={value.auto_approved === false ? 'manual' : 'auto'}>
        <span className="scheduler-setting-label">成果批准</span>
        <strong className="scheduler-state-mark">{value.auto_approved === false ? '人工批准' : '自动批准'}</strong>
        {!props.readOnly && !props.script && <fieldset className="scheduler-policy-options" aria-label="auto_approved" disabled={props.disabled}>
          {(['auto', 'manual'] as const).map(mode => <label key={mode} className="scheduler-setting-choice"><input type="radio" aria-label={mode} name={`${id}-approval-mode`} value={mode} checked={value.auto_approved === (mode === 'auto')} onChange={() => props.onChange({ auto_approved: mode === 'auto' })} />{mode === 'auto' ? '自动（默认）' : '人工'}</label>)}
        </fieldset>}
        {!props.script && <p className="scheduler-help">{value.auto_approved === false ? '需在节点 Chat 中确认成果。' : '由执行者批准成果。'}之后仍由管理者确认节点完成。</p>}
      </div>
      <div className="scheduler-status-setting" data-auto-close={String(value.auto_close)}>
        <span className="scheduler-setting-label">完成后自动关闭</span>
        <strong className="scheduler-state-mark">{value.auto_close ? '已开启' : '已关闭'}</strong>
        {!props.readOnly && !props.script && <label className="scheduler-setting-choice"><input type="checkbox" aria-label="auto_close" checked={value.auto_close} disabled={props.disabled} aria-describedby={`${id}-close-help`} onChange={event => props.onChange({ auto_close: event.target.checked })} />启用自动关闭</label>}
      </div>
    </div>}
    {props.script && <div className="scheduler-help"><p>脚本节点自动批准并提交报告，不创建 Chat，无需关闭 Tab。</p>
      {(value.auto_approved === false || value.auto_close) && <><p role="status">脚本节点需使用自动批准并关闭自动关闭选项。</p>
        {!props.readOnly && <button type="button" disabled={props.disabled} onClick={() => props.onChange({ auto_approved: true, auto_close: false })}>改为自动批准且不关闭会话</button>}
      </>}
    </div>}
    <div className="scheduler-zone-setting">
      {props.readOnly ? <><span className="scheduler-setting-label">zone</span><p className="scheduler-node-value scheduler-identifier">{typeof value.zone === 'string' ? value.zone || '尚未绑定 zone' : '动态 zone · 由管理者在每次执行前指定'}</p></> : <>
        <fieldset className="scheduler-policy-options" aria-label="zone 类型" disabled={props.disabled}>
          <label className="scheduler-setting-choice"><input type="radio" name={`${id}-zone-type`} checked={typeof value.zone === 'string'} onChange={() => props.onChange({ zone: '' })} />固定 zone</label>
          <label className="scheduler-setting-choice"><input type="radio" name={`${id}-zone-type`} checked={typeof value.zone !== 'string'} onChange={() => props.onChange({ zone: { type: 'dynamic' } })} />动态 zone</label>
        </fieldset>
        {typeof value.zone === 'string' ? <><label htmlFor={`${id}-zone`}>zone</label><input id={`${id}-zone`} value={value.zone} placeholder="namespace/name" aria-describedby={`${id}-zone-help`} disabled={props.disabled} onChange={event => props.onChange({ zone: event.target.value })} /></> : <p>由管理者在每次执行前指定已登记的 zone。</p>}
      </>}
    </div>
    <div className="scheduler-setting-help">
      {!props.readOnly && typeof value.zone === 'string' && <p id={`${id}-zone-help`}>固定 zone 填写 ID（不是目录路径），启动 Run 前需完成登记。</p>}
      {!value.end_node && !props.script && <p id={`${id}-close-help`}>自动关闭会在节点完成后关闭 Tab，终止其中的终端及相关进程。</p>}
    </div>
  </section>
}
