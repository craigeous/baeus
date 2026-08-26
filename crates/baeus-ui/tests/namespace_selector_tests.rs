// T088: Multi-Namespace Selector tests

use baeus_ui::layout::header::EnhancedNamespaceSelector;

// ========================================================================
// Construction and defaults
// ========================================================================

#[test]
fn test_enhanced_ns_new_defaults() {
    let sel = EnhancedNamespaceSelector::new();
    assert!(sel.selected_namespaces.is_empty());
    assert!(sel.available_namespaces.is_empty());
    assert!(!sel.is_dropdown_open);
}

#[test]
fn test_enhanced_ns_default_trait() {
    let sel = EnhancedNamespaceSelector::default();
    assert!(sel.selected_namespaces.is_empty());
    assert!(!sel.is_dropdown_open);
}

// ========================================================================
// Toggle individual namespaces
// ========================================================================

#[test]
fn test_enhanced_ns_toggle_adds_namespace() {
    let mut sel = EnhancedNamespaceSelector::new();
    sel.toggle_namespace("default");
    assert_eq!(sel.selected_count(), 1);
    assert!(sel.selected_namespaces.contains(&"default".to_string()));
}

#[test]
fn test_enhanced_ns_toggle_removes_namespace() {
    let mut sel = EnhancedNamespaceSelector::new();
    sel.toggle_namespace("default");
    sel.toggle_namespace("default");
    assert_eq!(sel.selected_count(), 0);
}

#[test]
fn test_enhanced_ns_toggle_multiple() {
    let mut sel = EnhancedNamespaceSelector::new();
    sel.toggle_namespace("default");
    sel.toggle_namespace("kube-system");
    sel.toggle_namespace("monitoring");
    assert_eq!(sel.selected_count(), 3);
}

#[test]
fn test_enhanced_ns_toggle_remove_middle() {
    let mut sel = EnhancedNamespaceSelector::new();
    sel.toggle_namespace("a");
    sel.toggle_namespace("b");
    sel.toggle_namespace("c");
    sel.toggle_namespace("b"); // remove b
    assert_eq!(sel.selected_count(), 2);
    assert!(!sel.selected_namespaces.contains(&"b".to_string()));
    assert!(sel.selected_namespaces.contains(&"a".to_string()));
    assert!(sel.selected_namespaces.contains(&"c".to_string()));
}

// ========================================================================
// Multi-selection tracking
// ========================================================================

#[test]
fn test_enhanced_ns_is_namespace_selected_empty_means_all() {
    let sel = EnhancedNamespaceSelector::new();
    // Empty selection = all namespaces
    assert!(sel.is_namespace_selected("default"));
    assert!(sel.is_namespace_selected("kube-system"));
    assert!(sel.is_namespace_selected("anything"));
}

#[test]
fn test_enhanced_ns_is_namespace_selected_with_selection() {
    let mut sel = EnhancedNamespaceSelector::new();
    sel.toggle_namespace("default");
    assert!(sel.is_namespace_selected("default"));
    assert!(!sel.is_namespace_selected("kube-system"));
}

#[test]
fn test_enhanced_ns_selected_count_empty() {
    let sel = EnhancedNamespaceSelector::new();
    assert_eq!(sel.selected_count(), 0);
}

#[test]
fn test_enhanced_ns_selected_count_after_toggles() {
    let mut sel = EnhancedNamespaceSelector::new();
    sel.toggle_namespace("a");
    sel.toggle_namespace("b");
    assert_eq!(sel.selected_count(), 2);
    sel.toggle_namespace("a");
    assert_eq!(sel.selected_count(), 1);
}

// ========================================================================
// Display label for 0, 1, 2, many selections
// ========================================================================

#[test]
fn test_enhanced_ns_display_label_none() {
    let sel = EnhancedNamespaceSelector::new();
    assert_eq!(sel.display_label(), "All Namespaces");
}

#[test]
fn test_enhanced_ns_display_label_one() {
    let mut sel = EnhancedNamespaceSelector::new();
    sel.toggle_namespace("default");
    assert_eq!(sel.display_label(), "Namespace: default");
}

#[test]
fn test_enhanced_ns_display_label_two() {
    let mut sel = EnhancedNamespaceSelector::new();
    sel.toggle_namespace("default");
    sel.toggle_namespace("kube-system");
    assert_eq!(sel.display_label(), "Namespaces: default, kube-system");
}

#[test]
fn test_enhanced_ns_display_label_many() {
    let mut sel = EnhancedNamespaceSelector::new();
    sel.toggle_namespace("a");
    sel.toggle_namespace("b");
    sel.toggle_namespace("c");
    sel.toggle_namespace("d");
    sel.toggle_namespace("e");
    assert_eq!(sel.display_label(), "Namespaces: a, b, c, d, e");
}

#[test]
fn test_enhanced_ns_display_label_after_clear() {
    let mut sel = EnhancedNamespaceSelector::new();
    sel.toggle_namespace("default");
    sel.clear_selection();
    assert_eq!(sel.display_label(), "All Namespaces");
}

// ========================================================================
// matches_namespace filtering
// ========================================================================

#[test]
fn test_enhanced_ns_matches_namespace_all() {
    let sel = EnhancedNamespaceSelector::new();
    assert!(sel.matches_namespace("default"));
    assert!(sel.matches_namespace("kube-system"));
    assert!(sel.matches_namespace("anything-goes"));
}

