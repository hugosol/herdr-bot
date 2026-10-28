import type { WorkflowMap } from './schedulerApi'

export function MapNodeDetails({ map, nodeId, botProblem, onSelect, onBot }: {
  map: WorkflowMap; nodeId: string; botProblem?: string
  onSelect: (id: string) => void
  onBot: (id: string) => void
}) {
  const node = map.nodes[nodeId]
  const dependents = Object.keys(map.nodes).filter((id) => map.nodes[id].blocked_by.includes(nodeId))
  return <section aria-label="node 详情" className="scheduler-details">
    <h3>{node.title} · {nodeId}</h3>
    <p className="scheduler-text">bot_id：{node.bot_id}</p>
    {botProblem && <p role="alert">{botProblem}</p>}
    <p className="scheduler-text">zone 路径引用：{node.zone || '尚未绑定 zone'}</p>
    <section aria-label="本次输入">
      <h4>本次输入</h4>
      <pre className="scheduler-text" data-testid="node-input">{node.input}</pre>
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
