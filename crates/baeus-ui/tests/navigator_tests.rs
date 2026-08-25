// Navigator tree rendering tests (T304)
//
// Tests for the Navigator sidebar that shows ALL clusters as expandable tree nodes.
// Each cluster has categories (Workloads, Config, Network, etc.) that expand to show
// resource types.

use baeus_ui::icons::ResourceCategory;
use baeus_ui::layout::NavigationTarget;
use baeus_ui::layout::sidebar::*;
use uuid::Uuid;

// ===================================================================
// 1. Cluster nodes structure
// ===================================================================

#[test]
fn test_cluster_entry_has_expected_fields() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("prod-us-east", "Production US East");

    let cluster = state.clusters.iter().find(|c| c.id == id).unwrap();
    assert_eq!(cluster.context_name, "prod-us-east");
    assert_eq!(cluster.display_name, "Production US East");
    assert_eq!(cluster.initials, "PU"); // prod-us-east => P + U
    assert_ne!(cluster.color, 0);
    assert_eq!(cluster.status, ClusterStatus::Disconnected);
    assert!(!cluster.expanded); // collapsed by default
}

#[test]
fn test_multiple_clusters_can_be_iterated() {
    let mut state = SidebarState::default();
    state.add_cluster("kind-dev", "kind-dev");
    state.add_cluster("prod", "production");
    state.add_cluster("staging", "staging-eu");

    assert_eq!(state.clusters.len(), 3);

    let context_names: Vec<&str> = state.clusters.iter().map(|c| c.context_name.as_str()).collect();
    assert!(context_names.contains(&"kind-dev"));
    assert!(context_names.contains(&"prod"));
    assert!(context_names.contains(&"staging"));
}

#[test]
fn test_each_cluster_has_its_own_sections() {
    let mut state = SidebarState::default();
    let id1 = state.add_cluster("kind-dev", "kind-dev");
    let id2 = state.add_cluster("prod", "production");

    let c1 = state.clusters.iter().find(|c| c.id == id1).unwrap();
    let c2 = state.clusters.iter().find(|c| c.id == id2).unwrap();

    assert!(!c1.sections.is_empty());
    assert!(!c2.sections.is_empty());
    // Both clusters should have the same number of sections
    assert_eq!(c1.sections.len(), c2.sections.len());
}

#[test]
fn test_cluster_initials_generation() {
    assert_eq!(generate_initials("kind-dev"), "KD");
    assert_eq!(generate_initials("prod-us-east"), "PU");
    assert_eq!(generate_initials("my_cluster"), "MC");
    assert_eq!(generate_initials("staging.eu"), "SE");
    assert_eq!(generate_initials("production"), "PR");
    assert_eq!(generate_initials("a"), "AA");
    assert_eq!(generate_initials(""), "??");
}

#[test]
fn test_cluster_color_is_deterministic() {
    let c1 = generate_cluster_color("kind-dev");
    let c2 = generate_cluster_color("kind-dev");
    assert_eq!(c1, c2);
}

#[test]
fn test_cluster_color_varies_by_name() {
    let c1 = generate_cluster_color("kind-dev");
    let c2 = generate_cluster_color("prod-us-east");
    assert_ne!(c1, 0);
    assert_ne!(c2, 0);
    // Different names should generally produce different colors
}

// ===================================================================
// 2. Expand/collapse clusters
// ===================================================================

#[test]
fn test_toggle_cluster_expand_expands() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("kind-dev", "kind-dev");
    assert!(!state.clusters[0].expanded);

    state.toggle_cluster_expand(id);
    assert!(state.clusters[0].expanded);
}

#[test]
fn test_toggle_cluster_expand_collapses_again() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("kind-dev", "kind-dev");

    state.toggle_cluster_expand(id);
    assert!(state.clusters[0].expanded);

    state.toggle_cluster_expand(id);
    assert!(!state.clusters[0].expanded);
}

#[test]
fn test_multiple_clusters_expand_independently() {
    let mut state = SidebarState::default();
    let id1 = state.add_cluster("kind-dev", "kind-dev");
    let id2 = state.add_cluster("prod", "production");

    // Both start collapsed
    assert!(!state.clusters[0].expanded);
    assert!(!state.clusters[1].expanded);

    // Expand only the first cluster
    state.toggle_cluster_expand(id1);
    assert!(state.clusters.iter().find(|c| c.id == id1).unwrap().expanded);
    assert!(!state.clusters.iter().find(|c| c.id == id2).unwrap().expanded);

    // Expand the second cluster
    state.toggle_cluster_expand(id2);
    assert!(state.clusters.iter().find(|c| c.id == id1).unwrap().expanded);
    assert!(state.clusters.iter().find(|c| c.id == id2).unwrap().expanded);

    // Collapse the first cluster
    state.toggle_cluster_expand(id1);
    assert!(!state.clusters.iter().find(|c| c.id == id1).unwrap().expanded);
    assert!(state.clusters.iter().find(|c| c.id == id2).unwrap().expanded);
}

