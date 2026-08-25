// Integration tests for resource_detail module.
// Migrated from inline #[cfg(test)] mod tests in crates/baeus-ui/src/views/resource_detail.rs.

use std::collections::HashMap;

use baeus_editor::buffer::TextBuffer;
use baeus_ui::theme::{Color, Theme};
use baeus_ui::views::resource_detail::*;

// --- DetailTab tests ---

#[test]
fn test_detail_tab_labels() {
    assert_eq!(DetailTab::Overview.label(), "Overview");
    assert_eq!(DetailTab::Spec.label(), "Spec");
    assert_eq!(DetailTab::Status.label(), "Status");
    assert_eq!(DetailTab::Conditions.label(), "Conditions");
    assert_eq!(DetailTab::Events.label(), "Events");
    assert_eq!(DetailTab::Yaml.label(), "YAML");
    assert_eq!(DetailTab::Logs.label(), "Logs");
    assert_eq!(DetailTab::Terminal.label(), "Terminal");
    assert_eq!(DetailTab::PortForward.label(), "Port Forward");
}

#[test]
fn test_detail_tab_serialization() {
    let tab = DetailTab::Overview;
    let json = serde_json::to_string(&tab).unwrap();
    let deserialized: DetailTab = serde_json::from_str(&json).unwrap();
    assert_eq!(tab, deserialized);
}

#[test]
fn test_detail_tab_serialization_all_variants() {
    let tabs = vec![
        DetailTab::Overview,
        DetailTab::Spec,
        DetailTab::Status,
        DetailTab::Conditions,
        DetailTab::Events,
        DetailTab::Yaml,
        DetailTab::Logs,
        DetailTab::Terminal,
        DetailTab::PortForward,
    ];
    for tab in tabs {
        let json = serde_json::to_string(&tab).unwrap();
        let deserialized: DetailTab = serde_json::from_str(&json).unwrap();
        assert_eq!(tab, deserialized);
    }
}

// --- tabs_for_kind tests ---

#[test]
fn test_tabs_for_pod() {
    let tabs = tabs_for_kind("Pod");
    assert_eq!(tabs.len(), 9);
    assert_eq!(tabs[0], DetailTab::Overview);
    assert_eq!(tabs[6], DetailTab::Logs);
    assert_eq!(tabs[7], DetailTab::Terminal);
    assert_eq!(tabs[8], DetailTab::PortForward);
}

#[test]
fn test_tabs_for_deployment() {
    let tabs = tabs_for_kind("Deployment");
    assert_eq!(tabs.len(), 6);
    assert_eq!(tabs[0], DetailTab::Overview);
    assert_eq!(tabs[5], DetailTab::Yaml);
    // No Logs or Terminal for non-Pod kinds
    assert!(!tabs.contains(&DetailTab::Logs));
    assert!(!tabs.contains(&DetailTab::Terminal));
}

#[test]
fn test_tabs_for_service() {
    let tabs = tabs_for_kind("Service");
    assert_eq!(tabs.len(), 7);
    assert!(!tabs.contains(&DetailTab::Logs));
    assert!(!tabs.contains(&DetailTab::Terminal));
    assert!(tabs.contains(&DetailTab::PortForward));
}

#[test]
fn test_tabs_for_unknown_kind() {
    let tabs = tabs_for_kind("CustomResource");
    assert_eq!(tabs.len(), 6);
    assert!(!tabs.contains(&DetailTab::Logs));
}

#[test]
fn test_tabs_base_order() {
    let tabs = tabs_for_kind("ConfigMap");
    assert_eq!(tabs[0], DetailTab::Overview);
    assert_eq!(tabs[1], DetailTab::Spec);
    assert_eq!(tabs[2], DetailTab::Status);
    assert_eq!(tabs[3], DetailTab::Conditions);
    assert_eq!(tabs[4], DetailTab::Events);
    assert_eq!(tabs[5], DetailTab::Yaml);
}

// --- ResourceDetailState tests ---

#[test]
fn test_new_deployment() {
    let state = ResourceDetailState::new("Deployment", "nginx", Some("default".to_string()));
    assert_eq!(state.kind, "Deployment");
    assert_eq!(state.name, "nginx");
    assert_eq!(state.namespace.as_deref(), Some("default"));
    assert!(state.uid.is_none());
    assert_eq!(state.active_tab, DetailTab::Overview);
    assert_eq!(state.available_tabs.len(), 6);
    assert!(!state.loading);
    assert!(state.error.is_none());
    assert!(state.spec_json.is_none());
    assert!(state.status_json.is_none());
    assert!(state.conditions.is_empty());
    assert!(state.events.is_empty());
    assert!(state.related_resources.is_empty());
}

#[test]
fn test_new_pod() {
    let state = ResourceDetailState::new("Pod", "nginx-abc-123", Some("default".to_string()));
    assert_eq!(state.kind, "Pod");
    assert_eq!(state.available_tabs.len(), 9);
    assert!(state.available_tabs.contains(&DetailTab::Logs));
    assert!(state.available_tabs.contains(&DetailTab::Terminal));
    assert!(state.available_tabs.contains(&DetailTab::PortForward));
}

