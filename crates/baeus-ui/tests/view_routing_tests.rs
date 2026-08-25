// View routing integration tests (T329)
//
// Tests that verify NavigationTarget routing logic: each variant can be
// constructed, produces the expected label, and WorkspaceState correctly
// manages tabs (open, reuse, close, activate).

use baeus_ui::icons::ResourceCategory;
use baeus_ui::layout::NavigationTarget;
use baeus_ui::layout::workspace::WorkspaceState;
use std::collections::HashSet;

// ===================================================================
// Helper: build all 11 NavigationTarget variants for a given cluster
// ===================================================================

fn all_targets(cluster: &str) -> Vec<NavigationTarget> {
    vec![
        NavigationTarget::ClusterList,
        NavigationTarget::Dashboard { cluster_context: cluster.to_string() },
        NavigationTarget::ResourceList {
            cluster_context: cluster.to_string(),
            category: ResourceCategory::Workloads,
            kind: "Pod".to_string(),
        },
        NavigationTarget::ResourceDetail {
            cluster_context: cluster.to_string(),
            kind: "Pod".to_string(),
            name: "nginx".to_string(),
            namespace: Some("default".to_string()),
        },
        NavigationTarget::HelmReleases { cluster_context: cluster.to_string() },
        NavigationTarget::HelmInstall { cluster_context: cluster.to_string() },
        NavigationTarget::ResourceList {
            cluster_context: cluster.to_string(),
            category: ResourceCategory::Monitoring,
            kind: "Event".to_string(),
        },
        NavigationTarget::CrdBrowser { cluster_context: cluster.to_string() },
        NavigationTarget::NamespaceMap { cluster_context: cluster.to_string() },
        NavigationTarget::PluginManager { cluster_context: cluster.to_string() },
        NavigationTarget::Preferences,
    ]
}

// ===================================================================
// 1. Each NavigationTarget variant can be constructed and has correct
//    cluster_context
// ===================================================================

#[test]
fn test_cluster_list_has_no_cluster_context() {
    let target = NavigationTarget::ClusterList;
    assert_eq!(target.cluster_context(), None);
}

#[test]
fn test_dashboard_has_correct_cluster_context() {
    let target = NavigationTarget::Dashboard { cluster_context: "prod".to_string() };
    assert_eq!(target.cluster_context(), Some("prod"));
}

#[test]
fn test_resource_list_has_correct_cluster_context() {
    let target = NavigationTarget::ResourceList {
        cluster_context: "staging".to_string(),
        category: ResourceCategory::Network,
        kind: "Service".to_string(),
    };
    assert_eq!(target.cluster_context(), Some("staging"));
}

#[test]
fn test_resource_detail_has_correct_cluster_context() {
    let target = NavigationTarget::ResourceDetail {
        cluster_context: "dev".to_string(),
        kind: "Pod".to_string(),
        name: "web-abc123".to_string(),
        namespace: Some("app".to_string()),
    };
    assert_eq!(target.cluster_context(), Some("dev"));
}

#[test]
fn test_helm_releases_has_correct_cluster_context() {
    let target = NavigationTarget::HelmReleases { cluster_context: "prod".to_string() };
    assert_eq!(target.cluster_context(), Some("prod"));
}

#[test]
fn test_helm_install_has_correct_cluster_context() {
    let target = NavigationTarget::HelmInstall { cluster_context: "prod".to_string() };
    assert_eq!(target.cluster_context(), Some("prod"));
}

#[test]
fn test_events_has_correct_cluster_context() {
    let target = NavigationTarget::ResourceList {
        cluster_context: "prod".to_string(),
        category: ResourceCategory::Monitoring,
        kind: "Event".to_string(),
    };
    assert_eq!(target.cluster_context(), Some("prod"));
}

#[test]
fn test_crd_browser_has_correct_cluster_context() {
    let target = NavigationTarget::CrdBrowser { cluster_context: "prod".to_string() };
    assert_eq!(target.cluster_context(), Some("prod"));
}

