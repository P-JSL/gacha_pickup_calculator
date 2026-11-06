#!/usr/bin/env bash
# Capture actual v0.1.15 desktop GUI screens with sanitized local PNG images.
# Requires: xvfb-run, xdotool, scrot, Python3/Pillow; no generated mockups.
set -euo pipefail
test -x target/release/gacha_pickup_calculator
test -s target/release/Image/SHA256SUMS.txt
mkdir -p ci-readme-shots

# In the screenshot runner only, enable automatic calculation so each newly
# selected game produces its own real probability curves. The user-facing
# UiPreferences default remains OFF on Windows/Linux without this variable.
xvfb-run -a -s "-screen 0 1536x864x24" bash -eu -c '
  export LIBGL_ALWAYS_SOFTWARE=1
  export GACHA_CI_AUTO_REFRESH=1
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
  # Derive click coordinates from the preceding verified screenshot.
  # Avoid creating/deleting a transient probe image, which intermittently
  # vanished before Python could open it on the hosted virtual X11 runner.
  last_tab_capture=dashboard-hsr-aha
  game() {
    local index="$1" file="$2" offset active_x active_y target_x
    case "$index" in
      1) offset=114 ;;
      2) offset=228 ;;
      3) offset=325 ;;
      *) echo "Invalid game index $index" >&2; exit 1 ;;
    esac
    # Find the selected tab in the last captured real dashboard image.
    read -r active_x active_y < <(
      python3 scripts/ui_capture_tab.py "ci-readme-shots/$last_tab_capture.png"
    )
    target_x=$((hsr_x + offset))
    xdotool mousemove "$target_x" "$active_y" click 1
    sleep 11
    capture "$file"
    # Screenshot evidence must prove the new game is selected.
    python3 scripts/ui_capture_tab.py "ci-readme-shots/$file.png" \
      --expected-index "$index" --hsr-x "$hsr_x"
    last_tab_capture="$file"
  }

  sleep 12
  capture dashboard-hsr-aha
  navigate 225; capture simulator-hsr-aha
  navigate 285; capture currency
  navigate 343; capture characters
  navigate 400; capture weapons
  navigate 548; capture preferences
  navigate 168
  read -r hsr_x hsr_y < <(
    python3 scripts/ui_capture_tab.py ci-readme-shots/dashboard-hsr-aha.png
  )
  echo "Initial game tab found at x=$hsr_x, y=$hsr_y"
  game 1 dashboard-wuwa-latest
  game 2 dashboard-stellasora-latest
  game 3 dashboard-nte-latest
'

python3 - <<'PY'
from pathlib import Path
from PIL import Image
from scripts.ui_capture_tab import selected_game_tab

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

hsr_x, _ = selected_game_tab("ci-readme-shots/dashboard-hsr-aha.png")
for index, name in enumerate([
    "dashboard-hsr-aha.png", "dashboard-wuwa-latest.png",
    "dashboard-stellasora-latest.png", "dashboard-nte-latest.png"
]):
    x, y = selected_game_tab("ci-readme-shots/" + name)
    expected = hsr_x + (0, 114, 228, 325)[index]
    assert abs(x - expected) <= 20, (
        f"Wrong game selected in {name}: actual tab center ({x},{y}), "
        f"expected x around {expected}"
    )
print(f"Verified {len(paths)} actual graphical desktop screenshots and game tabs")
PY
