import { useEffect, useId, useRef, useState, type ReactNode } from 'react'

import { createPortal } from 'react-dom'
import './entity-groups.css'

export function useGroupRefresh() {
  const [revision, setRevision] = useState(0)
  useEffect(() => { const changed = () => setRevision(value => value + 1); window.addEventListener(GROUPS_CHANGED, changed); return () => window.removeEventListener(GROUPS_CHANGED, changed) }, [])
  return revision
}
export const GROUPS_CHANGED = 'herdr-groups-changed'
export function announceGroupChange() { window.dispatchEvent(new Event(GROUPS_CHANGED)) }
export function groupItems<T>(items: readonly T[], groupOf: (item: T) => string | null | undefined): { name: string | null; items: T[] }[] {
  const groups = new Map<string | null, T[]>()
  for (const item of items) {
    const name = groupOf(item) ?? null
    const entries = groups.get(name) ?? []
    entries.push(item)
    groups.set(name, entries)
  }
  return [...groups].sort(([a], [b]) => a === null ? 1 : b === null ? -1 : a < b ? -1 : a > b ? 1 : 0).map(([name, entries]) => ({ name, items: entries }))
}
export function GroupOptions<T>({ items, groupOf, children }: { items: readonly T[]; groupOf: (item: T) => string | null | undefined; children: (item: T) => ReactNode }) {
  return groupItems(items, groupOf).map(({ name, items: entries }) => <optgroup key={name === null ? 'ungrouped' : `group:${name}`} label={name === null ? '未分组' : `组：${name}`}>{entries.map(children)}</optgroup>)
}
export function EntityGroups<T>({ items, groupOf, children, listClass, testId }: { items: readonly T[]; groupOf: (item: T) => string | null | undefined; children: (item: T) => ReactNode; listClass: string; testId?: string }) {
  return groupItems(items, groupOf).map(({ name, items: entries }) => <details className="entity-group" key={name === null ? 'ungrouped' : `group:${name}`} open>
    <summary>{name === null ? '未分组' : `组：${name}`} <span>（{entries.length}）</span></summary>
    <ul className={listClass} data-testid={testId}>{entries.map(children)}</ul>
  </details>)
}

export function GroupButton({ name, group, names, disabled, onSave }: { name: string; group?: string | null; names: (string | null | undefined)[]; disabled?: boolean; onSave: (group: string | null) => Promise<void> }) {
  const id = useId()
  const dialog = useRef<HTMLDialogElement>(null)
  const [opened, setOpened] = useState(false)
  const [value, setValue] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => { if (opened) dialog.current?.showModal(); return () => dialog.current?.close() }, [opened])
  async function save(next: string | null) {
    if (busy) return
    if (next !== null && (!next || next.trim() !== next)) { setError('组名不能为空，且不能以空白开头或结尾。'); return }
    setBusy(true); setError(null)
    try { await onSave(next); announceGroupChange(); setOpened(false) }
    catch (error) { setError(error instanceof Error ? error.message : '分组保存失败；原配置未改变。') }
    finally { setBusy(false) }
  }
  return <><button type="button" className="entity-group-button" disabled={disabled} aria-haspopup="dialog" aria-label={`group：${name}（${group ?? '未分组'}）`} onClick={() => { setValue(''); setError(null); setOpened(true) }} title={`调整分组：${group ?? '未分组'}`}><svg aria-hidden="true" focusable="false" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="miter"><path d="m12 3 9 5-9 5-9-5 9-5Zm-9 9 9 5 9-5M3 16l9 5 9-5" /></svg></button>
    {opened && createPortal(<dialog ref={dialog} className="zone-dialog entity-group-dialog" aria-labelledby={`${id}-title`} onCancel={event => { event.preventDefault(); if (!busy) setOpened(false) }}>
      <h2 id={`${id}-title`} className="zone-dialog__title">{name} · group</h2>
      <label htmlFor={`${id}-existing`}>选择分组</label>
      <select id={`${id}-existing`} className="zone-form__input zone-form__select" value={group ?? ''} disabled={busy} onChange={event => void save(event.target.value || null)}>
        <option value="">未分组</option>
        {[...new Set(names.filter((name): name is string => typeof name === 'string'))].sort().map(name => <option key={name} value={name}>组：{name}</option>)}
      </select>
      <form onSubmit={event => { event.preventDefault(); event.stopPropagation(); void save(value) }}>
        <label htmlFor={`${id}-new`}>新组名</label>
        <input id={`${id}-new`} className="zone-form__input" value={value} disabled={busy} onChange={event => setValue(event.target.value)} aria-describedby={`${id}-help`} />
        <p id={`${id}-help`} className="zone-dialog__hint">区分大小写；非空，不能有首尾空白。</p>
        {error && <p role="alert">{error}</p>}
        <div className="zone-dialog__actions"><button type="submit" className="zone-dialog__submit" disabled={busy}>保存新组名</button><button type="button" className="zone-dialog__cancel" disabled={busy} onClick={() => setOpened(false)}>取消</button></div>
      </form>
    </dialog>, document.body)}
  </>
}
