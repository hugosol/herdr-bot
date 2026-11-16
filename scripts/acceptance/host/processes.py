"""Owned process observation, output draining and bounded acceptance assertions."""
import ctypes
import queue
import subprocess
import threading
import time


def require(condition, detail):
    if not condition:
        raise AssertionError(detail)


def until(description, probe, seconds=45):
    deadline = time.monotonic() + seconds
    while True:
        result = probe()
        if result:
            return result
        if time.monotonic() >= deadline:
            raise AssertionError('Timed out: ' + description)
        time.sleep(0.1)


class Process:
    """Continuously drain both streams, including startup and cleanup failures."""
    def __init__(self, argv, cwd, env, root, label):
        self.lines = queue.Queue()
        self.child = subprocess.Popen(argv, cwd=cwd, env=env, stdout=subprocess.PIPE,
                                      stderr=subprocess.PIPE, text=True, encoding='utf-8', errors='replace')
        self.threads = []
        for name, stream in [('stdout', self.child.stdout), ('stderr', self.child.stderr)]:
            def drain(stream=stream, name=name):
                with (root / f'{label}-{name}.log').open('w', encoding='utf-8') as log:
                    for line in stream:
                        log.write(line)
                        log.flush()
                        self.lines.put(line)
            thread = threading.Thread(target=drain, daemon=True)
            thread.start()
            self.threads.append(thread)

    def stop(self):
        if self.child.poll() is None:
            # Wait for taskkill itself before starting the exit-observation window.
            result = subprocess.run(['taskkill', '/PID', str(self.child.pid), '/T', '/F'],
                                    capture_output=True, encoding='utf-8', errors='replace', timeout=60)
            self.child.wait(timeout=15)
            require(result.returncode == 0 or self.child.poll() is not None,
                    ('taskkill', result.returncode, result.stdout, result.stderr))
        for thread in self.threads:
            thread.join(timeout=15)
            require(not thread.is_alive(), 'Process output drain did not finish')


class LiveProcess:
    """Hold the actual process handle: PID reuse cannot produce false evidence."""
    def __init__(self, pid):
        self.kernel = ctypes.WinDLL('kernel32', use_last_error=True)
        self.kernel.OpenProcess.argtypes = [ctypes.c_ulong, ctypes.c_int, ctypes.c_ulong]
        self.kernel.OpenProcess.restype = ctypes.c_void_p
        self.kernel.WaitForSingleObject.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
        self.kernel.CloseHandle.argtypes = [ctypes.c_void_p]
        self.handle = self.kernel.OpenProcess(0x00100000, False, pid)
        require(self.handle, ('Cannot observe controlled PID', pid, ctypes.get_last_error()))
        self.pid = pid

    def alive(self):
        status = self.kernel.WaitForSingleObject(self.handle, 0)
        require(status in [0, 258], ('WaitForSingleObject', status))
        return status == 258

    def close(self):
        self.kernel.CloseHandle(self.handle)

