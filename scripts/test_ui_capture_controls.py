#!/usr/bin/env python3
"""Pure status and screenshot-control regression tests for CI-only automation."""
import tempfile
import unittest
from pathlib import Path

from PIL import Image, ImageDraw

from ui_capture_button import simulation_button
from ui_capture_wait import assess


class CaptureControlTests(unittest.TestCase):
    def test_run_button_excludes_narrow_game_tab(self):
        with tempfile.TemporaryDirectory() as folder:
            target = Path(folder) / "actual.png"
            image = Image.new("RGB", (1536, 864), (8, 20, 32))
            draw = ImageDraw.Draw(image)
            draw.rectangle((470, 470, 575, 500), fill=(50, 218, 241))
            draw.rectangle((1100, 690, 1274, 730), fill=(50, 218, 241))
            image.save(target)
            x, y = simulation_button(target)
            self.assertGreaterEqual(x, 1100)
            self.assertLessEqual(x, 1274)
            self.assertGreaterEqual(y, 690)
            self.assertLessEqual(y, 730)

    def test_only_matching_clicked_generation_can_complete(self):
        self.assertEqual(assess([
            "GACHA_CI_SIM_COMPLETED game=StarRail generation=2",
            "GACHA_CI_SIM_STARTED game=StarRail generation=3",
            "GACHA_CI_SIM_COMPLETED game=Neverness generation=3",
        ], "StarRail"), ("waiting", 3))
        self.assertEqual(assess([
            "GACHA_CI_SIM_STARTED game=StarRail generation=3",
            "GACHA_CI_SIM_COMPLETED game=StarRail generation=3",
        ], "StarRail"), ("completed", 3))

    def test_failed_generation_is_not_success(self):
        self.assertEqual(assess([
            "GACHA_CI_SIM_STARTED game=StellaSora generation=6",
            "GACHA_CI_SIM_FAILED game=StellaSora generation=6",
        ], "StellaSora"), ("failed", 6))


if __name__ == "__main__":
    unittest.main()
