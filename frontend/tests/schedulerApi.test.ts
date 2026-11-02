import { afterEach, describe, expect, it, vi } from 'vitest'
import { saveNode } from '../src/scheduler/schedulerApi'

afterEach(() => vi.unstubAllGlobals())

const node = { title: 'Consumer', bot_id: 'worker', input: 'untouched input', zone: '', blocked_by: [] }
const draft = { id: 'B', content: '{}', revision: null, create: true }
function responseWith(fields: Record<string, unknown>) {
  vi.stubGlobal('fetch', vi.fn(async () => new Response(JSON.stringify({ title: 'Map', description: '', nodes: {
    A: { ...node, output_rule: [{ id: 'material' }] },
    B: { ...node, ...fields },
  } }))))
}

describe('node save response handoff schema boundary', () => {
  it('accepts array rules without changing opaque input text or adding dependencies', async () => {
    const input_rule = [{ id: 'source', type: 'node', node_id: 'A', output_id: 'material' }]
    responseWith({ input_rule, output_rule: [{ id: 'result' }] })
    const map = await saveNode('map.json', draft)
    expect(map.nodes.B).toEqual({ ...node, input_rule, output_rule: [{ id: 'result' }] })
  })

  it.each([
    { inputs: { source: { source: 'file', path: 'C:/input.txt' } } },
    { input_rule: { source: { type: 'file', path: 'C:/input.txt' } } },
    { input_rule: [{ id: 'source', source: 'node', node_id: 'A', output: 'material' }] },
    { input_rule: [{ id: 'same', type: 'file', path: 'C:/one.txt' }, { id: 'same', type: 'file', path: 'C:/two.txt' }] },
    { output_rule: [{ id: 'same' }, { id: 'same' }] },
    { output_rule: [{ id: 'result', path: 'C:/not-a-rule.txt' }] },
  ])('does not confirm a save with an invalid rule response: %j', async fields => {
    responseWith(fields)
    await expect(saveNode('map.json', draft)).rejects.toThrow('响应无效')
    expect(fetch).toHaveBeenCalledTimes(1)
  })
})
