// Tests extracted from crates/baeus-ui/src/layout/sidebar.rs

use baeus_ui::icons::{ResourceCategory, ResourceIcon};
use baeus_ui::layout::NavigationTarget;
use baeus_ui::layout::sidebar::*;
use uuid::Uuid;

#[test]
fn test_default_sidebar_sections() {
    let state = SidebarState::default();
    assert_eq!(state.sections.len(), 11);
    assert!(state.sections[0].expanded); // Workloads expanded by default
    assert!(!state.sections[1].expanded); // Network collapsed
}

#[test]
fn test_toggle_section() {
    let mut state = SidebarState::default();

    state.toggle_section(ResourceCategory::Workloads);
    assert!(!state.sections[0].expanded);

    state.toggle_section(ResourceCategory::Workloads);
    assert!(state.sections[0].expanded);
}

#[test]
fn test_collapse_all() {
    let mut state = SidebarState::default();
    state.collapse_all();
    assert!(state.sections.iter().all(|s| !s.expanded));
}

#[test]
fn test_update_badge() {
    let mut state = SidebarState::default();
    state.update_badge("Pod", Some(42));

    let pod_item = state.sections[0].items.iter().find(|i| i.kind == "Pod").unwrap();
    assert_eq!(pod_item.badge_count, Some(42));
}

#[test]
fn test_workloads_section_items() {
    let state = SidebarState::default();
    let workloads = &state.sections[0];
    assert_eq!(workloads.category, ResourceCategory::Workloads);
    assert_eq!(workloads.items.len(), 8);

    let kinds: Vec<&str> = workloads.items.iter().map(|i| i.kind.as_str()).collect();
    assert!(kinds.contains(&"Pod"));
    assert!(kinds.contains(&"Deployment"));
    assert!(kinds.contains(&"StatefulSet"));
}

#[test]
fn test_sidebar_item_navigation_target() {
    let item = SidebarItem {
        icon: ResourceIcon::Pod,
        label: "Pods".to_string(),
        kind: "Pod".to_string(),
        badge_count: None,
    };

    let target = item.navigation_target(ResourceCategory::Workloads, "test");
    if let NavigationTarget::ResourceList { category, kind, .. } = target {
        assert_eq!(category, ResourceCategory::Workloads);
        assert_eq!(kind, "Pod");
    } else {
        panic!("Expected ResourceList navigation target");
    }
}

// --- T063: Sidebar navigation wiring ---

#[test]
fn test_default_no_active_kind() {
    let state = SidebarState::default();
    assert!(state.active_kind.is_none());
}

#[test]
fn test_set_active_kind() {
    let mut state = SidebarState::default();
    state.set_active_kind("Pod");
    assert_eq!(state.active_kind.as_deref(), Some("Pod"));
    assert!(state.is_active("Pod"));
    assert!(!state.is_active("Deployment"));
}

#[test]
fn test_set_active_kind_expands_section() {
    let mut state = SidebarState::default();
    state.collapse_all();
    assert!(!state.sections[1].expanded); // Network collapsed

    state.set_active_kind("Service");
    assert!(state.sections[1].expanded); // Network auto-expanded
}

#[test]
fn test_clear_active_kind() {
    let mut state = SidebarState::default();
    state.set_active_kind("Pod");
    state.clear_active_kind();
    assert!(state.active_kind.is_none());
    assert!(!state.is_active("Pod"));
}

#[test]
fn test_navigate_to_kind_workloads() {
    let mut state = SidebarState::default();
    let target = state.navigate_to_kind("Deployment", "test");
    assert!(target.is_some());
    if let Some(NavigationTarget::ResourceList { category, kind, .. }) = target {
        assert_eq!(category, ResourceCategory::Workloads);
        assert_eq!(kind, "Deployment");
    }
    assert!(state.is_active("Deployment"));
}