#[test]
fn test_toggle_cluster_expand_nonexistent_is_noop() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("kind-dev", "kind-dev");
    let fake_id = Uuid::new_v4();

    state.toggle_cluster_expand(fake_id);
    // Original cluster unaffected (still collapsed)
    assert!(!state.clusters.iter().find(|c| c.id == id).unwrap().expanded);
}

// ===================================================================
// 3. Category expansion within clusters
// ===================================================================

#[test]
fn test_new_cluster_no_expanded_categories() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("kind-dev", "kind-dev");
    assert!(!state.is_category_expanded(id, ResourceCategory::Workloads));
    assert!(!state.is_category_expanded(id, ResourceCategory::Network));
}

#[test]
fn test_toggle_category_expand() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("kind-dev", "kind-dev");

    state.toggle_category_expand(id, ResourceCategory::Workloads);
    assert!(state.is_category_expanded(id, ResourceCategory::Workloads));

    state.toggle_category_expand(id, ResourceCategory::Workloads);
    assert!(!state.is_category_expanded(id, ResourceCategory::Workloads));
}

#[test]
fn test_categories_expand_independently_within_cluster() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("kind-dev", "kind-dev");

    state.toggle_category_expand(id, ResourceCategory::Workloads);
    state.toggle_category_expand(id, ResourceCategory::Network);

    assert!(state.is_category_expanded(id, ResourceCategory::Workloads));
    assert!(state.is_category_expanded(id, ResourceCategory::Network));
    assert!(!state.is_category_expanded(id, ResourceCategory::Storage));

    // Collapse Workloads, Network stays expanded
    state.toggle_category_expand(id, ResourceCategory::Workloads);
    assert!(!state.is_category_expanded(id, ResourceCategory::Workloads));
    assert!(state.is_category_expanded(id, ResourceCategory::Network));
}

#[test]
fn test_categories_independent_between_clusters() {
    let mut state = SidebarState::default();
    let id1 = state.add_cluster("kind-dev", "kind-dev");
    let id2 = state.add_cluster("prod", "production");

    state.toggle_category_expand(id1, ResourceCategory::Workloads);
    assert!(state.is_category_expanded(id1, ResourceCategory::Workloads));
    assert!(!state.is_category_expanded(id2, ResourceCategory::Workloads));

    state.toggle_category_expand(id2, ResourceCategory::Storage);
    assert!(!state.is_category_expanded(id1, ResourceCategory::Storage));
    assert!(state.is_category_expanded(id2, ResourceCategory::Storage));
}

#[test]
fn test_toggle_category_on_nonexistent_cluster_is_noop() {
    let mut state = SidebarState::default();
    let fake_id = Uuid::new_v4();
    state.toggle_category_expand(fake_id, ResourceCategory::Workloads);
    assert!(!state.is_category_expanded(fake_id, ResourceCategory::Workloads));
}

// ===================================================================
// 4. Resource type navigation targets
// ===================================================================

#[test]
fn test_navigation_target_for_pods_under_workloads() {
    let target = NavigationTarget::ResourceList {
        cluster_context: "prod".to_string(),
        category: ResourceCategory::Workloads,
        kind: "Pod".to_string(),
    };

    assert_eq!(target.cluster_context(), Some("prod"));
    if let NavigationTarget::ResourceList { cluster_context, category, kind } = &target {
        assert_eq!(cluster_context, "prod");
        assert_eq!(*category, ResourceCategory::Workloads);
        assert_eq!(kind, "Pod");
    } else {
        panic!("Expected ResourceList navigation target");
    }
}

#[test]
fn test_sidebar_item_produces_correct_navigation_target() {
    let item = SidebarItem {
        icon: baeus_ui::icons::ResourceIcon::Pod,
        label: "Pods".to_string(),
        kind: "Pod".to_string(),
        badge_count: None,
    };

    let target = item.navigation_target(ResourceCategory::Workloads, "prod");
    if let NavigationTarget::ResourceList { cluster_context, category, kind } = target {
        assert_eq!(cluster_context, "prod");
        assert_eq!(category, ResourceCategory::Workloads);
        assert_eq!(kind, "Pod");
    } else {
        panic!("Expected ResourceList navigation target");
    }
}

