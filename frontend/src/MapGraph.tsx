import { useCallback, useId, useLayoutEffect, useMemo, useRef, useState } from 'react'
import type { WorkflowMap } from './schedulerApi'
import { orderedMapNodeIds } from './mapNodeOrder'
import './map-graph.css'

// Every node is included, even if it has no edges. Shared display ordering puts
// prerequisites before dependents and preserves definition order for parallel rows.
function positions(map: WorkflowMap, nodeHeight: number) {
  const levels = new Map<string, number>()
  const rows = new Map<number, number>()
  return orderedMapNodeIds(map).map((id) => {
    const column = map.nodes[id].blocked_by.reduce((level, parent) => Math.max(level, levels.get(parent)! + 1), 0)
    levels.set(id, column)
    const row = rows.get(column) ?? 0
    rows.set(column, row + 1)
    return { id, x: 24 + column * 300, y: 24 + row * (nodeHeight + 32) }
  })
}

type View = { scale: number; x: number; y: number }
type Size = { width: number; height: number }
const nodeWidth = 220
const maximumScale = 2.5

// Keep a large graph reachable at both ends; center graphs smaller than the viewport.
function constrain(view: View, size: Size, width: number, height: number): View {
  const axis = (offset: number, extent: number, viewport: number) => extent <= viewport
    ? (viewport - extent) / 2
    : Math.max(viewport - extent - 24, Math.min(24, offset))
  return { ...view, x: axis(view.x, width * view.scale, size.width), y: axis(view.y, height * view.scale, size.height) }
}