#[test]
fn test_navigate_to_kind_network() {
    let mut state = SidebarState::default();
    let target = state.navigate_to_kind("Service", "test");
    assert!(target.is_some());
    if let Some(NavigationTarget::ResourceList { category, kind, .. }) = target {
        assert_eq!(category, ResourceCategory::Network);
        assert_eq!(kind, "Service");
    }
}

#[test]
fn test_navigate_to_kind_storage() {
    let mut state = SidebarState::default();
    let target = state.navigate_to_kind("PersistentVolumeClaim", "test");
    assert!(target.is_some());
    if let Some(NavigationTarget::ResourceList { category, kind, .. }) = target {
        assert_eq!(category, ResourceCategory::Storage);
        assert_eq!(kind, "PersistentVolumeClaim");
    }
}

#[test]
fn test_navigate_to_kind_rbac() {
    let mut state = SidebarState::default();
    let target = state.navigate_to_kind("ClusterRole", "test");
    assert!(target.is_some());
    if let Some(NavigationTarget::ResourceList { category, kind, .. }) = target {
        assert_eq!(category, ResourceCategory::Rbac);
        assert_eq!(kind, "ClusterRole");
    }
}

#[test]
fn test_navigate_to_unknown_kind() {
    let mut state = SidebarState::default();
    let target = state.navigate_to_kind("CustomThing", "test");
    assert!(target.is_none());
}

#[test]
fn test_find_kind_category() {
    let state = SidebarState::default();
    assert_eq!(state.find_kind_category("Pod"), Some(ResourceCategory::Workloads));
    assert_eq!(state.find_kind_category("Service"), Some(ResourceCategory::Network));
    assert_eq!(state.find_kind_category("ConfigMap"), Some(ResourceCategory::Configuration));
    assert_eq!(state.find_kind_category("PersistentVolume"), Some(ResourceCategory::Storage));
    assert_eq!(state.find_kind_category("Role"), Some(ResourceCategory::Rbac));
    assert_eq!(state.find_kind_category("Event"), Some(ResourceCategory::Monitoring));
    assert_eq!(state.find_kind_category("HelmRelease"), Some(ResourceCategory::Helm));
    assert_eq!(state.find_kind_category("HelmChart"), Some(ResourceCategory::Helm));
    assert_eq!(state.find_kind_category("Plugin"), Some(ResourceCategory::Plugins));
    assert_eq!(
        state.find_kind_category("CustomResourceDefinition"),
        Some(ResourceCategory::Custom)
    );
    assert_eq!(state.find_kind_category("Unknown"), None);
}

// --- T095: Helm sidebar section ---

#[test]
fn test_helm_section_exists() {
    let state = SidebarState::default();
    let helm_section = state.sections.iter().find(|s| s.category == ResourceCategory::Helm);
    assert!(helm_section.is_some());
    let helm_section = helm_section.unwrap();
    assert!(!helm_section.expanded); // collapsed by default
    assert_eq!(helm_section.items.len(), 2);
}

#[test]
fn test_helm_section_items() {
    let state = SidebarState::default();
    let helm_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Helm).unwrap();

    let kinds: Vec<&str> = helm_section.items.iter().map(|i| i.kind.as_str()).collect();
    assert!(kinds.contains(&"HelmRelease"));
    assert!(kinds.contains(&"HelmChart"));

    let labels: Vec<&str> = helm_section.items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"Releases"));
    assert!(labels.contains(&"Install Chart"));
}

#[test]
fn test_helm_section_icons() {
    let state = SidebarState::default();
    let helm_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Helm).unwrap();

    let releases_item = helm_section.items.iter().find(|i| i.kind == "HelmRelease").unwrap();
    assert_eq!(releases_item.icon, ResourceIcon::HelmRelease);

    let install_item = helm_section.items.iter().find(|i| i.kind == "HelmChart").unwrap();
    assert_eq!(install_item.icon, ResourceIcon::HelmChart);
}

