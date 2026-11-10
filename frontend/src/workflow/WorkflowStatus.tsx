const statuses: Record<string, { label: string; icon: string }> = {
  pending: { label: '待执行', icon: '…' },
  ready: { label: '可派发', icon: '○' },
  blocked: { label: '等待依赖', icon: '⊘' },
  running: { label: '运行中', icon: '' },
  paused: { label: '已暂停', icon: 'Ⅱ' },
  completed: { label: '已完成', icon: '' },
  failed: { label: '失败', icon: '!' },
}

export function WorkflowStatus({ status, saved = false, animate = false }: { status: string; saved?: boolean; animate?: boolean }) {
  const presentation = statuses[status]
  return <span className={`workflow-status workflow-status-${status}`} data-status={status} title={saved ? '已保存的状态，不表示当前后端正在管理。' : undefined}>
    <span aria-hidden="true" className={status === 'running' ? 'workflow-node-running-arrow' : undefined} data-animate={status === 'running' && animate && !saved ? 'true' : undefined}>{status === 'running' || status === 'completed' ? <svg viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round" style={{ verticalAlign: '-2px' }}><path d={status === 'running' ? 'M2 8h11M8 3l5 5-5 5' : 'm3 8 3 3 7-7'} /></svg> : presentation?.icon ?? '?'}</span> {presentation?.label ?? status}
  </span>
}