#[test]
fn test_navigate_to_kind_produces_resource_list() {
    let mut state = SidebarState::default();
    let target = state.navigate_to_kind("Pod", "prod");
    assert!(target.is_some());
    if let Some(NavigationTarget::ResourceList { cluster_context, category, kind }) = target {
        assert_eq!(cluster_context, "prod");
        assert_eq!(category, ResourceCategory::Workloads);
        assert_eq!(kind, "Pod");
    }
}

#[test]
fn test_navigate_to_service_in_network_category() {
    let mut state = SidebarState::default();
    let target = state.navigate_to_kind("Service", "staging");
    assert!(target.is_some());
    if let Some(NavigationTarget::ResourceList { cluster_context, category, kind }) = target {
        assert_eq!(cluster_context, "staging");
        assert_eq!(category, ResourceCategory::Network);
        assert_eq!(kind, "Service");
    }
}

// ===================================================================
// 5. FR-071 full resource tree
// ===================================================================

#[test]
fn test_fr071_categories_returns_11() {
    let categories = SidebarState::fr071_categories();
    assert_eq!(categories.len(), 11);
}

#[test]
fn test_fr071_categories_contains_all_required() {
    let categories = SidebarState::fr071_categories();
    assert!(categories.contains(&ResourceCategory::Workloads));
    assert!(categories.contains(&ResourceCategory::Configuration));
    assert!(categories.contains(&ResourceCategory::Network));
    assert!(categories.contains(&ResourceCategory::Storage));
    assert!(categories.contains(&ResourceCategory::Cluster));
    assert!(categories.contains(&ResourceCategory::Monitoring));
    assert!(categories.contains(&ResourceCategory::Helm));
    assert!(categories.contains(&ResourceCategory::Rbac));
    assert!(categories.contains(&ResourceCategory::Custom));
    assert!(categories.contains(&ResourceCategory::Plugins));
}

#[test]
fn test_fr071_workloads_has_8_kinds() {
    let kinds = SidebarState::fr071_category_kinds(ResourceCategory::Workloads);
    assert_eq!(kinds.len(), 8);
    assert!(kinds.contains(&"Pod"));
    assert!(kinds.contains(&"Deployment"));
    assert!(kinds.contains(&"DaemonSet"));
    assert!(kinds.contains(&"StatefulSet"));
    assert!(kinds.contains(&"ReplicaSet"));
    assert!(kinds.contains(&"ReplicationController"));
    assert!(kinds.contains(&"Job"));
    assert!(kinds.contains(&"CronJob"));
}

#[test]
fn test_fr071_configuration_has_12_kinds() {
    let kinds = SidebarState::fr071_category_kinds(ResourceCategory::Configuration);
    assert_eq!(kinds.len(), 12);
    assert!(kinds.contains(&"ConfigMap"));
    assert!(kinds.contains(&"Secret"));
    assert!(kinds.contains(&"ResourceQuota"));
    assert!(kinds.contains(&"LimitRange"));
    assert!(kinds.contains(&"HorizontalPodAutoscaler"));
    assert!(kinds.contains(&"VerticalPodAutoscaler"));
    assert!(kinds.contains(&"PodDisruptionBudget"));
    assert!(kinds.contains(&"PriorityClass"));
    assert!(kinds.contains(&"RuntimeClass"));
    assert!(kinds.contains(&"Lease"));
    assert!(kinds.contains(&"MutatingWebhookConfiguration"));
    assert!(kinds.contains(&"ValidatingWebhookConfiguration"));
}

#[test]
fn test_fr071_network_has_6_kinds() {
    let kinds = SidebarState::fr071_category_kinds(ResourceCategory::Network);
    assert_eq!(kinds.len(), 6);
    assert!(kinds.contains(&"Service"));
    assert!(kinds.contains(&"Endpoints"));
    assert!(kinds.contains(&"Ingress"));
    assert!(kinds.contains(&"IngressClass"));
    assert!(kinds.contains(&"NetworkPolicy"));
    assert!(kinds.contains(&"PortForwarding"));
}

#[test]
fn test_fr071_storage_has_3_kinds() {
    let kinds = SidebarState::fr071_category_kinds(ResourceCategory::Storage);
    assert_eq!(kinds.len(), 3);
    assert!(kinds.contains(&"PersistentVolumeClaim"));
    assert!(kinds.contains(&"PersistentVolume"));
    assert!(kinds.contains(&"StorageClass"));
}

