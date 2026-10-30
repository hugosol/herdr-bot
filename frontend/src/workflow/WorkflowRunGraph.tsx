import { useId, useLayoutEffect, useMemo, useRef, useState, type CSSProperties } from 'react'
import { orderedMapNodeIds } from '../scheduler/mapNodeOrder'
import type { WorkflowMap } from '../scheduler/schedulerApi'
import { WorkflowStatus } from './WorkflowStatus'
import './workflow-run-graph.css'

type Edge = { source: string; target: string; path: string }
type GraphGeometry = { width: number; height: number; edges: Edge[] }

export function WorkflowRunGraph({ map, selectedId, onSelect, nodeStates, nodeTasks, nodeBots, nodeReportPolicies, animateRunning }: {
  map: WorkflowMap
  selectedId: string | null
  onSelect: (id: string) => void
  nodeStates: Record<string, { status: string; label: string }>
  nodeTasks: Record<string, string>
  nodeBots: Record<string, string>
  nodeReportPolicies: Record<string, 'auto' | 'manual'>
  animateRunning: boolean
}) {
  const markerId = `${useId()}-workflow-arrow`
  const bodyRef = useRef<HTMLDivElement>(null)
  const buttonsRef = useRef(new Map<string, HTMLButtonElement>())
  const [geometry, setGeometry] = useState<GraphGeometry>({ width: 0, height: 0, edges: [] })
  const levels = useMemo(() => {
    const byId = new Map<string, number>()
    const columns: string[][] = []
    for (const id of orderedMapNodeIds(map)) {
      const level = map.nodes[id].blocked_by.reduce((maximum, prerequisite) => Math.max(maximum, (byId.get(prerequisite) ?? -1) + 1), 0)
      byId.set(id, level)
      ;(columns[level] ??= []).push(id)
    }
    return columns
  }, [map])

  useLayoutEffect(() => {
    const body = bodyRef.current
    if (!body) return
    const measure = () => {
      const bounds = body.getBoundingClientRect()
      const edges: Edge[] = []
      for (const target of Object.keys(map.nodes)) {
        const targetBounds = buttonsRef.current.get(target)?.getBoundingClientRect()
        if (!targetBounds) continue
        for (const source of map.nodes[target].blocked_by) {
          const sourceBounds = buttonsRef.current.get(source)?.getBoundingClientRect()
          if (!sourceBounds) continue
          const x1 = sourceBounds.right - bounds.left
          const y1 = sourceBounds.top + sourceBounds.height / 2 - bounds.top
          const x2 = targetBounds.left - bounds.left
          const y2 = targetBounds.top + targetBounds.height / 2 - bounds.top
          const middle = (x1 + x2) / 2
          edges.push({ source, target, path: `M ${x1} ${y1} C ${middle} ${y1}, ${middle} ${y2}, ${x2} ${y2}` })
        }
      }
      setGeometry({ width: bounds.width, height: bounds.height, edges })
    }
    measure()
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(measure)
    observer?.observe(body)
    for (const button of buttonsRef.current.values()) observer?.observe(button)
    window.addEventListener('resize', measure)
    return () => {
      observer?.disconnect()
      window.removeEventListener('resize', measure)
    }
  }, [map, nodeStates, nodeTasks, nodeBots])

  return <div className="workflow-run-graph" role="group" aria-label="Map 依赖图" tabIndex={0}>
    <div className="workflow-run-graph-body" ref={bodyRef} style={{ '--workflow-graph-levels': Math.max(1, levels.length) } as CSSProperties}>
      <svg className="workflow-run-graph-edges" width={geometry.width} height={geometry.height}>
        <defs><marker id={markerId} viewBox="0 0 7 7" refX="7" refY="3.5" markerWidth="7" markerHeight="7" orient="auto" markerUnits="userSpaceOnUse">
          <path d="M 0 0 L 7 3.5 L 0 7" fill="none" stroke="currentColor" strokeWidth="1" />
        </marker></defs>
        {geometry.edges.map(edge => <path key={`${edge.source}\u0000${edge.target}`} role="img" aria-label={`${edge.source} → ${edge.target}`}
          className="workflow-run-graph-edge" d={edge.path} markerEnd={`url(#${markerId})`} />)}
      </svg>
      {levels.length === 0 && <p className="workflow-run-graph-empty">此 Map 没有 node。</p>}
      {levels.map((ids, level) => <div className="workflow-run-graph-level" key={level}>
        <span className="workflow-run-graph-level-label">{level === 0 ? '起点' : ids.length > 1 ? '并行工作' : map.nodes[ids[0]].blocked_by.length > 1 ? '汇合交接' : `依赖层 ${level + 1}`}</span>
        {ids.map(id => {
          const node = map.nodes[id]
          const status = nodeStates[id]?.status ?? 'pending'
          return <button key={id} type="button" className={`workflow-run-graph-node${selectedId === id ? ' is-selected' : ''}`}
            ref={element => { if (element) buttonsRef.current.set(id, element); else buttonsRef.current.delete(id) }}
            aria-label={`${node.title} ${id}`} aria-pressed={selectedId === id} data-node-id={id} data-node-state={status}
            data-report-policy={nodeReportPolicies[id] ?? 'auto'} onClick={() => onSelect(id)}>
            {nodeReportPolicies[id] === 'manual' && <svg className="workflow-run-graph-node-shape" aria-hidden="true" viewBox="0 0 100 100" preserveAspectRatio="none">
              <polygon points="7,1 93,1 99,9 99,91 93,99 7,99 1,91 1,9" vectorEffect="non-scaling-stroke" />
            </svg>}
            <span className="workflow-run-graph-node-top"><code>{id}</code><WorkflowStatus status={status} saved={!animateRunning} animate={animateRunning} /></span>
            <strong>{node.title}</strong>
            <span className="workflow-run-graph-task"><span>Task</span>{nodeTasks[id] ?? '—'}</span>
            <span className="workflow-run-graph-bot">Bot · {nodeBots[id] ?? node.bot_id}</span>
          </button>
        })}
      </div>)}
    </div>
  </div>
}
