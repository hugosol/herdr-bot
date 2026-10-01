// 让 Vitest 的 expect 获得 @testing-library/jest-dom 的 DOM 断言（toBeInTheDocument 等）。
// `globals: true` 下 RTL 会在每个测试后用全局 afterEach 自动 cleanup。
import '@testing-library/jest-dom/vitest'
