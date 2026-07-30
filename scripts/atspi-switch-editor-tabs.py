#!/usr/bin/env python3
"""Measure activation latency for two already-open Pană Studio editor tabs."""

from __future__ import annotations

import sys
import time
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path


helper_path = Path(__file__).with_name("atspi-open-test-project.py")
helper_spec = spec_from_file_location("atspi_open_test_project", helper_path)
if helper_spec is None or helper_spec.loader is None:
    raise RuntimeError(f"Could not load accessibility helper: {helper_path}")
helper = module_from_spec(helper_spec)
helper_spec.loader.exec_module(helper)
activate = helper.activate
find_application = helper.find_application
find_descendant = helper.find_descendant


def main() -> int:
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} TAB [TAB ...]", file=sys.stderr)
        return 2
    application = find_application("pana-studio")
    if application is None:
        print("Pană Studio is not running.", file=sys.stderr)
        return 1
    tab_list = find_descendant(
        application,
        "page tab list",
        "Documentele spațiului de lucru",
    )
    if tab_list is None:
        print("Editor tab list not found.", file=sys.stderr)
        return 1

    for tab_name in sys.argv[1:]:
        tab = find_descendant(tab_list, "page tab", tab_name)
        if tab is None:
            print(f"Tab not found: {tab_name}", file=sys.stderr)
            return 1
        started = time.monotonic()
        if not activate(tab, "click"):
            print(f"Could not activate tab: {tab_name}", file=sys.stderr)
            return 1
        print(f"{tab_name}: action accepted in {(time.monotonic() - started) * 1000:.1f} ms")
        time.sleep(2)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
