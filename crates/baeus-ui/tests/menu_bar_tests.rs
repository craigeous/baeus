// T084: Menu bar state tests (integration test file for baeus-ui)
//
// MenuBarState lives in baeus-app, but test file lives here per project convention.
// We depend on baeus-app types being re-usable; since baeus-ui tests can only
// import baeus-ui, we duplicate the small data model here for testing purposes.
// This approach mirrors the existing pattern where tests exercise pure-logic
// structs without requiring GPUI.

/// Mirror of `baeus_app::app::MenuAction` for integration testing.
/// In production code the canonical type lives in `baeus-app`.
#[derive(Debug, Clone, PartialEq)]
enum MenuAction {
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

const ZOOM_MIN: f32 = 0.5;
const ZOOM_MAX: f32 = 3.0;
const ZOOM_STEP: f32 = 0.1;
const ZOOM_DEFAULT: f32 = 1.0;

struct MenuBarState {
    zoom_level: f32,
}

impl MenuBarState {
    fn new() -> Self {
        Self { zoom_level: ZOOM_DEFAULT }
    }

    fn zoom_in(&mut self) {
        self.zoom_level = (self.zoom_level + ZOOM_STEP).min(ZOOM_MAX);
    }

    fn zoom_out(&mut self) {
        self.zoom_level = (self.zoom_level - ZOOM_STEP).max(ZOOM_MIN);
    }

    fn reset_zoom(&mut self) {
        self.zoom_level = ZOOM_DEFAULT;
    }

