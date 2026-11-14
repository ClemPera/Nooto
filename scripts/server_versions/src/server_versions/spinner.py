from __future__ import annotations

import sys
import threading
import time
from types import TracebackType
from typing import Self


class Spinner:
    def __init__(self, message: str) -> None:
        self.message = message
        self.started = 0.0
        self._stop = threading.Event()
        self._thread: threading.Thread | None = None

    def __enter__(self) -> Self:
        self.started = time.monotonic()
        if sys.stdout.isatty():
            self._thread = threading.Thread(target=self._spin, daemon=True)
            self._thread.start()
        else:
            print(f"{self.message}...")
        return self

    def _spin(self) -> None:
        frames = "|/-\\"
        index = 0
        while not self._stop.is_set():
            elapsed = time.monotonic() - self.started
            sys.stdout.write(f"\r{self.message}... {frames[index % len(frames)]} {elapsed:5.1f}s")
            sys.stdout.flush()
            index += 1
            self._stop.wait(0.1)

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc: BaseException | None,
        traceback: TracebackType | None,
    ) -> None:
        self._stop.set()
        if self._thread is not None:
            self._thread.join(timeout=1)
            sys.stdout.write("\r" + " " * 100 + "\r")
            sys.stdout.flush()
        elapsed = time.monotonic() - self.started
        status = "failed" if exc_type else "done"
        print(f"{self.message}... {status} in {elapsed:.1f}s")
