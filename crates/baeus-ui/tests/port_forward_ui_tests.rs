// T091: Port-Forward Management UI tests

use baeus_ui::components::port_forward::*;

// ========================================================================
// Helper
// ========================================================================

fn make_entry(id: &str, pod: &str, local: u16, remote: u16) -> PortForwardEntry {
    PortForwardEntry {
        id: id.to_string(),
        pod_name: pod.to_string(),
        namespace: "default".to_string(),
        local_port: local,
        remote_port: remote,
        state: PortForwardDisplayState::Active,
        error_message: None,
    }
}

fn make_entry_with_state(
    id: &str,
    pod: &str,
    local: u16,
    remote: u16,
    state: PortForwardDisplayState,
) -> PortForwardEntry {
    PortForwardEntry {
        id: id.to_string(),
        pod_name: pod.to_string(),
        namespace: "default".to_string(),
        local_port: local,
        remote_port: remote,
        state,
        error_message: None,
    }
}

// ========================================================================
// Construction
// ========================================================================

#[test]
fn test_panel_state_new() {
    let state = PortForwardPanelState::new();
    assert!(state.entries.is_empty());
    assert!(!state.show_create_dialog);
    assert!(state.new_local_port.is_empty());
    assert!(state.new_remote_port.is_empty());
}

#[test]
fn test_panel_state_default() {
    let state = PortForwardPanelState::default();
    assert!(state.entries.is_empty());
}

// ========================================================================
// Entry management (add/remove/stop)
// ========================================================================

#[test]
fn test_add_entry() {
    let mut state = PortForwardPanelState::new();
    state.add_entry(make_entry("pf-1", "nginx", 8080, 80));
    assert_eq!(state.entry_count(), 1);
    assert_eq!(state.entries[0].id, "pf-1");
}

#[test]
fn test_add_multiple_entries() {
    let mut state = PortForwardPanelState::new();
    state.add_entry(make_entry("pf-1", "nginx", 8080, 80));
    state.add_entry(make_entry("pf-2", "redis", 6379, 6379));
    state.add_entry(make_entry("pf-3", "postgres", 5432, 5432));
    assert_eq!(state.entry_count(), 3);
}

#[test]
fn test_remove_entry() {
    let mut state = PortForwardPanelState::new();
    state.add_entry(make_entry("pf-1", "nginx", 8080, 80));
    assert!(state.remove_entry("pf-1"));
    assert_eq!(state.entry_count(), 0);
}

#[test]
fn test_remove_entry_nonexistent() {
    let mut state = PortForwardPanelState::new();
    assert!(!state.remove_entry("pf-999"));
}

#[test]
fn test_remove_entry_middle() {
    let mut state = PortForwardPanelState::new();
    state.add_entry(make_entry("pf-1", "a", 8080, 80));
    state.add_entry(make_entry("pf-2", "b", 8081, 81));
    state.add_entry(make_entry("pf-3", "c", 8082, 82));
    state.remove_entry("pf-2");
    assert_eq!(state.entry_count(), 2);
    assert_eq!(state.entries[0].id, "pf-1");
    assert_eq!(state.entries[1].id, "pf-3");
}

#[test]
fn test_stop_entry() {
    let mut state = PortForwardPanelState::new();
    state.add_entry(make_entry("pf-1", "nginx", 8080, 80));
    assert!(state.stop_entry("pf-1"));
    assert_eq!(state.entries[0].state, PortForwardDisplayState::Stopped);
}

#[test]
fn test_stop_entry_nonexistent() {
    let mut state = PortForwardPanelState::new();
    assert!(!state.stop_entry("pf-999"));
}

// ========================================================================
// Error state handling
// ========================================================================

#[test]
fn test_set_error() {
    let mut state = PortForwardPanelState::new();
    state.add_entry(make_entry("pf-1", "nginx", 8080, 80));
    assert!(state.set_error("pf-1", "connection refused".to_string()));
    assert_eq!(state.entries[0].state, PortForwardDisplayState::Error);
    assert_eq!(state.entries[0].error_message.as_deref(), Some("connection refused"));
}

#[test]
fn test_set_error_nonexistent() {
    let mut state = PortForwardPanelState::new();
    assert!(!state.set_error("pf-999", "error".to_string()));
}

#[test]
fn test_entry_is_error() {
    let mut entry = make_entry("pf-1", "nginx", 8080, 80);
    assert!(!entry.is_error());
    entry.state = PortForwardDisplayState::Error;
    assert!(entry.is_error());
}

#[test]
fn test_entry_is_active() {
    let active = make_entry("pf-1", "nginx", 8080, 80);
    assert!(active.is_active());

    let stopped =
        make_entry_with_state("pf-2", "redis", 6379, 6379, PortForwardDisplayState::Stopped);
    assert!(!stopped.is_active());
}

// ========================================================================
// Active count tracking
// ========================================================================

