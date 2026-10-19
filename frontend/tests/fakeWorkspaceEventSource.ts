/**
 * 测试用的 `EventSource` 假实现：只提供 workspace 自动更新数据源真正用到的成员。
 *
 * 它不模拟网络、重连或时序；用例自己决定何时送来哪一帧，因此页面行为与真实 `EventSource`
 * 的接线可以由同一段数据源代码驱动。
 */
export type FrameEvent = { data: string }

export class FakeWorkspaceEventSource {
  static instances: FakeWorkspaceEventSource[] = []

  readonly url: string
  closed = false
  private listeners = new Map<string, Array<(event: FrameEvent) => void>>()

  constructor(url: string) {
    this.url = url
    FakeWorkspaceEventSource.instances.push(this)
  }

  addEventListener(type: string, listener: (event: FrameEvent) => void): void {
    const listeners = this.listeners.get(type) ?? []
    listeners.push(listener)
    this.listeners.set(type, listeners)
  }

  close(): void {
    this.closed = true
  }

  emit(type: string, payload: unknown): void {
    const data = typeof payload === 'string' ? payload : JSON.stringify(payload)
    for (const listener of this.listeners.get(type) ?? []) listener({ data })
  }
}

export function resetFakeWorkspaceEventSources(): void {
  FakeWorkspaceEventSource.instances = []
}
