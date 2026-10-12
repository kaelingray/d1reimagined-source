// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! Style and fonts, applied once at startup.
//!
//! `historicaldocs/design/egui-practices.md` opens with "kill the default look first", and it is right about why:
//! **the stock fonts and the stock spacing are the tell.** Two things follow from that here.
//!
//! **Spacing and radius are on a grid.** One corner radius everywhere, a 4/8 px spacing grid, and no
//! window shadow. egui gives per-widget control and the temptation to vary it is how a UI gets
//! visually noisy for no information gain.
//!
//! **Fonts are loaded from the host, not vendored.** The advice is to install real fonts, but this
//! project does not ship binary assets it does not need and the lab box has no internet to fetch
//! one from. So [`install_fonts`] looks for a small set of fonts that are actually present on this
//! machine and silently keeps egui's defaults if none are. That is a deliberate half-measure and it
//! is worth naming: if the operator wants a specific typeface, vendoring one `.ttf` is the fix.

use std::collections::BTreeMap;
use std::sync::Arc;

use egui::{Color32, CornerRadius, FontFamily, FontId, TextStyle};

/// Colours. Kept in one place so a widget cannot invent its own.
pub struct Palette;

impl Palette {
    /// Selection / focus. A cold blue rather than egui's default, which is the other big tell.
    pub const ACCENT: Color32 = Color32::from_rgb(0x4A, 0x8F, 0xE7);
    /// Validation errors, and the "post-01.29" era badge.
    pub const ERROR: Color32 = Color32::from_rgb(0xE0, 0x6C, 0x75);
    /// Validation warnings.
    pub const WARN: Color32 = Color32::from_rgb(0xE5, 0xC0, 0x7B);
    /// Applied / valid.
    pub const OK: Color32 = Color32::from_rgb(0x8C, 0xC2, 0x65);
    /// Provenance and "expected gap" text - present, but not shouting.
    pub const MUTED: Color32 = Color32::from_rgb(0x93, 0x9A, 0xA6);
    /// Year-1 badge.
    pub const YEAR_ONE: Color32 = Color32::from_rgb(0xC0, 0x8C, 0xE0);
}

/// The one corner radius in the app.
const RADIUS: u8 = 4;

/// ⚠ API drift on 0.36.1: `Context::style()` / `set_style()` are gone. Style is now *per theme*
/// (`style_of` / `set_style_of` / `style_mut_of`), and [`egui::Context::all_styles_mut`] is the way
/// to apply one set of tweaks to both the light and dark styles at once. Doing it per-theme by hand
/// is how an app ends up looking right in one theme and wrong in the other.
pub fn apply(ctx: &egui::Context) {
    ctx.all_styles_mut(tweak);
}

fn tweak(style: &mut egui::Style) {
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.spacing.menu_margin = egui::Margin::same(8);
    style.spacing.indent = 16.0;

    let r = CornerRadius::same(RADIUS);
    for w in [
        &mut style.visuals.widgets.noninteractive,
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        w.corner_radius = r;
    }
    style.visuals.window_corner_radius = r;
    style.visuals.menu_corner_radius = r;
    style.visuals.window_shadow = egui::Shadow::NONE;
    style.visuals.popup_shadow = egui::Shadow::NONE;
    style.visuals.selection.bg_fill = Palette::ACCENT.gamma_multiply(0.55);
    style.visuals.selection.stroke.color = Palette::ACCENT;
    style.visuals.hyperlink_color = Palette::ACCENT;

    style.text_styles = BTreeMap::from([
        (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(13.5, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(13.5, FontFamily::Proportional)),
        (TextStyle::Heading, FontId::new(18.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(12.5, FontFamily::Monospace)),
    ]);
}

/// Candidate host fonts, best first. Fedora ships the Adwaita and Liberation families; the others
/// are common enough elsewhere to be worth trying.
const PROPORTIONAL: &[&str] = &[
    "/usr/share/fonts/adwaita-sans-fonts/AdwaitaSans-Regular.ttf",
    "/usr/share/fonts/google-noto/NotoSans-Regular.ttf",
    "/usr/share/fonts/liberation-sans/LiberationSans-Regular.ttf",
    "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
];

const MONOSPACE: &[&str] = &[
    "/usr/share/fonts/adwaita-mono-fonts/AdwaitaMono-Regular.ttf",
    "/usr/share/fonts/adobe-source-code-pro/SourceCodePro-Regular.otf",
    "/usr/share/fonts/liberation-mono/LiberationMono-Regular.ttf",
    "/usr/share/fonts/dejavu-sans-mono-fonts/DejaVuSansMono.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
];

/// Load host fonts if any are present. Returns the names installed, for the diagnostics line.
pub fn install_fonts(ctx: &egui::Context) -> Vec<String> {
    let mut defs = egui::FontDefinitions::default();
    let mut installed = Vec::new();

    for (family, candidates, key) in [
        (FontFamily::Proportional, PROPORTIONAL, "host-proportional"),
        (FontFamily::Monospace, MONOSPACE, "host-monospace"),
    ] {
        let Some((path, bytes)) = candidates
            .iter()
            .find_map(|p| std::fs::read(p).ok().map(|b| (*p, b)))
        else {
            continue;
        };
        defs.font_data
            .insert(key.to_string(), Arc::new(egui::FontData::from_owned(bytes)));
        // Insert at the front so it wins, but keep egui's own fonts behind it as the fallback chain
        // - dropping them loses the glyphs the host font does not have.
        defs.families
            .entry(family)
            .or_default()
            .insert(0, key.to_string());
        installed.push(path.to_string());
    }

    if !installed.is_empty() {
        ctx.set_fonts(defs);
    }
    installed
}
