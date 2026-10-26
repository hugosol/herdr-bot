import type { NodeTemplate } from './schedulerApi'

function NamedDescriptions({ title, values }: { title: string; values: Record<string, string> }) {
  const entries = Object.entries(values)
  return <section aria-label={title}>
    <h4>{title}</h4>
    {entries.length === 0 ? <p>无。</p> : <dl>{entries.map(([name, description]) => <div key={name}>
      <dt>{name}</dt><dd className="scheduler-text">{description}</dd>
    </div>)}</dl>}
  </section>
}

export function TemplatePreview({ template }: { template: NodeTemplate }) {
  return <>
    <h3 className="scheduler-text">{template.title}</h3>
    <dl>
      <dt>模板 ID</dt><dd className="scheduler-text">{template.id}</dd>
      <dt>适用场景（use_when）</dt><dd className="scheduler-text">{template.use_when}</dd>
      <dt>能力说明（description）</dt><dd className="scheduler-text">{template.description}</dd>
      <dt>实现类型</dt><dd>{template.implementation.type}</dd>
    </dl>
    <NamedDescriptions title="输入说明（inputs）" values={template.inputs} />
    <NamedDescriptions title="输出说明（outputs）" values={template.outputs} />
    <p>仅展示能力定义，不启动 Chat 或执行任何工作。</p>
  </>
}
