#!/usr/bin/env python3
"""Find the active quick-settings game tab from a real desktop screenshot.

Uses the known solid CYAN egui background of the *selected* button. It does not
infer the tab row from hardcoded Y coordinates, which change with responsive
layout, installed fonts, or translated labels.
"""
from __future__ import annotations

import argparse
from collections import defaultdict
from pathlib import Path

from PIL import Image


def selected_game_tab(path: str | Path) -> tuple[int, int]:
    with Image.open(path) as source:
        image = source.convert("RGB")
        width, height = image.size
        # The quick-settings tab row is in the central content panel. Exclude
        # left navigation, top cards/plots and the far-right Run button.
        x_min, x_max = 430, min(970, width - 1)
        y_min, y_max = 450, min(830, height - 1)
        runs: dict[int, list[tuple[int, int, int]]] = defaultdict(list)
        px = image.load()

        def cyan(x: int, y: int) -> bool:
            r, g, b = px[x, y]
            return 36 <= r <= 73 and 195 <= g <= 235 and 215 <= b <= 255

        for y in range(y_min, y_max):
            x = x_min
            while x < x_max:
                if not cyan(x, y):
                    x += 1
                    continue
                left = x
                while x < x_max and cyan(x, y):
                    x += 1
                # Chart strokes are only a few pixels tall; a selected tab
                # has a 90-124px solid background for many nearby rows.
                if 64 <= x - left <= 150:
                    mid = (left + x) // 2
                    runs[mid // 8].append((left, x, y))

        groups = [
            values for values in runs.values()
            if len(set(y for _, _, y in values)) >= 7
        ]
        if not groups:
            raise AssertionError(
                "Could not locate the filled cyan game tab in the main panel; "
                "the quick settings may be off-screen. "
                f"Screenshot: {path}"
            )
        best = max(groups, key=lambda values: (
            len(set(y for _, _, y in values)),
            sum(right - left for left, right, _ in values),
        ))
        # The button's fill above and below its text remains a solid cyan
        # strip; the fill's median Y is inside the button's click target.
        x = round(sum((left + right) / 2 for left, right, _ in best) / len(best))
        y = round((min(item[2] for item in best) + max(item[2] for item in best)) / 2)
        return x, y


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("image")
    parser.add_argument("--expected-index", type=int, choices=range(4))
    parser.add_argument("--hsr-x", type=int)
    args = parser.parse_args()

    x, y = selected_game_tab(args.image)
    if args.expected_index is not None:
        if args.hsr_x is None:
            raise ValueError("--hsr-x is required when validating a non-default tab")
        # Actual egui button widths: HSR 90 / WuWa 124 / Sora 90 / NTE 90,
        # with the default item spacing between them.
        relative_centers = (0, 114, 228, 325)
        wanted = args.hsr_x + relative_centers[args.expected_index]
        if abs(x - wanted) > 20:
            raise AssertionError(
                f"Wrong game selected in {args.image}: actual tab x={x}, "
                f"expected tab x≈{wanted} (index={args.expected_index}), y={y}"
            )
    print(f"{x} {y}")


if __name__ == "__main__":
    main()
