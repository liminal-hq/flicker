// The projector-booth palette: tone-to-colour mapping
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: MIT

//! The projector-booth palette. Terminal-native dark, warmed by the lamp.

use ratatui::style::Color;

use crate::plugin::Tone;

/// Marquee amber — focus, titles, accents.
pub const ACCENT: Color = Color::Rgb(0xff, 0xb4, 0x54);
/// Curtain crimson — danger, confirms, Jax's box.
pub const ACCENT2: Color = Color::Rgb(0xe2, 0x5d, 0x75);
/// Projector cyan — informational values.
pub const INFO: Color = Color::Rgb(0x6e, 0xcd, 0xdc);
/// Screen-glow green — healthy, downloading, up.
pub const GOOD: Color = Color::Rgb(0x98, 0xd2, 0x79);
/// House-lights gold — warnings, paused states.
pub const WARN: Color = Color::Rgb(0xf0, 0xc8, 0x64);
/// Trouble red.
pub const BAD: Color = Color::Rgb(0xeb, 0x64, 0x64);
/// Dust — chrome, muted text.
pub const MUTED: Color = Color::Rgb(0x78, 0x76, 0x82);
/// Selection background — a dimmed lamp.
pub const SELECT_BG: Color = Color::Rgb(0x3a, 0x30, 0x22);

pub fn tone(t: Tone) -> Color {
    match t {
        Tone::Default => Color::Reset,
        Tone::Accent => ACCENT,
        Tone::Accent2 => ACCENT2,
        Tone::Info => INFO,
        Tone::Good => GOOD,
        Tone::Warn => WARN,
        Tone::Bad => BAD,
        Tone::Muted => MUTED,
    }
}
