//! Tests for the navigator indent guide computation and tree flattening.

use baeus_ui::icons::ResourceCategory;
use baeus_ui::layout::indent_guides::{IndentGuideLayout, compute_indent_guides};
use baeus_ui::layout::sidebar::{NavigatorFlatEntry, SidebarState};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// compute_indent_guides tests
// ---------------------------------------------------------------------------

#[test]
fn test_empty_entries_produces_no_guides() {
    let guides = compute_indent_guides(&[]);
    assert!(guides.is_empty());
}

#[test]
fn test_single_leaf_produces_one_guide() {
    let entries = vec![NavigatorFlatEntry::Leaf {
        depth: 1,
        label: "Overview".into(),
        target_kind: "__Dashboard__".into(),
        cluster_id: Uuid::nil(),
        context_name: "test".into(),
        is_last_sibling: true,
    }];
    let guides = compute_indent_guides(&entries);
    assert_eq!(guides.len(), 1);
    assert_eq!(guides[0], IndentGuideLayout { depth: 1, start_row: 0, end_row: 0 });
}

#[test]
fn test_all_leaves_one_depth1_guide() {
    let entries = vec![
        NavigatorFlatEntry::Leaf {
            depth: 1,
            label: "Overview".into(),
            target_kind: "__Dashboard__".into(),
            cluster_id: Uuid::nil(),
            context_name: "test".into(),
            is_last_sibling: false,
        },
        NavigatorFlatEntry::Leaf {
            depth: 1,
            label: "Nodes".into(),
            target_kind: "Node".into(),
            cluster_id: Uuid::nil(),
            context_name: "test".into(),
            is_last_sibling: false,
        },
        NavigatorFlatEntry::Leaf {
            depth: 1,
            label: "Events".into(),
            target_kind: "Event".into(),
            cluster_id: Uuid::nil(),
            context_name: "test".into(),
            is_last_sibling: true,
        },
    ];
    let guides = compute_indent_guides(&entries);
    assert_eq!(guides.len(), 1);
    assert_eq!(guides[0], IndentGuideLayout { depth: 1, start_row: 0, end_row: 2 });
}

#[test]
fn test_branch_with_expanded_children_produces_depth2_guides() {
    // Overview (depth 1, not last)
    // Workloads (depth 1, not last) — category header
    //   Pod (depth 2, not last)
    //   Deployment (depth 2, last)
    // Events (depth 1, last)
    let entries = vec![
        NavigatorFlatEntry::Leaf {
            depth: 1,
            label: "Overview".into(),
            target_kind: "__Dashboard__".into(),
            cluster_id: Uuid::nil(),
            context_name: "test".into(),
            is_last_sibling: false,
        },
        NavigatorFlatEntry::CategoryHeader {
            depth: 1,
            label: "Workloads".into(),
            category: ResourceCategory::Workloads,
            cluster_id: Uuid::nil(),
            expanded: true,
            is_last_sibling: false,
        },
        NavigatorFlatEntry::ResourceKind {
            depth: 2,
            label: "Pod".into(),
            kind: "Pod".into(),
            category: ResourceCategory::Workloads,
            cluster_id: Uuid::nil(),
            context_name: "test".into(),
            badge_count: Some(5),
            is_last_sibling: false,
        },
        NavigatorFlatEntry::ResourceKind {
            depth: 2,
            label: "Deployment".into(),
            kind: "Deployment".into(),
            category: ResourceCategory::Workloads,
            cluster_id: Uuid::nil(),
            context_name: "test".into(),
            badge_count: None,
            is_last_sibling: true,
        },
        NavigatorFlatEntry::Leaf {
            depth: 1,
            label: "Events".into(),
            target_kind: "Event".into(),
            cluster_id: Uuid::nil(),
            context_name: "test".into(),
            is_last_sibling: true,
        },
    ];

    let guides = compute_indent_guides(&entries);

    // Should have a depth-2 guide (rows 2-3) and a depth-1 guide (rows 0-4)
    let depth1: Vec<_> = guides.iter().filter(|g| g.depth == 1).collect();
    let depth2: Vec<_> = guides.iter().filter(|g| g.depth == 2).collect();

    assert_eq!(depth1.len(), 1);
    assert_eq!(depth1[0].start_row, 0);
    assert_eq!(depth1[0].end_row, 4);

    assert_eq!(depth2.len(), 1);
    assert_eq!(depth2[0].start_row, 2);
    assert_eq!(depth2[0].end_row, 3);
}