#[test]
fn test_namespace_map_has_correct_cluster_context() {
    let target = NavigationTarget::NamespaceMap { cluster_context: "prod".to_string() };
    assert_eq!(target.cluster_context(), Some("prod"));
}

#[test]
fn test_plugin_manager_has_correct_cluster_context() {
    let target = NavigationTarget::PluginManager { cluster_context: "prod".to_string() };
    assert_eq!(target.cluster_context(), Some("prod"));
}

#[test]
fn test_all_cluster_scoped_variants_carry_context() {
    let targets = all_targets("my-cluster");
    for target in &targets {
        match target {
            NavigationTarget::ClusterList | NavigationTarget::Preferences => {
                assert_eq!(target.cluster_context(), None);
            }
            _ => {
                assert_eq!(
                    target.cluster_context(),
                    Some("my-cluster"),
                    "Expected cluster_context 'my-cluster' for {:?}",
                    target
                );
            }
        }
    }
}

// ===================================================================
// 2. Opening a tab for each variant creates a tab with the correct label
// ===================================================================

#[test]
fn test_open_tab_cluster_list_label() {
    let mut ws = WorkspaceState::default();
    ws.open_tab(NavigationTarget::ClusterList);
    assert_eq!(ws.active_tab().unwrap().label, "Clusters");
}

#[test]
fn test_open_tab_dashboard_label() {
    let mut ws = WorkspaceState::default();
    ws.open_tab(NavigationTarget::Dashboard { cluster_context: "prod".to_string() });
    assert_eq!(ws.active_tab().unwrap().label, "prod - Overview");
}

#[test]
fn test_open_tab_resource_list_label() {
    let mut ws = WorkspaceState::default();
    ws.open_tab(NavigationTarget::ResourceList {
        cluster_context: "staging".to_string(),
        category: ResourceCategory::Workloads,
        kind: "Deployment".to_string(),
    });
    assert_eq!(ws.active_tab().unwrap().label, "staging - Deployment");
}

#[test]
fn test_open_tab_resource_detail_label() {
    let mut ws = WorkspaceState::default();
    ws.open_tab(NavigationTarget::ResourceDetail {
        cluster_context: "dev".to_string(),
        kind: "Pod".to_string(),
        name: "nginx".to_string(),
        namespace: Some("default".to_string()),
    });
    assert_eq!(ws.active_tab().unwrap().label, "dev - Pod/nginx");
}

#[test]
fn test_open_tab_helm_releases_label() {
    let mut ws = WorkspaceState::default();
    ws.open_tab(NavigationTarget::HelmReleases { cluster_context: "prod".to_string() });
    assert_eq!(ws.active_tab().unwrap().label, "prod - Helm Releases");
}

#[test]
fn test_open_tab_helm_install_label() {
    let mut ws = WorkspaceState::default();
    ws.open_tab(NavigationTarget::HelmInstall { cluster_context: "prod".to_string() });
    assert_eq!(ws.active_tab().unwrap().label, "prod - Install Chart");
}

#[test]
fn test_open_tab_events_label() {
    let mut ws = WorkspaceState::default();
    ws.open_tab(NavigationTarget::ResourceList {
        cluster_context: "prod".to_string(),
        category: ResourceCategory::Monitoring,
        kind: "Event".to_string(),
    });
    assert_eq!(ws.active_tab().unwrap().label, "prod - Event");
}

#[test]
fn test_open_tab_crd_browser_label() {
    let mut ws = WorkspaceState::default();
    ws.open_tab(NavigationTarget::CrdBrowser { cluster_context: "prod".to_string() });
    assert_eq!(ws.active_tab().unwrap().label, "prod - Custom Resources");
}

#[test]
fn test_open_tab_namespace_map_label() {
    let mut ws = WorkspaceState::default();
    ws.open_tab(NavigationTarget::NamespaceMap { cluster_context: "prod".to_string() });
    assert_eq!(ws.active_tab().unwrap().label, "prod - Resource Map");
}

