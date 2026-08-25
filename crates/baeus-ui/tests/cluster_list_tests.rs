// Tests extracted from crates/baeus-ui/src/views/cluster_list.rs

use baeus_ui::views::cluster_list::*;

fn sample_items() -> Vec<ClusterListItem> {
    vec![
        {
            let mut item = ClusterListItem::new(
                "prod-us-east",
                "Production US East",
                "https://k8s.prod-east.example.com:6443",
                "oidc",
            );
            item.connected = true;
            item.connection_state = ClusterConnectionState::Connected;
            item
        },
        ClusterListItem::new(
            "staging-eu",
            "Staging EU",
            "https://k8s.staging-eu.example.com:6443",
            "token",
        ),
        {
            let mut item = ClusterListItem::new(
                "dev-local",
                "Development Local",
                "https://127.0.0.1:6443",
                "certificate",
            );
            item.connected = true;
            item.connection_state = ClusterConnectionState::Connected;
            item.favorite = true;
            item
        },
    ]
}

#[test]
fn test_cluster_list_item_new() {
    let item =
        ClusterListItem::new("my-context", "My Cluster", "https://api.example.com:6443", "oidc");
    assert_eq!(item.context_name, "my-context");
    assert_eq!(item.display_name, "My Cluster");
    assert_eq!(item.api_server_url, "https://api.example.com:6443");
    assert_eq!(item.auth_method, "oidc");
    assert!(!item.connected);
    assert!(!item.favorite);
}

#[test]
fn test_default_state() {
    let state = ClusterListState::default();
    assert!(state.items.is_empty());
    assert!(state.selected_index.is_none());
    assert!(state.filter_text.is_empty());
}

#[test]
fn test_new_state() {
    let items = sample_items();
    let state = ClusterListState::new(items.clone());
    assert_eq!(state.items.len(), 3);
    assert!(state.selected_index.is_none());
    assert!(state.filter_text.is_empty());
}

#[test]
fn test_filtered_items_no_filter() {
    let state = ClusterListState::new(sample_items());
    let filtered = state.filtered_items();
    assert_eq!(filtered.len(), 3);
}

#[test]
fn test_filtered_items_by_display_name() {
    let mut state = ClusterListState::new(sample_items());
    state.filter_text = "staging".to_string();
    let filtered = state.filtered_items();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].context_name, "staging-eu");
}

#[test]
fn test_filtered_items_by_context_name() {
    let mut state = ClusterListState::new(sample_items());
    state.filter_text = "prod-us".to_string();
    let filtered = state.filtered_items();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].display_name, "Production US East");
}

#[test]
fn test_filtered_items_case_insensitive() {
    let mut state = ClusterListState::new(sample_items());
    state.filter_text = "PRODUCTION".to_string();
    let filtered = state.filtered_items();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].context_name, "prod-us-east");
}

#[test]
fn test_filtered_items_no_match() {
    let mut state = ClusterListState::new(sample_items());
    state.filter_text = "nonexistent".to_string();
    let filtered = state.filtered_items();
    assert!(filtered.is_empty());
}

#[test]
fn test_filtered_items_partial_match_multiple() {
    let mut state = ClusterListState::new(sample_items());
    // "e" appears in all display names
    state.filter_text = "e".to_string();
    let filtered = state.filtered_items();
    assert_eq!(filtered.len(), 3);
}

#[test]
fn test_toggle_favorite() {
    let mut state = ClusterListState::new(sample_items());
    assert!(!state.items[0].favorite);

    state.toggle_favorite(0);
    assert!(state.items[0].favorite);

    state.toggle_favorite(0);
    assert!(!state.items[0].favorite);
}

#[test]
fn test_toggle_favorite_out_of_bounds() {
    let mut state = ClusterListState::new(sample_items());
    // Should not panic
    state.toggle_favorite(100);
    assert_eq!(state.items.len(), 3);
}

#[test]
fn test_select() {
    let mut state = ClusterListState::new(sample_items());
    assert!(state.selected_index.is_none());

    state.select(1);
    assert_eq!(state.selected_index, Some(1));

    state.select(0);
    assert_eq!(state.selected_index, Some(0));
}

#[test]
fn test_select_out_of_bounds() {
    let mut state = ClusterListState::new(sample_items());
    state.select(1);
    assert_eq!(state.selected_index, Some(1));

    state.select(100);
    assert!(state.selected_index.is_none());
}

#[test]
fn test_selected_item() {
    let mut state = ClusterListState::new(sample_items());
    assert!(state.selected_item().is_none());

    state.select(2);
    let selected = state.selected_item().unwrap();
    assert_eq!(selected.context_name, "dev-local");
    assert_eq!(selected.display_name, "Development Local");
}

