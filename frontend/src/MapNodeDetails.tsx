import type { WorkflowMap } from './schedulerApi'

export function MapNodeDetails({ map, nodeId }: { map: WorkflowMap; nodeId: string }) {
  const node = map.nodes[nodeId]
  const entries = Object.entries(node.parameters)
  return <section aria-label="node 详情" className="scheduler-details">
    <h3>{node.title} · {nodeId}</h3>
    <p className="scheduler-text">node_template_id：{node.node_template_id}</p>
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
        {node.blocked_by.map((id, index) => <li key={index}>{map.nodes[id].title} · {id}</li>)}
      </ul>}
    </section>
  </section>
}
