// T041: UI tests for cluster list view rendering contexts from kubeconfig

use baeus_core::cluster::{AuthMethod, ClusterConnection, ClusterManager};
use baeus_ui::views::cluster_list::{ClusterListItem, ClusterListState};

/// Helper to create a ClusterListItem from a ClusterConnection.
fn item_from_connection(conn: &ClusterConnection) -> ClusterListItem {
    let mut item = ClusterListItem::new(
        &conn.context_name,
        &conn.name,
        &conn.api_server_url,
        format!("{:?}", conn.auth_method).to_lowercase(),
    );
    item.connected = conn.is_connected();
    item.favorite = conn.favorite;
    item
}

fn sample_connections() -> Vec<ClusterConnection> {
    vec![
        ClusterConnection::new(
            "Production US".to_string(),
            "prod-us".to_string(),
            "https://prod.example.com:6443".to_string(),
            AuthMethod::OIDC,
        ),
        ClusterConnection::new(
            "Staging EU".to_string(),
            "staging-eu".to_string(),
            "https://staging.example.com:6443".to_string(),
            AuthMethod::Token,
        ),
        ClusterConnection::new(
            "Dev Local".to_string(),
            "dev-local".to_string(),
            "https://127.0.0.1:6443".to_string(),
            AuthMethod::Certificate,
        ),
    ]
}

#[test]
fn test_cluster_list_from_kubeconfig_contexts() {
    let connections = sample_connections();
    let items: Vec<ClusterListItem> = connections.iter().map(item_from_connection).collect();
    let state = ClusterListState::new(items);

    assert_eq!(state.items.len(), 3);
    assert_eq!(state.items[0].context_name, "prod-us");
    assert_eq!(state.items[0].display_name, "Production US");
    assert!(!state.items[0].connected);
}

#[test]
fn test_cluster_list_reflects_connection_status() {
    let mut connections = sample_connections();
    connections[0].set_connected();
    connections[2].set_connected();

    let items: Vec<ClusterListItem> = connections.iter().map(item_from_connection).collect();
    let state = ClusterListState::new(items);

    assert!(state.items[0].connected);
    assert!(!state.items[1].connected);
    assert!(state.items[2].connected);
}

#[test]
fn test_cluster_list_reflects_favorites() {
    let mut connections = sample_connections();
    connections[1].favorite = true;

    let items: Vec<ClusterListItem> = connections.iter().map(item_from_connection).collect();
    let state = ClusterListState::new(items);

    assert!(!state.items[0].favorite);
    assert!(state.items[1].favorite);
    assert!(!state.items[2].favorite);
}

#[test]
fn test_cluster_list_filter_matches_display_name() {
    let connections = sample_connections();
    let items: Vec<ClusterListItem> = connections.iter().map(item_from_connection).collect();
    let mut state = ClusterListState::new(items);

    state.filter_text = "Production".to_string();
    let filtered = state.filtered_items();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].display_name, "Production US");
}

#[test]
fn test_cluster_list_filter_matches_context_name() {
    let connections = sample_connections();
    let items: Vec<ClusterListItem> = connections.iter().map(item_from_connection).collect();
    let mut state = ClusterListState::new(items);

    state.filter_text = "dev-local".to_string();
    let filtered = state.filtered_items();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].context_name, "dev-local");
}

#[test]
fn test_cluster_list_select_and_get_details() {
    let connections = sample_connections();
    let items: Vec<ClusterListItem> = connections.iter().map(item_from_connection).collect();
    let mut state = ClusterListState::new(items);

    state.select(1);
    let selected = state.selected_item().unwrap();
    assert_eq!(selected.context_name, "staging-eu");
    assert_eq!(selected.display_name, "Staging EU");
}

#[test]
fn test_cluster_list_sync_with_manager() {
    let mut mgr = ClusterManager::new();
    let connections = sample_connections();
    for conn in connections {
        mgr.add_connection(conn);
    }

    let items: Vec<ClusterListItem> =
        mgr.list_connections().iter().map(|c| item_from_connection(c)).collect();
    let state = ClusterListState::new(items);

    assert_eq!(state.items.len(), 3);
}

#[test]
fn test_cluster_list_empty_when_no_contexts() {
    let state = ClusterListState::new(Vec::new());
    assert!(state.items.is_empty());
    assert!(state.filtered_items().is_empty());
    assert!(state.selected_item().is_none());
}

#[test]
fn test_cluster_list_connection_status_variants() {
    let mut conn = ClusterConnection::new(
        "test".to_string(),
        "test-ctx".to_string(),
        "https://localhost:6443".to_string(),
        AuthMethod::Token,
    );

    // Disconnected
    let item = item_from_connection(&conn);
    assert!(!item.connected);

    // Connecting (not yet connected)
    conn.set_connecting();
    let item = item_from_connection(&conn);
    assert!(!item.connected);

    // Connected
    conn.set_connected();
    let item = item_from_connection(&conn);
    assert!(item.connected);

    // Reconnecting (still counts as not connected for the list)
    conn.set_reconnecting();
    let item = item_from_connection(&conn);
    assert!(!item.connected);

    // Error
    conn.set_error("timeout".to_string());
    let item = item_from_connection(&conn);
    assert!(!item.connected);
}
