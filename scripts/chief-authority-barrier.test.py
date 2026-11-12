"""HTTP scheduling seam only, not P31/real-backend/model evidence.

A barrier that holds only the response (not authorization) would invalidate the
handoff scenario. The independent upstream arrival event must stay unset until
release; its literal conflict response must then survive unchanged.
"""
import http.server
import json
from pathlib import Path
import runpy
import threading
import unittest
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor

Barrier = runpy.run_path(str(Path(__file__).with_name('chief_authority_native_acceptance.py')))['ReceiveBarrier']


class ReceiptBarrierTransport(unittest.TestCase):
    def test_suspends_actual_upstream_request_not_only_its_response(self):
        arrived = threading.Event()
        seen = []
        payload = b'{"code":"independent-upstream-conflict"}'

        class Upstream(http.server.BaseHTTPRequestHandler):
            def log_message(self, *_):
                pass

            def do_POST(self):
                seen.append((self.path, self.rfile.read(int(self.headers['Content-Length']))))
                arrived.set()
                self.send_response(409)
                self.end_headers()
                self.wfile.write(payload)

        server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Upstream)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        barrier = Barrier(f'http://127.0.0.1:{server.server_port}')
        body = b'{"sessionId":"actual-caller"}'

        def submit():
            request = urllib.request.Request(barrier.base + '/api/chief/reports/report-731/receive', data=body,
                headers={'Content-Type': 'application/json'})
            try:
                result = urllib.request.urlopen(request, timeout=5)
            except urllib.error.HTTPError as error:
                result = error
            with result:
                return result.status, result.read()

        try:
            with ThreadPoolExecutor(max_workers=1) as executor:
                future = executor.submit(submit)
                try:
                    self.assertTrue(barrier.entered.wait(2), 'Actual receive request did not enter barrier')
                    self.assertFalse(arrived.wait(.15), 'Backend authorization already started before release')
                finally:
                    barrier.release.set()
                self.assertEqual(future.result(5), (409, payload))
            self.assertEqual(seen, [('/api/chief/reports/report-731/receive', body)])
            record = barrier.records[0]
            self.assertLessEqual(record['arrived'], record['forwarded'])
            self.assertEqual(json.loads(record['response']), {'code': 'independent-upstream-conflict'})
            self.assertNotIn('error', record)
        finally:
            barrier.close()
            server.shutdown()
            server.server_close()
            thread.join(5)
            self.assertFalse(thread.is_alive())


if __name__ == '__main__':
    unittest.main()