#[test]
fn test_navigate_to_helm_release() {
    let mut state = SidebarState::default();
    let target = state.navigate_to_kind("HelmRelease", "test");
    assert!(target.is_some());
    if let Some(NavigationTarget::ResourceList { category, kind, .. }) = target {
        assert_eq!(category, ResourceCategory::Helm);
        assert_eq!(kind, "HelmRelease");
    }
    assert!(state.is_active("HelmRelease"));
}

#[test]
fn test_navigate_to_helm_chart() {
    let mut state = SidebarState::default();
    let target = state.navigate_to_kind("HelmChart", "test");
    assert!(target.is_some());
    if let Some(NavigationTarget::ResourceList { category, kind, .. }) = target {
        assert_eq!(category, ResourceCategory::Helm);
        assert_eq!(kind, "HelmChart");
    }
    assert!(state.is_active("HelmChart"));
}

#[test]
fn test_helm_section_toggle() {
    let mut state = SidebarState::default();
    let helm_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Helm).unwrap();
    assert!(!helm_section.expanded);

    state.toggle_section(ResourceCategory::Helm);
    let helm_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Helm).unwrap();
    assert!(helm_section.expanded);

    state.toggle_section(ResourceCategory::Helm);
    let helm_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Helm).unwrap();
    assert!(!helm_section.expanded);
}

#[test]
fn test_helm_section_auto_expand_on_navigate() {
    let mut state = SidebarState::default();
    state.collapse_all();

    let helm_collapsed =
        state.sections.iter().find(|s| s.category == ResourceCategory::Helm).unwrap();
    assert!(!helm_collapsed.expanded);

    state.set_active_kind("HelmRelease");

    let helm_expanded =
        state.sections.iter().find(|s| s.category == ResourceCategory::Helm).unwrap();
    assert!(helm_expanded.expanded);
}

#[test]
fn test_helm_section_badge_update() {
    let mut state = SidebarState::default();
    state.update_badge("HelmRelease", Some(12));

    let helm_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Helm).unwrap();
    let releases_item = helm_section.items.iter().find(|i| i.kind == "HelmRelease").unwrap();
    assert_eq!(releases_item.badge_count, Some(12));
}

// --- T102: Events/Monitoring sidebar section ---

#[test]
fn test_monitoring_section_exists() {
    let state = SidebarState::default();
    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Monitoring);
    assert!(section.is_some());
    let section = section.unwrap();
    assert!(!section.expanded);
    assert_eq!(section.items.len(), 1);
}

#[test]
fn test_monitoring_section_has_events() {
    let state = SidebarState::default();
    let section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Monitoring).unwrap();

    let event_item = section.items.iter().find(|i| i.kind == "Event");
    assert!(event_item.is_some());
    let event_item = event_item.unwrap();
    assert_eq!(event_item.label, "Events");
    assert_eq!(event_item.icon, ResourceIcon::Event);
}

#[test]
fn test_navigate_to_events() {
    let mut state = SidebarState::default();
    let target = state.navigate_to_kind("Event", "test");
    assert!(target.is_some());
    if let Some(NavigationTarget::ResourceList { category, kind, .. }) = target {
        assert_eq!(category, ResourceCategory::Monitoring);
        assert_eq!(kind, "Event");
    }
    assert!(state.is_active("Event"));
}

#[test]
fn test_monitoring_section_auto_expand_on_navigate() {
    let mut state = SidebarState::default();
    state.collapse_all();

    let section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Monitoring).unwrap();
    assert!(!section.expanded);

    state.set_active_kind("Event");

    let section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Monitoring).unwrap();
    assert!(section.expanded);
}

#[test]
fn test_monitoring_section_badge_update() {
    let mut state = SidebarState::default();
    state.update_badge("Event", Some(47));

    let section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Monitoring).unwrap();
    let event_item = section.items.iter().find(|i| i.kind == "Event").unwrap();
    assert_eq!(event_item.badge_count, Some(47));
}

