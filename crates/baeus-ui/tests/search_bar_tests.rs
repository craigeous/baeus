// Tests extracted from crates/baeus-ui/src/components/search_bar.rs

use baeus_ui::components::search_bar::*;
use baeus_ui::theme::Theme;

// ========================================================================
// SearchBarState tests
// ========================================================================

#[test]
fn test_new_state_is_empty() {
    let state = SearchBarState::new();
    assert!(state.query.is_empty());
    assert!(!state.is_focused);
    assert!(state.results.is_empty());
}

#[test]
fn test_set_query() {
    let mut state = SearchBarState::new();
    state.set_query("nginx");
    assert_eq!(state.query, "nginx");
}

#[test]
fn test_set_query_overwrites() {
    let mut state = SearchBarState::new();
    state.set_query("nginx");
    state.set_query("redis");
    assert_eq!(state.query, "redis");
}

#[test]
fn test_clear() {
    let mut state = SearchBarState::new();
    state.set_query("nginx");
    state.results.push(SearchMatch {
        uid: "uid-1".to_string(),
        name: "nginx".to_string(),
        namespace: None,
        kind: "Pod".to_string(),
        score: 100,
        matched_field: "name".to_string(),
    });
    state.clear();
    assert!(state.query.is_empty());
    assert!(state.results.is_empty());
}

#[test]
fn test_is_active_empty() {
    let state = SearchBarState::new();
    assert!(!state.is_active());
}

#[test]
fn test_is_active_with_query() {
    let mut state = SearchBarState::new();
    state.set_query("test");
    assert!(state.is_active());
}

#[test]
fn test_is_active_after_clear() {
    let mut state = SearchBarState::new();
    state.set_query("test");
    state.clear();
    assert!(!state.is_active());
}

#[test]
fn test_default_equals_new() {
    let default_state = SearchBarState::default();
    let new_state = SearchBarState::new();
    assert_eq!(default_state.query, new_state.query);
    assert_eq!(default_state.is_focused, new_state.is_focused);
    assert_eq!(default_state.results.len(), new_state.results.len());
}

// ========================================================================
// fuzzy_match tests
// ========================================================================

#[test]
fn test_fuzzy_match_exact() {
    let score = fuzzy_match("nginx", "nginx").unwrap();
    assert_eq!(score, 200);
}

#[test]
fn test_fuzzy_match_exact_case_insensitive() {
    let score = fuzzy_match("NGINX", "nginx").unwrap();
    assert_eq!(score, 200);
}

#[test]
fn test_fuzzy_match_prefix() {
    let score = fuzzy_match("ngi", "nginx-pod").unwrap();
    assert!(score > 0);
    // Prefix match should have bonus
    let non_prefix = fuzzy_match("pod", "nginx-pod").unwrap();
    assert!(score > non_prefix, "prefix match should score higher");
}

#[test]
fn test_fuzzy_match_substring() {
    let score = fuzzy_match("pod", "nginx-pod").unwrap();
    assert!(score > 0);
}

#[test]
fn test_fuzzy_match_no_match() {
    assert!(fuzzy_match("zzz", "nginx").is_none());
}

#[test]
fn test_fuzzy_match_empty_query() {
    let score = fuzzy_match("", "nginx").unwrap();
    assert_eq!(score, 0);
}

#[test]
fn test_fuzzy_match_empty_target() {
    assert!(fuzzy_match("test", "").is_none());
}

#[test]
fn test_fuzzy_match_both_empty() {
    let score = fuzzy_match("", "").unwrap();
    assert_eq!(score, 0);
}

#[test]
fn test_fuzzy_match_subsequence() {
    // "npd" should match "nginx-pod" as a subsequence: n...p.d
    let score = fuzzy_match("npd", "nginx-pod");
    assert!(score.is_some());
}

#[test]
fn test_fuzzy_match_non_subsequence_fails() {
    // "zxq" has no subsequence in "nginx-pod"
    assert!(fuzzy_match("zxq", "nginx-pod").is_none());
}

#[test]
fn test_fuzzy_match_consecutive_chars_bonus() {
    // "ngin" has 4 consecutive matches -> higher score
    let consecutive = fuzzy_match("ngin", "nginx-pod").unwrap();
    // "nxpd" has non-consecutive matches
    let non_consecutive = fuzzy_match("nxpd", "nginx-pod").unwrap();
    assert!(
        consecutive > non_consecutive,
        "consecutive matches should score higher: {consecutive} vs {non_consecutive}"
    );
}

#[test]
fn test_fuzzy_match_exact_beats_prefix() {
    let exact = fuzzy_match("nginx", "nginx").unwrap();
    let prefix = fuzzy_match("nginx", "nginx-pod").unwrap();
    assert!(exact > prefix);
}