#[test]
fn test_enhanced_ns_matches_namespace_filtered() {
    let mut sel = EnhancedNamespaceSelector::new();
    sel.toggle_namespace("default");
    sel.toggle_namespace("monitoring");
    assert!(sel.matches_namespace("default"));
    assert!(sel.matches_namespace("monitoring"));
    assert!(!sel.matches_namespace("kube-system"));
    assert!(!sel.matches_namespace("staging"));
}

#[test]
fn test_enhanced_ns_matches_namespace_single() {
    let mut sel = EnhancedNamespaceSelector::new();
    sel.toggle_namespace("production");
    assert!(sel.matches_namespace("production"));
    assert!(!sel.matches_namespace("staging"));
}

// ========================================================================
// Dropdown open/close
// ========================================================================

#[test]
fn test_enhanced_ns_toggle_dropdown_open() {
    let mut sel = EnhancedNamespaceSelector::new();
    sel.toggle_dropdown();
    assert!(sel.is_dropdown_open);
}

#[test]
fn test_enhanced_ns_toggle_dropdown_close() {
    let mut sel = EnhancedNamespaceSelector::new();
    sel.toggle_dropdown();
    sel.toggle_dropdown();
    assert!(!sel.is_dropdown_open);
}

#[test]
fn test_enhanced_ns_dropdown_default_closed() {
    let sel = EnhancedNamespaceSelector::new();
    assert!(!sel.is_dropdown_open);
}

// ========================================================================
// Clear selection
// ========================================================================

#[test]
fn test_enhanced_ns_clear_selection() {
    let mut sel = EnhancedNamespaceSelector::new();
    sel.toggle_namespace("a");
    sel.toggle_namespace("b");
    sel.toggle_namespace("c");
    sel.clear_selection();
    assert_eq!(sel.selected_count(), 0);
    assert!(sel.selected_namespaces.is_empty());
}

#[test]
fn test_enhanced_ns_clear_selection_reverts_to_all() {
    let mut sel = EnhancedNamespaceSelector::new();
    sel.toggle_namespace("default");
    sel.clear_selection();
    assert!(sel.matches_namespace("default"));
    assert!(sel.matches_namespace("anything"));
    assert_eq!(sel.display_label(), "All Namespaces");
}

// ========================================================================
// Available namespaces management
// ========================================================================

#[test]
fn test_enhanced_ns_set_available_namespaces() {
    let mut sel = EnhancedNamespaceSelector::new();
    sel.set_available_namespaces(vec![
        "default".to_string(),
        "kube-system".to_string(),
        "monitoring".to_string(),
    ]);
    assert_eq!(sel.available_namespaces.len(), 3);
}

#[test]
fn test_enhanced_ns_select_all_available() {
    let mut sel = EnhancedNamespaceSelector::new();
    sel.set_available_namespaces(vec!["default".to_string(), "kube-system".to_string()]);
    sel.select_all_available();
    assert_eq!(sel.selected_count(), 2);
    assert!(sel.is_namespace_selected("default"));
    assert!(sel.is_namespace_selected("kube-system"));
}

#[test]
fn test_enhanced_ns_select_all_then_clear() {
    let mut sel = EnhancedNamespaceSelector::new();
    sel.set_available_namespaces(vec!["a".to_string(), "b".to_string()]);
    sel.select_all_available();
    assert_eq!(sel.selected_count(), 2);
    sel.clear_selection();
    assert_eq!(sel.selected_count(), 0);
    assert_eq!(sel.display_label(), "All Namespaces");
}

// ========================================================================
// Serialization roundtrip
// ========================================================================

#[test]
fn test_enhanced_ns_serialization() {
    let mut sel = EnhancedNamespaceSelector::new();
    sel.set_available_namespaces(vec!["default".to_string(), "monitoring".to_string()]);
    sel.toggle_namespace("default");
    sel.is_dropdown_open = true;

    let json = serde_json::to_string(&sel).unwrap();
    let deser: EnhancedNamespaceSelector = serde_json::from_str(&json).unwrap();

    assert_eq!(deser.selected_namespaces.len(), 1);
    assert_eq!(deser.available_namespaces.len(), 2);
    assert!(deser.is_dropdown_open);
}

// ========================================================================
// NamespaceSelectorViewComponent
// ========================================================================

#[test]
fn test_ns_view_component_creation() {
    use baeus_ui::layout::header::NamespaceSelectorViewComponent;

    let sel = EnhancedNamespaceSelector::new();
    let component = NamespaceSelectorViewComponent::new(sel);
    assert!(!component.selector.is_dropdown_open);
    assert_eq!(component.selector.display_label(), "All Namespaces");
}

#[test]
fn test_ns_view_component_with_selections() {
    use baeus_ui::layout::header::NamespaceSelectorViewComponent;

    let mut sel = EnhancedNamespaceSelector::new();
    sel.set_available_namespaces(vec![
        "default".to_string(),
        "kube-system".to_string(),
        "monitoring".to_string(),
    ]);
    sel.toggle_namespace("default");
    sel.toggle_namespace("monitoring");

    let component = NamespaceSelectorViewComponent::new(sel);
    assert_eq!(component.selector.display_label(), "Namespaces: default, monitoring");
    assert!(component.selector.is_namespace_selected("default"));
    assert!(component.selector.is_namespace_selected("monitoring"));
    assert!(!component.selector.is_namespace_selected("kube-system"));
}