// --- T114: Custom Resources sidebar section ---

#[test]
fn test_custom_section_exists() {
    let state = SidebarState::default();
    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Custom);
    assert!(section.is_some());
    let section = section.unwrap();
    assert!(!section.expanded); // collapsed by default
    assert_eq!(section.items.len(), 1);
}

#[test]
fn test_custom_section_has_crd_item() {
    let state = SidebarState::default();
    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Custom).unwrap();

    let crd_item = section.items.iter().find(|i| i.kind == "CustomResourceDefinition");
    assert!(crd_item.is_some());
    let crd_item = crd_item.unwrap();
    assert_eq!(crd_item.label, "Custom Resources");
    assert_eq!(crd_item.icon, ResourceIcon::CustomResource);
}

#[test]
fn test_navigate_to_custom_resources() {
    let mut state = SidebarState::default();
    let target = state.navigate_to_kind("CustomResourceDefinition", "test");
    assert!(target.is_some());
    if let Some(NavigationTarget::ResourceList { category, kind, .. }) = target {
        assert_eq!(category, ResourceCategory::Custom);
        assert_eq!(kind, "CustomResourceDefinition");
    }
    assert!(state.is_active("CustomResourceDefinition"));
}

#[test]
fn test_custom_section_auto_expand_on_navigate() {
    let mut state = SidebarState::default();
    state.collapse_all();

    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Custom).unwrap();
    assert!(!section.expanded);

    state.set_active_kind("CustomResourceDefinition");

    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Custom).unwrap();
    assert!(section.expanded);
}

#[test]
fn test_custom_section_toggle() {
    let mut state = SidebarState::default();
    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Custom).unwrap();
    assert!(!section.expanded);

    state.toggle_section(ResourceCategory::Custom);
    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Custom).unwrap();
    assert!(section.expanded);

    state.toggle_section(ResourceCategory::Custom);
    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Custom).unwrap();
    assert!(!section.expanded);
}

#[test]
fn test_custom_section_badge_update() {
    let mut state = SidebarState::default();
    state.update_badge("CustomResourceDefinition", Some(15));

    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Custom).unwrap();
    let crd_item = section.items.iter().find(|i| i.kind == "CustomResourceDefinition").unwrap();
    assert_eq!(crd_item.badge_count, Some(15));
}

// ===================================================================
// T109: Network and Storage sidebar wiring verification
// ===================================================================

#[test]
fn test_network_section_exists() {
    let state = SidebarState::default();
    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Network);
    assert!(section.is_some());
    let section = section.unwrap();
    assert!(!section.expanded); // collapsed by default
}

#[test]
fn test_network_section_has_all_items() {
    let state = SidebarState::default();
    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Network).unwrap();

    assert_eq!(section.items.len(), 6);

    let kinds: Vec<&str> = section.items.iter().map(|i| i.kind.as_str()).collect();
    assert!(kinds.contains(&"Service"));
    assert!(kinds.contains(&"Ingress"));
    assert!(kinds.contains(&"NetworkPolicy"));
    assert!(kinds.contains(&"Endpoints"));
}

#[test]
fn test_network_section_item_labels() {
    let state = SidebarState::default();
    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Network).unwrap();

    let labels: Vec<&str> = section.items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"Services"));
    assert!(labels.contains(&"Ingresses"));
    assert!(labels.contains(&"Network Policies"));
    assert!(labels.contains(&"Endpoints"));
}

#[test]
fn test_network_section_item_icons() {
    let state = SidebarState::default();
    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Network).unwrap();

    let svc = section.items.iter().find(|i| i.kind == "Service").unwrap();
    assert_eq!(svc.icon, ResourceIcon::Service);

    let ing = section.items.iter().find(|i| i.kind == "Ingress").unwrap();
    assert_eq!(ing.icon, ResourceIcon::Ingress);

    let np = section.items.iter().find(|i| i.kind == "NetworkPolicy").unwrap();
    assert_eq!(np.icon, ResourceIcon::NetworkPolicy);

    let ep = section.items.iter().find(|i| i.kind == "Endpoints").unwrap();
    assert_eq!(ep.icon, ResourceIcon::Endpoints);
}

