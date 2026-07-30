#!/usr/bin/env python3
"""Print a bounded AT-SPI tree for a running application."""

from __future__ import annotations

import sys

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi  # noqa: E402


def printable(value: object) -> str:
    return str(value or "").replace("\n", "\\n")


def print_tree(node: Atspi.Accessible, depth: int, max_depth: int) -> None:
    if depth > max_depth:
        return
    try:
        role = node.get_role_name()
        name = node.get_name()
        child_count = node.get_child_count()
        action = node.get_action_iface()
        actions = (
            [action.get_action_name(index) for index in range(action.get_n_actions())]
            if action is not None
            else []
        )
    except Exception as error:  # pragma: no cover - diagnostic best effort
        print(f"{'  ' * depth}<unavailable: {error}>")
        return
    print(
        f"{'  ' * depth}{printable(role)} "
        f"name={printable(name)!r} children={child_count} actions={actions}",
    )
    for index in range(child_count):
        child = node.get_child_at_index(index)
        if child is not None:
            print_tree(child, depth + 1, max_depth)


def main() -> int:
    if len(sys.argv) not in (2, 3):
        print(f"Usage: {sys.argv[0]} APP_NAME [MAX_DEPTH]", file=sys.stderr)
        return 2
    wanted = sys.argv[1].casefold()
    max_depth = int(sys.argv[2]) if len(sys.argv) == 3 else 8
    desktop = Atspi.get_desktop(0)
    for index in range(desktop.get_child_count()):
        application = desktop.get_child_at_index(index)
        if application is None:
            continue
        if wanted in printable(application.get_name()).casefold():
            print_tree(application, 0, max_depth)
            return 0
    print(f"Application not found: {sys.argv[1]}", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
