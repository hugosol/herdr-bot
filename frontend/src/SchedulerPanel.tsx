import { useEffect, useState } from 'react'
import { listMaps, loadMap, listTemplates, type TemplateCatalog, type WorkflowMap } from './schedulerApi'
import { MapGraph } from './MapGraph'
import { MapNodeDetails } from './MapNodeDetails'
import './scheduler.css'

type Listing = { kind: 'loading' } | { kind: 'ready'; files: string[] } | { kind: 'error'; message: string }

export function SchedulerPanel({ active }: { active: boolean }) {
  const [listing, setListing] = useState<Listing>({ kind: 'loading' })
  const [templates, setTemplates] = useState<{ kind: 'loading' } | ({ kind: 'ready' } & TemplateCatalog) | { kind: 'error'; message: string }>({ kind: 'loading' })
  const [revision, setRevision] = useState(0)
  const [file, setFile] = useState<string | null>(null)
  const [preview, setPreview] = useState<{ kind: 'loading' } | { kind: 'ready'; map: WorkflowMap } | { kind: 'error'; message: string } | null>(null)
  const [selectedId, setSelectedId] = useState<string | null>(null)
  useEffect(() => {
    if (!active || file === null) return
    const controller = new AbortController()
    setPreview({ kind: 'loading' })
    setSelectedId(null)
    loadMap(file, controller.signal).then(
      (map) => { if (!controller.signal.aborted) setPreview({ kind: 'ready', map }) },
      (error: unknown) => {
        if (!controller.signal.aborted) setPreview({ kind: 'error', message: error instanceof Error ? error.message : 'Map 读取失败。' })
      },
    )
    return () => controller.abort()
  }, [active, file, revision])
  useEffect(() => {
    if (!active) return
    const controller = new AbortController()
    setListing({ kind: 'loading' })
    listMaps(controller.signal).then(
      (files) => { if (!controller.signal.aborted) setListing({ kind: 'ready', files }) },
      (error: unknown) => {
        if (!controller.signal.aborted) setListing({ kind: 'error', message: error instanceof Error ? error.message : 'Map 目录读取失败。' })
      },
    )
    return () => controller.abort()
  }, [active, revision])
  useEffect(() => {
    if (!active) return
    const controller = new AbortController()
    setTemplates({ kind: 'loading' })
    listTemplates(controller.signal).then(
      (catalog) => { if (!controller.signal.aborted) setTemplates({ kind: 'ready', ...catalog }) },
      (error: unknown) => {
        if (!controller.signal.aborted) setTemplates({ kind: 'error', message: error instanceof Error ? error.message : '共享模板目录读取失败。' })
      },
    )
    return () => controller.abort()
  }, [active, file, revision])
  const templateProblems: Record<string, string> = Object.create(null)
  if (preview?.kind === 'ready' && templates.kind === 'ready') {
    const available = new Set(templates.templates.map((template) => template.id))
    for (const [id, node] of Object.entries(preview.map.nodes)) {
      const problem = templates.problems?.find((problem) => problem.template_id === node.node_template_id)
      if (problem) templateProblems[id] = `共享模板 ${node.node_template_id}：${problem.message}`
      else if (!available.has(node.node_template_id)) templateProblems[id] = `缺失共享模板：${node.node_template_id}`
    }
  }
  return <section className="scheduler" aria-label="Map 目录">
    <h2>scheduler</h2>
    <p>服务主机上的 Map，与当前 zone 无关。仅列出目录直接包含的 JSON 文件。</p>
    <button type="button" onClick={() => setRevision((value) => value + 1)}>刷新 Map 列表</button>
    <button type="button" disabled={file === null} onClick={() => setRevision((value) => value + 1)}>重新加载当前 Map</button>
    {listing.kind === 'loading' && <p role="status">正在读取 Map 目录……</p>}
    {listing.kind === 'error' && <p role="alert" data-testid="scheduler-error">{listing.message}</p>}
    {listing.kind === 'ready' && (listing.files.length === 0
      ? <p data-testid="scheduler-empty">此目录没有 Map JSON 文件。</p>
      : <ul data-testid="scheduler-map-list">{listing.files.map((name) => <li key={name}>
        <button type="button" aria-pressed={file === name} onClick={() => setFile(name)}>{name}</button>
      </li>)}</ul>)}
    {templates.kind === 'loading' && <p role="status">正在读取共享模板目录……</p>}
    {templates.kind === 'error' && <p role="alert" data-testid="scheduler-template-error">{templates.message}</p>}
    {templates.kind === 'ready' && templates.templates.length === 0 && !templates.problems?.length && <p data-testid="scheduler-template-empty">共享模板目录没有模板 JSON 文件。</p>}
    {(Object.keys(templateProblems).length > 0 || (templates.kind === 'ready' && !!templates.problems?.length)) && <section aria-label="模板问题">
      <h4>模板问题</h4><ul>
        {templates.kind === 'ready' && templates.problems?.map((problem, index) => <li key={`file-${index}`}>{problem.message}</li>)}
        {Object.entries(templateProblems).map(([id, message]) => <li key={id}>node {id}：{message}</li>)}
      </ul>
    </section>}
    {preview?.kind === 'loading' && <p role="status">正在读取 Map……</p>}
    {preview?.kind === 'error' && <p role="alert">{preview.message}</p>}
    {preview?.kind === 'ready' && <section aria-label="Map 预览">
      <h3>{preview.map.title}</h3>
      <p className="scheduler-text">{preview.map.description}</p>
      <p>箭头仅表示 blocked_by 前置依赖，不表示执行、材料传递或运行状态。</p>
      <MapGraph map={preview.map} selectedId={selectedId} onSelect={setSelectedId} templateProblems={templateProblems} />
      {selectedId !== null && <MapNodeDetails key={`${file}:${selectedId}`} map={preview.map} nodeId={selectedId} templateProblem={templateProblems[selectedId]}
        template={templates.kind === 'ready' ? templates.templates.find((template) => template.id === preview.map.nodes[selectedId].node_template_id) : undefined}
        templateStatus={templates.kind === 'loading' ? '正在读取共享模板目录……' : templates.kind === 'error' ? templates.message : null} />}
    </section>}
  </section>
}
