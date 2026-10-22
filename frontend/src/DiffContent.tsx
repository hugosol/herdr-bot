import { Fragment, useState } from 'react'
import { diffRows, sideBySideRows, type DiffRow } from './diff'
import type { ChangeDiff } from './diffApi'

/** 同一份行分段同时用于宽屏对照和窄屏统一展示，行号始终来自完整 diff。 */
export function DiffContent({ diff, narrow }: { diff: ChangeDiff; narrow: boolean }) {
  const [expanded, setExpanded] = useState<Set<number>>(() => new Set())
  const oldText = diff.old.kind === 'text' ? diff.old.content : ''
  const newText = diff.new.kind === 'text' ? diff.new.content : ''
  const rows = diffRows(oldText, newText)
  if (!rows.some(row => row.kind !== 'context')) {
    return <p className="browser-empty" data-testid="diff-empty">两侧内容相同，没有行级差异。</p>
  }

  const segments: { start: number; rows: DiffRow[]; hidden: boolean }[] = []
  let visibleStart = 0
  for (let start = 0; start < rows.length;) {
    if (rows[start].kind !== 'context') {
      start += 1
      continue
    }
    let end = start
    while (end < rows.length && rows[end].kind === 'context') end += 1
    const hiddenStart = start + (start === 0 ? 0 : 3)
    const hiddenEnd = end - (end === rows.length ? 0 : 3)
    if (hiddenEnd > hiddenStart) {
      segments.push({ start: visibleStart, rows: rows.slice(visibleStart, hiddenStart), hidden: false })
      segments.push({ start: hiddenStart, rows: rows.slice(hiddenStart, hiddenEnd), hidden: true })
      visibleStart = hiddenEnd
    }
    start = end
  }
  segments.push({ start: visibleStart, rows: rows.slice(visibleStart), hidden: false })

  function renderRows(visible: DiffRow[]) {
    if (narrow) return visible.map(row => (
      <div key={`${row.oldNumber}:${row.newNumber}`} className={`diff-line diff-line--${row.kind}`} data-testid={`diff-row-${row.kind}`}>
        <span className="diff-line__number" aria-hidden="true">{row.oldNumber ?? ''}</span>
        <span className="diff-line__number" aria-hidden="true">{row.newNumber ?? ''}</span>
        <span className="diff-line__marker" aria-hidden="true">{row.kind === 'added' ? '+' : row.kind === 'removed' ? '-' : ' '}</span>
        <span className="diff-line__text">{row.text}</span>
      </div>
    ))
    return sideBySideRows(visible).map((pair, index) => (
      <div key={index} className="diff-pair">
        <div className={`diff-cell diff-cell--${pair.old.kind}`} data-testid={`diff-old-${pair.old.kind}`}>
          <span className="diff-line__number" aria-hidden="true">{pair.old.number ?? ''}</span>
          <span className="diff-line__text" data-testid="diff-old-text">{pair.old.text}</span>
        </div>
        <div className={`diff-cell diff-cell--${pair.new.kind}`} data-testid={`diff-new-${pair.new.kind}`}>
          <span className="diff-line__number" aria-hidden="true">{pair.new.number ?? ''}</span>
          <span className="diff-line__text" data-testid="diff-new-text">{pair.new.text}</span>
        </div>
      </div>
    ))
  }

  return (
    <div className={narrow ? 'diff-lines' : 'diff-side-by-side'} data-testid="diff-view" data-mode={narrow ? 'unified' : 'side-by-side'}>
      {segments.map(segment => {
        const open = expanded.has(segment.start)
        return <Fragment key={`${segment.hidden}:${segment.start}`}>
          {segment.hidden && <button type="button" className="diff-context-toggle" aria-expanded={open} onClick={() => {
            setExpanded(previous => {
              const next = new Set(previous)
              if (next.has(segment.start)) next.delete(segment.start)
              else next.add(segment.start)
              return next
            })
          }}>{open ? `收起 ${segment.rows.length} 行未修改内容` : `已隐藏 ${segment.rows.length} 行 · 展开`}</button>}
          {(!segment.hidden || open) && renderRows(segment.rows)}
        </Fragment>
      })}
    </div>
  )
}