#[test]
fn test_multiple_expanded_branches_produce_separate_depth2_guides() {
    // Overview (depth 1)
    // Workloads (depth 1) — expanded
    //   Pod (depth 2, not last)
    //   Deployment (depth 2, last)
    // Config (depth 1) — expanded
    //   ConfigMap (depth 2, not last)
    //   Secret (depth 2, last)
    // Events (depth 1, last)
    let entries = vec![
        NavigatorFlatEntry::Leaf {
            depth: 1,
            label: "Overview".into(),
            target_kind: "__Dashboard__".into(),
            cluster_id: Uuid::nil(),
            context_name: "test".into(),
            is_last_sibling: false,
        },
        NavigatorFlatEntry::CategoryHeader {
            depth: 1,
            label: "Workloads".into(),
            category: ResourceCategory::Workloads,
            cluster_id: Uuid::nil(),
            expanded: true,
            is_last_sibling: false,
        },
        NavigatorFlatEntry::ResourceKind {
            depth: 2,
            label: "Pod".into(),
            kind: "Pod".into(),
            category: ResourceCategory::Workloads,
            cluster_id: Uuid::nil(),
            context_name: "test".into(),
            badge_count: None,
            is_last_sibling: false,
        },
        NavigatorFlatEntry::ResourceKind {
            depth: 2,
            label: "Deployment".into(),
            kind: "Deployment".into(),
            category: ResourceCategory::Workloads,
            cluster_id: Uuid::nil(),
            context_name: "test".into(),
            badge_count: None,
            is_last_sibling: true,
        },
        NavigatorFlatEntry::CategoryHeader {
            depth: 1,
            label: "Config".into(),
            category: ResourceCategory::Configuration,
            cluster_id: Uuid::nil(),
            expanded: true,
            is_last_sibling: false,
        },
        NavigatorFlatEntry::ResourceKind {
            depth: 2,
            label: "ConfigMap".into(),
            kind: "ConfigMap".into(),
            category: ResourceCategory::Configuration,
            cluster_id: Uuid::nil(),
            context_name: "test".into(),
            badge_count: None,
            is_last_sibling: false,
        },
        NavigatorFlatEntry::ResourceKind {
            depth: 2,
            label: "Secret".into(),
            kind: "Secret".into(),
            category: ResourceCategory::Configuration,
            cluster_id: Uuid::nil(),
            context_name: "test".into(),
            badge_count: None,
            is_last_sibling: true,
        },
        NavigatorFlatEntry::Leaf {
            depth: 1,
            label: "Events".into(),
            target_kind: "Event".into(),
            cluster_id: Uuid::nil(),
            context_name: "test".into(),
            is_last_sibling: true,
        },
    ];

    let guides = compute_indent_guides(&entries);

    let depth1: Vec<_> = guides.iter().filter(|g| g.depth == 1).collect();
    let depth2: Vec<_> = guides.iter().filter(|g| g.depth == 2).collect();

    // One depth-1 guide spanning all rows
    assert_eq!(depth1.len(), 1);
    assert_eq!(depth1[0].start_row, 0);
    assert_eq!(depth1[0].end_row, 7);

    // Two separate depth-2 guide segments
    assert_eq!(depth2.len(), 2);
    // First segment: Workloads children (rows 2-3)
    assert_eq!(depth2[0].start_row, 2);
    assert_eq!(depth2[0].end_row, 3);
    // Second segment: Config children (rows 5-6)
    assert_eq!(depth2[1].start_row, 5);
    assert_eq!(depth2[1].end_row, 6);
}

#[test]
fn test_last_sibling_terminates_guide() {
    // Two entries at depth 1, first is last_sibling=true (unusual but possible)
    // This tests that the guide terminates correctly
    let entries = vec![NavigatorFlatEntry::Leaf {
        depth: 1,
        label: "Only".into(),
        target_kind: "Node".into(),
        cluster_id: Uuid::nil(),
        context_name: "test".into(),
        is_last_sibling: true,
    }];
    let guides = compute_indent_guides(&entries);
    assert_eq!(guides.len(), 1);
    assert_eq!(guides[0].start_row, 0);
    assert_eq!(guides[0].end_row, 0);
}

#[test]
fn test_collapsed_category_has_no_depth2_guides() {
    let entries = vec![NavigatorFlatEntry::CategoryHeader {
        depth: 1,
        label: "Workloads".into(),
        category: ResourceCategory::Workloads,
        cluster_id: Uuid::nil(),
        expanded: false,
        is_last_sibling: true,
    }];
    let guides = compute_indent_guides(&entries);
    // Only depth-1 guide, no depth-2
    let depth2: Vec<_> = guides.iter().filter(|g| g.depth == 2).collect();
    assert!(depth2.is_empty());
}

// ---------------------------------------------------------------------------
// flatten_navigator_tree tests
// ---------------------------------------------------------------------------

