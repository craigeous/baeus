//! Integration tests for Network and Storage sections in the sidebar.
//!
//! Tests verify that the sidebar correctly includes Network and Storage sections
//! with the appropriate resource kinds and navigation works properly.

use baeus_ui::icons::ResourceCategory;
use baeus_ui::layout::sidebar::SidebarState;

// ---------------------------------------------------------------------------
// T073: Network section verification
// ---------------------------------------------------------------------------

#[test]
fn test_sidebar_has_network_section() {
    let state = SidebarState::default();

    let network_section = state.sections.iter().find(|s| s.category == ResourceCategory::Network);
    assert!(network_section.is_some(), "Network section should exist");

    let section = network_section.unwrap();
    assert!(!section.expanded, "Network section should be collapsed by default");
}

#[test]
fn test_network_section_contains_service() {
    let state = SidebarState::default();

    let network_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Network).unwrap();
    let service_item = network_section.items.iter().find(|i| i.kind == "Service");

    assert!(service_item.is_some(), "Network section should contain Service");
    let item = service_item.unwrap();
    assert_eq!(item.label, "Services");
}

#[test]
fn test_network_section_contains_ingress() {
    let state = SidebarState::default();

    let network_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Network).unwrap();
    let ingress_item = network_section.items.iter().find(|i| i.kind == "Ingress");

    assert!(ingress_item.is_some(), "Network section should contain Ingress");
    let item = ingress_item.unwrap();
    assert_eq!(item.label, "Ingresses");
}

#[test]
fn test_network_section_contains_network_policy() {
    let state = SidebarState::default();

    let network_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Network).unwrap();
    let np_item = network_section.items.iter().find(|i| i.kind == "NetworkPolicy");

    assert!(np_item.is_some(), "Network section should contain NetworkPolicy");
    let item = np_item.unwrap();
    assert_eq!(item.label, "Network Policies");
}

#[test]
fn test_network_section_contains_endpoints() {
    let state = SidebarState::default();

    let network_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Network).unwrap();
    let endpoints_item = network_section.items.iter().find(|i| i.kind == "Endpoints");

    assert!(endpoints_item.is_some(), "Network section should contain Endpoints");
    let item = endpoints_item.unwrap();
    assert_eq!(item.label, "Endpoints");
}

#[test]
fn test_network_section_has_four_items() {
    let state = SidebarState::default();

    let network_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Network).unwrap();
    assert_eq!(network_section.items.len(), 6, "Network section should have exactly 6 items");
}

// ---------------------------------------------------------------------------
// T073: Storage section verification
// ---------------------------------------------------------------------------

#[test]
fn test_sidebar_has_storage_section() {
    let state = SidebarState::default();

    let storage_section = state.sections.iter().find(|s| s.category == ResourceCategory::Storage);
    assert!(storage_section.is_some(), "Storage section should exist");

    let section = storage_section.unwrap();
    assert!(!section.expanded, "Storage section should be collapsed by default");
}

#[test]
fn test_storage_section_contains_persistent_volume() {
    let state = SidebarState::default();

    let storage_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Storage).unwrap();
    let pv_item = storage_section.items.iter().find(|i| i.kind == "PersistentVolume");

    assert!(pv_item.is_some(), "Storage section should contain PersistentVolume");
    let item = pv_item.unwrap();
    assert_eq!(item.label, "Persistent Volumes");
}

#[test]
fn test_storage_section_contains_persistent_volume_claim() {
    let state = SidebarState::default();

    let storage_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Storage).unwrap();
    let pvc_item = storage_section.items.iter().find(|i| i.kind == "PersistentVolumeClaim");

    assert!(pvc_item.is_some(), "Storage section should contain PersistentVolumeClaim");
    let item = pvc_item.unwrap();
    assert_eq!(item.label, "PVCs");
}

#[test]
fn test_storage_section_contains_storage_class() {
    let state = SidebarState::default();

    let storage_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Storage).unwrap();
    let sc_item = storage_section.items.iter().find(|i| i.kind == "StorageClass");

    assert!(sc_item.is_some(), "Storage section should contain StorageClass");
    let item = sc_item.unwrap();
    assert_eq!(item.label, "Storage Classes");
}

