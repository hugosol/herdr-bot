import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, expect, it, vi } from 'vitest'
import { MapRulesEditor } from '../../src/scheduler/maps/MapRulesEditor'

afterEach(() => vi.unstubAllGlobals())

it('preserves Markdown and file declarations in the same revisioned save', async () => {
  const map_input_rule = { description: '# 原材料\n\n保留 **格式**。  \n', files: [{ id: 'brief', required: true, description: '业务委托及约束。' }] }
  const revision = { file: 'map.json', original: '{"title":"Map"}' }
  const fetcher = vi.fn().mockResolvedValueOnce(new Response(JSON.stringify({ map_input_rule, revision })))
    .mockResolvedValueOnce(new Response(JSON.stringify({ title: 'Map', description: '', nodes: {}, map_input_rule })))
  vi.stubGlobal('fetch', fetcher)
  const onSaved = vi.fn()
  render(<MapRulesEditor file="map.json" onClose={vi.fn()} onSaved={onSaved} />)
  const prose = screen.getByRole('textbox', { name: 'Map 输入要求 Markdown' })
  await waitFor(() => expect(prose).toHaveValue(map_input_rule.description))
  expect(screen.getByRole('textbox', { name: 'Map 文件输入声明 JSON' })).toHaveValue(JSON.stringify(map_input_rule.files, null, 2))
  const edited = `${map_input_rule.description}\n补充说明。`
  fireEvent.change(prose, { target: { value: edited } })
  fireEvent.click(screen.getByRole('button', { name: '保存公开规则' }))
  await waitFor(() => expect(onSaved).toHaveBeenCalledOnce())
  expect(JSON.parse(String(fetcher.mock.calls[1][1].body))).toEqual({ map_input_rule: { ...map_input_rule, description: edited }, revision })
})
