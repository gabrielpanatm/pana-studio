#!/usr/bin/env python3
"""Emit deterministic trusted X11 pointer clicks for local GUI benchmarks."""

from __future__ import annotations

import argparse
import ctypes
import ctypes.util
import time


CURRENT_TIME = 0
PRIMARY_BUTTON = 1


def load_library(name: str) -> ctypes.CDLL:
    path = ctypes.util.find_library(name)
    if not path:
        raise RuntimeError(f"Biblioteca X11 necesară lipsește: {name}")
    return ctypes.CDLL(path)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "coordinates",
        nargs="+",
        help="Perechi x,y în coordonate absolute ale ecranului, de exemplu 640,360",
    )
    parser.add_argument("--cycles", type=int, default=1)
    parser.add_argument("--settle-ms", type=float, default=80.0)
    parser.add_argument(
        "--move-only",
        action="store_true",
        help="Emite numai mișcări trusted, fără apăsarea butonului primar.",
    )
    arguments = parser.parse_args()
    if arguments.cycles <= 0 or arguments.settle_ms < 0:
        parser.error("cycles trebuie să fie pozitiv, iar settle-ms nenegativ")

    points: list[tuple[int, int]] = []
    for coordinate in arguments.coordinates:
        try:
            x_text, y_text = coordinate.split(",", maxsplit=1)
            points.append((int(x_text), int(y_text)))
        except ValueError as error:
            raise SystemExit(f"Coordonată invalidă: {coordinate!r}") from error

    x11 = load_library("X11")
    xtst = load_library("Xtst")
    x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
    x11.XOpenDisplay.restype = ctypes.c_void_p
    x11.XFlush.argtypes = [ctypes.c_void_p]
    xtst.XTestFakeMotionEvent.argtypes = [
        ctypes.c_void_p,
        ctypes.c_int,
        ctypes.c_int,
        ctypes.c_int,
        ctypes.c_ulong,
    ]
    xtst.XTestFakeButtonEvent.argtypes = [
        ctypes.c_void_p,
        ctypes.c_uint,
        ctypes.c_bool,
        ctypes.c_ulong,
    ]

    display = x11.XOpenDisplay(None)
    if not display:
        raise RuntimeError("Nu s-a putut deschide display-ul X11 curent")
    try:
        for _ in range(arguments.cycles):
            for x, y in points:
                xtst.XTestFakeMotionEvent(display, -1, x, y, CURRENT_TIME)
                if not arguments.move_only:
                    xtst.XTestFakeButtonEvent(display, PRIMARY_BUTTON, True, CURRENT_TIME)
                    xtst.XTestFakeButtonEvent(display, PRIMARY_BUTTON, False, CURRENT_TIME)
                x11.XFlush(display)
                time.sleep(arguments.settle_ms / 1_000)
    finally:
        x11.XCloseDisplay(display)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
