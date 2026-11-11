import { useEffect, useState } from 'react'
import { groupOnlyRevision, loadMapInputs, saveMapInputs, type BotRevision } from './schedulerApi'

export function MapInputsEditor({ file, onClose, onSaved }: { file: string; onClose: () => void; onSaved: () => void }) {
  const [content, setContent] = useState('')
  const [requirements, setRequirements] = useState('')
  const [revision, setRevision] = useState<BotRevision | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    let cancelled = false
    void loadMapInputs(file).then(draft => {
      if (!cancelled) { setContent(JSON.stringify(draft.input_files, null, 2)); setRequirements(draft.input_requirements); setRevision(draft.revision) }
    }, failure => { if (!cancelled) setError(failure instanceof Error ? failure.message : '输入声明读取失败。') })
    return () => { cancelled = true }
  }, [file])
  useEffect(() => {
    function updated(event: Event) { const { kind, id, revision } = (event as CustomEvent<{ kind: string; id: string; revision: BotRevision }>).detail; if (kind === 'maps' && id === file) setRevision(previous => groupOnlyRevision(previous, revision) ? revision : previous) }
    window.addEventListener('herdr-group-revision', updated)
    return () => window.removeEventListener('herdr-group-revision', updated)
  }, [file])
  async function save() {
    if (!revision || busy) return
    setBusy(true); setError(null)
    try {
      const input_files: unknown = JSON.parse(content)
      await saveMapInputs(file, { input_requirements: requirements, input_files, revision })
      onSaved()
    } catch (failure) { setError(failure instanceof Error ? failure.message : '输入声明保存失败。') }
    finally { setBusy(false) }
  }
  return <form className="scheduler-run-form" aria-label="Map 文件输入声明" onSubmit={event => { event.preventDefault(); void save() }}>
    <h3>Map 文件输入声明</h3>
    <p>整体输入要求仅填写非文件要求；每个文件的说明填写在 description，将显示在对应输入框上方。</p>
    <p>声明数组使用 {`{ "id": "brief", "required": true, "description": "业务委托及约束" }`}；可选项设 required 为 false，无输入使用 []。ID 仅在本 Map 接口内唯一。这里只约定材料，不填写本次文件地址。</p>
    <label>Map 输入要求 Markdown<textarea value={requirements} disabled={!revision || busy} onChange={event => setRequirements(event.target.value)} /></label>
    <label>Map 文件输入声明 JSON<textarea spellCheck={false} value={content} disabled={!revision || busy} onChange={event => setContent(event.target.value)} /></label>
    {error && <p role="alert">{error}</p>}
    <div className="scheduler-actions">
      <button type="submit" disabled={!revision || busy}>保存输入声明</button>
      <button type="button" disabled={busy} onClick={onClose}>放弃输入声明编辑</button>
    </div>
  </form>
}