#[test]
fn test_navigate_to_ingress() {
    let mut state = SidebarState::default();
    let target = state.navigate_to_kind("Ingress", "test");
    assert!(target.is_some());
    if let Some(NavigationTarget::ResourceList { category, kind, .. }) = target {
        assert_eq!(category, ResourceCategory::Network);
        assert_eq!(kind, "Ingress");
    }
    assert!(state.is_active("Ingress"));
}

#[test]
fn test_navigate_to_network_policy() {
    let mut state = SidebarState::default();
    let target = state.navigate_to_kind("NetworkPolicy", "test");
    assert!(target.is_some());
    if let Some(NavigationTarget::ResourceList { category, kind, .. }) = target {
        assert_eq!(category, ResourceCategory::Network);
        assert_eq!(kind, "NetworkPolicy");
    }
    assert!(state.is_active("NetworkPolicy"));
}

#[test]
fn test_navigate_to_endpoints() {
    let mut state = SidebarState::default();
    let target = state.navigate_to_kind("Endpoints", "test");
    assert!(target.is_some());
    if let Some(NavigationTarget::ResourceList { category, kind, .. }) = target {
        assert_eq!(category, ResourceCategory::Network);
        assert_eq!(kind, "Endpoints");
    }
    assert!(state.is_active("Endpoints"));
}

#[test]
fn test_network_section_auto_expand_on_navigate() {
    let mut state = SidebarState::default();
    state.collapse_all();

    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Network).unwrap();
    assert!(!section.expanded);

    state.set_active_kind("Ingress");

    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Network).unwrap();
    assert!(section.expanded);
}

#[test]
fn test_network_section_toggle() {
    let mut state = SidebarState::default();
    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Network).unwrap();
    assert!(!section.expanded);

    state.toggle_section(ResourceCategory::Network);
    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Network).unwrap();
    assert!(section.expanded);

    state.toggle_section(ResourceCategory::Network);
    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Network).unwrap();
    assert!(!section.expanded);
}

#[test]
fn test_network_section_badge_update() {
    let mut state = SidebarState::default();
    state.update_badge("Service", Some(25));
    state.update_badge("Ingress", Some(3));

    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Network).unwrap();
    let svc = section.items.iter().find(|i| i.kind == "Service").unwrap();
    assert_eq!(svc.badge_count, Some(25));
    let ing = section.items.iter().find(|i| i.kind == "Ingress").unwrap();
    assert_eq!(ing.badge_count, Some(3));
}

#[test]
fn test_storage_section_exists() {
    let state = SidebarState::default();
    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Storage);
    assert!(section.is_some());
    let section = section.unwrap();
    assert!(!section.expanded); // collapsed by default
}

#[test]
fn test_storage_section_has_all_items() {
    let state = SidebarState::default();
    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Storage).unwrap();

    assert_eq!(section.items.len(), 3);

    let kinds: Vec<&str> = section.items.iter().map(|i| i.kind.as_str()).collect();
    assert!(kinds.contains(&"PersistentVolume"));
    assert!(kinds.contains(&"PersistentVolumeClaim"));
    assert!(kinds.contains(&"StorageClass"));
}

#[test]
fn test_storage_section_item_labels() {
    let state = SidebarState::default();
    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Storage).unwrap();

    let labels: Vec<&str> = section.items.iter().map(|i| i.label.as_str()).collect();
    assert!(labels.contains(&"Persistent Volumes"));
    assert!(labels.contains(&"PVCs"));
    assert!(labels.contains(&"Storage Classes"));
}

