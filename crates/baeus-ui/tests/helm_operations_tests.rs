//! T064 – Wire Helm operations
//!
//! Integration tests for `HelmOperation` CLI argument generation,
//! `HelmCommandResult` success/failure, and the `HelmOperationState`
//! lifecycle on `HelmReleasesViewState`.

use baeus_helm::operations::{HelmCommandResult, HelmOperation};
use baeus_helm::{HelmRelease, HelmReleaseStatus};
use baeus_ui::views::helm_releases::{HelmOperationState, HelmReleasesViewState};
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_release(name: &str, namespace: &str, status: HelmReleaseStatus) -> HelmRelease {
    HelmRelease {
        name: name.to_string(),
        namespace: namespace.to_string(),
        chart_name: format!("{name}-chart"),
        chart_version: "1.0.0".to_string(),
        app_version: Some("1.0.0".to_string()),
        status,
        revision: 1,
        last_deployed: Utc::now(),
        values: json!({}),
        cluster_id: Uuid::new_v4(),
    }
}

// ===========================================================================
// HelmOperation::Install – CLI args
// ===========================================================================

#[test]
fn install_generates_correct_cli_args() {
    let op = HelmOperation::Install {
        release_name: "my-app".to_string(),
        chart: "bitnami/nginx".to_string(),
        namespace: "production".to_string(),
        values_file: None,
        version: None,
        create_namespace: false,
    };

    let args = op.to_args();
    assert_eq!(args[0], "install");
    assert_eq!(args[1], "my-app");
    assert_eq!(args[2], "bitnami/nginx");
    assert!(args.contains(&"--namespace".to_string()));
    assert!(args.contains(&"production".to_string()));
}

#[test]
fn install_with_values_file() {
    let op = HelmOperation::Install {
        release_name: "app".to_string(),
        chart: "repo/chart".to_string(),
        namespace: "default".to_string(),
        values_file: Some("/tmp/values.yaml".to_string()),
        version: None,
        create_namespace: false,
    };

    let args = op.to_args();
    assert!(args.contains(&"--values".to_string()));
    assert!(args.contains(&"/tmp/values.yaml".to_string()));
}

#[test]
fn install_with_version() {
    let op = HelmOperation::Install {
        release_name: "app".to_string(),
        chart: "repo/chart".to_string(),
        namespace: "default".to_string(),
        values_file: None,
        version: Some("2.1.0".to_string()),
        create_namespace: false,
    };

    let args = op.to_args();
    assert!(args.contains(&"--version".to_string()));
    assert!(args.contains(&"2.1.0".to_string()));
}

#[test]
fn install_with_create_namespace() {
    let op = HelmOperation::Install {
        release_name: "app".to_string(),
        chart: "repo/chart".to_string(),
        namespace: "new-ns".to_string(),
        values_file: None,
        version: None,
        create_namespace: true,
    };

    let args = op.to_args();
    assert!(args.contains(&"--create-namespace".to_string()));
}

#[test]
fn install_without_create_namespace() {
    let op = HelmOperation::Install {
        release_name: "app".to_string(),
        chart: "repo/chart".to_string(),
        namespace: "existing-ns".to_string(),
        values_file: None,
        version: None,
        create_namespace: false,
    };

    let args = op.to_args();
    assert!(!args.contains(&"--create-namespace".to_string()));
}

// ===========================================================================
// HelmOperation::Upgrade – CLI args
// ===========================================================================

#[test]
fn upgrade_generates_correct_cli_args() {
    let op = HelmOperation::Upgrade {
        release_name: "my-app".to_string(),
        chart: "bitnami/nginx".to_string(),
        namespace: "default".to_string(),
        values_file: None,
        version: None,
        reuse_values: false,
    };

    let args = op.to_args();
    assert_eq!(args[0], "upgrade");
    assert_eq!(args[1], "my-app");
    assert_eq!(args[2], "bitnami/nginx");
}

#[test]
fn upgrade_with_reuse_values() {
    let op = HelmOperation::Upgrade {
        release_name: "svc".to_string(),
        chart: "repo/chart".to_string(),
        namespace: "default".to_string(),
        values_file: None,
        version: None,
        reuse_values: true,
    };

    let args = op.to_args();
    assert!(args.contains(&"--reuse-values".to_string()));
}

#[test]
fn upgrade_without_reuse_values() {
    let op = HelmOperation::Upgrade {
        release_name: "svc".to_string(),
        chart: "repo/chart".to_string(),
        namespace: "default".to_string(),
        values_file: None,
        version: None,
        reuse_values: false,
    };

    let args = op.to_args();
    assert!(!args.contains(&"--reuse-values".to_string()));
}