export function MapGraph({ map, selectedId, onSelect, botProblems = {} }: {
  map: WorkflowMap; selectedId: string | null; onSelect: (id: string) => void; botProblems?: Record<string, string>
}) {
  const instanceId = useId()
  const markerId = `${instanceId}-arrow`
  const helpId = `${instanceId}-help`
  const viewportRef = useRef<HTMLDivElement>(null)
  const dragRef = useRef<{ pointerId: number; x: number; y: number } | null>(null)
  const [dragging, setDragging] = useState(false)
  const [size, setSize] = useState<Size>({ width: 640, height: 384 })
  const [view, setView] = useState<View>({ scale: 1, x: 0, y: 0 })
  const nodeHeight = 104
  const nodes = useMemo(() => positions(map, nodeHeight), [map, nodeHeight])
  const byId = useMemo(() => new Map(nodes.map((node) => [node.id, node])), [nodes])
  const width = Math.max(280, ...nodes.map((node) => node.x + nodeWidth + 24))
  const height = Math.max(152, ...nodes.map((node) => node.y + nodeHeight + 24))
  const fitScale = Math.min(1, size.width / width, size.height / height)
  const minimumScale = Math.min(0.1, fitScale)
  const prerequisites = new Set(selectedId === null ? [] : map.nodes[selectedId]?.blocked_by ?? [])
  const successors = new Set(nodes.filter(({ id }) => selectedId !== null && map.nodes[id].blocked_by.includes(selectedId)).map(({ id }) => id))

  useLayoutEffect(() => {
    const viewport = viewportRef.current
    if (!viewport) return
    const measure = () => {
      const { width: measuredWidth, height: measuredHeight } = viewport.getBoundingClientRect()
      if (measuredWidth > 0 && measuredHeight > 0) setSize({ width: measuredWidth, height: measuredHeight })
    }
    measure()
    if (typeof ResizeObserver === 'undefined') {
      window.addEventListener('resize', measure)
      return () => window.removeEventListener('resize', measure)
    }
    const observer = new ResizeObserver(measure)
    observer.observe(viewport)
    return () => observer.disconnect()
  }, [nodes.length === 0])

  // A new Map starts fitted. Resizing also restores an overview without moving focus.
  useLayoutEffect(() => {
    setView(constrain({ scale: fitScale, x: 0, y: 0 }, size, width, height))
  }, [nodes, size, fitScale, width, height])

  const revealNode = useCallback((id: string) => {
    const node = byId.get(id)
    if (!node) return
    setView((current) => {
      // Zoom out only when a whole focused node cannot fit at the current scale.
      const scale = Math.min(current.scale, (size.width - 32) / nodeWidth, (size.height - 32) / nodeHeight)
      const left = node.x * scale + current.x
      const top = node.y * scale + current.y
      const right = left + nodeWidth * scale
      const bottom = top + nodeHeight * scale
      const x = left < 16 ? current.x + 16 - left : right > size.width - 16 ? current.x + size.width - 16 - right : current.x
      const y = top < 16 ? current.y + 16 - top : bottom > size.height - 16 ? current.y + size.height - 16 - bottom : current.y
      return constrain({ scale, x, y }, size, width, height)
    })
  }, [byId, size, width, height, nodeHeight])

  useLayoutEffect(() => {
    // Selection reveals its node but does not reset the user's zoom or fit the Map.
    if (selectedId !== null) revealNode(selectedId)
  }, [selectedId, revealNode])

  function zoom(factor: number) {
    setView((current) => {
      const scale = Math.max(minimumScale, Math.min(maximumScale, current.scale * factor))
      const ratio = scale / current.scale
      return constrain({ scale, x: size.width / 2 - (size.width / 2 - current.x) * ratio, y: size.height / 2 - (size.height / 2 - current.y) * ratio }, size, width, height)
    })
  }

  if (nodes.length === 0) return <p className="scheduler-graph-empty">此 Map 没有 node。</p>

  return <div className="scheduler-graph">
    <div className="scheduler-graph-tools" role="group" aria-label="依赖图视图控制">
      <button type="button" aria-label="缩小依赖图" disabled={view.scale <= minimumScale} onClick={() => zoom(1 / 1.25)}>−</button>
      <output aria-label="依赖图缩放比例">{Math.round(view.scale * 100)}%</output>
      <button type="button" aria-label="放大依赖图" disabled={view.scale >= maximumScale} onClick={() => zoom(1.25)}>+</button>
      <button type="button" onClick={() => setView(constrain({ scale: fitScale, x: 0, y: 0 }, size, width, height))}>适应全部</button>
    </div>
    <p id={helpId} className="scheduler-graph-help">拖动空白处平移；用 Tab 聚焦节点，Enter 或空格选择。加减按钮缩放，适应全部恢复总览。选中节点以双线框标示，直接前置与后继以标签和虚线边标示。</p>
    <div className="scheduler-graph-viewport" ref={viewportRef} data-dragging={dragging || undefined}>
      <svg viewBox={`0 0 ${size.width} ${size.height}`} role="group" aria-label="Map 依赖图" aria-describedby={helpId}
        onPointerDown={(event) => {
          if (!event.isPrimary || event.button !== 0 || dragRef.current || (event.target instanceof Element && event.target.closest('button'))) return
          event.preventDefault()
          dragRef.current = { pointerId: event.pointerId, x: event.clientX, y: event.clientY }
          event.currentTarget.setPointerCapture?.(event.pointerId)
          setDragging(true)
        }}
        onPointerMove={(event) => {
          const drag = dragRef.current
          if (!drag || drag.pointerId !== event.pointerId) return
          const dx = event.clientX - drag.x, dy = event.clientY - drag.y
          drag.x = event.clientX
          drag.y = event.clientY
          setView((current) => constrain({ ...current, x: current.x + dx, y: current.y + dy }, size, width, height))
        }}
        onPointerUp={(event) => {
          if (dragRef.current?.pointerId !== event.pointerId) return
          dragRef.current = null
          setDragging(false)
          if (event.currentTarget.hasPointerCapture?.(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId)
        }}
        onPointerCancel={() => { dragRef.current = null; setDragging(false) }}
        onLostPointerCapture={() => { dragRef.current = null; setDragging(false) }}>
        <defs><marker id={markerId} viewBox="0 0 10 10" refX="10" refY="5" markerWidth="10" markerHeight="10" orient="auto" markerUnits="userSpaceOnUse">
          <path d="M 0 0 L 10 5 L 0 10 z" fill="#a9d0f0" />
        </marker></defs>
        <g transform={`translate(${view.x} ${view.y}) scale(${view.scale})`}>
          {nodes.flatMap((target) => map.nodes[target.id].blocked_by.map((sourceId, index) => {
            const source = byId.get(sourceId)!
            const x1 = source.x + nodeWidth, y1 = source.y + nodeHeight / 2, x2 = target.x, y2 = target.y + nodeHeight / 2
            const middle = (x1 + x2) / 2
            const related = sourceId === selectedId || target.id === selectedId
            return <path key={`${target.id}-${index}`} role="img" aria-label={`${sourceId} → ${target.id}`}
              className={`scheduler-graph-edge${related ? ' scheduler-graph-edge-related' : ''}`}
              d={`M ${x1} ${y1} C ${middle} ${y1}, ${middle} ${y2}, ${x2} ${y2}`}
              markerEnd={`url(#${markerId})`} />
          }))}
          {nodes.map(({ id, x, y }) => <foreignObject key={id} x={x} y={y} width={nodeWidth} height={nodeHeight}>
            <button type="button" className={`scheduler-node${prerequisites.has(id) || successors.has(id) ? ' scheduler-node-related' : ''}`}
              aria-pressed={selectedId === id} aria-label={`${map.nodes[id].title} ${id}`}
              onFocus={() => revealNode(id)} onClick={() => onSelect(id)}>
              <strong title={map.nodes[id].title}>{map.nodes[id].title}</strong><span className="scheduler-node-id-label" title={id}>{id}</span>
              {prerequisites.has(id) && <small className="scheduler-node-relation">直接前置</small>}
              {successors.has(id) && <small className="scheduler-node-relation">直接后继</small>}
              {botProblems[id] && <small className="scheduler-node-problem">{botProblems[id]}</small>}
            </button>
          </foreignObject>)}
        </g>
      </svg>
    </div>
  </div>
}