#[test]
fn test_selected_item_none_when_no_selection() {
    let state = ClusterListState::new(sample_items());
    assert!(state.selected_item().is_none());
}

// --- T047: Cluster list view enhancements ---

#[test]
fn test_connection_state_labels() {
    assert_eq!(ClusterConnectionState::Disconnected.label(), "Disconnected");
    assert_eq!(ClusterConnectionState::Connecting.label(), "Connecting...");
    assert_eq!(ClusterConnectionState::Connected.label(), "Connected");
    assert_eq!(ClusterConnectionState::Reconnecting.label(), "Reconnecting...");
    assert_eq!(ClusterConnectionState::Error.label(), "Error");
}

#[test]
fn test_action_labels() {
    let mut item = ClusterListItem::new("ctx", "name", "url", "token");
    assert_eq!(item.action_label(), "Connect");

    item.connection_state = ClusterConnectionState::Connected;
    assert_eq!(item.action_label(), "Disconnect");

    item.connection_state = ClusterConnectionState::Error;
    assert_eq!(item.action_label(), "Retry");
}

#[test]
fn test_action_enabled() {
    let mut item = ClusterListItem::new("ctx", "name", "url", "token");
    assert!(item.is_action_enabled()); // disconnected

    item.connection_state = ClusterConnectionState::Connecting;
    assert!(!item.is_action_enabled()); // in progress

    item.connection_state = ClusterConnectionState::Connected;
    assert!(item.is_action_enabled()); // can disconnect

    item.connection_state = ClusterConnectionState::Error;
    assert!(item.is_action_enabled()); // can retry
}

#[test]
fn test_set_connection_state() {
    let mut state = ClusterListState::new(sample_items());
    state.set_connection_state("staging-eu", ClusterConnectionState::Connected);

    let item = state.items.iter().find(|i| i.context_name == "staging-eu").unwrap();
    assert!(item.connected);
    assert_eq!(item.connection_state, ClusterConnectionState::Connected);
}

#[test]
fn test_set_error() {
    let mut state = ClusterListState::new(sample_items());
    state.set_error("staging-eu", "connection refused".to_string());

    let item = state.items.iter().find(|i| i.context_name == "staging-eu").unwrap();
    assert!(!item.connected);
    assert_eq!(item.connection_state, ClusterConnectionState::Error);
    assert_eq!(item.error_message.as_deref(), Some("connection refused"));
}

#[test]
fn test_sort_favorites_first() {
    let mut state = ClusterListState::new(sample_items());
    // dev-local is already favorite
    state.sort_favorites_first();
    assert_eq!(state.items[0].context_name, "dev-local");
}

#[test]
fn test_connected_count() {
    let mut state = ClusterListState::new(sample_items());
    // sample_items has prod-us-east and dev-local as connected
    assert_eq!(state.connected_count(), 2);

    state.set_connection_state("staging-eu", ClusterConnectionState::Connected);
    assert_eq!(state.connected_count(), 3);
}

#[test]
fn test_cluster_list_item_serialization() {
    let item = ClusterListItem::new("test-ctx", "Test Cluster", "https://localhost:6443", "token");
    let json = serde_json::to_string(&item).unwrap();
    let deserialized: ClusterListItem = serde_json::from_str(&json).unwrap();
    assert_eq!(item, deserialized);
}

// --- T020: Render tests for ClusterListView ---

/// Build a realistic multi-cluster list that exercises every connection state,
/// simulating the data that would feed into rendered cluster cards.
fn render_scenario_items() -> Vec<ClusterListItem> {
    vec![
        {
            let mut item = ClusterListItem::new(
                "prod-us-east",
                "Production US East",
                "https://k8s.prod-east.example.com:6443",
                "oidc",
            );
            item.connected = true;
            item.connection_state = ClusterConnectionState::Connected;
            item.favorite = true;
            item
        },
        ClusterListItem::new(
            "staging-eu",
            "Staging EU",
            "https://k8s.staging-eu.example.com:6443",
            "token",
        ),
        {
            let mut item = ClusterListItem::new(
                "dev-local",
                "Development Local",
                "https://127.0.0.1:6443",
                "certificate",
            );
            item.connection_state = ClusterConnectionState::Connecting;
            item
        },
        {
            let mut item = ClusterListItem::new(
                "qa-cluster",
                "QA Cluster",
                "https://qa.example.com:6443",
                "token",
            );
            item.connection_state = ClusterConnectionState::Error;
            item.error_message = Some("connection refused".to_string());
            item
        },
        {
            let mut item = ClusterListItem::new(
                "canary-west",
                "Canary US West",
                "https://canary.west.example.com:6443",
                "oidc",
            );
            item.connection_state = ClusterConnectionState::Reconnecting;
            item
        },
    ]
}

