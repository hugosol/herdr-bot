import type { WorkflowMap } from './schedulerApi'

export function MapNodeDetails({ map, nodeId, botProblem, onSelect, onBot }: {
  map: WorkflowMap; nodeId: string; botProblem?: string
  onSelect: (id: string) => void
  onBot: (id: string) => void
}) {
  const node = map.nodes[nodeId]
  const entries = Object.entries(node.parameters)
  const dependents = Object.keys(map.nodes).filter((id) => map.nodes[id].blocked_by.includes(nodeId))
  return <section aria-label="node 详情" className="scheduler-details">
    <h3>{node.title} · {nodeId}</h3>
    <p className="scheduler-text">bot_id：{node.bot_id}</p>
    {botProblem && <p role="alert">{botProblem}</p>}
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
    <button type="button" onClick={() => onBot(node.bot_id)}>Bot 明细</button>
  </section>
}
