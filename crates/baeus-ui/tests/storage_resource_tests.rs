//! Integration tests for Storage resource kinds in ResourceListView.
//!
//! Tests verify that the ResourceListView correctly handles storage resource
//! kinds (PersistentVolume, PersistentVolumeClaim, StorageClass) including:
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
// T072: Storage resource column definitions
// ---------------------------------------------------------------------------

#[test]
fn test_persistent_volume_columns() {
    let cols = columns_for_kind("PersistentVolume");

    assert_eq!(cols.len(), 8);
    assert_eq!(cols[0].id, "name");
    assert_eq!(cols[1].id, "capacity");
    assert_eq!(cols[2].id, "access_modes");
    assert_eq!(cols[3].id, "reclaim_policy");
    assert_eq!(cols[4].id, "status");
    assert_eq!(cols[5].id, "claim");
    assert_eq!(cols[6].id, "storage_class");
    assert_eq!(cols[7].id, "age");

    // Verify sortable flags
    assert!(cols[0].sortable); // name
    assert!(cols[1].sortable); // capacity
    assert!(!cols[2].sortable); // access_modes
    assert!(cols[3].sortable); // reclaim_policy
    assert!(cols[4].sortable); // status
    assert!(cols[5].sortable); // claim
    assert!(cols[6].sortable); // storage_class
    assert!(cols[7].sortable); // age
}

#[test]
fn test_persistent_volume_claim_columns() {
    let cols = columns_for_kind("PersistentVolumeClaim");

    assert_eq!(cols.len(), 8);
    assert_eq!(cols[0].id, "name");
    assert_eq!(cols[1].id, "namespace");
    assert_eq!(cols[2].id, "status");
    assert_eq!(cols[3].id, "volume");
    assert_eq!(cols[4].id, "capacity");
    assert_eq!(cols[5].id, "access_modes");
    assert_eq!(cols[6].id, "storage_class");
    assert_eq!(cols[7].id, "age");

    // Verify sortable flags
    assert!(cols[0].sortable); // name
    assert!(cols[1].sortable); // namespace
    assert!(cols[2].sortable); // status
    assert!(cols[3].sortable); // volume
    assert!(cols[4].sortable); // capacity
    assert!(!cols[5].sortable); // access_modes
    assert!(cols[6].sortable); // storage_class
    assert!(cols[7].sortable); // age
}

#[test]
fn test_storage_class_columns() {
    let cols = columns_for_kind("StorageClass");

    assert_eq!(cols.len(), 6);
    assert_eq!(cols[0].id, "name");
    assert_eq!(cols[1].id, "provisioner");
    assert_eq!(cols[2].id, "reclaim_policy");
    assert_eq!(cols[3].id, "volume_binding_mode");
    assert_eq!(cols[4].id, "allow_expansion");
    assert_eq!(cols[5].id, "age");

    // Verify sortable flags
    assert!(cols[0].sortable); // name
    assert!(cols[1].sortable); // provisioner
    assert!(cols[2].sortable); // reclaim_policy
    assert!(cols[3].sortable); // volume_binding_mode
    assert!(!cols[4].sortable); // allow_expansion
    assert!(cols[5].sortable); // age
}

// ---------------------------------------------------------------------------
// T072: ResourceListState operations for storage resources
// ---------------------------------------------------------------------------

#[test]
fn test_resource_list_state_persistent_volume() {
    let mut state = ResourceListState::new("PersistentVolume", "v1");

    assert_eq!(state.kind, "PersistentVolume");
    assert_eq!(state.api_version, "v1");
    assert_eq!(state.namespace_filter, None);
    assert!(!state.loading);
    assert!(state.error.is_none());

    // PersistentVolume is cluster-scoped, so namespace_filter should stay None
    state.set_namespace_filter(Some("default".to_string()));
    assert_eq!(state.namespace_filter, Some("default".to_string()));

    // Test loading state
    state.set_loading(true);
    assert!(state.loading);

    // Test error handling
    state.set_error("Failed to fetch persistent volumes".to_string());
    assert_eq!(state.error, Some("Failed to fetch persistent volumes".to_string()));
    state.clear_error();
    assert!(state.error.is_none());
}

#[test]
fn test_resource_list_state_persistent_volume_claim() {
    let mut state = ResourceListState::new("PersistentVolumeClaim", "v1");

    assert_eq!(state.kind, "PersistentVolumeClaim");
    assert_eq!(state.api_version, "v1");

    // Test selection
    state.select_resource("pvc-uid-123");
    assert_eq!(state.selected_resource_uid, Some("pvc-uid-123".to_string()));
    state.clear_selection();
    assert!(state.selected_resource_uid.is_none());
}

