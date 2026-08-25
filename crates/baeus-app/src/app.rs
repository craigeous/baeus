use crate::settings::UserPreferences;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
#[allow(dead_code)]
pub struct App {
    pub preferences: UserPreferences,
}

#[allow(dead_code)]
impl App {
    pub fn new() -> Self {
        let preferences = UserPreferences::load().unwrap_or_default();
        Self { preferences }
    }

    pub fn with_preferences(preferences: UserPreferences) -> Self {
        Self { preferences }
    }
}

// ---------------------------------------------------------------------------
// T084: macOS Menu Bar Integration
// ---------------------------------------------------------------------------

/// Actions that can be triggered from the macOS menu bar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MenuAction {
    // File
    OpenPreferences,
    CloseWindow,
    Quit,
    // Edit
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    // View
    ToggleSidebar,
    ToggleTheme,
    ZoomIn,
    ZoomOut,
    ResetZoom,
    // Help
    About,
    Documentation,
}

/// State for the macOS menu bar, tracking zoom level.
#[derive(Debug)]
#[allow(dead_code)]
pub struct MenuBarState {
    pub zoom_level: f32,
}

const ZOOM_MIN: f32 = 0.5;
const ZOOM_MAX: f32 = 3.0;
const ZOOM_STEP: f32 = 0.1;
const ZOOM_DEFAULT: f32 = 1.0;

#[allow(dead_code)]
impl MenuBarState {
    /// Creates a new `MenuBarState` with the default zoom level of 1.0.
    pub fn new() -> Self {
        Self { zoom_level: ZOOM_DEFAULT }
    }

    /// Increments the zoom level by 0.1, clamped to a maximum of 3.0.
    pub fn zoom_in(&mut self) {
        self.zoom_level = (self.zoom_level + ZOOM_STEP).min(ZOOM_MAX);
    }

    /// Decrements the zoom level by 0.1, clamped to a minimum of 0.5.
    pub fn zoom_out(&mut self) {
        self.zoom_level = (self.zoom_level - ZOOM_STEP).max(ZOOM_MIN);
    }

    /// Resets the zoom level to 1.0.
    pub fn reset_zoom(&mut self) {
        self.zoom_level = ZOOM_DEFAULT;
    }

    /// Handles a menu action and returns the action to be dispatched.
    ///
    /// For zoom actions the state is updated internally and the action is still
    /// returned so callers can propagate it further. All other actions are
    /// passed through unchanged.
    pub fn handle_action(&mut self, action: MenuAction) -> Option<MenuAction> {
        match &action {
            MenuAction::ZoomIn => self.zoom_in(),
            MenuAction::ZoomOut => self.zoom_out(),
            MenuAction::ResetZoom => self.reset_zoom(),
            _ => {}
        }
        Some(action)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_new_loads_defaults() {
        let app = App::with_preferences(UserPreferences::default());
        assert_eq!(app.preferences.font_size, 13.0);
    }
}
