#!/usr/bin/env python3
"""Thin AT-SPI adapter for the canonical Pană Studio performance runner."""

from __future__ import annotations

import argparse
import json
import math
import os
import sys
import time
import warnings
from dataclasses import dataclass

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi  # noqa: E402

EXPECTED_PID = int(os.environ.get("PANA_BENCHMARK_APP_PID", "0")) or None
warnings.filterwarnings("ignore", category=DeprecationWarning)


@dataclass(frozen=True)
class ActionControl:
    name: str
    node: object
    action: object
    action_index: int


def emit_sample(
    *,
    scenario: str,
    profile: str,
    mode: str,
    metric: str,
    value: float,
    iteration: int,
    status: str = "ok",
    attributes: dict[str, object] | None = None,
) -> None:
    print(
        json.dumps(
            {
                "schemaVersion": 1,
                "kind": "sample",
                "layer": "ui",
                "scenario": scenario,
                "profile": profile,
                "mode": mode,
                "metric": metric,
                "value": round(value, 3),
                "unit": "ms",
                "iteration": iteration,
                "status": status,
                "attributes": attributes or {},
            },
            ensure_ascii=False,
            separators=(",", ":"),
        ),
        flush=True,
    )


def descendants(node):
    stack = [node]
    while stack:
        current = stack.pop()
        yield current
        try:
            children = [
                current.get_child_at_index(index)
                for index in range(current.get_child_count())
            ]
        except Exception:
            continue
        stack.extend(child for child in reversed(children) if child is not None)


def find_application(name: str):
    wanted = name.casefold()
    desktop = Atspi.get_desktop(0)
    for index in range(desktop.get_child_count()):
        application = desktop.get_child_at_index(index)
        if application is None:
            continue
        if EXPECTED_PID is not None:
            try:
                if application.get_process_id() != EXPECTED_PID:
                    continue
            except Exception:
                continue
        if wanted in str(application.get_name() or "").casefold():
            return application
    return None


def wait_application(name: str, timeout_seconds: float):
    deadline = time.monotonic() + timeout_seconds
    while time.monotonic() < deadline:
        application = find_application(name)
        if application is not None:
            return application
        time.sleep(0.025)
    raise RuntimeError(f"Aplicația accesibilă nu a fost găsită: {name}")


def find_node(root, *, role: str | None = None, name: str | None = None):
    for node in descendants(root):
        try:
            if role is not None and node.get_role_name() != role:
                continue
            if name is not None and str(node.get_name() or "") != name:
                continue
            return node
        except Exception:
            continue
    return None


def wait_node(
    application_name: str,
    *,
    role: str | None = None,
    name: str | None = None,
    timeout_seconds: float,
):
    deadline = time.monotonic() + timeout_seconds
    while time.monotonic() < deadline:
        application = find_application(application_name)
        if application is not None:
            node = find_node(application, role=role, name=name)
            if node is not None:
                return node
        time.sleep(0.025)
    return None


def action_control(node, name: str) -> ActionControl:
    action = node.get_action_iface()
    if action is None:
        raise RuntimeError(f"Controlul nu publică acțiuni: {name}")
    preferred = ("press", "click", "activate")
    actions = [action.get_action_name(index) for index in range(action.get_n_actions())]
    for wanted in preferred:
        if wanted in actions:
            return ActionControl(name, node, action, actions.index(wanted))
    if len(actions) == 1:
        return ActionControl(name, node, action, 0)
    raise RuntimeError(f"Controlul nu are o acțiune compatibilă: {name} / {actions}")


def activate(control: ActionControl) -> None:
    if not control.action.do_action(control.action_index):
        raise RuntimeError(f"Acțiunea a fost refuzată: {control.name}")


def state_active(node) -> bool:
    states = node.get_state_set()
    return any(
        states.contains(state)
        for state in (
            Atspi.StateType.ACTIVE,
            Atspi.StateType.CHECKED,
            Atspi.StateType.SELECTED,
        )
    )


def project_parts_below_home(project_root: str) -> list[str]:
    home = os.path.realpath(os.path.expanduser("~"))
    project = os.path.realpath(project_root)
    relative = os.path.relpath(project, home)
    parts = relative.split(os.sep)
    if not parts or parts[0] == "..":
        raise RuntimeError(
            f"Adaptorul acceptă proiecte aflate sub home ({home}), nu {project}."
        )
    return parts


