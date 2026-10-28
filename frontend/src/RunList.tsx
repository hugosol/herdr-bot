import type { RunEntry } from './useWorkflowRefresh'

export function RunList({ entries }: { entries: RunEntry[] | null }) {
  return <section aria-label="Run 列表">
    <h2>全部 Run</h2>
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
