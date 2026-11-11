import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { MapInputsEditor } from '../src/scheduler/MapInputsEditor'

afterEach(() => vi.unstubAllGlobals())

it('preserves Markdown and file declarations in the same revisioned save', async () => {
  const input_requirements = '# 原材料\n\n保留 **格式**。  \n'
  const input_files = [{ id: 'brief', required: true }]
  const revision = { file: 'map.json', original: '{"title":"Map"}' }
  const fetcher = vi.fn().mockResolvedValueOnce(new Response(JSON.stringify({ input_requirements, input_files, revision })))
    .mockResolvedValueOnce(new Response(JSON.stringify({ title: 'Map', description: '', nodes: {}, input_requirements, input_files })))
  vi.stubGlobal('fetch', fetcher)
  const onSaved = vi.fn()
  render(<MapInputsEditor file="map.json" onClose={vi.fn()} onSaved={onSaved} />)
  const prose = screen.getByRole('textbox', { name: 'Map 输入要求 Markdown' })
  await waitFor(() => expect(prose).toHaveValue(input_requirements))
  expect(screen.getByRole('textbox', { name: 'Map 文件输入声明 JSON' })).toHaveValue(JSON.stringify(input_files, null, 2))
  const edited = `${input_requirements}\n补充说明。`
  fireEvent.change(prose, { target: { value: edited } })
  fireEvent.click(screen.getByRole('button', { name: '保存输入声明' }))
  await waitFor(() => expect(onSaved).toHaveBeenCalledOnce())
  expect(JSON.parse(String(fetcher.mock.calls[1][1].body))).toEqual({ input_requirements: edited, input_files, revision })
})
