// Tests extracted from crates/baeus-ui/src/layout/command_palette.rs

use baeus_ui::layout::command_palette::*;

fn sample_commands() -> Vec<CommandEntry> {
    vec![
        CommandEntry::new(
            "nav-dashboard",
            "Dashboard",
            "Go to the cluster dashboard",
            CommandCategory::Navigation,
            "navigate:dashboard",
        )
        .with_shortcut("Cmd+1"),
        CommandEntry::new(
            "nav-pods",
            "Pods",
            "View all pods",
            CommandCategory::Navigation,
            "navigate:pods",
        ),
        CommandEntry::new(
            "action-scale",
            "Scale Deployment",
            "Scale a deployment to N replicas",
            CommandCategory::Action,
            "action:scale",
        ),
        CommandEntry::new(
            "action-restart",
            "Restart Workload",
            "Restart a deployment or statefulset",
            CommandCategory::Action,
            "action:restart",
        ),
        CommandEntry::new(
            "resource-nginx",
            "nginx-pod",
            "Pod in default namespace",
            CommandCategory::Resource,
            "resource:pod/nginx-pod",
        ),
        CommandEntry::new(
            "view-yaml",
            "YAML Editor",
            "Open the YAML editor view",
            CommandCategory::View,
            "view:yaml",
        ),
    ]
}

// ========================================================================
// CommandCategory tests
// ========================================================================

#[test]
fn test_command_category_equality() {
    assert_eq!(CommandCategory::Navigation, CommandCategory::Navigation);
    assert_ne!(CommandCategory::Navigation, CommandCategory::Action);
    assert_ne!(CommandCategory::Resource, CommandCategory::View);
}

#[test]
fn test_command_category_copy() {
    let cat = CommandCategory::Action;
    let copy = cat;
    assert_eq!(cat, copy);
}

// ========================================================================
// CommandEntry tests
// ========================================================================

#[test]
fn test_command_entry_new() {
    let entry = CommandEntry::new(
        "test-id",
        "Test Label",
        "Test Description",
        CommandCategory::Action,
        "test:action",
    );
    assert_eq!(entry.id, "test-id");
    assert_eq!(entry.label, "Test Label");
    assert_eq!(entry.description, "Test Description");
    assert_eq!(entry.category, CommandCategory::Action);
    assert_eq!(entry.action, "test:action");
    assert!(entry.shortcut.is_none());
}

#[test]
fn test_command_entry_with_shortcut() {
    let entry =
        CommandEntry::new("test-id", "Test", "Desc", CommandCategory::Navigation, "nav:test")
            .with_shortcut("Cmd+T");
    assert_eq!(entry.shortcut, Some("Cmd+T".to_string()));
}

#[test]
fn test_command_entry_without_shortcut() {
    let entry =
        CommandEntry::new("test-id", "Test", "Desc", CommandCategory::Action, "action:test");
    assert!(entry.shortcut.is_none());
}

// ========================================================================
// CommandPaletteState default / new tests
// ========================================================================

#[test]
fn test_default_state() {
    let state = CommandPaletteState::default();
    assert!(state.query.is_empty());
    assert!(state.results.is_empty());
    assert_eq!(state.selected_index, 0);
    assert!(!state.visible);
    assert!(!state.loading);
}

#[test]
fn test_new_with_commands() {
    let commands = sample_commands();
    let state = CommandPaletteState::new(commands.clone());
    assert_eq!(state.commands.len(), commands.len());
    assert!(!state.visible);
    assert!(state.query.is_empty());
}

// ========================================================================
// open / close / toggle tests
// ========================================================================

#[test]
fn test_open() {
    let mut state = CommandPaletteState::new(sample_commands());
    state.open();
    assert!(state.visible);
    assert!(state.query.is_empty());
    assert_eq!(state.selected_index, 0);
    assert!(!state.loading);
}

#[test]
fn test_open_resets_query() {
    let mut state = CommandPaletteState::new(sample_commands());
    state.open();
    state.set_query("pods");
    state.selected_index = 2;
    // Opening again should reset everything
    state.open();
    assert!(state.query.is_empty());
    assert_eq!(state.selected_index, 0);
    assert!(state.results.is_empty());
}

#[test]
fn test_close() {
    let mut state = CommandPaletteState::new(sample_commands());
    state.open();
    state.set_query("test");
    state.close();
    assert!(!state.visible);
    assert!(state.query.is_empty());
    assert!(state.results.is_empty());
    assert_eq!(state.selected_index, 0);
}

