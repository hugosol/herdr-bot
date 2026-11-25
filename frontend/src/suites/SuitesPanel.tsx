import { useEffect, useRef, useState, type FormEvent } from 'react'
import { suiteRequest, type OmpSkillSource, type Suite, type SuiteList } from '../controllers/suiteController'
import './suites.css'
import { ResourceCatalog, ResourceChoices } from './SuiteResources'
import type { ResourceKind } from '../controllers/suiteController'

type Harness = Suite['harness']
type Resources = { skills: string; extensions: string; mcp: string[] }
type Draft = {
  id: string; name: string; harness: Harness
  omp: Resources & { skillCollections: string; lsp: string[]; dap: string[] }
  pi: Resources & { prompts: string; codemode: 'on' | 'only' }
}
const harnessLabels = { omp: 'OMP', pi: 'Pi' } as const
const emptyResources: Resources = { skills: '', extensions: '', mcp: [] }
const emptyDraft: Draft = {
  id: '', name: '', harness: 'omp',
  omp: { ...emptyResources, skillCollections: '', lsp: [], dap: [] },
  pi: { ...emptyResources, prompts: '', codemode: 'on' },
}
function draftFrom(suite: Suite): Draft {
  const { id, name, harness } = suite
  const resources = { extensions: suite.extensions.join('\n'), mcp: suite.mcp }
  if (suite.harness === 'omp') {
    const paths: Record<OmpSkillSource['kind'], string[]> = { collection: [], skill: [] }
    for (const { kind, path } of suite.skills) paths[kind].push(path)
    return { ...emptyDraft, id, name, harness, omp: {
      ...resources, skills: paths.skill.join('\n'), skillCollections: paths.collection.join('\n'),
      lsp: suite.lsp, dap: suite.dap,
    } }
  }
  return { ...emptyDraft, id, name, harness, pi: {
    ...resources, skills: suite.skills.join('\n'), prompts: suite.prompts.join('\n'), codemode: suite.codemode,
  } }
}
const lines = (text: string) => text.split(/\r?\n/).map(line => line.trim()).filter(Boolean)

function ompSkillSources(draft: Draft['omp'], previous: OmpSkillSource[]): OmpSkillSource[] {
  const paths = {
    collection: new Set(lines(draft.skillCollections)).values(),
    skill: new Set(lines(draft.skills)).values(),
  }
  const skills: OmpSkillSource[] = []
  // The separate inputs do not display the saved order between kinds.
  for (const { kind } of previous) {
    const next = paths[kind].next()
    if (!next.done) skills.push({ kind, path: next.value })
  }
  for (const kind of ['collection', 'skill'] as const) {
    for (const path of paths[kind]) skills.push({ kind, path })
  }
  return skills
}
const message = (error: unknown) => error instanceof Error ? error.message : '无法连接 Suite 服务，未确认保存。'

type Scope = Harness | 'all'
const hosts = ['omp', 'pi'] as const

function SuiteCatalog({ data, scope, setScope, editing, busy, error, open }: {
  data: SuiteList | null; scope: Scope; setScope: (scope: Scope) => void
  editing: string | null; busy: boolean; error: string; open: (suite: Suite) => void
}) {
  return <section aria-label="Suite 列表" className="suites-list">
    <div className="suites-filters" role="group" aria-label="按宿主筛选">
      {(['all', ...hosts] as const).map(host => <button type="button" key={host} aria-pressed={scope === host} onClick={() => setScope(host)}>
        {host === 'all' ? '全部' : harnessLabels[host]}
      </button>)}
    </div>
    {!data ? <p>{error ? '未能读取 Suite，请重新读取。' : '正在读取 Suite…'}</p> : <div className="suites-groups">
      {hosts.filter(host => scope === 'all' || scope === host).map(host => {
        const suites = data.suites.filter(suite => suite.harness === host)
        return <section className="suites-group" key={host} aria-label={`${harnessLabels[host]} 工具集`}>
          <header><h3>{harnessLabels[host]} 工具集</h3><span>{suites.length} 个 Suite</span></header>
          {suites.length === 0 ? <p>尚未定义 Suite</p> : suites.map(suite => <button className="suites-row" key={suite.id} type="button" disabled={busy}
            aria-label={`${suite.name} · ${suite.id}`} aria-pressed={editing === suite.id} onClick={() => open(suite)}>
            <span className="suites-row-identity"><span className="suites-list-name">{suite.name}</span><span className="suites-list-id">{suite.id}</span></span>
            <span className="suites-list-meta">{suite.skills.length} 技能来源 · {suite.mcp.length} MCP</span>
          </button>)}
        </section>
      })}
    </div>}
  </section>
}

