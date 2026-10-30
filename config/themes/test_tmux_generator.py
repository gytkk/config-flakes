#!/usr/bin/env python3
from __future__ import annotations

import unittest
from pathlib import Path

from generate import build_tmux_slots, load_yaml, theme_context, tmux_theme_conf


ROOT = Path(__file__).resolve().parent


class TmuxGeneratorTest(unittest.TestCase):
    def test_prefix_state_uses_session_accent_and_conditional_help_without_clock(self) -> None:
        theme = load_yaml(ROOT / "core" / "vira-graphene.yaml")
        ctx = theme_context(theme)

        slots = build_tmux_slots(ctx)
        theme_conf = tmux_theme_conf(ctx, ROOT)

        self.assertIn("#{?client_prefix,", slots["status_left"])
        self.assertIn(" #S #[default]", slots["status_left"])
        self.assertIn("#{?pane_synchronized,", slots["status_left"])

        self.assertEqual(
            '#{?client_prefix,#[fg=#D9D9D9] ? help | w tree | s sessions | #, rename | %%%% side split | " stack split | C-hjkl resize | z zoom | [ copy #[default],}',
            slots["status_right"],
        )
        self.assertEqual(
            "#[fg=#545454]#[bg=#212121] #I #W#F #(tmux-agent-window-status #{window_id}) #[default]",
            slots["window_status_format"],
        )
        self.assertEqual(
            "#[fg=#82AAFF]#[bg=#212121]#[bold] #I #[fg=#D9D9D9]#[bg=#212121]#[bold]#W#F #(tmux-agent-window-status #{window_id}) #[default]",
            slots["window_status_current_format"],
        )
        self.assertIn(" #I #W#F #(tmux-agent-window-status", theme_conf)
        self.assertIn(" #I #[fg=#D9D9D9]", theme_conf)
        self.assertIn("#(tmux-agent-window-status #{window_id})", theme_conf)
        self.assertNotIn("#I:#W#F", theme_conf)

        self.assertNotIn("PREFIX", theme_conf)
        self.assertNotIn("%Y", theme_conf)
        self.assertNotIn("%H", theme_conf)
        self.assertNotIn("C-a", theme_conf)


if __name__ == "__main__":
    unittest.main()