#[test]
fn test_fr071_cluster_has_2_kinds() {
    let kinds = SidebarState::fr071_category_kinds(ResourceCategory::Cluster);
    assert_eq!(kinds.len(), 2);
    assert!(kinds.contains(&"Namespace"));
    assert!(kinds.contains(&"Node"));
}

#[test]
fn test_fr071_monitoring_has_event() {
    let kinds = SidebarState::fr071_category_kinds(ResourceCategory::Monitoring);
    assert_eq!(kinds.len(), 1);
    assert!(kinds.contains(&"Event"));
}

#[test]
fn test_fr071_helm_has_2_kinds() {
    let kinds = SidebarState::fr071_category_kinds(ResourceCategory::Helm);
    assert_eq!(kinds.len(), 2);
    assert!(kinds.contains(&"HelmChart"));
    assert!(kinds.contains(&"HelmRelease"));
}

#[test]
fn test_fr071_rbac_has_6_kinds() {
    let kinds = SidebarState::fr071_category_kinds(ResourceCategory::Rbac);
    assert_eq!(kinds.len(), 6);
    assert!(kinds.contains(&"ServiceAccount"));
    assert!(kinds.contains(&"ClusterRole"));
    assert!(kinds.contains(&"Role"));
    assert!(kinds.contains(&"ClusterRoleBinding"));
    assert!(kinds.contains(&"RoleBinding"));
    assert!(kinds.contains(&"PodSecurityPolicy"));
}

#[test]
fn test_fr071_custom_has_crd() {
    let kinds = SidebarState::fr071_category_kinds(ResourceCategory::Custom);
    assert_eq!(kinds.len(), 1);
    assert!(kinds.contains(&"CustomResourceDefinition"));
}

#[test]
fn test_fr071_plugins_has_plugin() {
    let kinds = SidebarState::fr071_category_kinds(ResourceCategory::Plugins);
    assert_eq!(kinds.len(), 1);
    assert!(kinds.contains(&"Plugin"));
}

#[test]
fn test_all_fr071_categories_independently_expandable_per_cluster() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("kind-dev", "kind-dev");

    // Expand every category
    for &cat in SidebarState::fr071_categories() {
        state.toggle_category_expand(id, cat);
    }

    // All should be expanded
    for &cat in SidebarState::fr071_categories() {
        assert!(state.is_category_expanded(id, cat), "Category {:?} should be expanded", cat);
    }

    // Collapse every category
    for &cat in SidebarState::fr071_categories() {
        state.toggle_category_expand(id, cat);
    }

    // All should be collapsed again
    for &cat in SidebarState::fr071_categories() {
        assert!(!state.is_category_expanded(id, cat), "Category {:?} should be collapsed", cat);
    }
}

// ===================================================================
// 6. Contextual tracking (FR-070)
// ===================================================================

#[test]
fn test_navigation_target_cluster_context_for_resource_list() {
    let target = NavigationTarget::ResourceList {
        cluster_context: "prod".to_string(),
        category: ResourceCategory::Workloads,
        kind: "Pod".to_string(),
    };
    assert_eq!(target.cluster_context(), Some("prod"));
}

#[test]
fn test_navigation_target_cluster_context_for_dashboard() {
    let target = NavigationTarget::Dashboard { cluster_context: "staging".to_string() };
    assert_eq!(target.cluster_context(), Some("staging"));
}

#[test]
fn test_navigation_target_cluster_list_has_no_context() {
    let target = NavigationTarget::ClusterList;
    assert_eq!(target.cluster_context(), None);
}

#[test]
fn test_navigation_target_identifies_active_cluster_and_category() {
    // Given an active NavigationTarget, we can determine which cluster and category
    // should be highlighted.
    let target = NavigationTarget::ResourceList {
        cluster_context: "prod".to_string(),
        category: ResourceCategory::Workloads,
        kind: "Pod".to_string(),
    };

    // Determine which cluster should be highlighted
    assert_eq!(target.cluster_context(), Some("prod"));

    // Determine which category should be highlighted
    if let NavigationTarget::ResourceList { category, .. } = &target {
        assert_eq!(*category, ResourceCategory::Workloads);
    }
}

#[test]
fn test_navigation_target_label_includes_cluster_prefix() {
    let target = NavigationTarget::ResourceList {
        cluster_context: "prod".to_string(),
        category: ResourceCategory::Workloads,
        kind: "Pod".to_string(),
    };
    assert_eq!(target.label(), "prod - Pod");
}

