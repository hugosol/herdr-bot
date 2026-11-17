import { useState } from 'react'
import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { NodeSettings, type NodeSettingsValue } from '../../src/scheduler/maps/NodeSettings'

function Editor() {
  const [value, setValue] = useState<NodeSettingsValue>({ zone: 'team/preset', auto_approved: false, auto_close: true })
  return <><NodeSettings value={value} disabled={false} onChange={patch => setValue(current => ({ ...current, ...patch }))} /><output aria-label="settings payload">{JSON.stringify(value)}</output></>
}

describe('node zone editor', () => {
  it('switches to a preset-free dynamic declaration and never restores a discarded fixed ID', () => {
    render(<Editor />)
    expect(screen.getByRole('radio', { name: '固定 zone' })).toBeChecked()
    expect(screen.getByRole('textbox', { name: 'zone' })).toHaveValue('team/preset')
    fireEvent.click(screen.getByRole('radio', { name: '动态 zone' }))
    expect(screen.queryByRole('textbox', { name: 'zone' })).not.toBeInTheDocument()
    expect(JSON.parse(screen.getByLabelText('settings payload').textContent!)).toEqual({ zone: { type: 'dynamic' }, auto_approved: false, auto_close: true })
    fireEvent.click(screen.getByRole('radio', { name: '固定 zone' }))
    expect(screen.getByRole('textbox', { name: 'zone' })).toHaveValue('')
    fireEvent.change(screen.getByRole('textbox', { name: 'zone' }), { target: { value: 'other/location' } })
    expect(JSON.parse(screen.getByLabelText('settings payload').textContent!).zone).toBe('other/location')
  })

  it('preserves ordinary policies while enabling and disabling the end node', () => {
    render(<Editor />)
    fireEvent.click(screen.getByRole('checkbox', { name: 'end_node' }))
    expect(screen.queryByRole('checkbox', { name: 'auto_close' })).not.toBeInTheDocument()
    expect(screen.queryByRole('radio', { name: 'manual' })).not.toBeInTheDocument()
    expect(JSON.parse(screen.getByLabelText('settings payload').textContent!)).toEqual({ zone: 'team/preset', auto_approved: false, auto_close: true, end_node: true })
    fireEvent.click(screen.getByRole('checkbox', { name: 'end_node' }))
    expect(screen.getByRole('checkbox', { name: 'auto_close' })).toBeChecked()
    expect(screen.getByRole('radio', { name: 'manual' })).toBeChecked()
  })

  it('disables an ineligible end-node choice but allows explicitly removing an invalid saved flag', () => {
    const onChange = () => {}
    const value = { zone: '', auto_approved: true as const, auto_close: false }
    const view = render(<NodeSettings value={value} disabled={false} endNodeProblem="不允许脚本" onChange={onChange} />)
    expect(screen.getByRole('checkbox', { name: 'end_node' })).toBeDisabled()
    view.rerender(<NodeSettings value={{ ...value, end_node: true }} disabled={false} endNodeProblem="不允许脚本" onChange={onChange} />)
    expect(screen.getByRole('checkbox', { name: 'end_node' })).toBeEnabled()
  })

  it('renders dynamic declarations read-only without treating them as a directory', () => {
    render(<NodeSettings readOnly value={{ zone: { type: 'dynamic' }, auto_approved: true, auto_close: false }} />)
    expect(screen.getByText('dynamic · 每次执行时指定，尚未指定 zone')).toBeVisible()
    expect(screen.queryByRole('radio')).not.toBeInTheDocument()
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument()
  })
})