#[test]
fn test_toggle_open() {
    let mut state = CommandPaletteState::new(sample_commands());
    assert!(!state.visible);
    state.toggle();
    assert!(state.visible);
}

#[test]
fn test_toggle_close() {
    let mut state = CommandPaletteState::new(sample_commands());
    state.open();
    assert!(state.visible);
    state.toggle();
    assert!(!state.visible);
}

#[test]
fn test_toggle_twice_returns_to_closed() {
    let mut state = CommandPaletteState::new(sample_commands());
    state.toggle();
    state.toggle();
    assert!(!state.visible);
}

// ========================================================================
// set_query and filtered_results tests
// ========================================================================

#[test]
fn test_set_query_updates_results() {
    let mut state = CommandPaletteState::new(sample_commands());
    state.open();
    state.set_query("Dashboard");
    assert!(!state.results.is_empty());
    // The Dashboard entry should be the top result
    assert_eq!(state.results[0].entry.id, "nav-dashboard");
}

#[test]
fn test_set_query_resets_selected_index() {
    let mut state = CommandPaletteState::new(sample_commands());
    state.open();
    state.set_query("pod");
    state.selected_index = 2;
    state.set_query("dash");
    assert_eq!(state.selected_index, 0);
}

#[test]
fn test_empty_query_returns_all_commands() {
    let commands = sample_commands();
    let state = CommandPaletteState::new(commands.clone());
    let results = state.filtered_results();
    assert_eq!(results.len(), commands.len());
}

#[test]
fn test_filtered_results_fuzzy_match_label() {
    let mut state = CommandPaletteState::new(sample_commands());
    state.open();
    state.set_query("Pods");
    let has_pods = state.results.iter().any(|r| r.entry.id == "nav-pods");
    assert!(has_pods, "Should match 'Pods' command by label");
}

#[test]
fn test_filtered_results_fuzzy_match_description() {
    let mut state = CommandPaletteState::new(sample_commands());
    state.open();
    state.set_query("replicas");
    let has_scale = state.results.iter().any(|r| r.entry.id == "action-scale");
    assert!(has_scale, "Should match 'Scale Deployment' by description containing 'replicas'");
}

#[test]
fn test_filtered_results_no_match() {
    let mut state = CommandPaletteState::new(sample_commands());
    state.open();
    state.set_query("zzzzzzz");
    assert!(state.results.is_empty());
}

#[test]
fn test_filtered_results_sorted_by_score() {
    let mut state = CommandPaletteState::new(sample_commands());
    state.open();
    state.set_query("pod");
    for window in state.results.windows(2) {
        assert!(window[0].score >= window[1].score, "Results should be sorted by score descending");
    }
}

#[test]
fn test_filtered_results_case_insensitive() {
    let mut state = CommandPaletteState::new(sample_commands());
    state.open();
    state.set_query("dashboard");
    let has_dashboard = state.results.iter().any(|r| r.entry.id == "nav-dashboard");
    assert!(has_dashboard, "Case-insensitive matching should work");
}

// ========================================================================
// select_next / select_previous tests
// ========================================================================

#[test]
fn test_select_next() {
    let mut state = CommandPaletteState::new(sample_commands());
    state.open();
    // Empty query returns all commands
    state.set_query("");
    assert_eq!(state.selected_index, 0);
    state.select_next();
    assert_eq!(state.selected_index, 1);
}

#[test]
fn test_select_next_wraps_around() {
    let mut state = CommandPaletteState::new(sample_commands());
    state.open();
    state.set_query("");
    let count = state.results.len();
    // Move to last
    for _ in 0..count - 1 {
        state.select_next();
    }
    assert_eq!(state.selected_index, count - 1);
    // One more should wrap to 0
    state.select_next();
    assert_eq!(state.selected_index, 0);
}

#[test]
fn test_select_next_empty_results() {
    let mut state = CommandPaletteState::new(sample_commands());
    state.open();
    state.set_query("zzzzzzz");
    assert!(state.results.is_empty());
    // Should not panic
    state.select_next();
    assert_eq!(state.selected_index, 0);
}

#[test]
fn test_select_previous() {
    let mut state = CommandPaletteState::new(sample_commands());
    state.open();
    state.set_query("");
    state.select_next();
    state.select_next();
    assert_eq!(state.selected_index, 2);
    state.select_previous();
    assert_eq!(state.selected_index, 1);
}

