// 让 Vitest 的 expect 获得 @testing-library/jest-dom 的 DOM 断言（toBeInTheDocument 等）。
// `globals: true` 下 RTL 会在每个测试后用全局 afterEach 自动 cleanup。
import '@testing-library/jest-dom/vitest'

// jsdom 不实现 dialog 的顶层显示；组件测试只模拟 open 状态。
// 焦点隔离、Escape 与布局由真实 Chromium 验收，不由此模拟证明。
if (typeof HTMLDialogElement !== 'undefined' && !HTMLDialogElement.prototype.showModal) {
  HTMLDialogElement.prototype.showModal = function () { this.open = true }
  HTMLDialogElement.prototype.close = function (returnValue = '') {
    this.open = false
    this.returnValue = returnValue
    this.dispatchEvent(new Event('close'))
  }
}
