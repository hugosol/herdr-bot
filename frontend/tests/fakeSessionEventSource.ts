import type { SessionEventSource } from '../src/sseSessionFeed'

/**
 * 假 `EventSource`：jsdom 里没有真的 `EventSource`，这里按同一形状（命名事件 + `data:` 载荷）手动派发
 * 帧。接线测试因此不依赖真服务端，只见证页面消费帧的路径；组件若绕过数据源契约直接碰全局
 * `EventSource`，这些用例会当场失败。
 */
type FrameListener = (event: { data: string }) => void

export class FakeSessionEventSource implements SessionEventSource {
  static instances: FakeSessionEventSource[] = []

  private readonly listeners = new Map<string, FrameListener[]>()
  closed = false

  constructor(readonly url: string) {
    FakeSessionEventSource.instances.push(this)
  }

  addEventListener(type: string, listener: FrameListener): void {
    this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener])
  }

  close(): void {
    this.closed = true
  }

  /** 派发一帧命名事件（服务端的 `event: <name>` + `data: <json>`）。 */
  emit(type: string, data: unknown): void {
    this.dispatch(type, JSON.stringify(data))
  }

  /** 派发一帧原样字节：用来驱动解析不了的坏帧。 */
  emitRaw(type: string, data: string): void {
    this.dispatch(type, data)
  }

  /** 一次连接错误：浏览器会按服务端给的 `retry` 自行重连。 */
  fail(): void {
    this.dispatch('error', '')
  }

  /** 一次（重）连成功。 */
  open(): void {
    this.dispatch('open', '')
  }

  private dispatch(type: string, data: string): void {
    for (const listener of this.listeners.get(type) ?? []) listener({ data })
  }
}

/** 每条用例都从零开始数连接。 */
export function resetFakeEventSources(): void {
  FakeSessionEventSource.instances = []
}
