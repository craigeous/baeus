// T359: Port Forward management view tests
//
// Verifies:
// - Table renders with columns (Name, Namespace, Kind, Pod Port, Local Port, Protocol, Status)
// - Open action opens the forwarded endpoint URL
// - Stop action terminates the port forward

use baeus_ui::theme::Theme;
use baeus_ui::views::port_forward::{
    PORT_FORWARD_COLUMNS, PortForwardEntry, PortForwardState, PortForwardStatus, PortForwardView,
};
use uuid::Uuid;

// ========================================================================
// Helpers
// ========================================================================

fn make_entry(
    name: &str,
    namespace: &str,
    kind: &str,
    pod_port: u16,
    local_port: u16,
) -> PortForwardEntry {
    PortForwardEntry {
        id: Uuid::new_v4(),
        name: name.to_string(),
        namespace: namespace.to_string(),
        kind: kind.to_string(),
        pod_port,
        local_port,
        protocol: "TCP".to_string(),
        status: PortForwardStatus::Active,
    }
}

fn make_entry_with_status(
    name: &str,
    namespace: &str,
    kind: &str,
    pod_port: u16,
    local_port: u16,
    status: PortForwardStatus,
) -> PortForwardEntry {
    PortForwardEntry {
        id: Uuid::new_v4(),
        name: name.to_string(),
        namespace: namespace.to_string(),
        kind: kind.to_string(),
        pod_port,
        local_port,
        protocol: "TCP".to_string(),
        status,
    }
}

fn make_udp_entry(
    name: &str,
    namespace: &str,
    kind: &str,
    pod_port: u16,
    local_port: u16,
) -> PortForwardEntry {
    PortForwardEntry {
        id: Uuid::new_v4(),
        name: name.to_string(),
        namespace: namespace.to_string(),
        kind: kind.to_string(),
        pod_port,
        local_port,
        protocol: "UDP".to_string(),
        status: PortForwardStatus::Active,
    }
}

fn make_view() -> PortForwardView {
    PortForwardView::new(PortForwardState::new(), Theme::dark())
}

fn make_view_with_entries() -> PortForwardView {
    let mut state = PortForwardState::new();
    state.add_forward(make_entry("nginx-pod", "default", "Pod", 80, 8080));
    state.add_forward(make_entry("redis-svc", "cache", "Service", 6379, 6379));
    state.add_forward(make_entry("postgres-pod", "db", "Pod", 5432, 5432));
    PortForwardView::new(state, Theme::dark())
}

// ========================================================================
// Table column verification
// ========================================================================

#[test]
fn test_table_has_seven_columns() {
    assert_eq!(PORT_FORWARD_COLUMNS.len(), 7);
}

#[test]
fn test_table_column_name() {
    assert_eq!(PORT_FORWARD_COLUMNS[0], "Name");
}

#[test]
fn test_table_column_namespace() {
    assert_eq!(PORT_FORWARD_COLUMNS[1], "Namespace");
}

#[test]
fn test_table_column_kind() {
    assert_eq!(PORT_FORWARD_COLUMNS[2], "Kind");
}

#[test]
fn test_table_column_pod_port() {
    assert_eq!(PORT_FORWARD_COLUMNS[3], "Pod Port");
}

#[test]
fn test_table_column_local_port() {
    assert_eq!(PORT_FORWARD_COLUMNS[4], "Local Port");
}

#[test]
fn test_table_column_protocol() {
    assert_eq!(PORT_FORWARD_COLUMNS[5], "Protocol");
}

#[test]
fn test_table_column_status() {
    assert_eq!(PORT_FORWARD_COLUMNS[6], "Status");
}

#[test]
fn test_all_columns_present() {
    let expected = ["Name", "Namespace", "Kind", "Pod Port", "Local Port", "Protocol", "Status"];
    assert_eq!(PORT_FORWARD_COLUMNS, &expected);
}

// ========================================================================
// PortForwardStatus tests
// ========================================================================

#[test]
fn test_status_active_label() {
    assert_eq!(PortForwardStatus::Active.label(), "Active");
}

#[test]
fn test_status_stopped_label() {
    assert_eq!(PortForwardStatus::Stopped.label(), "Stopped");
}

