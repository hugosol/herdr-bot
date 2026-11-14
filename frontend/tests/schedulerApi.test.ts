import { afterEach, describe, expect, it, vi } from 'vitest'
import { loadMap, loadMapInputs, saveMapInputs, saveNode } from '../src/scheduler/controller'

afterEach(() => vi.unstubAllGlobals())

const node = { title: 'Consumer', bot_id: 'worker', input: 'untouched input', zone: '', blocked_by: [] }
const draft = { id: 'B', content: '{}', revision: null, create: true }
function responseWith(fields: Record<string, unknown>) {
  vi.stubGlobal('fetch', vi.fn(async () => new Response(JSON.stringify({ title: 'Map', description: '', nodes: {
    A: { ...node, output_rule: [{ id: 'material' }] },
    B: { ...node, ...fields },
  } }))))
}

describe('Map input preparation contract', () => {
  const revision = { file: 'map.json', original: '{"title":"Map"}' }
  const input_requirements = '# 材料\n\n保留 **完整** Markdown 和尾随空白。  \n'
  const input_files = [{ id: 'brief', required: true, description: '业务委托及约束。' }]

  it('loads and saves prose separately from file declarations without losing either', async () => {
    const fetcher = vi.fn().mockResolvedValueOnce(new Response(JSON.stringify({ input_requirements, input_files, revision })))
      .mockResolvedValueOnce(new Response(JSON.stringify({ title: 'Map', description: '', nodes: {}, input_requirements, input_files })))
    vi.stubGlobal('fetch', fetcher)
    const loaded = await loadMapInputs('map.json')
    expect(loaded).toEqual({ input_requirements, input_files, revision })
    const saved = await saveMapInputs('map.json', loaded)
    expect(saved.input_requirements).toBe(input_requirements)
    expect(saved.input_files).toEqual(input_files)
    expect(fetcher.mock.calls[1]).toEqual(['/api/scheduler/maps/map.json/inputs', {
      method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(loaded),
    }])
  })

  it.each([null, 7, [], {}])('rejects malformed requirements: %j', async input_requirements => {
    vi.stubGlobal('fetch', vi.fn(async () => new Response(JSON.stringify({ title: 'Map', description: '', nodes: {}, input_requirements, input_files, revision }))))
    await expect(loadMapInputs('map.json')).rejects.toThrow('响应无效')
    await expect(loadMap('map.json')).rejects.toThrow('响应无效')
    await expect(saveMapInputs('map.json', { input_requirements: '', input_files, revision })).rejects.toThrow('响应无效')
  })
})

describe('node save response handoff schema boundary', () => {
  it('accepts array rules without changing opaque input text or adding dependencies', async () => {
    const input_rule = [{ id: 'source', type: 'node', node_id: 'A', output_id: 'material' }]
    responseWith({ input_rule, output_rule: [{ id: 'result' }] })
    const map = await saveNode('map.json', draft)
    expect(map.nodes.B).toEqual({ ...node, input_rule, output_rule: [{ id: 'result' }] })
  })

  it('accepts portable relative files with explicit or manager-default zones', async () => {
    const input_rule = [
      { id: 'default', type: 'file', path: 'docs/input.txt' },
      { id: 'explicit', type: 'file', path: 'other/input.txt', zone_id: 'team/materials' },
    ]
    responseWith({ input_rule })
    const map = await saveNode('map.json', draft)
    expect(map.nodes.B.input_rule).toEqual(input_rule)
  })

  // Public save API: rejecting the new valid source would hide a successfully
  // persisted definition; accepting run_id would allow an unsupported source.
  it('accepts a local name for this Run input without changing dependencies', async () => {
    const input_rule = [{ id: 'local', type: 'run', input_id: 'brief' }]
    responseWith({ input_rule })
    const map = await saveNode('map.json', draft)
    expect(map.nodes.B).toEqual({ ...node, input_rule })
  })

  it.each([
    { input_rule: [{ id: 'local', type: 'run', input_id: 'brief', run_id: 'another-run' }] },
    { input_rule: [{ id: 'local', type: 'run' }] },
    { input_rule: [{ id: 'source', type: 'file', path: 'input.txt', zone_id: 3 }] },
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
