// T087: Global Search Rendering tests

use baeus_ui::components::search_bar::*;

// ========================================================================
// GlobalSearchState construction and defaults
// ========================================================================

#[test]
fn test_global_search_new_default_scope() {
    let state = GlobalSearchState::new();
    assert_eq!(state.scope, SearchScope::AllNamespaces);
    assert!(state.query.is_empty());
    assert!(state.results.is_empty());
    assert_eq!(state.selected_result_index, None);
    assert!(!state.is_open);
    assert!(!state.is_searching);
}

#[test]
fn test_global_search_default_trait() {
    let state = GlobalSearchState::default();
    assert_eq!(state.scope, SearchScope::AllNamespaces);
}

// ========================================================================
// Query management
// ========================================================================

#[test]
fn test_global_search_set_query() {
    let mut state = GlobalSearchState::new();
    state.set_query("nginx");
    assert_eq!(state.query, "nginx");
}

#[test]
fn test_global_search_set_query_overwrites() {
    let mut state = GlobalSearchState::new();
    state.set_query("nginx");
    state.set_query("redis");
    assert_eq!(state.query, "redis");
}

#[test]
fn test_global_search_set_query_empty() {
    let mut state = GlobalSearchState::new();
    state.set_query("nginx");
    state.set_query("");
    assert!(state.query.is_empty());
}

// ========================================================================
// Scope changes
// ========================================================================

#[test]
fn test_global_search_set_scope_current_namespace() {
    let mut state = GlobalSearchState::new();
    state.set_scope(SearchScope::CurrentNamespace);
    assert_eq!(state.scope, SearchScope::CurrentNamespace);
}

#[test]
fn test_global_search_set_scope_all_clusters() {
    let mut state = GlobalSearchState::new();
    state.set_scope(SearchScope::AllClusters);
    assert_eq!(state.scope, SearchScope::AllClusters);
}

#[test]
fn test_global_search_scope_label_all_namespaces() {
    let state = GlobalSearchState::new();
    assert_eq!(state.scope_label(), "All Namespaces");
}

#[test]
fn test_global_search_scope_label_current_namespace() {
    let mut state = GlobalSearchState::new();
    state.set_scope(SearchScope::CurrentNamespace);
    assert_eq!(state.scope_label(), "Current Namespace");
}

#[test]
fn test_global_search_scope_label_all_clusters() {
    let mut state = GlobalSearchState::new();
    state.set_scope(SearchScope::AllClusters);
    assert_eq!(state.scope_label(), "All Clusters");
}

// ========================================================================
// Result setting and selection
// ========================================================================

fn make_search_match(name: &str, kind: &str) -> SearchMatch {
    SearchMatch {
        uid: format!("uid-{name}"),
        name: name.to_string(),
        namespace: Some("default".to_string()),
        kind: kind.to_string(),
        score: 100,
        matched_field: "name".to_string(),
    }
}

#[test]
fn test_global_search_set_results_selects_first() {
    let mut state = GlobalSearchState::new();
    state.set_results(vec![
        make_search_match("nginx", "Pod"),
        make_search_match("redis", "Deployment"),
    ]);
    assert_eq!(state.result_count(), 2);
    assert_eq!(state.selected_result_index, Some(0));
}

#[test]
fn test_global_search_set_results_empty_clears_selection() {
    let mut state = GlobalSearchState::new();
    state.set_results(vec![make_search_match("nginx", "Pod")]);
    assert_eq!(state.selected_result_index, Some(0));
    state.set_results(vec![]);
    assert_eq!(state.selected_result_index, None);
    assert_eq!(state.result_count(), 0);
}

#[test]
fn test_global_search_set_results_clears_searching() {
    let mut state = GlobalSearchState::new();
    state.is_searching = true;
    state.set_results(vec![make_search_match("nginx", "Pod")]);
    assert!(!state.is_searching);
}

// ========================================================================
// Result navigation (next/previous with wrapping)
// ========================================================================

#[test]
fn test_global_search_select_next() {
    let mut state = GlobalSearchState::new();
    state.set_results(vec![
        make_search_match("a", "Pod"),
        make_search_match("b", "Pod"),
        make_search_match("c", "Pod"),
    ]);
    assert_eq!(state.selected_result_index, Some(0));
    state.select_next();
    assert_eq!(state.selected_result_index, Some(1));
    state.select_next();
    assert_eq!(state.selected_result_index, Some(2));
}

#[test]
fn test_global_search_select_next_wraps() {
    let mut state = GlobalSearchState::new();
    state.set_results(vec![make_search_match("a", "Pod"), make_search_match("b", "Pod")]);
    state.select_next(); // 0 -> 1
    state.select_next(); // 1 -> 0 (wrap)
    assert_eq!(state.selected_result_index, Some(0));
}

#[test]
fn test_global_search_select_next_empty_results() {
    let mut state = GlobalSearchState::new();
    state.select_next();
    assert_eq!(state.selected_result_index, None);
}

#[test]
fn test_global_search_select_next_from_none() {
    let mut state = GlobalSearchState::new();
    state.set_results(vec![make_search_match("a", "Pod")]);
    state.selected_result_index = None;
    state.select_next();
    assert_eq!(state.selected_result_index, Some(0));
}

