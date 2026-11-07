#!/usr/bin/env python3
"""Wait for an actual button-triggered worker generation, never a sleep timer.

A startup event proves the egui application reached its update loop. A
simulation event requires STARTED and COMPLETED for the *same* game/generation,
both occurring after the user-interface click.
"""
from __future__ import annotations

import argparse
from pathlib import Path
import re
import time

EVENT = re.compile(
    r"GACHA_CI_SIM_(STARTED|COMPLETED|FAILED) game=(\\w+) generation=(\\d+)"
)
BUTTON = re.compile(
    r"GACHA_CI_RUN_BUTTON game=(\\w+) x=(\\d+) y=(\\d+) visible=(true|false)"
)


def assess(lines: list[str], expected_game: str) -> tuple[str, int | None]:
    started: int | None = None
    for line in lines:
        match = EVENT.search(line)
        if not match or match.group(2) != expected_game:
            continue
        stage, _, token = match.groups()
        generation = int(token)
        if stage == "STARTED":
            started = generation
        elif started == generation and stage == "FAILED":
            return "failed", generation
        elif started == generation and stage == "COMPLETED":
            return "completed", generation
    return "waiting", started


def visible_button(lines: list[str], expected_game: str) -> tuple[int, int] | None:
    for line in reversed(lines):
        found = BUTTON.search(line)
        if not found or found.group(1) != expected_game or found.group(4) != "true":
            continue
        x, y = int(found.group(2)), int(found.group(3))
        # Pixel coordinates are real X11 screen coordinates at 1x CI scale.
        if 240 < x < 1500 and 90 < y < 830:
            return x, y
    return None


def wait(log: Path, *, game: str, from_line: int, seconds: float,
         button: bool = False) -> None:
    start = time.monotonic()
    while time.monotonic() - start < seconds:
        lines = log.read_text(errors="replace").splitlines() if log.exists() else []
        fragment = lines[from_line:]
        if button:
            point = visible_button(fragment, game)
            if point is not None:
                print(*point)
                return
        elif game == "ui":
            if any("GACHA_CI_UI_READY game=StarRail" in x for x in fragment):
                print("Application rendered: input remains uncomputed")
                return
        else:
            state, generation = assess(fragment, game)
            if state == "completed":
                print(f"Run Simulation completed: game={game} generation={generation}")
                return
            if state == "failed":
                raise RuntimeError(f"Run Simulation failed: game={game} generation={generation}")
        time.sleep(0.1)
    raise TimeoutError(
        f"Expected Run Simulation events did not arrive for {game} within {seconds}s. "
        f"Log: {log}"
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--log", type=Path, required=True)
    parser.add_argument("--game", choices=["ui", "StarRail", "WutheringWaves", "StellaSora", "Neverness"], required=True)
    parser.add_argument("--from-line", type=int, default=0)
    parser.add_argument("--button", action="store_true",
                        help="Wait for real, visible egui button geometry")
    parser.add_argument("--timeout", type=float, default=90)
    args = parser.parse_args()
    wait(args.log, game=args.game, from_line=args.from_line,
         seconds=args.timeout, button=args.button)