#[test]
fn test_status_error_label() {
    assert_eq!(PortForwardStatus::Error.label(), "Error");
}

// ========================================================================
// PortForwardEntry tests
// ========================================================================

#[test]
fn test_entry_fields() {
    let entry = make_entry("nginx-pod", "default", "Pod", 80, 8080);
    assert_eq!(entry.name, "nginx-pod");
    assert_eq!(entry.namespace, "default");
    assert_eq!(entry.kind, "Pod");
    assert_eq!(entry.pod_port, 80);
    assert_eq!(entry.local_port, 8080);
    assert_eq!(entry.protocol, "TCP");
    assert_eq!(entry.status, PortForwardStatus::Active);
}

#[test]
fn test_entry_endpoint_url() {
    let entry = make_entry("nginx-pod", "default", "Pod", 80, 8080);
    assert_eq!(entry.endpoint_url(), "http://localhost:8080");
}

#[test]
fn test_entry_endpoint_url_different_port() {
    let entry = make_entry("redis", "default", "Service", 6379, 16379);
    assert_eq!(entry.endpoint_url(), "http://localhost:16379");
}

#[test]
fn test_entry_port_display() {
    let entry = make_entry("nginx-pod", "default", "Pod", 80, 8080);
    assert_eq!(entry.port_display(), "8080 -> 80");
}

#[test]
fn test_entry_is_active() {
    let entry = make_entry("nginx", "default", "Pod", 80, 8080);
    assert!(entry.is_active());
    assert!(!entry.is_stopped());
    assert!(!entry.is_error());
}

#[test]
fn test_entry_is_stopped() {
    let entry =
        make_entry_with_status("nginx", "default", "Pod", 80, 8080, PortForwardStatus::Stopped);
    assert!(!entry.is_active());
    assert!(entry.is_stopped());
    assert!(!entry.is_error());
}

#[test]
fn test_entry_is_error() {
    let entry =
        make_entry_with_status("nginx", "default", "Pod", 80, 8080, PortForwardStatus::Error);
    assert!(!entry.is_active());
    assert!(!entry.is_stopped());
    assert!(entry.is_error());
}

#[test]
fn test_entry_kind_service() {
    let entry = make_entry("redis-svc", "cache", "Service", 6379, 6379);
    assert_eq!(entry.kind, "Service");
}

#[test]
fn test_entry_udp_protocol() {
    let entry = make_udp_entry("dns-pod", "kube-system", "Pod", 53, 5353);
    assert_eq!(entry.protocol, "UDP");
}

#[test]
fn test_entry_serialization_roundtrip() {
    let entry = make_entry("nginx", "default", "Pod", 80, 8080);
    let json = serde_json::to_string(&entry).unwrap();
    let deser: PortForwardEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.id, entry.id);
    assert_eq!(deser.name, "nginx");
    assert_eq!(deser.namespace, "default");
    assert_eq!(deser.kind, "Pod");
    assert_eq!(deser.pod_port, 80);
    assert_eq!(deser.local_port, 8080);
    assert_eq!(deser.protocol, "TCP");
    assert_eq!(deser.status, PortForwardStatus::Active);
}

#[test]
fn test_status_serialization_roundtrip() {
    for status in [PortForwardStatus::Active, PortForwardStatus::Stopped, PortForwardStatus::Error]
    {
        let json = serde_json::to_string(&status).unwrap();
        let deser: PortForwardStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deser, status);
    }
}

// ========================================================================
// PortForwardState tests
// ========================================================================

#[test]
fn test_state_new_empty() {
    let state = PortForwardState::new();
    assert_eq!(state.forward_count(), 0);
    assert!(state.active_forwards().is_empty());
}

#[test]
fn test_state_default_empty() {
    let state = PortForwardState::default();
    assert_eq!(state.forward_count(), 0);
}

#[test]
fn test_state_add_forward() {
    let mut state = PortForwardState::new();
    state.add_forward(make_entry("nginx", "default", "Pod", 80, 8080));
    assert_eq!(state.forward_count(), 1);
    assert_eq!(state.active_count(), 1);
}

#[test]
fn test_state_add_multiple_forwards() {
    let mut state = PortForwardState::new();
    state.add_forward(make_entry("nginx", "default", "Pod", 80, 8080));
    state.add_forward(make_entry("redis", "cache", "Service", 6379, 6379));
    state.add_forward(make_entry("postgres", "db", "Pod", 5432, 5432));
    assert_eq!(state.forward_count(), 3);
    assert_eq!(state.active_count(), 3);
}