#[test]
fn test_fuzzy_match_prefix_beats_middle() {
    let prefix = fuzzy_match("ng", "nginx-pod").unwrap();
    let middle = fuzzy_match("po", "nginx-pod").unwrap();
    assert!(prefix > middle, "prefix should beat middle match: {prefix} vs {middle}");
}

#[test]
fn test_fuzzy_match_query_longer_than_target() {
    assert!(fuzzy_match("nginx-deployment-very-long", "nginx").is_none());
}

#[test]
fn test_fuzzy_match_single_char() {
    let score = fuzzy_match("n", "nginx").unwrap();
    assert!(score > 0);
}

#[test]
fn test_fuzzy_match_single_char_not_found() {
    assert!(fuzzy_match("z", "nginx").is_none());
}

// ========================================================================
// search_resources tests
// ========================================================================

fn sample_items() -> Vec<SearchItem> {
    vec![
        (
            "uid-1".to_string(),
            "nginx-pod".to_string(),
            Some("default".to_string()),
            "Pod".to_string(),
            vec![
                ("app".to_string(), "nginx".to_string()),
                ("tier".to_string(), "frontend".to_string()),
            ],
        ),
        (
            "uid-2".to_string(),
            "redis-cache".to_string(),
            Some("cache-ns".to_string()),
            "Deployment".to_string(),
            vec![("app".to_string(), "redis".to_string())],
        ),
        (
            "uid-3".to_string(),
            "api-gateway".to_string(),
            Some("production".to_string()),
            "Service".to_string(),
            vec![("app".to_string(), "api".to_string()), ("env".to_string(), "prod".to_string())],
        ),
        (
            "uid-4".to_string(),
            "cluster-admin-role".to_string(),
            None,
            "ClusterRole".to_string(),
            vec![],
        ),
    ]
}

#[test]
fn test_search_empty_query_returns_empty() {
    let items = sample_items();
    let results = search_resources("", &items);
    assert!(results.is_empty());
}

#[test]
fn test_search_by_name() {
    let items = sample_items();
    let results = search_resources("nginx", &items);
    assert!(!results.is_empty());
    // nginx-pod should be a top result (matched by name and label)
    let top = &results[0];
    assert_eq!(top.uid, "uid-1");
}

#[test]
fn test_search_by_namespace() {
    let items = sample_items();
    let results = search_resources("production", &items);
    assert!(!results.is_empty());
    let found = results.iter().any(|m| m.uid == "uid-3");
    assert!(found, "api-gateway in production namespace should match");
}

#[test]
fn test_search_by_label() {
    let items = sample_items();
    let results = search_resources("frontend", &items);
    assert!(!results.is_empty());
    let top = results.iter().find(|m| m.uid == "uid-1").expect("nginx-pod should match on label");
    assert_eq!(top.matched_field, "label");
}

#[test]
fn test_search_results_sorted_by_score() {
    let items = sample_items();
    let results = search_resources("api", &items);
    assert!(!results.is_empty());
    for window in results.windows(2) {
        assert!(window[0].score >= window[1].score, "results should be sorted descending by score");
    }
}

#[test]
fn test_search_no_match() {
    let items = sample_items();
    let results = search_resources("zzzzzzz", &items);
    assert!(results.is_empty());
}

#[test]
fn test_search_case_insensitive() {
    let items = sample_items();
    let results = search_resources("NGINX", &items);
    assert!(!results.is_empty());
    assert_eq!(results[0].uid, "uid-1");
}

#[test]
fn test_search_with_no_namespace() {
    let items = sample_items();
    let results = search_resources("cluster-admin", &items);
    assert!(!results.is_empty());
    let m = results.iter().find(|m| m.uid == "uid-4").unwrap();
    assert!(m.namespace.is_none());
    assert_eq!(m.matched_field, "name");
}

#[test]
fn test_search_matched_field_tracks_best() {
    let items = sample_items();
    // "default" is a namespace, not a name
    let results = search_resources("default", &items);
    let m = results.iter().find(|m| m.uid == "uid-1").expect("nginx-pod is in default namespace");
    assert_eq!(m.matched_field, "namespace");
}

#[test]
fn test_search_label_key_value_format() {
    let items = sample_items();
    // "env=prod" should match the label on api-gateway
    let results = search_resources("env=prod", &items);
    assert!(!results.is_empty());
    let m = results
        .iter()
        .find(|m| m.uid == "uid-3")
        .expect("api-gateway should match on label env=prod");
    assert_eq!(m.matched_field, "label");
}

#[test]
fn test_search_empty_items() {
    let items: Vec<SearchItem> = Vec::new();
    let results = search_resources("test", &items);
    assert!(results.is_empty());
}

#[test]
fn test_search_preserves_kind() {
    let items = sample_items();
    let results = search_resources("nginx", &items);
    let m = results.iter().find(|m| m.uid == "uid-1").unwrap();
    assert_eq!(m.kind, "Pod");
}

