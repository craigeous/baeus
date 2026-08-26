use baeus_core::rbac::{PermissionCheck, PermissionResult, RbacCache, RbacVerb};
use baeus_ui::components::resource_table::columns_for_kind as table_columns_for_kind;
use baeus_ui::theme::{Color, Theme};
use baeus_ui::views::resource_list::*;

// --- QuickAction tests ---

#[test]
fn test_quick_action_labels() {
    assert_eq!(QuickAction::Scale { current_replicas: 1, desired_replicas: 3 }.label(), "Scale");
    assert_eq!(QuickAction::Restart.label(), "Restart");
    assert_eq!(QuickAction::Delete.label(), "Delete");
    assert_eq!(QuickAction::Cordon.label(), "Cordon");
    assert_eq!(QuickAction::Uncordon.label(), "Uncordon");
    assert_eq!(QuickAction::ViewLogs.label(), "View Logs");
    assert_eq!(QuickAction::Exec.label(), "Exec");
    assert_eq!(QuickAction::EditYaml.label(), "Edit YAML");
}

#[test]
fn test_is_destructive() {
    assert!(QuickAction::Delete.is_destructive());
    assert!(!QuickAction::Restart.is_destructive());
    assert!(!QuickAction::Scale { current_replicas: 0, desired_replicas: 0 }.is_destructive());
    assert!(!QuickAction::Cordon.is_destructive());
    assert!(!QuickAction::Uncordon.is_destructive());
    assert!(!QuickAction::ViewLogs.is_destructive());
    assert!(!QuickAction::Exec.is_destructive());
    assert!(!QuickAction::EditYaml.is_destructive());
}

#[test]
fn test_requires_confirmation() {
    assert!(
        QuickAction::Scale { current_replicas: 1, desired_replicas: 3 }.requires_confirmation()
    );
    assert!(QuickAction::Restart.requires_confirmation());
    assert!(QuickAction::Delete.requires_confirmation());
    assert!(QuickAction::Cordon.requires_confirmation());
    assert!(QuickAction::Uncordon.requires_confirmation());
    assert!(!QuickAction::ViewLogs.requires_confirmation());
    assert!(!QuickAction::Exec.requires_confirmation());
    assert!(!QuickAction::EditYaml.requires_confirmation());
}

// --- actions_for_kind tests ---

#[test]
fn test_actions_for_pod() {
    let actions = actions_for_kind("Pod");
    assert_eq!(actions.len(), 3);
    assert_eq!(actions[0], QuickAction::ViewLogs);
    assert_eq!(actions[1], QuickAction::Exec);
    assert_eq!(actions[2], QuickAction::Delete);
}

#[test]
fn test_actions_for_deployment() {
    let actions = actions_for_kind("Deployment");
    assert_eq!(actions.len(), 4);
    assert_eq!(actions[0], QuickAction::Scale { current_replicas: 0, desired_replicas: 0 });
    assert_eq!(actions[1], QuickAction::Restart);
    assert_eq!(actions[2], QuickAction::EditYaml);
    assert_eq!(actions[3], QuickAction::Delete);
}

#[test]
fn test_actions_for_statefulset() {
    let actions = actions_for_kind("StatefulSet");
    assert_eq!(actions.len(), 4);
    assert_eq!(actions[0], QuickAction::Scale { current_replicas: 0, desired_replicas: 0 });
    assert_eq!(actions[1], QuickAction::Restart);
}

#[test]
fn test_actions_for_daemonset() {
    let actions = actions_for_kind("DaemonSet");
    assert_eq!(actions.len(), 3);
    assert_eq!(actions[0], QuickAction::Restart);
    assert_eq!(actions[1], QuickAction::EditYaml);
    assert_eq!(actions[2], QuickAction::Delete);
}

#[test]
fn test_actions_for_replicaset() {
    let actions = actions_for_kind("ReplicaSet");
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0], QuickAction::Delete);
}

#[test]
fn test_actions_for_job() {
    let actions = actions_for_kind("Job");
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0], QuickAction::Delete);
}

#[test]
fn test_actions_for_cronjob() {
    let actions = actions_for_kind("CronJob");
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0], QuickAction::EditYaml);
    assert_eq!(actions[1], QuickAction::Delete);
}

#[test]
fn test_actions_for_node() {
    let actions = actions_for_kind("Node");
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0], QuickAction::Cordon);
    assert_eq!(actions[1], QuickAction::Uncordon);
}

#[test]
fn test_actions_for_service() {
    let actions = actions_for_kind("Service");
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0], QuickAction::EditYaml);
    assert_eq!(actions[1], QuickAction::Delete);
}

#[test]
fn test_actions_for_ingress() {
    let actions = actions_for_kind("Ingress");
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0], QuickAction::EditYaml);
    assert_eq!(actions[1], QuickAction::Delete);
}

#[test]
fn test_actions_for_configmap() {
    let actions = actions_for_kind("ConfigMap");
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0], QuickAction::EditYaml);
    assert_eq!(actions[1], QuickAction::Delete);
}

#[test]
fn test_actions_for_secret() {
    let actions = actions_for_kind("Secret");
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0], QuickAction::EditYaml);
    assert_eq!(actions[1], QuickAction::Delete);
}

#[test]
fn test_actions_for_network_policy() {
    let actions = actions_for_kind("NetworkPolicy");
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0], QuickAction::EditYaml);
    assert_eq!(actions[1], QuickAction::Delete);
}

#[test]
fn test_actions_for_unknown_kind() {
    let actions = actions_for_kind("CustomResource");
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0], QuickAction::EditYaml);
    assert_eq!(actions[1], QuickAction::Delete);
}

// --- ResourceListState tests ---

#[test]
fn test_new() {
    let state = ResourceListState::new("Deployment", "apps/v1");
    assert_eq!(state.kind, "Deployment");
    assert_eq!(state.api_version, "apps/v1");
    assert!(state.namespace_filter.is_none());
    assert!(!state.loading);
    assert!(state.error.is_none());
    assert!(state.selected_resource_uid.is_none());
    assert!(state.pending_action.is_none());
    assert!(state.current_action_request.is_none());
    assert!(state.show_create_dialog.is_none());
}

