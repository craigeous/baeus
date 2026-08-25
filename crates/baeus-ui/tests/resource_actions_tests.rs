// T334: Resource-specific actions tests (FR-009)

use baeus_ui::components::resource_table::*;

// ---------------------------------------------------------------------------
// Pod actions
// ---------------------------------------------------------------------------

#[test]
fn pod_actions_count() {
    let actions = actions_for_kind("Pod");
    assert_eq!(actions.len(), 6, "Pod should have 6 actions");
}

#[test]
fn pod_actions_labels() {
    let actions = actions_for_kind("Pod");
    let labels: Vec<&str> = actions.iter().map(|a| a.label.as_str()).collect();
    assert_eq!(labels, vec!["Shell", "Attach", "Evict", "Logs", "Edit", "Delete"]);
}

// ---------------------------------------------------------------------------
// Deployment actions
// ---------------------------------------------------------------------------

#[test]
fn deployment_actions_count() {
    let actions = actions_for_kind("Deployment");
    assert_eq!(actions.len(), 5, "Deployment should have 5 actions");
}

#[test]
fn deployment_actions_labels() {
    let actions = actions_for_kind("Deployment");
    let labels: Vec<&str> = actions.iter().map(|a| a.label.as_str()).collect();
    assert_eq!(labels, vec!["Scale", "Restart", "Logs", "Edit", "Delete"]);
}

// ---------------------------------------------------------------------------
// StatefulSet actions
// ---------------------------------------------------------------------------

#[test]
fn statefulset_actions_count() {
    let actions = actions_for_kind("StatefulSet");
    assert_eq!(actions.len(), 5, "StatefulSet should have 5 actions");
}

#[test]
fn statefulset_actions_labels() {
    let actions = actions_for_kind("StatefulSet");
    let labels: Vec<&str> = actions.iter().map(|a| a.label.as_str()).collect();
    assert_eq!(labels, vec!["Scale", "Restart", "Logs", "Edit", "Delete"]);
}

// ---------------------------------------------------------------------------
// DaemonSet actions
// ---------------------------------------------------------------------------

#[test]
fn daemonset_actions_count() {
    let actions = actions_for_kind("DaemonSet");
    assert_eq!(actions.len(), 4, "DaemonSet should have 4 actions");
}

#[test]
fn daemonset_actions_labels() {
    let actions = actions_for_kind("DaemonSet");
    let labels: Vec<&str> = actions.iter().map(|a| a.label.as_str()).collect();
    assert_eq!(labels, vec!["Restart", "Logs", "Edit", "Delete"]);
}

// ---------------------------------------------------------------------------
// CronJob actions
// ---------------------------------------------------------------------------

#[test]
fn cronjob_actions_count() {
    let actions = actions_for_kind("CronJob");
    assert_eq!(actions.len(), 4, "CronJob should have 4 actions");
}

#[test]
fn cronjob_actions_labels() {
    let actions = actions_for_kind("CronJob");
    let labels: Vec<&str> = actions.iter().map(|a| a.label.as_str()).collect();
    assert_eq!(labels, vec!["Trigger", "Suspend", "Edit", "Delete"]);
}

// ---------------------------------------------------------------------------
// Job actions
// ---------------------------------------------------------------------------

#[test]
fn job_actions_count() {
    let actions = actions_for_kind("Job");
    assert_eq!(actions.len(), 3, "Job should have 3 actions");
}

#[test]
fn job_actions_labels() {
    let actions = actions_for_kind("Job");
    let labels: Vec<&str> = actions.iter().map(|a| a.label.as_str()).collect();
    assert_eq!(labels, vec!["Logs", "Edit", "Delete"]);
}

// ---------------------------------------------------------------------------
// Node actions
// ---------------------------------------------------------------------------

#[test]
fn node_actions_count() {
    let actions = actions_for_kind("Node");
    assert_eq!(actions.len(), 5, "Node should have 5 actions");
}

#[test]
fn node_actions_labels() {
    let actions = actions_for_kind("Node");
    let labels: Vec<&str> = actions.iter().map(|a| a.label.as_str()).collect();
    assert_eq!(labels, vec!["Shell", "Cordon", "Drain", "Edit", "Delete"]);
}

// ---------------------------------------------------------------------------
// Unknown / default actions
// ---------------------------------------------------------------------------

#[test]
fn unknown_kind_returns_default_actions() {
    let actions = actions_for_kind("UnknownKind");
    assert_eq!(actions.len(), 2, "Unknown kind should have 2 default actions");
    let labels: Vec<&str> = actions.iter().map(|a| a.label.as_str()).collect();
    assert_eq!(labels, vec!["Edit", "Delete"]);
}

#[test]
fn another_unknown_kind_returns_default_actions() {
    let actions = actions_for_kind("CustomResource");
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0].label, "Edit");
    assert_eq!(actions[1].label, "Delete");
}

// ---------------------------------------------------------------------------
// Cross-cutting: Delete is always the last action
// ---------------------------------------------------------------------------

#[test]
fn all_action_sets_end_with_delete() {
    let kinds = vec![
        "Pod",
        "Deployment",
        "StatefulSet",
        "DaemonSet",
        "CronJob",
        "Job",
        "Node",
        "UnknownKind",
    ];

    for kind in kinds {
        let actions = actions_for_kind(kind);
        let last = actions
            .last()
            .unwrap_or_else(|| panic!("Kind '{kind}' should have at least one action"));
        assert_eq!(
            last.label, "Delete",
            "Kind '{kind}': last action should be 'Delete', got '{}'",
            last.label
        );
    }
}

// ---------------------------------------------------------------------------
// Cross-cutting: All action sets with mutating types include Edit
// ---------------------------------------------------------------------------

#[test]
fn all_action_sets_include_edit() {
    let kinds = vec![
        "Pod",
        "Deployment",
        "StatefulSet",
        "DaemonSet",
        "CronJob",
        "Job",
        "Node",
        "UnknownKind",
    ];

    for kind in kinds {
        let actions = actions_for_kind(kind);
        let has_edit = actions.iter().any(|a| a.label == "Edit");
        assert!(has_edit, "Kind '{kind}': action set should include 'Edit'");
    }
}

// ---------------------------------------------------------------------------
// Action IDs match labels (lowercase)
// ---------------------------------------------------------------------------

#[test]
fn action_ids_are_lowercase_of_labels() {
    let kinds = vec![
        "Pod",
        "Deployment",
        "StatefulSet",
        "DaemonSet",
        "CronJob",
        "Job",
        "Node",
        "UnknownKind",
    ];

    for kind in kinds {
        let actions = actions_for_kind(kind);
        for action in &actions {
            assert_eq!(
                action.id,
                action.label.to_lowercase(),
                "Kind '{kind}': action id '{}' should be lowercase of label '{}'",
                action.id,
                action.label
            );
        }
    }
}
