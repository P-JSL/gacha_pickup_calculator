#!/usr/bin/env bash
# Capture actual v0.1.15 desktop GUI screens, using the sanitized local PNG library.
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
    # Click actual game tabs. Header search+Enter did not change the
    # selected game in an earlier verified CI capture.
    xdotool mousemove "$1" 540 click 1
    sleep 10
    capture "$2"
  }

  sleep 12
  capture dashboard-hsr-aha
  navigate 225; capture simulator-hsr-aha
  navigate 285; capture currency
  navigate 343; capture characters
  navigate 400; capture weapons
  navigate 548; capture preferences
  # The README photo retains default auto-refresh OFF. Enable it only for
  # subsequent screenshots so switching game finishes the chart simulation.
  xdotool mousemove 420 230 click 1
  sleep 1
  navigate 168
  game 650 dashboard-wuwa-latest
  game 766 dashboard-stellasora-latest
  game 866 dashboard-nte-latest
'

python3 - <<'PY'
from pathlib import Path
from PIL import Image

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
# Verify actual selected tabs, not merely differing cursor positions/history.
# A previous CI run produced HSR images under all three game-specific names.
selected_tabs = {
    "dashboard-hsr-aha.png": (503, 535),
    "dashboard-wuwa-latest.png": (600, 535),
    "dashboard-stellasora-latest.png": (731, 535),
    "dashboard-nte-latest.png": (827, 535),
}
for name, (x, y) in selected_tabs.items():
    with Image.open("ci-readme-shots/" + name) as original:
        rgb = original.convert("RGB").getpixel((x, y))
        assert rgb[0] < 115 and rgb[1] > 185 and rgb[2] > 185, (
            f"Wrong game selected in {name}: tab color {rgb}"
        )
print(f"Verified {len(paths)} actual graphical desktop screenshots")
PY
