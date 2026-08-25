//! Integration tests for Network resource kinds in ResourceListView.
//!
//! Tests verify that the ResourceListView correctly handles network resource
//! kinds (Service, Ingress, NetworkPolicy, Endpoints) including:
//! - Column definitions
//! - ResourceListState operations
//! - Quick actions
//! - RBAC integration

use baeus_core::rbac::{PermissionCheck, PermissionResult, RbacCache, RbacVerb};
use baeus_ui::views::resource_list::{
    QuickAction, ResourceListState, actions_for_kind, api_group_for_kind, columns_for_kind,
    kind_to_plural, resource_for_action, verb_for_action,
};

// ---------------------------------------------------------------------------
// T071: Network resource column definitions
// ---------------------------------------------------------------------------

#[test]
fn test_service_columns() {
    let cols = columns_for_kind("Service");

    assert_eq!(cols.len(), 7);
    assert_eq!(cols[0].id, "name");
    assert_eq!(cols[1].id, "namespace");
    assert_eq!(cols[2].id, "type");
    assert_eq!(cols[3].id, "cluster_ip");
    assert_eq!(cols[4].id, "external_ip");
    assert_eq!(cols[5].id, "ports");
    assert_eq!(cols[6].id, "age");

    // Verify sortable flags
    assert!(cols[0].sortable); // name
    assert!(cols[1].sortable); // namespace
    assert!(cols[2].sortable); // type
    assert!(!cols[3].sortable); // cluster_ip
    assert!(!cols[4].sortable); // external_ip
    assert!(!cols[5].sortable); // ports
    assert!(cols[6].sortable); // age
}

#[test]
fn test_ingress_columns() {
    let cols = columns_for_kind("Ingress");

    assert_eq!(cols.len(), 6);
    assert_eq!(cols[0].id, "name");
    assert_eq!(cols[1].id, "namespace");
    assert_eq!(cols[2].id, "hosts");
    assert_eq!(cols[3].id, "address");
    assert_eq!(cols[4].id, "ports");
    assert_eq!(cols[5].id, "age");

    // Verify sortable flags
    assert!(cols[0].sortable); // name
    assert!(cols[1].sortable); // namespace
    assert!(!cols[2].sortable); // hosts
    assert!(!cols[3].sortable); // address
    assert!(!cols[4].sortable); // ports
    assert!(cols[5].sortable); // age
}

#[test]
fn test_network_policy_columns() {
    let cols = columns_for_kind("NetworkPolicy");

    assert_eq!(cols.len(), 4);
    assert_eq!(cols[0].id, "name");
    assert_eq!(cols[1].id, "namespace");
    assert_eq!(cols[2].id, "pod_selector");
    assert_eq!(cols[3].id, "age");

    // Verify sortable flags
    assert!(cols[0].sortable); // name
    assert!(cols[1].sortable); // namespace
    assert!(!cols[2].sortable); // pod_selector
    assert!(cols[3].sortable); // age
}

#[test]
fn test_endpoints_columns() {
    let cols = columns_for_kind("Endpoints");

    assert_eq!(cols.len(), 4);
    assert_eq!(cols[0].id, "name");
    assert_eq!(cols[1].id, "namespace");
    assert_eq!(cols[2].id, "endpoints");
    assert_eq!(cols[3].id, "age");

    // Verify sortable flags
    assert!(cols[0].sortable); // name
    assert!(cols[1].sortable); // namespace
    assert!(!cols[2].sortable); // endpoints
    assert!(cols[3].sortable); // age
}

// ---------------------------------------------------------------------------
// T071: ResourceListState operations for network resources
// ---------------------------------------------------------------------------

#[test]
fn test_resource_list_state_service() {
    let mut state = ResourceListState::new("Service", "v1");

    assert_eq!(state.kind, "Service");
    assert_eq!(state.api_version, "v1");
    assert_eq!(state.namespace_filter, None);
    assert!(!state.loading);
    assert!(state.error.is_none());

    // Test namespace filtering
    state.set_namespace_filter(Some("default".to_string()));
    assert_eq!(state.namespace_filter, Some("default".to_string()));

    // Test loading state
    state.set_loading(true);
    assert!(state.loading);

    // Test error handling
    state.set_error("Failed to fetch services".to_string());
    assert_eq!(state.error, Some("Failed to fetch services".to_string()));
    state.clear_error();
    assert!(state.error.is_none());
}

