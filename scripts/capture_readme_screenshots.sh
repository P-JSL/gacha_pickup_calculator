#!/usr/bin/env bash
# Capture real Windows/Linux-equivalent egui screens after explicit simulation.
# No input-change auto detection or hardcoded computation sleeps are permitted.
set -euo pipefail
test -x target/release/gacha_pickup_calculator
test -s target/release/Image/SHA256SUMS.txt
mkdir -p ci-readme-shots

xvfb-run -a -s "-screen 0 1536x864x24" bash -euo pipefail -c '
  export LIBGL_ALWAYS_SOFTWARE=1
  export GACHA_CI_SIMULATION_EVENTS=1
  log=/tmp/gacha-documentation-screens.log
  : > "$log"
  ./target/release/gacha_pickup_calculator >"$log" 2>&1 &
  app=$!
  trap "kill $app 2>/dev/null || true" EXIT
  python3 scripts/ui_capture_wait.py --log "$log" --game ui --timeout 30

  capture() {
    kill -0 "$app" || { cat "$log"; exit 1; }
    xdotool mousemove 1490 110
    scrot -q 92 "ci-readme-shots/$1.png"
  }
  navigate() {
    xdotool mousemove 90 "$1" click 1
    sleep 2
  }
  run_simulation() {
    local game="$1" x y offset attempt
    # Bring the actual Run Simulation button into view; never infer when the
    # user has finished input from typing pauses or a game selection event.
    offset=$(wc -l < "$log")
    xdotool mousemove 600 630 key ctrl+End
    x=""; y=""
    for attempt in $(seq 1 12); do
      if read -r x y < <(python3 scripts/ui_capture_wait.py --log "$log" \\
        --game "$game" --from-line "$offset" --button --timeout 1.5 2>/dev/null); then
        break
      fi
      # Scroll only in the content panel; never guess pixel colors or click
      # navigation/side-panel controls instead of the actual Run widget.
      xdotool mousemove 600 630 click --repeat 5 --delay 35 5
    done
    if [[ -z "${x:-}" || -z "${y:-}" ]]; then
      echo "Run Simulation widget was never visible after scrolling" >&2
      tail -n 25 "$log" >&2
      exit 1
    fi
    offset=$(wc -l < "$log")
    xdotool mousemove "$x" "$y" click 1
    # Match both STARTED and COMPLETED with the same generation from this click.
    # A timed-out or failed calculation is a CI failure, never a stale screenshot.
    python3 scripts/ui_capture_wait.py --log "$log" --game "$game" \
      --from-line "$offset" --timeout 90 || { tail -n 40 "$log"; exit 1; }
    xdotool mousemove 740 500 key ctrl+Home
    sleep 0.2
  }

  # The default game is preconfigured but must NOT compute until this click.
  run_simulation StarRail
  capture dashboard-hsr-aha
  navigate 225; capture simulator-hsr-aha
  navigate 285; capture currency
  navigate 343; capture characters
  navigate 400; capture weapons
  navigate 548; capture preferences
  navigate 168

  # Refresh tab coordinates after navigation; the viewport can be scrolled.
  xdotool mousemove 740 500 key ctrl+Home
  sleep 0.2
  scrot -q 92 ci-readme-shots/current-dashboard.png
  read -r hsr_x hsr_y < <(
    python3 scripts/ui_capture_tab.py ci-readme-shots/current-dashboard.png
  )
  last_tab_capture=current-dashboard
  game() {
    local index="$1" file="$2" expected="$3" offset active_x active_y target_x
    case "$index" in
      1) offset=114 ;;
      2) offset=228 ;;
      3) offset=325 ;;
      *) echo "Invalid game index $index" >&2; exit 1 ;;
    esac
    read -r active_x active_y < <(
      python3 scripts/ui_capture_tab.py "ci-readme-shots/$last_tab_capture.png"
    )
    target_x=$((hsr_x + offset))
    xdotool mousemove "$target_x" "$active_y" click 1
    # A game selection is an INPUT edit: it must never compute automatically.
    run_simulation "$expected"
    capture "$file"
    python3 scripts/ui_capture_tab.py "ci-readme-shots/$file.png" \
      --expected-index "$index" --hsr-x "$hsr_x"
    last_tab_capture="$file"
  }
  game 1 dashboard-wuwa-latest WutheringWaves
  rm -f ci-readme-shots/current-dashboard.png
  game 2 dashboard-stellasora-latest StellaSora
  game 3 dashboard-nte-latest Neverness
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