#[test]
fn upgrade_with_values_and_version() {
    let op = HelmOperation::Upgrade {
        release_name: "svc".to_string(),
        chart: "repo/chart".to_string(),
        namespace: "prod".to_string(),
        values_file: Some("/tmp/prod-values.yaml".to_string()),
        version: Some("3.0.0".to_string()),
        reuse_values: true,
    };

    let args = op.to_args();
    assert!(args.contains(&"--values".to_string()));
    assert!(args.contains(&"/tmp/prod-values.yaml".to_string()));
    assert!(args.contains(&"--version".to_string()));
    assert!(args.contains(&"3.0.0".to_string()));
    assert!(args.contains(&"--reuse-values".to_string()));
}

// ===========================================================================
// HelmOperation::Rollback – CLI args
// ===========================================================================

#[test]
fn rollback_generates_correct_cli_args() {
    let op = HelmOperation::Rollback {
        release_name: "my-app".to_string(),
        namespace: "default".to_string(),
        revision: 5,
    };

    let args = op.to_args();
    assert_eq!(args[0], "rollback");
    assert_eq!(args[1], "my-app");
    assert_eq!(args[2], "5");
    assert!(args.contains(&"--namespace".to_string()));
}

#[test]
fn rollback_to_specific_revision() {
    let op = HelmOperation::Rollback {
        release_name: "web".to_string(),
        namespace: "staging".to_string(),
        revision: 42,
    };

    let args = op.to_args();
    assert_eq!(args[2], "42");
}

#[test]
fn rollback_revision_zero() {
    let op = HelmOperation::Rollback {
        release_name: "app".to_string(),
        namespace: "default".to_string(),
        revision: 0,
    };

    let args = op.to_args();
    assert_eq!(args[2], "0");
}

// ===========================================================================
// HelmOperation::Uninstall – CLI args
// ===========================================================================

#[test]
fn uninstall_generates_correct_cli_args() {
    let op = HelmOperation::Uninstall {
        release_name: "my-app".to_string(),
        namespace: "default".to_string(),
    };

    let args = op.to_args();
    assert_eq!(args[0], "uninstall");
    assert_eq!(args[1], "my-app");
    assert!(args.contains(&"--namespace".to_string()));
    assert!(args.contains(&"default".to_string()));
}

// ===========================================================================
// Operation labels
// ===========================================================================

#[test]
fn operation_labels_are_correct() {
    assert_eq!(
        HelmOperation::Install {
            release_name: String::new(),
            chart: String::new(),
            namespace: String::new(),
            values_file: None,
            version: None,
            create_namespace: false,
        }
        .label(),
        "Install"
    );
    assert_eq!(
        HelmOperation::Upgrade {
            release_name: String::new(),
            chart: String::new(),
            namespace: String::new(),
            values_file: None,
            version: None,
            reuse_values: false,
        }
        .label(),
        "Upgrade"
    );
    assert_eq!(
        HelmOperation::Rollback {
            release_name: String::new(),
            namespace: String::new(),
            revision: 0,
        }
        .label(),
        "Rollback"
    );
    assert_eq!(
        HelmOperation::Uninstall { release_name: String::new(), namespace: String::new() }.label(),
        "Uninstall"
    );
}

// ===========================================================================
// HelmCommandResult
// ===========================================================================

#[test]
fn command_result_success() {
    let result = HelmCommandResult {
        success: true,
        stdout: "Release \"my-app\" installed".to_string(),
        stderr: String::new(),
        exit_code: 0,
    };

    assert!(result.success);
    assert_eq!(result.exit_code, 0);
    assert!(result.stderr.is_empty());
}

#[test]
fn command_result_failure() {
    let result = HelmCommandResult {
        success: false,
        stdout: String::new(),
        stderr: "Error: release name already exists".to_string(),
        exit_code: 1,
    };

    assert!(!result.success);
    assert_eq!(result.exit_code, 1);
    assert!(!result.stderr.is_empty());
}

// ===========================================================================
// HelmOperationState enum
// ===========================================================================

#[test]
fn operation_state_default_is_idle() {
    let state = HelmOperationState::default();
    assert_eq!(state, HelmOperationState::Idle);
}

#[test]
fn operation_state_in_progress() {
    let state = HelmOperationState::InProgress {
        operation: "Install".to_string(),
        release_name: "nginx".to_string(),
    };

    assert!(matches!(state, HelmOperationState::InProgress { .. }));
}

#[test]
fn operation_state_success() {
    let state = HelmOperationState::Success { message: "Release installed".to_string() };

    assert!(matches!(state, HelmOperationState::Success { .. }));
}

#[test]
fn operation_state_failed() {
    let state = HelmOperationState::Failed { error: "timeout".to_string() };

    assert!(matches!(state, HelmOperationState::Failed { .. }));
}

// ===========================================================================
// HelmReleasesViewState – operation lifecycle
// ===========================================================================

