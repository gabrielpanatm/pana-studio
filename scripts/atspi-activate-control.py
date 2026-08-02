#!/usr/bin/env python3
"""Activate one Pană Studio accessibility control by exact role and name."""

from __future__ import annotations

import sys
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path


helper_path = Path(__file__).with_name("atspi-open-test-project.py")
helper_spec = spec_from_file_location("atspi_open_test_project", helper_path)
if helper_spec is None or helper_spec.loader is None:
    raise RuntimeError(f"Could not load accessibility helper: {helper_path}")
helper = module_from_spec(helper_spec)
helper_spec.loader.exec_module(helper)


def main() -> int:
    if len(sys.argv) != 3:
        print(f"Usage: {sys.argv[0]} ROLE EXACT_NAME", file=sys.stderr)
        return 2
    role, name = sys.argv[1:]
    application = helper.find_application("pana-studio")
    if application is None:
        print("Pană Studio is not running.", file=sys.stderr)
        return 1
    control = helper.find_descendant(application, role, name)
    if control is None:
        print(f"Control not found: role={role!r}, name={name!r}", file=sys.stderr)
        return 1
    if not helper.activate(control, "press"):
        print(f"Control did not accept activation: {name}", file=sys.stderr)
        return 1
    print(name)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