#[test]
fn test_set_namespace_filter() {
    let mut state = ResourceListState::new("Pod", "v1");
    assert!(state.namespace_filter.is_none());

    state.set_namespace_filter(Some("kube-system".to_string()));
    assert_eq!(state.namespace_filter.as_deref(), Some("kube-system"));

    state.set_namespace_filter(None);
    assert!(state.namespace_filter.is_none());
}

#[test]
fn test_set_loading() {
    let mut state = ResourceListState::new("Pod", "v1");
    assert!(!state.loading);

    state.set_loading(true);
    assert!(state.loading);

    state.set_loading(false);
    assert!(!state.loading);
}

#[test]
fn test_set_error_and_clear() {
    let mut state = ResourceListState::new("Pod", "v1");
    assert!(state.error.is_none());

    state.set_error("connection refused".to_string());
    assert_eq!(state.error.as_deref(), Some("connection refused"));

    state.clear_error();
    assert!(state.error.is_none());
}

#[test]
fn test_select_resource_and_clear() {
    let mut state = ResourceListState::new("Pod", "v1");
    assert!(state.selected_resource_uid.is_none());

    state.select_resource("uid-123");
    assert_eq!(state.selected_resource_uid.as_deref(), Some("uid-123"));

    state.clear_selection();
    assert!(state.selected_resource_uid.is_none());
}

#[test]
fn test_request_action_and_take() {
    let mut state = ResourceListState::new("Deployment", "apps/v1");
    assert!(state.pending_action.is_none());

    state.request_action("uid-456", QuickAction::Restart);
    assert!(state.pending_action.is_some());

    let action = state.take_pending_action();
    assert!(action.is_some());
    let (uid, act) = action.unwrap();
    assert_eq!(uid, "uid-456");
    assert_eq!(act, QuickAction::Restart);

    // After take, pending should be None
    assert!(state.pending_action.is_none());
    assert!(state.take_pending_action().is_none());
}

#[test]
fn test_request_action_overwrite() {
    let mut state = ResourceListState::new("Pod", "v1");
    state.request_action("uid-1", QuickAction::Delete);
    state.request_action("uid-2", QuickAction::ViewLogs);

    let (uid, act) = state.take_pending_action().unwrap();
    assert_eq!(uid, "uid-2");
    assert_eq!(act, QuickAction::ViewLogs);
}

#[test]
fn test_available_actions() {
    let state = ResourceListState::new("Pod", "v1");
    let actions = state.available_actions();
    assert_eq!(actions.len(), 3);
    assert_eq!(actions[0], QuickAction::ViewLogs);

    let state = ResourceListState::new("Node", "v1");
    let actions = state.available_actions();
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0], QuickAction::Cordon);
}

#[test]
fn test_is_workload_kind() {
    assert!(ResourceListState::new("Pod", "v1").is_workload_kind());
    assert!(ResourceListState::new("Deployment", "apps/v1").is_workload_kind());
    assert!(ResourceListState::new("StatefulSet", "apps/v1").is_workload_kind());
    assert!(ResourceListState::new("DaemonSet", "apps/v1").is_workload_kind());
    assert!(ResourceListState::new("ReplicaSet", "apps/v1").is_workload_kind());
    assert!(ResourceListState::new("Job", "batch/v1").is_workload_kind());
    assert!(ResourceListState::new("CronJob", "batch/v1").is_workload_kind());

    assert!(!ResourceListState::new("Service", "v1").is_workload_kind());
    assert!(!ResourceListState::new("ConfigMap", "v1").is_workload_kind());
    assert!(!ResourceListState::new("Node", "v1").is_workload_kind());
    assert!(!ResourceListState::new("Ingress", "networking.k8s.io/v1").is_workload_kind());
    assert!(!ResourceListState::new("CustomThing", "example.com/v1").is_workload_kind());
}

#[test]
fn test_quick_action_serialization() {
    let action = QuickAction::Scale { current_replicas: 3, desired_replicas: 5 };
    let json = serde_json::to_string(&action).unwrap();
    let deserialized: QuickAction = serde_json::from_str(&json).unwrap();
    assert_eq!(action, deserialized);
}

#[test]
fn test_quick_action_serialization_simple_variant() {
    let action = QuickAction::Restart;
    let json = serde_json::to_string(&action).unwrap();
    let deserialized: QuickAction = serde_json::from_str(&json).unwrap();
    assert_eq!(action, deserialized);
}

#[test]
fn test_full_workflow() {
    let mut state = ResourceListState::new("Deployment", "apps/v1");

    // Start loading
    state.set_loading(true);
    assert!(state.loading);

    // Finish loading
    state.set_loading(false);
    assert!(!state.loading);

    // Filter to a namespace
    state.set_namespace_filter(Some("production".to_string()));
    assert_eq!(state.namespace_filter.as_deref(), Some("production"));

    // Select a resource
    state.select_resource("deploy-uid-001");
    assert_eq!(state.selected_resource_uid.as_deref(), Some("deploy-uid-001"));

    // Request a scale action
    state.request_action(
        "deploy-uid-001",
        QuickAction::Scale { current_replicas: 2, desired_replicas: 5 },
    );

    // Take and verify the pending action
    let (uid, action) = state.take_pending_action().unwrap();
    assert_eq!(uid, "deploy-uid-001");
    assert_eq!(action, QuickAction::Scale { current_replicas: 2, desired_replicas: 5 });
    assert!(action.requires_confirmation());
    assert!(!action.is_destructive());
}

#[test]
fn test_error_workflow() {
    let mut state = ResourceListState::new("Pod", "v1");

    state.set_loading(true);
    state.set_error("forbidden: access denied".to_string());
    assert_eq!(state.error.as_deref(), Some("forbidden: access denied"));

    state.clear_error();
    assert!(state.error.is_none());
}

// ===================================================================
// T062: Action Execution Tests
// ===================================================================

#[test]
fn test_action_status_pending_confirmation() {
    let status = ActionStatus::PendingConfirmation;
    assert_eq!(status, ActionStatus::PendingConfirmation);
}

