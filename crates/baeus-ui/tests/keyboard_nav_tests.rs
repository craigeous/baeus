// T086: Keyboard Navigation Tests

use baeus_ui::layout::app_shell::{
    AppShellState, Direction, FocusMode, KeyAction, KeyModifiers, KeybindingConfig,
    KeyboardNavigationState,
};

// =========================================================================
// FocusMode transitions
// =========================================================================

#[test]
fn test_default_focus_mode_is_normal() {
    let state = AppShellState::default();
    assert_eq!(state.focus_mode, FocusMode::Normal);
}

#[test]
fn test_enter_table_navigation_from_normal() {
    let mut state = AppShellState::default();
    state.enter_table_navigation();
    assert_eq!(state.focus_mode, FocusMode::TableNavigation { row: 0, col: 0 });
}

#[test]
fn test_exit_focus_mode_from_table_navigation() {
    let mut state = AppShellState::default();
    state.enter_table_navigation();
    state.exit_focus_mode();
    assert_eq!(state.focus_mode, FocusMode::Normal);
}

#[test]
fn test_exit_focus_mode_from_command_palette() {
    let mut state = AppShellState { focus_mode: FocusMode::CommandPalette };
    state.exit_focus_mode();
    assert_eq!(state.focus_mode, FocusMode::Normal);
}

#[test]
fn test_exit_focus_mode_from_search() {
    let mut state = AppShellState { focus_mode: FocusMode::Search };
    state.exit_focus_mode();
    assert_eq!(state.focus_mode, FocusMode::Normal);
}

#[test]
fn test_exit_focus_mode_from_modal() {
    let mut state = AppShellState { focus_mode: FocusMode::Modal };
    state.exit_focus_mode();
    assert_eq!(state.focus_mode, FocusMode::Normal);
}

#[test]
fn test_exit_focus_mode_when_already_normal() {
    let mut state = AppShellState::default();
    state.exit_focus_mode();
    assert_eq!(state.focus_mode, FocusMode::Normal);
}

// =========================================================================
// handle_key_action dispatches correctly
// =========================================================================

#[test]
fn test_handle_key_action_toggle_command_palette_opens() {
    let mut state = AppShellState::default();
    state.handle_key_action(KeyAction::ToggleCommandPalette);
    assert_eq!(state.focus_mode, FocusMode::CommandPalette);
}

#[test]
fn test_handle_key_action_toggle_command_palette_closes() {
    let mut state = AppShellState { focus_mode: FocusMode::CommandPalette };
    state.handle_key_action(KeyAction::ToggleCommandPalette);
    assert_eq!(state.focus_mode, FocusMode::Normal);
}

#[test]
fn test_handle_key_action_focus_search_opens() {
    let mut state = AppShellState::default();
    state.handle_key_action(KeyAction::FocusSearch);
    assert_eq!(state.focus_mode, FocusMode::Search);
}

#[test]
fn test_handle_key_action_focus_search_closes() {
    let mut state = AppShellState { focus_mode: FocusMode::Search };
    state.handle_key_action(KeyAction::FocusSearch);
    assert_eq!(state.focus_mode, FocusMode::Normal);
}

#[test]
fn test_handle_key_action_navigation_does_not_change_focus() {
    let mut state = AppShellState::default();
    state.handle_key_action(KeyAction::NavigateToDashboard);
    assert_eq!(state.focus_mode, FocusMode::Normal);
}

#[test]
fn test_handle_key_action_toggle_sidebar_does_not_change_focus() {
    let mut state = AppShellState::default();
    state.handle_key_action(KeyAction::ToggleSidebar);
    assert_eq!(state.focus_mode, FocusMode::Normal);
}

#[test]
fn test_handle_key_action_refresh_does_not_change_focus() {
    let mut state = AppShellState::default();
    state.handle_key_action(KeyAction::Refresh);
    assert_eq!(state.focus_mode, FocusMode::Normal);
}

// =========================================================================
// Table navigation movement (up/down/left/right with bounds)
// =========================================================================

#[test]
fn test_move_table_down() {
    let mut state = AppShellState::default();
    state.enter_table_navigation();
    state.move_table_selection(Direction::Down, 10, 5);
    assert_eq!(state.focus_mode, FocusMode::TableNavigation { row: 1, col: 0 });
}

#[test]
fn test_move_table_right() {
    let mut state = AppShellState::default();
    state.enter_table_navigation();
    state.move_table_selection(Direction::Right, 10, 5);
    assert_eq!(state.focus_mode, FocusMode::TableNavigation { row: 0, col: 1 });
}