#[test]
fn test_storage_section_has_three_items() {
    let state = SidebarState::default();

    let storage_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Storage).unwrap();
    assert_eq!(storage_section.items.len(), 3, "Storage section should have exactly 3 items");
}

// ---------------------------------------------------------------------------
// T073: Navigation to network resources
// ---------------------------------------------------------------------------

#[test]
fn test_navigate_to_service() {
    let mut state = SidebarState::default();

    let target = state.navigate_to_kind("Service", "test");
    assert!(target.is_some());
    assert_eq!(state.active_kind, Some("Service".to_string()));
    assert!(state.is_active("Service"));

    // Verify the Network section is auto-expanded
    let network_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Network).unwrap();
    assert!(
        network_section.expanded,
        "Network section should be expanded after navigating to Service"
    );
}

#[test]
fn test_navigate_to_ingress() {
    let mut state = SidebarState::default();

    let target = state.navigate_to_kind("Ingress", "test");
    assert!(target.is_some());
    assert_eq!(state.active_kind, Some("Ingress".to_string()));
    assert!(state.is_active("Ingress"));

    let network_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Network).unwrap();
    assert!(network_section.expanded);
}

#[test]
fn test_navigate_to_network_policy() {
    let mut state = SidebarState::default();

    let target = state.navigate_to_kind("NetworkPolicy", "test");
    assert!(target.is_some());
    assert_eq!(state.active_kind, Some("NetworkPolicy".to_string()));
    assert!(state.is_active("NetworkPolicy"));

    let network_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Network).unwrap();
    assert!(network_section.expanded);
}

#[test]
fn test_navigate_to_endpoints() {
    let mut state = SidebarState::default();

    let target = state.navigate_to_kind("Endpoints", "test");
    assert!(target.is_some());
    assert_eq!(state.active_kind, Some("Endpoints".to_string()));
    assert!(state.is_active("Endpoints"));

    let network_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Network).unwrap();
    assert!(network_section.expanded);
}

#[test]
fn test_set_active_kind_service() {
    let mut state = SidebarState::default();

    state.set_active_kind("Service");
    assert_eq!(state.active_kind, Some("Service".to_string()));

    let network_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Network).unwrap();
    assert!(network_section.expanded);
}

// ---------------------------------------------------------------------------
// T073: Navigation to storage resources
// ---------------------------------------------------------------------------

#[test]
fn test_navigate_to_persistent_volume() {
    let mut state = SidebarState::default();

    let target = state.navigate_to_kind("PersistentVolume", "test");
    assert!(target.is_some());
    assert_eq!(state.active_kind, Some("PersistentVolume".to_string()));
    assert!(state.is_active("PersistentVolume"));

    // Verify the Storage section is auto-expanded
    let storage_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Storage).unwrap();
    assert!(
        storage_section.expanded,
        "Storage section should be expanded after navigating to PersistentVolume"
    );
}

#[test]
fn test_navigate_to_persistent_volume_claim() {
    let mut state = SidebarState::default();

    let target = state.navigate_to_kind("PersistentVolumeClaim", "test");
    assert!(target.is_some());
    assert_eq!(state.active_kind, Some("PersistentVolumeClaim".to_string()));
    assert!(state.is_active("PersistentVolumeClaim"));

    let storage_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Storage).unwrap();
    assert!(storage_section.expanded);
}

#[test]
fn test_navigate_to_storage_class() {
    let mut state = SidebarState::default();

    let target = state.navigate_to_kind("StorageClass", "test");
    assert!(target.is_some());
    assert_eq!(state.active_kind, Some("StorageClass".to_string()));
    assert!(state.is_active("StorageClass"));

    let storage_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Storage).unwrap();
    assert!(storage_section.expanded);
}

#[test]
fn test_set_active_kind_persistent_volume() {
    let mut state = SidebarState::default();

    state.set_active_kind("PersistentVolume");
    assert_eq!(state.active_kind, Some("PersistentVolume".to_string()));

    let storage_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Storage).unwrap();
    assert!(storage_section.expanded);
}