#[test]
fn test_action_status_executing() {
    let status = ActionStatus::Executing;
    assert_eq!(status, ActionStatus::Executing);
}

#[test]
fn test_action_status_completed() {
    let status = ActionStatus::Completed { message: "Deleted successfully".to_string() };
    assert_eq!(status, ActionStatus::Completed { message: "Deleted successfully".to_string() });
}

#[test]
fn test_action_status_failed() {
    let status = ActionStatus::Failed { error: "forbidden".to_string() };
    assert_eq!(status, ActionStatus::Failed { error: "forbidden".to_string() });
}

#[test]
fn test_submit_action_destructive_requires_confirmation() {
    let mut state = ResourceListState::new("Pod", "v1");
    let req = state.submit_action("uid-1", "my-pod", Some("default"), "Pod", QuickAction::Delete);

    assert_eq!(req.resource_uid, "uid-1");
    assert_eq!(req.resource_name, "my-pod");
    assert_eq!(req.resource_namespace.as_deref(), Some("default"));
    assert_eq!(req.kind, "Pod");
    assert_eq!(req.action, QuickAction::Delete);
    assert_eq!(req.status, ActionStatus::PendingConfirmation);
}

#[test]
fn test_submit_action_restart_requires_confirmation() {
    let mut state = ResourceListState::new("Deployment", "apps/v1");
    let req = state.submit_action(
        "uid-2",
        "my-deploy",
        Some("default"),
        "Deployment",
        QuickAction::Restart,
    );
    assert_eq!(req.status, ActionStatus::PendingConfirmation);
}

#[test]
fn test_submit_action_scale_requires_confirmation() {
    let mut state = ResourceListState::new("Deployment", "apps/v1");
    let req = state.submit_action(
        "uid-3",
        "my-deploy",
        Some("default"),
        "Deployment",
        QuickAction::Scale { current_replicas: 1, desired_replicas: 5 },
    );
    assert_eq!(req.status, ActionStatus::PendingConfirmation);
}

#[test]
fn test_submit_action_non_confirmation_starts_executing() {
    let mut state = ResourceListState::new("Pod", "v1");
    let req = state.submit_action("uid-4", "my-pod", Some("default"), "Pod", QuickAction::ViewLogs);
    assert_eq!(req.status, ActionStatus::Executing);
}

#[test]
fn test_submit_action_exec_starts_executing() {
    let mut state = ResourceListState::new("Pod", "v1");
    let req = state.submit_action("uid-5", "my-pod", Some("default"), "Pod", QuickAction::Exec);
    assert_eq!(req.status, ActionStatus::Executing);
}

#[test]
fn test_submit_action_edit_yaml_starts_executing() {
    let mut state = ResourceListState::new("Deployment", "apps/v1");
    let req = state.submit_action(
        "uid-6",
        "my-deploy",
        Some("default"),
        "Deployment",
        QuickAction::EditYaml,
    );
    assert_eq!(req.status, ActionStatus::Executing);
}

#[test]
fn test_submit_action_no_namespace() {
    let mut state = ResourceListState::new("Node", "v1");
    let req = state.submit_action("uid-7", "node-1", None, "Node", QuickAction::Cordon);
    assert!(req.resource_namespace.is_none());
    assert_eq!(req.status, ActionStatus::PendingConfirmation);
}

#[test]
fn test_confirm_action() {
    let mut state = ResourceListState::new("Pod", "v1");
    state.submit_action("uid-1", "my-pod", Some("default"), "Pod", QuickAction::Delete);
    assert!(state.has_pending_confirmation());

    state.confirm_action();

    let req = state.current_action().unwrap();
    assert_eq!(req.status, ActionStatus::Executing);
    assert!(!state.has_pending_confirmation());
}

#[test]
fn test_confirm_action_only_moves_from_pending() {
    let mut state = ResourceListState::new("Pod", "v1");
    // Submit non-confirmation action (ViewLogs -> starts Executing)
    state.submit_action("uid-1", "my-pod", Some("default"), "Pod", QuickAction::ViewLogs);
    assert!(!state.has_pending_confirmation());

    // confirm_action should not change Executing to something else
    state.confirm_action();
    let req = state.current_action().unwrap();
    assert_eq!(req.status, ActionStatus::Executing);
}

#[test]
fn test_cancel_action() {
    let mut state = ResourceListState::new("Pod", "v1");
    state.submit_action("uid-1", "my-pod", Some("default"), "Pod", QuickAction::Delete);
    assert!(state.current_action().is_some());

    state.cancel_action();
    assert!(state.current_action().is_none());
    assert!(!state.has_pending_confirmation());
}

#[test]
fn test_complete_action() {
    let mut state = ResourceListState::new("Pod", "v1");
    state.submit_action("uid-1", "my-pod", Some("default"), "Pod", QuickAction::ViewLogs);

    state.complete_action("Logs streaming started");

    let req = state.current_action().unwrap();
    assert_eq!(
        req.status,
        ActionStatus::Completed { message: "Logs streaming started".to_string() }
    );
}

#[test]
fn test_fail_action() {
    let mut state = ResourceListState::new("Pod", "v1");
    state.submit_action("uid-1", "my-pod", Some("default"), "Pod", QuickAction::Delete);
    state.confirm_action();

    state.fail_action("RBAC: forbidden");

    let req = state.current_action().unwrap();
    assert_eq!(req.status, ActionStatus::Failed { error: "RBAC: forbidden".to_string() });
}

#[test]
fn test_current_action_none_initially() {
    let state = ResourceListState::new("Pod", "v1");
    assert!(state.current_action().is_none());
}

#[test]
fn test_has_pending_confirmation_false_initially() {
    let state = ResourceListState::new("Pod", "v1");
    assert!(!state.has_pending_confirmation());
}

#[test]
fn test_has_pending_confirmation_false_after_confirm() {
    let mut state = ResourceListState::new("Pod", "v1");
    state.submit_action("uid-1", "my-pod", Some("default"), "Pod", QuickAction::Delete);
    assert!(state.has_pending_confirmation());
    state.confirm_action();
    assert!(!state.has_pending_confirmation());
}

