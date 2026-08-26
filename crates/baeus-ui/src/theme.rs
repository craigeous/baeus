use gpui::Rgba;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ThemeMode {
    Light,
    Dark,
    #[default]
    System,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Convert to GPUI Rgba (floats 0.0-1.0)
    pub fn to_gpui(&self) -> Rgba {
        Rgba {
            r: self.r as f32 / 255.0,
            g: self.g as f32 / 255.0,
            b: self.b as f32 / 255.0,
            a: self.a as f32 / 255.0,
        }
    }

    pub fn to_hex(&self) -> String {
        if self.a == 255 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColorTokens {
    pub background: Color,
    pub surface: Color,
    pub surface_hover: Color,
    pub border: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_muted: Color,
    pub accent: Color,
    pub accent_hover: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub info: Color,
    pub sidebar_bg: Color,
    pub header_bg: Color,
    pub selection: Color,
    /// Alternating table row background.
    pub table_stripe: Color,
    /// Selected table row background.
    pub table_selected: Color,
    /// Tab bar background.
    pub tab_bar_bg: Color,
    /// Active tab background.
    pub tab_active_bg: Color,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub mode: ThemeMode,
    pub colors: ColorTokens,
}

impl Theme {
    pub fn light() -> Self {
        Self {
            mode: ThemeMode::Light,
            colors: ColorTokens {
                background: Color::rgb(255, 255, 255),
                surface: Color::rgb(249, 250, 251),
                surface_hover: Color::rgb(243, 244, 246),
                border: Color::rgb(229, 231, 235),
                text_primary: Color::rgb(17, 24, 39),
                text_secondary: Color::rgb(75, 85, 99),
                text_muted: Color::rgb(156, 163, 175),
                accent: Color::rgb(59, 130, 246),
                accent_hover: Color::rgb(37, 99, 235),
                success: Color::rgb(34, 197, 94),
                warning: Color::rgb(245, 158, 11),
                error: Color::rgb(239, 68, 68),
                info: Color::rgb(59, 130, 246),
                sidebar_bg: Color::rgb(249, 250, 251),
                header_bg: Color::rgb(255, 255, 255),
                selection: Color::rgba(59, 130, 246, 30),
                table_stripe: Color::rgb(243, 244, 246),
                table_selected: Color::rgb(229, 231, 235),
                tab_bar_bg: Color::rgb(0xf3, 0xf4, 0xf6), // #f3f4f6
                tab_active_bg: Color::rgb(0xff, 0xff, 0xff), // #ffffff
            },
        }
    }

    pub fn dark() -> Self {
        Self {
            mode: ThemeMode::Dark,
            colors: ColorTokens {
                background: Color::rgb(0x1e, 0x21, 0x24),     // #1e2124
                surface: Color::rgb(0x26, 0x2b, 0x2f),        // #262b2f
                surface_hover: Color::rgb(0x36, 0x39, 0x3e),  // #36393e
                border: Color::rgb(0x4c, 0x50, 0x53),         // #4c5053
                text_primary: Color::rgb(0xff, 0xff, 0xff),   // #ffffff
                text_secondary: Color::rgb(0xa0, 0xa0, 0xa0), // #a0a0a0
                text_muted: Color::rgb(0x8e, 0x92, 0x97),     // #8e9297
                accent: Color::rgb(0x00, 0xa7, 0xa0),         // #00a7a0 (teal)
                accent_hover: Color::rgb(0x00, 0xc4, 0xbc),   // #00c4bc
                success: Color::rgb(0x4c, 0xaf, 0x50),        // #4caf50
                warning: Color::rgb(0xff, 0x98, 0x00),        // #ff9800
                error: Color::rgb(0xce, 0x39, 0x33),          // #ce3933
                info: Color::rgb(0x00, 0xa7, 0xa0),           // #00a7a0
                sidebar_bg: Color::rgb(0x36, 0x39, 0x3e),     // #36393e
                header_bg: Color::rgb(0x26, 0x2b, 0x2f),      // #262b2f
                selection: Color::rgba(0x00, 0xa7, 0xa0, 40), // rgba(0,167,160,40)
                table_stripe: Color::rgb(0x2a, 0x2d, 0x33),   // #2a2d33
                table_selected: Color::rgb(0x38, 0x3c, 0x42), // #383c42
                tab_bar_bg: Color::rgb(0x26, 0x2b, 0x2f),     // #262b2f
                tab_active_bg: Color::rgb(0x1e, 0x21, 0x24),  // #1e2124
            },
        }
    }

    pub fn for_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Light => Self::light(),
            ThemeMode::Dark => Self::dark(),
            ThemeMode::System => Self::dark(), // default to dark for system
        }
    }
}

