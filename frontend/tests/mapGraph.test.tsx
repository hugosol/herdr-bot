import { render, screen, within } from '@testing-library/react'
import { expect, it, vi } from 'vitest'
import { MapGraph } from '../src/MapGraph'
import { WorkflowRunGraph } from '../src/WorkflowRunGraph'
import type { MapNode, WorkflowMap } from '../src/schedulerApi'

const node = (title: string, blocked_by: string[] = []): MapNode => ({ title, blocked_by, bot_id: 'shared', input: '', zone: 'workspace' })
const map: WorkflowMap = {
  title: 'Unordered dependencies', description: '',
  nodes: {
    finish: node('Finish', ['join', 'branch']),
    join: node('Join', ['alpha', 'beta']),
    branch: node('Branch', ['beta']),
    beta: node('Beta'),
    alpha: node('Alpha'),
    independent: node('Independent'),
  },
}

it('renders dependency layers before dependents and keeps parallel definition order rather than traversal discovery order', () => {
  render(<MapGraph map={map} selectedId={null} onSelect={vi.fn()} />)
  const graph = screen.getByRole('group', { name: 'Map 依赖图' })
  const buttons = within(graph).getAllByRole('button')
  expect(buttons.map(button => button.getAttribute('aria-label'))).toEqual([
    'Beta beta', 'Alpha alpha', 'Independent independent', 'Join join', 'Branch branch', 'Finish finish',
  ])
  const position = (name: string) => {
    const frame = within(graph).getByRole('button', { name }).closest('foreignObject')!
    return { x: Number(frame.getAttribute('x')), y: Number(frame.getAttribute('y')) }
  }
  const beta = position('Beta beta'), alpha = position('Alpha alpha'), independent = position('Independent independent')
  const join = position('Join join'), branch = position('Branch branch'), finish = position('Finish finish')
  expect(beta.x).toBe(alpha.x)
  expect(alpha.x).toBe(independent.x)
  expect(beta.y).toBeLessThan(alpha.y)
  expect(alpha.y).toBeLessThan(independent.y)
  expect(beta.x).toBeLessThan(join.x)
  expect(join.x).toBe(branch.x)
  expect(join.y).toBeLessThan(branch.y)
  expect(branch.x).toBeLessThan(finish.x)
})

it('shows full Task names and keeps running arrows separate from selection, with animation opt-in', () => {
  const task = 'A complete Task name that remains readable even when its visual label wraps or truncates'
  const props = {
    map, selectedId: 'beta', onSelect: vi.fn(), nodeTasks: { beta: task }, nodeBots: { beta: 'Shared Bot' }, nodeReportPolicies: {}, animateRunning: false,
    nodeStates: { beta: { status: 'running', label: '运行中' }, alpha: { status: 'completed', label: '已完成' } },
  }
  const { rerender } = render(<WorkflowRunGraph {...props} />)
  const running = screen.getByRole('button', { name: 'Beta beta' })
  const completed = screen.getByRole('button', { name: 'Alpha alpha' })
  expect(running).toHaveAttribute('aria-pressed', 'true')
  expect(running).toHaveTextContent(task)
  expect(running).toHaveTextContent('运行中')
  const arrow = running.querySelector('.workflow-node-running-arrow')!
  expect(arrow).toHaveAttribute('aria-hidden', 'true')
  expect(arrow).not.toHaveAttribute('data-animate')
  expect(completed.querySelector('.workflow-node-running-arrow')).toBeNull()
  rerender(<WorkflowRunGraph {...props} selectedId="alpha" animateRunning />)
  expect(running).toHaveAttribute('aria-pressed', 'false')
  expect(arrow).toHaveAttribute('data-animate', 'true')
  expect(completed).toHaveAttribute('aria-pressed', 'true')
  expect(completed.querySelector('.workflow-node-running-arrow')).toBeNull()
  rerender(<WorkflowRunGraph {...props} selectedId="join" animateRunning={false} />)
  expect(arrow).not.toHaveAttribute('data-animate')
  expect(running).toHaveTextContent(task)
  expect(running).toHaveTextContent('运行中')
})