#[test]
fn test_state_stop_forward() {
    let mut state = PortForwardState::new();
    let entry = make_entry("nginx", "default", "Pod", 80, 8080);
    let id = entry.id;
    state.add_forward(entry);

    assert!(state.stop_forward(id));
    let stopped = state.get_forward(id).unwrap();
    assert_eq!(stopped.status, PortForwardStatus::Stopped);
    assert_eq!(state.active_count(), 0);
}

#[test]
fn test_state_stop_forward_nonexistent() {
    let mut state = PortForwardState::new();
    assert!(!state.stop_forward(Uuid::new_v4()));
}

#[test]
fn test_state_remove_forward() {
    let mut state = PortForwardState::new();
    let entry = make_entry("nginx", "default", "Pod", 80, 8080);
    let id = entry.id;
    state.add_forward(entry);

    assert!(state.remove_forward(id));
    assert_eq!(state.forward_count(), 0);
    assert!(state.get_forward(id).is_none());
}

#[test]
fn test_state_remove_forward_nonexistent() {
    let mut state = PortForwardState::new();
    assert!(!state.remove_forward(Uuid::new_v4()));
}

#[test]
fn test_state_remove_forward_middle() {
    let mut state = PortForwardState::new();
    let e1 = make_entry("nginx", "default", "Pod", 80, 8080);
    let e2 = make_entry("redis", "default", "Pod", 6379, 6379);
    let e3 = make_entry("postgres", "default", "Pod", 5432, 5432);
    let id2 = e2.id;
    state.add_forward(e1);
    state.add_forward(e2);
    state.add_forward(e3);

    state.remove_forward(id2);
    assert_eq!(state.forward_count(), 2);
    assert!(state.get_forward(id2).is_none());
}

#[test]
fn test_state_active_forwards() {
    let mut state = PortForwardState::new();
    let e1 = make_entry("nginx", "default", "Pod", 80, 8080);
    let e2 = make_entry("redis", "default", "Pod", 6379, 6379);
    let id1 = e1.id;
    state.add_forward(e1);
    state.add_forward(e2);

    assert_eq!(state.active_forwards().len(), 2);

    state.stop_forward(id1);
    assert_eq!(state.active_forwards().len(), 1);
    assert_eq!(state.active_forwards()[0].name, "redis");
}

#[test]
fn test_state_get_forward() {
    let mut state = PortForwardState::new();
    let entry = make_entry("nginx", "default", "Pod", 80, 8080);
    let id = entry.id;
    state.add_forward(entry);

    let found = state.get_forward(id).unwrap();
    assert_eq!(found.name, "nginx");
    assert_eq!(found.namespace, "default");
}

#[test]
fn test_state_get_forward_nonexistent() {
    let state = PortForwardState::new();
    assert!(state.get_forward(Uuid::new_v4()).is_none());
}

#[test]
fn test_state_is_local_port_in_use() {
    let mut state = PortForwardState::new();
    state.add_forward(make_entry("nginx", "default", "Pod", 80, 8080));
    assert!(state.is_local_port_in_use(8080));
    assert!(!state.is_local_port_in_use(9090));
}

#[test]
fn test_state_port_not_in_use_after_stop() {
    let mut state = PortForwardState::new();
    let entry = make_entry("nginx", "default", "Pod", 80, 8080);
    let id = entry.id;
    state.add_forward(entry);
    state.stop_forward(id);
    assert!(!state.is_local_port_in_use(8080));
}

// ========================================================================
// PortForwardView tests -- Open action
// ========================================================================

#[test]
fn test_view_creation() {
    let view = make_view();
    assert!(view.state.forwards.is_empty());
    assert!(view.last_opened_url.is_none());
    assert!(view.last_stopped_id.is_none());
}

#[test]
fn test_view_with_entries() {
    let view = make_view_with_entries();
    assert_eq!(view.state.forward_count(), 3);
    assert_eq!(view.state.active_count(), 3);
}