#[test]
fn test_submit_action_replaces_previous() {
    let mut state = ResourceListState::new("Pod", "v1");
    state.submit_action("uid-1", "pod-a", Some("default"), "Pod", QuickAction::Delete);
    state.submit_action("uid-2", "pod-b", Some("default"), "Pod", QuickAction::ViewLogs);

    let req = state.current_action().unwrap();
    assert_eq!(req.resource_uid, "uid-2");
    assert_eq!(req.resource_name, "pod-b");
    assert_eq!(req.action, QuickAction::ViewLogs);
}

#[test]
fn test_full_action_execution_lifecycle() {
    let mut state = ResourceListState::new("Deployment", "apps/v1");

    // 1. Submit a destructive action
    state.submit_action(
        "uid-deploy-1",
        "nginx-deploy",
        Some("production"),
        "Deployment",
        QuickAction::Delete,
    );
    assert!(state.has_pending_confirmation());
    assert_eq!(state.current_action().unwrap().status, ActionStatus::PendingConfirmation);

    // 2. Confirm it
    state.confirm_action();
    assert!(!state.has_pending_confirmation());
    assert_eq!(state.current_action().unwrap().status, ActionStatus::Executing);

    // 3. Complete it
    state.complete_action("Deployment nginx-deploy deleted");
    assert_eq!(
        state.current_action().unwrap().status,
        ActionStatus::Completed { message: "Deployment nginx-deploy deleted".to_string() }
    );

    // 4. Clear it
    state.cancel_action();
    assert!(state.current_action().is_none());
}

#[test]
fn test_action_execution_failure_lifecycle() {
    let mut state = ResourceListState::new("Pod", "v1");

    state.submit_action("uid-1", "crash-pod", Some("default"), "Pod", QuickAction::Delete);
    state.confirm_action();
    state.fail_action("API server returned 500");

    let req = state.current_action().unwrap();
    assert_eq!(req.status, ActionStatus::Failed { error: "API server returned 500".to_string() });
}

// ===================================================================
// T064: RBAC Integration Tests
// ===================================================================

#[test]
fn test_kind_to_plural_known_kinds() {
    assert_eq!(kind_to_plural("Pod"), "pods");
    assert_eq!(kind_to_plural("Deployment"), "deployments");
    assert_eq!(kind_to_plural("StatefulSet"), "statefulsets");
    assert_eq!(kind_to_plural("DaemonSet"), "daemonsets");
    assert_eq!(kind_to_plural("ReplicaSet"), "replicasets");
    assert_eq!(kind_to_plural("Job"), "jobs");
    assert_eq!(kind_to_plural("CronJob"), "cronjobs");
    assert_eq!(kind_to_plural("Node"), "nodes");
    assert_eq!(kind_to_plural("Service"), "services");
    assert_eq!(kind_to_plural("Ingress"), "ingresses");
    assert_eq!(kind_to_plural("ConfigMap"), "configmaps");
    assert_eq!(kind_to_plural("Secret"), "secrets");
    assert_eq!(kind_to_plural("NetworkPolicy"), "networkpolicies");
    assert_eq!(kind_to_plural("Namespace"), "namespaces");
    assert_eq!(kind_to_plural("ServiceAccount"), "serviceaccounts");
    assert_eq!(kind_to_plural("PersistentVolumeClaim"), "persistentvolumeclaims");
    assert_eq!(kind_to_plural("PersistentVolume"), "persistentvolumes");
}

#[test]
fn test_kind_to_plural_unknown_returns_unknown() {
    assert_eq!(kind_to_plural("MyCustomThing"), "unknown");
}

// NOTE: `kind_to_plural_owned` is a private function and cannot be tested
// directly from integration tests. Its behavior for known kinds is covered via
// `kind_to_plural`, and its unknown-kind fallback (lowercase + "s") is
// exercised indirectly through `resource_for_action` with an unknown kind
// (see `test_resource_for_action_unknown_kind`).

#[test]
fn test_verb_for_action() {
    assert_eq!(verb_for_action(&QuickAction::Delete), RbacVerb::Delete);
    assert_eq!(
        verb_for_action(&QuickAction::Scale { current_replicas: 0, desired_replicas: 0 }),
        RbacVerb::Update
    );
    assert_eq!(verb_for_action(&QuickAction::Restart), RbacVerb::Update);
    assert_eq!(verb_for_action(&QuickAction::Cordon), RbacVerb::Patch);
    assert_eq!(verb_for_action(&QuickAction::Uncordon), RbacVerb::Patch);
    assert_eq!(verb_for_action(&QuickAction::EditYaml), RbacVerb::Update);
    assert_eq!(verb_for_action(&QuickAction::ViewLogs), RbacVerb::Get);
    assert_eq!(verb_for_action(&QuickAction::Exec), RbacVerb::Create);
}

#[test]
fn test_resource_for_action_regular() {
    assert_eq!(resource_for_action("Pod", &QuickAction::Delete), "pods");
    assert_eq!(resource_for_action("Deployment", &QuickAction::Restart), "deployments");
    assert_eq!(
        resource_for_action(
            "Deployment",
            &QuickAction::Scale { current_replicas: 0, desired_replicas: 0 }
        ),
        "deployments"
    );
    assert_eq!(resource_for_action("Deployment", &QuickAction::EditYaml), "deployments");
}

#[test]
fn test_resource_for_action_subresources() {
    assert_eq!(resource_for_action("Pod", &QuickAction::ViewLogs), "pods/log");
    assert_eq!(resource_for_action("Pod", &QuickAction::Exec), "pods/exec");
}

#[test]
fn test_resource_for_action_cordon_uncordon() {
    assert_eq!(resource_for_action("Node", &QuickAction::Cordon), "nodes");
    assert_eq!(resource_for_action("Node", &QuickAction::Uncordon), "nodes");
}

#[test]
fn test_resource_for_action_unknown_kind() {
    assert_eq!(resource_for_action("Widget", &QuickAction::Delete), "widgets");
}

