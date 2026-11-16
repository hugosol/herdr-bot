import { useState } from 'react'
import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { NodeSettings, type NodeSettingsValue } from '../../src/scheduler/maps/NodeSettings'

function Editor() {
  const [value, setValue] = useState<NodeSettingsValue>({ zone: 'team/preset', report_policy: 'manual', auto_close: true })
  return <><NodeSettings value={value} disabled={false} onChange={patch => setValue(current => ({ ...current, ...patch }))} /><output aria-label="settings payload">{JSON.stringify(value)}</output></>
}

describe('node zone editor', () => {
  it('switches to a preset-free dynamic declaration and never restores a discarded fixed ID', () => {
    render(<Editor />)
    expect(screen.getByRole('radio', { name: '固定 zone' })).toBeChecked()
    expect(screen.getByRole('textbox', { name: 'zone' })).toHaveValue('team/preset')
    fireEvent.click(screen.getByRole('radio', { name: '动态 zone' }))
    expect(screen.queryByRole('textbox', { name: 'zone' })).not.toBeInTheDocument()
    expect(JSON.parse(screen.getByLabelText('settings payload').textContent!)).toEqual({ zone: { type: 'dynamic' }, report_policy: 'manual', auto_close: true })
    fireEvent.click(screen.getByRole('radio', { name: '固定 zone' }))
    expect(screen.getByRole('textbox', { name: 'zone' })).toHaveValue('')
    fireEvent.change(screen.getByRole('textbox', { name: 'zone' }), { target: { value: 'other/location' } })
    expect(JSON.parse(screen.getByLabelText('settings payload').textContent!).zone).toBe('other/location')
  })

  it('renders dynamic declarations read-only without treating them as a directory', () => {
    render(<NodeSettings readOnly value={{ zone: { type: 'dynamic' }, report_policy: 'auto', auto_close: false }} />)
    expect(screen.getByText('dynamic · 每次执行时指定，尚未指定 zone')).toBeVisible()
    expect(screen.queryByRole('radio')).not.toBeInTheDocument()
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument()
  })
})