#[test]
fn test_move_table_up_at_zero_stays_at_zero() {
    let mut state = AppShellState::default();
    state.enter_table_navigation();
    state.move_table_selection(Direction::Up, 10, 5);
    assert_eq!(state.focus_mode, FocusMode::TableNavigation { row: 0, col: 0 });
}

#[test]
fn test_move_table_left_at_zero_stays_at_zero() {
    let mut state = AppShellState::default();
    state.enter_table_navigation();
    state.move_table_selection(Direction::Left, 10, 5);
    assert_eq!(state.focus_mode, FocusMode::TableNavigation { row: 0, col: 0 });
}

#[test]
fn test_move_table_down_clamps_at_max() {
    let mut state = AppShellState { focus_mode: FocusMode::TableNavigation { row: 9, col: 0 } };
    state.move_table_selection(Direction::Down, 10, 5);
    assert_eq!(state.focus_mode, FocusMode::TableNavigation { row: 9, col: 0 });
}

#[test]
fn test_move_table_right_clamps_at_max() {
    let mut state = AppShellState { focus_mode: FocusMode::TableNavigation { row: 0, col: 4 } };
    state.move_table_selection(Direction::Right, 10, 5);
    assert_eq!(state.focus_mode, FocusMode::TableNavigation { row: 0, col: 4 });
}

#[test]
fn test_move_table_multiple_directions() {
    let mut state = AppShellState::default();
    state.enter_table_navigation();
    state.move_table_selection(Direction::Down, 10, 5);
    state.move_table_selection(Direction::Down, 10, 5);
    state.move_table_selection(Direction::Right, 10, 5);
    assert_eq!(state.focus_mode, FocusMode::TableNavigation { row: 2, col: 1 });
}

#[test]
fn test_move_table_no_op_when_not_in_table_mode() {
    let mut state = AppShellState::default();
    // In Normal mode, move should do nothing
    state.move_table_selection(Direction::Down, 10, 5);
    assert_eq!(state.focus_mode, FocusMode::Normal);
}

#[test]
fn test_move_table_with_zero_max_rows() {
    let mut state = AppShellState::default();
    state.enter_table_navigation();
    // max_rows = 0, Down should not move
    state.move_table_selection(Direction::Down, 0, 5);
    assert_eq!(state.focus_mode, FocusMode::TableNavigation { row: 0, col: 0 });
}

#[test]
fn test_move_table_with_zero_max_cols() {
    let mut state = AppShellState::default();
    state.enter_table_navigation();
    // max_cols = 0, Right should not move
    state.move_table_selection(Direction::Right, 10, 0);
    assert_eq!(state.focus_mode, FocusMode::TableNavigation { row: 0, col: 0 });
}

// =========================================================================
// Escape exits any focus mode to Normal (simulated via exit_focus_mode)
// =========================================================================

#[test]
fn test_escape_from_all_focus_modes() {
    let modes = vec![
        FocusMode::CommandPalette,
        FocusMode::Search,
        FocusMode::Modal,
        FocusMode::TableNavigation { row: 3, col: 2 },
    ];
    for mode in modes {
        let mut state = AppShellState { focus_mode: mode };
        state.exit_focus_mode();
        assert_eq!(state.focus_mode, FocusMode::Normal);
    }
}

// =========================================================================
// Modal detection
// =========================================================================

#[test]
fn test_is_modal_open_command_palette() {
    let state = AppShellState { focus_mode: FocusMode::CommandPalette };
    assert!(state.is_modal_open());
}

#[test]
fn test_is_modal_open_modal() {
    let state = AppShellState { focus_mode: FocusMode::Modal };
    assert!(state.is_modal_open());
}

#[test]
fn test_is_modal_not_open_normal() {
    let state = AppShellState::default();
    assert!(!state.is_modal_open());
}

#[test]
fn test_is_modal_not_open_search() {
    let state = AppShellState { focus_mode: FocusMode::Search };
    assert!(!state.is_modal_open());
}

#[test]
fn test_is_modal_not_open_table_nav() {
    let state = AppShellState { focus_mode: FocusMode::TableNavigation { row: 0, col: 0 } };
    assert!(!state.is_modal_open());
}

// =========================================================================
// KeyboardNavigationState processes keys correctly
// =========================================================================

#[test]
fn test_keyboard_nav_state_new_has_default_bindings() {
    let nav = KeyboardNavigationState::new();
    assert!(!nav.config.bindings.is_empty());
    assert_eq!(nav.config.bindings.len(), 21);
}

