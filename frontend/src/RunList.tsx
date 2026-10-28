import { useEffect, useState } from 'react'

type Entry = { id: string; managed: boolean; readError: string | null; run: { map: string; createdAt: number; status: string; nodes: Record<string, { status: string }> } | null }

export function RunList({ active, revision }: { active: boolean; revision: number }) {
  const [entries, setEntries] = useState<Entry[] | null>(null)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    if (!active) return
    const controller = new AbortController()
    setError(null)
    void fetch('/api/workflow/runs', { signal: controller.signal }).then(async response => {
      const value = await response.json()
      if (!response.ok) throw new Error(value.message ?? `Run 列表查询失败：HTTP ${response.status}`)
      if (!controller.signal.aborted) setEntries(value.runs)
    }).catch(failure => { if (!controller.signal.aborted) setError(failure instanceof Error ? failure.message : 'Run 列表读取失败。') })
    return () => controller.abort()
  }, [active, revision])
  return <section aria-label="Run 列表">
    <h2>全部 Run</h2>
    {error && <p role="alert">{error} 不会自动重试。</p>}
    {entries?.length === 0 && <p>尚无 Run。从 scheduler 选择 Map 并显式发起。</p>}
    {entries && entries.length > 0 && <div className="workflow-table-scroll"><table aria-label="全部 Run">
      <thead><tr><th>Run</th><th>Map</th><th>创建时间</th><th>状态</th><th>完成数量</th><th>管理归属</th></tr></thead>
      <tbody>{entries.map(({ id, run, managed, readError }) => <tr key={id}>
        <th scope="row"><a href={`#workflow/${id}`}>{id}</a></th>
        <td>{run?.map ?? '定义标识不可读'}</td>
        <td>{run && <time dateTime={new Date(run.createdAt).toISOString()}>{new Date(run.createdAt).toLocaleString()}</time>}</td>
        <td>{run?.status ?? '记录读取失败'}{readError && <p role="alert">{readError}</p>}</td>
        <td>{run && `${Object.values(run.nodes).filter(node => node.status === 'completed').length} / ${Object.keys(run.nodes).length}`}</td>
        <td>{managed ? '当前后端管理' : '历史现场 / 未接管'}</td>
      </tr>)}</tbody>
    </table></div>}
  </section>
}