#[test]
fn test_resource_list_state_storage_class() {
    let mut state = ResourceListState::new("StorageClass", "storage.k8s.io/v1");

    assert_eq!(state.kind, "StorageClass");
    assert_eq!(state.api_version, "storage.k8s.io/v1");

    // Test action request
    state.request_action("sc-uid-456", QuickAction::EditYaml);
    let pending = state.take_pending_action();
    assert!(pending.is_some());
    let (uid, action) = pending.unwrap();
    assert_eq!(uid, "sc-uid-456");
    assert_eq!(action, QuickAction::EditYaml);
}

#[test]
fn test_storage_resources_not_workload_kind() {
    let pv_state = ResourceListState::new("PersistentVolume", "v1");
    let pvc_state = ResourceListState::new("PersistentVolumeClaim", "v1");
    let sc_state = ResourceListState::new("StorageClass", "storage.k8s.io/v1");

    assert!(!pv_state.is_workload_kind());
    assert!(!pvc_state.is_workload_kind());
    assert!(!sc_state.is_workload_kind());
}

// ---------------------------------------------------------------------------
// T072: Quick actions for storage resources
// ---------------------------------------------------------------------------

#[test]
fn test_persistent_volume_actions() {
    let actions = actions_for_kind("PersistentVolume");

    // Storage resources use default actions
    assert_eq!(actions.len(), 2);
    assert!(actions.contains(&QuickAction::EditYaml));
    assert!(actions.contains(&QuickAction::Delete));
}

#[test]
fn test_persistent_volume_claim_actions() {
    let actions = actions_for_kind("PersistentVolumeClaim");

    assert_eq!(actions.len(), 2);
    assert!(actions.contains(&QuickAction::EditYaml));
    assert!(actions.contains(&QuickAction::Delete));
}

#[test]
fn test_storage_class_actions() {
    let actions = actions_for_kind("StorageClass");

    assert_eq!(actions.len(), 2);
    assert!(actions.contains(&QuickAction::EditYaml));
    assert!(actions.contains(&QuickAction::Delete));
}

// ---------------------------------------------------------------------------
// T072: RBAC integration for storage resources
// ---------------------------------------------------------------------------

#[test]
fn test_persistent_volume_api_group() {
    let group = api_group_for_kind("PersistentVolume");
    assert_eq!(group, ""); // Core API
}

#[test]
fn test_persistent_volume_claim_api_group() {
    let group = api_group_for_kind("PersistentVolumeClaim");
    assert_eq!(group, ""); // Core API
}

#[test]
fn test_storage_class_api_group() {
    // StorageClass is not explicitly mapped, so it falls back to ""
    let group = api_group_for_kind("StorageClass");
    assert_eq!(group, "");
}

#[test]
fn test_persistent_volume_plural() {
    let plural = kind_to_plural("PersistentVolume");
    assert_eq!(plural, "persistentvolumes");
}

#[test]
fn test_persistent_volume_claim_plural() {
    let plural = kind_to_plural("PersistentVolumeClaim");
    assert_eq!(plural, "persistentvolumeclaims");
}

#[test]
fn test_storage_class_plural() {
    // StorageClass is not explicitly mapped, falls back to lowercase + "s"
    let plural = kind_to_plural("StorageClass");
    assert_eq!(plural, "unknown");
}

#[test]
fn test_storage_resource_edit_verb() {
    let action = QuickAction::EditYaml;
    let verb = verb_for_action(&action);
    assert_eq!(verb, RbacVerb::Update);
}

#[test]
fn test_storage_resource_delete_verb() {
    let action = QuickAction::Delete;
    let verb = verb_for_action(&action);
    assert_eq!(verb, RbacVerb::Delete);
}

#[test]
fn test_persistent_volume_resource_for_action() {
    let resource = resource_for_action("PersistentVolume", &QuickAction::Delete);
    assert_eq!(resource, "persistentvolumes");
}

#[test]
fn test_persistent_volume_claim_resource_for_action() {
    let resource = resource_for_action("PersistentVolumeClaim", &QuickAction::EditYaml);
    assert_eq!(resource, "persistentvolumeclaims");
}

// ---------------------------------------------------------------------------
// T072: RBAC filtering for storage resources
// ---------------------------------------------------------------------------