#[test]
fn test_api_group_for_kind_core() {
    assert_eq!(api_group_for_kind("Pod"), "");
    assert_eq!(api_group_for_kind("Service"), "");
    assert_eq!(api_group_for_kind("ConfigMap"), "");
    assert_eq!(api_group_for_kind("Secret"), "");
    assert_eq!(api_group_for_kind("Namespace"), "");
    assert_eq!(api_group_for_kind("Node"), "");
    assert_eq!(api_group_for_kind("PersistentVolume"), "");
    assert_eq!(api_group_for_kind("PersistentVolumeClaim"), "");
    assert_eq!(api_group_for_kind("ServiceAccount"), "");
}

#[test]
fn test_api_group_for_kind_apps() {
    assert_eq!(api_group_for_kind("Deployment"), "apps");
    assert_eq!(api_group_for_kind("StatefulSet"), "apps");
    assert_eq!(api_group_for_kind("DaemonSet"), "apps");
    assert_eq!(api_group_for_kind("ReplicaSet"), "apps");
}

#[test]
fn test_api_group_for_kind_batch() {
    assert_eq!(api_group_for_kind("Job"), "batch");
    assert_eq!(api_group_for_kind("CronJob"), "batch");
}

#[test]
fn test_api_group_for_kind_networking() {
    assert_eq!(api_group_for_kind("Ingress"), "networking.k8s.io");
    assert_eq!(api_group_for_kind("NetworkPolicy"), "networking.k8s.io");
}

#[test]
fn test_api_group_for_kind_unknown() {
    assert_eq!(api_group_for_kind("SomeCustomResource"), "");
}

#[test]
fn test_filtered_actions_all_allowed() {
    let mut cache = RbacCache::new();
    // Allow all verbs on pods in default namespace
    cache.record(
        PermissionCheck::new(RbacVerb::Get, "pods/log", "", Some("default".to_string())),
        PermissionResult::allowed(),
    );
    cache.record(
        PermissionCheck::new(RbacVerb::Create, "pods/exec", "", Some("default".to_string())),
        PermissionResult::allowed(),
    );
    cache.record(
        PermissionCheck::new(RbacVerb::Delete, "pods", "", Some("default".to_string())),
        PermissionResult::allowed(),
    );

    let state = ResourceListState::new("Pod", "v1");
    let actions = state.filtered_actions(&cache, Some("default"));
    // Pod actions: ViewLogs, Exec, Delete -- all allowed
    assert_eq!(actions.len(), 3);
}

#[test]
fn test_filtered_actions_some_denied() {
    let mut cache = RbacCache::new();
    // Allow ViewLogs, deny Exec, allow Delete
    cache.record(
        PermissionCheck::new(RbacVerb::Get, "pods/log", "", Some("default".to_string())),
        PermissionResult::allowed(),
    );
    cache.record(
        PermissionCheck::new(RbacVerb::Create, "pods/exec", "", Some("default".to_string())),
        PermissionResult::denied("forbidden"),
    );
    cache.record(
        PermissionCheck::new(RbacVerb::Delete, "pods", "", Some("default".to_string())),
        PermissionResult::allowed(),
    );

    let state = ResourceListState::new("Pod", "v1");
    let actions = state.filtered_actions(&cache, Some("default"));
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0], QuickAction::ViewLogs);
    assert_eq!(actions[1], QuickAction::Delete);
}

#[test]
fn test_filtered_actions_uncached_assumed_allowed() {
    // Empty cache: all actions should pass through (optimistic)
    let cache = RbacCache::new();
    let state = ResourceListState::new("Pod", "v1");
    let actions = state.filtered_actions(&cache, Some("default"));
    assert_eq!(actions.len(), 3); // ViewLogs, Exec, Delete
}

#[test]
fn test_filtered_actions_deployment_rbac() {
    let mut cache = RbacCache::new();
    let ns = Some("production".to_string());

    // Allow Update on deployments (covers Scale, Restart, EditYaml)
    cache.record(
        PermissionCheck::new(RbacVerb::Update, "deployments", "apps", ns.clone()),
        PermissionResult::allowed(),
    );
    // Deny Delete on deployments
    cache.record(
        PermissionCheck::new(RbacVerb::Delete, "deployments", "apps", ns.clone()),
        PermissionResult::denied("read-only"),
    );

    let state = ResourceListState::new("Deployment", "apps/v1");
    let actions = state.filtered_actions(&cache, Some("production"));
    // Deployment actions: Scale, Restart, EditYaml, Delete
    // Scale/Restart/EditYaml -> Update (allowed), Delete -> denied
    assert_eq!(actions.len(), 3);
    assert!(actions.iter().all(|a| !a.is_destructive()));
}

#[test]
fn test_filtered_actions_node_cordon_uncordon() {
    let mut cache = RbacCache::new();
    // Cordon/Uncordon need Patch on "nodes" with "" api_group
    cache.record(
        PermissionCheck::new(RbacVerb::Patch, "nodes", "", None),
        PermissionResult::allowed(),
    );

    let state = ResourceListState::new("Node", "v1");
    let actions = state.filtered_actions(&cache, None);
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0], QuickAction::Cordon);
    assert_eq!(actions[1], QuickAction::Uncordon);
}

#[test]
fn test_filtered_actions_node_cordon_denied() {
    let mut cache = RbacCache::new();
    cache.record(
        PermissionCheck::new(RbacVerb::Patch, "nodes", "", None),
        PermissionResult::denied("cluster admin only"),
    );

    let state = ResourceListState::new("Node", "v1");
    let actions = state.filtered_actions(&cache, None);
    assert!(actions.is_empty());
}

#[test]
fn test_filtered_actions_mixed_cached_and_uncached() {
    let mut cache = RbacCache::new();
    // Only cache Delete as denied; ViewLogs and Exec are uncached (optimistic)
    cache.record(
        PermissionCheck::new(RbacVerb::Delete, "pods", "", Some("default".to_string())),
        PermissionResult::denied("read-only"),
    );

    let state = ResourceListState::new("Pod", "v1");
    let actions = state.filtered_actions(&cache, Some("default"));
    // ViewLogs (uncached->allowed), Exec (uncached->allowed), Delete (denied)
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0], QuickAction::ViewLogs);
    assert_eq!(actions[1], QuickAction::Exec);
}

