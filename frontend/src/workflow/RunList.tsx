import { useState } from 'react'
import type { RunEntry } from './useWorkflowRefresh'
import { WorkflowStatus } from './WorkflowStatus'

function creationTime(entry: RunEntry): number | null {
  const value = entry.run?.createdAt
  return typeof value === 'number' && Number.isFinite(value) && !Number.isNaN(new Date(value).getTime()) ? value : null
}

export function RunList({ entries, error = null, onOpen }: { entries: RunEntry[] | null; error?: string | null; onOpen?: () => void }) {
  const [search, setSearch] = useState('')
  const [map, setMap] = useState('')
  const [status, setStatus] = useState('')
  const [managed, setManaged] = useState('')
  const query = search.trim().toLocaleLowerCase()
  const visible = (entries ?? []).filter(entry => {
    return (!query || `${entry.id}\n${entry.run?.name ?? ''}\n${entry.run?.map ?? ''}`.toLocaleLowerCase().includes(query))
      && (!map || entry.run?.map === map)
      && (!status || (entry.run?.status ?? 'unreadable') === status)
      && (!managed || String(entry.managed) === managed)
  }).sort((a, b) => {
    const left = creationTime(a)
    const right = creationTime(b)
    return left === null ? right === null ? 0 : 1 : right === null ? -1 : right - left
  })
  const today = new Date().toLocaleDateString()
  const previousDay = new Date()
  previousDay.setDate(previousDay.getDate() - 1)
  const yesterday = previousDay.toLocaleDateString()
  const groups = new Map<string, RunEntry[]>()
  for (const entry of visible) {
    const timestamp = creationTime(entry)
    const date = timestamp === null ? '时间未知' : new Date(timestamp).toLocaleDateString()
    const group = groups.get(date) ?? []
    group.push(entry)
    groups.set(date, group)
  }
  const maps = [...new Set([...(map ? [map] : []), ...(entries ?? []).flatMap(entry => entry.run ? [entry.run.map] : [])])].sort()
  const statuses = [...new Set(['running', 'paused', 'completed', ...(status ? [status] : []), ...(entries ?? []).map(entry => entry.run?.status ?? 'unreadable')])].sort()
  return <section aria-label="Run 列表" className="workflow-run-list">
    <header className="workflow-list-heading"><div><span className="workflow-eyebrow">WORKFLOW / RUNS</span><h1>运行记录</h1><p className="workflow-muted">从一次运行出发，查看工作流现场与交接材料。</p></div><span className="workflow-count">{entries?.length ?? 0} 次运行</span></header>
    <div className="workflow-filters">
      <label>搜索 Run / Map<input type="search" value={search} onChange={event => setSearch(event.target.value)} placeholder="Run 名称、ID 或 Map 标识" /></label>
      <label>状态<select value={status} onChange={event => setStatus(event.target.value)}><option value="">全部状态</option>{statuses.map(value => <option key={value} value={value}>{({ pending: '待执行', ready: '可派发', blocked: '等待依赖', running: '运行中', paused: '已暂停', completed: '已完成', failed: '失败', unreadable: '记录读取失败' } as Record<string, string>)[value] ?? value}</option>)}</select></label>
      <label>Map<select value={map} onChange={event => setMap(event.target.value)}><option value="">全部 Map</option>{maps.map(value => <option key={value}>{value}</option>)}</select></label>
      <label>管理归属<select value={managed} onChange={event => setManaged(event.target.value)}><option value="">全部归属</option><option value="true">当前后端管理</option><option value="false">历史现场 / 未接管</option></select></label>
    </div>
    <p className="workflow-muted" role="status" aria-label="Run 显示计数">显示 {visible.length} / {entries?.length ?? 0} · 创建时间倒序 · 未命名时使用 Map 文件标识</p>
    {!entries && <p className="workflow-empty">{error ? 'Run 列表读取失败，请查看更新错误。' : '正在加载 Run 列表…'}</p>}
    {entries?.length === 0 && <p className="workflow-empty">尚无 Run。从 scheduler 选择 Map 并显式发起。</p>}
    {entries && entries.length > 0 && visible.length === 0 && <p className="workflow-empty">没有符合筛选条件的 Run。</p>}
    {[...groups].map(([date, runs]) => <section className="workflow-date-group" aria-label={date} key={date}>
      <h3>{date === today ? '今天' : date === yesterday ? '昨天' : date === '时间未知' ? '时间未知' : '历史运行'}{date !== '时间未知' && <small>{date}</small>}</h3><div className="workflow-run-table-scroll"><table className="workflow-run-table" aria-label={`全部 Run · ${date}`}>
        <colgroup><col className="workflow-time-column" /><col className="workflow-identity-column" /><col className="workflow-status-column" /><col className="workflow-count-column" /></colgroup>
        <tbody>{runs.map(({ id, run, managed: owned, readError }) => {
          const timestamp = creationTime({ id, run, managed: owned, readError })
          const completed = Object.values(run?.nodes ?? {}).filter(node => node.status === 'completed').length
          return <tr key={id}>
            <td className="workflow-run-time" aria-label="创建时间">{timestamp === null ? '时间未知' : <time dateTime={new Date(timestamp).toISOString()}>{new Date(timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</time>}</td>
            <th scope="row" className="workflow-run-identity"><a href={`#workflow/${id}`} onClick={onOpen}><strong>{run?.name || run?.map || 'Map 标识不可读'}</strong><code>{id}</code></a>{readError && <p role="alert">{readError}</p>}</th>
            <td aria-label="Run 状态">{run ? <WorkflowStatus status={run.status} saved={!owned} /> : <span className="workflow-status workflow-status-unreadable">记录读取失败</span>}</td>
            <td className="workflow-run-count" aria-label="交接数量">{run ? `${completed}/${Object.keys(run.nodes).length}` : '—'}</td>
          </tr>
        })}</tbody>
      </table></div>
    </section>)}
  </section>
}
