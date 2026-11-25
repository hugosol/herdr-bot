import { useEffect, useRef, useState, type FormEvent, type MutableRefObject } from 'react'
import { saveResource, type ResourceKind, type ResourceOption, type SuiteList } from '../controllers/suiteController'

export const resourceKinds = ['mcp', 'lsp', 'dap'] as const
const explanations: Record<ResourceKind, string> = {
  mcp: 'MCP 为 agent 提供外部工具。这里只选择追加的集中配置；空清单不关闭宿主原生 MCP。',
  lsp: 'LSP 提供代码补全、诊断和符号导航。仅适用于 OMP；保留默认、用户和项目配置的原生优先级，不是语言服务白名单。程序缺失时不会自动安装。',
  dap: 'DAP 提供断点、单步和变量查看等调试能力。仅适用于 OMP；选中配置不表示已安装或通过调试验证，也不限制原生调试器。',
}

export function ResourceHelp({ kind }: { kind: ResourceKind }) {
  return <details className="suites-help"><summary aria-label={`${kind.toUpperCase()} 说明`}>说明</summary><p>{explanations[kind]}</p></details>
}

export function ResourceChoices({ kind, options, selected, onChange }: {
  kind: ResourceKind; options: ResourceOption[]; selected: string[]
  onChange?: (id: string, checked: boolean) => void
}) {
  const missing = selected.filter(id => !options.some(option => option.id === id))
  return <fieldset className="suites-resource-selection" aria-label={`集中 ${kind.toUpperCase()} 选择`}>
    <div className="suites-selection-heading"><h3>{kind.toUpperCase()} <span>{selected.length} 已选</span></h3><ResourceHelp kind={kind} /></div>
    <div className="suites-options">
      {options.map(option => <div className="suites-option" key={option.id} data-selected={selected.includes(option.id)}>
        {onChange ? <label className="suites-choice">
          <input type="checkbox" checked={selected.includes(option.id)} onChange={event => onChange(option.id, event.target.checked)} />
          <span>{option.name}</span>
        </label> : <div className="suites-choice suites-choice-readonly"><span>{option.name}</span><span className="suites-selection-state">{selected.includes(option.id) ? '已选' : '未选'}</span></div>}
        <details className="suites-option-detail"><summary aria-label={`${option.name} 详情`}>详情</summary>
          <div><p>{option.description || '暂无用途说明。'}</p><code>{option.id}</code></div>
        </details>
      </div>)}
      {missing.map(id => <div className="suites-option suites-missing" key={id}>
        {onChange ? <label className="suites-choice"><input type="checkbox" checked onChange={() => onChange(id, false)} /><span>{id}（失效）</span></label>
          : <p>{id}（失效：配置不存在）</p>}
      </div>)}
      {options.length === 0 && missing.length === 0 && <p className="suites-empty">暂无 {kind.toUpperCase()} 配置。</p>}
    </div>
  </fieldset>
}

export function ResourceCatalog({ data, onSaved, guard }: {
  data: SuiteList; onSaved: (data: SuiteList) => void; guard: MutableRefObject<(() => boolean) | null>
}) {
  const [kind, setKind] = useState<ResourceKind>('mcp')
  const [editing, setEditing] = useState<ResourceOption | null>(null)
  const [raw, setRaw] = useState('')
  const original = useRef('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [notice, setNotice] = useState('')
  const editor = useRef<HTMLTextAreaElement>(null)
  const opener = useRef<HTMLButtonElement | null>(null)
  const openerId = useRef<string | null>(null)
  const dirty = editing !== null && raw !== original.current

  useEffect(() => {
    guard.current = () => !busy && (!dirty || window.confirm('放弃未保存的配置修改？'))
    const leave = (event: BeforeUnloadEvent) => { if (dirty) event.preventDefault() }
    window.addEventListener('beforeunload', leave)
    return () => { guard.current = null; window.removeEventListener('beforeunload', leave) }
  }, [busy, dirty, guard])

  function close() {
    if (guard.current?.() === false) return
    setEditing(null); setError('')
    requestAnimationFrame(() => opener.current?.focus())
  }

  async function save(event: FormEvent) {
    event.preventDefault()
    if (!editing) return
    setError(''); setNotice('')
    try { JSON.parse(raw) } catch { setError('JSON 格式无效，请修正后保存。'); return }
    setBusy(true)
    try {
      const result = await saveResource(kind, editing.id, raw)
      onSaved(result); setEditing(null); setNotice(`${editing.name} 配置已保存。`)
      requestAnimationFrame(() => opener.current?.focus())
    } catch (cause) { setError(cause instanceof Error ? cause.message : '无法确认配置已保存。') }
    finally { setBusy(false) }
  }

  return <section className="suites-resource-catalog" aria-label="组件配置">
    <div className="suites-filters" role="group" aria-label="配置类别">
      {resourceKinds.map(value => <button type="button" key={value} aria-pressed={kind === value} disabled={busy} onClick={() => {
        if (kind === value || guard.current?.() === false) return
        setKind(value); setEditing(null); setError(''); setNotice('')
      }}>{value.toUpperCase()} 配置</button>)}
    </div>
    {error && <p className="zone-error" role="alert">{error}</p>}
    {notice && <p role="status">{notice}</p>}
    {editing ? <form onSubmit={event => void save(event)} className="suites-json-editor" aria-label={`${editing.name} 配置编辑`}>
      <header className="suites-toolbar"><div><h2>{editing.name}</h2><code>{kind}/{editing.id}.json</code></div></header>
      <p className="scope-note">修改会影响引用此组件的所有 Suite；不改变它们的选择，也不更新已运行的会话。</p>
      <label htmlFor="suite-resource-json">配置 JSON</label>
      <textarea ref={editor} id="suite-resource-json" value={raw} onChange={event => setRaw(event.target.value)} disabled={busy} spellCheck={false} autoCapitalize="off" autoCorrect="off" rows={18} />
      <div className="suites-toolbar suites-save-actions">
        <button className="suites-primary" type="submit" disabled={busy}>{busy ? '正在保存…' : '保存配置'}</button>
        <button type="button" onClick={close} disabled={busy}>取消编辑</button>
      </div>
    </form> : <>
      <header className="suites-resource-heading"><h2>{kind.toUpperCase()} 配置</h2><ResourceHelp kind={kind} /></header>
      <p className="scope-note">每个组件独立保存为一个 JSON 文件。</p>
      <div className="suites-definition-list">
        {data[kind].map(option => <article className="suites-definition" key={option.id}>
          <div className="suites-definition-heading"><div><h3>{option.name}</h3><code>{kind}/{option.id}.json</code></div>
            <button ref={node => { if (openerId.current === option.id) opener.current = node }} type="button" aria-label={`编辑 ${option.name} 配置`} onClick={event => {
              openerId.current = option.id
              opener.current = event.currentTarget
              const { id: _id, ...definition } = option
              original.current = JSON.stringify(definition, null, 2)
              setRaw(original.current); setEditing(option); setError(''); setNotice('')
              requestAnimationFrame(() => editor.current?.focus())
            }}>Edit</button></div>
          <details className="suites-definition-detail"><summary aria-label={`${option.name} 详情`}>用途说明</summary><p>{option.description || '暂无用途说明。'}</p></details>
        </article>)}
        {data[kind].length === 0 && <p className="suites-empty">暂无 {kind.toUpperCase()} 配置。</p>}
      </div>
    </>}
  </section>
}
