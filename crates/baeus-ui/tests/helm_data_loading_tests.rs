//! T063 – Wire Helm data loading
//!
//! Integration tests for the data-loading lifecycle of `HelmReleasesViewState`
//! and `HelmInstallViewState`, including sorting, filtering, search, and
//! status decoding.

use baeus_helm::charts::ChartEntry;
use baeus_helm::{HelmRelease, HelmReleaseStatus};
use baeus_ui::views::helm_install::HelmInstallViewState;
use baeus_ui::views::helm_releases::HelmReleasesViewState;
use chrono::{Duration, Utc};
use serde_json::json;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_release(
    name: &str,
    namespace: &str,
    status: HelmReleaseStatus,
    minutes_ago: i64,
) -> HelmRelease {
    HelmRelease {
        name: name.to_string(),
        namespace: namespace.to_string(),
        chart_name: format!("{name}-chart"),
        chart_version: "1.0.0".to_string(),
        app_version: Some("1.0.0".to_string()),
        status,
        revision: 1,
        last_deployed: Utc::now() - Duration::minutes(minutes_ago),
        values: json!({}),
        cluster_id: Uuid::new_v4(),
    }
}

fn sample_releases() -> Vec<HelmRelease> {
    vec![
        make_release("nginx", "default", HelmReleaseStatus::Deployed, 10),
        make_release("redis", "default", HelmReleaseStatus::Deployed, 20),
        make_release("prometheus", "monitoring", HelmReleaseStatus::Deployed, 30),
        make_release("broken-app", "staging", HelmReleaseStatus::Failed, 5),
        make_release("upgrading-app", "staging", HelmReleaseStatus::PendingUpgrade, 2),
    ]
}

fn sample_chart(name: &str, version: &str) -> ChartEntry {
    ChartEntry {
        name: name.to_string(),
        version: version.to_string(),
        app_version: Some("1.0.0".to_string()),
        description: Some(format!("{name} chart")),
        home: None,
        sources: vec![],
        urls: vec![format!("https://charts.example.com/{name}-{version}.tgz")],
    }
}

// ===========================================================================
// HelmReleasesViewState – loading lifecycle
// ===========================================================================

#[test]
fn loading_lifecycle_success() {
    let mut state = HelmReleasesViewState::default();

    // 1. begin loading
    state.set_loading(true);
    assert!(state.loading);
    assert!(state.error.is_none());

    // 2. receive releases
    state.set_releases(sample_releases());
    assert!(!state.loading);
    assert_eq!(state.releases.len(), 5);
    assert!(state.error.is_none());
}

#[test]
fn loading_lifecycle_error() {
    let mut state = HelmReleasesViewState::default();

    state.set_loading(true);
    assert!(state.loading);

    state.set_error("timeout connecting to cluster".to_string());
    assert!(!state.loading);
    assert_eq!(state.error.as_deref(), Some("timeout connecting to cluster"));
}

#[test]
fn loading_clears_previous_error() {
    let mut state = HelmReleasesViewState::default();
    state.set_error("old".to_string());

    state.set_loading(true);
    assert!(state.error.is_none());
}

#[test]
fn set_releases_clears_loading_and_error() {
    let mut state = HelmReleasesViewState::default();
    state.loading = true;
    state.error = Some("stale error".to_string());

    state.set_releases(vec![]);
    assert!(!state.loading);
    assert!(state.error.is_none());
}

#[test]
fn multiple_releases_populated_correctly() {
    let mut state = HelmReleasesViewState::default();
    state.set_releases(sample_releases());

    assert_eq!(state.releases.len(), 5);
    assert_eq!(state.releases[0].name, "nginx");
    assert_eq!(state.releases[4].name, "upgrading-app");
}

#[test]
fn release_count_returns_total() {
    let mut state = HelmReleasesViewState::default();
    assert_eq!(state.release_count(), 0);

    state.set_releases(sample_releases());
    assert_eq!(state.release_count(), 5);
}

#[test]
fn release_count_ignores_namespace_filter() {
    let mut state = HelmReleasesViewState::default();
    state.set_releases(sample_releases());
    state.filter_by_namespace(Some("default".to_string()));

    // filtered shows 2, but release_count is always total
    assert_eq!(state.filtered_releases().len(), 2);
    assert_eq!(state.release_count(), 5);
}

// ===========================================================================
// Namespace filtering after loading
// ===========================================================================

#[test]
fn namespace_filter_applied_after_loading() {
    let mut state = HelmReleasesViewState::default();
    state.set_loading(true);
    state.set_releases(sample_releases());

    state.filter_by_namespace(Some("monitoring".to_string()));
    let filtered = state.filtered_releases();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].name, "prometheus");
}

