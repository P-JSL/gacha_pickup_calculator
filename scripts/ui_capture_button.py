#!/usr/bin/env python3
"""Locate the actual Run Simulation button in a virtual-desktop screenshot.

This detects the wide, solid cyan button rendered by egui, not a guessed
screen coordinate. The narrow cyan selected-game tab is deliberately excluded.
"""
from __future__ import annotations

import sys
from collections import defaultdict
from pathlib import Path

from PIL import Image


def simulation_button(path: str | Path) -> tuple[int, int]:
    with Image.open(path) as source:
        image = source.convert("RGB")
        width, height = image.size
        pixels = image.load()
        groups: dict[int, list[tuple[int, int, int]]] = defaultdict(list)

        def cyan(x: int, y: int) -> bool:
            r, g, b = pixels[x, y]
            return 36 <= r <= 73 and 195 <= g <= 235 and 215 <= b <= 255

        for y in range(130, min(850, height - 4), 2):
            x = 260
            while x < min(1510, width - 4):
                if not cyan(x, y):
                    x += 1
                    continue
                left = x
                while x < min(1510, width - 4) and cyan(x, y):
                    x += 1
                # A selected game tab is <=124px; the primary action is >=175px.
                if 150 <= x - left <= 350:
                    groups[((left + x) // 2) // 12].append((left, x, y))

        candidates = [
            values for values in groups.values()
            if len({y for _, _, y in values}) >= 4
        ]
        if not candidates:
            # This error is displayed in CI after retries, so include the
            # broadest solid-cyan run to distinguish scrolling from color drift.
            largest = sorted((
                (right - left, left, right, y)
                for values in groups.values() for left, right, y in values
            ), reverse=True)[:5]
            raise ValueError(
                f"Run Simulation button is not visible: {path}; "
                f"cyan candidates={largest}; screenshot size={width}x{height}"
            )
        best = max(candidates, key=lambda values: (
            len({y for _, _, y in values}),
            sum(right - left for left, right, _ in values),
        ))
        return (
            round(sum((left + right) / 2 for left, right, _ in best) / len(best)),
            round((min(y for _, _, y in best) + max(y for _, _, y in best)) / 2),
        )


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit("usage: ui_capture_button.py <screenshot.png>")
    x, y = simulation_button(sys.argv[1])
    print(x, y)
