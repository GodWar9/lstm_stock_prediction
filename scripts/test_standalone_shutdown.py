"""Exercise OS graceful shutdown and temporary-workspace cleanup of the release."""
import argparse
import ctypes
import json
import os
from pathlib import Path
import queue
import signal
import subprocess
import tempfile
import threading
import urllib.request


def verify(binary):
    with tempfile.TemporaryDirectory(prefix='lstm-shutdown-acceptance-') as directory:
        root = Path(directory)
        env = {**os.environ, 'TEMP': directory, 'TMP': directory, 'TMPDIR': directory,
               'PATH': ''}
        options = {}
        if os.name == 'nt':
            startup = subprocess.STARTUPINFO()
            startup.dwFlags |= subprocess.STARTF_USESHOWWINDOW
            startup.wShowWindow = subprocess.SW_HIDE
            options = {'creationflags': subprocess.CREATE_NEW_CONSOLE, 'startupinfo': startup}
        process = subprocess.Popen([str(binary.resolve()), '--no-browser'], cwd=root,
                                   env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                   text=True, **options)
        attached = False
        try:
            lines = queue.Queue()
            def read_lines():
                for line in process.stdout:
                    lines.put(line)
                lines.put(None)
            reader = threading.Thread(target=read_lines, daemon=True)
            reader.start()
            while True:
                line = lines.get(timeout=30)
                if line is None:
                    raise AssertionError('App exited before announcing its URL')
                if line.startswith('Research inspector: '):
                    url = line.removeprefix('Research inspector: ').strip()
                    break
            with urllib.request.urlopen(url + '/api/health', timeout=10) as response:
                assert response.status == 200
            assert len(list(root.glob('lstm-inspector-*'))) == 1
            if os.name == 'nt':
                kernel = ctypes.WinDLL('kernel32', use_last_error=True)
                # Only this helper and the hidden child share the new console.
                kernel.FreeConsole()
                if not kernel.AttachConsole(process.pid):
                    raise ctypes.WinError(ctypes.get_last_error())
                attached = True
                if not kernel.SetConsoleCtrlHandler(None, True):
                    raise ctypes.WinError(ctypes.get_last_error())
                if not kernel.GenerateConsoleCtrlEvent(0, 0):
                    raise ctypes.WinError(ctypes.get_last_error())
            else:
                process.send_signal(signal.SIGINT)
            assert process.wait(timeout=15) == 0, process.stderr.read()
            reader.join(timeout=2)
            assert list(root.iterdir()) == [], 'Graceful shutdown left private artifacts behind'
        finally:
            if attached:
                kernel.FreeConsole()
            if process.poll() is None:
                process.kill()
                process.wait(timeout=10)
            process.stdout.close()
            process.stderr.close()
    print(json.dumps({'status': 'passed', 'os_signal_shutdown': True, 'workspace_removed': True}))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    verify(**vars(parser.parse_args()))
