import { describe, expect, it } from 'vitest'
import { renderChatNameExample } from '../../src/tasks/chatNaming'

describe('Task Chat 名称示例', () => {
  it('Task 管理的三参数示例不变，真实上下文保留原值与未知字段', () => {
    const rule = '{zone.name}/{task.name}/{agent}/{chat.id}/{unknown}'
    expect(renderChatNameExample(rule, '  审查  ', 'omp')).toBe('alpha/审查/omp/a7f3c2/{unknown}')
    expect(renderChatNameExample(rule, '  审查  ', 'pi', { zoneName: '实际目录', chatId: 'd4e5f6' })).toBe('实际目录/  审查  /pi/d4e5f6/{unknown}')
  })

})
