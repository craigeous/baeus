use gpui::{Context, SharedString, Window, div, prelude::*, px};
use serde::{Deserialize, Serialize};

use crate::theme::Color;

/// Visual variants for status badges used throughout the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BadgeVariant {
    Connected,
    Disconnected,
    Error,
    Warning,
    Pending,
    Healthy,
    Unknown,
}

impl BadgeVariant {
    /// Returns a human-readable label for the variant.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Connected => "Connected",
            Self::Disconnected => "Disconnected",
            Self::Error => "Error",
            Self::Warning => "Warning",
            Self::Pending => "Pending",
            Self::Healthy => "Healthy",
            Self::Unknown => "Unknown",
        }
    }
}

/// State representation for a status badge component.
///
/// Tracks the variant, auto-derived label, and optional tooltip text.
/// Provides theme-aware color selection via [`StatusBadge::color`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusBadge {
    pub variant: BadgeVariant,
    pub label: String,
    pub tooltip: Option<String>,
    /// Whether the badge renders with dark-mode colors (defaults to `true`).
    pub is_dark: bool,
}

impl StatusBadge {
    /// Creates a new status badge with its label auto-derived from the variant.
    pub fn new(variant: BadgeVariant) -> Self {
        Self { label: variant.label().to_string(), variant, tooltip: None, is_dark: true }
    }

    /// Sets an optional tooltip on the badge, consuming and returning self for chaining.
    pub fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Sets the dark-mode flag, consuming and returning self for chaining.
    pub fn with_dark_mode(mut self, is_dark: bool) -> Self {
        self.is_dark = is_dark;
        self
    }