#[test]
fn test_select_previous_wraps_around() {
    let mut state = CommandPaletteState::new(sample_commands());
    state.open();
    state.set_query("");
    let count = state.results.len();
    assert_eq!(state.selected_index, 0);
    state.select_previous();
    assert_eq!(state.selected_index, count - 1);
}

#[test]
fn test_select_previous_empty_results() {
    let mut state = CommandPaletteState::new(sample_commands());
    state.open();
    state.set_query("zzzzzzz");
    assert!(state.results.is_empty());
    // Should not panic
    state.select_previous();
    assert_eq!(state.selected_index, 0);
}

// ========================================================================
// execute_selected tests
// ========================================================================

#[test]
fn test_execute_selected_returns_action() {
    let mut state = CommandPaletteState::new(sample_commands());
    state.open();
    state.set_query("Dashboard");
    let action = state.execute_selected();
    assert_eq!(action, Some("navigate:dashboard".to_string()));
}

#[test]
fn test_execute_selected_closes_palette() {
    let mut state = CommandPaletteState::new(sample_commands());
    state.open();
    state.set_query("Dashboard");
    state.execute_selected();
    assert!(!state.visible);
}

#[test]
fn test_execute_selected_with_no_results() {
    let mut state = CommandPaletteState::new(sample_commands());
    state.open();
    state.set_query("zzzzzzz");
    let action = state.execute_selected();
    assert!(action.is_none());
}

#[test]
fn test_execute_selected_respects_index() {
    let mut state = CommandPaletteState::new(sample_commands());
    state.open();
    state.set_query(""); // all commands visible
    assert!(!state.results.is_empty());
    state.select_next(); // move to index 1
    let action = state.execute_selected();
    assert!(action.is_some());
    // The action should be from the second entry
}

#[test]
fn test_execute_selected_clears_state() {
    let mut state = CommandPaletteState::new(sample_commands());
    state.open();
    state.set_query("Dashboard");
    state.execute_selected();
    assert!(state.query.is_empty());
    assert!(state.results.is_empty());
    assert_eq!(state.selected_index, 0);
}

// ========================================================================
// Category filtering in results
// ========================================================================

#[test]
fn test_results_include_multiple_categories() {
    let mut state = CommandPaletteState::new(sample_commands());
    state.open();
    state.set_query(""); // all results
    let categories: Vec<CommandCategory> = state.results.iter().map(|r| r.entry.category).collect();
    assert!(categories.contains(&CommandCategory::Navigation));
    assert!(categories.contains(&CommandCategory::Action));
    assert!(categories.contains(&CommandCategory::Resource));
    assert!(categories.contains(&CommandCategory::View));
}

// ========================================================================
// Scored command tests
// ========================================================================

#[test]
fn test_exact_match_scores_highest() {
    let mut state = CommandPaletteState::new(sample_commands());
    state.open();
    state.set_query("Pods");
    // "Pods" exact matches "Pods" label
    let pods_entry = state.results.iter().find(|r| r.entry.id == "nav-pods");
    assert!(pods_entry.is_some());
    let pods_score = pods_entry.unwrap().score;
    // All other results should have lower or equal scores
    for r in &state.results {
        if r.entry.id != "nav-pods" {
            assert!(
                pods_score >= r.score,
                "Exact match 'Pods' should score highest, but {} scored {} vs {}",
                r.entry.label,
                r.score,
                pods_score
            );
        }
    }
}

// ========================================================================
// Integration-style tests
// ========================================================================

#[test]
fn test_full_workflow() {
    let mut state = CommandPaletteState::new(sample_commands());

    // Initially closed
    assert!(!state.visible);

    // Open palette
    state.toggle();
    assert!(state.visible);

    // Type a query
    state.set_query("Scale");
    assert!(!state.results.is_empty());
    let has_scale = state.results.iter().any(|r| r.entry.id == "action-scale");
    assert!(has_scale);

    // Navigate down
    state.select_next();

    // Execute
    let action = state.execute_selected();
    assert!(action.is_some());

    // Palette should be closed
    assert!(!state.visible);
}

#[test]
fn test_open_search_navigate_close() {
    let mut state = CommandPaletteState::new(sample_commands());

    state.open();
    state.set_query("pod");

    // Navigate through results
    let result_count = state.results.len();
    if result_count > 1 {
        state.select_next();
        assert_eq!(state.selected_index, 1);
        state.select_previous();
        assert_eq!(state.selected_index, 0);
    }

    // Close without executing
    state.close();
    assert!(!state.visible);
    assert!(state.query.is_empty());
}