#[test]
fn test_new_cluster_scoped_resource() {
    let state = ResourceDetailState::new("Node", "node-1", None);
    assert_eq!(state.kind, "Node");
    assert_eq!(state.name, "node-1");
    assert!(state.namespace.is_none());
    assert_eq!(state.available_tabs.len(), 6);
}

#[test]
fn test_switch_tab() {
    let mut state = ResourceDetailState::new("Pod", "nginx", Some("default".to_string()));
    assert_eq!(state.active_tab, DetailTab::Overview);

    state.switch_tab(DetailTab::Logs);
    assert_eq!(state.active_tab, DetailTab::Logs);

    state.switch_tab(DetailTab::Yaml);
    assert_eq!(state.active_tab, DetailTab::Yaml);

    state.switch_tab(DetailTab::Overview);
    assert_eq!(state.active_tab, DetailTab::Overview);
}

#[test]
fn test_set_loading() {
    let mut state = ResourceDetailState::new("Deployment", "nginx", Some("default".to_string()));
    assert!(!state.loading);

    state.set_loading(true);
    assert!(state.loading);

    state.set_loading(false);
    assert!(!state.loading);
}

#[test]
fn test_set_error_and_clear() {
    let mut state = ResourceDetailState::new("Deployment", "nginx", Some("default".to_string()));
    assert!(state.error.is_none());

    state.set_error("not found".to_string());
    assert_eq!(state.error.as_deref(), Some("not found"));

    state.clear_error();
    assert!(state.error.is_none());
}

#[test]
fn test_set_spec() {
    let mut state = ResourceDetailState::new("Deployment", "nginx", Some("default".to_string()));
    assert!(state.spec_json.is_none());

    let spec = r#"{"replicas": 3, "selector": {"matchLabels": {"app": "nginx"}}}"#;
    state.set_spec(spec.to_string());
    assert_eq!(state.spec_json.as_deref(), Some(spec));
}

#[test]
fn test_set_status() {
    let mut state = ResourceDetailState::new("Deployment", "nginx", Some("default".to_string()));
    assert!(state.status_json.is_none());

    let status = r#"{"availableReplicas": 3, "readyReplicas": 3}"#;
    state.set_status(status.to_string());
    assert_eq!(state.status_json.as_deref(), Some(status));
}

#[test]
fn test_set_conditions() {
    let mut state = ResourceDetailState::new("Deployment", "nginx", Some("default".to_string()));
    assert!(state.conditions.is_empty());

    let conditions = vec![
        ConditionDisplay {
            type_name: "Available".to_string(),
            status: "True".to_string(),
            reason: Some("MinimumReplicasAvailable".to_string()),
            message: Some("Deployment has minimum availability.".to_string()),
            age: "5m".to_string(),
        },
        ConditionDisplay {
            type_name: "Progressing".to_string(),
            status: "True".to_string(),
            reason: Some("NewReplicaSetAvailable".to_string()),
            message: None,
            age: "10m".to_string(),
        },
    ];
    state.set_conditions(conditions);
    assert_eq!(state.conditions.len(), 2);
    assert_eq!(state.conditions[0].type_name, "Available");
    assert_eq!(state.conditions[1].type_name, "Progressing");
}

#[test]
fn test_set_events() {
    let mut state = ResourceDetailState::new("Pod", "nginx-abc", Some("default".to_string()));
    assert!(state.events.is_empty());

    let events = vec![
        EventDisplay {
            type_name: "Normal".to_string(),
            reason: "Scheduled".to_string(),
            message: "Successfully assigned default/nginx-abc to node-1".to_string(),
            age: "2m".to_string(),
            count: 1,
        },
        EventDisplay {
            type_name: "Warning".to_string(),
            reason: "BackOff".to_string(),
            message: "Back-off restarting failed container".to_string(),
            age: "1m".to_string(),
            count: 3,
        },
    ];
    state.set_events(events);
    assert_eq!(state.events.len(), 2);
    assert_eq!(state.events[0].reason, "Scheduled");
    assert_eq!(state.events[1].count, 3);
}

#[test]
fn test_add_related() {
    let mut state = ResourceDetailState::new("Deployment", "nginx", Some("default".to_string()));
    assert!(state.related_resources.is_empty());

    state.add_related(RelatedResource {
        kind: "ReplicaSet".to_string(),
        name: "nginx-abc".to_string(),
        namespace: Some("default".to_string()),
        relationship: "Owned".to_string(),
    });
    assert_eq!(state.related_resources.len(), 1);
    assert_eq!(state.related_resources[0].kind, "ReplicaSet");
    assert_eq!(state.related_resources[0].relationship, "Owned");

    state.add_related(RelatedResource {
        kind: "Service".to_string(),
        name: "nginx-svc".to_string(),
        namespace: Some("default".to_string()),
        relationship: "Selected by Service".to_string(),
    });
    assert_eq!(state.related_resources.len(), 2);
}