#[test]
fn test_navigation_target_various_contexts() {
    // Verify cluster_context works for all cluster-scoped variants
    let targets_with_context = vec![
        NavigationTarget::Dashboard { cluster_context: "a".to_string() },
        NavigationTarget::ResourceList {
            cluster_context: "b".to_string(),
            category: ResourceCategory::Workloads,
            kind: "Pod".to_string(),
        },
        NavigationTarget::ResourceDetail {
            cluster_context: "c".to_string(),
            kind: "Pod".to_string(),
            name: "nginx".to_string(),
            namespace: Some("default".to_string()),
        },
        NavigationTarget::HelmReleases { cluster_context: "d".to_string() },
        NavigationTarget::HelmInstall { cluster_context: "e".to_string() },
        NavigationTarget::ResourceList {
            cluster_context: "f".to_string(),
            category: ResourceCategory::Monitoring,
            kind: "Event".to_string(),
        },
        NavigationTarget::CrdBrowser { cluster_context: "g".to_string() },
        NavigationTarget::NamespaceMap { cluster_context: "h".to_string() },
        NavigationTarget::PluginManager { cluster_context: "i".to_string() },
    ];

    let expected_contexts = vec!["a", "b", "c", "d", "e", "f", "g", "h", "i"];

    for (target, expected) in targets_with_context.iter().zip(expected_contexts.iter()) {
        assert_eq!(
            target.cluster_context(),
            Some(*expected),
            "Expected context {:?} for {:?}",
            expected,
            target
        );
    }
}

// ===================================================================
// 7. Cluster status rendering data
// ===================================================================

#[test]
fn test_new_cluster_starts_disconnected() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("kind-dev", "kind-dev");
    let cluster = state.clusters.iter().find(|c| c.id == id).unwrap();
    assert_eq!(cluster.status, ClusterStatus::Disconnected);
}

#[test]
fn test_cluster_status_can_be_set_to_connected() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("kind-dev", "kind-dev");
    state.clusters.iter_mut().find(|c| c.id == id).unwrap().status = ClusterStatus::Connected;

    let cluster = state.clusters.iter().find(|c| c.id == id).unwrap();
    assert_eq!(cluster.status, ClusterStatus::Connected);
}

#[test]
fn test_cluster_status_can_be_set_to_connecting() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("kind-dev", "kind-dev");
    state.clusters.iter_mut().find(|c| c.id == id).unwrap().status = ClusterStatus::Connecting;

    let cluster = state.clusters.iter().find(|c| c.id == id).unwrap();
    assert_eq!(cluster.status, ClusterStatus::Connecting);
}

#[test]
fn test_cluster_status_can_be_set_to_error() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("kind-dev", "kind-dev");
    state.clusters.iter_mut().find(|c| c.id == id).unwrap().status = ClusterStatus::Error;

    let cluster = state.clusters.iter().find(|c| c.id == id).unwrap();
    assert_eq!(cluster.status, ClusterStatus::Error);
}

#[test]
fn test_cluster_status_tracked_per_cluster() {
    let mut state = SidebarState::default();
    let id1 = state.add_cluster("kind-dev", "kind-dev");
    let id2 = state.add_cluster("prod", "production");

    state.clusters.iter_mut().find(|c| c.id == id1).unwrap().status = ClusterStatus::Connected;
    state.clusters.iter_mut().find(|c| c.id == id2).unwrap().status = ClusterStatus::Error;

    let c1 = state.clusters.iter().find(|c| c.id == id1).unwrap();
    let c2 = state.clusters.iter().find(|c| c.id == id2).unwrap();
    assert_eq!(c1.status, ClusterStatus::Connected);
    assert_eq!(c2.status, ClusterStatus::Error);
}

#[test]
fn test_cluster_status_equality_variants() {
    assert_eq!(ClusterStatus::Connected, ClusterStatus::Connected);
    assert_eq!(ClusterStatus::Disconnected, ClusterStatus::Disconnected);
    assert_eq!(ClusterStatus::Connecting, ClusterStatus::Connecting);
    assert_eq!(ClusterStatus::Error, ClusterStatus::Error);
    assert_ne!(ClusterStatus::Connected, ClusterStatus::Disconnected);
    assert_ne!(ClusterStatus::Connecting, ClusterStatus::Error);
}

// ===================================================================
// 8. Badge counts
// ===================================================================

