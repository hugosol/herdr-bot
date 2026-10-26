import { useState } from 'react'
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
  const [section, setSection] = useState('概览')
  return <>
    <p className="scheduler-filename">{template.id}</p>
    <h3 className="scheduler-text">{template.title}</h3>
    <nav className="scheduler-template-sections" aria-label="模板内容分区">
      {['概览', '输入与输出', '原始定义'].map((name) => <button type="button" key={name} aria-pressed={section === name} onClick={() => setSection(name)}>{name}</button>)}
    </nav>
    <div hidden={section !== '概览'}><dl>
      <dt>适用场景（use_when）</dt><dd className="scheduler-text">{template.use_when}</dd>
      <dt>能力说明（description）</dt><dd className="scheduler-text">{template.description}</dd>
      <dt>实现类型</dt><dd>{template.implementation.type}</dd>
    </dl></div>
    <div hidden={section !== '输入与输出'}>
      <div className="scheduler-template-io">
        <NamedDescriptions title="输入说明（inputs）" values={template.inputs} />
        <NamedDescriptions title="输出说明（outputs）" values={template.outputs} />
      </div>
    </div>
    <div hidden={section !== '原始定义'}><pre aria-label="模板原始定义">{JSON.stringify(template, null, 2)}</pre></div>
    <p>仅展示能力定义，不启动 Chat 或执行任何工作。</p>
  </>
}