#[test]
fn test_keyboard_nav_state_default_focus_normal() {
    let nav = KeyboardNavigationState::new();
    assert_eq!(nav.focus_mode, FocusMode::Normal);
    assert!(nav.last_action.is_none());
}

#[test]
fn test_keyboard_nav_process_key_cmd_k() {
    let nav = KeyboardNavigationState::new();
    let action = nav.process_key("k", &KeyModifiers::cmd());
    assert_eq!(action, Some(KeyAction::ToggleCommandPalette));
}

#[test]
fn test_keyboard_nav_process_key_cmd_1() {
    let nav = KeyboardNavigationState::new();
    let action = nav.process_key("1", &KeyModifiers::cmd());
    assert_eq!(action, Some(KeyAction::NavigateToDashboard));
}

#[test]
fn test_keyboard_nav_process_key_no_match() {
    let nav = KeyboardNavigationState::new();
    let action = nav.process_key("z", &KeyModifiers::none());
    assert_eq!(action, None);
}

#[test]
fn test_keyboard_nav_process_key_cmd_f() {
    let nav = KeyboardNavigationState::new();
    let action = nav.process_key("f", &KeyModifiers::cmd());
    assert_eq!(action, Some(KeyAction::FocusSearch));
}

#[test]
fn test_keyboard_nav_process_key_cmd_b() {
    let nav = KeyboardNavigationState::new();
    let action = nav.process_key("b", &KeyModifiers::cmd());
    assert_eq!(action, Some(KeyAction::ToggleSidebar));
}

// =========================================================================
// Default bindings all resolve
// =========================================================================

#[test]
fn test_all_default_bindings_resolve() {
    let config = KeybindingConfig::default_bindings();
    // Every binding in default should return its own action
    for binding in &config.bindings {
        let found = config.find_action(&binding.key, &binding.modifiers);
        assert_eq!(found, Some(binding.action));
    }
}

// =========================================================================
// is_shortcut_active
// =========================================================================

#[test]
fn test_is_shortcut_active_true() {
    let nav = KeyboardNavigationState::new();
    assert!(nav.is_shortcut_active(&KeyAction::ToggleCommandPalette));
    assert!(nav.is_shortcut_active(&KeyAction::NavigateToDashboard));
    assert!(nav.is_shortcut_active(&KeyAction::FocusSearch));
    assert!(nav.is_shortcut_active(&KeyAction::Refresh));
}

// =========================================================================
// Direction enum coverage
// =========================================================================

#[test]
fn test_direction_all_variants() {
    let directions = [Direction::Up, Direction::Down, Direction::Left, Direction::Right];
    // Ensure all are distinct
    for (i, a) in directions.iter().enumerate() {
        for (j, b) in directions.iter().enumerate() {
            if i != j {
                assert_ne!(a, b);
            }
        }
    }
}

#[test]
fn test_direction_clone() {
    let d = Direction::Up;
    let cloned = d.clone();
    assert_eq!(d, cloned);
}

#[test]
fn test_direction_debug() {
    assert_eq!(format!("{:?}", Direction::Up), "Up");
    assert_eq!(format!("{:?}", Direction::Down), "Down");
    assert_eq!(format!("{:?}", Direction::Left), "Left");
    assert_eq!(format!("{:?}", Direction::Right), "Right");
}

// =========================================================================
// FocusMode enum coverage
// =========================================================================

#[test]
fn test_focus_mode_all_variants_distinct() {
    let modes = [
        FocusMode::Normal,
        FocusMode::TableNavigation { row: 0, col: 0 },
        FocusMode::CommandPalette,
        FocusMode::Search,
        FocusMode::Modal,
    ];
    for (i, a) in modes.iter().enumerate() {
        for (j, b) in modes.iter().enumerate() {
            if i != j {
                assert_ne!(a, b);
            }
        }
    }
}

#[test]
fn test_focus_mode_default() {
    let mode = FocusMode::default();
    assert_eq!(mode, FocusMode::Normal);
}

#[test]
fn test_focus_mode_clone() {
    let mode = FocusMode::TableNavigation { row: 3, col: 5 };
    let cloned = mode.clone();
    assert_eq!(mode, cloned);
}

#[test]
fn test_focus_mode_debug() {
    let debug = format!("{:?}", FocusMode::Normal);
    assert_eq!(debug, "Normal");
}

// =========================================================================
// KeyboardNavigationState default trait
// =========================================================================

#[test]
fn test_keyboard_nav_state_default() {
    let nav = KeyboardNavigationState::default();
    assert_eq!(nav.focus_mode, FocusMode::Normal);
    assert!(nav.last_action.is_none());
    assert_eq!(nav.config.bindings.len(), 21);
}