def navigate_chooser(application_name: str, project_root: str, timeout_seconds: float):
    chooser = wait_node(
        application_name,
        role="file chooser",
        name="Deschide dosarul proiectului",
        timeout_seconds=timeout_seconds,
    )
    if chooser is None:
        raise RuntimeError("Dialogul de alegere a proiectului nu s-a deschis.")
    home_name = os.path.basename(os.path.expanduser("~"))
    home_button = find_node(chooser, role="toggle button", name=home_name)
    if home_button is None:
        raise RuntimeError("Butonul Home nu este disponibil în dialog.")
    activate(action_control(home_button, home_name))
    time.sleep(0.1)
    for part in project_parts_below_home(project_root):
        deadline = time.monotonic() + timeout_seconds
        cell = None
        while cell is None and time.monotonic() < deadline:
            application = find_application(application_name)
            chooser = (
                find_node(
                    application,
                    role="file chooser",
                    name="Deschide dosarul proiectului",
                )
                if application is not None
                else None
            )
            cell = find_node(chooser, role="table cell", name=part) if chooser else None
            if cell is None:
                time.sleep(0.025)
        if cell is None:
            raise RuntimeError(f"Directorul nu este vizibil în dialog: {part}")
        activate(action_control(cell, part))
        time.sleep(0.05)
    application = find_application(application_name)
    chooser = (
        find_node(
            application,
            role="file chooser",
            name="Deschide dosarul proiectului",
        )
        if application is not None
        else None
    )
    open_button = (
        find_node(chooser, role="push button", name="Open")
        or find_node(chooser, role="push button", name="Deschide")
        if chooser is not None
        else None
    )
    if open_button is None:
        raise RuntimeError("Butonul Open nu este disponibil în dialog.")
    return action_control(open_button, "Open")


def open_project(arguments) -> int:
    wait_application(arguments.application, arguments.timeout_seconds)
    chooser = wait_node(
        arguments.application,
        role="file chooser",
        name="Deschide dosarul proiectului",
        timeout_seconds=0.1,
    )
    if chooser is None:
        open_button = (
            wait_node(
                arguments.application,
                role="push button",
                name="Deschide dosar",
                timeout_seconds=arguments.timeout_seconds,
            )
            or wait_node(
                arguments.application,
                role="push button",
                name="Alege alt dosar",
                timeout_seconds=0.2,
            )
        )
        if open_button is None:
            raise RuntimeError("Acțiunea de deschidere a proiectului lipsește.")
        activate(action_control(open_button, "Deschide dosar"))
    final_open = navigate_chooser(
        arguments.application,
        arguments.project,
        arguments.timeout_seconds,
    )
    started = time.perf_counter()
    activate(final_open)
    workspace_at = None
    canvas_at = None
    rejection_at = None
    deadline = started + arguments.timeout_seconds
    while time.perf_counter() < deadline:
        application = find_application(arguments.application)
        if application is None:
            time.sleep(0.025)
            continue
        if workspace_at is None and find_node(application, name="Editor") is not None:
            workspace_at = time.perf_counter()
        if canvas_at is None and (
            find_node(application, name="Previzualizare interactivă") is not None
            or find_node(application, name="Controale previzualizare") is not None
        ):
            canvas_at = time.perf_counter()
        if rejection_at is None and find_node(
            application,
            name="Proiectul necesită corectare înainte de deschidere",
        ) is not None:
            rejection_at = time.perf_counter()
        if arguments.expected == "accepted" and workspace_at and canvas_at:
            break
        if arguments.expected == "rejected" and (rejection_at or workspace_at):
            break
        time.sleep(0.025)
    expected_pass = (
        arguments.expected == "accepted"
        and workspace_at is not None
        and canvas_at is not None
    ) or (
        arguments.expected == "rejected"
        and rejection_at is not None
        and workspace_at is None
    )
    terminal = rejection_at or canvas_at or workspace_at or time.perf_counter()
    emit_sample(
        scenario="project_open",
        profile=arguments.profile,
        mode="cold_process",
        metric="terminal_state",
        value=(terminal - started) * 1_000,
        iteration=arguments.iteration,
        status="ok" if expected_pass else "contract_violation",
        attributes={
            "expected": arguments.expected,
            "workspaceReady": workspace_at is not None,
            "canvasReady": canvas_at is not None,
            "rejected": rejection_at is not None,
        },
    )
    if workspace_at is not None:
        emit_sample(
            scenario="project_open",
            profile=arguments.profile,
            mode="cold_process",
            metric="workspace_ready",
            value=(workspace_at - started) * 1_000,
            iteration=arguments.iteration,
        )
    if canvas_at is not None:
        emit_sample(
            scenario="project_open",
            profile=arguments.profile,
            mode="cold_process",
            metric="canvas_accessible",
            value=(canvas_at - started) * 1_000,
            iteration=arguments.iteration,
        )
    return 0 if expected_pass else 3