#[test]
fn namespace_filter_shows_all_when_none() {
    let mut state = HelmReleasesViewState::default();
    state.set_releases(sample_releases());
    state.filter_by_namespace(Some("staging".to_string()));
    assert_eq!(state.filtered_releases().len(), 2);

    state.filter_by_namespace(None);
    assert_eq!(state.filtered_releases().len(), 5);
}

// ===========================================================================
// Sorting
// ===========================================================================

#[test]
fn sort_by_name_ascending() {
    let mut state = HelmReleasesViewState::default();
    state.set_releases(sample_releases());

    state.sort_by_name();

    let names: Vec<&str> = state.releases.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["broken-app", "nginx", "prometheus", "redis", "upgrading-app"]);
}

#[test]
fn sort_by_name_case_insensitive() {
    let mut state = HelmReleasesViewState::default();
    state.set_releases(vec![
        make_release("Zebra", "default", HelmReleaseStatus::Deployed, 1),
        make_release("alpha", "default", HelmReleaseStatus::Deployed, 2),
    ]);

    state.sort_by_name();

    assert_eq!(state.releases[0].name, "alpha");
    assert_eq!(state.releases[1].name, "Zebra");
}

#[test]
fn sort_by_status() {
    let mut state = HelmReleasesViewState::default();
    state.set_releases(sample_releases());

    state.sort_by_status();

    // Debug representations sort alphabetically:
    // Deployed, Deployed, Deployed, Failed, PendingUpgrade
    let statuses: Vec<&HelmReleaseStatus> = state.releases.iter().map(|r| &r.status).collect();
    assert_eq!(statuses[0..3], [&HelmReleaseStatus::Deployed; 3]);
    assert_eq!(statuses[3], &HelmReleaseStatus::Failed);
    assert_eq!(statuses[4], &HelmReleaseStatus::PendingUpgrade);
}

#[test]
fn sort_by_last_deployed_newest_first() {
    let mut state = HelmReleasesViewState::default();
    state.set_releases(sample_releases());

    state.sort_by_last_deployed();

    // minutes_ago: upgrading-app(2), broken-app(5), nginx(10), redis(20), prometheus(30)
    let names: Vec<&str> = state.releases.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names[0], "upgrading-app");
    assert_eq!(names[4], "prometheus");
}

#[test]
fn sort_by_name_empty_releases() {
    let mut state = HelmReleasesViewState::default();
    state.sort_by_name(); // should not panic
    assert!(state.releases.is_empty());
}

#[test]
fn sort_by_status_empty_releases() {
    let mut state = HelmReleasesViewState::default();
    state.sort_by_status();
    assert!(state.releases.is_empty());
}

#[test]
fn sort_by_last_deployed_empty_releases() {
    let mut state = HelmReleasesViewState::default();
    state.sort_by_last_deployed();
    assert!(state.releases.is_empty());
}

// ===========================================================================
// HelmInstallViewState – chart search lifecycle
// ===========================================================================

#[test]
fn begin_search_sets_searching() {
    let mut state = HelmInstallViewState::default();
    state.error = Some("old error".to_string());

    state.begin_search();

    assert!(state.searching);
    assert!(state.error.is_none());
}

#[test]
fn search_complete_sets_results_and_clears_searching() {
    let mut state = HelmInstallViewState::default();
    state.begin_search();

    let results = vec![sample_chart("nginx", "15.4.0"), sample_chart("redis", "18.0.0")];
    state.search_complete(results);

    assert!(!state.searching);
    assert_eq!(state.search_results.len(), 2);
    assert!(state.error.is_none());
}

#[test]
fn search_failed_sets_error_and_clears_searching() {
    let mut state = HelmInstallViewState::default();
    state.begin_search();

    state.search_failed("network error".to_string());

    assert!(!state.searching);
    assert_eq!(state.error.as_deref(), Some("network error"));
}

#[test]
fn search_complete_with_empty_results() {
    let mut state = HelmInstallViewState::default();
    state.begin_search();
    state.search_complete(vec![]);

    assert!(!state.searching);
    assert!(state.search_results.is_empty());
    assert!(state.error.is_none());
}

#[test]
fn set_search_results_updates_list() {
    let mut state = HelmInstallViewState::default();

    // First batch
    state.set_search_results(vec![sample_chart("nginx", "15.4.0")]);
    assert_eq!(state.search_results.len(), 1);

    // Second batch replaces
    state.set_search_results(vec![
        sample_chart("redis", "18.0.0"),
        sample_chart("postgresql", "13.0.0"),
    ]);
    assert_eq!(state.search_results.len(), 2);
    assert_eq!(state.search_results[0].name, "redis");
}

// ===========================================================================
// Chart selection populates detail
// ===========================================================================

