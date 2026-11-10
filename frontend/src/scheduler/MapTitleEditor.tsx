import { useEffect, useState } from 'react'
import { groupOnlyRevision, loadMapTitle, saveMapTitle, type BotRevision } from './schedulerApi'

export function MapTitleEditor({ file, title, onClose, onSaved }: { file: string; title: string; onClose: () => void; onSaved: () => void }) {
  const [name, setName] = useState(title)
  const [revision, setRevision] = useState<BotRevision | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    let cancelled = false
    setRevision(null); setError(null)
    void loadMapTitle(file).then(draft => {
      if (!cancelled) { setName(draft.title); setRevision(draft.revision) }
    }, failure => { if (!cancelled) setError(failure instanceof Error ? failure.message : 'Map 名称读取失败。') })
    return () => { cancelled = true }
  }, [file])
  useEffect(() => {
    function updated(event: Event) { const { kind, id, revision } = (event as CustomEvent<{ kind: string; id: string; revision: BotRevision }>).detail; if (kind === 'maps' && id === file) setRevision(previous => groupOnlyRevision(previous, revision) ? revision : previous) }
    window.addEventListener('herdr-group-revision', updated)
    return () => window.removeEventListener('herdr-group-revision', updated)
  }, [file])
  async function save() {
    if (!revision || busy) return
    const trimmed = name.trim()
    if (!trimmed) { setError('Map 显示名称不能为空。'); return }
    setBusy(true); setError(null)
    try {
      await saveMapTitle(file, { title: trimmed, revision })
      onSaved()
    } catch (failure) { setError(failure instanceof Error ? failure.message : 'Map 名称保存失败。') }
    finally { setBusy(false) }
  }
  return <form className="scheduler-run-form" aria-label="编辑 Map 名称" onSubmit={event => { event.preventDefault(); void save() }}>
    <h3>Map 显示名称</h3>
    <label>Map 显示名称<input aria-label="Map 显示名称" value={name} disabled={!revision || busy} onChange={event => setName(event.target.value)} /></label>
    {error && <p role="alert">{error}</p>}
    <div className="scheduler-actions">
      <button type="submit" disabled={!revision || busy}>保存名称</button>
      <button type="button" disabled={busy} onClick={onClose}>取消</button>
    </div>
  </form>
}
