#!/usr/bin/env python3
"""Measure Pană Studio activity activation without AT-SPI tree-walk overhead."""

from __future__ import annotations

import argparse
import math
import statistics
import time
from dataclasses import dataclass

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi


@dataclass(frozen=True)
class ActivityControl:
    name: str
    node: object
    action: object
    action_index: int


def descendants(node):
    yield node
    try:
        child_count = node.get_child_count()
    except Exception:
        return
    for child_index in range(child_count):
        try:
            child = node.get_child_at_index(child_index)
        except Exception:
            continue
        yield from descendants(child)


def find_application(name: str, timeout_seconds: float):
    deadline = time.monotonic() + timeout_seconds
    desktop = Atspi.get_desktop(0)
    while time.monotonic() < deadline:
        for application_index in range(desktop.get_child_count()):
            candidate = desktop.get_child_at_index(application_index)
            if candidate.get_name() == name:
                return candidate
        time.sleep(0.05)
    raise RuntimeError(f"Aplicația accesibilă nu a fost găsită: {name}")


def find_controls(application, names: tuple[str, ...]) -> dict[str, ActivityControl]:
    pending = set(names)
    controls: dict[str, ActivityControl] = {}
    for node in descendants(application):
        try:
            name = node.get_name()
            if name not in pending:
                continue
            action = node.get_action_iface()
            if action is None:
                continue
            for action_index in range(action.get_n_actions()):
                if action.get_action_name(action_index) != "press":
                    continue
                controls[name] = ActivityControl(name, node, action, action_index)
                pending.remove(name)
                break
        except Exception:
            continue
        if not pending:
            break
    if pending:
        raise RuntimeError(f"Controale lipsă: {', '.join(sorted(pending))}")
    return controls


def is_active(control: ActivityControl) -> bool:
    return control.node.get_state_set().contains(Atspi.StateType.ACTIVE)


def activate(control: ActivityControl, timeout_seconds: float) -> float:
    started = time.perf_counter()
    if not control.action.do_action(control.action_index):
        raise RuntimeError(f"Acțiunea a fost refuzată: {control.name}")
    deadline = started + timeout_seconds
    while time.perf_counter() < deadline:
        if is_active(control):
            return (time.perf_counter() - started) * 1_000
        time.sleep(0.001)
    raise RuntimeError(f"Activarea a expirat: {control.name}")


def percentile(values: list[float], quantile: float) -> float:
    ordered = sorted(values)
    index = max(0, min(len(ordered) - 1, math.ceil(len(ordered) * quantile) - 1))
    return ordered[index]


def print_summary(label: str, values: list[float]):
    print(
        f"summary\tactivity={label}\tcount={len(values)}"
        f"\tmedian_ms={statistics.median(values):.3f}"
        f"\tp95_ms={percentile(values, 0.95):.3f}"
        f"\tmax_ms={max(values):.3f}",
        flush=True,
    )


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--application", default="pana-studio")
    parser.add_argument("--cycles", type=int, default=25)
    parser.add_argument("--warmup-cycles", type=int, default=1)
    parser.add_argument("--settle-ms", type=float, default=50)
    parser.add_argument("--timeout-seconds", type=float, default=5)
    parser.add_argument("activities", nargs="+", default=("Editor", "Șabloane"))
    arguments = parser.parse_args()
    if arguments.cycles <= 0 or arguments.warmup_cycles < 0:
        parser.error("Numărul de cicluri trebuie să fie pozitiv.")
    activities = tuple(arguments.activities)
    application = find_application(arguments.application, arguments.timeout_seconds)
    controls = find_controls(application, activities)

    for _ in range(arguments.warmup_cycles):
        for activity in activities:
            activate(controls[activity], arguments.timeout_seconds)
            time.sleep(arguments.settle_ms / 1_000)

    samples: dict[str, list[float]] = {activity: [] for activity in activities}
    for _ in range(arguments.cycles):
        for activity in activities:
            elapsed = activate(controls[activity], arguments.timeout_seconds)
            samples[activity].append(elapsed)
            print(f"sample\tactivity={activity}\telapsed_ms={elapsed:.3f}", flush=True)
            time.sleep(arguments.settle_ms / 1_000)

    for activity in activities:
        print_summary(activity, samples[activity])
    print_summary("all", [value for values in samples.values() for value in values])


if __name__ == "__main__":
    main()
