import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { DiffContent } from '../src/DiffContent'
import type { ChangeDiff } from '../src/diffApi'

function diff(oldLines: string[], newLines: string[]): ChangeDiff {
  return {
    zone: 'test', path: 'file.txt', complete: true,
    old: { kind: 'text', content: oldLines.join('\n'), size: 0 },
    new: { kind: 'text', content: newLines.join('\n'), size: 0 },
  }
}

const lines = Array.from({ length: 40 }, (_, i) => `line-${i + 1}`)

describe.each([false, true])('上下文折叠 narrow=%s', narrow => {
  it('只隐藏上下文，保留真实行号，独立展开与收起', async () => {
    const user = userEvent.setup()
    const changed = [...lines]
    changed[10] = 'changed-11'
    changed[29] = 'changed-30'
    const { container } = render(<DiffContent diff={diff(lines, changed)} narrow={narrow} />)
    expect(screen.queryAllByText('line-1', { exact: true })).toHaveLength(0)
    expect(screen.getAllByText('line-8', { exact: true }).length).toBeGreaterThan(0)
    expect(screen.getByText('changed-11')).toBeVisible()
    expect(screen.getByText('changed-30')).toBeVisible()
    expect(screen.getByRole('button', { name: '已隐藏 12 行 · 展开' })).toBeVisible()
    await user.click(screen.getAllByRole('button', { name: '已隐藏 7 行 · 展开' })[0])
    expect(screen.getAllByText('line-1', { exact: true }).length).toBeGreaterThan(0)
    expect(screen.queryAllByText('line-20', { exact: true })).toHaveLength(0)
    await user.click(screen.getByRole('button', { name: '收起 7 行未修改内容' }))
    expect(screen.queryAllByText('line-1', { exact: true })).toHaveLength(0)
    const changedRow = screen.getByText('changed-30').parentElement!
    expect(Array.from(changedRow.querySelectorAll('.diff-line__number'), cell => cell.textContent)).toContain('30')
    expect(container.querySelectorAll('.diff-context-toggle')).toHaveLength(3)
  })

  it('相接的上下文无多余折叠，新增删除不丢行，相同内容不展示全文', () => {
    const old = lines.slice(0, 8)
    const changed = [...old]
    changed[0] = 'first'
    changed[7] = 'last'
    const { rerender } = render(<DiffContent diff={diff(old, changed)} narrow={narrow} />)
    expect(screen.queryByRole('button')).toBeNull()
    expect(screen.getAllByText('line-4', { exact: true }).length).toBeGreaterThan(0)
    rerender(<DiffContent diff={diff([], old)} narrow={narrow} />)
    for (const line of old) expect(screen.getByText(line, { exact: true })).toBeVisible()
    expect(screen.queryByRole('button')).toBeNull()
    rerender(<DiffContent diff={diff(old, [])} narrow={narrow} />)
    for (const line of old) expect(screen.getByText(line, { exact: true })).toBeVisible()
    rerender(<DiffContent diff={diff(old, old)} narrow={narrow} />)
    expect(screen.getByText('两侧内容相同，没有行级差异。')).toBeVisible()
    expect(screen.queryByText('line-1', { exact: true })).toBeNull()
  })
})
