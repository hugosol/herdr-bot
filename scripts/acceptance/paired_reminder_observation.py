"""Windows desktop and audio observations for the paired reminder acceptance."""
import ctypes
from ctypes import wintypes
import math
import subprocess
import sys
import time
import traceback
import wave

from .host.processes import require, until


def launch_tui_client(executable, session, title, root, environment):
    code = ('import ctypes, subprocess, sys; '
        'ctypes.windll.kernel32.SetConsoleTitleW(sys.argv[3]); '
        'sys.exit(subprocess.call([sys.argv[1], "session", "attach", sys.argv[2]]))')
    return subprocess.Popen([sys.executable, '-c', code, str(executable), session, title],
        cwd=root, env=environment, creationflags=subprocess.CREATE_NEW_CONSOLE)


def foreground_window(*, pid=None, title=None, anchor=None):
    require(sum(selector is not None for selector in (pid, title, anchor)) == 1,
        'identify the owned window by exactly one selector')
    user = ctypes.WinDLL('user32', use_last_error=True)
    user.GetForegroundWindow.restype = wintypes.HWND
    user.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
    user.GetWindowTextW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
    user.IsWindowVisible.argtypes = [wintypes.HWND]
    user.ShowWindow.argtypes = [wintypes.HWND, ctypes.c_int]
    user.SetForegroundWindow.argtypes = [wintypes.HWND]
    callback = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    user.EnumWindows.argtypes = [callback, wintypes.LPARAM]

    def find():
        windows = []

        @callback
        def collect(window, _):
            if user.IsWindowVisible(window):
                caption = ctypes.create_unicode_buffer(512)
                user.GetWindowTextW(window, caption, len(caption))
                owner = wintypes.DWORD()
                user.GetWindowThreadProcessId(window, ctypes.byref(owner))
                if anchor is not None:
                    matches = window == anchor['hwnd'] and owner.value == anchor['ownerPid']
                elif pid is not None:
                    matches = owner.value == pid and bool(caption.value)
                else:
                    matches = title in caption.value
                if matches:
                    windows.append({'hwnd': window, 'ownerPid': owner.value, 'caption': caption.value})
            return True

        user.EnumWindows(collect, 0)
        require(len(windows) <= 1, ('ambiguous owned desktop window', windows))
        return windows[0] if windows else None

    window = until('owned interactive desktop window', find, seconds=15)
    hwnd = window['hwnd']
    if user.GetForegroundWindow() != hwnd:
        user.ShowWindow(hwnd, 5)
        user.SetForegroundWindow(hwnd)
    until('owned host window actually foreground', lambda: user.GetForegroundWindow() == hwnd, seconds=5)
    return window


class Loopback:
    """Observe a default WASAPI endpoint; no system settings are changed."""
    def __init__(self, path):
        self.path = path
        self.device = None
        self.error = None
        self.audio = self.silent = self.stream = None
        self.frames = []

    def __enter__(self):
        self.started = time.monotonic()
        try:
            import pyaudiowpatch as pa
            self.audio = pa.PyAudio()
            self.device = self.audio.get_default_wasapi_loopback()
            host = self.audio.get_host_api_info_by_type(pa.paWASAPI)
            output = self.audio.get_device_info_by_index(host['defaultOutputDevice'])
            self.channels = self.device['maxInputChannels']
            self.rate = int(self.device['defaultSampleRate'])
            def record(data, count, _time_info, status):
                self.frames.append((time.monotonic(), count, status, data))
                return (None, pa.paContinue)
            # Keep the loopback clock running even in the silence controls;
            # this stream submits only zero samples, never an audible cue.
            self.silent = self.audio.open(format=pa.paInt16, channels=1, rate=int(output['defaultSampleRate']),
                frames_per_buffer=512, output=True, output_device_index=output['index'],
                stream_callback=lambda _data, count, _time_info, _status: (bytes(count * 2), pa.paContinue))
            self.stream = self.audio.open(format=pa.paInt16, channels=self.channels, rate=self.rate,
                frames_per_buffer=512, input=True, input_device_index=self.device['index'], stream_callback=record)
        except Exception:
            self.error = traceback.format_exc()
        return self

    def __exit__(self, *_):
        self.ended = time.monotonic()
        try:
            for stream in (self.stream, self.silent):
                if stream is not None:
                    stream.stop_stream()
                    stream.close()
        finally:
            if self.audio is not None:
                self.audio.terminate()
        if self.error:
            return
        try:
            require(len(self.frames) > 1, 'no WASAPI callback frames; silence is not proven')
            require(not any(flags for _, _, flags, _ in self.frames), 'WASAPI callback signalled an input discontinuity')
            frame_count = sum(count for _, count, _, _ in self.frames)
            self.origin = self.frames[0][0] - self.frames[0][1] / self.rate
            with wave.open(str(self.path), 'wb') as wav:
                wav.setnchannels(self.channels)
                wav.setsampwidth(2)
                wav.setframerate(self.rate)
                for _, _, _, data in self.frames:
                    wav.writeframesraw(data)
            require(frame_count / self.rate >= (self.ended - self.origin) - 0.25,
                ('loopback did not cover observation', frame_count, self.rate, self.started, self.origin, self.ended))
        except Exception:
            self.error = traceback.format_exc()

    def describe(self):
        return {'path': str(self.path), 'device': self.device, 'frameCount': len(self.frames),
            'sampleOrigin': getattr(self, 'origin', None), 'started': self.started, 'ended': self.ended, 'error': self.error}


def identify_audio(ffmpeg, wav, references, origin):
    """Match decoded reference waveforms using normalized FFT correlation.

    Nonmaximum suppression uses each reference's actual duration; a sine's
    neighboring phase peaks do not count as separate playbacks.
    """
    import numpy as np
    rate = 8000
    def samples(source):
        result = subprocess.run([str(ffmpeg), '-hide_banner', '-loglevel', 'error', '-i', str(source),
            '-f', 'f32le', '-ac', '1', '-ar', str(rate), 'pipe:1'], capture_output=True, check=True, timeout=20)
        return np.frombuffer(result.stdout, dtype='<f4').astype(np.float64)
    captured = samples(wav)
    power = np.r_[0., np.cumsum(captured ** 2)]
    matches = []
    for name, path in references.items():
        shape = samples(path)
        rms = math.sqrt(np.mean(shape ** 2))
        require(len(shape) and rms > 0.001, ('silent or empty reference', path))
        n = len(shape)
        if len(captured) < n:
            continue
        fft_size = 1 << (len(captured) + n - 2).bit_length()
        dot = np.fft.irfft(np.fft.rfft(captured, fft_size) * np.fft.rfft(shape[::-1], fft_size), fft_size)[n-1:len(captured)]
        energy = np.maximum(power[n:] - power[:-n], 0.)
        score = np.abs(dot) / np.maximum(np.sqrt(energy * np.dot(shape, shape)), 1e-10)
        score[energy / n < 1e-8] = 0
        while score.size:
            offset = int(np.argmax(score))
            strength = float(score[offset])
            if strength < 0.68:
                break
            matches.append({'sound': name, 'monotonic': origin + offset/rate, 'correlation': strength,
                'rms': math.sqrt(energy[offset] / n)})
            radius = int(n * 0.85)
            score[max(0, offset-radius):offset+radius+1] = 0
    return sorted(matches, key=lambda match: match['monotonic'])