/// Action struct for triggering a theme toggle from the UI.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeToggleAction;

#[derive(Debug)]
pub struct ThemeManager {
    current: Theme,
    system_is_dark: bool,
}

impl ThemeManager {
    pub fn new(mode: ThemeMode) -> Self {
        let system_is_dark = true; // macOS default detection would go here
        let mut current = match mode {
            ThemeMode::System => {
                if system_is_dark {
                    Theme::dark()
                } else {
                    Theme::light()
                }
            }
            other => Theme::for_mode(other),
        };
        current.mode = mode;
        Self { current, system_is_dark }
    }

    pub fn current(&self) -> &Theme {
        &self.current
    }

    pub fn set_mode(&mut self, mode: ThemeMode) {
        self.current = match mode {
            ThemeMode::System => {
                if self.system_is_dark {
                    Theme::dark()
                } else {
                    Theme::light()
                }
            }
            other => Theme::for_mode(other),
        };
        self.current.mode = mode;
    }

    pub fn toggle(&mut self) {
        let new_mode = match self.current.mode {
            ThemeMode::Light => ThemeMode::Dark,
            ThemeMode::Dark => ThemeMode::Light,
            ThemeMode::System => ThemeMode::Light,
        };
        self.set_mode(new_mode);
    }

    /// Toggle the theme and return the new current theme.
    pub fn apply_toggle(&mut self) -> Theme {
        self.toggle();
        self.current.clone()
    }

    pub fn update_system_appearance(&mut self, is_dark: bool) {
        self.system_is_dark = is_dark;
        if self.current.mode == ThemeMode::System {
            self.current = if is_dark { Theme::dark() } else { Theme::light() };
            self.current.mode = ThemeMode::System;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_to_gpui() {
        let color = Color::rgb(255, 128, 0);
        let rgba = color.to_gpui();
        assert!((rgba.r - 1.0).abs() < 0.01);
        assert!((rgba.g - 0.502).abs() < 0.01);
        assert!((rgba.b - 0.0).abs() < 0.01);
        assert!((rgba.a - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_light_theme_colors() {
        let theme = Theme::light();
        assert_eq!(theme.mode, ThemeMode::Light);
        assert_eq!(theme.colors.background, Color::rgb(255, 255, 255));
        assert_eq!(theme.colors.text_primary, Color::rgb(17, 24, 39));
    }

    #[test]
    fn test_dark_theme_colors() {
        let theme = Theme::dark();
        assert_eq!(theme.mode, ThemeMode::Dark);
        assert_eq!(theme.colors.background, Color::rgb(0x1e, 0x21, 0x24));
        assert_eq!(theme.colors.text_primary, Color::rgb(0xff, 0xff, 0xff));
    }

    #[test]
    fn test_color_hex_conversion() {
        assert_eq!(Color::rgb(255, 0, 0).to_hex(), "#ff0000");
        assert_eq!(Color::rgb(0, 128, 255).to_hex(), "#0080ff");
        assert_eq!(Color::rgba(255, 255, 255, 128).to_hex(), "#ffffff80");
    }

    #[test]
    fn test_theme_manager_toggle() {
        let mut manager = ThemeManager::new(ThemeMode::Light);
        assert_eq!(manager.current().mode, ThemeMode::Light);

        manager.toggle();
        assert_eq!(manager.current().mode, ThemeMode::Dark);

        manager.toggle();
        assert_eq!(manager.current().mode, ThemeMode::Light);
    }

    #[test]
    fn test_theme_manager_system_mode() {
        let mut manager = ThemeManager::new(ThemeMode::System);

        manager.update_system_appearance(true);
        assert_eq!(manager.current().colors.background, Color::rgb(0x1e, 0x21, 0x24));

        manager.update_system_appearance(false);
        assert_eq!(manager.current().colors.background, Color::rgb(255, 255, 255));
        assert_eq!(manager.current().mode, ThemeMode::System);
    }

    #[test]
    fn test_system_appearance_change_only_affects_system_mode() {
        let mut manager = ThemeManager::new(ThemeMode::Dark);
        let dark_bg = manager.current().colors.background;

        manager.update_system_appearance(false);
        assert_eq!(manager.current().colors.background, dark_bg);
    }

    #[test]
    fn test_theme_mode_default() {
        assert_eq!(ThemeMode::default(), ThemeMode::System);
    }

    #[test]
    fn test_for_mode() {
        let light = Theme::for_mode(ThemeMode::Light);
        assert_eq!(light.colors.background, Color::rgb(255, 255, 255));

        let dark = Theme::for_mode(ThemeMode::Dark);
        assert_eq!(dark.colors.background, Color::rgb(0x1e, 0x21, 0x24));
    }
}
