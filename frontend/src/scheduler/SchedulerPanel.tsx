import { useEffect, useRef, useState, type RefObject } from 'react'
import { useSchedulerEditor } from './useSchedulerEditor'
import { MapGraph } from './MapGraph'
import { orderedMapNodeIds } from './mapNodeOrder'
import { MapNodeDetails } from './MapNodeDetails'
import { BotPreview } from './BotPreview'
import { BotTaskSelect } from './BotTaskSelect'
import { NodeSettings } from './NodeSettings'
import { RunLaunchForm } from './RunLaunchForm'
import './scheduler.css'

export function SchedulerPanel({ active, view, onViewChange, navigationGuard, onTask, onZones }: { active: boolean; view: 'bot' | 'map'; onViewChange: (view: 'bot' | 'map') => void; navigationGuard?: RefObject<(() => boolean) | null>; onTask: (id?: string) => void; onZones: () => void }) {
  const botView = view === 'bot'
  const editorState = useSchedulerEditor({ active, view, onViewChange })
  const { listing, bots, file, preview, selectedId, botId, draft, editingBusy, editError, returnProblem, deleteMapTarget, deletingMap, deleteMapError,
    abandonDraft, selectNode, openBot, beginEdit, editDraft, submitDefinition, removeSelectedNode, removeBot, requestMapDeletion, closeMapDeletion, removeMap, refreshAll, returnToMap } = editorState
  const [launchOpen, setLaunchOpen] = useState(false)
  const [filter, setFilter] = useState('')
  const [botFilter, setBotFilter] = useState('')
  const deleteMapDialog = useRef<HTMLDialogElement>(null)
  const cancelMapDeletion = useRef<HTMLButtonElement>(null)
  const directoryHeading = useRef<HTMLHeadingElement>(null)
  const schedulerRoot = useRef<HTMLElement>(null)
  const botHeading = useRef<HTMLHeadingElement>(null)
  const nodeButtons = useRef(new Map<string, HTMLButtonElement>())
  const previousDeleteTarget = useRef<string | null>(null)
  useEffect(() => {
    if (navigationGuard) navigationGuard.current = abandonDraft
    return () => { if (navigationGuard) navigationGuard.current = null }
  })
  useEffect(() => {
    const element = deleteMapDialog.current
    if (deleteMapTarget !== null && element !== null) {
      element.showModal()
      cancelMapDeletion.current?.focus()
    } else if (previousDeleteTarget.current !== null && file === null) {
      setLaunchOpen(false)
      directoryHeading.current?.focus()
    }
    previousDeleteTarget.current = deleteMapTarget
    return () => element?.close()
  }, [deleteMapTarget, file])

  useEffect(() => {
    // Internal Bot links move into their content; outer navigation keeps its own focus.
    if (botView && schedulerRoot.current?.contains(document.activeElement)) botHeading.current?.focus()
  }, [botView, botId])

  const bot = bots.kind === 'ready' ? bots.bots.find((item) => item.id === botId) : undefined
  const botProblem = bots.kind === 'ready' ? bots.problems?.find((item) => item.bot_id === botId) : undefined
  const map = preview?.kind === 'ready' ? preview.map : null
  const busy = listing.kind === 'loading' || bots.kind === 'loading' || preview?.kind === 'loading'
  const visibleFiles = listing.kind === 'ready'
    ? listing.files.filter((name) => `${listing.titles?.[name] ?? ''} ${name}`.toLocaleLowerCase().includes(filter.toLocaleLowerCase())) : []
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
    if (editorState.selectFile(name)) setLaunchOpen(false)
  }

  function selectRelatedNode(id: string) {
    selectNode(id)
    nodeButtons.current.get(id)?.focus()
  }

  const editor = draft && <form className="scheduler-editor" aria-label={draft.kind === 'node' ? 'node 编辑' : 'Bot 编辑'} onSubmit={(event) => { event.preventDefault(); void submitDefinition() }}>
    <h3>{draft.create ? '新增' : '修改'}{draft.kind === 'node' ? ' node' : ' Bot'}</h3>
    <label>{draft.kind} ID<input value={draft.id} readOnly={!draft.create} disabled={editingBusy} onChange={(event) => editDraft({ id: event.target.value })} /></label>
    <p>内容不含顶层 id；身份由独立 ID 决定。保存期间请勿用外部编辑器修改定义。</p>
    <p>{draft.kind === 'node' ? 'node 必须填写 title、bot_id、字符串 input、字符串 zone 路径和 blocked_by。允许暂未登记的 Bot 或 zone；同图依赖必须存在且无环。' : 'Bot 必须填写 name、when、plan_guide、node_guide；三份文档允许空文本，task_id 可暂缺；执行时必须引用普通（normal）Task，不能使用 manager。同一普通 Task 也可从 zone 独立启动。只保存 Task 引用，不复制 Task 或 Chat。'} 不支持 schema_version 或旧格式字段。</p>
    {draft.kind === 'Bot' ? <BotTaskSelect content={draft.content} disabled={editingBusy} onChange={content => editDraft({ content })} />
      : <NodeSettings content={draft.content} disabled={editingBusy} onChange={content => editDraft({ content })} />}
    <label htmlFor="definition-content-json">{draft.kind} 内容 JSON</label><textarea id="definition-content-json" aria-label={draft.kind === 'node' ? 'node 内容 JSON' : 'Bot 内容 JSON'} spellCheck={false} value={draft.content} disabled={editingBusy} onChange={(event) => editDraft({ content: event.target.value })} />
    <div className="scheduler-actions"><button type="submit" disabled={editingBusy}>{draft.kind === 'Bot' ? 'save' : '保存 node'}</button><button type="button" disabled={editingBusy} onClick={abandonDraft}>{draft.kind === 'Bot' ? 'cancel' : '放弃编辑'}</button>{draft.kind === 'node' && <button type="button" disabled={editingBusy} onClick={abandonDraft}>返回 Map</button>}</div>
  </form>

  const refreshButton = <button type="button" disabled={busy || editingBusy} onClick={refreshAll}>{botView ? 'refresh' : '刷新全部'}</button>

  return <section ref={schedulerRoot} className="scheduler" aria-label="Map 目录">
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
      <div className="scheduler-bot-heading">
        <h2 ref={botHeading} tabIndex={-1}>bot</h2>
        <div className="scheduler-actions">
          {!draft && <button type="button" disabled={editingBusy} onClick={() => void beginEdit(null)}>new</button>}
          {refreshButton}
        </div>
      </div>
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
          {draft ? editor : botId === null ? <div className="scheduler-placeholder"><h3>选择一个 Bot</h3><p>从左侧列表选择 Bot，分别阅读 when、plan_guide 和 node_guide。</p></div> : <>
            {bot && !botProblem && <div className="scheduler-actions"><button type="button" disabled={editingBusy} onClick={() => void beginEdit(bot.id)}>edit</button><button type="button" disabled={editingBusy} onClick={() => void removeBot()}>delete</button></div>}
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
        <div className="scheduler-section-heading scheduler-directory-heading"><h3 ref={directoryHeading} tabIndex={-1}>Map 目录</h3>{listing.kind === 'ready' && <span>{listing.files.length} 个文件</span>}{refreshButton}</div>
        <label className="scheduler-filter">筛选 Map<input type="search" value={filter} onChange={(event) => setFilter(event.target.value)} placeholder="名称或文件名…" /></label>
        {listing.kind === 'loading' && <p>正在读取 Map 目录……</p>}
        {listing.kind === 'error' && <p role="alert" data-testid="scheduler-error">{listing.message}</p>}
        {listing.kind === 'ready' && (listing.files.length === 0
          ? <p data-testid="scheduler-empty">此目录没有 Map JSON 文件。</p>
          : <>
            <p className="scheduler-filter-count" role="status">显示 {visibleFiles.length} / {listing.files.length}</p>
            {visibleFiles.length === 0 && <p>没有匹配的 Map 文件。清除筛选可查看全部。</p>}
            <ul data-testid="scheduler-map-list" className="scheduler-map-list">{visibleFiles.map((name) => <li key={name}>
              <button type="button" data-map-file={name} aria-pressed={file === name} onClick={() => selectFile(name)}>{listing.titles?.[name]?.trim() ? listing.titles[name] : name}</button>
            </li>)}</ul>
          </>)}
      </aside>
      <div className="scheduler-main">
        {editError && <p role="alert">{editError}</p>}
        {draft?.kind === 'node' && editor}
        {file !== null && !draft && <div className="scheduler-actions">
          {map && <>
            <button type="button" disabled={editingBusy} onClick={() => void beginEdit(null, 'node')}>新增 node</button>
            <button type="button" disabled={editingBusy} onClick={() => setLaunchOpen(true)}>+ 发起 Run</button>
          </>}
          <button type="button" disabled={editingBusy || deletingMap} onClick={requestMapDeletion}>删除 Map</button>
        </div>}
        {launchOpen && file && <div hidden={botView || draft !== null}><RunLaunchForm key={file} file={file} active={active && !botView && !draft}
          onClose={() => { setLaunchOpen(false); void returnToMap() }}
          onNode={(id) => { selectNode(id); void beginEdit(id, 'node') }}
          onBot={(id) => { openBot(id); if (bots.kind === 'ready' && bots.bots.some(bot => bot.id === id)) void beginEdit(id, 'Bot') }}
          onTask={onTask} onZones={onZones} /></div>}
        {file === null && <div className="scheduler-placeholder"><h3>选择一个 Map</h3><p>从目录中选择文件，阅读节点定义与前置依赖。</p></div>}
        {preview?.kind === 'loading' && <p className="scheduler-placeholder">正在读取 Map……</p>}
        {preview?.kind === 'error' && <div className="scheduler-placeholder"><h3 className="scheduler-text">{file}</h3><p role="alert">{preview.message}</p><p>可选择其他 Map；修复文件后点击刷新全部。</p></div>}
        {map && <section aria-label="Map 预览">
          <header className="scheduler-map-heading"><div><p className="scheduler-filename">{file}</p><h3 className="scheduler-text">{map.title}</h3><p className="scheduler-text">{map.description}</p></div><span className="scheduler-badge">静态定义</span></header>
          <MapGraph map={map} selectedId={selectedId} onSelect={selectNode} botProblems={botProblems} />
          <div className="scheduler-reading">
            <div className="scheduler-outline">
              <section aria-label="节点列表">
              <div className="scheduler-section-heading"><h4>节点与前置依赖</h4><span>{Object.keys(map.nodes).length} 个节点</span></div>
              {Object.keys(map.nodes).length === 0 && <p>此 Map 没有 node。</p>}
              <ul className="scheduler-node-list">{orderedMapNodeIds(map).map(id => {
                const node = map.nodes[id]
                return <li key={id}>
                <button type="button" ref={(element) => { if (element) nodeButtons.current.set(id, element); else nodeButtons.current.delete(id) }}
                  aria-label={`${node.title} ${id}`} aria-pressed={selectedId === id} onClick={() => selectNode(id)}>
                  <span className="scheduler-node-id">{id}</span><span className="scheduler-node-summary"><strong>{node.title}</strong>
                    <span>{node.blocked_by.length === 0 ? '无前置依赖' : `前置：${node.blocked_by.map((parent) => `${map.nodes[parent].title} · ${parent}`).join('、')}`}</span>
                    <span>Bot：{node.bot_id}</span>{botProblems[id] && <span className="scheduler-node-warning">{botProblems[id]}</span>}
                  </span><span aria-hidden="true">→</span>
                </button>
              </li>})}</ul>
              </section>
            </div>
            <aside className="scheduler-inspector" aria-label="节点检查器">
              {selectedId !== null && Object.hasOwn(map.nodes, selectedId) ? <>
                <div className="scheduler-inspector-heading"><span>节点详情</span><button type="button" aria-label="关闭节点详情" onClick={() => {
                  nodeButtons.current.get(selectedId)?.focus()
                  selectNode(null)
                }}>关闭</button></div>
                {!draft && <div className="scheduler-actions"><button type="button" disabled={editingBusy} onClick={() => void beginEdit(selectedId, 'node')}>修改 node</button><button type="button" disabled={editingBusy} onClick={() => void removeSelectedNode()}>删除 node</button></div>}
                <MapNodeDetails key={`${file}:${selectedId}`} map={map} nodeId={selectedId} onSelect={selectRelatedNode}
                  botProblem={botProblems[selectedId]} onBot={openBot}
                  bot={bots.kind === 'ready' && !botProblems[selectedId] ? bots.bots.find((bot) => bot.id === map.nodes[selectedId].bot_id) : undefined} />
              </> : <div className="scheduler-placeholder"><h4>选择一个节点</h4><p>查看本次输入、zone 路径引用、前置依赖与共享 Bot。</p></div>}
            </aside>
          </div>
        </section>}
      </div>
    </div>}
    {deleteMapTarget !== null && <dialog ref={deleteMapDialog} className="zone-dialog scheduler-map-delete-dialog" data-testid="scheduler-map-delete-dialog"
      aria-modal="true" aria-labelledby="scheduler-map-delete-title" aria-describedby="scheduler-map-delete-description" aria-busy={deletingMap}
      onCancel={(event) => { event.preventDefault(); closeMapDeletion() }}>
      <h2 id="scheduler-map-delete-title" className="zone-dialog__title">删除 Map「{deleteMapTarget}」？</h2>
      <div id="scheduler-map-delete-description" className="zone-dialog__hint">
        <p>“删除历史”和“保留历史”都会不可撤销地删除此 Map 定义。</p>
        <p>删除历史：同时删除此 Map 关联的所有 Run 记录及详情，不论 Run 状态或 Chat 是否在线。仍在运行的 Chat 不会停止，后续汇报可能因 Run 已删除而失败。</p>
        <p>保留历史：保留关联 Run，仍可在工作流中查看已保存事实，但当前 Map 定义与依赖图将不可用。</p>
        <p>两种选择都不会停止 Chat，也不会删除 Chat、原生会话历史、共享 Bot、Task 或 zone 文件。</p>
      </div>
      {deleteMapError && <p role="alert">{deleteMapError}</p>}
      {deletingMap && <p role="status">正在删除，请勿重复提交……</p>}
      <div className="zone-dialog__actions">
        <button type="button" className="zone-dialog__cancel" disabled={deletingMap} onClick={() => void removeMap(true)}>删除历史</button>
        <button type="button" className="zone-dialog__cancel" disabled={deletingMap} onClick={() => void removeMap(false)}>保留历史</button>
        <button type="button" className="zone-dialog__cancel" ref={cancelMapDeletion} autoFocus disabled={deletingMap} onClick={closeMapDeletion}>取消</button>
      </div>
    </dialog>}
  </section>
}
