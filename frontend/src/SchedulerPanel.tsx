import { useEffect, useRef, useState } from 'react'
import { listMaps, loadMap, listTemplates, type TemplateCatalog, type WorkflowMap } from './schedulerApi'
import { MapGraph } from './MapGraph'
import { MapNodeDetails } from './MapNodeDetails'
import { TemplatePreview } from './TemplatePreview'
import './scheduler.css'

type Listing = { kind: 'loading' } | { kind: 'ready'; files: string[] } | { kind: 'error'; message: string }
type Preview = { kind: 'loading' } | { kind: 'ready'; map: WorkflowMap } | { kind: 'error'; message: string }
type Templates = { kind: 'loading' } | ({ kind: 'ready' } & TemplateCatalog) | { kind: 'error'; message: string }

export function SchedulerPanel({ active }: { active: boolean }) {
  const [listing, setListing] = useState<Listing>({ kind: 'loading' })
  const [templates, setTemplates] = useState<Templates>({ kind: 'loading' })
  const [revision, setRevision] = useState(0)
  const [file, setFile] = useState<string | null>(null)
  const [preview, setPreview] = useState<Preview | null>(null)
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [filter, setFilter] = useState('')
  const [graphOpen, setGraphOpen] = useState(false)
  const [templateView, setTemplateView] = useState(false)
  const [templateId, setTemplateId] = useState<string | null>(null)
  const templateHeading = useRef<HTMLHeadingElement>(null)
  const nodeButtons = useRef(new Map<string, HTMLButtonElement>())

  useEffect(() => {
    if (!active || file === null) return
    const controller = new AbortController()
    setPreview({ kind: 'loading' })
    setSelectedId(null)
    setGraphOpen(false)
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

  useEffect(() => {
    if (templateView) templateHeading.current?.focus()
  }, [templateView, templateId])

  const template = templates.kind === 'ready' ? templates.templates.find((item) => item.id === templateId) : undefined
  const templateProblem = templates.kind === 'ready' ? templates.problems?.find((item) => item.template_id === templateId) : undefined
  const map = preview?.kind === 'ready' ? preview.map : null
  const busy = listing.kind === 'loading' || templates.kind === 'loading' || preview?.kind === 'loading'
  const visibleFiles = listing.kind === 'ready'
    ? listing.files.filter((name) => name.toLocaleLowerCase().includes(filter.toLocaleLowerCase())) : []
  const templateProblems: Record<string, string> = Object.create(null)
  if (map && templates.kind === 'ready') {
    const available = new Set(templates.templates.map((template) => template.id))
    for (const [id, node] of Object.entries(map.nodes)) {
      const problem = templates.problems?.find((problem) => problem.template_id === node.node_template_id)
      if (problem) templateProblems[id] = `共享模板 ${node.node_template_id}：${problem.message}`
      else if (!available.has(node.node_template_id)) templateProblems[id] = `缺失共享模板：${node.node_template_id}`
    }
  }

  function selectFile(name: string) {
    if (name === file) return
    setFile(name)
    setSelectedId(null)
    setGraphOpen(false)
    setPreview({ kind: 'loading' })
    setTemplates({ kind: 'loading' })
  }

  function selectRelatedNode(id: string) {
    setSelectedId(id)
    nodeButtons.current.get(id)?.focus()
  }

  function openTemplate(id: string | null) {
    setTemplateId(id)
    setTemplateView(true)
  }

  return <section className="scheduler" aria-label="Map 目录">
    <header className="scheduler-header">
      <div><p className="scheduler-eyebrow">静态工作定义</p><h2>scheduler <span>阅读优先</span></h2></div>
      <div className="scheduler-actions">
      <button type="button" onClick={() => openTemplate(null)}>模板管理</button>
      <button type="button" disabled={busy} onClick={() => {
        setSelectedId(null)
        setGraphOpen(false)
        if (file !== null) setPreview({ kind: 'loading' })
        setListing({ kind: 'loading' })
        setTemplates({ kind: 'loading' })
        setRevision((value) => value + 1)
      }}>刷新全部</button>
      </div>
    </header>
    <p className="scheduler-intro">浏览服务主机上的 Map，与当前 zone 无关。只读定义，不执行工作。</p>
    <p className="scheduler-refresh-note">刷新会重新读取目录、当前 Map 和共享模板，并清除节点选择。</p>
    {busy && <p role="status">正在读取工作定义……</p>}
    <div className="scheduler-notices">
      {templates.kind === 'loading' && <p>正在读取共享模板目录……</p>}
      {templates.kind === 'error' && <p role="alert" data-testid="scheduler-template-error">{templates.message}</p>}
      {!templateView && templates.kind === 'ready' && templates.templates.length === 0 && !templates.problems?.length && <p data-testid="scheduler-template-empty">共享模板目录没有模板 JSON 文件。</p>}
      {(Object.keys(templateProblems).length > 0 || (templates.kind === 'ready' && !!templates.problems?.length)) && <section aria-label="模板问题" className="scheduler-problems">
        <h3>模板问题</h3><ul>
          {templates.kind === 'ready' && templates.problems?.map((problem, index) => <li key={`file-${index}`}>{problem.message}</li>)}
          {Object.entries(templateProblems).map(([id, message]) => <li key={id}>node {id}：{message}</li>)}
        </ul>
      </section>}
    </div>
    {templateView ? <div className="scheduler-template-page">
      <nav aria-label="模板导航" className="scheduler-actions">
        <button type="button" onClick={() => setTemplateView(false)}>返回 Map</button>
        {templateId !== null && <button type="button" onClick={() => setTemplateId(null)}>返回模板列表</button>}
      </nav>
      <h2 ref={templateHeading} tabIndex={-1}>{templateId === null ? '共享模板' : '模板预览'}</h2>
      {templateId === null ? <section aria-label="共享模板列表">
        {templates.kind === 'ready' && templates.templates.length === 0 && !templates.problems?.length && <p data-testid="scheduler-template-empty">共享模板目录没有模板 JSON 文件。</p>}
        {templates.kind === 'ready' && <ul className="scheduler-map-list">{templates.templates.map((item) => <li key={item.id}>
          <button type="button" onClick={() => setTemplateId(item.id)}>{item.title} · {item.id}</button>
        </li>)}</ul>}
      </section> : <section aria-label="模板预览" className="scheduler-details">
        {templates.kind === 'ready' && (templateProblem
          ? <p role="alert">共享模板 {templateId}：{templateProblem.message}</p>
          : template ? <TemplatePreview template={template} /> : <p role="alert">缺失共享模板：{templateId}</p>)}
      </section>}
    </div> : <div className="scheduler-layout">
      <aside className="scheduler-directory" aria-label="Map 文件目录">
        <div className="scheduler-section-heading"><h3>Map 目录</h3>{listing.kind === 'ready' && <span>{listing.files.length} 个文件</span>}</div>
        <label className="scheduler-filter">筛选 Map 文件<input type="search" value={filter} onChange={(event) => setFilter(event.target.value)} placeholder="输入文件名…" /></label>
        {listing.kind === 'loading' && <p>正在读取 Map 目录……</p>}
        {listing.kind === 'error' && <p role="alert" data-testid="scheduler-error">{listing.message}</p>}
        {listing.kind === 'ready' && (listing.files.length === 0
          ? <p data-testid="scheduler-empty">此目录没有 Map JSON 文件。</p>
          : <>
            <p className="scheduler-filter-count" role="status">显示 {visibleFiles.length} / {listing.files.length}</p>
            {visibleFiles.length === 0 && <p>没有匹配的 Map 文件。清除筛选可查看全部。</p>}
            <ul data-testid="scheduler-map-list" className="scheduler-map-list">{visibleFiles.map((name) => <li key={name}>
              <button type="button" aria-pressed={file === name} onClick={() => selectFile(name)}>{name}</button>
            </li>)}</ul>
          </>)}
        <p className="scheduler-directory-note">仅列出目录直接包含的 JSON 文件。筛选不会重载或关闭当前 Map。</p>
      </aside>
      <div className="scheduler-main">

        {file === null && <div className="scheduler-placeholder"><h3>选择一个 Map</h3><p>从目录中选择文件，阅读节点定义与前置依赖。</p></div>}
        {preview?.kind === 'loading' && <p className="scheduler-placeholder">正在读取 Map……</p>}
        {preview?.kind === 'error' && <div className="scheduler-placeholder"><h3 className="scheduler-text">{file}</h3><p role="alert">{preview.message}</p><p>可选择其他 Map；修复文件后点击刷新全部。</p></div>}
        {map && <section aria-label="Map 预览">
          <header className="scheduler-map-heading"><div><p className="scheduler-filename">{file}</p><h3 className="scheduler-text">{map.title}</h3><p className="scheduler-text">{map.description}</p></div><span className="scheduler-badge">只读定义</span></header>
          <div className="scheduler-reading">
            <div className="scheduler-outline">
              <section aria-label="节点列表">
              <div className="scheduler-section-heading"><h4>节点与前置依赖</h4><span>{Object.keys(map.nodes).length} 个节点</span></div>
              {Object.keys(map.nodes).length === 0 && <p>此 Map 没有 node。</p>}
              <ul className="scheduler-node-list">{Object.entries(map.nodes).map(([id, node]) => <li key={id}>
                <button type="button" ref={(element) => { if (element) nodeButtons.current.set(id, element); else nodeButtons.current.delete(id) }}
                  aria-label={`${node.title} ${id}`} aria-pressed={selectedId === id} onClick={() => setSelectedId(id)}>
                  <span className="scheduler-node-id">{id}</span><span className="scheduler-node-summary"><strong>{node.title}</strong>
                    <span>{node.blocked_by.length === 0 ? '无前置依赖' : `前置：${node.blocked_by.map((parent) => `${map.nodes[parent].title} · ${parent}`).join('、')}`}</span>
                    <span>模板：{node.node_template_id}</span>{templateProblems[id] && <span className="scheduler-node-warning">{templateProblems[id]}</span>}
                  </span><span aria-hidden="true">→</span>
                </button>
              </li>)}</ul>
              </section>
              <details className="scheduler-graph-disclosure" open={graphOpen} onToggle={(event) => setGraphOpen(event.currentTarget.open)}>
                <summary>依赖图</summary>
                <p>前置 node → 依赖它的 node。箭头仅表示 blocked_by，不表示执行、材料传递或运行状态。</p>
                {graphOpen && <MapGraph map={map} selectedId={selectedId} onSelect={setSelectedId} templateProblems={templateProblems} />}
              </details>
            </div>
            <aside className="scheduler-inspector" aria-label="节点检查器">
              {selectedId !== null && Object.hasOwn(map.nodes, selectedId) ? <>
                <div className="scheduler-inspector-heading"><span>节点详情</span><button type="button" aria-label="关闭节点详情" onClick={() => {
                  nodeButtons.current.get(selectedId)?.focus()
                  setSelectedId(null)
                }}>关闭</button></div>
                <MapNodeDetails key={`${file}:${selectedId}`} map={map} nodeId={selectedId} onSelect={selectRelatedNode}
                  templateProblem={templateProblems[selectedId]} onTemplate={openTemplate} />
              </> : <div className="scheduler-placeholder"><h4>选择一个节点</h4><p>查看本次参数、前置依赖与共享模板。</p></div>}
            </aside>
          </div>
        </section>}
      </div>
    </div>}
  </section>
}
