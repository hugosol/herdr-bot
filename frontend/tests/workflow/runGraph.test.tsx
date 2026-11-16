import { render, screen } from '@testing-library/react'
import { expect, it, vi } from 'vitest'
import { WorkflowRunGraph } from '../../src/workflow/WorkflowRunGraph'
import { map } from '../support/unorderedMap'

it('shows full Task names and keeps running arrows separate from selection, with animation opt-in', () => {
  const task = 'A complete Task name that remains readable even when its visual label wraps or truncates'
  const props = {
    map, selectedId: 'beta', onSelect: vi.fn(), nodeTargets: { beta: { label: 'Task', name: task } }, nodeBots: { beta: 'Shared Bot' }, nodeReportPolicies: {}, animateRunning: false,
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