export function SuitesPanel({ active }: { active: boolean }) {
  const [data, setData] = useState<SuiteList | null>(null)
  const [draft, setDraft] = useState<Draft | null>(null)
  const [editing, setEditing] = useState<string | null>(null)
  const [scope, setScope] = useState<Scope>('all')
  const [view, setView] = useState<'suites' | 'resources'>('suites')
  const resourceGuard = useRef<(() => boolean) | null>(null)
  const [resourceRevision, setResourceRevision] = useState(0)
  const createButton = useRef<HTMLButtonElement>(null)
  const opener = useRef<HTMLElement | null>(null)
  const editorTitle = useRef<HTMLHeadingElement>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [notice, setNotice] = useState('')
  const selected = data?.suites.find(suite => suite.id === editing)
  const dirty = draft !== null && (!selected || JSON.stringify(draft) !== JSON.stringify(draftFrom(selected)))
  const missing = data && draft ? (draft.harness === 'omp' ? ['mcp', 'lsp', 'dap'] as const : ['mcp'] as const)
    .flatMap(kind => {
      const ids = kind === 'mcp' ? draft[draft.harness].mcp : draft.omp[kind]
      return ids.filter(id => !data[kind].some(option => option.id === id)).map(id => `${kind.toUpperCase()} / ${id}`)
    }) : []

  function canLeave() {
    return !busy && resourceGuard.current?.() !== false && (!dirty || window.confirm('放弃未保存的 Suite 修改？'))
  }

  useEffect(() => {
    const leave = (event: BeforeUnloadEvent) => { if (dirty) event.preventDefault() }
    window.addEventListener('beforeunload', leave)
    return () => window.removeEventListener('beforeunload', leave)
  }, [dirty])

  useEffect(() => {
    if (!active) return
    let current = true
    setBusy(true)
    suiteRequest().then(result => { if (current) { setData(result); setError('') } })
      .catch(error => { if (current) setError(message(error)) })
      .finally(() => { if (current) setBusy(false) })
    return () => { current = false }
  }, [active])

  function open(suite?: Suite) {
    if (!canLeave()) return
    opener.current = document.activeElement instanceof HTMLElement ? document.activeElement : null
    setView('suites')
    setDraft(suite ? null : { ...emptyDraft, harness: scope === 'pi' ? 'pi' : 'omp' }); setEditing(suite?.id ?? null); setError(''); setNotice('')
    requestAnimationFrame(() => editorTitle.current?.focus())
  }
  function closeEditor() {
    if (!canLeave()) return
    setDraft(null); setEditing(null)
    requestAnimationFrame(() => {
      const target = opener.current
      const button = target?.getClientRects().length ? target : createButton.current
      button?.focus()
    })
  }
  async function reload() {
    if (!canLeave()) return
    setBusy(true); setError(''); setNotice('')
    try {
      const result = await suiteRequest()
      setData(result)
      const selected = result.suites.find(suite => suite.id === editing)
      setDraft(null)
      if (!selected) setEditing(null)
      setResourceRevision(value => value + 1)
      setNotice('已从权威文件重新读取。')
    } catch (error) { setError(message(error)) }
    finally { setBusy(false) }
  }
  async function save(event: FormEvent) {
    event.preventDefault()
    if (!draft) return
    setError(''); setNotice('')
    let suite: Suite
    try {
      const { harness } = draft
      const id = draft.id.trim()
      const nameBase = draft.name.trim().replace(/\s*·\s*(?:omp|pi)$/i, '').trimEnd()
      if (!id || !nameBase) throw new Error('请填写非空的 Suite ID 和名称。')
      const { extensions, mcp } = draft[harness]
      const common = {
        id: editing ?? id,
        name: `${nameBase} · ${harnessLabels[harness]}`,
        extensions: [...new Set(lines(extensions))], mcp,
      }
      if (harness === 'omp') {
        if (missing.length > 0) throw new Error('请先移除失效的组件引用。')
        const previous = data?.suites.find(saved => saved.id === editing)
        suite = { ...common, harness, skills: ompSkillSources(draft.omp, previous?.harness === 'omp' ? previous.skills : []),
          lsp: draft.omp.lsp, dap: draft.omp.dap }
      } else {
        suite = { ...common, harness, skills: [...new Set(lines(draft.pi.skills))],
          prompts: lines(draft.pi.prompts), codemode: draft.pi.codemode }
      }
    } catch (error) { setError(message(error)); return }
    setBusy(true)
    try {
      const result = await suiteRequest(editing ? 'PUT' : 'POST', editing ?? undefined, suite)
      if (scope !== 'all') setScope(suite.harness)
      setData(result); setEditing(suite.id); setDraft(null); setNotice('Suite 已保存到权威文件。')
    } catch (error) { setError(message(error)) }
    finally { setBusy(false) }
  }
  async function remove() {
    if (!editing || !window.confirm(`删除 Suite「${selected?.name}」？`)) return
    setBusy(true); setError(''); setNotice('')
    try {
      setData(await suiteRequest('DELETE', editing)); setDraft(null); setEditing(null); setNotice('Suite 已删除。')
    } catch (error) { setError(message(error)) }
    finally { setBusy(false) }
  }

  function changeResource(kind: ResourceKind, id: string, checked: boolean) {
    setDraft(current => {
      if (!current) return current
      const host = current.harness
      if (kind !== 'mcp' && host !== 'omp') return current
      const resources = current[host]
      const selectedIds = kind === 'mcp' ? resources.mcp : current.omp[kind]
      const next = checked ? [...selectedIds, id] : selectedIds.filter(value => value !== id)
      return { ...current, [host]: { ...resources, [kind]: next } }
    })
  }

  const harness = draft?.harness ?? 'omp'

  return <section className="suites-panel" aria-label="Suite 管理" data-detail={draft !== null || selected !== undefined}>
    <header className="suites-toolbar suites-page-header">
      <div><h1>Suite</h1><p className="suites-subtitle">按宿主组合技能与工具。</p></div>
      <button ref={createButton} className={draft || selected || view === 'resources' ? undefined : "suites-primary"} type="button" disabled={busy} onClick={() => open()}>创建 Suite</button>
      <button type="button" disabled={busy} onClick={() => void reload()}>重新读取 Suite</button>
    </header>
    {error && <p className="zone-error" role="alert">{error}</p>}
    {notice && <p role="status">{notice}</p>}
    <div className="suites-view-switch" role="group" aria-label="Suite 内容">
      <button type="button" aria-pressed={view === 'suites'} onClick={() => { if (view !== 'suites' && canLeave()) { setView('suites'); setError(''); setNotice('') } }}>工具集</button>
      <button type="button" aria-pressed={view === 'resources'} onClick={() => { if (view !== 'resources' && canLeave()) { setDraft(null); setView('resources'); setError(''); setNotice('') } }}>组件配置</button>
    </div>
    {view === 'resources' ? data ? <ResourceCatalog key={resourceRevision} data={data} onSaved={setData} guard={resourceGuard} /> : <p>正在读取组件配置…</p> : <>
    <div className="suites-layout">
      <SuiteCatalog data={data} scope={scope} setScope={setScope} editing={editing} busy={busy} error={error} open={open} />
      {selected && !draft && <section className="suites-overview" aria-label="Suite 概览">
        <header className="suites-toolbar suites-editor-actions">
          <div><h2 ref={editorTitle} tabIndex={-1}>{selected.name}</h2><code>{selected.id}</code></div>
          <button className="suites-primary" type="button" disabled={busy} onClick={() => { setDraft(draftFrom(selected)); setError(''); setNotice(''); requestAnimationFrame(() => editorTitle.current?.focus()) }}>编辑 Suite</button>
          <button type="button" onClick={closeEditor}>返回 Suite 列表</button>
        </header>
        <div className="suites-overview-facts">
          <span>{harnessLabels[selected.harness]} 宿主</span><span>{selected.skills.length} 技能来源</span><span>{selected.extensions.length} 扩展</span>
          {selected.harness === 'pi' && <span>Codemode {selected.codemode}</span>}
        </div>
        <div className="suites-resource-grid">
          <ResourceChoices kind="mcp" options={data!.mcp} selected={selected.mcp} />
          {selected.harness === 'omp' && <>
            <ResourceChoices kind="lsp" options={data!.lsp} selected={selected.lsp} />
            <ResourceChoices kind="dap" options={data!.dap} selected={selected.dap} />
          </>}
        </div>
        <details className="suites-path-summary"><summary>技能与扩展路径</summary>
          <dl><dt>技能来源</dt><dd>{selected.skills.length ? <ul>{selected.skills.map((source, index) => <li key={index}>{typeof source === 'string' ? source : `${source.kind === 'collection' ? '集合' : '单技能'} · ${source.path}`}</li>)}</ul> : '未选择'}</dd>
            <dt>扩展</dt><dd>{selected.extensions.length ? <ul>{selected.extensions.map(path => <li key={path}>{path}</li>)}</ul> : '未选择'}</dd>
            {selected.harness === 'pi' && <><dt>Prompt templates</dt><dd>{selected.prompts.length ? <ul>{selected.prompts.map(path => <li key={path}>{path}</li>)}</ul> : '未选择'}</dd></>}
          </dl>
        </details>
      </section>}
      {draft && <form className="suites-editor" onSubmit={event => void save(event)} aria-label="Suite 编辑">
        <fieldset disabled={busy}>
          <legend>{editing ? '查看 / 编辑 Suite' : '创建 Suite'}</legend>
          <div className="suites-toolbar suites-editor-actions">
            <h2 ref={editorTitle} tabIndex={-1}>{editing ? draft.name : '新工具集'}</h2>
            <button type="submit" className="suites-primary" disabled={missing.length > 0} aria-describedby={missing.length > 0 ? 'suite-resource-missing' : undefined}>保存 Suite</button>
            <button type="button" onClick={() => { if (canLeave()) { setDraft(null); setError('') } }}>取消编辑</button>
            <button type="button" onClick={closeEditor}>返回 Suite 列表</button>
            {missing.length > 0 && <p className="suites-dap-missing zone-error" id="suite-resource-missing">请先移除失效的组件引用：{missing.join('、')}。</p>}
          </div>
          <section className="suites-section" aria-label="基本信息"><h3>基本信息</h3>
          <div className="suites-field-grid">
          <label>Suite ID<input className="zone-form__input" value={draft.id} disabled={editing !== null} required aria-describedby="suite-harness-note" onChange={event => setDraft({ ...draft, id: event.target.value })} /></label>
          <label>Suite 名称<input className="zone-form__input" value={draft.name} required aria-describedby="suite-harness-note" onChange={event => setDraft({ ...draft, name: event.target.value })} /></label>
          <div>
            <label htmlFor="suite-harness">适用宿主</label>
            <select id="suite-harness" className="zone-form__input" value={harness} disabled={editing !== null} aria-describedby="suite-harness-note" onChange={event => {
              const selected = event.target.value
              if (editing === null && (selected === 'omp' || selected === 'pi')) setDraft({ ...draft, harness: selected })
            }}><option value="omp">OMP</option><option value="pi">Pi</option></select>
          </div>
          </div>
          <details className="suites-form-help"><summary>说明</summary><p className="scope-note" id="suite-harness-note">ID 只能包含英文字母、数字、连字符和下划线，且须全局唯一，不要求宿主后缀。ID 和宿主在保存后不可更改。名称以「· {harnessLabels[harness]}」结尾，保存时自动添加名称后缀。</p></details>
          {harness === 'pi' && <fieldset><legend>Pi 配置</legend>
            <label className="suites-choice"><input type="checkbox" checked={draft.pi.codemode === 'only'} onChange={event => setDraft({ ...draft, pi: { ...draft.pi, codemode: event.target.checked ? 'only' : 'on' } })} />Codemode only</label>
            <details className="suites-form-help"><summary>说明</summary><p className="scope-note">关闭 only 表示明确的 on，不禁用，也不继承全局模式。</p></details>
          </fieldset>}
          </section>
          <section className="suites-section" aria-label="资源路径"><h3>资源路径</h3>
          <div className="suites-field-grid">
          {harness === 'omp' ? <>
            <div className="suites-resource-block">
              <label htmlFor="suite-omp-collections">OMP 技能集合目录（每行一个路径）</label>
              <textarea id="suite-omp-collections" className="zone-form__input" rows={3} aria-describedby="suite-omp-collections-note suite-omp-skills-note" value={draft.omp.skillCollections} onChange={event => setDraft({ ...draft, omp: { ...draft.omp, skillCollections: event.target.value } })} />
              <details className="suites-form-help"><summary>说明</summary><p className="scope-note" id="suite-omp-collections-note">每次启动选中直接子目录中含 SKILL.md 的技能，不递归，也不加载集合目录自身。增删集合成员无需编辑 Suite。</p></details>
            </div>
            <div className="suites-resource-block">
              <label htmlFor="suite-omp-skills">OMP 单技能目录（每行一个路径）</label>
              <textarea id="suite-omp-skills" className="zone-form__input" rows={3} aria-describedby="suite-omp-single-note suite-omp-skills-note" value={draft.omp.skills} onChange={event => setDraft({ ...draft, omp: { ...draft.omp, skills: event.target.value } })} />
              <details className="suites-form-help"><summary>说明</summary><p className="scope-note" id="suite-omp-single-note">填写直接含 SKILL.md 的目录。不会自动选中同级目录中的技能。</p></details>
            </div>
          </> : <div className="suites-resource-block">
            <label htmlFor="suite-skills">Pi 单技能目录（每行一个路径）</label>
            <textarea id="suite-skills" className="zone-form__input" rows={3} aria-describedby="suite-skills-note" value={draft.pi.skills} onChange={event => setDraft({ ...draft, pi: { ...draft.pi, skills: event.target.value } })} />
            <details className="suites-form-help"><summary>说明</summary><p className="scope-note" id="suite-skills-note">每行填写一个直接含 SKILL.md 的单技能根目录，不填写技能集合父目录。仅加载指定技能；留空表示不加载技能。路径按 Suite 文件或集中 roots 解析，直接加载真实目录，不复制或创建链接。路径缺失或不可读会报错，不回退到默认来源；不同目录的技能同名会列出路径并报错。</p></details>
          </div>}
          <div className="suites-extensions suites-resource-block">
            <label htmlFor="suite-extensions">{harnessLabels[harness]} Extensions（每行一个具体路径）</label>
            <textarea id="suite-extensions" className="zone-form__input" rows={3} aria-describedby="suite-extensions-note" value={draft[harness].extensions} onChange={event => setDraft({ ...draft, [harness]: { ...draft[harness], extensions: event.target.value } })} />
            <details className="suites-form-help"><summary>说明</summary><p className="scope-note" id="suite-extensions-note">{harness === 'pi' ? 'Pi 填写具体源入口文件。' : 'OMP 填写源入口文件或原生扩展目录。'}每行一个路径，按 Suite 文件或集中 roots 解析；直接从源码目录加载，不复制或链接，不表示已经运行。</p></details>
          </div>
          {harness === 'pi' && <div className="suites-resource-block">
            <label htmlFor="suite-prompts">Prompt templates（每行一个路径）</label>
            <textarea id="suite-prompts" className="zone-form__input" rows={3} value={draft.pi.prompts} onChange={event => setDraft({ ...draft, pi: { ...draft.pi, prompts: event.target.value } })} />
          </div>}
          </div>
          {harness === 'omp' && <details className="suites-form-help"><summary>说明</summary><p className="scope-note" id="suite-omp-skills-note">两处填写的来源共同生成技能白名单，排除未选中的默认、托管或插件技能。原生发现仍可能扫描默认目录。两个字段都留空表示禁用技能。路径按 Suite 文件或集中 roots 解析，直接加载真实目录，不复制或创建链接。路径缺失或不可读会报错，不回退到默认来源；所选技能来自不同目录且同名时，会列出路径并报错。</p></details>}
          <details className="suites-form-help"><summary>说明</summary><p className="scope-note">未选择 Suite 时保留宿主原生技能行为。</p></details>
          </section>
          <section className="suites-section" aria-label="组件选择"><h3>组件选择</h3>
            <div className="suites-resource-grid">
              <ResourceChoices kind="mcp" options={data?.mcp ?? []} selected={draft[harness].mcp} onChange={(id, checked) => changeResource('mcp', id, checked)} />
              {harness === 'omp' && <>
                <ResourceChoices kind="lsp" options={data?.lsp ?? []} selected={draft.omp.lsp} onChange={(id, checked) => changeResource('lsp', id, checked)} />
                <ResourceChoices kind="dap" options={data?.dap ?? []} selected={draft.omp.dap} onChange={(id, checked) => changeResource('dap', id, checked)} />
              </>}
            </div>
          </section>
          {editing && <div className="suites-danger"><p>删除此工具集前需要确认；被引用的 Suite 仍受服务端保护。</p><button type="button" onClick={() => void remove()}>删除 Suite</button></div>}
        </fieldset>
      </form>}
    </div>
    <section className="suites-fixed" aria-label="全局固定扩展" aria-describedby="suite-fixed-note">
      <details>
      <summary>全局固定扩展 <span>只读{data && ` · ${data.fixedExtensions.length} 项`}</span></summary>
      <p className="scope-note" id="suite-fixed-note">固定扩展按宿主和执行角色加载，不受 Suite 选择影响；未选 Suite 的 Task 也会加载。这里展示已配置的入口与适用角色，不表示已连接或正在运行。</p>
      {data === null ? <p>{error ? '未能读取固定扩展，请重新读取。' : '正在读取固定扩展…'}</p> : <div className="suites-fixed-groups">
        {hosts.map(agent => <section key={agent} aria-label={`${agent.toUpperCase()} 固定扩展`}>
          <h4>{harnessLabels[agent]}</h4>
          {data.fixedExtensions.some(extension => extension.agent === agent) ? <ul>
            {data.fixedExtensions.filter(extension => extension.agent === agent).map(extension => <li key={extension.id}>
              <span className="suites-fixed-name">{extension.label}{extension.label !== extension.id && <> · <span className="suites-resource-id">{extension.id}</span></>}</span>
              <span className="suites-extension-path">{extension.entrypoint}</span>
              <span className="suites-fixed-roles">适用：{extension.roles.map(role => ({ standalone: '普通独立Chat', manager: '管理者', node: 'workflow节点', chief: '需求分流者', 'chief-delegated': 'chief委派独立Chat' })[role]).join('、') || '无适用角色'}</span>
            </li>)}
          </ul> : <p className="scope-note">此宿主暂无固定扩展。</p>}
        </section>)}
      </div>}
      </details>
    </section>
    </>}
  </section>
}