#[test]
fn test_resource_list_state_ingress() {
    let mut state = ResourceListState::new("Ingress", "networking.k8s.io/v1");

    assert_eq!(state.kind, "Ingress");
    assert_eq!(state.api_version, "networking.k8s.io/v1");

    // Test selection
    state.select_resource("ingress-uid-123");
    assert_eq!(state.selected_resource_uid, Some("ingress-uid-123".to_string()));
    state.clear_selection();
    assert!(state.selected_resource_uid.is_none());
}

#[test]
fn test_resource_list_state_network_policy() {
    let mut state = ResourceListState::new("NetworkPolicy", "networking.k8s.io/v1");

    assert_eq!(state.kind, "NetworkPolicy");
    assert_eq!(state.api_version, "networking.k8s.io/v1");

    // Test action request
    state.request_action("np-uid-456", QuickAction::EditYaml);
    let pending = state.take_pending_action();
    assert!(pending.is_some());
    let (uid, action) = pending.unwrap();
    assert_eq!(uid, "np-uid-456");
    assert_eq!(action, QuickAction::EditYaml);
}

#[test]
fn test_resource_list_state_endpoints() {
    let state = ResourceListState::new("Endpoints", "v1");

    assert_eq!(state.kind, "Endpoints");
    assert_eq!(state.api_version, "v1");
    assert!(!state.is_workload_kind());
}

// ---------------------------------------------------------------------------
// T071: Quick actions for network resources
// ---------------------------------------------------------------------------

#[test]
fn test_service_actions() {
    let actions = actions_for_kind("Service");

    assert_eq!(actions.len(), 2);
    assert!(actions.contains(&QuickAction::EditYaml));
    assert!(actions.contains(&QuickAction::Delete));
}

#[test]
fn test_ingress_actions() {
    let actions = actions_for_kind("Ingress");

    assert_eq!(actions.len(), 2);
    assert!(actions.contains(&QuickAction::EditYaml));
    assert!(actions.contains(&QuickAction::Delete));
}

#[test]
fn test_network_policy_actions() {
    let actions = actions_for_kind("NetworkPolicy");

    assert_eq!(actions.len(), 2);
    assert!(actions.contains(&QuickAction::EditYaml));
    assert!(actions.contains(&QuickAction::Delete));
}

// ---------------------------------------------------------------------------
// T071: RBAC integration for network resources
// ---------------------------------------------------------------------------

#[test]
fn test_service_api_group() {
    let group = api_group_for_kind("Service");
    assert_eq!(group, ""); // Core API
}

#[test]
fn test_ingress_api_group() {
    let group = api_group_for_kind("Ingress");
    assert_eq!(group, "networking.k8s.io");
}

#[test]
fn test_network_policy_api_group() {
    let group = api_group_for_kind("NetworkPolicy");
    assert_eq!(group, "networking.k8s.io");
}

#[test]
fn test_service_plural() {
    let plural = kind_to_plural("Service");
    assert_eq!(plural, "services");
}

#[test]
fn test_ingress_plural() {
    let plural = kind_to_plural("Ingress");
    assert_eq!(plural, "ingresses");
}

#[test]
fn test_network_policy_plural() {
    let plural = kind_to_plural("NetworkPolicy");
    assert_eq!(plural, "networkpolicies");
}

#[test]
fn test_network_resource_edit_verb() {
    let action = QuickAction::EditYaml;
    let verb = verb_for_action(&action);
    assert_eq!(verb, RbacVerb::Update);
}

#[test]
fn test_network_resource_delete_verb() {
    let action = QuickAction::Delete;
    let verb = verb_for_action(&action);
    assert_eq!(verb, RbacVerb::Delete);
}

#[test]
fn test_service_resource_for_action() {
    let resource = resource_for_action("Service", &QuickAction::Delete);
    assert_eq!(resource, "services");
}