#[test]
fn test_storage_section_item_icons() {
    let state = SidebarState::default();
    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Storage).unwrap();

    let pv = section.items.iter().find(|i| i.kind == "PersistentVolume").unwrap();
    assert_eq!(pv.icon, ResourceIcon::PersistentVolume);

    let pvc = section.items.iter().find(|i| i.kind == "PersistentVolumeClaim").unwrap();
    assert_eq!(pvc.icon, ResourceIcon::PersistentVolumeClaim);

    let sc = section.items.iter().find(|i| i.kind == "StorageClass").unwrap();
    assert_eq!(sc.icon, ResourceIcon::StorageClass);
}

#[test]
fn test_navigate_to_persistent_volume() {
    let mut state = SidebarState::default();
    let target = state.navigate_to_kind("PersistentVolume", "test");
    assert!(target.is_some());
    if let Some(NavigationTarget::ResourceList { category, kind, .. }) = target {
        assert_eq!(category, ResourceCategory::Storage);
        assert_eq!(kind, "PersistentVolume");
    }
    assert!(state.is_active("PersistentVolume"));
}

#[test]
fn test_navigate_to_storage_class() {
    let mut state = SidebarState::default();
    let target = state.navigate_to_kind("StorageClass", "test");
    assert!(target.is_some());
    if let Some(NavigationTarget::ResourceList { category, kind, .. }) = target {
        assert_eq!(category, ResourceCategory::Storage);
        assert_eq!(kind, "StorageClass");
    }
    assert!(state.is_active("StorageClass"));
}

#[test]
fn test_storage_section_auto_expand_on_navigate() {
    let mut state = SidebarState::default();
    state.collapse_all();

    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Storage).unwrap();
    assert!(!section.expanded);

    state.set_active_kind("PersistentVolumeClaim");

    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Storage).unwrap();
    assert!(section.expanded);
}

#[test]
fn test_storage_section_toggle() {
    let mut state = SidebarState::default();
    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Storage).unwrap();
    assert!(!section.expanded);

    state.toggle_section(ResourceCategory::Storage);
    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Storage).unwrap();
    assert!(section.expanded);

    state.toggle_section(ResourceCategory::Storage);
    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Storage).unwrap();
    assert!(!section.expanded);
}

#[test]
fn test_storage_section_badge_update() {
    let mut state = SidebarState::default();
    state.update_badge("PersistentVolumeClaim", Some(8));
    state.update_badge("StorageClass", Some(2));

    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Storage).unwrap();
    let pvc = section.items.iter().find(|i| i.kind == "PersistentVolumeClaim").unwrap();
    assert_eq!(pvc.badge_count, Some(8));
    let sc = section.items.iter().find(|i| i.kind == "StorageClass").unwrap();
    assert_eq!(sc.badge_count, Some(2));
}

#[test]
fn test_find_kind_category_network_items() {
    let state = SidebarState::default();
    assert_eq!(state.find_kind_category("Service"), Some(ResourceCategory::Network));
    assert_eq!(state.find_kind_category("Ingress"), Some(ResourceCategory::Network));
    assert_eq!(state.find_kind_category("NetworkPolicy"), Some(ResourceCategory::Network));
    assert_eq!(state.find_kind_category("Endpoints"), Some(ResourceCategory::Network));
}

#[test]
fn test_find_kind_category_storage_items() {
    let state = SidebarState::default();
    assert_eq!(state.find_kind_category("PersistentVolume"), Some(ResourceCategory::Storage));
    assert_eq!(state.find_kind_category("PersistentVolumeClaim"), Some(ResourceCategory::Storage));
    assert_eq!(state.find_kind_category("StorageClass"), Some(ResourceCategory::Storage));
}

// --- T121: Map tab in sidebar ---

#[test]
fn test_navigate_to_map() {
    let mut state = SidebarState::default();
    let target = state.navigate_to_map("test");
    assert_eq!(target, NavigationTarget::NamespaceMap { cluster_context: "test".to_string() });
}

