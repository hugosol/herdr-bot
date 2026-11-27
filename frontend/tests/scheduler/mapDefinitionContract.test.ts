import { afterEach, describe, expect, it, vi } from 'vitest'
import { loadMap, loadMapRules, saveMapRules, saveNode } from '../../src/controllers/schedulerController'

afterEach(() => vi.unstubAllGlobals())

const node = { title: 'Consumer', bot_id: 'worker', input: 'untouched input', zone: '', blocked_by: [] }
const draft = { id: 'B', content: '{}', revision: null, create: true }
function responseWith(fields: Record<string, unknown>) {
  vi.stubGlobal('fetch', vi.fn(async () => new Response(JSON.stringify({ title: 'Map', description: '', nodes: {
    A: { ...node, output_rule: [{ id: 'material' }] },
    B: { ...node, ...fields },
  } }))))
}

describe('Map public rule response contract', () => {
  const revision = { file: 'map.json', original: '{"title":"Map"}' }
  const map_input_rule = { description: '# 材料\n\n保留 **完整** Markdown 和尾随空白。  \n', files: [{ id: 'brief', required: true, description: '业务委托及约束。' }] }
  const map_output_rule = { description: '只说明交接条件' }

  it('loads and saves the rule prose and files without inventing an output file', async () => {
    const fetcher = vi.fn().mockResolvedValueOnce(new Response(JSON.stringify({ map_input_rule, map_output_rule, revision })))
      .mockResolvedValueOnce(new Response(JSON.stringify({ title: 'Map', description: '', nodes: {}, map_input_rule, map_output_rule })))
    vi.stubGlobal('fetch', fetcher)
    const loaded = await loadMapRules('map.json')
    expect(loaded).toEqual({ map_input_rule, map_output_rule, revision })
    const saved = await saveMapRules('map.json', loaded)
    expect(saved.map_input_rule).toEqual(map_input_rule)
    expect(saved.map_output_rule).toEqual(map_output_rule)
    expect(fetcher.mock.calls[1]).toEqual(['/api/scheduler/maps/map.json/inputs', {
      method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(loaded),
    }])
  })

  it.each([null, 7, [], {}])('rejects malformed description: %j', async description => {
    vi.stubGlobal('fetch', vi.fn(async () => new Response(JSON.stringify({ title: 'Map', description: '', nodes: {}, map_input_rule: { description }, revision }))))
    await expect(loadMapRules('map.json')).rejects.toThrow('响应无效')
    await expect(loadMap('map.json')).rejects.toThrow('响应无效')
    await expect(saveMapRules('map.json', { map_input_rule, revision })).rejects.toThrow('响应无效')
  })
})

describe('node zone schema boundary', () => {
  const input_rule = [{ id: 'registered-repo', type: 'node', node_id: 'A', output_id: 'material' }]

  it.each(['team/work', { type: 'dynamic' }, { type: 'dynamic', input_id: 'registered-repo' }])('loads and confirms the exact zone representation: %j', async zone => {
    responseWith({ zone, input_rule, auto_approved: false, auto_close: true })
    expect((await loadMap('map.json')).nodes.B).toEqual({ ...node, zone, input_rule, auto_approved: false, auto_close: true })
    expect((await saveNode('map.json', { ...draft, content: JSON.stringify({ ...node, zone, input_rule }) })).nodes.B.zone).toEqual(zone)
    const request = vi.mocked(fetch).mock.calls[1][1]!
    expect(JSON.parse(JSON.parse(request.body as string).content).zone).toEqual(zone)
  })

  it.each([
    null, [], {}, { type: 'fixed' }, { type: 'dynamic', default: 'team/work' }, { type: 'dynamic', value: 'team/work' },
    { type: 'dynamic', input_id: null }, { type: 'dynamic', input_id: '' }, { type: 'dynamic', input_id: '   ' },
    { type: 'dynamic', input_id: 'material' }, { type: 'dynamic', input_id: 'registered-repo', default: 'team/work' },
  ])('rejects invalid or preset-bearing zone objects: %j', async zone => {
    responseWith({ zone, input_rule })
    await expect(loadMap('map.json')).rejects.toThrow('响应无效')
    await expect(saveNode('map.json', draft)).rejects.toThrow('响应无效')
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
