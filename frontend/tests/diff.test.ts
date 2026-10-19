import { describe, expect, it } from 'vitest'

import { diffRows, splitLines } from '../src/diff'

describe('diff 行结束符', () => {
  it('CRLF 与 LF 的相同内容不产生行级差异', () => {
    expect(splitLines('alpha\r\nbeta\r\n')).toEqual(['alpha', 'beta'])
    expect(diffRows('alpha\r\nbeta\r\n', 'alpha\nbeta\n')).toEqual([
      { kind: 'context', text: 'alpha', oldNumber: 1, newNumber: 1 },
      { kind: 'context', text: 'beta', oldNumber: 2, newNumber: 2 },
    ])
  })

  it('CRLF 文件中真实的局部修改仍只标记该处', () => {
    expect(diffRows('alpha\r\nold\r\nomega\r\n', 'alpha\r\nnew\r\nomega\r\n')).toEqual([
      { kind: 'context', text: 'alpha', oldNumber: 1, newNumber: 1 },
      { kind: 'removed', text: 'old', oldNumber: 2, newNumber: null },
      { kind: 'added', text: 'new', oldNumber: null, newNumber: 2 },
      { kind: 'context', text: 'omega', oldNumber: 3, newNumber: 3 },
    ])
  })
})