#[test]
fn begin_operation_sets_in_progress() {
    let mut state = HelmReleasesViewState::default();
    state.set_releases(vec![make_release("nginx", "default", HelmReleaseStatus::Deployed)]);

    state.begin_operation("Install", "nginx");

    assert!(state.is_operation_in_progress());
    assert_eq!(
        state.operation_state,
        HelmOperationState::InProgress {
            operation: "Install".to_string(),
            release_name: "nginx".to_string(),
        }
    );
}

#[test]
fn operation_success_sets_message() {
    let mut state = HelmReleasesViewState::default();
    state.begin_operation("Upgrade", "redis");

    state.operation_success("Upgrade of redis to v2.0 complete".to_string());

    assert!(!state.is_operation_in_progress());
    assert_eq!(
        state.operation_state,
        HelmOperationState::Success { message: "Upgrade of redis to v2.0 complete".to_string() }
    );
}

#[test]
fn operation_failed_sets_error() {
    let mut state = HelmReleasesViewState::default();
    state.begin_operation("Uninstall", "broken");

    state.operation_failed("release not found".to_string());

    assert!(!state.is_operation_in_progress());
    assert_eq!(
        state.operation_state,
        HelmOperationState::Failed { error: "release not found".to_string() }
    );
}

#[test]
fn dismiss_operation_result_resets_to_idle() {
    let mut state = HelmReleasesViewState::default();
    state.begin_operation("Install", "app");
    state.operation_success("done".to_string());

    state.dismiss_operation_result();

    assert_eq!(state.operation_state, HelmOperationState::Idle);
    assert!(!state.is_operation_in_progress());
}

#[test]
fn is_operation_in_progress_false_when_idle() {
    let state = HelmReleasesViewState::default();
    assert!(!state.is_operation_in_progress());
}

#[test]
fn is_operation_in_progress_false_after_success() {
    let mut state = HelmReleasesViewState::default();
    state.begin_operation("Rollback", "app");
    state.operation_success("Rolled back".to_string());

    assert!(!state.is_operation_in_progress());
}

#[test]
fn is_operation_in_progress_false_after_failure() {
    let mut state = HelmReleasesViewState::default();
    state.begin_operation("Rollback", "app");
    state.operation_failed("revision not found".to_string());

    assert!(!state.is_operation_in_progress());
}

// ===========================================================================
// Install workflow: begin -> complete
// ===========================================================================

#[test]
fn install_workflow_begin_to_complete() {
    let mut state = HelmReleasesViewState::default();

    state.begin_operation("Install", "new-app");
    assert!(state.is_operation_in_progress());

    state.operation_success("Release new-app installed successfully".to_string());
    assert!(!state.is_operation_in_progress());
    assert!(matches!(state.operation_state, HelmOperationState::Success { .. }));

    state.dismiss_operation_result();
    assert_eq!(state.operation_state, HelmOperationState::Idle);
}

// ===========================================================================
// Install workflow: begin -> failed
// ===========================================================================

#[test]
fn install_workflow_begin_to_failed() {
    let mut state = HelmReleasesViewState::default();

    state.begin_operation("Install", "bad-app");
    assert!(state.is_operation_in_progress());

    state.operation_failed("chart not found".to_string());
    assert!(!state.is_operation_in_progress());
    assert!(matches!(state.operation_state, HelmOperationState::Failed { .. }));
}

// ===========================================================================
// Upgrade with reuse_values
// ===========================================================================

#[test]
fn upgrade_operation_with_reuse_values() {
    let op = HelmOperation::Upgrade {
        release_name: "my-svc".to_string(),
        chart: "bitnami/nginx".to_string(),
        namespace: "production".to_string(),
        values_file: None,
        version: Some("16.0.0".to_string()),
        reuse_values: true,
    };

    let args = op.to_args();
    assert_eq!(args[0], "upgrade");
    assert!(args.contains(&"--reuse-values".to_string()));
    assert!(args.contains(&"--version".to_string()));
    assert!(args.contains(&"16.0.0".to_string()));
}

// ===========================================================================
// Rollback to specific revision (full lifecycle)
// ===========================================================================

#[test]
fn rollback_lifecycle() {
    let mut state = HelmReleasesViewState::default();
    state.set_releases(vec![make_release("web-app", "prod", HelmReleaseStatus::Failed)]);

    // Build the rollback operation
    let op = HelmOperation::Rollback {
        release_name: "web-app".to_string(),
        namespace: "prod".to_string(),
        revision: 3,
    };
    assert_eq!(op.label(), "Rollback");

    let args = op.to_args();
    assert_eq!(args[2], "3");

    // Track operation in state
    state.begin_operation("Rollback", "web-app");
    assert!(state.is_operation_in_progress());

    state.operation_success("Rolled back web-app to revision 3".to_string());
    assert!(!state.is_operation_in_progress());
}