#[test]
fn test_active_count_empty() {
    let state = PortForwardPanelState::new();
    assert_eq!(state.active_count(), 0);
}

#[test]
fn test_active_count_all_active() {
    let mut state = PortForwardPanelState::new();
    state.add_entry(make_entry("pf-1", "a", 8080, 80));
    state.add_entry(make_entry("pf-2", "b", 8081, 81));
    assert_eq!(state.active_count(), 2);
}

#[test]
fn test_active_count_mixed_states() {
    let mut state = PortForwardPanelState::new();
    state.add_entry(make_entry("pf-1", "a", 8080, 80));
    state.add_entry(make_entry_with_state("pf-2", "b", 8081, 81, PortForwardDisplayState::Stopped));
    state.add_entry(make_entry_with_state("pf-3", "c", 8082, 82, PortForwardDisplayState::Error));
    assert_eq!(state.active_count(), 1);
}

#[test]
fn test_active_count_after_stop() {
    let mut state = PortForwardPanelState::new();
    state.add_entry(make_entry("pf-1", "a", 8080, 80));
    state.add_entry(make_entry("pf-2", "b", 8081, 81));
    state.stop_entry("pf-1");
    assert_eq!(state.active_count(), 1);
}

// ========================================================================
// Pod filtering
// ========================================================================

#[test]
fn test_entries_for_pod() {
    let mut state = PortForwardPanelState::new();
    state.add_entry(make_entry("pf-1", "nginx", 8080, 80));
    state.add_entry(make_entry("pf-2", "nginx", 8443, 443));
    state.add_entry(make_entry("pf-3", "redis", 6379, 6379));

    let nginx_entries = state.entries_for_pod("nginx");
    assert_eq!(nginx_entries.len(), 2);

    let redis_entries = state.entries_for_pod("redis");
    assert_eq!(redis_entries.len(), 1);
}

#[test]
fn test_entries_for_pod_none() {
    let state = PortForwardPanelState::new();
    assert!(state.entries_for_pod("nonexistent").is_empty());
}

#[test]
fn test_entries_for_pod_with_mixed_pods() {
    let mut state = PortForwardPanelState::new();
    state.add_entry(make_entry("pf-1", "nginx", 8080, 80));
    state.add_entry(make_entry("pf-2", "redis", 6379, 6379));
    state.add_entry(make_entry("pf-3", "postgres", 5432, 5432));

    assert_eq!(state.entries_for_pod("nginx").len(), 1);
    assert_eq!(state.entries_for_pod("redis").len(), 1);
    assert_eq!(state.entries_for_pod("postgres").len(), 1);
    assert_eq!(state.entries_for_pod("missing").len(), 0);
}

// ========================================================================
// Port conflict detection
// ========================================================================

#[test]
fn test_port_in_use_active() {
    let mut state = PortForwardPanelState::new();
    state.add_entry(make_entry("pf-1", "nginx", 8080, 80));
    assert!(state.is_port_in_use(8080));
    assert!(!state.is_port_in_use(9090));
}

#[test]
fn test_port_not_in_use_when_stopped() {
    let mut state = PortForwardPanelState::new();
    state.add_entry(make_entry("pf-1", "nginx", 8080, 80));
    state.stop_entry("pf-1");
    assert!(!state.is_port_in_use(8080));
}

#[test]
fn test_port_not_in_use_when_error() {
    let mut state = PortForwardPanelState::new();
    state.add_entry(make_entry("pf-1", "nginx", 8080, 80));
    state.set_error("pf-1", "connection refused".to_string());
    assert!(!state.is_port_in_use(8080));
}

// ========================================================================
// Create dialog
// ========================================================================

#[test]
fn test_open_create_dialog() {
    let mut state = PortForwardPanelState::new();
    state.open_create_dialog();
    assert!(state.show_create_dialog);
}

#[test]
fn test_close_create_dialog() {
    let mut state = PortForwardPanelState::new();
    state.open_create_dialog();
    state.close_create_dialog();
    assert!(!state.show_create_dialog);
}

#[test]
fn test_open_create_dialog_clears_inputs() {
    let mut state = PortForwardPanelState::new();
    state.new_local_port = "8080".to_string();
    state.new_remote_port = "80".to_string();
    state.open_create_dialog();
    assert!(state.new_local_port.is_empty());
    assert!(state.new_remote_port.is_empty());
}

// ========================================================================
// Create dialog validation
// ========================================================================

#[test]
fn test_validate_valid_ports() {
    let mut state = PortForwardPanelState::new();
    state.new_local_port = "8080".to_string();
    state.new_remote_port = "80".to_string();
    let result = state.validate_new_forward();
    assert_eq!(result, Ok((8080, 80)));
}

#[test]
fn test_validate_invalid_local_port() {
    let mut state = PortForwardPanelState::new();
    state.new_local_port = "abc".to_string();
    state.new_remote_port = "80".to_string();
    assert!(state.validate_new_forward().is_err());
}

