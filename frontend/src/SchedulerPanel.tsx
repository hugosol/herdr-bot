import { useEffect, useRef, useState, type RefObject } from 'react'
import { listMaps, loadMap, listBots, loadBotDraft, saveBot, checkBotDelete, deleteBot, loadNodeDraft, saveNode, checkNodeDeletion, deleteNode, type BotRevision, type BotCatalog, type WorkflowMap } from './schedulerApi'
import { MapGraph } from './MapGraph'
import { MapNodeDetails } from './MapNodeDetails'
import { BotPreview } from './BotPreview'
import { RunLaunchForm } from './RunLaunchForm'
import './scheduler.css'

type Listing = { kind: 'loading' } | { kind: 'ready'; files: string[] } | { kind: 'error'; message: string }
type Preview = { kind: 'loading' } | { kind: 'ready'; map: WorkflowMap } | { kind: 'error'; message: string }
type Bots = { kind: 'loading' } | ({ kind: 'ready' } & BotCatalog) | { kind: 'error'; message: string }

type Draft = { kind: 'Bot' | 'node'; create: boolean; id: string; content: string; revision: BotRevision | null; initialId: string; initialContent: string }

export function SchedulerPanel({ active, navigationGuard, onTask, onZones }: { active: boolean; navigationGuard?: RefObject<(() => boolean) | null>; onTask: (id?: string) => void; onZones: () => void }) {
  const [launchOpen, setLaunchOpen] = useState(false)
  const [draft, setDraft] = useState<Draft | null>(null)
  const [editingBusy, setEditingBusy] = useState(false)
  const [editError, setEditError] = useState<string | null>(null)
  const [returnProblem, setReturnProblem] = useState<string | null>(null)
  const dirty = draft !== null && (draft.id !== draft.initialId || draft.content !== draft.initialContent)

  function abandonDraft() {
    if (editingBusy || (dirty && !window.confirm(`放弃尚未保存的 ${draft?.kind ?? '定义'} 修改？`))) return false
    setDraft(null)
    setEditError(null)
    return true
  }

  useEffect(() => {
    if (navigationGuard) navigationGuard.current = abandonDraft
    return () => { if (navigationGuard) navigationGuard.current = null }
  })

  useEffect(() => {
    if (!dirty) return
    const beforeUnload = (event: BeforeUnloadEvent) => { event.preventDefault(); event.returnValue = '' }
    window.addEventListener('beforeunload', beforeUnload)
    return () => window.removeEventListener('beforeunload', beforeUnload)
  }, [dirty])

  async function removeSelectedNode() {
    if (editingBusy || draft || file === null || selectedId === null) return
    setEditingBusy(true)
    setEditError(null)
    try {
      const target = await checkNodeDeletion(file, selectedId)
      if (!window.confirm(`确认删除 node ${target.id}？仅删除当前 Map 中此 node，不可撤销。`)) return
      const current = await deleteNode(file, target)
      setPreview({ kind: 'ready', map: current })
      setSelectedId(null)
    } catch (error) { setEditError(error instanceof Error ? error.message : 'node 删除失败。') }
    finally { setEditingBusy(false) }
  }

  async function beginEdit(id: string | null, kind: Draft['kind'] = 'Bot') {
    if (!abandonDraft() || (kind === 'node' && file === null)) return
    setEditingBusy(true)
    try {
      const loaded = kind === 'Bot' ? await loadBotDraft(id) : await loadNodeDraft(file!, id)
      setDraft({ ...loaded, kind, create: id === null, initialId: loaded.id, initialContent: loaded.content })
      setBotView(kind === 'Bot')
    } catch (error) { setEditError(error instanceof Error ? error.message : `${kind} 读取失败。`) }
    finally { setEditingBusy(false) }
  }

  async function removeBot() {
    if (botId === null || editingBusy) return
    setEditError(null)
    setEditingBusy(true)
    try {
      const target = await checkBotDelete(botId)
      if (!window.confirm(`确认删除共享 Bot ${target.id}？此操作无法撤销。`)) return
      const catalog = await deleteBot(target)
      setBots({ kind: 'ready', ...catalog })
      setBotId(null)
    } catch (error) { setEditError(error instanceof Error ? error.message : 'Bot 删除失败。') }
    finally { setEditingBusy(false) }
  }

  async function submitDefinition() {
    if (!draft || editingBusy) return
    setEditError(null)
    setEditingBusy(true)
    try {
      const value: unknown = JSON.parse(draft.content)
      if (typeof value === 'object' && value !== null && Object.hasOwn(value, 'id')) throw new Error(`请移除 ${draft.kind} 内容中的顶层 id；使用独立 ${draft.kind} ID。`)
      const payload = { id: draft.id, content: draft.content, revision: draft.revision }
      if (draft.kind === 'Bot') {
        const saved = await saveBot(payload)
        setBots({ kind: 'ready', ...await listBots() })
        setBotId(saved.id)
      } else {
        await saveNode(file!, { ...payload, create: draft.create })
        setPreview({ kind: 'ready', map: await loadMap(file!) })
        setSelectedId(draft.id)
      }
      setDraft(null)
    } catch (error) { setEditError(error instanceof Error ? error.message : `${draft.kind} 保存失败。`) }
    finally { setEditingBusy(false) }
  }

  async function returnToMap() {
    if (!abandonDraft()) return
    setBotView(false)
    setReturnProblem(null)
    if (file === null) return
    setPreview({ kind: 'loading' })
    try {
      const current = await loadMap(file)
      setPreview({ kind: 'ready', map: current })
      if (selectedId !== null && !Object.hasOwn(current.nodes, selectedId)) {
        setReturnProblem(`来源 node ${selectedId} 已移除，原选择已失效。`)
        setSelectedId(null)
      }
    } catch (error) {
      setSelectedId(null)
      setPreview({ kind: 'error', message: `来源 Map 已失效：${error instanceof Error ? error.message : '读取失败'}` })
    }
    try { setBots({ kind: 'ready', ...await listBots() }) }
    catch (error) { setBots({ kind: 'error', message: error instanceof Error ? error.message : 'Bot 读取失败。' }) }
  }
  const [listing, setListing] = useState<Listing>({ kind: 'loading' })
  const [bots, setBots] = useState<Bots>({ kind: 'loading' })
  const [revision, setRevision] = useState(0)
  const [file, setFile] = useState<string | null>(null)
  const [preview, setPreview] = useState<Preview | null>(null)
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [filter, setFilter] = useState('')
  const [graphOpen, setGraphOpen] = useState(false)
  const [botView, setBotView] = useState(false)
  const [botId, setBotId] = useState<string | null>(null)
  const [botFilter, setBotFilter] = useState('')
  const botHeading = useRef<HTMLHeadingElement>(null)
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
    setBots({ kind: 'loading' })
    listBots(controller.signal).then(
      (catalog) => { if (!controller.signal.aborted) setBots({ kind: 'ready', ...catalog }) },
      (error: unknown) => {
        if (!controller.signal.aborted) setBots({ kind: 'error', message: error instanceof Error ? error.message : '共享 Bot 目录读取失败。' })
      },
    )
    return () => controller.abort()
  }, [active, file, revision])

  useEffect(() => {
    if (botView) botHeading.current?.focus()
  }, [botView, botId])

  const bot = bots.kind === 'ready' ? bots.bots.find((item) => item.id === botId) : undefined
  const botProblem = bots.kind === 'ready' ? bots.problems?.find((item) => item.bot_id === botId) : undefined
  const map = preview?.kind === 'ready' ? preview.map : null
  const busy = listing.kind === 'loading' || bots.kind === 'loading' || preview?.kind === 'loading'
  const visibleFiles = listing.kind === 'ready'
    ? listing.files.filter((name) => name.toLocaleLowerCase().includes(filter.toLocaleLowerCase())) : []
  const visibleBots = bots.kind === 'ready'
    ? bots.bots.filter((item) => `${item.name} ${item.id}`.toLocaleLowerCase().includes(botFilter.toLocaleLowerCase())) : []
  const botProblems: Record<string, string> = Object.create(null)
  if (map && bots.kind === 'ready') {
    const available = new Set(bots.bots.map((bot) => bot.id))
    for (const [id, node] of Object.entries(map.nodes)) {
      const problem = bots.problems?.find((problem) => problem.bot_id === node.bot_id)
      if (problem) botProblems[id] = `共享 Bot ${node.bot_id}：${problem.message}`
      else if (!available.has(node.bot_id)) botProblems[id] = `缺失共享 Bot：${node.bot_id}`
    }
  }

  function selectFile(name: string) {
    if (name === file || !abandonDraft()) return
    setLaunchOpen(false)
    setFile(name)
    setSelectedId(null)
    setGraphOpen(false)
    setPreview({ kind: 'loading' })
    setBots({ kind: 'loading' })
  }

  function selectRelatedNode(id: string) {
    setSelectedId(id)
    nodeButtons.current.get(id)?.focus()
  }

  function openBot(id: string | null) {
    if (!abandonDraft()) return
    setBotId(id)
    setBotView(true)
  }

  const editor = draft && <form className="scheduler-editor" aria-label={draft.kind === 'node' ? 'node 编辑' : 'Bot 编辑'} onSubmit={(event) => { event.preventDefault(); void submitDefinition() }}>
    <h3>{draft.create ? '新增' : '修改'}{draft.kind === 'node' ? ' node' : ' Bot'}</h3>
    <label>{draft.kind} ID<input value={draft.id} readOnly={!draft.create} disabled={editingBusy} onChange={(event) => setDraft({ ...draft, id: event.target.value })} /></label>
    <p>内容不含顶层 id；身份由独立 ID 决定。保存期间请勿用外部编辑器修改定义。</p>
    <p>{draft.kind === 'node' ? 'node 必须填写 title、bot_id、字符串 input、字符串 zone 路径和 blocked_by。允许暂未登记的 Bot 或 zone；同图依赖必须存在且无环。' : 'Bot 必须填写 name、when、plan_guide、node_guide；三份文档允许空文本，task_id 可暂缺；执行时必须引用普通（normal）Task，不能使用 manager。同一普通 Task 也可从 zone 独立启动。只保存 Task 引用，不复制 Task 或 Chat。'} 不支持 schema_version 或旧格式字段。</p>
    <label htmlFor="definition-content-json">{draft.kind} 内容 JSON</label><textarea id="definition-content-json" aria-label={draft.kind === 'node' ? 'node 内容 JSON' : 'Bot 内容 JSON'} spellCheck={false} value={draft.content} disabled={editingBusy} onChange={(event) => setDraft({ ...draft, content: event.target.value })} />
    <div className="scheduler-actions"><button type="submit" disabled={editingBusy}>保存{draft.kind === 'node' ? ' node' : ' Bot'}</button><button type="button" disabled={editingBusy} onClick={abandonDraft}>放弃编辑</button>{draft.kind === 'node' && <button type="button" disabled={editingBusy} onClick={abandonDraft}>返回 Map</button>}</div>
  </form>

  return <section className="scheduler" aria-label="Map 目录">
    <header className="scheduler-header">
      <div><p className="scheduler-eyebrow">静态工作定义</p><h2>scheduler <span>阅读优先</span></h2></div>
      <div className="scheduler-actions">
      <button type="button" onClick={() => openBot(null)}>Bot 管理</button>
      <button type="button" disabled={busy || editingBusy} onClick={() => {
        if (!abandonDraft()) return
        setSelectedId(null)
        setGraphOpen(false)
        if (file !== null) setPreview({ kind: 'loading' })
        setListing({ kind: 'loading' })
        setBots({ kind: 'loading' })
        setRevision((value) => value + 1)
      }}>刷新全部</button>
      </div>
    </header>
    <p className="scheduler-intro">浏览服务主机上的 Map，与当前 zone 无关。可新增和修改当前 Map 的 node 与共享 Bot；保存静态定义不执行工作，需显式发起 Run。</p>
    <p className="scheduler-refresh-note">刷新会重新读取目录、当前 Map 和共享 Bot，并清除节点选择。</p>
    {busy && <p role="status">正在读取工作定义……</p>}
    <div className="scheduler-notices">
      {returnProblem && <p role="alert">{returnProblem}</p>}
      {bots.kind === 'loading' && <p>正在读取共享 Bot 目录……</p>}
      {bots.kind === 'error' && <p role="alert" data-testid="scheduler-bot-error">{bots.message}</p>}
      {!botView && bots.kind === 'ready' && bots.bots.length === 0 && !bots.problems?.length && <p data-testid="scheduler-bot-empty">共享 Bot 目录没有 Bot JSON 文件。</p>}
      {(Object.keys(botProblems).length > 0 || (bots.kind === 'ready' && !!bots.problems?.length)) && <section aria-label="Bot 问题" className="scheduler-problems">
        <h3>Bot 问题</h3><ul>
          {bots.kind === 'ready' && bots.problems?.map((problem, index) => <li key={`file-${index}`}>{problem.message}</li>)}
          {Object.entries(botProblems).map(([id, message]) => <li key={id}>node {id}：{message}</li>)}
        </ul>
      </section>}
    </div>
    {botView ? <div className="scheduler-bot-page">
      <nav aria-label="Bot 导航" className="scheduler-actions">
        <button type="button" onClick={() => void returnToMap()}>返回 Map</button>
        {!draft && <button type="button" disabled={editingBusy} onClick={() => void beginEdit(null)}>新增 Bot</button>}
      </nav>
      <h2 ref={botHeading} tabIndex={-1}>共享 Bot</h2>
      <div className="scheduler-bot-layout">
        <section aria-label="共享 Bot 列表" className="scheduler-bot-directory">
          <label className="scheduler-filter">筛选 Bot<input type="search" value={botFilter} onChange={(event) => setBotFilter(event.target.value)} placeholder="名称或 ID…" /></label>
          {bots.kind === 'ready' && <>
            <p className="scheduler-filter-count" role="status">显示 {visibleBots.length} / {bots.bots.length}</p>
            {bots.bots.length === 0 && !bots.problems?.length && <p data-testid="scheduler-bot-empty">共享 Bot 目录没有 Bot JSON 文件。</p>}
            {bots.bots.length > 0 && visibleBots.length === 0 && <p>没有匹配的 Bot。清除筛选可查看全部。</p>}
            <ul className="scheduler-map-list">{visibleBots.map((item) => <li key={item.id}>
              <button type="button" disabled={editingBusy} aria-pressed={botId === item.id} aria-label={`${item.name} · ${item.id}`} onClick={() => openBot(item.id)}><strong>{item.name}</strong><small>{item.id}</small></button>
            </li>)}</ul>
          </>}
        </section>
        <div className="scheduler-bot-main">
          {editError && <p role="alert">{editError}</p>}
          {draft ? editor : botId === null ? <div className="scheduler-placeholder"><h3>选择一个 Bot</h3><p>从左侧列表选择 Bot，分别阅读选用说明、规划指引和节点指引。</p></div> : <>
            {bot && !botProblem && <div className="scheduler-actions"><button type="button" disabled={editingBusy} onClick={() => void beginEdit(bot.id)}>修改 Bot</button><button type="button" disabled={editingBusy} onClick={() => void removeBot()}>删除 Bot</button></div>}
            <section aria-label="Bot 预览" className="scheduler-details">
              {bots.kind === 'ready' && (botProblem
                ? <p role="alert">共享 Bot {botId}：{botProblem.message}</p>
                : bot ? <BotPreview key={bot.id} bot={bot} /> : <p role="alert">缺失共享 Bot：{botId}</p>)}
            </section>
          </>}
        </div>
      </div>
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
        {editError && <p role="alert">{editError}</p>}
        {draft?.kind === 'node' && editor}
        {map && !draft && <div className="scheduler-actions">
          <button type="button" disabled={editingBusy} onClick={() => void beginEdit(null, 'node')}>新增 node</button>
          <button type="button" disabled={editingBusy} onClick={() => setLaunchOpen(true)}>+ 发起 Run</button>
        </div>}
        {launchOpen && file && <div hidden={botView || draft !== null}><RunLaunchForm key={file} file={file} active={active && !botView && !draft}
          onClose={() => { setLaunchOpen(false); void returnToMap() }}
          onNode={(id) => { setSelectedId(id); void beginEdit(id, 'node') }}
          onBot={(id) => { openBot(id); if (bots.kind === 'ready' && bots.bots.some(bot => bot.id === id)) void beginEdit(id, 'Bot') }}
          onTask={onTask} onZones={onZones} /></div>}
        {file === null && <div className="scheduler-placeholder"><h3>选择一个 Map</h3><p>从目录中选择文件，阅读节点定义与前置依赖。</p></div>}
        {preview?.kind === 'loading' && <p className="scheduler-placeholder">正在读取 Map……</p>}
        {preview?.kind === 'error' && <div className="scheduler-placeholder"><h3 className="scheduler-text">{file}</h3><p role="alert">{preview.message}</p><p>可选择其他 Map；修复文件后点击刷新全部。</p></div>}
        {map && <section aria-label="Map 预览">
          <header className="scheduler-map-heading"><div><p className="scheduler-filename">{file}</p><h3 className="scheduler-text">{map.title}</h3><p className="scheduler-text">{map.description}</p></div><span className="scheduler-badge">静态定义</span></header>
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
                    <span>Bot：{node.bot_id}</span>{botProblems[id] && <span className="scheduler-node-warning">{botProblems[id]}</span>}
                  </span><span aria-hidden="true">→</span>
                </button>
              </li>)}</ul>
              </section>
              <details className="scheduler-graph-disclosure" open={graphOpen} onToggle={(event) => setGraphOpen(event.currentTarget.open)}>
                <summary>依赖图</summary>
                <p>前置 node → 依赖它的 node。箭头仅表示 blocked_by，不表示执行、材料传递或运行状态。</p>
                {graphOpen && <MapGraph map={map} selectedId={selectedId} onSelect={setSelectedId} botProblems={botProblems} />}
              </details>
            </div>
            <aside className="scheduler-inspector" aria-label="节点检查器">
              {selectedId !== null && Object.hasOwn(map.nodes, selectedId) ? <>
                <div className="scheduler-inspector-heading"><span>节点详情</span><button type="button" aria-label="关闭节点详情" onClick={() => {
                  nodeButtons.current.get(selectedId)?.focus()
                  setSelectedId(null)
                }}>关闭</button></div>
                {!draft && <div className="scheduler-actions"><button type="button" disabled={editingBusy} onClick={() => void beginEdit(selectedId, 'node')}>修改 node</button><button type="button" disabled={editingBusy} onClick={() => void removeSelectedNode()}>删除 node</button></div>}
                <MapNodeDetails key={`${file}:${selectedId}`} map={map} nodeId={selectedId} onSelect={selectRelatedNode}
                  botProblem={botProblems[selectedId]} onBot={openBot} />
              </> : <div className="scheduler-placeholder"><h4>选择一个节点</h4><p>查看本次输入、zone 路径引用、前置依赖与共享 Bot。</p></div>}
            </aside>
          </div>
        </section>}
      </div>
    </div>}
  </section>
}
