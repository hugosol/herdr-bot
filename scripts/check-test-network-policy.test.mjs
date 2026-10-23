import assert from 'node:assert/strict'
import test from 'node:test'
import { inspect } from './check-test-network-policy.mjs'

test('new tests cannot bind sockets directly or hide them in an exempt directory', () => {
  for (const file of ['backend/tests/new_case.rs', 'backend/tests/common/other.rs']) {
    assert.match(inspect(file, 'async fn case() { tokio::net::TcpListener::bind(address).await; }').join('\n'), /Direct network listener/)
  }
  assert.match(inspect('frontend/browser-tests/new.spec.ts', 'test("case", () => { net.createServer().listen(port, host) })').join('\n'), /Direct network listener/)
})

test('renamed imports cannot bypass listener checks', () => {
  assert.match(inspect('backend/tests/new_case.rs', 'use tokio::net::TcpListener as Listener; async fn case() { Listener::bind(address); }').join('\n'), /Listener imports/)
  assert.match(inspect('frontend/tests/new.test.ts', 'import { Server as Hidden } from "node:net"; new Hidden()').join('\n'), /Network modules/)
})

test('registered infrastructure exception does not cover another function', () => {
  assert.deepEqual(inspect('backend/tests/common/mod.rs', 'async fn bind_app_on() { TcpListener::bind(address); }'), [])
  assert.match(inspect('backend/tests/common/mod.rs', 'async fn accidental() { TcpListener::bind(address); }').join('\n'), /Direct network listener/)
})

test('manual LAN tests cannot silently lose their ignore attribute', () => {
  const file = 'backend/tests/session_write_boundary.rs'
  const body = 'async fn same_host_access_via_local_nic_address_is_accepted() { TestServer::start_manual_app_on(address, app).await; }'
  assert.match(inspect(file, '#[tokio::test]\n' + body).join('\n'), /must remain ignored/)
  assert.deepEqual(inspect(file, '#[tokio::test]\n#[ignore = "manual"]\n' + body), [])
})

test('ordinary cases cannot call manual helpers', () => {
  assert.match(inspect('backend/tests/new_case.rs', 'async fn case() { TestServer::start_manual_app_on(address, app); }').join('\n'), /Manual network helper/)
  assert.match(inspect('frontend/browser-tests/new.spec.ts', 'test("case", async () => { await startManualServer() })').join('\n'), /Manual network helper/)
})

test('mDNS cases stay opt-in and comments do not count as listeners', () => {
  const file = 'backend/tests/mdns_failure.rs'
  assert.match(inspect(file, '#[test]\nfn case() {}').join('\n'), /must be ignored/)
  assert.deepEqual(inspect(file, '#[test]\n#[ignore]\nfn case() { /* TcpListener::bind(address) */ }'), [])
})