#[test]
fn test_validate_invalid_remote_port() {
    let mut state = PortForwardPanelState::new();
    state.new_local_port = "8080".to_string();
    state.new_remote_port = "xyz".to_string();
    assert!(state.validate_new_forward().is_err());
}

#[test]
fn test_validate_zero_local_port() {
    let mut state = PortForwardPanelState::new();
    state.new_local_port = "0".to_string();
    state.new_remote_port = "80".to_string();
    let result = state.validate_new_forward();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Local port must be greater"));
}

#[test]
fn test_validate_zero_remote_port() {
    let mut state = PortForwardPanelState::new();
    state.new_local_port = "8080".to_string();
    state.new_remote_port = "0".to_string();
    let result = state.validate_new_forward();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Remote port must be greater"));
}

#[test]
fn test_validate_port_already_in_use() {
    let mut state = PortForwardPanelState::new();
    state.add_entry(make_entry("pf-1", "nginx", 8080, 80));
    state.new_local_port = "8080".to_string();
    state.new_remote_port = "80".to_string();
    let result = state.validate_new_forward();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("already in use"));
}

#[test]
fn test_validate_port_not_in_use_after_stop() {
    let mut state = PortForwardPanelState::new();
    state.add_entry(make_entry("pf-1", "nginx", 8080, 80));
    state.stop_entry("pf-1");
    state.new_local_port = "8080".to_string();
    state.new_remote_port = "80".to_string();
    assert!(state.validate_new_forward().is_ok());
}

#[test]
fn test_validate_empty_local_port() {
    let mut state = PortForwardPanelState::new();
    state.new_local_port = String::new();
    state.new_remote_port = "80".to_string();
    assert!(state.validate_new_forward().is_err());
}

// ========================================================================
// Display state labels
// ========================================================================

#[test]
fn test_display_state_label_active() {
    assert_eq!(PortForwardDisplayState::Active.label(), "Active");
}

#[test]
fn test_display_state_label_stopped() {
    assert_eq!(PortForwardDisplayState::Stopped.label(), "Stopped");
}

#[test]
fn test_display_state_label_error() {
    assert_eq!(PortForwardDisplayState::Error.label(), "Error");
}

#[test]
fn test_display_state_label_starting() {
    assert_eq!(PortForwardDisplayState::Starting.label(), "Starting");
}

// ========================================================================
// Entry port display
// ========================================================================

#[test]
fn test_entry_port_display() {
    let entry = make_entry("pf-1", "nginx", 8080, 80);
    assert_eq!(entry.port_display(), "8080:80");
}

#[test]
fn test_entry_port_display_same_ports() {
    let entry = make_entry("pf-1", "redis", 6379, 6379);
    assert_eq!(entry.port_display(), "6379:6379");
}

// ========================================================================
// Get entry
// ========================================================================

#[test]
fn test_get_entry() {
    let mut state = PortForwardPanelState::new();
    state.add_entry(make_entry("pf-1", "nginx", 8080, 80));
    let entry = state.get_entry("pf-1").unwrap();
    assert_eq!(entry.pod_name, "nginx");
}

#[test]
fn test_get_entry_nonexistent() {
    let state = PortForwardPanelState::new();
    assert!(state.get_entry("pf-999").is_none());
}

// ========================================================================
// Serialization
// ========================================================================

#[test]
fn test_entry_serialization_roundtrip() {
    let entry = make_entry("pf-1", "nginx", 8080, 80);
    let json = serde_json::to_string(&entry).unwrap();
    let deser: PortForwardEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(deser.id, "pf-1");
    assert_eq!(deser.pod_name, "nginx");
    assert_eq!(deser.local_port, 8080);
    assert_eq!(deser.remote_port, 80);
    assert_eq!(deser.state, PortForwardDisplayState::Active);
}

#[test]
fn test_display_state_serialization() {
    let state = PortForwardDisplayState::Error;
    let json = serde_json::to_string(&state).unwrap();
    let deser: PortForwardDisplayState = serde_json::from_str(&json).unwrap();
    assert_eq!(deser, PortForwardDisplayState::Error);
}

// ========================================================================
// View component
// ========================================================================

#[test]
fn test_panel_component_creation() {
    let state = PortForwardPanelState::new();
    let component = PortForwardPanelComponent::new(state, baeus_ui::theme::Theme::dark());
    assert!(component.state.entries.is_empty());
    assert!(!component.state.show_create_dialog);
}

#[test]
fn test_panel_component_with_entries() {
    let mut state = PortForwardPanelState::new();
    state.add_entry(make_entry("pf-1", "nginx", 8080, 80));
    state.add_entry(make_entry("pf-2", "redis", 6379, 6379));
    let component = PortForwardPanelComponent::new(state, baeus_ui::theme::Theme::dark());
    assert_eq!(component.state.active_count(), 2);
}
