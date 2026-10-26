import { useId, useState } from 'react'
import type { NodeTemplate, WorkflowMap } from './schedulerApi'

function NamedDescriptions({ title, values }: { title: string; values: Record<string, string> }) {
  const entries = Object.entries(values)
  return <section aria-label={title}>
    <h5>{title}</h5>
    {entries.length === 0 ? <p>无。</p> : <dl>{entries.map(([name, description]) => <div key={name}>
      <dt>{name}</dt><dd className="scheduler-text">{description}</dd>
    </div>)}</dl>}
  </section>
}

export function MapNodeDetails({ map, nodeId, template, templateStatus, templateProblem }: {
  map: WorkflowMap; nodeId: string; template: NodeTemplate | undefined; templateStatus: string | null; templateProblem?: string
}) {
  const [expanded, setExpanded] = useState(false)
  const templateDetailsId = useId()
  const node = map.nodes[nodeId]
  const entries = Object.entries(node.parameters)
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
        {node.blocked_by.map((id, index) => <li key={index}>{map.nodes[id].title} · {id}</li>)}
      </ul>}
    </section>
    <button type="button" aria-expanded={expanded} aria-controls={templateDetailsId} onClick={() => setExpanded((value) => !value)}>模板明细</button>
    {expanded && <section id={templateDetailsId} aria-label="模板明细">
      {templateProblem ? <p>{templateProblem}</p> : templateStatus !== null ? <p>{templateStatus}</p> : template === undefined
        ? <p>未找到共享模板：{node.node_template_id}</p>
        : <>
          <h4 className="scheduler-text">{template.title}</h4>
          <dl>
            <dt>适用场景（use_when）</dt><dd className="scheduler-text">{template.use_when}</dd>
            <dt>能力说明（description）</dt><dd className="scheduler-text">{template.description}</dd>
            <dt>实现类型</dt><dd>{template.implementation.type}</dd>
          </dl>
          <NamedDescriptions title="输入说明（inputs）" values={template.inputs} />
          <NamedDescriptions title="输出说明（outputs）" values={template.outputs} />
          <p>仅展示能力定义，不启动 Chat 或执行任何工作。</p>
        </>}
    </section>}
  </section>
}