#[test]
fn test_update_cluster_badge_sets_count() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("kind-dev", "kind-dev");

    state.update_cluster_badge(id, "Pod", Some(42));

    let cluster = state.clusters.iter().find(|c| c.id == id).unwrap();
    let workloads =
        cluster.sections.iter().find(|s| s.category == ResourceCategory::Workloads).unwrap();
    let pod_item = workloads.items.iter().find(|i| i.kind == "Pod").unwrap();
    assert_eq!(pod_item.badge_count, Some(42));
}

#[test]
fn test_update_cluster_badge_clears_count() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("kind-dev", "kind-dev");

    state.update_cluster_badge(id, "Pod", Some(42));
    state.update_cluster_badge(id, "Pod", None);

    let cluster = state.clusters.iter().find(|c| c.id == id).unwrap();
    let workloads =
        cluster.sections.iter().find(|s| s.category == ResourceCategory::Workloads).unwrap();
    let pod_item = workloads.items.iter().find(|i| i.kind == "Pod").unwrap();
    assert_eq!(pod_item.badge_count, None);
}

#[test]
fn test_update_cluster_badge_isolated_per_cluster() {
    let mut state = SidebarState::default();
    let id1 = state.add_cluster("kind-dev", "kind-dev");
    let id2 = state.add_cluster("prod", "production");

    state.update_cluster_badge(id1, "Pod", Some(10));
    state.update_cluster_badge(id2, "Pod", Some(200));

    let c1 = state.clusters.iter().find(|c| c.id == id1).unwrap();
    let c1_pods = c1
        .sections
        .iter()
        .find(|s| s.category == ResourceCategory::Workloads)
        .unwrap()
        .items
        .iter()
        .find(|i| i.kind == "Pod")
        .unwrap();
    assert_eq!(c1_pods.badge_count, Some(10));

    let c2 = state.clusters.iter().find(|c| c.id == id2).unwrap();
    let c2_pods = c2
        .sections
        .iter()
        .find(|s| s.category == ResourceCategory::Workloads)
        .unwrap()
        .items
        .iter()
        .find(|i| i.kind == "Pod")
        .unwrap();
    assert_eq!(c2_pods.badge_count, Some(200));
}

#[test]
fn test_update_cluster_badge_on_nonexistent_cluster_is_noop() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("kind-dev", "kind-dev");

    // Should not panic
    state.update_cluster_badge(Uuid::new_v4(), "Pod", Some(99));

    // Original cluster unaffected
    let cluster = state.clusters.iter().find(|c| c.id == id).unwrap();
    let pods = cluster
        .sections
        .iter()
        .find(|s| s.category == ResourceCategory::Workloads)
        .unwrap()
        .items
        .iter()
        .find(|i| i.kind == "Pod")
        .unwrap();
    assert_eq!(pods.badge_count, None);
}

#[test]
fn test_update_cluster_badge_multiple_kinds() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("kind-dev", "kind-dev");

    state.update_cluster_badge(id, "Pod", Some(5));
    state.update_cluster_badge(id, "Deployment", Some(3));
    state.update_cluster_badge(id, "Service", Some(12));

    let cluster = state.clusters.iter().find(|c| c.id == id).unwrap();

    let workloads =
        cluster.sections.iter().find(|s| s.category == ResourceCategory::Workloads).unwrap();
    assert_eq!(workloads.items.iter().find(|i| i.kind == "Pod").unwrap().badge_count, Some(5));
    assert_eq!(
        workloads.items.iter().find(|i| i.kind == "Deployment").unwrap().badge_count,
        Some(3)
    );

    let network =
        cluster.sections.iter().find(|s| s.category == ResourceCategory::Network).unwrap();
    assert_eq!(network.items.iter().find(|i| i.kind == "Service").unwrap().badge_count, Some(12));
}

// ===================================================================
// 9. Drill-into mode (T306)
// ===================================================================

#[test]
fn test_default_not_in_drill_into_mode() {
    let state = SidebarState::default();
    assert!(!state.is_drill_into());
    assert!(state.drill_into_cluster.is_none());
}

#[test]
fn test_enter_drill_into() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("kind-dev", "kind-dev");

    state.enter_drill_into(id);
    assert!(state.is_drill_into());
    assert_eq!(state.drill_into_cluster, Some(id));
}

#[test]
fn test_exit_drill_into() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("kind-dev", "kind-dev");

    state.enter_drill_into(id);
    assert!(state.is_drill_into());

    state.exit_drill_into();
    assert!(!state.is_drill_into());
    assert!(state.drill_into_cluster.is_none());
}