#[test]
fn test_open_tab_plugin_manager_label() {
    let mut ws = WorkspaceState::default();
    ws.open_tab(NavigationTarget::PluginManager { cluster_context: "prod".to_string() });
    assert_eq!(ws.active_tab().unwrap().label, "prod - Plugins");
}

// ===================================================================
// 3. NavigationTarget::label() returns the expected format
// ===================================================================

#[test]
fn test_label_cluster_list() {
    assert_eq!(NavigationTarget::ClusterList.label(), "Clusters");
}

#[test]
fn test_label_dashboard_format() {
    let target = NavigationTarget::Dashboard { cluster_context: "alpha".to_string() };
    assert_eq!(target.label(), "alpha - Overview");
}

#[test]
fn test_label_resource_list_format() {
    let target = NavigationTarget::ResourceList {
        cluster_context: "beta".to_string(),
        category: ResourceCategory::Storage,
        kind: "PersistentVolumeClaim".to_string(),
    };
    assert_eq!(target.label(), "beta - PersistentVolumeClaim");
}

#[test]
fn test_label_resource_detail_format() {
    let target = NavigationTarget::ResourceDetail {
        cluster_context: "gamma".to_string(),
        kind: "Deployment".to_string(),
        name: "api-server".to_string(),
        namespace: Some("backend".to_string()),
    };
    assert_eq!(target.label(), "gamma - Deployment/api-server");
}

#[test]
fn test_label_resource_detail_no_namespace() {
    let target = NavigationTarget::ResourceDetail {
        cluster_context: "gamma".to_string(),
        kind: "Node".to_string(),
        name: "worker-1".to_string(),
        namespace: None,
    };
    assert_eq!(target.label(), "gamma - Node/worker-1");
}

#[test]
fn test_label_helm_releases_format() {
    let target = NavigationTarget::HelmReleases { cluster_context: "delta".to_string() };
    assert_eq!(target.label(), "delta - Helm Releases");
}

#[test]
fn test_label_helm_install_format() {
    let target = NavigationTarget::HelmInstall { cluster_context: "delta".to_string() };
    assert_eq!(target.label(), "delta - Install Chart");
}

#[test]
fn test_label_events_format() {
    let target = NavigationTarget::ResourceList {
        cluster_context: "epsilon".to_string(),
        category: ResourceCategory::Monitoring,
        kind: "Event".to_string(),
    };
    assert_eq!(target.label(), "epsilon - Event");
}

#[test]
fn test_label_crd_browser_format() {
    let target = NavigationTarget::CrdBrowser { cluster_context: "zeta".to_string() };
    assert_eq!(target.label(), "zeta - Custom Resources");
}

#[test]
fn test_label_namespace_map_format() {
    let target = NavigationTarget::NamespaceMap { cluster_context: "eta".to_string() };
    assert_eq!(target.label(), "eta - Resource Map");
}

#[test]
fn test_label_plugin_manager_format() {
    let target = NavigationTarget::PluginManager { cluster_context: "theta".to_string() };
    assert_eq!(target.label(), "theta - Plugins");
}

// ===================================================================
// 4. ResourceList tab for "Pod" under Workloads in cluster "prod"
// ===================================================================

#[test]
fn test_resource_list_pod_workloads_prod_tab_label() {
    let mut ws = WorkspaceState::default();
    let target = NavigationTarget::ResourceList {
        cluster_context: "prod".to_string(),
        category: ResourceCategory::Workloads,
        kind: "Pod".to_string(),
    };
    ws.open_tab(target);

    let active = ws.active_tab().expect("should have active tab");
    assert_eq!(active.label, "prod - Pod");
}

// ===================================================================
// 5. Dashboard tab for "dev" creates a tab labeled "dev - Overview"
// ===================================================================

#[test]
fn test_dashboard_dev_tab_label() {
    let mut ws = WorkspaceState::default();
    let target = NavigationTarget::Dashboard { cluster_context: "dev".to_string() };
    ws.open_tab(target);

    let active = ws.active_tab().expect("should have active tab");
    assert_eq!(active.label, "dev - Overview");
}