#[test]
fn test_render_cluster_cards_contain_name_and_status() {
    let state = ClusterListState::new(render_scenario_items());

    // Each cluster card should expose a display_name and a connection state label
    let expected: Vec<(&str, &str)> = vec![
        ("Production US East", "Connected"),
        ("Staging EU", "Disconnected"),
        ("Development Local", "Connecting..."),
        ("QA Cluster", "Error"),
        ("Canary US West", "Reconnecting..."),
    ];

    for (item, (name, status)) in state.items.iter().zip(expected.iter()) {
        assert_eq!(&item.display_name, name);
        assert_eq!(item.connection_state.label(), *status);
    }
}

#[test]
fn test_render_cluster_cards_connect_button_label_and_enabled() {
    let state = ClusterListState::new(render_scenario_items());

    let expected: Vec<(&str, bool)> = vec![
        ("Disconnect", true),       // Connected -> can disconnect
        ("Connect", true),          // Disconnected -> can connect
        ("Connecting...", false),   // Connecting -> button disabled
        ("Retry", true),            // Error -> can retry
        ("Reconnecting...", false), // Reconnecting -> button disabled
    ];

    for (item, (label, enabled)) in state.items.iter().zip(expected.iter()) {
        assert_eq!(
            item.action_label(),
            *label,
            "Wrong action label for cluster {}",
            item.display_name
        );
        assert_eq!(
            item.is_action_enabled(),
            *enabled,
            "Wrong enabled state for cluster {}",
            item.display_name
        );
    }
}

#[test]
fn test_render_cluster_cards_show_error_message_when_present() {
    let state = ClusterListState::new(render_scenario_items());

    // Only the QA Cluster (index 3) should have an error message rendered
    for (i, item) in state.items.iter().enumerate() {
        if i == 3 {
            assert_eq!(
                item.error_message.as_deref(),
                Some("connection refused"),
                "QA Cluster should show error message"
            );
        } else {
            assert!(
                item.error_message.is_none(),
                "Cluster {} should not show an error message",
                item.display_name
            );
        }
    }
}

#[test]
fn test_render_cluster_list_favorites_sorted_first() {
    let mut state = ClusterListState::new(render_scenario_items());
    state.sort_favorites_first();

    // The favorite cluster (Production US East) should render at the top
    assert_eq!(state.items[0].context_name, "prod-us-east");
    assert!(state.items[0].favorite);

    // Remaining items should be alphabetically sorted by display_name
    let non_fav_names: Vec<&str> =
        state.items[1..].iter().map(|i| i.display_name.as_str()).collect();
    let mut sorted = non_fav_names.clone();
    sorted.sort();
    assert_eq!(non_fav_names, sorted);
}

#[test]
fn test_render_filtered_cluster_list_reduces_visible_cards() {
    let mut state = ClusterListState::new(render_scenario_items());

    // Typing "prod" in the search bar should show only the production cluster
    state.filter_text = "prod".to_string();
    let visible = state.filtered_items();
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].display_name, "Production US East");

    // Typing "us" matches Production US East, QA Cluster ("us" in cluster), and Canary US West
    state.filter_text = "us".to_string();
    let visible = state.filtered_items();
    assert_eq!(visible.len(), 3);
}

#[test]
fn test_render_selected_cluster_card_highlighted() {
    let mut state = ClusterListState::new(render_scenario_items());

    // No card highlighted initially
    assert!(state.selected_item().is_none());

    // Selecting index 2 should highlight Development Local
    state.select(2);
    let selected = state.selected_item().unwrap();
    assert_eq!(selected.display_name, "Development Local");
    assert_eq!(selected.context_name, "dev-local");
}

#[test]
fn test_render_connection_state_actionability() {
    // Verify which connection states allow user interaction (clicking connect/disconnect)
    assert!(ClusterConnectionState::Disconnected.is_actionable());
    assert!(!ClusterConnectionState::Connecting.is_actionable());
    assert!(!ClusterConnectionState::Connected.is_actionable());
    assert!(!ClusterConnectionState::Reconnecting.is_actionable());
    assert!(ClusterConnectionState::Error.is_actionable());
}

#[test]
fn test_render_connected_count_badge() {
    let state = ClusterListState::new(render_scenario_items());
    // Only prod-us-east is connected
    assert_eq!(state.connected_count(), 1);
}

#[test]
fn test_render_cluster_card_all_fields_populated() {
    let item = ClusterListItem::new(
        "test-context",
        "Test Cluster",
        "https://api.test.example.com:6443",
        "oidc",
    );

    // A rendered cluster card should have access to all these fields
    assert!(!item.context_name.is_empty());
    assert!(!item.display_name.is_empty());
    assert!(!item.api_server_url.is_empty());
    assert!(!item.auth_method.is_empty());
    assert!(!item.action_label().is_empty());
    assert!(!item.connection_state.label().is_empty());
}