#[test]
fn test_global_search_select_previous() {
    let mut state = GlobalSearchState::new();
    state.set_results(vec![
        make_search_match("a", "Pod"),
        make_search_match("b", "Pod"),
        make_search_match("c", "Pod"),
    ]);
    state.selected_result_index = Some(2);
    state.select_previous();
    assert_eq!(state.selected_result_index, Some(1));
    state.select_previous();
    assert_eq!(state.selected_result_index, Some(0));
}

#[test]
fn test_global_search_select_previous_wraps() {
    let mut state = GlobalSearchState::new();
    state.set_results(vec![
        make_search_match("a", "Pod"),
        make_search_match("b", "Pod"),
        make_search_match("c", "Pod"),
    ]);
    // Starting at 0, previous should wrap to last
    state.select_previous();
    assert_eq!(state.selected_result_index, Some(2));
}

#[test]
fn test_global_search_select_previous_empty_results() {
    let mut state = GlobalSearchState::new();
    state.select_previous();
    assert_eq!(state.selected_result_index, None);
}

#[test]
fn test_global_search_select_previous_from_none() {
    let mut state = GlobalSearchState::new();
    state.set_results(vec![make_search_match("a", "Pod"), make_search_match("b", "Pod")]);
    state.selected_result_index = None;
    state.select_previous();
    assert_eq!(state.selected_result_index, Some(1));
}

// ========================================================================
// Selected result
// ========================================================================

#[test]
fn test_global_search_selected_result() {
    let mut state = GlobalSearchState::new();
    state.set_results(vec![
        make_search_match("nginx", "Pod"),
        make_search_match("redis", "Deployment"),
    ]);
    let result = state.selected_result().unwrap();
    assert_eq!(result.name, "nginx");
}

#[test]
fn test_global_search_selected_result_after_next() {
    let mut state = GlobalSearchState::new();
    state.set_results(vec![
        make_search_match("nginx", "Pod"),
        make_search_match("redis", "Deployment"),
    ]);
    state.select_next();
    let result = state.selected_result().unwrap();
    assert_eq!(result.name, "redis");
}

#[test]
fn test_global_search_selected_result_none() {
    let state = GlobalSearchState::new();
    assert!(state.selected_result().is_none());
}

// ========================================================================
// Open / close / toggle
// ========================================================================

#[test]
fn test_global_search_open() {
    let mut state = GlobalSearchState::new();
    state.open();
    assert!(state.is_open);
}

#[test]
fn test_global_search_close() {
    let mut state = GlobalSearchState::new();
    state.open();
    state.close();
    assert!(!state.is_open);
}

#[test]
fn test_global_search_toggle_open() {
    let mut state = GlobalSearchState::new();
    state.toggle();
    assert!(state.is_open);
}

#[test]
fn test_global_search_toggle_close() {
    let mut state = GlobalSearchState::new();
    state.open();
    state.toggle();
    assert!(!state.is_open);
}

#[test]
fn test_global_search_toggle_twice() {
    let mut state = GlobalSearchState::new();
    state.toggle();
    state.toggle();
    assert!(!state.is_open);
}

// ========================================================================
// Clear
// ========================================================================

#[test]
fn test_global_search_clear_resets_everything() {
    let mut state = GlobalSearchState::new();
    state.set_query("test");
    state.set_results(vec![make_search_match("nginx", "Pod")]);
    state.is_searching = true;

    state.clear();

    assert!(state.query.is_empty());
    assert!(state.results.is_empty());
    assert_eq!(state.selected_result_index, None);
    assert!(!state.is_searching);
}

#[test]
fn test_global_search_clear_preserves_scope() {
    let mut state = GlobalSearchState::new();
    state.set_scope(SearchScope::AllClusters);
    state.set_query("test");
    state.clear();
    assert_eq!(state.scope, SearchScope::AllClusters);
}

#[test]
fn test_global_search_clear_preserves_open_state() {
    let mut state = GlobalSearchState::new();
    state.open();
    state.set_query("test");
    state.clear();
    assert!(state.is_open);
}

// ========================================================================
// Result count
// ========================================================================

#[test]
fn test_global_search_result_count_empty() {
    let state = GlobalSearchState::new();
    assert_eq!(state.result_count(), 0);
}

#[test]
fn test_global_search_result_count_with_results() {
    let mut state = GlobalSearchState::new();
    state.set_results(vec![
        make_search_match("a", "Pod"),
        make_search_match("b", "Pod"),
        make_search_match("c", "Pod"),
    ]);
    assert_eq!(state.result_count(), 3);
}

// ========================================================================
// SearchScope equality
// ========================================================================

#[test]
fn test_search_scope_equality() {
    assert_eq!(SearchScope::CurrentNamespace, SearchScope::CurrentNamespace);
    assert_eq!(SearchScope::AllNamespaces, SearchScope::AllNamespaces);
    assert_eq!(SearchScope::AllClusters, SearchScope::AllClusters);
    assert_ne!(SearchScope::CurrentNamespace, SearchScope::AllNamespaces);
}

// ========================================================================
// GlobalSearchView rendering helpers
// ========================================================================

#[test]
fn test_global_search_view_creation() {
    let state = GlobalSearchState::new();
    let view = GlobalSearchView::new(state, baeus_ui::theme::Theme::dark());
    assert!(!view.state.is_open);
}

#[test]
fn test_global_search_view_with_open_state() {
    let mut state = GlobalSearchState::new();
    state.open();
    state.set_query("test");
    let view = GlobalSearchView::new(state, baeus_ui::theme::Theme::dark());
    assert!(view.state.is_open);
    assert_eq!(view.state.query, "test");
}
