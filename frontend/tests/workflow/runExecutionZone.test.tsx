import { render, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { Execution, Run } from '../../src/controllers/workflowController'
import type { CurrentDefinition } from '../../src/workflow/useWorkflowRefresh'
import { RunCurrentView } from '../../src/workflow/RunCurrentView'

const current: CurrentDefinition = { map: { title: 'Dynamic map', description: '', nodes: { A: { title: 'Worker', bot_id: 'worker', input: '', blocked_by: [], zone: { type: 'dynamic' } } } }, mapError: null, bots: [], tasks: [], catalogErrors: [] }
function runWith(execution: Execution | null): Run {
  return { id: 'run', map: 'map.json', createdAt: 1, managerTaskId: 'manager', zone: 'C:/manager', workspaceName: 'workspace', userInput: '', status: 'running', manager: null, nodes: { A: { status: execution ? 'running' : 'pending', execution, failure: null, completion: null, previousExecutions: [] } } }
}
const files = { id: 'execution', zoneId: 'team/selected', zone: 'C:/frozen-actual-directory', inputRule: [], outputRule: [], inputs: [] }
const executions: Execution[] = [
  { ...files, taskId: 'task', agent: 'omp', chat: null },
  { ...files, taskId: 'manager-task', agent: 'pi', chat: null, childRunId: 'child-run' },
  { ...files, kind: 'typescript', script: 'worker.ts', lifecycle: { status: 'running', startedAt: 1 } },
  { ...files, kind: 'rust_script', rustScript: 'worker', lifecycle: { status: 'running', startedAt: 1 } },
]

describe('frozen execution zone display', () => {
  it('shows an unresolved dynamic zone before dispatch without substituting the manager directory', () => {
    render(<RunCurrentView run={runWith(null)} reports={[]} current={current} managed />)
    const facts = within(screen.getByRole('region', { name: '当前执行事实' }))
    expect(facts.getByText(/动态 zone 尚未指定/)).toBeVisible()
    expect(facts.queryByText('C:/manager')).not.toBeInTheDocument()
  })

  it.each(executions)('shows saved ID and actual directory for each execution kind: $id $kind $childRunId', execution => {
    const run = runWith(execution)
    run.nodes.A.previousExecutions = [{ execution: { ...execution, id: 'old', zoneId: 'team/old', zone: 'C:/old-directory' }, failure: null }]
    render(<RunCurrentView run={run} reports={[]} current={current} managed />)
    const facts = within(screen.getByRole('region', { name: '当前执行事实' }))
    expect(facts.getByText('team/selected')).toBeVisible()
    expect(facts.getByText('C:/frozen-actual-directory')).toBeVisible()
    expect(facts.getByText('zone（实际目录）')).toBeVisible()
    expect(facts.queryByText(/动态 zone 尚未指定/)).not.toBeInTheDocument()
    const history = within(screen.getByLabelText('已保存旧执行事实'))
    expect(history.getByText('team/old')).toBeInTheDocument()
    expect(history.getByText('C:/old-directory')).toBeInTheDocument()
  })

  it('does not infer a missing historical zone ID from the current definition', () => {
    const execution = { ...executions[0], zoneId: undefined }
    render(<RunCurrentView run={runWith(execution)} reports={[]} current={current} managed />)
    const facts = within(screen.getByRole('region', { name: '当前执行事实' }))
    expect(facts.getByText('未记录（不使用当前 Map 补造）')).toBeVisible()
    expect(facts.getByText('C:/frozen-actual-directory')).toBeVisible()
  })
})