#[test]
fn test_ingress_resource_for_action() {
    let resource = resource_for_action("Ingress", &QuickAction::EditYaml);
    assert_eq!(resource, "ingresses");
}

// ---------------------------------------------------------------------------
// T071: RBAC filtering for network resources
// ---------------------------------------------------------------------------

#[test]
fn test_filtered_actions_service_all_allowed() {
    let state = ResourceListState::new("Service", "v1");
    let mut rbac_cache = RbacCache::default();

    // Grant all permissions
    rbac_cache.record(
        PermissionCheck::new(RbacVerb::Update, "services", "", Some("default".to_string())),
        PermissionResult::allowed(),
    );
    rbac_cache.record(
        PermissionCheck::new(RbacVerb::Delete, "services", "", Some("default".to_string())),
        PermissionResult::allowed(),
    );

    let actions = state.filtered_actions(&rbac_cache, Some("default"));
    assert_eq!(actions.len(), 2);
    assert!(actions.contains(&QuickAction::EditYaml));
    assert!(actions.contains(&QuickAction::Delete));
}

#[test]
fn test_filtered_actions_service_delete_denied() {
    let state = ResourceListState::new("Service", "v1");
    let mut rbac_cache = RbacCache::default();

    // Grant update but deny delete
    rbac_cache.record(
        PermissionCheck::new(RbacVerb::Update, "services", "", Some("default".to_string())),
        PermissionResult::allowed(),
    );
    rbac_cache.record(
        PermissionCheck::new(RbacVerb::Delete, "services", "", Some("default".to_string())),
        PermissionResult::denied_no_reason(),
    );

    let actions = state.filtered_actions(&rbac_cache, Some("default"));
    assert_eq!(actions.len(), 1);
    assert!(actions.contains(&QuickAction::EditYaml));
    assert!(!actions.contains(&QuickAction::Delete));
}

#[test]
fn test_filtered_actions_ingress_all_allowed() {
    let state = ResourceListState::new("Ingress", "networking.k8s.io/v1");
    let mut rbac_cache = RbacCache::default();

    // Grant permissions for ingress in networking.k8s.io group
    rbac_cache.record(
        PermissionCheck::new(
            RbacVerb::Update,
            "ingresses",
            "networking.k8s.io",
            Some("default".to_string()),
        ),
        PermissionResult::allowed(),
    );
    rbac_cache.record(
        PermissionCheck::new(
            RbacVerb::Delete,
            "ingresses",
            "networking.k8s.io",
            Some("default".to_string()),
        ),
        PermissionResult::allowed(),
    );

    let actions = state.filtered_actions(&rbac_cache, Some("default"));
    assert_eq!(actions.len(), 2);
}

#[test]
fn test_filtered_actions_network_policy_update_denied() {
    let state = ResourceListState::new("NetworkPolicy", "networking.k8s.io/v1");
    let mut rbac_cache = RbacCache::default();

    // Deny update but allow delete
    rbac_cache.record(
        PermissionCheck::new(
            RbacVerb::Update,
            "networkpolicies",
            "networking.k8s.io",
            Some("default".to_string()),
        ),
        PermissionResult::denied_no_reason(),
    );
    rbac_cache.record(
        PermissionCheck::new(
            RbacVerb::Delete,
            "networkpolicies",
            "networking.k8s.io",
            Some("default".to_string()),
        ),
        PermissionResult::allowed(),
    );

    let actions = state.filtered_actions(&rbac_cache, Some("default"));
    assert_eq!(actions.len(), 1);
    assert!(!actions.contains(&QuickAction::EditYaml));
    assert!(actions.contains(&QuickAction::Delete));
}

// ---------------------------------------------------------------------------
// T071: Action execution for network resources
// ---------------------------------------------------------------------------

#[test]
fn test_submit_delete_service_requires_confirmation() {
    let mut state = ResourceListState::new("Service", "v1");

    state.submit_action(
        "svc-uid-789",
        "my-service",
        Some("default"),
        "Service",
        QuickAction::Delete,
    );

    assert!(state.has_pending_confirmation());
    let req = state.current_action().unwrap();
    assert_eq!(req.resource_uid, "svc-uid-789");
    assert_eq!(req.resource_name, "my-service");
    assert_eq!(req.resource_namespace, Some("default".to_string()));
    assert_eq!(req.kind, "Service");
}