#[test]
fn test_flatten_collapsed_categories_only_depth1() {
    let (state, mut cluster) = make_test_cluster();
    // Ensure no categories are expanded
    cluster.expanded_categories.clear();

    let entries = state.flatten_navigator_tree(&cluster);

    // All entries should be depth 1 (no depth-2 children)
    for entry in &entries {
        assert_eq!(entry.depth(), 1, "Expected all entries at depth 1, got {:?}", entry);
    }

    // Should have the same count as navigator_tree()
    let nav_tree = SidebarState::navigator_tree();
    assert_eq!(entries.len(), nav_tree.len());
}

#[test]
fn test_flatten_expanded_category_has_depth2_children() {
    let (state, mut cluster) = make_test_cluster();
    cluster.expanded_categories.insert(ResourceCategory::Workloads);

    let entries = state.flatten_navigator_tree(&cluster);

    // Should have some depth-2 entries (Workloads children)
    let depth2_count = entries.iter().filter(|e| e.depth() == 2).count();
    assert!(depth2_count > 0, "Expected depth-2 entries for expanded Workloads category");

    // Depth-2 entries should be ResourceKind variants
    for entry in entries.iter().filter(|e| e.depth() == 2) {
        assert!(
            matches!(entry, NavigatorFlatEntry::ResourceKind { .. }),
            "Expected ResourceKind at depth 2, got {:?}",
            entry,
        );
    }
}

#[test]
fn test_flatten_is_last_sibling_correct_for_top_level() {
    let (state, mut cluster) = make_test_cluster();
    cluster.expanded_categories.clear();

    let entries = state.flatten_navigator_tree(&cluster);
    assert!(!entries.is_empty());

    // Only the very last entry should have is_last_sibling=true
    for (i, entry) in entries.iter().enumerate() {
        if i == entries.len() - 1 {
            assert!(entry.is_last_sibling(), "Last entry should be is_last_sibling=true",);
        } else {
            assert!(!entry.is_last_sibling(), "Entry {} should be is_last_sibling=false", i,);
        }
    }
}

#[test]
fn test_flatten_is_last_sibling_correct_for_depth2() {
    let (state, mut cluster) = make_test_cluster();
    cluster.expanded_categories.insert(ResourceCategory::Workloads);

    let entries = state.flatten_navigator_tree(&cluster);

    // Find the depth-2 entries (Workloads children)
    let depth2: Vec<_> = entries.iter().filter(|e| e.depth() == 2).collect();
    assert!(!depth2.is_empty());

    // Only the last depth-2 entry should have is_last_sibling=true
    for (i, entry) in depth2.iter().enumerate() {
        if i == depth2.len() - 1 {
            assert!(entry.is_last_sibling(), "Last depth-2 entry should be is_last_sibling=true",);
        } else {
            assert!(
                !entry.is_last_sibling(),
                "Depth-2 entry {} should be is_last_sibling=false",
                i,
            );
        }
    }
}

#[test]
fn test_flatten_first_entry_is_overview() {
    let (state, cluster) = make_test_cluster();

    let entries = state.flatten_navigator_tree(&cluster);
    assert!(!entries.is_empty());

    match &entries[0] {
        NavigatorFlatEntry::Leaf { label, target_kind, .. } => {
            assert_eq!(label, "Overview");
            assert_eq!(target_kind, "__Dashboard__");
        }
        other => panic!("Expected Leaf for first entry, got {:?}", other),
    }
}

#[test]
fn test_flatten_category_header_has_correct_expanded_state() {
    let (state, mut cluster) = make_test_cluster();
    cluster.expanded_categories.insert(ResourceCategory::Workloads);

    let entries = state.flatten_navigator_tree(&cluster);

    // Find the Workloads category header
    let workloads_header = entries.iter().find(|e| {
        matches!(
            e,
            NavigatorFlatEntry::CategoryHeader { category: ResourceCategory::Workloads, .. }
        )
    });
    assert!(workloads_header.is_some());

    match workloads_header.unwrap() {
        NavigatorFlatEntry::CategoryHeader { expanded, .. } => {
            assert!(*expanded, "Workloads should be expanded");
        }
        _ => unreachable!(),
    }

    // Network should not be expanded
    let network_header = entries.iter().find(|e| {
        matches!(e, NavigatorFlatEntry::CategoryHeader { category: ResourceCategory::Network, .. })
    });
    assert!(network_header.is_some());

    match network_header.unwrap() {
        NavigatorFlatEntry::CategoryHeader { expanded, .. } => {
            assert!(!*expanded, "Network should not be expanded");
        }
        _ => unreachable!(),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_test_cluster() -> (SidebarState, baeus_ui::layout::sidebar::ClusterEntry) {
    let mut state = SidebarState::default();
    let id = state.add_cluster("test-cluster", "Test Cluster");
    let cluster = state.clusters.iter().find(|c| c.id == id).unwrap().clone();
    (state, cluster)
}
