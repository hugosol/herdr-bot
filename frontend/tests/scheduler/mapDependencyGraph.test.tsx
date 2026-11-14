import { render, screen, within } from '@testing-library/react'
import { expect, it, vi } from 'vitest'
import { MapGraph } from '../../src/scheduler/MapGraph'

import { map } from '../support/unorderedMap'

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

