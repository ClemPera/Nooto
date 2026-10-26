#!/usr/bin/env python3
"""Compatibility launcher for the server_versions package."""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent / "server_versions" / "src"))

from server_versions.cli import main

raise SystemExit(main())
