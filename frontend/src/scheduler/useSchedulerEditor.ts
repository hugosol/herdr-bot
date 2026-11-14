import { useEffect, useRef, useState } from 'react'
import type { NodeSettingsValue } from './NodeSettings'
import { listBots, loadBotDraft, saveBot, checkBotDelete, deleteBot, updateBotGroup, type BotRevision, type BotCatalog } from '../controllers/botController'
import { groupOnlyRevision, updateMapGroup, listMaps, loadMap, loadNodeDraft, saveNode, checkNodeDeletion, deleteNode, deleteMap, type WorkflowMap, type MapCatalog } from '../controllers/schedulerController'

type Listing = { kind: 'loading' } | ({ kind: 'ready' } & MapCatalog) | { kind: 'error'; message: string }
type Preview = { kind: 'loading' } | { kind: 'ready'; map: WorkflowMap } | { kind: 'error'; message: string }
type Bots = { kind: 'loading' } | ({ kind: 'ready' } & BotCatalog) | { kind: 'error'; message: string }
type Draft = { kind: 'Bot' | 'node'; create: boolean; id: string; content: string; revision: BotRevision | null; initialId: string; initialContent: string; file: string | null; settings: NodeSettingsValue | null; initialSettings: NodeSettingsValue | null }
const draftChanged = (draft: Draft) => draft.id !== draft.initialId || draft.content !== draft.initialContent || draft.settings?.zone !== draft.initialSettings?.zone || draft.settings?.report_policy !== draft.initialSettings?.report_policy || draft.settings?.auto_close !== draft.initialSettings?.auto_close
type State = { listing: Listing; bots: Bots; file: string | null; preview: Preview | null; selectedId: string | null; botId: string | null; draft: Draft | null; editingBusy: boolean; editError: string | null; returnProblem: string | null; deleteMapTarget: string | null; deletingMap: boolean; deleteMapError: string | null }
const message = (error: unknown) => error instanceof Error ? error.message : '读取或修改失败。'