#[test]
fn test_filtered_actions_all_denied() {
    let mut cache = RbacCache::new();
    let ns = Some("locked".to_string());
    cache.record(
        PermissionCheck::new(RbacVerb::Get, "pods/log", "", ns.clone()),
        PermissionResult::denied("no"),
    );
    cache.record(
        PermissionCheck::new(RbacVerb::Create, "pods/exec", "", ns.clone()),
        PermissionResult::denied("no"),
    );
    cache.record(
        PermissionCheck::new(RbacVerb::Delete, "pods", "", ns.clone()),
        PermissionResult::denied("no"),
    );

    let state = ResourceListState::new("Pod", "v1");
    let actions = state.filtered_actions(&cache, Some("locked"));
    assert!(actions.is_empty());
}

// ===================================================================
// T064a: Create Resource State Tests
// ===================================================================

#[test]
fn test_create_resource_state_new() {
    let state = CreateResourceState::new("Pod", "v1", Some("default"));
    assert_eq!(state.kind, "Pod");
    assert_eq!(state.api_version, "v1");
    assert_eq!(state.namespace.as_deref(), Some("default"));
    assert!(state.modified_yaml.is_none());
    assert!(state.validation_error.is_none());
    assert!(!state.submitting);
    assert!(!state.yaml_template.is_empty());
}

#[test]
fn test_create_resource_state_no_namespace() {
    let state = CreateResourceState::new("Node", "v1", None);
    assert!(state.namespace.is_none());
    assert!(!state.yaml_template.contains("namespace"));
}

#[test]
fn test_default_template_pod() {
    let tmpl = CreateResourceState::default_template("Pod", "v1", Some("default"));
    assert!(tmpl.contains("apiVersion: v1"));
    assert!(tmpl.contains("kind: Pod"));
    assert!(tmpl.contains("name: my-pod"));
    assert!(tmpl.contains("namespace: default"));
    assert!(tmpl.contains("containers:"));
    assert!(tmpl.contains("image: nginx:latest"));
}

#[test]
fn test_default_template_deployment() {
    let tmpl = CreateResourceState::default_template("Deployment", "apps/v1", Some("prod"));
    assert!(tmpl.contains("apiVersion: apps/v1"));
    assert!(tmpl.contains("kind: Deployment"));
    assert!(tmpl.contains("replicas: 1"));
    assert!(tmpl.contains("matchLabels:"));
    assert!(tmpl.contains("template:"));
    assert!(tmpl.contains("namespace: prod"));
}

#[test]
fn test_default_template_service() {
    let tmpl = CreateResourceState::default_template("Service", "v1", Some("default"));
    assert!(tmpl.contains("kind: Service"));
    assert!(tmpl.contains("ports:"));
    assert!(tmpl.contains("port: 80"));
}

#[test]
fn test_default_template_configmap() {
    let tmpl = CreateResourceState::default_template("ConfigMap", "v1", None);
    assert!(tmpl.contains("kind: ConfigMap"));
    assert!(tmpl.contains("data:"));
    assert!(tmpl.contains("key: value"));
    assert!(!tmpl.contains("namespace"));
}

#[test]
fn test_default_template_secret() {
    let tmpl = CreateResourceState::default_template("Secret", "v1", Some("default"));
    assert!(tmpl.contains("kind: Secret"));
    assert!(tmpl.contains("type: Opaque"));
    assert!(tmpl.contains("data:"));
}

#[test]
fn test_default_template_unknown_kind() {
    let tmpl = CreateResourceState::default_template("CustomWidget", "example.com/v1", None);
    assert!(tmpl.contains("kind: CustomWidget"));
    assert!(tmpl.contains("apiVersion: example.com/v1"));
    assert!(tmpl.contains("spec: {}"));
}

#[test]
fn test_set_yaml() {
    let mut state = CreateResourceState::new("Pod", "v1", Some("default"));
    assert!(state.modified_yaml.is_none());

    state.set_yaml("apiVersion: v1\nkind: Pod\nmetadata:\n  name: custom");
    assert_eq!(
        state.modified_yaml.as_deref(),
        Some("apiVersion: v1\nkind: Pod\nmetadata:\n  name: custom")
    );
}

#[test]
fn test_validate_valid_yaml() {
    let mut state = CreateResourceState::new("Pod", "v1", Some("default"));
    // The default template should be valid YAML
    assert!(state.validate());
    assert!(state.validation_error.is_none());
}

#[test]
fn test_validate_modified_valid_yaml() {
    let mut state = CreateResourceState::new("Pod", "v1", Some("default"));
    state.set_yaml("apiVersion: v1\nkind: Pod\nmetadata:\n  name: test");
    assert!(state.validate());
    assert!(state.validation_error.is_none());
}

#[test]
fn test_validate_invalid_yaml() {
    let mut state = CreateResourceState::new("Pod", "v1", Some("default"));
    state.set_yaml("this: is: not: valid:\n  yaml: [unclosed");
    assert!(!state.validate());
    assert!(state.validation_error.is_some());
}

#[test]
fn test_validate_uses_modified_over_template() {
    let mut state = CreateResourceState::new("Pod", "v1", Some("default"));
    // Template is valid, but set invalid modified yaml
    state.set_yaml("key: [unclosed bracket");
    assert!(!state.validate());
    assert!(state.validation_error.is_some());
}

#[test]
fn test_validate_falls_back_to_template() {
    let mut state = CreateResourceState::new("Pod", "v1", Some("default"));
    // No modified yaml set; should validate the template
    assert!(state.validate());
}

#[test]
fn test_set_submitting() {
    let mut state = CreateResourceState::new("Pod", "v1", Some("default"));
    assert!(!state.submitting);

    state.set_submitting(true);
    assert!(state.submitting);

    state.set_submitting(false);
    assert!(!state.submitting);
}

#[test]
fn test_set_validation_error() {
    let mut state = CreateResourceState::new("Pod", "v1", Some("default"));
    assert!(state.validation_error.is_none());

    state.set_validation_error("missing required field: spec.containers");
    assert_eq!(state.validation_error.as_deref(), Some("missing required field: spec.containers"));
}

