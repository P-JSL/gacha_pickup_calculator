#!/usr/bin/env python3
"""Pure status and screenshot-control regression tests for CI-only automation."""
import unittest

from ui_capture_wait import assess, visible_button


class CaptureControlTests(unittest.TestCase):
    def test_only_visible_real_button_is_clickable(self):
        self.assertIsNone(visible_button([
            "GACHA_CI_RUN_BUTTON game=StarRail x=750 y=1300 visible=false"
        ], "StarRail"))
        self.assertIsNone(visible_button([
            "GACHA_CI_RUN_BUTTON game=Neverness x=1100 y=650 visible=true"
        ], "StarRail"))
        self.assertEqual(visible_button([
            "GACHA_CI_RUN_BUTTON game=StarRail x=400 y=650 visible=true"
        ], "StarRail"), (400, 650))

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


    def test_game_tab_finder_handles_final_probability_hero_above_controls(self):
        from pathlib import Path
        from tempfile import TemporaryDirectory

        from PIL import Image, ImageDraw
        from ui_capture_tab import selected_game_tab

        # The first-screen hero can shift quick settings above or below y=450.
        with TemporaryDirectory() as directory:
            filename = Path(directory) / "dashboard.png"
            for y in (300, 560):
                frame = Image.new("RGB", (1200, 860), (16, 43, 64))
                draw = ImageDraw.Draw(frame)
                draw.rectangle((480, y, 570, y + 31), fill=(50, 218, 241))
                frame.save(filename)
                x, detected_y = selected_game_tab(filename)
                self.assertAlmostEqual(x, 525, delta=10)
                self.assertAlmostEqual(detected_y, y + 15, delta=10)

    def test_failed_generation_is_not_success(self):
        self.assertEqual(assess([
            "GACHA_CI_SIM_STARTED game=StellaSora generation=6",
            "GACHA_CI_SIM_FAILED game=StellaSora generation=6",
        ], "StellaSora"), ("failed", 6))


if __name__ == "__main__":
    unittest.main()