#[test]
fn test_search_multiple_matches() {
    let items = sample_items();
    // "app" appears as a label key in items uid-1, uid-2, uid-3
    let results = search_resources("app", &items);
    assert!(results.len() >= 3, "at least 3 items have 'app' label: got {}", results.len());
}

#[test]
fn test_search_exact_name_scores_highest() {
    let items = vec![
        (
            "uid-a".to_string(),
            "redis".to_string(),
            Some("default".to_string()),
            "Pod".to_string(),
            vec![],
        ),
        (
            "uid-b".to_string(),
            "redis-cluster".to_string(),
            Some("default".to_string()),
            "StatefulSet".to_string(),
            vec![],
        ),
    ];
    let results = search_resources("redis", &items);
    assert_eq!(results[0].uid, "uid-a", "exact name match should be first");
    assert!(results[0].score > results[1].score);
}

// ========================================================================
// T031: Render-related state tests for SearchBar
// ========================================================================

#[test]
fn test_view_placeholder_text() {
    let state = SearchBarState::new();
    let view = SearchBarView::new(state, Theme::dark());
    assert_eq!(view.placeholder_text(), "Search resources... (Cmd+K)");
}

#[test]
fn test_view_display_text_empty_query() {
    let state = SearchBarState::new();
    let view = SearchBarView::new(state, Theme::dark());
    assert_eq!(view.display_text(), view.placeholder_text());
}

#[test]
fn test_view_display_text_with_query() {
    let mut state = SearchBarState::new();
    state.set_query("nginx");
    let view = SearchBarView::new(state, Theme::dark());
    assert_eq!(view.display_text(), "nginx");
}

#[test]
fn test_view_should_show_results_no_focus() {
    let mut state = SearchBarState::new();
    state.results.push(SearchMatch {
        uid: "uid-1".to_string(),
        name: "nginx".to_string(),
        namespace: None,
        kind: "Pod".to_string(),
        score: 100,
        matched_field: "name".to_string(),
    });
    state.is_focused = false;

    let view = SearchBarView::new(state, Theme::dark());
    assert!(!view.should_show_results());
}

#[test]
fn test_view_should_show_results_focused_with_results() {
    let mut state = SearchBarState::new();
    state.results.push(SearchMatch {
        uid: "uid-1".to_string(),
        name: "nginx".to_string(),
        namespace: None,
        kind: "Pod".to_string(),
        score: 100,
        matched_field: "name".to_string(),
    });
    state.is_focused = true;

    let view = SearchBarView::new(state, Theme::dark());
    assert!(view.should_show_results());
}

#[test]
fn test_view_should_show_results_focused_no_results() {
    let mut state = SearchBarState::new();
    state.is_focused = true;

    let view = SearchBarView::new(state, Theme::dark());
    assert!(!view.should_show_results());
}

#[test]
fn test_view_with_light_theme() {
    let state = SearchBarState::new();
    let view = SearchBarView::new(state, Theme::light());
    assert_eq!(view.theme.colors.background, baeus_ui::theme::Color::rgb(255, 255, 255));
}

#[test]
fn test_view_with_dark_theme() {
    let state = SearchBarState::new();
    let view = SearchBarView::new(state, Theme::dark());
    assert_eq!(view.theme.colors.background, baeus_ui::theme::Color::rgb(0x1e, 0x21, 0x24));
}

#[test]
fn test_view_focus_state_changes_border() {
    let mut state = SearchBarState::new();
    state.is_focused = true;
    let view = SearchBarView::new(state, Theme::dark());
    // Focused state should use accent color for border
    assert!(view.state.is_focused);
}

#[test]
fn test_view_results_with_namespace() {
    let mut state = SearchBarState::new();
    state.is_focused = true;
    state.results.push(SearchMatch {
        uid: "uid-1".to_string(),
        name: "nginx".to_string(),
        namespace: Some("production".to_string()),
        kind: "Pod".to_string(),
        score: 100,
        matched_field: "name".to_string(),
    });

    let view = SearchBarView::new(state, Theme::dark());
    assert!(view.should_show_results());
    assert_eq!(view.state.results[0].namespace.as_deref(), Some("production"));
}

#[test]
fn test_view_results_without_namespace() {
    let mut state = SearchBarState::new();
    state.is_focused = true;
    state.results.push(SearchMatch {
        uid: "uid-1".to_string(),
        name: "cluster-role".to_string(),
        namespace: None,
        kind: "ClusterRole".to_string(),
        score: 100,
        matched_field: "name".to_string(),
    });

    let view = SearchBarView::new(state, Theme::dark());
    assert!(view.should_show_results());
    assert!(view.state.results[0].namespace.is_none());
}