#[test]
fn test_clear_validation_error() {
    let mut state = CreateResourceState::new("Pod", "v1", Some("default"));
    state.set_validation_error("some error");
    assert!(state.validation_error.is_some());

    state.clear_validation_error();
    assert!(state.validation_error.is_none());
}

// --- ResourceListState create dialog tests ---

#[test]
fn test_open_create_dialog() {
    let mut state = ResourceListState::new("Pod", "v1");
    assert!(state.create_dialog().is_none());

    state.open_create_dialog();
    let dialog = state.create_dialog().unwrap();
    assert_eq!(dialog.kind, "Pod");
    assert_eq!(dialog.api_version, "v1");
    assert!(dialog.namespace.is_none()); // no namespace_filter set
}

#[test]
fn test_open_create_dialog_with_namespace_filter() {
    let mut state = ResourceListState::new("Deployment", "apps/v1");
    state.set_namespace_filter(Some("production".to_string()));

    state.open_create_dialog();
    let dialog = state.create_dialog().unwrap();
    assert_eq!(dialog.kind, "Deployment");
    assert_eq!(dialog.namespace.as_deref(), Some("production"));
    assert!(dialog.yaml_template.contains("namespace: production"));
}

#[test]
fn test_close_create_dialog() {
    let mut state = ResourceListState::new("Pod", "v1");
    state.open_create_dialog();
    assert!(state.create_dialog().is_some());

    state.close_create_dialog();
    assert!(state.create_dialog().is_none());
}

#[test]
fn test_close_create_dialog_when_not_open() {
    let mut state = ResourceListState::new("Pod", "v1");
    state.close_create_dialog(); // should be a no-op
    assert!(state.create_dialog().is_none());
}

#[test]
fn test_create_dialog_returns_none_initially() {
    let state = ResourceListState::new("Pod", "v1");
    assert!(state.create_dialog().is_none());
}

#[test]
fn test_create_dialog_full_workflow() {
    let mut state = ResourceListState::new("ConfigMap", "v1");
    state.set_namespace_filter(Some("staging".to_string()));

    // Open dialog
    state.open_create_dialog();
    let dialog = state.create_dialog().unwrap();
    assert_eq!(dialog.kind, "ConfigMap");
    assert!(dialog.yaml_template.contains("data:"));
    assert!(dialog.yaml_template.contains("namespace: staging"));

    // Modify YAML through the show_create_dialog field
    state.show_create_dialog.as_mut().unwrap().set_yaml(
        "apiVersion: v1\nkind: ConfigMap\nmetadata:\n  name: my-config\ndata:\n  foo: bar",
    );

    // Validate
    assert!(state.show_create_dialog.as_mut().unwrap().validate());

    // Submit
    state.show_create_dialog.as_mut().unwrap().set_submitting(true);
    assert!(state.show_create_dialog.as_ref().unwrap().submitting);

    // Close after success
    state.close_create_dialog();
    assert!(state.create_dialog().is_none());
}

#[test]
fn test_default_template_is_valid_yaml_for_all_kinds() {
    let test_cases = vec![
        ("Pod", "v1"),
        ("Deployment", "apps/v1"),
        ("Service", "v1"),
        ("ConfigMap", "v1"),
        ("Secret", "v1"),
        ("CustomWidget", "example.com/v1"),
    ];

    for (kind, api_version) in test_cases {
        let mut state = CreateResourceState::new(kind, api_version, Some("test"));
        assert!(state.validate(), "Default template for {kind} should be valid YAML");
    }
}

// ===================================================================
// T104: Network resource column configuration tests
// ===================================================================

#[test]
fn test_columns_for_service() {
    let cols = columns_for_kind("Service");
    assert_eq!(cols.len(), 7);
    assert_eq!(cols[0].id, "name");
    assert_eq!(cols[1].id, "namespace");
    assert_eq!(cols[2].id, "type");
    assert_eq!(cols[3].id, "cluster_ip");
    assert_eq!(cols[4].id, "external_ip");
    assert_eq!(cols[5].id, "ports");
    assert_eq!(cols[6].id, "age");
    // Name and type should be sortable
    assert!(cols[0].sortable);
    assert!(cols[2].sortable);
    // Cluster IP and ports not sortable
    assert!(!cols[3].sortable);
    assert!(!cols[5].sortable);
}

#[test]
fn test_columns_for_ingress() {
    let cols = columns_for_kind("Ingress");
    assert_eq!(cols.len(), 6);
    assert_eq!(cols[0].id, "name");
    assert_eq!(cols[1].id, "namespace");
    assert_eq!(cols[2].id, "hosts");
    assert_eq!(cols[3].id, "address");
    assert_eq!(cols[4].id, "ports");
    assert_eq!(cols[5].id, "age");
    assert!(cols[0].sortable);
    assert!(!cols[2].sortable); // hosts not sortable
}

#[test]
fn test_columns_for_network_policy() {
    let cols = columns_for_kind("NetworkPolicy");
    assert_eq!(cols.len(), 4);
    assert_eq!(cols[0].id, "name");
    assert_eq!(cols[1].id, "namespace");
    assert_eq!(cols[2].id, "pod_selector");
    assert_eq!(cols[3].id, "age");
    assert!(!cols[2].sortable); // pod_selector not sortable
}

#[test]
fn test_columns_for_endpoints() {
    let cols = columns_for_kind("Endpoints");
    assert_eq!(cols.len(), 4);
    assert_eq!(cols[0].id, "name");
    assert_eq!(cols[1].id, "namespace");
    assert_eq!(cols[2].id, "endpoints");
    assert_eq!(cols[3].id, "age");
    assert!(!cols[2].sortable); // endpoints not sortable
}

// ===================================================================
// T105: Storage resource column configuration tests
// ===================================================================

#[test]
fn test_columns_for_persistent_volume() {
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
    // PV is cluster-scoped, no namespace column
    assert!(cols.iter().all(|c| c.id != "namespace"));
    assert!(cols[1].sortable); // capacity sortable
    assert!(!cols[2].sortable); // access_modes not sortable
}