#[test]
fn chart_selection_populates_detail() {
    let mut state = HelmInstallViewState::default();
    let chart = sample_chart("nginx", "15.4.0");

    state.select_chart(chart);

    assert_eq!(state.selected_chart_name(), Some("nginx"));
    assert_eq!(state.selected_version.as_deref(), Some("15.4.0"));
}

#[test]
fn chart_selection_overrides_previous() {
    let mut state = HelmInstallViewState::default();
    state.select_chart(sample_chart("nginx", "15.4.0"));
    state.select_chart(sample_chart("redis", "18.5.0"));

    assert_eq!(state.selected_chart_name(), Some("redis"));
    assert_eq!(state.selected_version.as_deref(), Some("18.5.0"));
}

// ===========================================================================
// Install lifecycle
// ===========================================================================

#[test]
fn begin_install_sets_installing() {
    let mut state = HelmInstallViewState::default();
    state.select_chart(sample_chart("nginx", "15.4.0"));
    state.error = Some("stale".to_string());

    state.begin_install();

    assert!(state.installing);
    assert!(state.error.is_none());
    assert!(!state.can_install()); // locked while installing
}

#[test]
fn install_complete_resets_state() {
    let mut state = HelmInstallViewState::default();
    state.select_chart(sample_chart("nginx", "15.4.0"));
    state.begin_install();

    state.install_complete();

    assert!(!state.installing);
    assert!(state.selected_chart.is_none());
    assert!(state.search_results.is_empty());
    assert_eq!(state.namespace, "default");
}

#[test]
fn install_failed_sets_error() {
    let mut state = HelmInstallViewState::default();
    state.select_chart(sample_chart("nginx", "15.4.0"));
    state.begin_install();

    state.install_failed("helm install timed out".to_string());

    assert!(!state.installing);
    assert_eq!(state.error.as_deref(), Some("helm install timed out"));
    // Chart selection still present so the user can retry
    assert!(state.selected_chart.is_some());
}

// ===========================================================================
// Release status decoding
// ===========================================================================

#[test]
fn status_deployed_is_healthy() {
    assert!(HelmReleaseStatus::Deployed.is_healthy());
}

#[test]
fn status_failed_is_not_healthy() {
    assert!(!HelmReleaseStatus::Failed.is_healthy());
}

#[test]
fn status_pending_upgrade_is_not_healthy() {
    assert!(!HelmReleaseStatus::PendingUpgrade.is_healthy());
}

#[test]
fn status_pending_install_is_not_healthy() {
    assert!(!HelmReleaseStatus::PendingInstall.is_healthy());
}

#[test]
fn status_pending_rollback_is_not_healthy() {
    assert!(!HelmReleaseStatus::PendingRollback.is_healthy());
}

#[test]
fn status_uninstalling_is_not_healthy() {
    assert!(!HelmReleaseStatus::Uninstalling.is_healthy());
}

#[test]
fn status_superseded_is_not_healthy() {
    assert!(!HelmReleaseStatus::Superseded.is_healthy());
}

#[test]
fn status_unknown_is_not_healthy() {
    assert!(!HelmReleaseStatus::Unknown.is_healthy());
}

#[test]
fn from_str_status_all_variants() {
    assert_eq!(HelmReleaseStatus::from_str_status("deployed"), HelmReleaseStatus::Deployed);
    assert_eq!(HelmReleaseStatus::from_str_status("failed"), HelmReleaseStatus::Failed);
    assert_eq!(HelmReleaseStatus::from_str_status("uninstalling"), HelmReleaseStatus::Uninstalling);
    assert_eq!(
        HelmReleaseStatus::from_str_status("pending-install"),
        HelmReleaseStatus::PendingInstall
    );
    assert_eq!(
        HelmReleaseStatus::from_str_status("pending-upgrade"),
        HelmReleaseStatus::PendingUpgrade
    );
    assert_eq!(
        HelmReleaseStatus::from_str_status("pending-rollback"),
        HelmReleaseStatus::PendingRollback
    );
    assert_eq!(HelmReleaseStatus::from_str_status("superseded"), HelmReleaseStatus::Superseded);
    assert_eq!(HelmReleaseStatus::from_str_status("anything-else"), HelmReleaseStatus::Unknown);
}

#[test]
fn from_str_status_case_insensitive() {
    assert_eq!(HelmReleaseStatus::from_str_status("DEPLOYED"), HelmReleaseStatus::Deployed);
    assert_eq!(HelmReleaseStatus::from_str_status("Failed"), HelmReleaseStatus::Failed);
    assert_eq!(
        HelmReleaseStatus::from_str_status("Pending-Upgrade"),
        HelmReleaseStatus::PendingUpgrade
    );
}