// ===================================================================
// 6. Opening tabs for all 11 NavigationTarget variants creates
//    11 distinct tabs
// ===================================================================

#[test]
fn test_all_10_variants_create_10_distinct_tabs() {
    let mut ws = WorkspaceState::default();
    let targets = all_targets("multi");

    assert_eq!(targets.len(), 11, "Should have exactly 11 NavigationTarget variants");

    for target in targets {
        ws.open_tab(target);
    }

    assert_eq!(ws.tab_count(), 11);

    // All tab IDs should be unique
    let ids: HashSet<_> = ws.tabs.iter().map(|t| t.id).collect();
    assert_eq!(ids.len(), 11);

    // All tab labels should be unique
    let labels: HashSet<_> = ws.tabs.iter().map(|t| t.label.clone()).collect();
    assert_eq!(labels.len(), 11);
}

#[test]
fn test_all_10_variants_have_expected_labels() {
    let mut ws = WorkspaceState::default();
    let targets = all_targets("ctx");

    for target in targets {
        ws.open_tab(target);
    }

    let labels: Vec<&str> = ws.tabs.iter().map(|t| t.label.as_str()).collect();
    assert!(labels.contains(&"Clusters"));
    assert!(labels.contains(&"ctx - Overview"));
    assert!(labels.contains(&"ctx - Pod"));
    assert!(labels.contains(&"ctx - Pod/nginx"));
    assert!(labels.contains(&"ctx - Helm Releases"));
    assert!(labels.contains(&"ctx - Install Chart"));
    assert!(labels.contains(&"ctx - Event"));
    assert!(labels.contains(&"ctx - Custom Resources"));
    assert!(labels.contains(&"ctx - Resource Map"));
    assert!(labels.contains(&"ctx - Plugins"));
    assert!(labels.contains(&"Preferences"));
}

// ===================================================================
// 7. WorkspaceState with multiple open tabs correctly tracks the
//    active tab
// ===================================================================

#[test]
fn test_active_tab_tracks_most_recently_opened() {
    let mut ws = WorkspaceState::default();

    let id1 = ws.open_tab(NavigationTarget::ClusterList);
    assert_eq!(ws.active_tab_id, Some(id1));
    assert_eq!(ws.active_tab().unwrap().label, "Clusters");

    let id2 = ws.open_tab(NavigationTarget::Dashboard { cluster_context: "prod".to_string() });
    assert_eq!(ws.active_tab_id, Some(id2));
    assert_eq!(ws.active_tab().unwrap().label, "prod - Overview");

    let id3 = ws.open_tab(NavigationTarget::ResourceList {
        cluster_context: "prod".to_string(),
        category: ResourceCategory::Monitoring,
        kind: "Event".to_string(),
    });
    assert_eq!(ws.active_tab_id, Some(id3));
    assert_eq!(ws.active_tab().unwrap().label, "prod - Event");
}

#[test]
fn test_activate_tab_switches_active() {
    let mut ws = WorkspaceState::default();

    let id1 = ws.open_tab(NavigationTarget::ClusterList);
    let id2 = ws.open_tab(NavigationTarget::Dashboard { cluster_context: "prod".to_string() });
    let _id3 = ws.open_tab(NavigationTarget::ResourceList {
        cluster_context: "prod".to_string(),
        category: ResourceCategory::Monitoring,
        kind: "Event".to_string(),
    });

    // Switch back to tab 1
    assert!(ws.activate_tab(id1));
    assert_eq!(ws.active_tab_id, Some(id1));
    assert_eq!(ws.active_tab().unwrap().label, "Clusters");

    // Switch to tab 2
    assert!(ws.activate_tab(id2));
    assert_eq!(ws.active_tab_id, Some(id2));
    assert_eq!(ws.active_tab().unwrap().label, "prod - Overview");
}

#[test]
fn test_activate_nonexistent_tab_returns_false() {
    let mut ws = WorkspaceState::default();
    let id = ws.open_tab(NavigationTarget::ClusterList);
    let fake_id = uuid::Uuid::new_v4();

    assert!(!ws.activate_tab(fake_id));
    // Active tab should remain unchanged
    assert_eq!(ws.active_tab_id, Some(id));
}

