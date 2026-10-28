import { useEffect, useRef, useState } from 'react'
import { listBots, loadMap, type Bot, type WorkflowMap } from './schedulerApi'
import { fetchTasks, type Task } from './taskApi'
import type { RunDetail } from './workflowApi'

export type RunEntry = { id: string; managed: boolean; readError: string | null; run: { map: string; createdAt: number; status: string; nodes: Record<string, { status: string }> } | null }
export type CurrentDefinition = { map: WorkflowMap | null; mapError: string | null; bots: Bot[]; tasks: Task[]; catalogErrors: string[] }
type Snapshot = { id: string | null; entries: RunEntry[]; detail: RunDetail | null; current: CurrentDefinition | null; at: number }

async function readJSON<T>(url: string): Promise<T> {
  const response = await fetch(url)
  const value = await response.json()
  if (!response.ok) throw new Error(value.message ?? `查询失败：HTTP ${response.status}`)
  return value
}

// These reads belong to the same refresh cycle as the Run facts. Keeping them
// here prevents a slow definition/catalog read overlapping the next poll.
async function currentDefinition(detail: RunDetail, visible: () => boolean): Promise<CurrentDefinition | null> {
  const run = detail.run
  if (!run) return null
  const current: CurrentDefinition = { map: null, mapError: null, bots: [], tasks: [], catalogErrors: [] }
  try {
    const map = await loadMap(run.map)
    const savedIds = Object.keys(run.nodes)
    if (savedIds.length !== Object.keys(map.nodes).length || savedIds.some(id => !Object.hasOwn(map.nodes, id))) {
      current.mapError = '当前 node 标识集合与 Run 不一致，无法可靠对应。'
    } else current.map = map
  } catch (error) { current.mapError = error instanceof Error ? error.message : 'Map 无法读取。' }
  if (!visible()) return current
  try {
    const catalog = await listBots()
    current.bots = catalog.bots
    current.catalogErrors.push(...(catalog.problems ?? []).map(problem => problem.message))
  } catch (error) { current.catalogErrors.push(`Bot 名称不可读：${String(error)}`) }
  if (!visible()) return current
  const tasks = await fetchTasks()
  if (tasks.kind === 'ok') current.tasks = tasks.tasks
  else current.catalogErrors.push('Task 目录不可读；显示保存标识。')
  return current
}

export function useWorkflowRefresh(id: string | null, active: boolean) {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null)
  const [error, setError] = useState<string | null>(null)
  const inFlight = useRef<Promise<void> | null>(null)
  const interval = useRef(5000)
  const refresh = useRef(() => {})
  useEffect(() => {
    let valid = true
    let waiting = false
    let timer: number | undefined
    const visible = () => valid && active && document.visibilityState === 'visible'
    const clear = () => { clearTimeout(timer); timer = undefined }
    const query = (waitForCurrent = false) => {
      clear()
      if (!visible()) return
      if (inFlight.current) {
        // Returning to this page must query immediately after the old request
        // settles; repeated manual clicks during that request are coalesced.
        if (waitForCurrent && !waiting) {
          waiting = true
          void inFlight.current.then(() => { waiting = false; if (visible()) query() })
        }
        return
      }
      const request = async () => {
        try {
          const listing = await readJSON<{ runs: RunEntry[]; pollIntervalSeconds: number }>('/api/workflow/runs')
          interval.current = listing.pollIntervalSeconds * 1000
          if (!visible()) return
          const detail = id ? await readJSON<RunDetail>(`/api/workflow/runs/${encodeURIComponent(id)}`) : null
          if (!visible()) return
          const current = detail ? await currentDefinition(detail, visible) : null
          if (visible()) {
            setSnapshot({ id, entries: listing.runs, detail, current, at: Date.now() })
            setError(null)
          }
        } catch (failure) {
          if (valid) setError(failure instanceof Error ? failure.message : '查询未取得有效响应。')
        }
      }
      inFlight.current = request().finally(() => {
        inFlight.current = null
        if (visible() && !waiting) {
          const due = Date.now() + interval.current
          const tick = () => {
            const remaining = due - Date.now()
            if (remaining <= 0) query()
            else timer = window.setTimeout(tick, Math.min(remaining, 2_147_483_647))
          }
          tick()
        }
      })
    }
    refresh.current = () => query()
    const visibility = () => { clear(); if (visible()) query(true) }
    document.addEventListener('visibilitychange', visibility)
    setError(null)
    query(true)
    return () => {
      valid = false
      clear()
      document.removeEventListener('visibilitychange', visibility)
    }
  }, [id, active])
  return { snapshot, error, refresh: () => refresh.current() }
}