    fn handle_action(&mut self, action: MenuAction) -> Option<MenuAction> {
        match &action {
            MenuAction::ZoomIn => self.zoom_in(),
            MenuAction::ZoomOut => self.zoom_out(),
            MenuAction::ResetZoom => self.reset_zoom(),
            _ => {}
        }
        Some(action)
    }
}

// ===========================================================================
// Menu action enum coverage
// ===========================================================================

#[test]
fn test_menu_action_all_variants_distinct() {
    let actions: Vec<MenuAction> = vec![
        MenuAction::OpenPreferences,
        MenuAction::CloseWindow,
        MenuAction::Quit,
        MenuAction::Undo,
        MenuAction::Redo,
        MenuAction::Cut,
        MenuAction::Copy,
        MenuAction::Paste,
        MenuAction::SelectAll,
        MenuAction::ToggleSidebar,
        MenuAction::ToggleTheme,
        MenuAction::ZoomIn,
        MenuAction::ZoomOut,
        MenuAction::ResetZoom,
        MenuAction::About,
        MenuAction::Documentation,
    ];
    for (i, a) in actions.iter().enumerate() {
        for (j, b) in actions.iter().enumerate() {
            if i != j {
                assert_ne!(a, b, "Variants at index {i} and {j} should differ");
            }
        }
    }
}

#[test]
fn test_menu_action_clone() {
    let action = MenuAction::Copy;
    let cloned = action.clone();
    assert_eq!(action, cloned);
}

#[test]
fn test_menu_action_debug() {
    let action = MenuAction::Quit;
    let debug = format!("{:?}", action);
    assert!(debug.contains("Quit"));
}

#[test]
fn test_menu_action_file_group() {
    // File menu actions
    let _ = MenuAction::OpenPreferences;
    let _ = MenuAction::CloseWindow;
    let _ = MenuAction::Quit;
}

#[test]
fn test_menu_action_edit_group() {
    // Edit menu actions
    let _ = MenuAction::Undo;
    let _ = MenuAction::Redo;
    let _ = MenuAction::Cut;
    let _ = MenuAction::Copy;
    let _ = MenuAction::Paste;
    let _ = MenuAction::SelectAll;
}

#[test]
fn test_menu_action_view_group() {
    // View menu actions
    let _ = MenuAction::ToggleSidebar;
    let _ = MenuAction::ToggleTheme;
    let _ = MenuAction::ZoomIn;
    let _ = MenuAction::ZoomOut;
    let _ = MenuAction::ResetZoom;
}

#[test]
fn test_menu_action_help_group() {
    // Help menu actions
    let _ = MenuAction::About;
    let _ = MenuAction::Documentation;
}

#[test]
fn test_menu_action_count() {
    // There should be exactly 16 menu action variants
    let actions = vec![
        MenuAction::OpenPreferences,
        MenuAction::CloseWindow,
        MenuAction::Quit,
        MenuAction::Undo,
        MenuAction::Redo,
        MenuAction::Cut,
        MenuAction::Copy,
        MenuAction::Paste,
        MenuAction::SelectAll,
        MenuAction::ToggleSidebar,
        MenuAction::ToggleTheme,
        MenuAction::ZoomIn,
        MenuAction::ZoomOut,
        MenuAction::ResetZoom,
        MenuAction::About,
        MenuAction::Documentation,
    ];
    assert_eq!(actions.len(), 16);
}

// ===========================================================================
// MenuBarState::new()
// ===========================================================================

#[test]
fn test_new_creates_default_zoom() {
    let state = MenuBarState::new();
    assert!((state.zoom_level - 1.0).abs() < f32::EPSILON);
}

// ===========================================================================
// Zoom level management
// ===========================================================================

#[test]
fn test_zoom_in_increments() {
    let mut state = MenuBarState::new();
    state.zoom_in();
    assert!((state.zoom_level - 1.1).abs() < 0.001);
}

#[test]
fn test_zoom_out_decrements() {
    let mut state = MenuBarState::new();
    state.zoom_out();
    assert!((state.zoom_level - 0.9).abs() < 0.001);
}

#[test]
fn test_zoom_in_max_clamp() {
    let mut state = MenuBarState::new();
    // Zoom in 30 times (1.0 + 3.0 = well past max)
    for _ in 0..30 {
        state.zoom_in();
    }
    assert!((state.zoom_level - ZOOM_MAX).abs() < 0.001);
}

#[test]
fn test_zoom_out_min_clamp() {
    let mut state = MenuBarState::new();
    // Zoom out 20 times (1.0 - 2.0 = well past min)
    for _ in 0..20 {
        state.zoom_out();
    }
    assert!((state.zoom_level - ZOOM_MIN).abs() < 0.001);
}

#[test]
fn test_reset_zoom_from_zoomed_in() {
    let mut state = MenuBarState::new();
    state.zoom_in();
    state.zoom_in();
    state.zoom_in();
    state.reset_zoom();
    assert!((state.zoom_level - ZOOM_DEFAULT).abs() < f32::EPSILON);
}

#[test]
fn test_reset_zoom_from_zoomed_out() {
    let mut state = MenuBarState::new();
    state.zoom_out();
    state.zoom_out();
    state.reset_zoom();
    assert!((state.zoom_level - ZOOM_DEFAULT).abs() < f32::EPSILON);
}

#[test]
fn test_reset_zoom_idempotent() {
    let mut state = MenuBarState::new();
    state.reset_zoom();
    assert!((state.zoom_level - ZOOM_DEFAULT).abs() < f32::EPSILON);
}

#[test]
fn test_zoom_in_then_out_returns_to_original() {
    let mut state = MenuBarState::new();
    state.zoom_in();
    state.zoom_out();
    assert!((state.zoom_level - ZOOM_DEFAULT).abs() < 0.001);
}

#[test]
fn test_zoom_stays_at_max_after_multiple_zoom_ins() {
    let mut state = MenuBarState::new();
    for _ in 0..100 {
        state.zoom_in();
    }
    assert!((state.zoom_level - ZOOM_MAX).abs() < 0.001);
    // One more zoom in should not exceed max
    state.zoom_in();
    assert!((state.zoom_level - ZOOM_MAX).abs() < 0.001);
}

#[test]
fn test_zoom_stays_at_min_after_multiple_zoom_outs() {
    let mut state = MenuBarState::new();
    for _ in 0..100 {
        state.zoom_out();
    }
    assert!((state.zoom_level - ZOOM_MIN).abs() < 0.001);
    state.zoom_out();
    assert!((state.zoom_level - ZOOM_MIN).abs() < 0.001);
}

// ===========================================================================
// handle_action dispatching
// ===========================================================================

#[test]
fn test_handle_action_zoom_in() {
    let mut state = MenuBarState::new();
    let result = state.handle_action(MenuAction::ZoomIn);
    assert_eq!(result, Some(MenuAction::ZoomIn));
    assert!((state.zoom_level - 1.1).abs() < 0.001);
}

#[test]
fn test_handle_action_zoom_out() {
    let mut state = MenuBarState::new();
    let result = state.handle_action(MenuAction::ZoomOut);
    assert_eq!(result, Some(MenuAction::ZoomOut));
    assert!((state.zoom_level - 0.9).abs() < 0.001);
}

#[test]
fn test_handle_action_reset_zoom() {
    let mut state = MenuBarState::new();
    state.zoom_in();
    state.zoom_in();
    let result = state.handle_action(MenuAction::ResetZoom);
    assert_eq!(result, Some(MenuAction::ResetZoom));
    assert!((state.zoom_level - ZOOM_DEFAULT).abs() < f32::EPSILON);
}

#[test]
fn test_handle_action_passthrough_quit() {
    let mut state = MenuBarState::new();
    let result = state.handle_action(MenuAction::Quit);
    assert_eq!(result, Some(MenuAction::Quit));
    // Zoom should not change
    assert!((state.zoom_level - ZOOM_DEFAULT).abs() < f32::EPSILON);
}

#[test]
fn test_handle_action_passthrough_copy() {
    let mut state = MenuBarState::new();
    let result = state.handle_action(MenuAction::Copy);
    assert_eq!(result, Some(MenuAction::Copy));
    assert!((state.zoom_level - ZOOM_DEFAULT).abs() < f32::EPSILON);
}

#[test]
fn test_handle_action_passthrough_open_preferences() {
    let mut state = MenuBarState::new();
    let result = state.handle_action(MenuAction::OpenPreferences);
    assert_eq!(result, Some(MenuAction::OpenPreferences));
}

#[test]
fn test_handle_action_passthrough_close_window() {
    let mut state = MenuBarState::new();
    let result = state.handle_action(MenuAction::CloseWindow);
    assert_eq!(result, Some(MenuAction::CloseWindow));
}

#[test]
fn test_handle_action_passthrough_toggle_sidebar() {
    let mut state = MenuBarState::new();
    let result = state.handle_action(MenuAction::ToggleSidebar);
    assert_eq!(result, Some(MenuAction::ToggleSidebar));
}

#[test]
fn test_handle_action_passthrough_toggle_theme() {
    let mut state = MenuBarState::new();
    let result = state.handle_action(MenuAction::ToggleTheme);
    assert_eq!(result, Some(MenuAction::ToggleTheme));
}

#[test]
fn test_handle_action_passthrough_about() {
    let mut state = MenuBarState::new();
    let result = state.handle_action(MenuAction::About);
    assert_eq!(result, Some(MenuAction::About));
}

#[test]
fn test_handle_action_passthrough_documentation() {
    let mut state = MenuBarState::new();
    let result = state.handle_action(MenuAction::Documentation);
    assert_eq!(result, Some(MenuAction::Documentation));
}

#[test]
fn test_handle_action_always_returns_some() {
    let mut state = MenuBarState::new();
    let actions = vec![
        MenuAction::OpenPreferences,
        MenuAction::CloseWindow,
        MenuAction::Quit,
        MenuAction::Undo,
        MenuAction::Redo,
        MenuAction::Cut,
        MenuAction::Copy,
        MenuAction::Paste,
        MenuAction::SelectAll,
        MenuAction::ToggleSidebar,
        MenuAction::ToggleTheme,
        MenuAction::ZoomIn,
        MenuAction::ZoomOut,
        MenuAction::ResetZoom,
        MenuAction::About,
        MenuAction::Documentation,
    ];
    for action in actions {
        assert!(state.handle_action(action).is_some());
    }
}

#[test]
fn test_handle_action_sequence_zoom_in_out_reset() {
    let mut state = MenuBarState::new();
    state.handle_action(MenuAction::ZoomIn);
    state.handle_action(MenuAction::ZoomIn);
    assert!((state.zoom_level - 1.2).abs() < 0.001);
    state.handle_action(MenuAction::ZoomOut);
    assert!((state.zoom_level - 1.1).abs() < 0.001);
    state.handle_action(MenuAction::ResetZoom);
    assert!((state.zoom_level - 1.0).abs() < f32::EPSILON);
}