#[test]
fn test_is_pod() {
    assert!(ResourceDetailState::new("Pod", "nginx", Some("default".to_string())).is_pod());
    assert!(!ResourceDetailState::new("Deployment", "nginx", Some("default".to_string())).is_pod());
    assert!(!ResourceDetailState::new("Node", "node-1", None).is_pod());
}

#[test]
fn test_has_conditions() {
    let mut state = ResourceDetailState::new("Deployment", "nginx", Some("default".to_string()));
    assert!(!state.has_conditions());

    state.set_conditions(vec![ConditionDisplay {
        type_name: "Available".to_string(),
        status: "True".to_string(),
        reason: None,
        message: None,
        age: "5m".to_string(),
    }]);
    assert!(state.has_conditions());
}

#[test]
fn test_warning_event_count() {
    let mut state = ResourceDetailState::new("Pod", "nginx", Some("default".to_string()));
    assert_eq!(state.warning_event_count(), 0);

    state.set_events(vec![
        EventDisplay {
            type_name: "Normal".to_string(),
            reason: "Pulled".to_string(),
            message: "Image pulled".to_string(),
            age: "5m".to_string(),
            count: 1,
        },
        EventDisplay {
            type_name: "Warning".to_string(),
            reason: "BackOff".to_string(),
            message: "Back-off restarting".to_string(),
            age: "3m".to_string(),
            count: 2,
        },
        EventDisplay {
            type_name: "Warning".to_string(),
            reason: "FailedMount".to_string(),
            message: "Mount failed".to_string(),
            age: "1m".to_string(),
            count: 1,
        },
    ]);
    assert_eq!(state.warning_event_count(), 2);
}

#[test]
fn test_warning_event_count_no_warnings() {
    let mut state = ResourceDetailState::new("Pod", "nginx", Some("default".to_string()));
    state.set_events(vec![EventDisplay {
        type_name: "Normal".to_string(),
        reason: "Pulled".to_string(),
        message: "Image pulled".to_string(),
        age: "5m".to_string(),
        count: 1,
    }]);
    assert_eq!(state.warning_event_count(), 0);
}