#[test]
fn test_is_map_active() {
    let mut state = SidebarState::default();
    assert!(!state.is_map_active());

    state.navigate_to_map("test");
    assert!(state.is_map_active());
}

#[test]
fn test_navigate_to_map_then_kind_clears_map() {
    let mut state = SidebarState::default();
    state.navigate_to_map("test");
    assert!(state.is_map_active());

    // Navigating to a resource kind should clear the map state
    state.navigate_to_kind("Pod", "test");
    assert!(!state.is_map_active());
    assert!(state.is_active("Pod"));
}

#[test]
fn test_navigate_to_kind_then_map() {
    let mut state = SidebarState::default();
    state.navigate_to_kind("Pod", "test");
    assert!(state.is_active("Pod"));
    assert!(!state.is_map_active());

    state.navigate_to_map("test");
    assert!(state.is_map_active());
    // Pod should no longer be considered active since map took over
    assert!(!state.is_active("Pod"));
}

// ===================================================================
// T210: Cluster-first sidebar types
// ===================================================================

#[test]
fn test_generate_initials_two_words() {
    assert_eq!(generate_initials("kind-dev"), "KD");
    assert_eq!(generate_initials("prod-us-east"), "PU");
    assert_eq!(generate_initials("my_cluster"), "MC");
    assert_eq!(generate_initials("staging.eu"), "SE");
}

#[test]
fn test_generate_initials_single_word() {
    assert_eq!(generate_initials("production"), "PR");
    assert_eq!(generate_initials("a"), "AA");
}

#[test]
fn test_generate_initials_empty() {
    assert_eq!(generate_initials(""), "??");
}

#[test]
fn test_generate_cluster_color_deterministic() {
    let c1 = generate_cluster_color("kind-dev");
    let c2 = generate_cluster_color("kind-dev");
    assert_eq!(c1, c2);
}

#[test]
fn test_generate_cluster_color_different_names() {
    let c1 = generate_cluster_color("kind-dev");
    let c2 = generate_cluster_color("prod-us-east");
    // Different names should usually produce different colors (not guaranteed but very likely)
    // Just verify they're valid hex colors from the palette
    assert!(c1 != 0);
    assert!(c2 != 0);
}

#[test]
fn test_cluster_status_equality() {
    assert_eq!(ClusterStatus::Connected, ClusterStatus::Connected);
    assert_ne!(ClusterStatus::Connected, ClusterStatus::Disconnected);
}

#[test]
fn test_add_cluster() {
    let mut state = SidebarState::default();
    assert!(state.clusters.is_empty());
    assert!(state.selected_cluster_id.is_none());

    let id = state.add_cluster("kind-dev", "kind-dev");
    assert_eq!(state.clusters.len(), 1);
    assert_eq!(state.selected_cluster_id, Some(id));
    assert_eq!(state.clusters[0].context_name, "kind-dev");
    assert_eq!(state.clusters[0].initials, "KD");
    assert_eq!(state.clusters[0].status, ClusterStatus::Disconnected);
    assert!(!state.clusters[0].expanded);
    assert!(!state.clusters[0].sections.is_empty());
}

#[test]
fn test_add_multiple_clusters_first_selected() {
    let mut state = SidebarState::default();
    let id1 = state.add_cluster("kind-dev", "kind-dev");
    let _id2 = state.add_cluster("prod", "production");

    assert_eq!(state.clusters.len(), 2);
    // First cluster stays selected
    assert_eq!(state.selected_cluster_id, Some(id1));
}

#[test]
fn test_select_cluster() {
    let mut state = SidebarState::default();
    let _id1 = state.add_cluster("kind-dev", "kind-dev");
    let id2 = state.add_cluster("prod", "production");

    state.select_cluster(id2);
    assert_eq!(state.selected_cluster_id, Some(id2));
}