#[test]
fn test_confirm_delete_ingress() {
    let mut state = ResourceListState::new("Ingress", "networking.k8s.io/v1");

    state.submit_action(
        "ing-uid-123",
        "my-ingress",
        Some("default"),
        "Ingress",
        QuickAction::Delete,
    );

    assert!(state.has_pending_confirmation());
    state.confirm_action();
    assert!(!state.has_pending_confirmation());
}

#[test]
fn test_cancel_delete_network_policy() {
    let mut state = ResourceListState::new("NetworkPolicy", "networking.k8s.io/v1");

    state.submit_action(
        "np-uid-456",
        "my-policy",
        Some("default"),
        "NetworkPolicy",
        QuickAction::Delete,
    );

    assert!(state.has_pending_confirmation());
    state.cancel_action();
    assert!(state.current_action().is_none());
}

#[test]
fn test_complete_action_service() {
    let mut state = ResourceListState::new("Service", "v1");

    state.submit_action(
        "svc-uid-789",
        "my-service",
        Some("default"),
        "Service",
        QuickAction::Delete,
    );

    state.confirm_action();
    state.complete_action("Service deleted successfully");

    let req = state.current_action().unwrap();
    match &req.status {
        baeus_ui::views::resource_list::ActionStatus::Completed { message } => {
            assert_eq!(message, "Service deleted successfully");
        }
        _ => panic!("Expected Completed status"),
    }
}

#[test]
fn test_fail_action_ingress() {
    let mut state = ResourceListState::new("Ingress", "networking.k8s.io/v1");

    state.submit_action(
        "ing-uid-123",
        "my-ingress",
        Some("default"),
        "Ingress",
        QuickAction::EditYaml,
    );

    state.fail_action("Invalid YAML");

    let req = state.current_action().unwrap();
    match &req.status {
        baeus_ui::views::resource_list::ActionStatus::Failed { error } => {
            assert_eq!(error, "Invalid YAML");
        }
        _ => panic!("Expected Failed status"),
    }
}

// ---------------------------------------------------------------------------
// T071: Column width weights verification
// ---------------------------------------------------------------------------

#[test]
fn test_service_column_weights() {
    let cols = columns_for_kind("Service");

    // Verify relative width weights
    assert_eq!(cols[0].width_weight, 2.0); // name
    assert_eq!(cols[1].width_weight, 1.0); // namespace
    assert_eq!(cols[2].width_weight, 1.0); // type
    assert_eq!(cols[3].width_weight, 1.0); // cluster_ip
    assert_eq!(cols[4].width_weight, 1.0); // external_ip
    assert_eq!(cols[5].width_weight, 1.5); // ports
    assert_eq!(cols[6].width_weight, 0.8); // age
}

#[test]
fn test_ingress_column_weights() {
    let cols = columns_for_kind("Ingress");

    assert_eq!(cols[0].width_weight, 2.0); // name
    assert_eq!(cols[1].width_weight, 1.0); // namespace
    assert_eq!(cols[2].width_weight, 2.0); // hosts
    assert_eq!(cols[3].width_weight, 1.0); // address
    assert_eq!(cols[4].width_weight, 0.8); // ports
    assert_eq!(cols[5].width_weight, 0.8); // age
}

#[test]
fn test_network_policy_column_weights() {
    let cols = columns_for_kind("NetworkPolicy");

    assert_eq!(cols[0].width_weight, 2.0); // name
    assert_eq!(cols[1].width_weight, 1.0); // namespace
    assert_eq!(cols[2].width_weight, 2.0); // pod_selector
    assert_eq!(cols[3].width_weight, 0.8); // age
}

#[test]
fn test_endpoints_column_weights() {
    let cols = columns_for_kind("Endpoints");

    assert_eq!(cols[0].width_weight, 2.0); // name
    assert_eq!(cols[1].width_weight, 1.0); // namespace
    assert_eq!(cols[2].width_weight, 3.0); // endpoints
    assert_eq!(cols[3].width_weight, 0.8); // age
}