// ===================================================================
// 8. Closing the active tab selects a neighboring tab
// ===================================================================

#[test]
fn test_close_active_tab_selects_previous_neighbor() {
    let mut ws = WorkspaceState::default();

    let _id1 = ws.open_tab(NavigationTarget::ClusterList);
    let id2 = ws.open_tab(NavigationTarget::Dashboard { cluster_context: "prod".to_string() });
    let id3 = ws.open_tab(NavigationTarget::ResourceList {
        cluster_context: "prod".to_string(),
        category: ResourceCategory::Monitoring,
        kind: "Event".to_string(),
    });

    // Active is id3 (Event). Close it.
    assert!(ws.close_tab(id3));
    assert_eq!(ws.tab_count(), 2);

    // Should activate the neighbor — which is id2 (Dashboard) since id3 was at end
    assert_eq!(ws.active_tab_id, Some(id2));
    assert_eq!(ws.active_tab().unwrap().label, "prod - Overview");
}

#[test]
fn test_close_middle_active_tab_selects_neighbor() {
    let mut ws = WorkspaceState::default();

    let _id1 = ws.open_tab(NavigationTarget::ClusterList);
    let id2 = ws.open_tab(NavigationTarget::Dashboard { cluster_context: "prod".to_string() });
    let id3 = ws.open_tab(NavigationTarget::ResourceList {
        cluster_context: "prod".to_string(),
        category: ResourceCategory::Monitoring,
        kind: "Event".to_string(),
    });

    // Activate the middle tab (Dashboard)
    ws.activate_tab(id2);
    assert_eq!(ws.active_tab_id, Some(id2));

    // Close the middle tab
    assert!(ws.close_tab(id2));
    assert_eq!(ws.tab_count(), 2);

    // Should select the tab now at the same index position (Event moved to idx=1)
    assert_eq!(ws.active_tab_id, Some(id3));
}

#[test]
fn test_close_first_active_tab_selects_next() {
    let mut ws = WorkspaceState::default();

    let id1 = ws.open_tab(NavigationTarget::ClusterList);
    let id2 = ws.open_tab(NavigationTarget::Dashboard { cluster_context: "prod".to_string() });
    let _id3 = ws.open_tab(NavigationTarget::ResourceList {
        cluster_context: "prod".to_string(),
        category: ResourceCategory::Monitoring,
        kind: "Event".to_string(),
    });

    // Activate the first tab
    ws.activate_tab(id1);

    // Close it
    assert!(ws.close_tab(id1));
    assert_eq!(ws.tab_count(), 2);

    // Should activate the tab now at index 0 (Dashboard)
    assert_eq!(ws.active_tab_id, Some(id2));
}

#[test]
fn test_close_last_remaining_tab_sets_active_to_none() {
    let mut ws = WorkspaceState::default();
    let id = ws.open_tab(NavigationTarget::ClusterList);

    assert!(ws.close_tab(id));
    assert_eq!(ws.tab_count(), 0);
    assert!(ws.active_tab_id.is_none());
    assert!(ws.active_tab().is_none());
}

#[test]
fn test_close_non_active_tab_does_not_change_active() {
    let mut ws = WorkspaceState::default();

    let id1 = ws.open_tab(NavigationTarget::ClusterList);
    let _id2 = ws.open_tab(NavigationTarget::Dashboard { cluster_context: "prod".to_string() });
    let id3 = ws.open_tab(NavigationTarget::ResourceList {
        cluster_context: "prod".to_string(),
        category: ResourceCategory::Monitoring,
        kind: "Event".to_string(),
    });

    // Active is id3 (Event). Close id1 (Clusters) which is not active.
    assert!(ws.close_tab(id1));
    assert_eq!(ws.tab_count(), 2);
    // Active tab should remain Event
    assert_eq!(ws.active_tab_id, Some(id3));
    assert_eq!(ws.active_tab().unwrap().label, "prod - Event");
}