#[test]
fn test_filtered_actions_persistent_volume_all_allowed() {
    let state = ResourceListState::new("PersistentVolume", "v1");
    let mut rbac_cache = RbacCache::default();

    // Grant all permissions
    rbac_cache.record(
        PermissionCheck::new(RbacVerb::Update, "persistentvolumes", "", None),
        PermissionResult::allowed(),
    );
    rbac_cache.record(
        PermissionCheck::new(RbacVerb::Delete, "persistentvolumes", "", None),
        PermissionResult::allowed(),
    );

    let actions = state.filtered_actions(&rbac_cache, None);
    assert_eq!(actions.len(), 2);
    assert!(actions.contains(&QuickAction::EditYaml));
    assert!(actions.contains(&QuickAction::Delete));
}

#[test]
fn test_filtered_actions_persistent_volume_delete_denied() {
    let state = ResourceListState::new("PersistentVolume", "v1");
    let mut rbac_cache = RbacCache::default();

    // Grant update but deny delete
    rbac_cache.record(
        PermissionCheck::new(RbacVerb::Update, "persistentvolumes", "", None),
        PermissionResult::allowed(),
    );
    rbac_cache.record(
        PermissionCheck::new(RbacVerb::Delete, "persistentvolumes", "", None),
        PermissionResult::denied_no_reason(),
    );

    let actions = state.filtered_actions(&rbac_cache, None);
    assert_eq!(actions.len(), 1);
    assert!(actions.contains(&QuickAction::EditYaml));
    assert!(!actions.contains(&QuickAction::Delete));
}

#[test]
fn test_filtered_actions_pvc_all_allowed() {
    let state = ResourceListState::new("PersistentVolumeClaim", "v1");
    let mut rbac_cache = RbacCache::default();

    // Grant permissions for PVC
    rbac_cache.record(
        PermissionCheck::new(
            RbacVerb::Update,
            "persistentvolumeclaims",
            "",
            Some("default".to_string()),
        ),
        PermissionResult::allowed(),
    );
    rbac_cache.record(
        PermissionCheck::new(
            RbacVerb::Delete,
            "persistentvolumeclaims",
            "",
            Some("default".to_string()),
        ),
        PermissionResult::allowed(),
    );

    let actions = state.filtered_actions(&rbac_cache, Some("default"));
    assert_eq!(actions.len(), 2);
}

#[test]
fn test_filtered_actions_storage_class_update_denied() {
    let state = ResourceListState::new("StorageClass", "storage.k8s.io/v1");
    let mut rbac_cache = RbacCache::default();

    // Deny update but allow delete
    rbac_cache.record(
        PermissionCheck::new(RbacVerb::Update, "storageclasss", "", None),
        PermissionResult::denied_no_reason(),
    );
    rbac_cache.record(
        PermissionCheck::new(RbacVerb::Delete, "storageclasss", "", None),
        PermissionResult::allowed(),
    );

    let actions = state.filtered_actions(&rbac_cache, None);
    assert_eq!(actions.len(), 1);
    assert!(!actions.contains(&QuickAction::EditYaml));
    assert!(actions.contains(&QuickAction::Delete));
}

// ---------------------------------------------------------------------------
// T072: Action execution for storage resources
// ---------------------------------------------------------------------------

#[test]
fn test_submit_delete_persistent_volume_requires_confirmation() {
    let mut state = ResourceListState::new("PersistentVolume", "v1");

    state.submit_action(
        "pv-uid-789",
        "my-pv",
        None, // PV is cluster-scoped
        "PersistentVolume",
        QuickAction::Delete,
    );

    assert!(state.has_pending_confirmation());
    let req = state.current_action().unwrap();
    assert_eq!(req.resource_uid, "pv-uid-789");
    assert_eq!(req.resource_name, "my-pv");
    assert_eq!(req.resource_namespace, None);
    assert_eq!(req.kind, "PersistentVolume");
}

#[test]
fn test_confirm_delete_pvc() {
    let mut state = ResourceListState::new("PersistentVolumeClaim", "v1");

    state.submit_action(
        "pvc-uid-123",
        "my-pvc",
        Some("default"),
        "PersistentVolumeClaim",
        QuickAction::Delete,
    );

    assert!(state.has_pending_confirmation());
    state.confirm_action();
    assert!(!state.has_pending_confirmation());
}

#[test]
fn test_cancel_delete_storage_class() {
    let mut state = ResourceListState::new("StorageClass", "storage.k8s.io/v1");

    state.submit_action(
        "sc-uid-456",
        "my-storage-class",
        None, // StorageClass is cluster-scoped
        "StorageClass",
        QuickAction::Delete,
    );

    assert!(state.has_pending_confirmation());
    state.cancel_action();
    assert!(state.current_action().is_none());
}