def controls(application_name: str, names: tuple[str, ...], timeout_seconds: float):
    deadline = time.monotonic() + timeout_seconds
    while time.monotonic() < deadline:
        application = find_application(application_name)
        if application is not None:
            result = {}
            for name in names:
                node = find_node(application, role="push button", name=name)
                if node is not None:
                    result[name] = action_control(node, name)
            if len(result) == len(names):
                return result
        time.sleep(0.02)
    raise RuntimeError(f"Controale lipsă: {', '.join(names)}")


def activity_switch(control: ActionControl, timeout_seconds: float) -> float:
    started = time.perf_counter()
    activate(control)
    deadline = started + timeout_seconds
    while time.perf_counter() < deadline:
        if state_active(control.node):
            return (time.perf_counter() - started) * 1_000
        time.sleep(0.001)
    raise RuntimeError(f"Activarea a expirat: {control.name}")


def run_activities(arguments) -> int:
    names = tuple(arguments.activities)
    activity_controls = controls(arguments.application, names, arguments.timeout_seconds)
    for _ in range(arguments.warmup_cycles):
        for name in names:
            activity_switch(activity_controls[name], arguments.timeout_seconds)
    for iteration in range(arguments.cycles):
        for name in names:
            elapsed = activity_switch(activity_controls[name], arguments.timeout_seconds)
            emit_sample(
                scenario="activity_switch",
                profile=arguments.profile,
                mode="warm",
                metric="action_to_accessibility_state",
                value=elapsed,
                iteration=iteration,
                attributes={"activity": name},
            )
    for iteration in range(arguments.sustained_operations):
        name = names[iteration % len(names)]
        elapsed = activity_switch(activity_controls[name], arguments.timeout_seconds)
        emit_sample(
            scenario="activity_switch_sustained",
            profile=arguments.profile,
            mode="sustained",
            metric="action_to_accessibility_state",
            value=elapsed,
            iteration=iteration,
            attributes={"activity": name},
        )
    return 0


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser()
    subcommands = root.add_subparsers(dest="command", required=True)
    common = argparse.ArgumentParser(add_help=False)
    common.add_argument("--application", default="pana-studio")
    common.add_argument("--profile", required=True)
    common.add_argument("--timeout-seconds", type=float, default=120)

    open_parser = subcommands.add_parser("open", parents=[common])
    open_parser.add_argument("--project", required=True)
    open_parser.add_argument("--expected", choices=("accepted", "rejected"), required=True)
    open_parser.add_argument("--iteration", type=int, default=0)
    open_parser.set_defaults(handler=open_project)

    activity_parser = subcommands.add_parser("activities", parents=[common])
    activity_parser.add_argument("--cycles", type=int, required=True)
    activity_parser.add_argument("--warmup-cycles", type=int, default=5)
    activity_parser.add_argument("--sustained-operations", type=int, default=0)
    activity_parser.add_argument(
        "activities",
        nargs="+",
        default=("Editor", "Șabloane", "Componente"),
    )
    activity_parser.set_defaults(handler=run_activities)
    return root


def main() -> int:
    arguments = parser().parse_args()
    try:
        return arguments.handler(arguments)
    except Exception as error:
        print(f"[pana-performance-atspi] {error}", file=sys.stderr, flush=True)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