#[test]
fn test_drill_into_switches_cluster() {
    let mut state = SidebarState::default();
    let id1 = state.add_cluster("kind-dev", "kind-dev");
    let id2 = state.add_cluster("prod", "production");

    state.enter_drill_into(id1);
    assert_eq!(state.drill_into_cluster, Some(id1));

    state.enter_drill_into(id2);
    assert_eq!(state.drill_into_cluster, Some(id2));
}

#[test]
fn test_drill_into_and_exit_round_trip() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("kind-dev", "kind-dev");

    assert!(!state.is_drill_into());
    state.enter_drill_into(id);
    assert!(state.is_drill_into());
    state.exit_drill_into();
    assert!(!state.is_drill_into());
}

#[test]
fn test_exit_drill_into_when_not_in_drill_into_is_noop() {
    let mut state = SidebarState::default();
    state.add_cluster("kind-dev", "kind-dev");

    // Should not panic
    state.exit_drill_into();
    assert!(!state.is_drill_into());
}

// ===================================================================
// 10. Sidebar width (T306)
// ===================================================================

#[test]
fn test_default_sidebar_width() {
    let state = SidebarState::default();
    assert_eq!(state.sidebar_width, 260.0);
}

#[test]
fn test_set_width_normal_value() {
    let mut state = SidebarState::default();
    state.set_width(350.0);
    assert_eq!(state.sidebar_width, 350.0);
}

#[test]
fn test_set_width_below_minimum_clamped() {
    let mut state = SidebarState::default();
    state.set_width(100.0);
    assert_eq!(state.sidebar_width, 200.0);
}

#[test]
fn test_set_width_at_minimum_boundary() {
    let mut state = SidebarState::default();
    state.set_width(200.0);
    assert_eq!(state.sidebar_width, 200.0);
}

#[test]
fn test_set_width_just_below_minimum() {
    let mut state = SidebarState::default();
    state.set_width(199.9);
    assert_eq!(state.sidebar_width, 200.0);
}

#[test]
fn test_set_width_just_above_minimum() {
    let mut state = SidebarState::default();
    state.set_width(200.1);
    assert_eq!(state.sidebar_width, 200.1);
}

#[test]
fn test_set_width_zero() {
    let mut state = SidebarState::default();
    state.set_width(0.0);
    assert_eq!(state.sidebar_width, 200.0);
}

#[test]
fn test_set_width_negative() {
    let mut state = SidebarState::default();
    state.set_width(-50.0);
    assert_eq!(state.sidebar_width, 200.0);
}

#[test]
fn test_set_width_large_value() {
    let mut state = SidebarState::default();
    state.set_width(1000.0);
    assert_eq!(state.sidebar_width, 1000.0);
}

// ===================================================================
// Additional: get_resource_types
// ===================================================================

#[test]
fn test_get_resource_types_returns_items_for_known_category() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("kind-dev", "kind-dev");

    let items = state.get_resource_types(id, ResourceCategory::Workloads);
    assert!(!items.is_empty());
    let kinds: Vec<&str> = items.iter().map(|i| i.kind.as_str()).collect();
    assert!(kinds.contains(&"Pod"));
    assert!(kinds.contains(&"Deployment"));
}

#[test]
fn test_get_resource_types_returns_empty_for_nonexistent_cluster() {
    let state = SidebarState::default();
    let fake_id = Uuid::new_v4();
    let items = state.get_resource_types(fake_id, ResourceCategory::Workloads);
    assert!(items.is_empty());
}

// ===================================================================
// T310: Contextual tracking — ensure_category_expanded / ensure_cluster_expanded
// ===================================================================

#[test]
fn test_ensure_category_expanded_expands_collapsed_category() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("kind-dev", "kind-dev");
    assert!(!state.is_category_expanded(id, ResourceCategory::Workloads));

    state.ensure_category_expanded(id, ResourceCategory::Workloads);
    assert!(state.is_category_expanded(id, ResourceCategory::Workloads));
}

#[test]
fn test_ensure_category_expanded_does_not_collapse_already_expanded() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("kind-dev", "kind-dev");

    // First expand it
    state.toggle_category_expand(id, ResourceCategory::Workloads);
    assert!(state.is_category_expanded(id, ResourceCategory::Workloads));

    // ensure_category_expanded should NOT collapse it (unlike toggle)
    state.ensure_category_expanded(id, ResourceCategory::Workloads);
    assert!(state.is_category_expanded(id, ResourceCategory::Workloads));
}

