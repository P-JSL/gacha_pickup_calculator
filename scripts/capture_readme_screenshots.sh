#!/usr/bin/env bash
# Capture actual v0.1.14 desktop GUI screens, using the sanitized local PNG library.
# Requires: xvfb-run, xdotool, scrot, Python3/Pillow; no generated mockups.
set -euo pipefail
test -x target/release/gacha_pickup_calculator
test -s target/release/Image/SHA256SUMS.txt
mkdir -p ci-readme-shots

xvfb-run -a -s "-screen 0 1536x864x24" bash -eu -c '
  export LIBGL_ALWAYS_SOFTWARE=1
  ./target/release/gacha_pickup_calculator >/tmp/gacha-documentation-screens.log 2>&1 &
  app=$!
  trap "kill $app 2>/dev/null || true" EXIT

  capture() {
    sleep 2
    kill -0 "$app" || { cat /tmp/gacha-documentation-screens.log; exit 1; }
    xdotool mousemove 1490 110
    scrot -q 92 "ci-readme-shots/$1.png"
  }
  navigate() {
    xdotool mousemove 90 "$1" click 1
    sleep 2
  }
  game() {
    # Header search, followed by Enter: real app navigation (no debug states).
    xdotool mousemove 1100 72 click 1
    xdotool key ctrl+a
    xdotool type --clearmodifiers --delay 55 "$1"
    xdotool key Return
    sleep 4
    # After a game change no result is visible yet, so the Run button
    # moves upwards. Detect the actual bright cyan button instead of
    # hard-coding the Y coordinate of the initial dashboard.
    scrot -q 75 /tmp/gacha-ui-before-run.png
    button_y=$(python3 -c "from PIL import Image; im=Image.open(\"/tmp/gacha-ui-before-run.png\").convert(\"RGB\"); rows=[y for y in range(140,840) if sum(1 for x in range(930,1214,3) if ((c:=im.getpixel((x,y)))[0]<110 and c[1]>180 and c[2]>205))>55]; print(round(sum(rows)/len(rows)) if rows else 0)")
    test "$button_y" -gt 140 || { echo "Run Simulation button not found in live GUI"; exit 1; }
    xdotool mousemove 1100 "$button_y" click 1
    sleep 9
    capture "$2"
  }

  sleep 12
  capture dashboard-hsr-aha
  navigate 225; capture simulator-hsr-aha
  navigate 285; capture currency
  navigate 343; capture characters
  navigate 400; capture weapons
  navigate 548; capture preferences
  navigate 168
  game wuthering dashboard-wuwa-latest
  game stella dashboard-stellasora-latest
  game neverness dashboard-nte-latest
'

python3 - <<'PY'
from pathlib import Path
from PIL import Image, ImageChops

paths = sorted(Path("ci-readme-shots").glob("*.png"))
required = {
    "dashboard-hsr-aha.png", "simulator-hsr-aha.png", "currency.png",
    "characters.png", "weapons.png", "preferences.png",
    "dashboard-wuwa-latest.png", "dashboard-stellasora-latest.png",
    "dashboard-nte-latest.png",
}
assert {p.name for p in paths} == required, "Incomplete GUI documentation screenshots"
for p in paths:
    with Image.open(p) as im:
        im.verify()
    with Image.open(p) as im:
        assert im.size == (1536, 864), f"Incorrect GUI capture dimensions: {p.name}"
games = ["dashboard-hsr-aha.png", "dashboard-wuwa-latest.png",
         "dashboard-stellasora-latest.png", "dashboard-nte-latest.png"]
for prior, current in zip(games, games[1:]):
    with Image.open("ci-readme-shots/" + prior) as first:
        with Image.open("ci-readme-shots/" + current) as other:
            assert ImageChops.difference(first, other).getbbox(), (
                f"Game switch capture did not update screen: {current}")
print(f"Verified {len(paths)} actual graphical desktop screenshots")
PY
