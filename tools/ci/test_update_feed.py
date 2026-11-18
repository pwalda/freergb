# SPDX-License-Identifier: GPL-2.0-or-later
# python3 -m unittest discover -s tools/ci
import importlib.util
import pathlib
import unittest

spec = importlib.util.spec_from_file_location("feed", pathlib.Path(__file__).with_name("update-feed.py"))
feed = importlib.util.module_from_spec(spec)
spec.loader.exec_module(feed)

BODY = """freeRGB, a front-end for OpenRGB.

## What's new

- Updates install from the app

## Downloads

| System | File |
"""


class Feed(unittest.TestCase):
    def test_every_installer_gets_its_targets(self):
        f = feed.feed(
            {"tagName": "v0.1.0-beta.2", "body": BODY, "publishedAt": "2026-11-20T10:00:00Z"},
            {
                "freeRGB_0.1.0-beta.2_x64-setup.exe": "W\n",
                "freergb_0.1.0-beta.2_amd64.AppImage": "A",
                "freergb_0.1.0-beta.2_amd64.deb": "D",
                "freergb-0.1.0-beta.2-1.x86_64.rpm": "R",
            },
            "https://example.test/dl",
        )
        self.assertEqual(f["version"], "0.1.0-beta.2")
        self.assertEqual(f["notes"], "- Updates install from the app")
        self.assertEqual(
            sorted(f["platforms"]),
            ["linux-x86_64", "linux-x86_64-appimage", "linux-x86_64-deb", "linux-x86_64-rpm",
             "windows-x86_64", "windows-x86_64-nsis"],
        )
        self.assertEqual(
            f["platforms"]["windows-x86_64-nsis"],
            {"signature": "W", "url": "https://example.test/dl/freeRGB_0.1.0-beta.2_x64-setup.exe"},
        )

    def test_no_signatures_means_no_platforms(self):
        f = feed.feed({"tagName": "v1.0.0", "body": None, "publishedAt": "x"}, {}, "b")
        self.assertEqual((f["platforms"], f["notes"]), ({}, ""))


if __name__ == "__main__":
    unittest.main()