// ---------------------------------------------------------------------------
// T073: Section expansion/collapse
// ---------------------------------------------------------------------------

#[test]
fn test_toggle_network_section() {
    let mut state = SidebarState::default();

    // Network starts collapsed
    let network_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Network).unwrap();
    assert!(!network_section.expanded);

    // Toggle to expand
    state.toggle_section(ResourceCategory::Network);
    let network_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Network).unwrap();
    assert!(network_section.expanded);

    // Toggle to collapse
    state.toggle_section(ResourceCategory::Network);
    let network_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Network).unwrap();
    assert!(!network_section.expanded);
}

#[test]
fn test_toggle_storage_section() {
    let mut state = SidebarState::default();

    // Storage starts collapsed
    let storage_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Storage).unwrap();
    assert!(!storage_section.expanded);

    // Toggle to expand
    state.toggle_section(ResourceCategory::Storage);
    let storage_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Storage).unwrap();
    assert!(storage_section.expanded);

    // Toggle to collapse
    state.toggle_section(ResourceCategory::Storage);
    let storage_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Storage).unwrap();
    assert!(!storage_section.expanded);
}

#[test]
fn test_expand_network_section() {
    let mut state = SidebarState::default();

    state.expand_section(ResourceCategory::Network);
    let network_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Network).unwrap();
    assert!(network_section.expanded);
}

#[test]
fn test_expand_storage_section() {
    let mut state = SidebarState::default();

    state.expand_section(ResourceCategory::Storage);
    let storage_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Storage).unwrap();
    assert!(storage_section.expanded);
}

#[test]
fn test_collapse_all_includes_network_and_storage() {
    let mut state = SidebarState::default();

    // Expand both sections
    state.expand_section(ResourceCategory::Network);
    state.expand_section(ResourceCategory::Storage);

    // Collapse all
    state.collapse_all();

    let network_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Network).unwrap();
    let storage_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Storage).unwrap();

    assert!(!network_section.expanded);
    assert!(!storage_section.expanded);
}

// ---------------------------------------------------------------------------
// T073: Badge updates for network and storage resources
// ---------------------------------------------------------------------------

#[test]
fn test_update_service_badge() {
    let mut state = SidebarState::default();

    state.update_badge("Service", Some(5));

    let network_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Network).unwrap();
    let service_item = network_section.items.iter().find(|i| i.kind == "Service").unwrap();

    assert_eq!(service_item.badge_count, Some(5));
}

#[test]
fn test_update_persistent_volume_badge() {
    let mut state = SidebarState::default();

    state.update_badge("PersistentVolume", Some(12));

    let storage_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Storage).unwrap();
    let pv_item = storage_section.items.iter().find(|i| i.kind == "PersistentVolume").unwrap();

    assert_eq!(pv_item.badge_count, Some(12));
}

#[test]
fn test_clear_ingress_badge() {
    let mut state = SidebarState::default();

    state.update_badge("Ingress", Some(3));
    state.update_badge("Ingress", None);

    let network_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Network).unwrap();
    let ingress_item = network_section.items.iter().find(|i| i.kind == "Ingress").unwrap();

    assert_eq!(ingress_item.badge_count, None);
}

// ---------------------------------------------------------------------------
// T073: Category detection for network and storage kinds
// ---------------------------------------------------------------------------

#[test]
fn test_find_service_category() {
    let state = SidebarState::default();

    let category = state.find_kind_category("Service");
    assert_eq!(category, Some(ResourceCategory::Network));
}

#[test]
fn test_find_ingress_category() {
    let state = SidebarState::default();

    let category = state.find_kind_category("Ingress");
    assert_eq!(category, Some(ResourceCategory::Network));
}

#[test]
fn test_find_network_policy_category() {
    let state = SidebarState::default();

    let category = state.find_kind_category("NetworkPolicy");
    assert_eq!(category, Some(ResourceCategory::Network));
}

#[test]
fn test_find_endpoints_category() {
    let state = SidebarState::default();

    let category = state.find_kind_category("Endpoints");
    assert_eq!(category, Some(ResourceCategory::Network));
}