    /// Returns the appropriate color for the badge given the current theme mode.
    ///
    /// When `is_dark` is true, lighter/brighter colors are returned for dark backgrounds.
    /// When `is_dark` is false, standard saturated colors are returned for light backgrounds.
    pub fn color(&self, is_dark: bool) -> Color {
        if is_dark {
            match self.variant {
                BadgeVariant::Connected => Color::rgb(0x4c, 0xaf, 0x50),
                BadgeVariant::Disconnected => Color::rgb(0x8e, 0x92, 0x97),
                BadgeVariant::Error => Color::rgb(0xce, 0x39, 0x33),
                BadgeVariant::Warning => Color::rgb(0xff, 0x98, 0x00),
                BadgeVariant::Pending => Color::rgb(0x00, 0xa7, 0xa0),
                BadgeVariant::Healthy => Color::rgb(0x4c, 0xaf, 0x50),
                BadgeVariant::Unknown => Color::rgb(0x8e, 0x92, 0x97),
            }
        } else {
            match self.variant {
                BadgeVariant::Connected => Color::rgb(34, 197, 94),
                BadgeVariant::Disconnected => Color::rgb(156, 163, 175),
                BadgeVariant::Error => Color::rgb(239, 68, 68),
                BadgeVariant::Warning => Color::rgb(245, 158, 11),
                BadgeVariant::Pending => Color::rgb(59, 130, 246),
                BadgeVariant::Healthy => Color::rgb(34, 197, 94),
                BadgeVariant::Unknown => Color::rgb(156, 163, 175),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// GPUI Render
// ---------------------------------------------------------------------------

impl Render for StatusBadge {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let dot_color = self.color(self.is_dark).to_gpui();
        let text_color = if self.is_dark {
            crate::theme::Color::rgb(249, 250, 251).to_gpui() // text_primary dark
        } else {
            crate::theme::Color::rgb(17, 24, 39).to_gpui() // text_primary light
        };
        let label = SharedString::from(self.label.clone());

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            // Colored status dot
            .child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(dot_color))
            // Label text
            .child(div().text_sm().text_color(text_color).child(label))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_badge_variant_labels() {
        assert_eq!(BadgeVariant::Connected.label(), "Connected");
        assert_eq!(BadgeVariant::Disconnected.label(), "Disconnected");
        assert_eq!(BadgeVariant::Error.label(), "Error");
        assert_eq!(BadgeVariant::Warning.label(), "Warning");
        assert_eq!(BadgeVariant::Pending.label(), "Pending");
        assert_eq!(BadgeVariant::Healthy.label(), "Healthy");
        assert_eq!(BadgeVariant::Unknown.label(), "Unknown");
    }

    #[test]
    fn test_new_badge_auto_derives_label() {
        let badge = StatusBadge::new(BadgeVariant::Connected);
        assert_eq!(badge.variant, BadgeVariant::Connected);
        assert_eq!(badge.label, "Connected");
        assert!(badge.tooltip.is_none());
    }

    #[test]
    fn test_with_tooltip() {
        let badge =
            StatusBadge::new(BadgeVariant::Error).with_tooltip("Connection refused on port 6443");
        assert_eq!(badge.tooltip.as_deref(), Some("Connection refused on port 6443"));
        assert_eq!(badge.variant, BadgeVariant::Error);
        assert_eq!(badge.label, "Error");
    }

    #[test]
    fn test_color_dark_theme() {
        let badge = StatusBadge::new(BadgeVariant::Connected);
        let color = badge.color(true);
        assert_eq!(color, Color::rgb(0x4c, 0xaf, 0x50));

        let badge = StatusBadge::new(BadgeVariant::Error);
        let color = badge.color(true);
        assert_eq!(color, Color::rgb(0xce, 0x39, 0x33));

        let badge = StatusBadge::new(BadgeVariant::Warning);
        let color = badge.color(true);
        assert_eq!(color, Color::rgb(0xff, 0x98, 0x00));

        let badge = StatusBadge::new(BadgeVariant::Pending);
        let color = badge.color(true);
        assert_eq!(color, Color::rgb(0x00, 0xa7, 0xa0));

        let badge = StatusBadge::new(BadgeVariant::Disconnected);
        let color = badge.color(true);
        assert_eq!(color, Color::rgb(0x8e, 0x92, 0x97));

        let badge = StatusBadge::new(BadgeVariant::Unknown);
        let color = badge.color(true);
        assert_eq!(color, Color::rgb(0x8e, 0x92, 0x97));
    }

    #[test]
    fn test_color_light_theme() {
        let badge = StatusBadge::new(BadgeVariant::Connected);
        let color = badge.color(false);
        assert_eq!(color, Color::rgb(34, 197, 94));

        let badge = StatusBadge::new(BadgeVariant::Error);
        let color = badge.color(false);
        assert_eq!(color, Color::rgb(239, 68, 68));

        let badge = StatusBadge::new(BadgeVariant::Warning);
        let color = badge.color(false);
        assert_eq!(color, Color::rgb(245, 158, 11));

        let badge = StatusBadge::new(BadgeVariant::Pending);
        let color = badge.color(false);
        assert_eq!(color, Color::rgb(59, 130, 246));
    }

    #[test]
    fn test_color_differs_by_theme_for_colored_variants() {
        let connected = StatusBadge::new(BadgeVariant::Connected);
        assert_ne!(connected.color(true), connected.color(false));

        let error = StatusBadge::new(BadgeVariant::Error);
        assert_ne!(error.color(true), error.color(false));

        let warning = StatusBadge::new(BadgeVariant::Warning);
        assert_ne!(warning.color(true), warning.color(false));

        let pending = StatusBadge::new(BadgeVariant::Pending);
        assert_ne!(pending.color(true), pending.color(false));
    }

    #[test]
    fn test_neutral_variants_have_muted_color() {
        // Neutral variants should use muted colors in both themes (may differ between themes)
        let disconnected = StatusBadge::new(BadgeVariant::Disconnected);
        assert_eq!(disconnected.color(true), Color::rgb(0x8e, 0x92, 0x97));
        assert_eq!(disconnected.color(false), Color::rgb(156, 163, 175));

        let unknown = StatusBadge::new(BadgeVariant::Unknown);
        assert_eq!(unknown.color(true), Color::rgb(0x8e, 0x92, 0x97));
        assert_eq!(unknown.color(false), Color::rgb(156, 163, 175));
    }

    #[test]
    fn test_healthy_matches_connected_colors() {
        let healthy = StatusBadge::new(BadgeVariant::Healthy);
        let connected = StatusBadge::new(BadgeVariant::Connected);
        assert_eq!(healthy.color(true), connected.color(true));
        assert_eq!(healthy.color(false), connected.color(false));
    }

    #[test]
    fn test_each_variant_has_unique_label() {
        let variants = [
            BadgeVariant::Connected,
            BadgeVariant::Disconnected,
            BadgeVariant::Error,
            BadgeVariant::Warning,
            BadgeVariant::Pending,
            BadgeVariant::Healthy,
            BadgeVariant::Unknown,
        ];
        let labels: Vec<&str> = variants.iter().map(|v| v.label()).collect();
        for (i, label) in labels.iter().enumerate() {
            for (j, other) in labels.iter().enumerate() {
                if i != j {
                    assert_ne!(label, other, "Duplicate label between variants {i} and {j}");
                }
            }
        }
    }

    // --- T022: Render tests for StatusBadge component ---

    /// All badge variants for iteration in render tests.
    const ALL_VARIANTS: [BadgeVariant; 7] = [
        BadgeVariant::Connected,
        BadgeVariant::Disconnected,
        BadgeVariant::Error,
        BadgeVariant::Warning,
        BadgeVariant::Pending,
        BadgeVariant::Healthy,
        BadgeVariant::Unknown,
    ];

    #[test]
    fn test_render_badge_colors_match_theme_success_error_warning() {
        use crate::theme::Theme;

        let dark = Theme::dark();
        let light = Theme::light();

        // Connected/Healthy badge should use the theme's success color
        let connected = StatusBadge::new(BadgeVariant::Connected);
        assert_eq!(connected.color(true), dark.colors.success);
        assert_eq!(connected.color(false), light.colors.success);

        let healthy = StatusBadge::new(BadgeVariant::Healthy);
        assert_eq!(healthy.color(true), dark.colors.success);
        assert_eq!(healthy.color(false), light.colors.success);

        // Error badge should use the theme's error color
        let error = StatusBadge::new(BadgeVariant::Error);
        assert_eq!(error.color(true), dark.colors.error);
        assert_eq!(error.color(false), light.colors.error);

        // Warning badge should use the theme's warning color
        let warning = StatusBadge::new(BadgeVariant::Warning);
        assert_eq!(warning.color(true), dark.colors.warning);
        assert_eq!(warning.color(false), light.colors.warning);

        // Pending badge should use the theme's info/accent color
        let pending = StatusBadge::new(BadgeVariant::Pending);
        assert_eq!(pending.color(true), dark.colors.info);
        assert_eq!(pending.color(false), light.colors.info);
    }

    #[test]
    fn test_render_badge_neutral_variants_use_muted_color() {
        use crate::theme::Theme;

        let dark = Theme::dark();
        let light = Theme::light();

        // Disconnected and Unknown should use the muted text color
        let disconnected = StatusBadge::new(BadgeVariant::Disconnected);
        assert_eq!(disconnected.color(true), dark.colors.text_muted);
        assert_eq!(disconnected.color(false), light.colors.text_muted);

        let unknown = StatusBadge::new(BadgeVariant::Unknown);
        assert_eq!(unknown.color(true), dark.colors.text_muted);
        assert_eq!(unknown.color(false), light.colors.text_muted);
    }

    #[test]
    fn test_render_every_variant_produces_non_zero_color() {
        for variant in ALL_VARIANTS {
            let badge = StatusBadge::new(variant);
            for is_dark in [true, false] {
                let c = badge.color(is_dark);
                // At least one RGB channel should be non-zero (no invisible badges)
                assert!(
                    c.r > 0 || c.g > 0 || c.b > 0,
                    "Badge {:?} in {} mode should produce a visible color",
                    variant,
                    if is_dark { "dark" } else { "light" }
                );
                // Alpha should be fully opaque
                assert_eq!(c.a, 255, "Badge color should be fully opaque");
            }
        }
    }

    #[test]
    fn test_render_badge_hex_colors_valid_format() {
        for variant in ALL_VARIANTS {
            let badge = StatusBadge::new(variant);
            for is_dark in [true, false] {
                let hex = badge.color(is_dark).to_hex();
                assert!(hex.starts_with('#'), "Hex color should start with '#': {hex}");
                // Opaque colors should be #rrggbb (7 chars)
                assert_eq!(hex.len(), 7, "Opaque badge color hex should be 7 chars: {hex}");
            }
        }
    }

    #[test]
    fn test_render_badge_color_to_gpui_conversion() {
        for variant in ALL_VARIANTS {
            let badge = StatusBadge::new(variant);
            let color = badge.color(true);
            let rgba = color.to_gpui();

            // GPUI Rgba uses 0.0-1.0 floats
            assert!(rgba.r >= 0.0 && rgba.r <= 1.0);
            assert!(rgba.g >= 0.0 && rgba.g <= 1.0);
            assert!(rgba.b >= 0.0 && rgba.b <= 1.0);
            assert!((rgba.a - 1.0).abs() < f32::EPSILON, "Alpha should be 1.0");
        }
    }

    #[test]
    fn test_render_badge_with_dark_mode_builder() {
        let badge = StatusBadge::new(BadgeVariant::Connected).with_dark_mode(false);
        assert!(!badge.is_dark);
        assert_eq!(badge.variant, BadgeVariant::Connected);
        assert_eq!(badge.label, "Connected");

        let badge = StatusBadge::new(BadgeVariant::Error).with_dark_mode(true);
        assert!(badge.is_dark);
    }

    #[test]
    fn test_render_badge_default_is_dark_mode() {
        let badge = StatusBadge::new(BadgeVariant::Warning);
        assert!(badge.is_dark, "New badges should default to dark mode");
    }

    #[test]
    fn test_render_badge_chaining_tooltip_and_dark_mode() {
        let badge = StatusBadge::new(BadgeVariant::Pending)
            .with_tooltip("Waiting for scheduler")
            .with_dark_mode(false);

        assert_eq!(badge.variant, BadgeVariant::Pending);
        assert_eq!(badge.label, "Pending");
        assert_eq!(badge.tooltip.as_deref(), Some("Waiting for scheduler"));
        assert!(!badge.is_dark);
    }

    #[test]
    fn test_render_colored_variants_have_distinct_colors_per_theme() {
        // Within each theme, the "colored" variants (Connected, Error, Warning, Pending)
        // should all have distinct colors so they are visually distinguishable.
        let colored_variants = [
            BadgeVariant::Connected,
            BadgeVariant::Error,
            BadgeVariant::Warning,
            BadgeVariant::Pending,
        ];

        for is_dark in [true, false] {
            let colors: Vec<Color> =
                colored_variants.iter().map(|v| StatusBadge::new(*v).color(is_dark)).collect();

            for i in 0..colors.len() {
                for j in (i + 1)..colors.len() {
                    assert_ne!(
                        colors[i],
                        colors[j],
                        "Variants {:?} and {:?} should have distinct colors in {} mode",
                        colored_variants[i],
                        colored_variants[j],
                        if is_dark { "dark" } else { "light" }
                    );
                }
            }
        }
    }
}
