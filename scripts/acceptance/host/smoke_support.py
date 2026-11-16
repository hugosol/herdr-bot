"""Shared assertions, JSON evidence and real Windows file-read denial."""
import contextlib
import ctypes
import json


def require(condition, detail):
    if not condition:
        raise AssertionError(detail)


def dump(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding='utf-8')


@contextlib.contextmanager
def deny_read(path):
    """Real Windows sharing denial, never a mocked filesystem result."""
    kernel = ctypes.WinDLL('kernel32', use_last_error=True)
    kernel.CreateFileW.argtypes = [ctypes.c_wchar_p, ctypes.c_uint32, ctypes.c_uint32,
                                  ctypes.c_void_p, ctypes.c_uint32, ctypes.c_uint32, ctypes.c_void_p]
    kernel.CreateFileW.restype = ctypes.c_void_p
    kernel.CloseHandle.argtypes = [ctypes.c_void_p]
    handle = kernel.CreateFileW(str(path), 0x80000000, 0, None, 3, 0x80, None)
    require(handle != ctypes.c_void_p(-1).value, ('CreateFileW', ctypes.get_last_error()))
    try:
        try:
            path.read_bytes()
        except PermissionError:
            pass
        else:
            raise AssertionError('platform did not deny actual file read')
        yield
    finally:
        require(kernel.CloseHandle(handle), ('CloseHandle', ctypes.get_last_error()))