#[test]
fn test_find_persistent_volume_category() {
    let state = SidebarState::default();

    let category = state.find_kind_category("PersistentVolume");
    assert_eq!(category, Some(ResourceCategory::Storage));
}

#[test]
fn test_find_persistent_volume_claim_category() {
    let state = SidebarState::default();

    let category = state.find_kind_category("PersistentVolumeClaim");
    assert_eq!(category, Some(ResourceCategory::Storage));
}

#[test]
fn test_find_storage_class_category() {
    let state = SidebarState::default();

    let category = state.find_kind_category("StorageClass");
    assert_eq!(category, Some(ResourceCategory::Storage));
}

// ---------------------------------------------------------------------------
// T073: Multiple navigation targets
// ---------------------------------------------------------------------------

#[test]
fn test_navigate_between_network_and_storage() {
    let mut state = SidebarState::default();

    // Navigate to Service
    state.navigate_to_kind("Service", "test");
    assert!(state.is_active("Service"));
    let network_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Network).unwrap();
    assert!(network_section.expanded);

    // Navigate to PersistentVolume
    state.navigate_to_kind("PersistentVolume", "test");
    assert!(state.is_active("PersistentVolume"));
    assert!(!state.is_active("Service"));
    let storage_section =
        state.sections.iter().find(|s| s.category == ResourceCategory::Storage).unwrap();
    assert!(storage_section.expanded);
}

#[test]
fn test_clear_active_kind_after_network_navigation() {
    let mut state = SidebarState::default();

    state.navigate_to_kind("Ingress", "test");
    assert!(state.is_active("Ingress"));

    state.clear_active_kind();
    assert!(!state.is_active("Ingress"));
    assert_eq!(state.active_kind, None);
}

#[test]
fn test_clear_active_kind_after_storage_navigation() {
    let mut state = SidebarState::default();

    state.navigate_to_kind("StorageClass", "test");
    assert!(state.is_active("StorageClass"));

    state.clear_active_kind();
    assert!(!state.is_active("StorageClass"));
    assert_eq!(state.active_kind, None);
}

// ---------------------------------------------------------------------------
// T073: Sidebar section ordering
// ---------------------------------------------------------------------------

#[test]
fn test_network_section_position() {
    let state = SidebarState::default();

    let network_idx = state.sections.iter().position(|s| s.category == ResourceCategory::Network);
    let workloads_idx =
        state.sections.iter().position(|s| s.category == ResourceCategory::Workloads);

    assert!(network_idx.is_some());
    assert!(workloads_idx.is_some());
    // Network should come after Workloads
    assert!(network_idx.unwrap() > workloads_idx.unwrap());
}

#[test]
fn test_storage_section_position() {
    let state = SidebarState::default();

    let storage_idx = state.sections.iter().position(|s| s.category == ResourceCategory::Storage);
    let network_idx = state.sections.iter().position(|s| s.category == ResourceCategory::Network);

    assert!(storage_idx.is_some());
    assert!(network_idx.is_some());
    // Storage should come after Network
    assert!(storage_idx.unwrap() > network_idx.unwrap());
}

#[test]
fn test_all_sections_present() {
    let state = SidebarState::default();

    // Verify we have all expected sections
    assert!(state.sections.iter().any(|s| s.category == ResourceCategory::Workloads));
    assert!(state.sections.iter().any(|s| s.category == ResourceCategory::Network));
    assert!(state.sections.iter().any(|s| s.category == ResourceCategory::Configuration));
    assert!(state.sections.iter().any(|s| s.category == ResourceCategory::Storage));
    assert!(state.sections.iter().any(|s| s.category == ResourceCategory::Rbac));
    assert!(state.sections.iter().any(|s| s.category == ResourceCategory::Monitoring));
    assert!(state.sections.iter().any(|s| s.category == ResourceCategory::Helm));
    assert!(state.sections.iter().any(|s| s.category == ResourceCategory::Plugins));
    assert!(state.sections.iter().any(|s| s.category == ResourceCategory::Custom));
}
