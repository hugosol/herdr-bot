"""Fault-injector seam: a committed response may be held/lost, never fabricated.

Independent oracle is the external HTTP peer's committed literal. A proxy that
holds before forwarding cannot expose that commit while its caller is blocked.
"""
import importlib.util
import http.server
import json
from pathlib import Path
import tempfile
import threading
import unittest
import urllib.error
import urllib.request


class RecoveryFaults(unittest.TestCase):
    def test_holds_only_after_commit_and_can_drop_original_response(self):
        spec = importlib.util.spec_from_file_location('faults', Path(__file__).with_name('chief_recovery_faults.py'))
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        committed = []
        class Peer(http.server.BaseHTTPRequestHandler):
            def log_message(self, *_):
                pass
            def do_POST(self):
                committed.append(json.loads(self.rfile.read(int(self.headers['Content-Length']))))
                self.send_response(200)
                self.end_headers()
                self.wfile.write(b'{"received":true,"process":true}')
        server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Peer)
        serving = threading.Thread(target=server.serve_forever)
        serving.start()
        try:
            with tempfile.TemporaryDirectory() as temporary:
                proxy = module.RecoveryFaultProxy(f'http://127.0.0.1:{server.server_port}', Path(temporary))
                failures = []
                closed = False
                try:
                    gate = proxy.arm('/original/receive', drop=True)
                    def caller():
                        try:
                            urllib.request.urlopen(urllib.request.Request(proxy.base + '/original/receive',
                                data=b'{"sessionId":"native-original"}', headers={'Content-Type': 'application/json'}), timeout=5)
                        except Exception as error:
                            failures.append(type(error).__name__)
                    thread = threading.Thread(target=caller)
                    thread.start()
                    self.assertTrue(gate.committed.wait(3))
                    self.assertEqual(committed, [{'sessionId': 'native-original'}])
                    self.assertTrue(thread.is_alive())
                    self.assertEqual(json.loads(gate.record['response']), {'received': True, 'process': True})
                    gate.release.set()
                    thread.join(3)
                    self.assertFalse(thread.is_alive())
                    self.assertEqual(len(failures), 1)
                    self.assertEqual(len(committed), 1)
                    # Cleanup must release a still-held *original* response and
                    # await its handler before evidence is considered settled.
                    waiting = proxy.arm('/original/receive')
                    delivered = []
                    def held_caller():
                        with urllib.request.urlopen(urllib.request.Request(proxy.base + '/original/receive',
                            data=b'{"sessionId":"native-restored"}', headers={'Content-Type': 'application/json'}), timeout=5) as response:
                            delivered.append(json.load(response))
                    second = threading.Thread(target=held_caller)
                    second.start()
                    self.assertTrue(waiting.committed.wait(3))
                    self.assertEqual(delivered, [])
                    proxy.close()
                    closed = True
                    second.join(3)
                    self.assertFalse(second.is_alive())
                    self.assertEqual(delivered, [{'received': True, 'process': True}])
                    index = json.loads((Path(temporary) / 'index.json').read_text(encoding='utf-8'))
                    self.assertEqual(len(index), 2)
                    self.assertTrue(all('ended' in record for record in index))
                finally:
                    if not closed:
                        proxy.close()
        finally:
            server.shutdown()
            server.server_close()
            serving.join(3)


if __name__ == '__main__':
    unittest.main()