// ===================================================================
// 9. Tab uniqueness: opening the same NavigationTarget twice should
//    reuse the existing tab
// ===================================================================

#[test]
fn test_open_same_target_twice_reuses_tab() {
    let mut ws = WorkspaceState::default();

    let target = NavigationTarget::Dashboard { cluster_context: "prod".to_string() };

    let id1 = ws.open_tab(target.clone());
    let id2 = ws.open_tab(target);

    assert_eq!(id1, id2, "Opening the same target should return the same tab ID");
    assert_eq!(ws.tab_count(), 1, "Should not create a second tab");
}

#[test]
fn test_open_same_resource_list_twice_reuses_tab() {
    let mut ws = WorkspaceState::default();

    let target = NavigationTarget::ResourceList {
        cluster_context: "staging".to_string(),
        category: ResourceCategory::Workloads,
        kind: "Pod".to_string(),
    };

    let id1 = ws.open_tab(target.clone());
    // Open another tab in between
    ws.open_tab(NavigationTarget::ResourceList {
        cluster_context: "staging".to_string(),
        category: ResourceCategory::Monitoring,
        kind: "Event".to_string(),
    });
    let id2 = ws.open_tab(target);

    assert_eq!(id1, id2);
    assert_eq!(ws.tab_count(), 2); // Pod + Event, not Pod + Event + Pod
    // Active should switch back to the Pod tab
    assert_eq!(ws.active_tab_id, Some(id1));
}

#[test]
fn test_open_different_clusters_same_variant_creates_separate_tabs() {
    let mut ws = WorkspaceState::default();

    let id1 = ws.open_tab(NavigationTarget::Dashboard { cluster_context: "prod".to_string() });
    let id2 = ws.open_tab(NavigationTarget::Dashboard { cluster_context: "staging".to_string() });

    assert_ne!(id1, id2);
    assert_eq!(ws.tab_count(), 2);
}

#[test]
fn test_open_different_kinds_same_cluster_creates_separate_tabs() {
    let mut ws = WorkspaceState::default();

    let id1 = ws.open_tab(NavigationTarget::ResourceList {
        cluster_context: "prod".to_string(),
        category: ResourceCategory::Workloads,
        kind: "Pod".to_string(),
    });
    let id2 = ws.open_tab(NavigationTarget::ResourceList {
        cluster_context: "prod".to_string(),
        category: ResourceCategory::Workloads,
        kind: "Deployment".to_string(),
    });

    assert_ne!(id1, id2);
    assert_eq!(ws.tab_count(), 2);
}

#[test]
fn test_reopen_closed_tab_creates_new_tab_with_new_id() {
    let mut ws = WorkspaceState::default();

    let target = NavigationTarget::ResourceList {
        cluster_context: "prod".to_string(),
        category: ResourceCategory::Monitoring,
        kind: "Event".to_string(),
    };
    let id1 = ws.open_tab(target.clone());

    // Close it
    ws.close_tab(id1);
    assert_eq!(ws.tab_count(), 0);

    // Re-open the same target
    let id2 = ws.open_tab(target);
    assert_ne!(id1, id2, "Re-opened tab should get a fresh UUID");
    assert_eq!(ws.tab_count(), 1);
    assert_eq!(ws.active_tab().unwrap().label, "prod - Event");
}

#[test]
fn test_open_all_10_then_reopen_each_reuses_all() {
    let mut ws = WorkspaceState::default();
    let targets = all_targets("reuse-test");

    // Open all 10
    let mut ids = Vec::new();
    for target in &targets {
        ids.push(ws.open_tab(target.clone()));
    }
    assert_eq!(ws.tab_count(), 11);

    // Re-open each one — tab count should stay at 11
    for (i, target) in targets.iter().enumerate() {
        let reopen_id = ws.open_tab(target.clone());
        assert_eq!(
            reopen_id, ids[i],
            "Re-opening target {:?} should return the same tab ID",
            target
        );
    }
    assert_eq!(ws.tab_count(), 11, "No new tabs should be created");
}
