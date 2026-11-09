import { useEffect, useRef, useState, type RefObject } from 'react'
import { EntityGroups, GroupButton } from '../app/entityGroups'
import { useSchedulerEditor } from './useSchedulerEditor'
import { MapGraph } from './MapGraph'
import { orderedMapNodeIds } from './mapNodeOrder'
import { MapNodeDetails } from './MapNodeDetails'
import { BotPreview } from './BotPreview'
import { BotTaskSelect } from './BotTaskSelect'
import { RunLaunchForm } from './RunLaunchForm'
import { MapInputsEditor } from './MapInputsEditor'
import './scheduler.css'
import './scheduler-workspace.css'

export function SchedulerPanel({ active, view, onViewChange, navigationGuard, onTask, onZones }: { active: boolean; view: 'bot' | 'map'; onViewChange: (view: 'bot' | 'map') => void; navigationGuard?: RefObject<(() => boolean) | null>; onTask: (id?: string) => void; onZones: () => void }) {
  const botView = view === 'bot'
  const editorState = useSchedulerEditor({ active, view, onViewChange })
  const { listing, bots, file, preview, selectedId, botId, draft, editingBusy, editError, returnProblem, deleteMapTarget, deletingMap, deleteMapError,
    changeGroup, abandonDraft, selectNode, openBot, beginEdit, editDraft, submitDefinition, removeSelectedNode, removeBot, requestMapDeletion, closeMapDeletion, removeMap, refreshAll, returnToMap } = editorState
  const [launchOpen, setLaunchOpen] = useState(false)
  const [inputsOpen, setInputsOpen] = useState(false)
  const [directoryOpen, setDirectoryOpen] = useState(false)
  const inspectorHeading = useRef<HTMLHeadingElement>(null)
  const editorHeading = useRef<HTMLHeadingElement>(null)
  const [filter, setFilter] = useState('')
  const [botFilter, setBotFilter] = useState('')
  const [botGroup, setBotGroup] = useState('all')
  const botListPosition = useRef({ top: 0, button: null as HTMLButtonElement | null })
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

  useEffect(() => {
    if (!active || botView) return
    if (draft?.kind === 'node') editorHeading.current?.focus()
    else if (selectedId !== null && window.matchMedia('(max-width: 1100px)').matches) inspectorHeading.current?.focus()
  }, [draft?.kind, selectedId, active, botView])

  const bot = bots.kind === 'ready' ? bots.bots.find((item) => item.id === botId) : undefined
  const botProblem = bots.kind === 'ready' ? bots.problems?.find((item) => item.bot_id === botId) : undefined
  const map = preview?.kind === 'ready' ? preview.map : null
  const busy = listing.kind === 'loading' || bots.kind === 'loading' || preview?.kind === 'loading'
  const visibleFiles = listing.kind === 'ready'
    ? listing.files.filter((name) => `${listing.titles?.[name] ?? ''} ${name}`.toLocaleLowerCase().includes(filter.toLocaleLowerCase())) : []
  const botGroups = bots.kind === 'ready' ? [...new Set(bots.bots.flatMap(item => item.group == null ? [] : [item.group]))].sort() : []
  const visibleBots = bots.kind === 'ready'
    ? bots.bots.filter((item) => (botGroup === 'all' || (botGroup === 'ungrouped' ? item.group == null : item.group === botGroup.slice(6)))
      && `${item.name} ${item.id} ${item.group ?? '未分组'}`.toLocaleLowerCase().includes(botFilter.toLocaleLowerCase())) : []
  const botDetailOpen = botId !== null || draft !== null

  function returnToBotList() {
    if (!abandonDraft()) return
    openBot(null)
    requestAnimationFrame(() => {
      botListPosition.current.button?.focus({ preventScroll: true })
      window.scrollTo({ top: botListPosition.current.top })
    })
  }
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
    if (editorState.selectFile(name)) { setLaunchOpen(false); setInputsOpen(false); setDirectoryOpen(false) }
  }

  function selectRelatedNode(id: string) {
    selectNode(id)
    if (!window.matchMedia('(max-width: 1100px)').matches) nodeButtons.current.get(id)?.focus()
  }

  function closeNode() {
    if (!abandonDraft()) return
    const id = selectedId
    selectNode(null)
    requestAnimationFrame(() => { if (id) nodeButtons.current.get(id)?.focus() })
  }

  const editor = draft && <form className="scheduler-editor" aria-label={draft.kind === 'node' ? 'node 编辑' : 'Bot 编辑'} onFocusCapture={event => {
    if (draft.kind === 'node' && event.target.matches('input:focus-visible, textarea:focus-visible, select:focus-visible, .scheduler-node-content button:focus-visible, .scheduler-node-content summary:focus-visible')) event.target.scrollIntoView({ block: 'center', inline: 'nearest' })
  }} onSubmit={(event) => { event.preventDefault(); void submitDefinition() }}>
    <div className="scheduler-section-heading"><h3 ref={editorHeading} tabIndex={-1}>{draft.create ? '新增' : '修改'}{draft.kind === 'node' ? ' node' : ' Bot'}</h3><span>保存后生效</span></div>
    {draft.kind === 'Bot' && !draft.create && bot && bots.kind === 'ready' && <GroupButton name={bot.name} group={bot.group} names={bots.bots.map(item => item.group)} disabled={editingBusy} onSave={group => changeGroup('bots', bot.id, group)} />}
    {draft.kind === 'Bot' && <label>Bot ID<input value={draft.id} readOnly={!draft.create} disabled={editingBusy} onChange={(event) => editDraft({ id: event.target.value })} /></label>}
    {draft.kind === 'node' && map && draft.settings && <MapNodeDetails key={`${draft.file}:${draft.initialId}`} map={map} nodeId={draft.id} catalog={bots.kind === 'ready' ? bots : undefined} onSelect={selectRelatedNode} edit={{ content: draft.content, settings: draft.settings, create: draft.create, disabled: editingBusy, onContentChange: content => editDraft({ content }), onSettingsChange: settings => editDraft({ settings }), onIdChange: id => editDraft({ id }) }} />}
    <details className="scheduler-format-help"><summary>字段格式与保存约束</summary><p>内容不含顶层 id；身份由独立 ID 决定。保存期间请勿用外部编辑器修改定义。</p>
    <p>{draft.kind === 'node' ? 'node ID 独立填写；zone、report_policy、auto_close 使用独立控件，不放入内容 JSON。其余字段保持原格式 JSON：必填 title、bot_id、字符串 input 和 blocked_by。可选 input_rule 数组填写 {id:"输入 ID",type:"file",path:"relative/file.txt",zone_id:"namespace/name"}（zone_id 可省略，默认使用本次 Run 的管理者 zone，不使用 node zone；path 必须为 / 分隔的相对路径，不可越界） 或 {id:"输入 ID",type:"node",node_id:"来源 ID",output_id:"输出 ID"}，或 {id:"本地 ID",type:"run",input_id:"本 Map 声明的输入 ID"}（仅本次 Run，不接受 run_id；未提供的可选项不生成地址）。实际文件在启动／派发时须存在且可读；output_rule 数组填写 {id:"输出 ID"}。各数组内 ID 唯一；输出 ID 不是文件名。输入引用不增加 blocked_by。允许暂未登记的 Bot 或 zone；来源 node 和其声明的 output_rule ID 必须存在，同图依赖必须存在且无环。' : 'Bot 必须填写 name、when、plan_guide、node_guide；三份文档允许空文本。可未绑定，或只填写 task_id（普通 normal Task）与 map_file（Map 文件名）之一，两者互斥。Map 型调用节点填写 manager_task_id（manager Task）；Bot 不固化 manager。不复制 Task、Map 或 Chat。'} 不支持 schema_version 或旧格式字段。</p></details>
    {draft.kind === 'Bot' && <BotTaskSelect content={draft.content} disabled={editingBusy} onChange={content => editDraft({ content })} />}
    {draft.kind === 'Bot' && <><label htmlFor="definition-content-json">Bot 内容 JSON</label><textarea id="definition-content-json" aria-label="Bot 内容 JSON" spellCheck={false} value={draft.content} disabled={editingBusy} onChange={(event) => editDraft({ content: event.target.value })} /></>}
    <div className="scheduler-actions"><button type="submit" disabled={editingBusy}>{draft.kind === 'Bot' ? 'save' : '保存 node'}</button><button type="button" disabled={editingBusy} onClick={abandonDraft}>{draft.kind === 'Bot' ? 'cancel' : '放弃编辑'}</button>{draft.kind === 'node' && <button type="button" disabled={editingBusy} onClick={closeNode}>返回 Map</button>}</div>
  </form>

  const refreshButton = <button type="button" disabled={busy || editingBusy} onClick={refreshAll}>{botView ? 'refresh' : '刷新全部'}</button>

  return <section ref={schedulerRoot} className={`scheduler${!botView ? ' scheduler-workspace' : ''}`} aria-label="Map 目录" data-node-open={selectedId !== null || draft?.kind === 'node' ? 'true' : undefined} data-directory-open={directoryOpen || file === null ? 'true' : undefined}>
    {!botView && <header className="scheduler-workspace-heading"><div><h1>Scheduler</h1><p>Map 定义与节点编排</p></div><button type="button" className="scheduler-directory-toggle" aria-expanded={directoryOpen || file === null} aria-controls="scheduler-map-directory" onClick={() => setDirectoryOpen(value => !value)}>Map 目录</button></header>}
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
    {botView ? <div className="scheduler-bot-page" data-detail-open={botDetailOpen}>
      <div className="scheduler-bot-heading">
        <h1 ref={botHeading} tabIndex={-1}>Bot</h1>
        <div className="scheduler-actions">
          {!draft && <button type="button" disabled={editingBusy} onClick={() => void beginEdit(null)}>new</button>}
          {refreshButton}
        </div>
      </div>
      {editError && <p role="alert">{editError}</p>}
      <div className="scheduler-bot-layout">
        <section aria-label="共享 Bot 列表" className="scheduler-bot-directory">
          <div className="scheduler-bot-filters">
            <label className="scheduler-filter">筛选 Bot<input type="search" value={botFilter} onChange={(event) => setBotFilter(event.target.value)} placeholder="名称、ID 或组名…" /></label>
            <label className="scheduler-filter">分组<select value={botGroup} onChange={event => setBotGroup(event.target.value)}>
              <option value="all">全部分组</option>
              <option value="ungrouped">未分组</option>
              {botGroups.map(name => <option key={name} value={'group:' + name}>{name}</option>)}
              {botGroup.startsWith('group:') && !botGroups.includes(botGroup.slice(6)) && <option value={botGroup}>{botGroup.slice(6)}（已无 Bot）</option>}
            </select></label>
          </div>
          {bots.kind === 'ready' && <>
            <div className="scheduler-bot-list-meta"><p className="scheduler-filter-count" role="status">显示 {visibleBots.length} / {bots.bots.length} 个 Bot</p>{(botFilter || botGroup !== 'all') && <button type="button" onClick={() => { setBotFilter(''); setBotGroup('all') }}>清除筛选</button>}</div>
            {bots.bots.length === 0 && !bots.problems?.length && <p data-testid="scheduler-bot-empty">共享 Bot 目录没有 Bot JSON 文件。</p>}
            {bots.bots.length > 0 && visibleBots.length === 0 && <p>没有匹配的 Bot。清除筛选可查看全部。</p>}
            <EntityGroups items={visibleBots} groupOf={item => item.group} listClass="scheduler-map-list">{(item) => <li key={item.id}>
              <button type="button" disabled={editingBusy} aria-pressed={botId === item.id} aria-label={`${item.name} · ${item.id}`} onClick={event => { botListPosition.current = { top: window.scrollY, button: event.currentTarget }; openBot(item.id) }}><strong>{item.name}</strong><small title={item.id}>{item.id}</small></button>
              <GroupButton compact name={item.name} group={item.group} names={bots.bots.map(bot => bot.group)} disabled={editingBusy} onSave={group => changeGroup('bots', item.id, group)} />
            </li>}</EntityGroups>
          </>}
        </section>
        <div className="scheduler-bot-main">
          {botDetailOpen && <button type="button" className="scheduler-bot-back" disabled={editingBusy} onClick={returnToBotList}>← 返回 Bot 列表</button>}
          {draft ? editor : botId === null ? <div className="scheduler-placeholder"><h3>选择一个 Bot</h3><p>从左侧列表选择 Bot，分别阅读 when、plan_guide 和 node_guide。</p></div> : <>
            {bot && !botProblem && <div className="scheduler-actions"><GroupButton name={bot.name} group={bot.group} names={bots.kind === 'ready' ? bots.bots.map(item => item.group) : []} disabled={editingBusy} onSave={group => changeGroup('bots', bot.id, group)} /><button type="button" disabled={editingBusy} onClick={() => void beginEdit(bot.id)}>edit</button><button type="button" disabled={editingBusy} onClick={() => void removeBot()}>delete</button></div>}
            <section aria-label="Bot 预览" className="scheduler-details">
              {bots.kind === 'ready' && (botProblem
                ? <p role="alert">共享 Bot {botId}：{botProblem.message}</p>
                : bot ? <BotPreview key={bot.id} bot={bot} /> : <p role="alert">缺失共享 Bot：{botId}</p>)}
            </section>
          </>}
        </div>
      </div>
    </div> : <div className="scheduler-layout">
      <aside id="scheduler-map-directory" className="scheduler-directory" aria-label="Map 文件目录">
        <div className="scheduler-section-heading scheduler-directory-heading"><h3 ref={directoryHeading} tabIndex={-1}>Map 目录</h3>{listing.kind === 'ready' && <span>{listing.files.length} 个文件</span>}{refreshButton}</div>
        <label className="scheduler-filter">筛选 Map<input type="search" value={filter} onChange={(event) => setFilter(event.target.value)} placeholder="名称或文件名…" /></label>
        {listing.kind === 'loading' && <p>正在读取 Map 目录……</p>}
        {listing.kind === 'error' && <p role="alert" data-testid="scheduler-error">{listing.message}</p>}
        {listing.kind === 'ready' && (listing.files.length === 0
          ? <p data-testid="scheduler-empty">此目录没有 Map JSON 文件。</p>
          : <>
            <p className="scheduler-filter-count" role="status">显示 {visibleFiles.length} / {listing.files.length}</p>
            {visibleFiles.length === 0 && <p>没有匹配的 Map 文件。清除筛选可查看全部。</p>}
            <div data-testid="scheduler-map-list"><EntityGroups items={visibleFiles} groupOf={name => listing.groups?.[name]} listClass="scheduler-map-list">{(name) => <li key={name}>
              <button type="button" data-map-file={name} aria-pressed={file === name} onClick={() => selectFile(name)}>{listing.titles?.[name]?.trim() ? listing.titles[name] : name}</button>
              <GroupButton name={listing.titles?.[name] || name} group={listing.groups?.[name]} names={Object.values(listing.groups ?? {})} disabled={editingBusy} onSave={group => changeGroup('maps', name, group)} />
            </li>}</EntityGroups></div>
          </>)}
      </aside>
      <div className="scheduler-main">
        {editError && <p role="alert">{editError}</p>}
        {draft?.kind === 'node' && !map && editor}
        {file !== null && !draft && <div className="scheduler-actions scheduler-map-actions">
          {map && <>
            <button type="button" disabled={editingBusy} onClick={() => void beginEdit(null, 'node')}>新增 node</button>
            <button type="button" disabled={editingBusy || inputsOpen} onClick={() => { setLaunchOpen(true); setInputsOpen(false) }}>+ 发起 Run</button>
            <button type="button" disabled={editingBusy || inputsOpen} onClick={() => { setInputsOpen(true); setLaunchOpen(false) }}>编辑文件输入声明</button>
          </>}
          <button type="button" disabled={editingBusy || deletingMap} onClick={requestMapDeletion}>删除 Map</button>
        </div>}
        {inputsOpen && file && !botView && !draft && <MapInputsEditor key={file} file={file} onClose={() => setInputsOpen(false)} onSaved={() => { setInputsOpen(false); refreshAll() }} />}
        {launchOpen && file && <div hidden={botView || draft !== null}><RunLaunchForm key={file} file={file} declarations={map?.input_files} active={active && !botView && !draft}
          onClose={() => { setLaunchOpen(false); void returnToMap() }}
          onNode={(id) => { selectNode(id); void beginEdit(id, 'node') }}
          onBot={(id) => { openBot(id); if (bots.kind === 'ready' && bots.bots.some(bot => bot.id === id)) void beginEdit(id, 'Bot') }}
          onTask={onTask} onZones={onZones} /></div>}
        {file === null && <div className="scheduler-placeholder"><h3>选择一个 Map</h3><p>从目录中选择文件，阅读节点定义与前置依赖。</p></div>}
        {preview?.kind === 'loading' && <p className="scheduler-placeholder">正在读取 Map……</p>}
        {preview?.kind === 'error' && <div className="scheduler-placeholder"><h3 className="scheduler-text">{file}</h3><p role="alert">{preview.message}</p><p>可选择其他 Map；修复文件后点击刷新全部。</p></div>}
        {map && <section aria-label="Map 预览" className="scheduler-map-workbench">
          <header className="scheduler-map-heading"><GroupButton name={map.title} group={map.group} names={listing.kind === 'ready' ? Object.values(listing.groups ?? {}) : []} disabled={editingBusy} onSave={group => changeGroup('maps', file!, group)} /><div><p className="scheduler-filename">{file}</p><h3 className="scheduler-text">{map.title}</h3><p className="scheduler-text">{map.description}</p></div><span className="scheduler-badge">静态定义</span></header>
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
                    <span>bot_id：{node.bot_id}</span>{botProblems[id] && <span className="scheduler-node-warning">{botProblems[id]}</span>}
                  </span><span aria-hidden="true">→</span>
                </button>
              </li>})}</ul>
              </section>
            </div>
            <aside className="scheduler-inspector" aria-label="节点检查器">
              {(selectedId !== null || draft?.kind === 'node') && <div className="scheduler-inspector-heading"><h2 ref={inspectorHeading} tabIndex={-1}>{draft?.kind === 'node' ? '节点编辑' : '节点详情'}</h2><button type="button" disabled={editingBusy} aria-label="关闭节点详情" onClick={closeNode}>返回 Map</button></div>}
              {draft?.kind === 'node' ? editor : selectedId !== null && Object.hasOwn(map.nodes, selectedId) ? <>
                {!draft && <div className="scheduler-actions"><button type="button" disabled={editingBusy} onClick={() => void beginEdit(selectedId, 'node')}>修改 node</button><button type="button" disabled={editingBusy} onClick={() => void removeSelectedNode()}>删除 node</button></div>}
                <MapNodeDetails key={`${file}:${selectedId}`} map={map} nodeId={selectedId} onSelect={selectRelatedNode} catalog={bots.kind === 'ready' ? bots : undefined} />
              </> : <div className="scheduler-placeholder"><h4>选择一个节点</h4><p>查看本次输入、zone ID 引用、前置依赖与共享 Bot。</p></div>}
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