#[test]
fn test_open_action_sets_endpoint_url() {
    let mut view = make_view_with_entries();
    let id = view.state.forwards[0].id;
    view.open_forward(id);
    assert_eq!(view.last_opened_url.as_deref(), Some("http://localhost:8080"));
}

#[test]
fn test_open_action_different_entry() {
    let mut view = make_view_with_entries();
    let id = view.state.forwards[1].id; // redis on port 6379
    view.open_forward(id);
    assert_eq!(view.last_opened_url.as_deref(), Some("http://localhost:6379"));
}

#[test]
fn test_open_action_nonexistent_id() {
    let mut view = make_view_with_entries();
    view.open_forward(Uuid::new_v4());
    assert!(view.last_opened_url.is_none());
}

#[test]
fn test_open_action_updates_url_on_second_call() {
    let mut view = make_view_with_entries();
    let id1 = view.state.forwards[0].id;
    let id2 = view.state.forwards[1].id;

    view.open_forward(id1);
    assert_eq!(view.last_opened_url.as_deref(), Some("http://localhost:8080"));

    view.open_forward(id2);
    assert_eq!(view.last_opened_url.as_deref(), Some("http://localhost:6379"));
}

// ========================================================================
// PortForwardView tests -- Stop action
// ========================================================================

#[test]
fn test_stop_action_terminates_forward() {
    let mut view = make_view_with_entries();
    let id = view.state.forwards[0].id;

    assert!(view.stop_forward(id));
    assert_eq!(view.last_stopped_id, Some(id));
    assert_eq!(view.state.get_forward(id).unwrap().status, PortForwardStatus::Stopped);
}

#[test]
fn test_stop_action_reduces_active_count() {
    let mut view = make_view_with_entries();
    let id = view.state.forwards[0].id;

    assert_eq!(view.state.active_count(), 3);
    view.stop_forward(id);
    assert_eq!(view.state.active_count(), 2);
}

#[test]
fn test_stop_action_nonexistent_id() {
    let mut view = make_view_with_entries();
    assert!(!view.stop_forward(Uuid::new_v4()));
    assert!(view.last_stopped_id.is_none());
}

#[test]
fn test_stop_all_forwards() {
    let mut view = make_view_with_entries();
    let ids: Vec<Uuid> = view.state.forwards.iter().map(|e| e.id).collect();

    for id in &ids {
        view.stop_forward(*id);
    }

    assert_eq!(view.state.active_count(), 0);
    assert_eq!(view.state.forward_count(), 3); // still present, just stopped
}

#[test]
fn test_stop_then_open_still_returns_url() {
    let mut view = make_view_with_entries();
    let id = view.state.forwards[0].id;

    view.stop_forward(id);
    // Even though stopped, open should still set the URL since the entry exists
    view.open_forward(id);
    assert_eq!(view.last_opened_url.as_deref(), Some("http://localhost:8080"));
}

// ========================================================================
// Full workflow
// ========================================================================

#[test]
fn test_full_workflow() {
    let mut state = PortForwardState::new();

    // Add forwards
    let e1 = make_entry("nginx-pod", "default", "Pod", 80, 8080);
    let e2 = make_entry("redis-svc", "cache", "Service", 6379, 6379);
    let e3 = make_entry("pg-pod", "db", "Pod", 5432, 5432);
    let id1 = e1.id;
    let id2 = e2.id;
    let id3 = e3.id;
    state.add_forward(e1);
    state.add_forward(e2);
    state.add_forward(e3);

    let mut view = PortForwardView::new(state, Theme::dark());
    assert_eq!(view.state.forward_count(), 3);
    assert_eq!(view.state.active_count(), 3);

    // Open first forward
    view.open_forward(id1);
    assert_eq!(view.last_opened_url.as_deref(), Some("http://localhost:8080"));

    // Stop second forward
    view.stop_forward(id2);
    assert_eq!(view.state.active_count(), 2);
    assert_eq!(view.state.get_forward(id2).unwrap().status, PortForwardStatus::Stopped);

    // Remove third forward
    view.state.remove_forward(id3);
    assert_eq!(view.state.forward_count(), 2);

    // Verify remaining state
    assert!(view.state.get_forward(id1).unwrap().is_active());
    assert!(view.state.get_forward(id2).unwrap().is_stopped());
    assert!(view.state.get_forward(id3).is_none());
}
