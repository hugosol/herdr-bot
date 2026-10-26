import { useEffect, useId, useMemo, useRef, useState, type PointerEvent } from 'react'
import type { WorkflowMap } from './schedulerApi'

// Lay out every node, not only those incident to an edge. Kahn traversal assigns
// each dependent a column after all its prerequisites; the API rejects cycles.
function positions(map: WorkflowMap) {
  const ids = Object.keys(map.nodes)
  const levels = new Map(ids.map((id) => [id, 0]))
  const remaining = new Map(ids.map((id) => [id, map.nodes[id].blocked_by.length]))
  const dependents = new Map<string, string[]>()
  for (const id of ids) for (const prerequisite of map.nodes[id].blocked_by) {
    const next = dependents.get(prerequisite) ?? []
    next.push(id)
    dependents.set(prerequisite, next)
  }
  const ready = ids.filter((id) => remaining.get(id) === 0)
  for (let index = 0; index < ready.length; index++) {
    const id = ready[index]
    for (const next of dependents.get(id) ?? []) {
      levels.set(next, Math.max(levels.get(next)!, levels.get(id)! + 1))
      remaining.set(next, remaining.get(next)! - 1)
      if (remaining.get(next) === 0) ready.push(next)
    }
  }
  const rows = new Map<number, number>()
  return ids.map((id) => {
    const column = levels.get(id)!
    const row = rows.get(column) ?? 0
    rows.set(column, row + 1)
    return { id, x: 24 + column * 300, y: 24 + row * 112 }
  })
}

export function MapGraph({ map, selectedId, onSelect }: {
  map: WorkflowMap; selectedId: string | null; onSelect: (id: string) => void
}) {
  const markerId = useId()
  const helpId = useId()
  const nodes = useMemo(() => positions(map), [map])
  const byId = useMemo(() => new Map(nodes.map((node) => [node.id, node])), [nodes])
  const width = Math.max(280, ...nodes.map((node) => node.x + 244))
  const height = Math.max(120, ...nodes.map((node) => node.y + 104))
  const [view, setView] = useState({ x: 0, y: 0, scale: 1 })
  const drag = useRef<{ pointerId: number; x: number; y: number; viewX: number; viewY: number } | null>(null)
  function fit() { setView({ x: 0, y: 0, scale: 1 }) }
  useEffect(() => { fit(); drag.current = null }, [map])
  function zoom(factor: number) {
    setView((current) => {
      const scale = Math.max(.25, Math.min(4, current.scale * factor))
      const ratio = scale / current.scale
      return { scale, x: width / 2 + (current.x - width / 2) * ratio, y: height / 2 + (current.y - height / 2) * ratio }
    })
  }
  function point(event: PointerEvent<SVGSVGElement>) {
    return new DOMPoint(event.clientX, event.clientY).matrixTransform(event.currentTarget.getScreenCTM()!.inverse())
  }
  return <div>
    <div className="scheduler-graph-controls" role="group" aria-label="依赖图视图控制">
      <button type="button" aria-label="放大依赖图" disabled={view.scale >= 4} onClick={() => zoom(1.25)}>放大</button>
      <button type="button" aria-label="缩小依赖图" disabled={view.scale <= .25} onClick={() => zoom(.8)}>缩小</button>
      <button type="button" onClick={fit}>适应画布</button>
    </div>
    <p id={helpId}>拖动图中空白处平移，用按钮缩放或适应画布。Tab 聚焦 node，Enter／Space 选择；继续 Tab 可进入模板明细，Shift+Tab 返回。键盘聚焦 node 时自动适应画布。</p>
    <div className="scheduler-graph-scroll">
    <svg viewBox={`0 0 ${width} ${height}`} role="group" aria-label="Map 依赖图" aria-describedby={helpId}
      onPointerDown={(event) => {
        if (event.button !== 0 || (event.target as Element).closest('button')) return
        const start = point(event)
        drag.current = { pointerId: event.pointerId, x: start.x, y: start.y, viewX: view.x, viewY: view.y }
        event.currentTarget.setPointerCapture(event.pointerId)
      }}
      onPointerMove={(event) => {
        const start = drag.current
        if (start?.pointerId !== event.pointerId) return
        const next = point(event)
        setView((current) => ({ ...current, x: start.viewX + next.x - start.x, y: start.viewY + next.y - start.y }))
      }}
      onPointerUp={() => { drag.current = null }} onPointerCancel={() => { drag.current = null }}>
      <g transform={`translate(${view.x} ${view.y}) scale(${view.scale})`}>
      <defs><marker id={markerId} viewBox="0 0 10 10" refX="10" refY="5" markerWidth="10" markerHeight="10" orient="auto" markerUnits="userSpaceOnUse">
        <path d="M 0 0 L 10 5 L 0 10 z" fill="#a9d0f0" />
      </marker></defs>
      {nodes.flatMap((target) => map.nodes[target.id].blocked_by.map((sourceId, index) => {
        const source = byId.get(sourceId)!
        const x1 = source.x + 220, y1 = source.y + 40, x2 = target.x, y2 = target.y + 40
        const middle = (x1 + x2) / 2
        return <path key={`${target.id}-${index}`} role="img" aria-label={`${sourceId} → ${target.id}`}
          d={`M ${x1} ${y1} C ${middle} ${y1}, ${middle} ${y2}, ${x2} ${y2}`}
          fill="none" stroke="#a9d0f0" strokeWidth="2" markerEnd={`url(#${markerId})`} />
      }))}
      {nodes.map(({ id, x, y }) => <foreignObject key={id} x={x} y={y} width="220" height="80">
        <button type="button" className="scheduler-node" aria-pressed={selectedId === id}
          aria-label={`${map.nodes[id].title} ${id}`} onClick={() => onSelect(id)}
          onFocus={(event) => { if (event.currentTarget.matches(':focus-visible')) fit() }}>
          <strong>{map.nodes[id].title}</strong><span>{id}</span>
        </button>
      </foreignObject>)}
      </g>
    </svg>
    </div>
    {nodes.length === 0 && <p>此 Map 没有 node。</p>}
  </div>
}
