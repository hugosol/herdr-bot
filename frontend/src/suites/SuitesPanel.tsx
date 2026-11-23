import { useEffect, useState, type FormEvent } from 'react'
import { suiteRequest, type OmpSkillSource, type Suite, type SuiteList } from '../controllers/suiteController'
import './suites.css'
import { NierTitleText } from '../app/NierTitleText'

type Harness = Suite['harness']
type Resources = { skills: string; extensions: string; mcp: string[] }
type Draft = {
  id: string; name: string; harness: Harness
  omp: Resources & { skillCollections: string; lsp: string; dap: string[] }
  pi: Resources & { prompts: string; codemode: 'on' | 'only' }
}
const harnessLabels = { omp: 'OMP', pi: 'Pi' } as const
const emptyResources: Resources = { skills: '', extensions: '', mcp: [] }
const emptyDraft: Draft = {
  id: '', name: '', harness: 'omp',
  omp: { ...emptyResources, skillCollections: '', lsp: '{}', dap: [] },
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
      lsp: JSON.stringify(suite.lsp, null, 2), dap: suite.dap,
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

export function SuitesPanel({ active }: { active: boolean }) {
  const [data, setData] = useState<SuiteList | null>(null)
  const [draft, setDraft] = useState<Draft | null>(null)
  const [editing, setEditing] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [notice, setNotice] = useState('')
  const missingDap = data && draft?.harness === 'omp'
    ? draft.omp.dap.filter(id => !data.dap.some(adapter => adapter.id === id))
    : []

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
    setDraft(suite ? draftFrom(suite) : emptyDraft); setEditing(suite?.id ?? null); setError(''); setNotice('')
  }
  async function reload() {
    const saved = data?.suites.find(suite => suite.id === editing)
    const dirty = draft !== null && (!saved || JSON.stringify(draft) !== JSON.stringify(draftFrom(saved)))
    if (dirty && !window.confirm('重新读取会丢弃未保存的编辑，继续？')) return
    setBusy(true); setError(''); setNotice('')
    try {
      const result = await suiteRequest()
      setData(result)
      const selected = result.suites.find(suite => suite.id === editing)
      setDraft(selected ? draftFrom(selected) : null)
      if (!selected) setEditing(null)
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
        if (missingDap.length > 0) throw new Error('请先移除失效的 DAP 引用。')
        const lsp: unknown = JSON.parse(draft.omp.lsp)
        if (lsp === null || typeof lsp !== 'object' || Array.isArray(lsp)) throw new Error('LSP 声明必须是 JSON 对象。')
        const previous = data?.suites.find(saved => saved.id === editing)
        suite = { ...common, harness, skills: ompSkillSources(draft.omp, previous?.harness === 'omp' ? previous.skills : []),
          lsp: lsp as Record<string, unknown>, dap: draft.omp.dap }
      } else {
        suite = { ...common, harness, skills: [...new Set(lines(draft.pi.skills))],
          prompts: lines(draft.pi.prompts), codemode: draft.pi.codemode }
      }
    } catch (error) { setError(message(error)); return }
    setBusy(true)
    try {
      const result = await suiteRequest(editing ? 'PUT' : 'POST', editing ?? undefined, suite)
      setData(result); setEditing(suite.id); setDraft(draftFrom(result.suites.find(saved => saved.id === suite.id)!)); setNotice('Suite 已保存到权威文件。')
    } catch (error) { setError(message(error)) }
    finally { setBusy(false) }
  }
  async function remove() {
    if (!editing || !window.confirm(`删除 Suite「${draft?.name}」？`)) return
    setBusy(true); setError(''); setNotice('')
    try {
      setData(await suiteRequest('DELETE', editing)); setDraft(null); setEditing(null); setNotice('Suite 已删除。')
    } catch (error) { setError(message(error)) }
    finally { setBusy(false) }
  }

  function changeMcp(id: string, checked: boolean) {
    setDraft(current => {
      if (!current) return current
      const { harness } = current
      const resources = current[harness]
      const mcp = checked ? [...resources.mcp, id] : resources.mcp.filter(value => value !== id)
      return { ...current, [harness]: { ...resources, mcp } }
    })
  }

  function changeDap(id: string, checked: boolean) {
    setDraft(current => {
      if (current?.harness !== 'omp') return current
      const dap = checked ? [...current.omp.dap, id] : current.omp.dap.filter(value => value !== id)
      return { ...current, omp: { ...current.omp, dap } }
    })
  }

  const harness = draft?.harness ?? 'omp'

  return <section className="suites-panel" aria-label="Suite 管理" data-editing={draft !== null}>
    <header className="suites-toolbar suites-page-header">
      <div><h1><NierTitleText>Suite</NierTitleText></h1><p className="suites-subtitle">场景工具集：每个 Suite 为 OMP 或 Pi 配置可复用的场景资源。</p></div>
      <button type="button" disabled={busy} onClick={() => open()}>创建 Suite</button>
      <button type="button" disabled={busy} onClick={() => void reload()}>重新读取 Suite</button>
    </header>
    <p className="scope-note">界面与手工维护使用同一份 Suite 权威文件。资源路径按声明文件所在目录或集中 roots 解析；这里不编辑 MCP 连接，也不表示资源已经运行。</p>
    {error && <p className="zone-error" role="alert">{error}</p>}
    {notice && <p role="status">{notice}</p>}
    <section className="suites-fixed" aria-label="全局固定扩展" aria-describedby="suite-fixed-note">
      <h3>全局固定扩展 · 只读</h3>
      <p className="scope-note" id="suite-fixed-note">固定扩展按宿主和执行角色加载，不受 Suite 选择影响；未选 Suite 的 Task 也会加载。这里展示已配置的入口与适用角色，不表示已连接或正在运行。</p>
      {data === null ? <p>{error ? '未能读取固定扩展，请重新读取。' : '正在读取固定扩展…'}</p> : <div className="suites-fixed-groups">
        {(['omp', 'pi'] as const).map(agent => <section key={agent} aria-label={`${agent.toUpperCase()} 固定扩展`}>
          <h4>{agent.toUpperCase()}</h4>
          {data.fixedExtensions.some(extension => extension.agent === agent) ? <ul>
            {data.fixedExtensions.filter(extension => extension.agent === agent).map(extension => <li key={extension.id}>
              <span className="suites-fixed-name">{extension.label}{extension.label !== extension.id && <> · <span className="suites-resource-id">{extension.id}</span></>}</span>
              <span className="suites-extension-path">{extension.entrypoint}</span>
              <span className="suites-fixed-roles">适用：{extension.roles.map(role => ({ standalone: '普通独立Chat', manager: '管理者', node: 'workflow节点', chief: '需求分流者', 'chief-delegated': 'chief委派独立Chat' })[role]).join('、') || '无适用角色'}</span>
            </li>)}
          </ul> : <p className="scope-note">此宿主暂无固定扩展。</p>}
        </section>)}
      </div>}
    </section>
    <div className="suites-layout">
      <section aria-label="Suite 列表" className="suites-list">
        <div className="suites-list-heading"><h3>工具集</h3><span>{data ? `${data.suites.length} 个 Suite` : '读取中'}</span></div>
        {data === null ? <p>{error ? '未能读取 Suite，请重新读取。' : '正在读取 Suite…'}</p> : data.suites.length === 0 ? <p>尚未定义 Suite</p> : data.suites.map(suite =>
          <button key={suite.id} type="button" disabled={busy} aria-label={`${suite.name} · ${suite.id}`} aria-pressed={editing === suite.id} onClick={() => open(suite)}>
            <span className="suites-list-name">{suite.name}</span><span className="suites-list-id">{suite.id}</span>
            <span className="suites-list-meta">{harnessLabels[suite.harness]}<span>{suite.skills.length} 技能来源 · {suite.mcp.length} MCP</span></span>
          </button>)}
      </section>
      {!draft && <section className="suites-empty" aria-label="编辑指引"><h3>选择一个场景工具集</h3><p>查看和编辑技能、宿主扩展、提示词模板、语言服务及调试器配置。</p><dl><div><dt>OMP</dt><dd>技能、扩展、LSP、集中 DAP 与 MCP</dd></div><div><dt>Pi</dt><dd>技能、扩展、提示词模板与集中 MCP</dd></div></dl><p className="scope-note">选择左侧工具集，或创建 Suite。保存的是资源声明，不代表资源已经运行。</p></section>}
      {draft && <form className="suites-editor" onSubmit={event => void save(event)} aria-label="Suite 编辑">
        <fieldset disabled={busy}>
          <legend>{editing ? '查看 / 编辑 Suite' : '创建 Suite'}</legend>
          <div className="suites-toolbar suites-editor-actions">
            <span>{editing ? draft.name : '新工具集'}</span>
            <button type="submit" className="suites-primary" disabled={missingDap.length > 0} aria-describedby={missingDap.length > 0 ? 'suite-dap-missing' : undefined}>保存 Suite</button>
            <button type="button" onClick={() => { setDraft(null); setEditing(null) }}>关闭编辑</button>
            {missingDap.length > 0 && <p className="suites-dap-missing zone-error" id="suite-dap-missing">请先移除失效的 DAP 引用。</p>}
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
          <p className="scope-note" id="suite-harness-note">ID 只能包含英文字母、数字、连字符和下划线，且须全局唯一，不要求宿主后缀。ID 和宿主在保存后不可更改。名称以「· {harnessLabels[harness]}」结尾，保存时自动添加名称后缀。</p>
          {harness === 'pi' && <fieldset><legend>Pi 配置</legend>
            <label className="suites-choice"><input type="checkbox" checked={draft.pi.codemode === 'only'} onChange={event => setDraft({ ...draft, pi: { ...draft.pi, codemode: event.target.checked ? 'only' : 'on' } })} />Codemode only</label>
            <p className="scope-note">关闭 only 表示明确的 on，不禁用，也不继承全局模式。</p>
          </fieldset>}
          </section>
          <section className="suites-section" aria-label="资源路径"><h3>资源路径</h3>
          <div className="suites-field-grid">
          {harness === 'omp' ? <>
            <div className="suites-resource-block">
              <label htmlFor="suite-omp-collections">OMP 技能集合目录（每行一个路径）</label>
              <textarea id="suite-omp-collections" className="zone-form__input" rows={3} aria-describedby="suite-omp-collections-note suite-omp-skills-note" value={draft.omp.skillCollections} onChange={event => setDraft({ ...draft, omp: { ...draft.omp, skillCollections: event.target.value } })} />
              <p className="scope-note" id="suite-omp-collections-note">每次启动选中直接子目录中含 SKILL.md 的技能，不递归，也不加载集合目录自身。增删集合成员无需编辑 Suite。</p>
            </div>
            <div className="suites-resource-block">
              <label htmlFor="suite-omp-skills">OMP 单技能目录（每行一个路径）</label>
              <textarea id="suite-omp-skills" className="zone-form__input" rows={3} aria-describedby="suite-omp-single-note suite-omp-skills-note" value={draft.omp.skills} onChange={event => setDraft({ ...draft, omp: { ...draft.omp, skills: event.target.value } })} />
              <p className="scope-note" id="suite-omp-single-note">填写直接含 SKILL.md 的目录。不会自动选中同级目录中的技能。</p>
            </div>
          </> : <div className="suites-resource-block">
            <label htmlFor="suite-skills">Pi 单技能目录（每行一个路径）</label>
            <textarea id="suite-skills" className="zone-form__input" rows={3} aria-describedby="suite-skills-note" value={draft.pi.skills} onChange={event => setDraft({ ...draft, pi: { ...draft.pi, skills: event.target.value } })} />
            <p className="scope-note" id="suite-skills-note">每行填写一个直接含 SKILL.md 的单技能根目录，不填写技能集合父目录。仅加载指定技能；留空表示不加载技能。路径按 Suite 文件或集中 roots 解析，直接加载真实目录，不复制或创建链接。路径缺失或不可读会报错，不回退到默认来源；不同目录的技能同名会列出路径并报错。</p>
          </div>}
          <div className="suites-extensions suites-resource-block">
            <label htmlFor="suite-extensions">{harnessLabels[harness]} Extensions（每行一个具体路径）</label>
            <textarea id="suite-extensions" className="zone-form__input" rows={3} aria-describedby="suite-extensions-note" value={draft[harness].extensions} onChange={event => setDraft({ ...draft, [harness]: { ...draft[harness], extensions: event.target.value } })} />
            <p className="scope-note" id="suite-extensions-note">{harness === 'pi' ? 'Pi 填写具体源入口文件。' : 'OMP 填写源入口文件或原生扩展目录。'}每行一个路径，按 Suite 文件或集中 roots 解析；直接从源码目录加载，不复制或链接，不表示已经运行。</p>
          </div>
          {harness === 'pi' && <div className="suites-resource-block">
            <label htmlFor="suite-prompts">Prompt templates（每行一个路径）</label>
            <textarea id="suite-prompts" className="zone-form__input" rows={3} value={draft.pi.prompts} onChange={event => setDraft({ ...draft, pi: { ...draft.pi, prompts: event.target.value } })} />
          </div>}
          </div>
          {harness === 'omp' && <p className="scope-note" id="suite-omp-skills-note">两处填写的来源共同生成技能白名单，排除未选中的默认、托管或插件技能。原生发现仍可能扫描默认目录。两个字段都留空表示禁用技能。路径按 Suite 文件或集中 roots 解析，直接加载真实目录，不复制或创建链接。路径缺失或不可读会报错，不回退到默认来源；所选技能来自不同目录且同名时，会列出路径并报错。</p>}
          <p className="scope-note">未选择 Suite 时保留宿主原生技能行为。</p>
          </section>
          {harness === 'omp' && <section className="suites-section" aria-label="语言服务"><h3>语言服务</h3>
          <label htmlFor="suite-lsp">LSP 声明（JSON）</label>
          <p className="scope-note">LSP 声明不表示已启用或正在运行。OMP 按项目标记和可执行程序原生发现；缺少程序时不会启动，不会自动安装或修改 PATH。</p>
          <p className="scope-note">保留 OMP 默认、用户及项目配置的原生优先级；Suite 不是语言服务白名单。</p>
          <textarea id="suite-lsp" className="zone-form__input" rows={6} value={draft.omp.lsp} onChange={event => setDraft({ ...draft, omp: { ...draft.omp, lsp: event.target.value } })} />
          </section>}
          {harness === 'omp' && <fieldset className="suites-section"><legend>集中 DAP 选择</legend>
            <p className="scope-note">仅引用集中 DAP 配置，不表示已安装或通过调试验证，也不限制 OMP 原生调试器。</p>
            {data?.dap.length === 0 && <p>集中目录没有 DAP 定义。</p>}
            {data?.dap.map(({ id, name, description, config }) => {
              const { command, languages, ...otherConfig } = config
              return <div key={id} className="suites-dap-row">
                <label className="suites-choice"><input type="checkbox" checked={draft.omp.dap.includes(id)} onChange={event => changeDap(id, event.target.checked)} />{name}</label>
                <details>
                  <summary aria-label={`${name} 详情`}>详情</summary>
                  <div className="suites-dap-detail">
                    <p>{description}</p>
                    <dl>
                      <dt>声明语言</dt><dd>{languages?.join('、') || '未声明'}</dd>
                      <dt>命令</dt><dd><code className="suites-extension-path">{command}</code></dd>
                      {Object.keys(otherConfig).length > 0 && <>
                        <dt>其他原生配置（JSON）</dt><dd><pre className="suites-extension-path">{JSON.stringify(otherConfig, null, 2)}</pre></dd>
                      </>}
                    </dl>
                  </div>
                </details>
              </div>
            })}
            {missingDap.map(id => <label key={id} className="suites-choice zone-error">
              <input type="checkbox" checked onChange={() => changeDap(id, false)} />{id}（失效：集中 DAP 定义不存在）
            </label>)}
          </fieldset>}
          <fieldset className="suites-section"><legend>集中 MCP 选择</legend>
            <p className="scope-note">仅引用集中目录；空清单表示不追加，不关闭宿主原生 MCP。</p>
            {data?.mcp.length === 0 && <p>集中目录没有 MCP 定义。</p>}
            {data?.mcp.map(id => <label key={id} className="suites-choice"><input type="checkbox" checked={draft[harness].mcp.includes(id)} onChange={event => changeMcp(id, event.target.checked)} />{id}</label>)}
            {data && draft[harness].mcp.filter(id => !data.mcp.includes(id)).map(id => <label key={id} className="suites-choice zone-error">
              <input type="checkbox" checked onChange={() => changeMcp(id, false)} />{id}（失效：集中 MCP 定义不存在）
            </label>)}
          </fieldset>
          {editing && <div className="suites-danger"><p>删除此工具集前需要确认；被引用的 Suite 仍受服务端保护。</p><button type="button" onClick={() => void remove()}>删除 Suite</button></div>}
        </fieldset>
      </form>}
    </div>
  </section>
}
