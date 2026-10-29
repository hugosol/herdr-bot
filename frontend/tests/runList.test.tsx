import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { RunList } from '../src/RunList'
import type { RunEntry } from '../src/useWorkflowRefresh'

function entry(id: string, createdAt: number, map = 'alpha.json', status = 'running', managed = true): RunEntry {
  return { id, managed, readError: null, run: { map, createdAt, status, nodes: { a: { status: 'completed' }, b: { status: 'pending' } } } }
}
const unreadable: RunEntry = { id: 'broken-run', managed: false, readError: 'invalid saved JSON', run: null }

function visibleIds() {
  return screen.queryAllByRole('row').map(row => within(row).getByRole('link').getAttribute('href'))
}

describe('Run list browsing', () => {
  it('combines all four filters instead of applying only the most recent one', () => {
    render(<RunList entries={[
      entry('match', 2000), entry('other-map', 3000, 'beta.json'),
      entry('other-state', 4000, 'alpha.json', 'completed'), entry('other-owner', 5000, 'alpha.json', 'running', false),
      entry('different-id', 6000), unreadable,
    ]} />)
    fireEvent.change(screen.getByRole('searchbox'), { target: { value: 'MATCH' } })
    fireEvent.change(screen.getByLabelText('Map', { exact: true }), { target: { value: 'alpha.json' } })
    fireEvent.change(screen.getByLabelText('状态', { exact: true }), { target: { value: 'running' } })
    fireEvent.change(screen.getByLabelText('管理归属', { exact: true }), { target: { value: 'true' } })
    expect(visibleIds()).toEqual(['#workflow/match'])
    expect(screen.getByRole('status', { name: 'Run 显示计数' })).toHaveTextContent('1 / 6')
    fireEvent.change(screen.getByRole('searchbox'), { target: { value: 'alpha' } })
    expect(visibleIds()).toEqual(['#workflow/different-id', '#workflow/match'])
    fireEvent.change(screen.getByLabelText('管理归属', { exact: true }), { target: { value: 'false' } })
    expect(visibleIds()).toEqual(['#workflow/other-owner'])
  })

  it('keeps the selected filters visible when a refresh removes their last matching record', () => {
    const { rerender } = render(<RunList entries={[entry('changing-run', 1000)]} />)
    const map = screen.getByLabelText('Map', { exact: true })
    const status = screen.getByLabelText('状态', { exact: true })
    fireEvent.change(map, { target: { value: 'alpha.json' } })
    fireEvent.change(status, { target: { value: 'running' } })
    rerender(<RunList entries={[entry('completed-run', 2000, 'beta.json', 'completed')]} />)
    expect(visibleIds()).toEqual([])
    expect(map).toHaveValue('alpha.json')
    expect(status).toHaveValue('running')
    fireEvent.change(map, { target: { value: 'beta.json' } })
    fireEvent.change(status, { target: { value: 'completed' } })
    expect(visibleIds()).toEqual(['#workflow/completed-run'])
  })

  it('sorts by valid creation time stably, retaining invalid timestamps and unreadable records', () => {
    render(<RunList entries={[
      entry('old', 0), entry('same-first', 2000), unreadable,
      entry('invalid', Number.NaN), entry('same-second', 2000), entry('newest', 3000), entry('out-of-range', 9e15),
    ]} />)
    expect(visibleIds()).toEqual([
      '#workflow/newest', '#workflow/same-first', '#workflow/same-second', '#workflow/old',
      '#workflow/broken-run', '#workflow/invalid', '#workflow/out-of-range',
    ])
    expect(screen.getByRole('alert')).toHaveTextContent('invalid saved JSON')
    const unknown = screen.getByRole('region', { name: '时间未知' })
    expect(within(unknown).getAllByRole('row')).toHaveLength(3)
    expect(screen.queryByRole('columnheader')).not.toBeInTheDocument()
    expect(within(screen.getAllByRole('row')[0]).getAllByRole('cell')).toHaveLength(3)
    expect(within(screen.getAllByRole('row')[0]).getByLabelText('交接数量')).toHaveTextContent('1/2')
  })

})
