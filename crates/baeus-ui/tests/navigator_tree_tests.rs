use baeus_ui::layout::sidebar::{NavigatorNode, SidebarState, tree_connector_prefix};

#[test]
fn test_tree_connector_depth_0() {
    // No prefix at depth 0
    assert_eq!(tree_connector_prefix(0, false, &[]), "");
    assert_eq!(tree_connector_prefix(0, true, &[]), "");
}

#[test]
fn test_tree_connector_depth_1_not_last() {
    assert_eq!(tree_connector_prefix(1, false, &[]), "├─ ");
}

#[test]
fn test_tree_connector_depth_1_last() {
    assert_eq!(tree_connector_prefix(1, true, &[]), "└─ ");
}

#[test]
fn test_tree_connector_depth_2_parent_has_more() {
    assert_eq!(tree_connector_prefix(2, false, &[true]), "│  ├─ ");
    assert_eq!(tree_connector_prefix(2, true, &[true]), "│  └─ ");
}

#[test]
fn test_tree_connector_depth_2_parent_no_more() {
    assert_eq!(tree_connector_prefix(2, false, &[false]), "   ├─ ");
    assert_eq!(tree_connector_prefix(2, true, &[false]), "   └─ ");
}

#[test]
fn test_tree_connector_depth_3() {
    assert_eq!(tree_connector_prefix(3, false, &[true, true]), "│  │  ├─ ");
    assert_eq!(tree_connector_prefix(3, true, &[true, false]), "│     └─ ");
    assert_eq!(tree_connector_prefix(3, false, &[false, true]), "   │  ├─ ");
}

#[test]
fn test_navigator_tree_returns_correct_count() {
    let tree = SidebarState::navigator_tree();
    assert_eq!(tree.len(), 13);
}

#[test]
fn test_navigator_tree_first_is_overview() {
    let tree = SidebarState::navigator_tree();
    match &tree[0] {
        NavigatorNode::Leaf { label, target_kind } => {
            assert_eq!(*label, "Overview");
            assert_eq!(*target_kind, "__Dashboard__");
        }
        _ => panic!("First item should be Overview leaf"),
    }
}

#[test]
fn test_navigator_tree_has_workloads_branch() {
    let tree = SidebarState::navigator_tree();
    let has_workloads =
        tree.iter().any(|n| matches!(n, NavigatorNode::Branch { label: "Workloads", .. }));
    assert!(has_workloads, "Tree should contain Workloads branch");
}

#[test]
fn test_navigator_tree_events_is_leaf() {
    let tree = SidebarState::navigator_tree();
    let events = tree.iter().find(|n| matches!(n, NavigatorNode::Leaf { label: "Events", .. }));
    assert!(events.is_some(), "Events should be a leaf node");
}

#[test]
fn test_navigator_tree_leaf_and_branch_count() {
    let tree = SidebarState::navigator_tree();
    let leaf_count = tree.iter().filter(|n| matches!(n, NavigatorNode::Leaf { .. })).count();
    let branch_count = tree.iter().filter(|n| matches!(n, NavigatorNode::Branch { .. })).count();
    assert_eq!(leaf_count, 5); // Overview, Topology, Nodes, Namespaces, Events
    assert_eq!(branch_count, 8); // Workloads, Config, Network, Storage, Helm, Access Control, ArgoCD, Custom Resources
}