#[test]
fn test_ensure_cluster_expanded_expands_collapsed_cluster() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("kind-dev", "kind-dev");

    // Starts collapsed
    assert!(!state.clusters[0].expanded);

    // ensure_cluster_expanded should expand it
    state.ensure_cluster_expanded(id);
    assert!(state.clusters[0].expanded);
}

#[test]
fn test_ensure_cluster_expanded_does_not_collapse_already_expanded() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("kind-dev", "kind-dev");

    // Expand it first
    state.toggle_cluster_expand(id);
    assert!(state.clusters[0].expanded);

    // ensure should keep it expanded
    state.ensure_cluster_expanded(id);
    assert!(state.clusters[0].expanded);
}

#[test]
fn test_ensure_category_expanded_on_nonexistent_cluster_is_noop() {
    let mut state = SidebarState::default();
    let fake_id = Uuid::new_v4();
    // Should not panic
    state.ensure_category_expanded(fake_id, ResourceCategory::Workloads);
    assert!(!state.is_category_expanded(fake_id, ResourceCategory::Workloads));
}

#[test]
fn test_ensure_cluster_expanded_on_nonexistent_cluster_is_noop() {
    let mut state = SidebarState::default();
    let fake_id = Uuid::new_v4();
    // Should not panic
    state.ensure_cluster_expanded(fake_id);
}

#[test]
fn test_find_cluster_id_by_context_returns_correct_id() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("prod", "production");
    state.add_cluster("staging", "staging-eu");

    assert_eq!(state.find_cluster_id_by_context("prod"), Some(id));
}

#[test]
fn test_find_cluster_id_by_context_returns_none_for_unknown() {
    let mut state = SidebarState::default();
    state.add_cluster("prod", "production");

    assert_eq!(state.find_cluster_id_by_context("dev"), None);
}

#[test]
fn test_find_cluster_id_by_context_empty_sidebar() {
    let state = SidebarState::default();
    assert_eq!(state.find_cluster_id_by_context("prod"), None);
}

// ===================================================================
// T311: Drill-into mode rendering data
// ===================================================================

#[test]
fn test_drill_into_filters_to_single_cluster() {
    let mut state = SidebarState::default();
    let id1 = state.add_cluster("kind-dev", "kind-dev");
    let _id2 = state.add_cluster("prod", "production");

    state.enter_drill_into(id1);

    // Verify only one cluster matches the drill-into filter
    let visible: Vec<_> =
        state.clusters.iter().filter(|c| state.drill_into_cluster == Some(c.id)).collect();
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].context_name, "kind-dev");
}

#[test]
fn test_drill_into_exit_shows_all_clusters() {
    let mut state = SidebarState::default();
    let id1 = state.add_cluster("kind-dev", "kind-dev");
    state.add_cluster("prod", "production");

    state.enter_drill_into(id1);
    state.exit_drill_into();

    // All clusters visible when not in drill-into
    assert!(!state.is_drill_into());
    assert_eq!(state.clusters.len(), 2);
}

// ===================================================================
// T313: Remove cluster from list
// ===================================================================

#[test]
fn test_remove_cluster_removes_from_list() {
    let mut state = SidebarState::default();
    let id1 = state.add_cluster("kind-dev", "kind-dev");
    let _id2 = state.add_cluster("prod", "production");

    assert_eq!(state.clusters.len(), 2);
    assert!(state.remove_cluster(id1));
    assert_eq!(state.clusters.len(), 1);
    assert_eq!(state.clusters[0].context_name, "prod");
}

#[test]
fn test_remove_cluster_returns_false_for_unknown_id() {
    let mut state = SidebarState::default();
    state.add_cluster("kind-dev", "kind-dev");

    assert!(!state.remove_cluster(Uuid::new_v4()));
    assert_eq!(state.clusters.len(), 1);
}

#[test]
fn test_remove_selected_cluster_updates_selection() {
    let mut state = SidebarState::default();
    let id1 = state.add_cluster("kind-dev", "kind-dev");
    let id2 = state.add_cluster("prod", "production");

    state.select_cluster(id1);
    assert_eq!(state.selected_cluster_id, Some(id1));

    state.remove_cluster(id1);
    // After removing the selected cluster, should auto-select the first remaining
    assert_eq!(state.selected_cluster_id, Some(id2));
}

#[test]
fn test_remove_drill_into_cluster_exits_drill_into() {
    let mut state = SidebarState::default();
    let id1 = state.add_cluster("kind-dev", "kind-dev");
    state.add_cluster("prod", "production");

    state.enter_drill_into(id1);
    assert!(state.is_drill_into());

    state.remove_cluster(id1);
    assert!(!state.is_drill_into());
}
