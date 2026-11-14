import { spawn } from 'node:child_process'

// Windows oplock holds the actual business read, not an application collaborator.
// Closing stdin acknowledges the break. Always await release before server cleanup.
export async function scriptFileGate(file: string) {
  const child = spawn('python', ['-u', '-c', String.raw`
import ctypes as c, sys
from ctypes import wintypes as w
k = c.WinDLL('kernel32', use_last_error=True)
class O(c.Structure):
    _fields_ = [('internal', c.c_size_t), ('high', c.c_size_t), ('offset', w.DWORD), ('offset_high', w.DWORD), ('event', w.HANDLE)]
k.CreateFileW.argtypes = [w.LPCWSTR,w.DWORD,w.DWORD,c.c_void_p,w.DWORD,w.DWORD,w.HANDLE]
k.CreateFileW.restype = w.HANDLE
k.CreateEventW.argtypes = [c.c_void_p,w.BOOL,w.BOOL,w.LPCWSTR]
k.CreateEventW.restype = w.HANDLE
k.DeviceIoControl.argtypes = [w.HANDLE,w.DWORD,c.c_void_p,w.DWORD,c.c_void_p,w.DWORD,c.c_void_p,c.POINTER(O)]
k.WaitForSingleObject.argtypes = [w.HANDLE,w.DWORD]
k.CancelIoEx.argtypes = [w.HANDLE,c.POINTER(O)]
k.GetOverlappedResult.argtypes = [w.HANDLE,c.POINTER(O),c.POINTER(w.DWORD),w.BOOL]
k.CloseHandle.argtypes = [w.HANDLE]
f = k.CreateFileW(sys.argv[1], 0xC0000000, 7, None, 3, 0x40000000, None)
assert f != w.HANDLE(-1).value, c.get_last_error()
e = k.CreateEventW(None, True, False, None)
o = O(0,0,0,0,e)
try:
    assert k.DeviceIoControl(f,0x90000,None,0,None,0,None,c.byref(o)) == 0
    assert c.get_last_error() == 997, c.get_last_error()
    print('ready', flush=True)
    import threading
    def observe():
        if k.WaitForSingleObject(e,30000) == 0: print('reached', flush=True)
    t = threading.Thread(target=observe,daemon=True)
    t.start()
    sys.stdin.readline()
finally:
    k.CancelIoEx(f,c.byref(o))
    count = w.DWORD()
    k.GetOverlappedResult(f,c.byref(o),c.byref(count),True)
    k.CloseHandle(f)
    k.CloseHandle(e)
`, file], { stdio: ['pipe', 'pipe', 'pipe'], windowsHide: true })
  let stdout = '', stderr = ''
  child.stdout.on('data', chunk => { stdout += chunk })
  child.stderr.on('data', chunk => { stderr += chunk })
  const ended = Promise.withResolvers<void>()
  child.once('error', ended.reject)
  child.once('exit', code => code === 0 ? ended.resolve() : ended.reject(new Error(`File gate exit ${code}: ${stderr}`)))
  const exit = ended.promise
  void exit.catch(() => {})
  const ready = Promise.withResolvers<void>()
  const deadline = setTimeout(() => ready.reject(new Error(`File gate not ready: ${stderr}`)), 10000)
  const timer = setInterval(() => { if (stdout.includes('ready')) ready.resolve() }, 10)
  try {
    await Promise.race([exit.then(() => { throw new Error('File gate ended before ready') }), ready.promise])
  } catch (error) { child.stdin.end('\n'); await exit; throw error }
  finally { clearTimeout(deadline); clearInterval(timer) }
  return { reached: () => stdout.includes('reached'), release: async () => { child.stdin.end('\n'); await exit } }
}
