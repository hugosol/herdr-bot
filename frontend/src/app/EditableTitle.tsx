import { useCallback, useEffect, useRef, useState } from 'react'
import './editable-title.css'

type Props = {
  value: string
  label: string
  active: boolean
  disabled?: boolean
  heading?: 'h2' | 'h3'
  className?: string
  onSave: (name: string) => Promise<void>
}

/** A draft never persists on blur: Enter is the sole commit action. */
export function EditableTitle({ value, label, active, disabled = false, heading: Heading = 'h3', className, onSave }: Props) {
  const [draft, setDraft] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const submitting = useRef(false)
  const button = useRef<HTMLButtonElement>(null)
  const mounted = useRef(true)
  const focusInput = useCallback((input: HTMLInputElement | null) => {
    if (input) { input.focus(); input.select() }
  }, [])
  useEffect(() => {
    mounted.current = true
    return () => { mounted.current = false }
  }, [])
  useEffect(() => { setDraft(null); setError(null) }, [active, value])
  useEffect(() => { if (disabled) setDraft(null) }, [disabled])

  function cancel(restoreFocus = false) {
    setDraft(null)
    if (restoreFocus) button.current?.focus()
  }

  async function save() {
    if (draft === null || submitting.current || !active || disabled) return
    const name = draft.trim()
    cancel(true)
    if (!name || name === value) return
    submitting.current = true
    setBusy(true)
    setError(null)
    try {
      await onSave(name)
    } catch (failure) {
      if (mounted.current) setError(failure instanceof Error ? failure.message : `${label}保存失败。`)
    } finally {
      submitting.current = false
      if (mounted.current) setBusy(false)
    }
  }

  return <div className="editable-title" aria-busy={busy}>
    <div className="editable-title__row">
      <Heading className={className}>
        {draft !== null && active && !disabled
          ? <input className="editable-title__input" aria-label={label} value={draft}
              ref={focusInput}
              onChange={event => setDraft(event.target.value)} onBlur={() => cancel()}
              onKeyDown={event => {
                if (event.nativeEvent.isComposing || event.nativeEvent.keyCode === 229) return
                if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); cancel(true) }
                if (event.key === 'Enter') { event.preventDefault(); event.stopPropagation(); void save() }
              }} />
          : value}
      </Heading>
      <button ref={button} type="button" className="editable-title__pencil" aria-label={`修改${label}`} title={`修改${label}（Enter 保存，Esc 取消）`}
        disabled={disabled || busy || !active} onClick={() => { setError(null); setDraft(value) }}>
        <svg aria-hidden="true" focusable="false" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round"><path d="m16 3 5 5-12 12-6 1 1-6L16 3Zm-2 2 5 5M4 15l5 5" /></svg>
      </button>
    </div>
    {error && <p className="editable-title__error" role="alert">{error}</p>}
  </div>
}
