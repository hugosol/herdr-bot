import type { WorkflowMap } from './schedulerApi'

export function MapNodeDetails({ map, nodeId, templateProblem, onSelect, onTemplate }: {
  map: WorkflowMap; nodeId: string; templateProblem?: string
  onSelect: (id: string) => void
  onTemplate: (id: string) => void
}) {
  const node = map.nodes[nodeId]
  const entries = Object.entries(node.parameters)
  const dependents = Object.keys(map.nodes).filter((id) => map.nodes[id].blocked_by.includes(nodeId))
  return <section aria-label="node 详情" className="scheduler-details">
    <h3>{node.title} · {nodeId}</h3>
    <p className="scheduler-text">node_template_id：{node.node_template_id}</p>
    {templateProblem && <p role="alert">{templateProblem}</p>}
    <section aria-label="本次参数">
      <h4>本次参数</h4>
      {entries.length === 0 ? <p>没有参数。</p> : <dl>
        {entries.map(([name, value]) => <div key={name}>
          <dt>{name}</dt>
          <dd><pre data-testid={`parameter-${name}`}>{typeof value === 'string' ? value : JSON.stringify(value, null, 2)}</pre></dd>
        </div>)}
      </dl>}
    </section>
    <section aria-label="前置依赖">
      <h4>前置依赖</h4>
      {node.blocked_by.length === 0 ? <p>无前置依赖。</p> : <ul>
        {node.blocked_by.map((id) => <li key={id}><button type="button" aria-label={`查看前置节点 ${map.nodes[id].title} ${id}`} onClick={() => onSelect(id)}>{map.nodes[id].title} · {id}</button></li>)}
      </ul>}
    </section>
    <section aria-label="依赖此节点">
      <h4>依赖此节点</h4>
      {dependents.length === 0 ? <p>无后继节点。</p> : <ul>{dependents.map((id) => <li key={id}>
        <button type="button" aria-label={`查看后继节点 ${map.nodes[id].title} ${id}`} onClick={() => onSelect(id)}>{map.nodes[id].title} · {id}</button>
      </li>)}</ul>}
    </section>
    <button type="button" onClick={() => onTemplate(node.node_template_id)}>模板明细</button>
  </section>
}
