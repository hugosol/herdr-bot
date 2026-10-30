import type { WorkflowMap } from './schedulerApi'

// Display dependency levels, not an execution schedule. The API validates the DAG;
// preserving definition order within each level keeps parallel nodes predictable.
export function orderedMapNodeIds(map: WorkflowMap): string[] {
  const ids = Object.keys(map.nodes)
  const levels = new Map(ids.map(id => [id, 0]))
  const remaining = new Map(ids.map(id => [id, map.nodes[id].blocked_by.length]))
  const dependents = new Map<string, string[]>()
  for (const id of ids) for (const prerequisite of map.nodes[id].blocked_by) {
    const next = dependents.get(prerequisite) ?? []
    next.push(id)
    dependents.set(prerequisite, next)
  }
  const ready = ids.filter(id => remaining.get(id) === 0)
  for (let index = 0; index < ready.length; index++) {
    const id = ready[index]
    for (const next of dependents.get(id) ?? []) {
      levels.set(next, Math.max(levels.get(next)!, levels.get(id)! + 1))
      remaining.set(next, remaining.get(next)! - 1)
      if (remaining.get(next) === 0) ready.push(next)
    }
  }
  return ids.sort((a, b) => levels.get(a)! - levels.get(b)!)
}