#[test]
fn test_full_detail_workflow() {
    let mut state =
        ResourceDetailState::new("Pod", "nginx-abc-123", Some("production".to_string()));

    // Start loading
    state.set_loading(true);
    assert!(state.loading);

    // Set spec and status
    state.set_spec(r#"{"containers": [{"name": "nginx"}]}"#.to_string());
    state.set_status(r#"{"phase": "Running"}"#.to_string());

    // Set conditions
    state.set_conditions(vec![ConditionDisplay {
        type_name: "Ready".to_string(),
        status: "True".to_string(),
        reason: Some("PodReady".to_string()),
        message: None,
        age: "2h".to_string(),
    }]);

    // Set events
    state.set_events(vec![
        EventDisplay {
            type_name: "Normal".to_string(),
            reason: "Scheduled".to_string(),
            message: "Pod scheduled".to_string(),
            age: "2h".to_string(),
            count: 1,
        },
        EventDisplay {
            type_name: "Normal".to_string(),
            reason: "Pulled".to_string(),
            message: "Image pulled".to_string(),
            age: "2h".to_string(),
            count: 1,
        },
    ]);

    // Add related resources
    state.add_related(RelatedResource {
        kind: "ReplicaSet".to_string(),
        name: "nginx-abc".to_string(),
        namespace: Some("production".to_string()),
        relationship: "Owner".to_string(),
    });

    // Finish loading
    state.set_loading(false);

    // Verify state
    assert!(!state.loading);
    assert!(state.is_pod());
    assert!(state.has_conditions());
    assert_eq!(state.warning_event_count(), 0);
    assert_eq!(state.events.len(), 2);
    assert_eq!(state.related_resources.len(), 1);
    assert!(state.spec_json.is_some());
    assert!(state.status_json.is_some());

    // Switch to logs tab (Pod-specific)
    state.switch_tab(DetailTab::Logs);
    assert_eq!(state.active_tab, DetailTab::Logs);
}

#[test]
fn test_related_resource_with_no_namespace() {
    let mut state = ResourceDetailState::new("Pod", "nginx", Some("default".to_string()));
    state.add_related(RelatedResource {
        kind: "Node".to_string(),
        name: "node-1".to_string(),
        namespace: None,
        relationship: "Scheduled on".to_string(),
    });
    assert!(state.related_resources[0].namespace.is_none());
    assert_eq!(state.related_resources[0].relationship, "Scheduled on");
}

#[test]
fn test_condition_display_optional_fields() {
    let condition = ConditionDisplay {
        type_name: "Ready".to_string(),
        status: "False".to_string(),
        reason: None,
        message: None,
        age: "30s".to_string(),
    };
    assert!(condition.reason.is_none());
    assert!(condition.message.is_none());
}

#[test]
fn test_error_then_recovery() {
    let mut state = ResourceDetailState::new("Deployment", "nginx", Some("default".to_string()));

    state.set_error("timeout".to_string());
    assert!(state.error.is_some());

    state.clear_error();
    state.set_loading(true);
    assert!(state.error.is_none());
    assert!(state.loading);

    state.set_loading(false);
    assert!(!state.loading);
}

// --- T075: Exec confirmation and tab wiring tests ---

#[test]
fn test_exec_confirm_state_new() {
    let confirm = ExecConfirmState::new("nginx-abc", Some("app"));
    assert_eq!(confirm.pod_name, "nginx-abc");
    assert_eq!(confirm.container_name.as_deref(), Some("app"));
    assert!(!confirm.is_confirmed());
    if let ExecConfirmation::Required { message } = &confirm.confirmation {
        assert!(message.contains("nginx-abc"));
        assert!(message.contains("app"));
    } else {
        panic!("Expected Required");
    }
}

#[test]
fn test_exec_confirm_no_container() {
    let confirm = ExecConfirmState::new("nginx", None);
    assert!(confirm.container_name.is_none());
    if let ExecConfirmation::Required { message } = &confirm.confirmation {
        assert!(message.contains("nginx"));
        assert!(!message.contains("container"));
    } else {
        panic!("Expected Required");
    }
}

#[test]
fn test_exec_confirm_and_confirm() {
    let mut confirm = ExecConfirmState::new("nginx", None);
    assert!(!confirm.is_confirmed());
    confirm.confirm();
    assert!(confirm.is_confirmed());
}

#[test]
fn test_request_exec_on_pod() {
    let mut state = ResourceDetailState::new("Pod", "nginx", Some("default".to_string()));
    assert!(!state.has_exec_confirm());

    state.request_exec(Some("app"));
    assert!(state.has_exec_confirm());
    assert!(!state.is_exec_confirmed());

    state.confirm_exec();
    assert!(state.is_exec_confirmed());
}

#[test]
fn test_cancel_exec() {
    let mut state = ResourceDetailState::new("Pod", "nginx", Some("default".to_string()));
    state.request_exec(None);
    assert!(state.has_exec_confirm());

    state.cancel_exec();
    assert!(!state.has_exec_confirm());
    assert!(!state.is_exec_confirmed());
}

#[test]
fn test_open_logs() {
    let mut state = ResourceDetailState::new("Pod", "nginx", Some("default".to_string()));
    state.open_logs();
    assert_eq!(state.active_tab, DetailTab::Logs);
}

#[test]
fn test_open_logs_non_pod_noop() {
    let mut state = ResourceDetailState::new("Deployment", "nginx", Some("default".to_string()));
    state.open_logs();
    assert_eq!(state.active_tab, DetailTab::Overview); // unchanged
}

#[test]
fn test_open_terminal_with_exec_confirm() {
    let mut state = ResourceDetailState::new("Pod", "nginx", Some("default".to_string()));
    state.open_terminal(Some("app"));
    assert_eq!(state.active_tab, DetailTab::Terminal);
    assert!(state.has_exec_confirm());
}

#[test]
fn test_open_terminal_non_pod_noop() {
    let mut state = ResourceDetailState::new("Service", "nginx", Some("default".to_string()));
    state.open_terminal(None);
    assert_eq!(state.active_tab, DetailTab::Overview); // unchanged
    assert!(!state.has_exec_confirm());
}

#[test]
fn test_open_port_forward_pod() {
    let mut state = ResourceDetailState::new("Pod", "nginx", Some("default".to_string()));
    state.open_port_forward();
    assert_eq!(state.active_tab, DetailTab::PortForward);
    assert!(state.has_port_forward_confirm());
    assert!(!state.is_port_forward_confirmed());
}

#[test]
fn test_open_port_forward_service() {
    let mut state = ResourceDetailState::new("Service", "nginx-svc", Some("default".to_string()));
    state.open_port_forward();
    assert_eq!(state.active_tab, DetailTab::PortForward);
    assert!(state.has_port_forward_confirm());
}

#[test]
fn test_open_port_forward_deployment_noop() {
    let mut state = ResourceDetailState::new("Deployment", "nginx", Some("default".to_string()));
    state.open_port_forward();
    assert_eq!(state.active_tab, DetailTab::Overview); // unchanged
    assert!(!state.has_port_forward_confirm());
}

// --- T137: Port-forward confirmation tests ---

#[test]
fn test_port_forward_confirm_state_new() {
    let confirm = PortForwardConfirmState::new("Pod", "nginx", Some(8080), Some(80));
    assert_eq!(confirm.resource_name, "nginx");
    assert_eq!(confirm.resource_kind, "Pod");
    assert_eq!(confirm.local_port, Some(8080));
    assert_eq!(confirm.remote_port, Some(80));
    assert!(!confirm.is_confirmed());
    if let PortForwardConfirmation::Required { message } = &confirm.confirmation {
        assert!(message.contains("nginx"));
        assert!(message.contains("8080"));
        assert!(message.contains("80"));
    } else {
        panic!("Expected Required");
    }
}

#[test]
fn test_port_forward_confirm_no_ports() {
    let confirm = PortForwardConfirmState::new("Service", "web", None, None);
    assert!(confirm.local_port.is_none());
    if let PortForwardConfirmation::Required { message } = &confirm.confirmation {
        assert!(message.contains("web"));
        // No port numbers in the message when ports are not specified
        assert!(!message.contains("local:"));
        assert!(!message.contains("remote:"));
    } else {
        panic!("Expected Required");
    }
}

#[test]
fn test_port_forward_confirm_and_confirm() {
    let mut confirm = PortForwardConfirmState::new("Pod", "nginx", None, Some(80));
    assert!(!confirm.is_confirmed());
    confirm.confirm();
    assert!(confirm.is_confirmed());
}

#[test]
fn test_request_port_forward_on_pod() {
    let mut state = ResourceDetailState::new("Pod", "nginx", Some("default".to_string()));
    assert!(!state.has_port_forward_confirm());

    state.request_port_forward(Some(8080), Some(80));
    assert!(state.has_port_forward_confirm());
    assert!(!state.is_port_forward_confirmed());

    state.confirm_port_forward();
    assert!(state.is_port_forward_confirmed());
}

#[test]
fn test_cancel_port_forward() {
    let mut state = ResourceDetailState::new("Pod", "nginx", Some("default".to_string()));
    state.request_port_forward(None, Some(80));
    assert!(state.has_port_forward_confirm());

    state.cancel_port_forward();
    assert!(!state.has_port_forward_confirm());
    assert!(!state.is_port_forward_confirmed());
}

#[test]
fn test_port_forward_full_workflow() {
    let mut state = ResourceDetailState::new("Pod", "nginx", Some("prod".to_string()));

    // Open port-forward tab with confirmation
    state.open_port_forward();
    assert_eq!(state.active_tab, DetailTab::PortForward);
    assert!(state.has_port_forward_confirm());
    assert!(!state.is_port_forward_confirmed());

    // Confirm port-forward
    state.confirm_port_forward();
    assert!(state.is_port_forward_confirmed());

    // Cleanup after session
    state.cancel_port_forward();
    assert!(!state.has_port_forward_confirm());
}

#[test]
fn test_supports_port_forward() {
    assert!(ResourceDetailState::new("Pod", "x", None).supports_port_forward());
    assert!(ResourceDetailState::new("Service", "x", None).supports_port_forward());
    assert!(!ResourceDetailState::new("Deployment", "x", None).supports_port_forward());
    assert!(!ResourceDetailState::new("ConfigMap", "x", None).supports_port_forward());
}

#[test]
fn test_set_container_names() {
    let mut state = ResourceDetailState::new("Pod", "nginx", Some("default".to_string()));
    assert!(state.container_names.is_empty());

    state.set_container_names(vec!["app".to_string(), "sidecar".to_string()]);
    assert_eq!(state.container_names.len(), 2);
}

// --- T084: YAML tab wiring tests ---

#[test]
fn test_set_resource_yaml() {
    let mut state = ResourceDetailState::new("Deployment", "nginx", Some("default".to_string()));
    assert!(state.resource_yaml.is_none());
    assert!(state.resource_version.is_none());

    state.set_resource_yaml(
        "apiVersion: apps/v1\nkind: Deployment\n".to_string(),
        "12345".to_string(),
    );
    assert_eq!(state.resource_yaml.as_deref(), Some("apiVersion: apps/v1\nkind: Deployment\n"));
    assert_eq!(state.resource_version.as_deref(), Some("12345"));
}

#[test]
fn test_open_yaml_editor() {
    let mut state = ResourceDetailState::new("Deployment", "nginx", Some("default".to_string()));
    state.set_resource_yaml("apiVersion: apps/v1\n".to_string(), "12345".to_string());

    assert!(!state.has_yaml_editor());
    state.open_yaml_editor();

    assert_eq!(state.active_tab, DetailTab::Yaml);
    assert!(state.has_yaml_editor());

    let editor = state.yaml_editor_ref().unwrap();
    assert_eq!(editor.resource_kind, "Deployment");
    assert_eq!(editor.resource_name, "nginx");
    assert_eq!(editor.resource_version, "12345");
}

#[test]
fn test_open_yaml_editor_no_yaml_set() {
    let mut state = ResourceDetailState::new("Deployment", "nginx", Some("default".to_string()));
    state.open_yaml_editor();

    assert_eq!(state.active_tab, DetailTab::Yaml);
    // Editor not initialized because no YAML content was set
    assert!(!state.has_yaml_editor());
}

#[test]
fn test_open_yaml_editor_reuses_existing() {
    let mut state = ResourceDetailState::new("Deployment", "nginx", Some("default".to_string()));
    state.set_resource_yaml("apiVersion: apps/v1\n".to_string(), "1".to_string());
    state.open_yaml_editor();

    // Make a change in the editor
    state.yaml_editor_mut().unwrap().insert(0, "# comment\n");
    assert!(state.yaml_editor_ref().unwrap().is_dirty);

    // Re-opening should reuse the existing editor (not reset it)
    state.open_yaml_editor();
    assert!(state.yaml_editor_ref().unwrap().is_dirty);
}

#[test]
fn test_set_resource_yaml_resets_editor() {
    let mut state = ResourceDetailState::new("Deployment", "nginx", Some("default".to_string()));
    state.set_resource_yaml("original: yaml\n".to_string(), "1".to_string());
    state.open_yaml_editor();
    assert!(state.has_yaml_editor());

    // Setting new YAML resets the editor
    state.set_resource_yaml("new: yaml\n".to_string(), "2".to_string());
    assert!(!state.has_yaml_editor());

    // Re-opening creates a fresh editor
    state.open_yaml_editor();
    assert!(state.has_yaml_editor());
    assert_eq!(state.yaml_editor_ref().unwrap().resource_version, "2");
}

#[test]
fn test_on_yaml_apply_success() {
    let mut state = ResourceDetailState::new("Deployment", "nginx", Some("default".to_string()));
    state.set_resource_yaml("original: yaml\n".to_string(), "1".to_string());
    state.open_yaml_editor();
    state.yaml_editor_mut().unwrap().insert(0, "# modified\n");
    state.yaml_editor_mut().unwrap().begin_apply();

    let new_yaml = state.yaml_editor_ref().unwrap().text();
    state.on_yaml_apply_success(new_yaml.clone(), "2".to_string());

    assert_eq!(state.resource_version.as_deref(), Some("2"));
    assert_eq!(state.resource_yaml.as_deref(), Some(new_yaml.as_str()));
    assert!(!state.yaml_editor_ref().unwrap().is_dirty);
}

#[test]
fn test_on_yaml_apply_conflict() {
    let mut state = ResourceDetailState::new("Deployment", "nginx", Some("default".to_string()));
    state.set_resource_yaml("original: yaml\n".to_string(), "1".to_string());
    state.open_yaml_editor();
    state.yaml_editor_mut().unwrap().insert(0, "# changed\n");
    state.yaml_editor_mut().unwrap().begin_apply();

    state.on_yaml_apply_conflict("server: version\n".to_string());
    assert!(state.yaml_editor_ref().unwrap().has_conflict());
}

#[test]
fn test_on_yaml_apply_failure() {
    let mut state = ResourceDetailState::new("Deployment", "nginx", Some("default".to_string()));
    state.set_resource_yaml("original: yaml\n".to_string(), "1".to_string());
    state.open_yaml_editor();
    state.yaml_editor_mut().unwrap().begin_apply();

    state.on_yaml_apply_failure("forbidden".to_string());
    assert_eq!(state.yaml_editor_ref().unwrap().apply_error.as_deref(), Some("forbidden"));
}

#[test]
fn test_yaml_editor_full_workflow() {
    let mut state = ResourceDetailState::new("Deployment", "nginx", Some("default".to_string()));

    // 1. Load resource data including YAML
    state.set_resource_yaml(
        "apiVersion: apps/v1\nkind: Deployment\nspec:\n  replicas: 3\n".to_string(),
        "100".to_string(),
    );

    // 2. User clicks YAML tab
    state.open_yaml_editor();
    assert_eq!(state.active_tab, DetailTab::Yaml);
    assert!(state.has_yaml_editor());

    // 3. User edits YAML
    let editor = state.yaml_editor_mut().unwrap();
    editor.buffer =
        TextBuffer::from_str("apiVersion: apps/v1\nkind: Deployment\nspec:\n  replicas: 5\n");
    editor.is_dirty = true;
    editor.validate();
    assert!(editor.can_apply());

    // 4. User clicks Apply
    editor.begin_apply();

    // 5. Server returns success
    let new_yaml = state.yaml_editor_ref().unwrap().text();
    state.on_yaml_apply_success(new_yaml, "101".to_string());

    assert_eq!(state.resource_version.as_deref(), Some("101"));
    assert!(!state.yaml_editor_ref().unwrap().is_dirty);
}

#[test]
fn test_exec_full_workflow() {
    let mut state = ResourceDetailState::new("Pod", "nginx-abc", Some("prod".to_string()));
    state.set_container_names(vec!["app".to_string(), "istio-proxy".to_string()]);

    // Open terminal tab with exec confirm
    state.open_terminal(Some("app"));
    assert_eq!(state.active_tab, DetailTab::Terminal);
    assert!(state.has_exec_confirm());
    assert!(!state.is_exec_confirmed());

    // Confirm exec
    state.confirm_exec();
    assert!(state.is_exec_confirmed());

    // After exec session is done, cancel cleans up
    state.cancel_exec();
    assert!(!state.has_exec_confirm());
}

// ===================================================================
// T106: Service-specific detail tests (inline)
// ===================================================================

#[test]
fn test_service_type_all_variants() {
    let types = [
        ServiceType::ClusterIP,
        ServiceType::NodePort,
        ServiceType::LoadBalancer,
        ServiceType::ExternalName,
    ];
    assert_ne!(types[0], types[1]);
    assert_ne!(types[1], types[2]);
    assert_ne!(types[2], types[3]);
}

#[test]
fn test_service_detail_construction() {
    let detail = ServiceDetail {
        service_type: ServiceType::ClusterIP,
        cluster_ip: Some("10.96.0.1".to_string()),
        external_ips: vec![],
        ports: vec![ServicePort {
            name: Some("http".to_string()),
            protocol: "TCP".to_string(),
            port: 80,
            target_port: "8080".to_string(),
            node_port: None,
        }],
        selectors: HashMap::from([("app".to_string(), "nginx".to_string())]),
    };
    assert_eq!(detail.service_type, ServiceType::ClusterIP);
    assert_eq!(detail.ports.len(), 1);
    assert_eq!(detail.selectors.len(), 1);
}

#[test]
fn test_service_detail_clone() {
    let detail = ServiceDetail {
        service_type: ServiceType::LoadBalancer,
        cluster_ip: Some("10.96.0.1".to_string()),
        external_ips: vec!["1.2.3.4".to_string()],
        ports: vec![],
        selectors: HashMap::new(),
    };
    let cloned = detail.clone();
    assert_eq!(cloned.service_type, ServiceType::LoadBalancer);
    assert_eq!(cloned.external_ips.len(), 1);
}

// ===================================================================
// T107: Ingress-specific detail tests (inline)
// ===================================================================

#[test]
fn test_ingress_detail_construction() {
    let detail = IngressDetail {
        rules: vec![IngressRule {
            host: Some("example.com".to_string()),
            paths: vec![IngressPath {
                path: "/".to_string(),
                path_type: "Prefix".to_string(),
                backend_service: "web".to_string(),
                backend_port: 80,
            }],
        }],
        default_backend: None,
        tls: vec![],
    };
    assert_eq!(detail.rules.len(), 1);
    assert!(detail.default_backend.is_none());
}

#[test]
fn test_tls_config_construction() {
    let tls = TlsConfig {
        hosts: vec!["example.com".to_string()],
        secret_name: Some("my-tls".to_string()),
    };
    assert_eq!(tls.hosts.len(), 1);
    assert_eq!(tls.secret_name.as_deref(), Some("my-tls"));
}

#[test]
fn test_ingress_detail_clone() {
    let detail = IngressDetail {
        rules: vec![],
        default_backend: Some("fallback:80".to_string()),
        tls: vec![TlsConfig { hosts: vec!["a.com".to_string()], secret_name: None }],
    };
    let cloned = detail.clone();
    assert_eq!(cloned.default_backend.as_deref(), Some("fallback:80"));
    assert_eq!(cloned.tls.len(), 1);
}

// ===================================================================
// T108: PVC-specific detail tests (inline)
// ===================================================================

#[test]
fn test_pvc_status_all_variants() {
    let statuses = [PvcStatus::Bound, PvcStatus::Pending, PvcStatus::Lost];
    assert_ne!(statuses[0], statuses[1]);
    assert_ne!(statuses[1], statuses[2]);
    assert_ne!(statuses[0], statuses[2]);
}

#[test]
fn test_pvc_access_mode_all_variants() {
    let modes =
        [PvcAccessMode::ReadWriteOnce, PvcAccessMode::ReadOnlyMany, PvcAccessMode::ReadWriteMany];
    assert_ne!(modes[0], modes[1]);
    assert_ne!(modes[1], modes[2]);
    assert_ne!(modes[0], modes[2]);
}

#[test]
fn test_pvc_detail_construction() {
    let detail = PvcDetail {
        status: PvcStatus::Bound,
        capacity: Some("10Gi".to_string()),
        access_modes: vec![PvcAccessMode::ReadWriteOnce],
        storage_class_name: Some("standard".to_string()),
        volume_name: Some("pv-001".to_string()),
    };
    assert_eq!(detail.status, PvcStatus::Bound);
    assert_eq!(detail.capacity.as_deref(), Some("10Gi"));
    assert_eq!(detail.access_modes.len(), 1);
}

#[test]
fn test_pvc_detail_clone() {
    let detail = PvcDetail {
        status: PvcStatus::Pending,
        capacity: None,
        access_modes: vec![PvcAccessMode::ReadWriteMany],
        storage_class_name: None,
        volume_name: None,
    };
    let cloned = detail.clone();
    assert_eq!(cloned.status, PvcStatus::Pending);
    assert!(cloned.capacity.is_none());
}

// ========================================================================
// T034: Render-related state tests for ResourceDetailView
// ========================================================================

#[test]
fn test_view_active_tab_label_default() {
    let state = ResourceDetailState::new("Deployment", "nginx", Some("default".to_string()));
    let view = ResourceDetailView::new(state, Theme::dark());
    assert_eq!(view.active_tab_label(), "Overview");
}

#[test]
fn test_view_active_tab_label_after_switch() {
    let mut state = ResourceDetailState::new("Pod", "nginx", Some("default".to_string()));
    state.switch_tab(DetailTab::Logs);
    let view = ResourceDetailView::new(state, Theme::dark());
    assert_eq!(view.active_tab_label(), "Logs");
}

#[test]
fn test_view_tab_labels_deployment() {
    let state = ResourceDetailState::new("Deployment", "nginx", Some("default".to_string()));
    let view = ResourceDetailView::new(state, Theme::dark());
    let labels = view.tab_labels();
    assert_eq!(labels.len(), 6);
    assert_eq!(labels[0], "Overview");
    assert_eq!(labels[1], "Spec");
    assert_eq!(labels[2], "Status");
    assert_eq!(labels[3], "Conditions");
    assert_eq!(labels[4], "Events");
    assert_eq!(labels[5], "YAML");
}

#[test]
fn test_view_tab_labels_pod() {
    let state = ResourceDetailState::new("Pod", "nginx", Some("default".to_string()));
    let view = ResourceDetailView::new(state, Theme::dark());
    let labels = view.tab_labels();
    assert_eq!(labels.len(), 9);
    assert!(labels.contains(&"Logs"));
    assert!(labels.contains(&"Terminal"));
    assert!(labels.contains(&"Port Forward"));
}

#[test]
fn test_view_tab_labels_service() {
    let state = ResourceDetailState::new("Service", "nginx-svc", Some("default".to_string()));
    let view = ResourceDetailView::new(state, Theme::dark());
    let labels = view.tab_labels();
    assert_eq!(labels.len(), 7);
    assert!(labels.contains(&"Port Forward"));
    assert!(!labels.contains(&"Logs"));
    assert!(!labels.contains(&"Terminal"));
}

#[test]
fn test_view_with_dark_theme() {
    let state = ResourceDetailState::new("Pod", "nginx", Some("default".to_string()));
    let view = ResourceDetailView::new(state, Theme::dark());
    assert_eq!(view.theme.colors.background, Color::rgb(0x1e, 0x21, 0x24));
}

#[test]
fn test_view_with_light_theme() {
    let state = ResourceDetailState::new("Pod", "nginx", Some("default".to_string()));
    let view = ResourceDetailView::new(state, Theme::light());
    assert_eq!(view.theme.colors.background, Color::rgb(255, 255, 255));
}

#[test]
fn test_view_loading_state() {
    let mut state = ResourceDetailState::new("Pod", "nginx", Some("default".to_string()));
    state.set_loading(true);
    let view = ResourceDetailView::new(state, Theme::dark());
    assert!(view.state.loading);
}

#[test]
fn test_view_error_state() {
    let mut state = ResourceDetailState::new("Pod", "nginx", Some("default".to_string()));
    state.set_error("not found".to_string());
    let view = ResourceDetailView::new(state, Theme::dark());
    assert_eq!(view.state.error.as_deref(), Some("not found"));
}

#[test]
fn test_view_cluster_scoped_resource() {
    let state = ResourceDetailState::new("Node", "node-1", None);
    let view = ResourceDetailView::new(state, Theme::dark());
    assert!(view.state.namespace.is_none());
}

#[test]
fn test_view_with_events_tab_active() {
    let mut state = ResourceDetailState::new("Pod", "nginx", Some("default".to_string()));
    state.switch_tab(DetailTab::Events);
    state.set_events(vec![
        EventDisplay {
            type_name: "Normal".to_string(),
            reason: "Pulled".to_string(),
            message: "Image pulled".to_string(),
            age: "5m".to_string(),
            count: 1,
        },
        EventDisplay {
            type_name: "Warning".to_string(),
            reason: "BackOff".to_string(),
            message: "Restart failed".to_string(),
            age: "2m".to_string(),
            count: 3,
        },
    ]);
    let view = ResourceDetailView::new(state, Theme::dark());
    assert_eq!(view.active_tab_label(), "Events");
    assert_eq!(view.state.events.len(), 2);
    assert_eq!(view.state.warning_event_count(), 1);
}

#[test]
fn test_view_with_spec_tab_active() {
    let mut state = ResourceDetailState::new("Deployment", "nginx", Some("default".to_string()));
    state.switch_tab(DetailTab::Spec);
    state.set_spec(r#"{"replicas": 3}"#.to_string());
    let view = ResourceDetailView::new(state, Theme::dark());
    assert_eq!(view.active_tab_label(), "Spec");
    assert_eq!(view.state.spec_json.as_deref(), Some(r#"{"replicas": 3}"#));
}

#[test]
fn test_view_with_status_tab_active() {
    let mut state = ResourceDetailState::new("Deployment", "nginx", Some("default".to_string()));
    state.switch_tab(DetailTab::Status);
    state.set_status(r#"{"readyReplicas": 3}"#.to_string());
    let view = ResourceDetailView::new(state, Theme::dark());
    assert_eq!(view.active_tab_label(), "Status");
    assert!(view.state.status_json.is_some());
}

#[test]
fn test_view_related_resources_accessible() {
    let mut state = ResourceDetailState::new("Deployment", "nginx", Some("default".to_string()));
    state.add_related(RelatedResource {
        kind: "ReplicaSet".to_string(),
        name: "nginx-abc".to_string(),
        namespace: Some("default".to_string()),
        relationship: "Owned".to_string(),
    });
    let view = ResourceDetailView::new(state, Theme::dark());
    assert_eq!(view.state.related_resources.len(), 1);
}