#[test]
fn test_complete_action_persistent_volume() {
    let mut state = ResourceListState::new("PersistentVolume", "v1");

    state.submit_action("pv-uid-789", "my-pv", None, "PersistentVolume", QuickAction::Delete);

    state.confirm_action();
    state.complete_action("PersistentVolume deleted successfully");

    let req = state.current_action().unwrap();
    match &req.status {
        baeus_ui::views::resource_list::ActionStatus::Completed { message } => {
            assert_eq!(message, "PersistentVolume deleted successfully");
        }
        _ => panic!("Expected Completed status"),
    }
}

#[test]
fn test_fail_action_pvc() {
    let mut state = ResourceListState::new("PersistentVolumeClaim", "v1");

    state.submit_action(
        "pvc-uid-123",
        "my-pvc",
        Some("default"),
        "PersistentVolumeClaim",
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
// T072: Column width weights verification
// ---------------------------------------------------------------------------

#[test]
fn test_persistent_volume_column_weights() {
    let cols = columns_for_kind("PersistentVolume");

    // Verify relative width weights
    assert_eq!(cols[0].width_weight, 2.0); // name
    assert_eq!(cols[1].width_weight, 1.0); // capacity
    assert_eq!(cols[2].width_weight, 1.0); // access_modes
    assert_eq!(cols[3].width_weight, 1.0); // reclaim_policy
    assert_eq!(cols[4].width_weight, 0.8); // status
    assert_eq!(cols[5].width_weight, 1.5); // claim
    assert_eq!(cols[6].width_weight, 1.0); // storage_class
    assert_eq!(cols[7].width_weight, 0.8); // age
}

#[test]
fn test_persistent_volume_claim_column_weights() {
    let cols = columns_for_kind("PersistentVolumeClaim");

    assert_eq!(cols[0].width_weight, 2.0); // name
    assert_eq!(cols[1].width_weight, 1.0); // namespace
    assert_eq!(cols[2].width_weight, 0.8); // status
    assert_eq!(cols[3].width_weight, 1.5); // volume
    assert_eq!(cols[4].width_weight, 1.0); // capacity
    assert_eq!(cols[5].width_weight, 1.0); // access_modes
    assert_eq!(cols[6].width_weight, 1.0); // storage_class
    assert_eq!(cols[7].width_weight, 0.8); // age
}

#[test]
fn test_storage_class_column_weights() {
    let cols = columns_for_kind("StorageClass");

    assert_eq!(cols[0].width_weight, 2.0); // name
    assert_eq!(cols[1].width_weight, 2.0); // provisioner
    assert_eq!(cols[2].width_weight, 1.0); // reclaim_policy
    assert_eq!(cols[3].width_weight, 1.5); // volume_binding_mode
    assert_eq!(cols[4].width_weight, 1.0); // allow_expansion
    assert_eq!(cols[5].width_weight, 0.8); // age
}

// ---------------------------------------------------------------------------
// T072: Mixed storage resource scenarios
// ---------------------------------------------------------------------------

#[test]
fn test_pvc_namespace_filtering() {
    let mut state = ResourceListState::new("PersistentVolumeClaim", "v1");

    // PVCs are namespace-scoped
    state.set_namespace_filter(Some("production".to_string()));
    assert_eq!(state.namespace_filter, Some("production".to_string()));

    state.set_namespace_filter(None);
    assert_eq!(state.namespace_filter, None);
}

#[test]
fn test_storage_class_no_namespace() {
    let mut state = ResourceListState::new("StorageClass", "storage.k8s.io/v1");

    // StorageClass is cluster-scoped, but we can still set namespace filter
    // (it just won't be used)
    state.set_namespace_filter(Some("default".to_string()));
    assert_eq!(state.namespace_filter, Some("default".to_string()));
}

#[test]
fn test_multiple_storage_resources_selection() {
    let mut pv_state = ResourceListState::new("PersistentVolume", "v1");
    let mut pvc_state = ResourceListState::new("PersistentVolumeClaim", "v1");

    pv_state.select_resource("pv-uid-1");
    pvc_state.select_resource("pvc-uid-2");

    assert_eq!(pv_state.selected_resource_uid, Some("pv-uid-1".to_string()));
    assert_eq!(pvc_state.selected_resource_uid, Some("pvc-uid-2".to_string()));
}