/** Owns edit/leave permission, authoritative mutation outcomes and all refresh ownership. */
export function useSchedulerEditor({ active, view, onViewChange, confirm = (text: string) => window.confirm(text) }: { active: boolean; view: 'bot' | 'map'; onViewChange: (view: 'bot' | 'map') => void; confirm?: (text: string) => boolean }) {
  const [state, setState] = useState<State>({ listing: { kind: 'loading' }, bots: { kind: 'loading' }, file: null, preview: null, selectedId: null, botId: null, draft: null, editingBusy: false, editError: null, returnProblem: null, deleteMapTarget: null, deletingMap: false, deleteMapError: null })
  const current = useRef(state)
  const enabled = useRef(active)
  enabled.current = active
  const mounted = useRef(false)
  const lifetime = useRef(0)
  const inFlight = useRef(false)
  const request = useRef<{ generation: number; controller: AbortController | null }>({ generation: 0, controller: null })
  const previousView = useRef(view)
  function update(patch: Partial<State>) {
    if (!mounted.current || !enabled.current) return
    current.current = { ...current.current, ...patch }
    setState(current.current)
  }
  function invalidate() { request.current.controller?.abort(); request.current = { generation: request.current.generation + 1, controller: null } }
  function beginRequest() {
    invalidate()
    const controller = new AbortController()
    request.current.controller = controller
    const generation = request.current.generation
    return { signal: controller.signal, valid: () => mounted.current && enabled.current && generation === request.current.generation && !controller.signal.aborted }
  }
  function applyMap(map: WorkflowMap) {
    const selected = current.current.selectedId
    const removed = selected !== null && !Object.hasOwn(map.nodes, selected)
    update({ preview: { kind: 'ready', map }, ...(removed ? { selectedId: null, returnProblem: `来源 node ${selected} 已移除，原选择已失效。` } : {}) })
  }
  async function refresh(includeListing = true, saved = false) {
    if (!enabled.current) return
    includeListing = includeListing || current.current.listing.kind === 'loading'
    const owner = beginRequest()
    const file = current.current.file
    update({ returnProblem: null, ...(includeListing ? { listing: { kind: 'loading' } as Listing } : {}), ...(!saved ? { bots: { kind: 'loading' } as Bots } : {}), ...(file ? { preview: { kind: 'loading' } as Preview } : {}) })
    const jobs: Promise<void>[] = []
    if (includeListing) jobs.push(listMaps(owner.signal).then(catalog => {
      if (owner.valid()) update({ listing: { kind: 'ready', ...catalog } })
    }, error => {
      if (owner.valid()) update({ listing: { kind: 'error', message: message(error) } })
    }))
    if (file) jobs.push(loadMap(file, owner.signal).then(map => {
      if (owner.valid()) applyMap(map)
    }, error => {
      if (!owner.valid()) return
      update({
        preview: { kind: 'error', message: `来源 Map 已失效：${message(error)}` },
        ...(error instanceof Error && 'status' in error && error.status === 404
          ? { selectedId: null, returnProblem: '来源 Map 已移除，原 node 选择已失效。' } : {}),
        ...(saved ? { editError: `Bot 已保存，但 Map 刷新失败：${message(error)}。请显式刷新；不会再次保存。` } : {}),
      })
    }))
    jobs.push(listBots(owner.signal).then(catalog => {
      if (owner.valid()) update({ bots: { kind: 'ready', ...catalog } })
    }, error => {
      if (!owner.valid()) return
      update(saved
        ? { editError: `Bot 已保存，但目录刷新失败：${message(error)}。当前显示已确认保存的 Bot，目录尚未核实；请显式刷新，不会再次保存。` }
        : { bots: { kind: 'error', message: message(error) } })
    }))
    await Promise.all(jobs)
  }
  function abandonDraft() {
    const draft = current.current.draft
    if (inFlight.current || current.current.deleteMapTarget !== null) return false
    if (draft && draftChanged(draft) && !confirm(`放弃尚未保存的 ${draft.kind} 修改？`)) return false
    update({ draft: null, editError: null })
    return true
  }
  async function operation(work: (valid: () => boolean) => Promise<void>, deletion = false) {
    const snapshot = current.current
    if (inFlight.current || !enabled.current || !mounted.current || snapshot.listing.kind === 'loading' || snapshot.bots.kind === 'loading' || snapshot.preview?.kind === 'loading') return
    inFlight.current = true
    invalidate()
    const epoch = lifetime.current
    const valid = () => mounted.current && enabled.current && epoch === lifetime.current
    update({ editingBusy: true, editError: null, ...(deletion ? { deletingMap: true, deleteMapError: null } : {}) })
    try { await work(valid) }
    catch (error) { if (valid()) update(deletion ? { deleteMapError: message(error) } : { editError: message(error) }) }
    finally { if (epoch === lifetime.current) { inFlight.current = false; if (valid()) update({ editingBusy: false, deletingMap: false }) } }
  }
  function selectNode(id: string | null) {
    if (inFlight.current || current.current.draft || current.current.deleteMapTarget) return
    const preview = current.current.preview
    if (id === null || (preview?.kind === 'ready' && Object.hasOwn(preview.map.nodes, id))) update({ selectedId: id })
  }
  function selectFile(file: string) {
    if (file === current.current.file || !abandonDraft()) return false
    update({ file, selectedId: null, preview: { kind: 'loading' }, returnProblem: null })
    void refresh(false)
    return true
  }
  function openBot(id: string | null) {
    if (!abandonDraft()) return
    update({ botId: id }); onViewChange('bot')
  }
  function discardForNavigation() {
    if (inFlight.current) return false
    update({ draft: null, editError: null, deleteMapTarget: null, deleteMapError: null })
    return true
  }
  function navigateDefinition(target: { kind: 'bot' | 'map'; id: string }) {
    if (!discardForNavigation()) return
    update(target.kind === 'bot'
      ? { botId: target.id, selectedId: null }
      : { file: target.id, selectedId: null, preview: { kind: 'loading' }, returnProblem: null })
    onViewChange(target.kind)
    void refresh()
  }
  async function beginEdit(id: string | null, kind: Draft['kind'] = 'Bot') {
    if (!abandonDraft()) return
    const file = current.current.file
    if (kind === 'node' && file === null) return
    await operation(async valid => {
      const loaded = kind === 'Bot' ? await loadBotDraft(id) : await loadNodeDraft(file!, id)
      if (!valid()) return
      let content = loaded.content
      let settings: NodeSettingsValue | null = null
      if (kind === 'node') {
        const { zone, report_policy = 'auto', auto_close = false, node_id: _nodeId, ...definition } = JSON.parse(content)
        settings = { zone, report_policy, auto_close }
        content = JSON.stringify(definition, null, 2)
      }
      update({ draft: { ...loaded, content, kind, create: id === null, initialId: loaded.id, initialContent: content, file, settings, initialSettings: settings } })
      onViewChange(kind === 'Bot' ? 'bot' : 'map')
    })
  }
  function editDraft(patch: { id?: string; content?: string; settings?: Partial<NodeSettingsValue> }) {
    const draft = current.current.draft
    if (!inFlight.current && draft) update({ draft: { ...draft, ...patch, settings: draft.settings && patch.settings ? { ...draft.settings, ...patch.settings } : draft.settings } })
  }
  async function submitDefinition() {
    const draft = current.current.draft
    if (!draft) return
    await operation(async valid => {
      const value: unknown = JSON.parse(draft.content)
      if (typeof value === 'object' && value !== null && Object.hasOwn(value, 'id')) throw new Error(`请移除 ${draft.kind} 内容中的顶层 id；使用独立 ${draft.kind} ID。`)
      let content = draft.content
      if (draft.kind === 'node') {
        if (typeof value !== 'object' || value === null || Array.isArray(value)) throw new Error('node 内容 JSON 必须是对象。')
        for (const field of ['node_id', 'zone', 'report_policy', 'auto_close']) {
          if (Object.hasOwn(value, field)) throw new Error(`请移除 node 内容 JSON 中的顶层 ${field}；使用独立控件。`)
        }
        content = JSON.stringify({ ...value, ...draft.settings })
      }
      const payload = { id: draft.id, content, revision: draft.revision }
      if (draft.kind === 'Bot') {
        const saved = await saveBot(payload)
        if (!valid()) return
        await refresh(false, true)
        if (!valid()) return
        const catalog = current.current.bots
        update({
          draft: null,
          botId: saved.id,
          bots: {
            kind: 'ready',
            bots: [...(catalog.kind === 'ready' ? catalog.bots.filter(bot => bot.id !== saved.id) : []), saved],
            ...(catalog.kind === 'ready' && catalog.problems
              ? { problems: catalog.problems.filter(problem => problem.bot_id !== saved.id) } : {}),
          },
        })
      } else {
        const map = await saveNode(draft.file!, { ...payload, create: draft.create })
        if (!valid()) return
        update({ draft: null, selectedId: draft.id, returnProblem: null })
        applyMap(map)
      }
    })
  }
  async function removeSelectedNode() {
    const { file, selectedId, draft } = current.current
    if (!file || !selectedId || draft) return
    await operation(async valid => {
      const target = await checkNodeDeletion(file, selectedId)
      if (!valid() || !confirm(`确认删除 node ${target.id}？仅删除当前 Map 中此 node，不可撤销。`)) return
      const map = await deleteNode(file, target)
      if (valid()) { update({ selectedId: null }); applyMap(map) }
    })
  }
  async function removeBot() {
    const id = current.current.botId
    if (id === null || current.current.draft) return
    await operation(async valid => {
      const target = await checkBotDelete(id)
      if (!valid() || !confirm(`确认删除共享 Bot ${target.id}？此操作无法撤销。`)) return
      const catalog = await deleteBot(target)
      if (valid()) update({ bots: { kind: 'ready', ...catalog }, botId: null })
    })
  }
  function requestMapDeletion() {
    if (!abandonDraft() || !current.current.file) return
    update({ deleteMapTarget: current.current.file, deleteMapError: null })
  }
  function closeMapDeletion() { if (!inFlight.current) update({ deleteMapTarget: null, deleteMapError: null }) }
  async function removeMap(deleteHistory: boolean) {
    const file = current.current.deleteMapTarget
    if (file === null) return
    await operation(async valid => {
      const catalog = await deleteMap(file, deleteHistory)
      if (valid()) update({ listing: { kind: 'ready', ...catalog }, file: null, preview: null, selectedId: null, returnProblem: null, deleteMapTarget: null })
    }, true)
  }
  async function changeGroup(kind: 'bots' | 'maps', id: string, group: string | null) {
    if (inFlight.current) throw new Error('请等待当前保存完成。')
    await operation(async valid => {
      const revision = await (kind === 'bots' ? updateBotGroup(id, group) : updateMapGroup(id, group))
      if (!valid()) return
      const { bots, listing, preview, file, draft } = current.current
      if (draft && groupOnlyRevision(draft.revision, revision) && (kind === 'bots' ? draft.kind === 'Bot' && draft.initialId === id : draft.kind === 'node' && draft.file === id)) update({ draft: { ...draft, revision } })
      window.dispatchEvent(new CustomEvent('herdr-group-revision', { detail: { kind, id, revision } }))
      if (kind === 'bots' && bots.kind === 'ready') update({ bots: { ...bots, bots: bots.bots.map(bot => bot.id === id ? { ...bot, group } : bot) } })
      if (kind === 'maps') {
        if (listing.kind === 'ready') { const groups = { ...listing.groups }; if (group === null) delete groups[id]; else groups[id] = group; update({ listing: { ...listing, groups } }) }
        if (file === id && preview?.kind === 'ready') update({ preview: { ...preview, map: { ...preview.map, group } } })
      }
    })
    if (current.current.editError) throw new Error(current.current.editError)
  }
  function refreshAll() { if (abandonDraft()) void refresh() }
  async function returnToMap() {
    if (!abandonDraft()) return
    if (view === 'bot') onViewChange('map')
    else await refresh(false)
  }
  useEffect(() => {
    mounted.current = true
    return () => { mounted.current = false; lifetime.current++; invalidate(); inFlight.current = false }
  }, [])
  useEffect(() => {
    lifetime.current++
    inFlight.current = false
    if (active) { update({ editingBusy: false, deletingMap: false }); void refresh() }
    else invalidate()
    return () => { lifetime.current++; invalidate(); inFlight.current = false }
  }, [active])
  useEffect(() => {
    const returning = previousView.current === 'bot' && view === 'map'
    previousView.current = view
    if (active && returning && !inFlight.current) void refresh(false)
  }, [active, view])
  const draft = state.draft
  const dirty = draft !== null && draftChanged(draft)
  useEffect(() => {
    if (!dirty && !state.editingBusy) return
    const beforeUnload = (event: BeforeUnloadEvent) => { event.preventDefault(); event.returnValue = '' }
    window.addEventListener('beforeunload', beforeUnload)
    return () => window.removeEventListener('beforeunload', beforeUnload)
  }, [dirty, state.editingBusy])
  return { ...state, editingBusy: state.editingBusy || state.listing.kind === 'loading' || state.bots.kind === 'loading' || state.preview?.kind === 'loading', changeGroup, abandonDraft, discardForNavigation, navigateDefinition, selectFile, selectNode, openBot, beginEdit, editDraft, submitDefinition, removeSelectedNode, removeBot, requestMapDeletion, closeMapDeletion, removeMap, refreshAll, returnToMap }
}