#[test]
fn test_columns_for_persistent_volume_claim() {
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
    assert!(cols[2].sortable); // status sortable
    assert!(!cols[5].sortable); // access_modes not sortable
}

#[test]
fn test_columns_for_storage_class() {
    let cols = columns_for_kind("StorageClass");
    assert_eq!(cols.len(), 6);
    assert_eq!(cols[0].id, "name");
    assert_eq!(cols[1].id, "provisioner");
    assert_eq!(cols[2].id, "reclaim_policy");
    assert_eq!(cols[3].id, "volume_binding_mode");
    assert_eq!(cols[4].id, "allow_expansion");
    assert_eq!(cols[5].id, "age");
    // StorageClass is cluster-scoped, no namespace column
    assert!(cols.iter().all(|c| c.id != "namespace"));
    assert!(cols[1].sortable); // provisioner sortable
    assert!(!cols[4].sortable); // allow_expansion not sortable
}

#[test]
fn test_columns_for_unknown_kind_returns_default() {
    let cols = columns_for_kind("CustomResource");
    assert_eq!(cols.len(), 4);
    assert_eq!(cols[0].id, "name");
    assert_eq!(cols[1].id, "namespace");
    assert_eq!(cols[2].id, "status");
    assert_eq!(cols[3].id, "age");
}

#[test]
fn test_columns_all_have_name_first() {
    let kinds = vec![
        "Service",
        "Ingress",
        "NetworkPolicy",
        "Endpoints",
        "PersistentVolume",
        "PersistentVolumeClaim",
        "StorageClass",
    ];
    for kind in kinds {
        let cols = columns_for_kind(kind);
        assert_eq!(cols[0].id, "name", "First column for {kind} should be 'name'");
        assert!(cols[0].sortable, "Name column for {kind} should be sortable");
    }
}

#[test]
fn test_columns_all_have_age_last() {
    let kinds = vec![
        "Service",
        "Ingress",
        "NetworkPolicy",
        "Endpoints",
        "PersistentVolume",
        "PersistentVolumeClaim",
        "StorageClass",
    ];
    for kind in kinds {
        let cols = columns_for_kind(kind);
        let last = cols.last().unwrap();
        assert_eq!(last.id, "age", "Last column for {kind} should be 'age'");
        assert!(last.sortable, "Age column for {kind} should be sortable");
    }
}

// ========================================================================
// T033: Render-related state tests for ResourceListView
// ========================================================================

#[test]
fn test_view_toolbar_title_pod() {
    let state = ResourceListState::new("Pod", "v1");
    let view = ResourceListView::new(state, Theme::dark());
    assert_eq!(view.toolbar_title(), "Pods");
}

#[test]
fn test_view_toolbar_title_deployment() {
    let state = ResourceListState::new("Deployment", "apps/v1");
    let view = ResourceListView::new(state, Theme::dark());
    assert_eq!(view.toolbar_title(), "Deployments");
}

#[test]
fn test_view_is_loading() {
    let mut state = ResourceListState::new("Pod", "v1");
    state.set_loading(true);
    let view = ResourceListView::new(state, Theme::dark());
    assert!(view.is_loading());
}

#[test]
fn test_view_not_loading() {
    let state = ResourceListState::new("Pod", "v1");
    let view = ResourceListView::new(state, Theme::dark());
    assert!(!view.is_loading());
}

#[test]
fn test_view_has_error() {
    let mut state = ResourceListState::new("Pod", "v1");
    state.set_error("connection refused".to_string());
    let view = ResourceListView::new(state, Theme::dark());
    assert!(view.has_error());
}

#[test]
fn test_view_no_error() {
    let state = ResourceListState::new("Pod", "v1");
    let view = ResourceListView::new(state, Theme::dark());
    assert!(!view.has_error());
}

#[test]
fn test_view_has_no_rows_initially() {
    let state = ResourceListState::new("Pod", "v1");
    let view = ResourceListView::new(state, Theme::dark());
    assert!(!view.has_rows());
}

#[test]
fn test_view_table_columns_match_kind() {
    let state = ResourceListState::new("Service", "v1");
    let view = ResourceListView::new(state, Theme::dark());
    // T337: ResourceListView now uses columns_for_kind from resource_table.
    let expected_cols = table_columns_for_kind("Service");
    assert_eq!(view.table_state.columns.len(), expected_cols.len());
    for (a, b) in view.table_state.columns.iter().zip(expected_cols.iter()) {
        assert_eq!(a.id, b.id);
    }
}

#[test]
fn test_view_table_columns_deployment() {
    let state = ResourceListState::new("Deployment", "apps/v1");
    let view = ResourceListView::new(state, Theme::dark());
    // T337: ResourceListView now uses columns_for_kind from resource_table.
    let expected_cols = table_columns_for_kind("Deployment");
    assert_eq!(view.table_state.columns.len(), expected_cols.len());
}

#[test]
fn test_view_search_state_initially_empty() {
    let state = ResourceListState::new("Pod", "v1");
    let view = ResourceListView::new(state, Theme::dark());
    assert!(!view.search_state.is_active());
    assert!(view.search_state.query.is_empty());
}

#[test]
fn test_view_with_light_theme() {
    let state = ResourceListState::new("Pod", "v1");
    let view = ResourceListView::new(state, Theme::light());
    assert_eq!(view.theme.colors.background, Color::rgb(255, 255, 255));
}

#[test]
fn test_view_with_dark_theme() {
    let state = ResourceListState::new("Pod", "v1");
    let view = ResourceListView::new(state, Theme::dark());
    assert_eq!(view.theme.colors.background, Color::rgb(0x1e, 0x21, 0x24));
}

#[test]
fn test_view_error_then_loading_state() {
    let mut state = ResourceListState::new("Pod", "v1");
    state.set_error("timeout".to_string());
    state.set_loading(true);
    let view = ResourceListView::new(state, Theme::dark());
    // Both error and loading are set; render prioritizes loading
    assert!(view.is_loading());
    assert!(view.has_error());
}
