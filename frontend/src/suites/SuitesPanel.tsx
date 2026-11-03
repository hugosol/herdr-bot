import { useEffect, useState, type FormEvent } from 'react'
import { suiteRequest, type Suite, type SuiteList } from './suiteApi'
import './suites.css'

type Draft = Omit<Suite, 'skills' | 'extensions' | 'prompts' | 'lsp'> & {
  skills: string; extensions: string; prompts: string; lsp: string
}
function draftFrom(suite: Suite): Draft {
  return { ...suite, skills: suite.skills.join('\n'), extensions: suite.extensions.join('\n'), prompts: suite.prompts.join('\n'), lsp: JSON.stringify(suite.lsp, null, 2) }
}
const emptySuite: Suite = { id: '', name: '', agents: ['omp'], skills: [], extensions: [], prompts: [], lsp: {}, mcp: [] }
const lines = (text: string) => text.split(/\r?\n/).map(line => line.trim()).filter(Boolean)
const message = (error: unknown) => error instanceof Error ? error.message : '无法连接 Suite 服务，未确认保存。'

export function SuitesPanel({ active }: { active: boolean }) {
  const [data, setData] = useState<SuiteList | null>(null)
  const [draft, setDraft] = useState<Draft | null>(null)
  const [editing, setEditing] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [notice, setNotice] = useState('')

  useEffect(() => {
    if (!active) return
    let current = true
    setBusy(true)
    suiteRequest().then(result => { if (current) { setData(result); setError('') } })
      .catch(error => { if (current) setError(message(error)) })
      .finally(() => { if (current) setBusy(false) })
    return () => { current = false }
  }, [active])

  function open(suite: Suite, create = false) {
    setDraft(draftFrom(suite)); setEditing(create ? null : suite.id); setError(''); setNotice('')
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
      const lsp: unknown = JSON.parse(draft.lsp)
      if (lsp === null || typeof lsp !== 'object' || Array.isArray(lsp)) throw new Error('LSP 声明必须是 JSON 对象。')
      suite = { ...draft, skills: lines(draft.skills), extensions: lines(draft.extensions), prompts: lines(draft.prompts), lsp: lsp as Record<string, unknown> }
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

  return <section className="suites-panel" aria-label="Suite 管理">
    <header className="suites-toolbar">
      <h2>Suite · 场景工具集</h2>
      <button type="button" disabled={busy} onClick={() => open(emptySuite, true)}>创建 Suite</button>
      <button type="button" disabled={busy} onClick={() => void reload()}>重新读取 Suite</button>
    </header>
    <p className="scope-note">界面与手工维护使用同一份 Suite 权威文件。资源路径按声明文件所在目录或集中 roots 解析；这里不编辑 MCP 连接，也不表示资源已经运行。</p>
    {error && <p className="zone-error" role="alert">{error}</p>}
    {notice && <p role="status">{notice}</p>}
    <div className="suites-layout">
      <section aria-label="Suite 列表" className="suites-list">
        {data === null ? <p>正在读取 Suite…</p> : data.suites.length === 0 ? <p>尚未定义 Suite</p> : data.suites.map(suite =>
          <button key={suite.id} type="button" disabled={busy} aria-pressed={editing === suite.id} onClick={() => open(suite)}>{suite.name} · {suite.id}</button>)}
      </section>
      {draft && <form className="suites-editor" onSubmit={event => void save(event)} aria-label="Suite 编辑">
        <fieldset disabled={busy}>
          <legend>{editing ? '查看 / 编辑 Suite' : '创建 Suite'}</legend>
          <label>Suite ID<input className="zone-form__input" value={draft.id} disabled={editing !== null} required onChange={event => setDraft({ ...draft, id: event.target.value })} /></label>
          <label>Suite 名称<input className="zone-form__input" value={draft.name} required onChange={event => setDraft({ ...draft, name: event.target.value })} /></label>
          <fieldset><legend>适用宿主</legend>{(['omp', 'pi'] as const).map(agent => <label className="suites-choice" key={agent}>
            <input type="checkbox" checked={draft.agents.includes(agent)} onChange={event => setDraft({ ...draft, agents: event.target.checked ? [...draft.agents, agent] : draft.agents.filter(value => value !== agent), pi: agent === 'pi' ? (event.target.checked ? { codemode: 'on' } : undefined) : draft.pi })} />{agent.toUpperCase()}
          </label>)}</fieldset>
          {draft.agents.includes('pi') && <fieldset><legend>Pi 配置</legend>
            <label className="suites-choice"><input type="checkbox" checked={draft.pi?.codemode === 'only'} onChange={event => setDraft({ ...draft, pi: { codemode: event.target.checked ? 'only' : 'on' } })} />Codemode only</label>
            <p className="scope-note">关闭 only 表示明确的 on，不禁用，也不继承全局模式。</p>
          </fieldset>}
          {(['skills', 'extensions', 'prompts'] as const).map((key, index) => <div key={key}>
            <label htmlFor={`suite-${key}`}>{['Skills（每行一个路径）', 'Extensions（每行一个路径）', 'Prompt templates（每行一个路径）'][index]}</label>
            <textarea id={`suite-${key}`} className="zone-form__input" rows={3} value={draft[key]} onChange={event => setDraft({ ...draft, [key]: event.target.value })} />
          </div>)}
          <p className="scope-note">Pi 跳过 LSP；OMP 跳过 Suite 本地 prompt templates，既不作为初始指示，也不转换语义。</p>
          <label htmlFor="suite-lsp">LSP 声明（JSON）</label>
          <textarea id="suite-lsp" className="zone-form__input" rows={6} value={draft.lsp} onChange={event => setDraft({ ...draft, lsp: event.target.value })} />
          <fieldset><legend>集中 MCP 选择</legend>
            <p className="scope-note">仅引用集中目录；空清单表示不追加，不关闭宿主原生 MCP。</p>
            {data?.mcp.length === 0 && <p>集中目录没有 MCP 定义。</p>}
            {data?.mcp.map(id => <label key={id} className="suites-choice"><input type="checkbox" checked={draft.mcp.includes(id)} onChange={event => setDraft({ ...draft, mcp: event.target.checked ? [...draft.mcp, id] : draft.mcp.filter(value => value !== id) })} />{id}</label>)}
          </fieldset>
          <div className="suites-toolbar">
            <button type="submit">保存 Suite</button>
            {editing && <button type="button" onClick={() => void remove()}>删除 Suite</button>}
            <button type="button" onClick={() => { setDraft(null); setEditing(null) }}>关闭编辑</button>
          </div>
        </fieldset>
      </form>}
    </div>
  </section>
}
