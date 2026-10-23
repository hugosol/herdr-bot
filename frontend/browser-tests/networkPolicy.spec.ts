import { expect, test } from '@playwright/test'
import { startServer } from './support'

// These fail before binary discovery, temporary files, port probes or process startup.
// No browser fixture is requested, so this guard can run without Chrome or built artifacts.
for (const listenAddr of [
  '0.0.0.0:0',
  '192.168.1.2:0',
  '10.0.0.1:0',
  '[::]:0',
  '[2001:db8::1]:0',
  '[::ffff:127.0.0.1]:0',
  'localhost:0',
  '127.1:0',
  '127.0.0.1.example:0',
  '127.0.0.1',
  '127.0.0.1:65536',
  '[::1]:65536',
]) {
  test(`ordinary server rejects ${listenAddr} before opening sockets`, async () => {
    await expect(startServer({ listenAddr })).rejects.toThrow('普通浏览器测试只允许数值回环监听地址')
  })
}
