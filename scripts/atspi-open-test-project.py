#!/usr/bin/env python3
"""Select a project directory in Pană Studio's native GTK file chooser."""

from __future__ import annotations

import os
import sys
import time

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi  # noqa: E402


def find_descendant(
    node: Atspi.Accessible,
    role_name: str,
    accessible_name: str | None = None,
) -> Atspi.Accessible | None:
    try:
        if node.get_role_name() == role_name:
            name = str(node.get_name() or "")
            if accessible_name is None or name == accessible_name:
                return node
        child_count = node.get_child_count()
    except Exception:
        return None
    for index in range(child_count):
        child = node.get_child_at_index(index)
        if child is not None:
            match = find_descendant(child, role_name, accessible_name)
            if match is not None:
                return match
    return None


def activate(accessible: Atspi.Accessible, wanted_action: str = "activate") -> bool:
    action = accessible.get_action_iface()
    if action is None:
        return False
    fallback_index = 0 if action.get_n_actions() == 1 else None
    for index in range(action.get_n_actions()):
        if action.get_action_name(index) == wanted_action:
            return bool(action.do_action(index))
    return fallback_index is not None and bool(action.do_action(fallback_index))


def find_application(name: str) -> Atspi.Accessible | None:
    desktop = Atspi.get_desktop(0)
    wanted = name.casefold()
    for index in range(desktop.get_child_count()):
        application = desktop.get_child_at_index(index)
        if application is None:
            continue
        if wanted in str(application.get_name() or "").casefold():
            return application
    return None


def main() -> int:
    if len(sys.argv) != 2:
        print(f"Usage: {sys.argv[0]} PROJECT_DIRECTORY", file=sys.stderr)
        return 2
    project_root = os.path.abspath(sys.argv[1])
    if not os.path.isdir(project_root):
        print(f"Project directory does not exist: {project_root}", file=sys.stderr)
        return 2

    application = find_application("pana-studio")
    if application is None:
        print("Pană Studio is not running.", file=sys.stderr)
        return 1
    chooser = find_descendant(application, "file chooser", "Deschide dosarul proiectului")
    if chooser is None:
        open_button = (
            find_descendant(application, "push button", "Deschide dosar")
            or find_descendant(application, "push button", "Alege alt dosar")
        )
        if open_button is None or not activate(open_button, "click"):
            print("Could not open the Pană Studio folder chooser.", file=sys.stderr)
            return 1
        deadline = time.monotonic() + 5
        while chooser is None and time.monotonic() < deadline:
            time.sleep(0.1)
            application = find_application("pana-studio")
            chooser = (
                find_descendant(
                    application,
                    "file chooser",
                    "Deschide dosarul proiectului",
                )
                if application is not None
                else None
            )
        if chooser is None:
            print("The Pană Studio folder chooser did not open.", file=sys.stderr)
            return 1

    home_root = os.path.expanduser("~")
    try:
        relative_parts = os.path.relpath(project_root, home_root).split(os.sep)
    except ValueError:
        relative_parts = []
    if not relative_parts or relative_parts[0] == "..":
        print(
            f"The accessibility helper currently accepts projects below {home_root}.",
            file=sys.stderr,
        )
        return 2

    home_path_button = find_descendant(chooser, "toggle button", os.path.basename(home_root))
    if home_path_button is None or not activate(home_path_button, "click"):
        print("Could not navigate the file chooser to the home directory.", file=sys.stderr)
        return 1
    time.sleep(0.4)

    for part in relative_parts:
        application = find_application("pana-studio")
        chooser = (
            find_descendant(application, "file chooser", "Deschide dosarul proiectului")
            if application is not None
            else None
        )
        directory_cell = (
            find_descendant(chooser, "table cell", part) if chooser is not None else None
        )
        if directory_cell is None or not activate(directory_cell):
            print(f"Could not navigate into directory: {part}", file=sys.stderr)
            return 1
        time.sleep(0.5)

    open_button = (
        find_descendant(chooser, "push button", "Open")
        or find_descendant(chooser, "push button", "Deschide")
    )
    if open_button is None:
        print("The file chooser Open button is unavailable.", file=sys.stderr)
        return 1
    if not activate(open_button, "click"):
        print("Could not activate the file chooser Open button.", file=sys.stderr)
        return 1
    print(project_root)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