#[test]
fn test_select_nonexistent_cluster() {
    let mut state = SidebarState::default();
    let id1 = state.add_cluster("kind-dev", "kind-dev");
    state.select_cluster(Uuid::new_v4()); // nonexistent
    assert_eq!(state.selected_cluster_id, Some(id1)); // unchanged
}

#[test]
fn test_toggle_cluster() {
    let mut state = SidebarState::default();
    let id = state.add_cluster("kind-dev", "kind-dev");
    assert!(!state.clusters[0].expanded);

    state.toggle_cluster(id);
    assert!(state.clusters[0].expanded);

    state.toggle_cluster(id);
    assert!(!state.clusters[0].expanded);
}

#[test]
fn test_selected_cluster() {
    let mut state = SidebarState::default();
    assert!(state.selected_cluster().is_none());

    let id = state.add_cluster("kind-dev", "kind-dev");
    let selected = state.selected_cluster().unwrap();
    assert_eq!(selected.id, id);
    assert_eq!(selected.display_name, "kind-dev");
}

#[test]
fn test_has_clusters() {
    let mut state = SidebarState::default();
    assert!(!state.has_clusters());

    state.add_cluster("kind-dev", "kind-dev");
    assert!(state.has_clusters());
}

#[test]
fn test_clear_active_kind_clears_map() {
    let mut state = SidebarState::default();
    state.navigate_to_map("test");
    assert!(state.is_map_active());

    state.clear_active_kind();
    assert!(!state.is_map_active());
}

#[test]
fn test_namespace_map_navigation_target_label() {
    assert_eq!(
        NavigationTarget::NamespaceMap { cluster_context: "test".to_string() }.label(),
        "test - Resource Map"
    );
}

// --- T131: Plugins sidebar section ---

#[test]
fn test_plugins_section_exists() {
    let state = SidebarState::default();
    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Plugins);
    assert!(section.is_some());
    let section = section.unwrap();
    assert!(!section.expanded); // collapsed by default
    assert_eq!(section.items.len(), 1);
}

#[test]
fn test_plugins_section_has_plugin_item() {
    let state = SidebarState::default();
    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Plugins).unwrap();

    let plugin_item = section.items.iter().find(|i| i.kind == "Plugin");
    assert!(plugin_item.is_some());
    let plugin_item = plugin_item.unwrap();
    assert_eq!(plugin_item.label, "Plugins");
    assert_eq!(plugin_item.icon, ResourceIcon::Plugin);
}

#[test]
fn test_navigate_to_plugins() {
    let mut state = SidebarState::default();
    let target = state.navigate_to_kind("Plugin", "test");
    assert!(target.is_some());
    if let Some(NavigationTarget::ResourceList { category, kind, .. }) = target {
        assert_eq!(category, ResourceCategory::Plugins);
        assert_eq!(kind, "Plugin");
    }
    assert!(state.is_active("Plugin"));
}

#[test]
fn test_plugins_section_auto_expand_on_navigate() {
    let mut state = SidebarState::default();
    state.collapse_all();

    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Plugins).unwrap();
    assert!(!section.expanded);

    state.set_active_kind("Plugin");

    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Plugins).unwrap();
    assert!(section.expanded);
}

#[test]
fn test_plugins_section_toggle() {
    let mut state = SidebarState::default();
    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Plugins).unwrap();
    assert!(!section.expanded);

    state.toggle_section(ResourceCategory::Plugins);
    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Plugins).unwrap();
    assert!(section.expanded);

    state.toggle_section(ResourceCategory::Plugins);
    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Plugins).unwrap();
    assert!(!section.expanded);
}

#[test]
fn test_plugins_section_badge_update() {
    let mut state = SidebarState::default();
    state.update_badge("Plugin", Some(5));

    let section = state.sections.iter().find(|s| s.category == ResourceCategory::Plugins).unwrap();
    let plugin_item = section.items.iter().find(|i| i.kind == "Plugin").unwrap();
    assert_eq!(plugin_item.badge_count, Some(5));
}
