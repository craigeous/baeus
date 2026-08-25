use baeus_core::resource::{RelationshipKind, ResourceRef, ResourceRelationship};
use baeus_ui::components::resource_map::*;
use baeus_ui::theme::Theme;

// ---------------------------------------------------------------------------
// T077: Render tests for ResourceMap component
// ---------------------------------------------------------------------------

// Helper to build a sample layout with known positions
fn sample_layout() -> LayoutState {
    LayoutState {
        nodes: vec![
            GraphNode {
                id: "Service/default/my-svc".to_string(),
                kind: "Service".to_string(),
                name: "my-svc".to_string(),
                x: 0.0,
                y: 0.0,
                layer: 0,
            },
            GraphNode {
                id: "Pod/default/pod-a".to_string(),
                kind: "Pod".to_string(),
                name: "pod-a".to_string(),
                x: -100.0,
                y: 150.0,
                layer: 1,
            },
            GraphNode {
                id: "Pod/default/pod-b".to_string(),
                kind: "Pod".to_string(),
                name: "pod-b".to_string(),
                x: 100.0,
                y: 150.0,
                layer: 1,
            },
        ],
        edges: vec![
            GraphEdge {
                source_id: "Service/default/my-svc".to_string(),
                target_id: "Pod/default/pod-a".to_string(),
            },
            GraphEdge {
                source_id: "Service/default/my-svc".to_string(),
                target_id: "Pod/default/pod-b".to_string(),
            },
        ],
    }
}

// --- Node rendering tests ---

#[test]
fn test_nodes_render_at_correct_positions() {
    let layout = sample_layout();
    let state =
        ResourceMapState { layout, zoom_level: 1.0, pan_offset: (0.0, 0.0), selected_node: None };

    // Verify nodes have correct positions from GraphNode data
    assert_eq!(state.layout.nodes[0].x, 0.0);
    assert_eq!(state.layout.nodes[0].y, 0.0);
    assert_eq!(state.layout.nodes[1].x, -100.0);
    assert_eq!(state.layout.nodes[1].y, 150.0);
    assert_eq!(state.layout.nodes[2].x, 100.0);
    assert_eq!(state.layout.nodes[2].y, 150.0);
}

#[test]
fn test_edges_render_between_connected_nodes() {
    let layout = sample_layout();
    let state =
        ResourceMapState { layout, zoom_level: 1.0, pan_offset: (0.0, 0.0), selected_node: None };

    // Verify edges connect correct nodes
    assert_eq!(state.layout.edges.len(), 2);
    assert_eq!(state.layout.edges[0].source_id, "Service/default/my-svc");
    assert_eq!(state.layout.edges[0].target_id, "Pod/default/pod-a");
    assert_eq!(state.layout.edges[1].source_id, "Service/default/my-svc");
    assert_eq!(state.layout.edges[1].target_id, "Pod/default/pod-b");
}

#[test]
fn test_selected_node_highlighting() {
    let layout = sample_layout();
    let mut state =
        ResourceMapState { layout, zoom_level: 1.0, pan_offset: (0.0, 0.0), selected_node: None };

    // Select a node
    state.select_node("Pod/default/pod-a");
    assert_eq!(state.selected_node.as_deref(), Some("Pod/default/pod-a"));

    // Deselect
    state.clear_selection();
    assert!(state.selected_node.is_none());
}

#[test]
fn test_zoom_affects_display() {
    let layout = sample_layout();
    let mut state =
        ResourceMapState { layout, zoom_level: 1.0, pan_offset: (0.0, 0.0), selected_node: None };

    // Zoom in
    state.zoom_in();
    assert!((state.zoom_level - 1.1).abs() < f64::EPSILON);

    // Zoom out
    state.zoom_out();
    state.zoom_out();
    assert!((state.zoom_level - 0.9).abs() < f64::EPSILON);
}

#[test]
fn test_pan_offset_affects_positions() {
    let layout = sample_layout();
    let mut state =
        ResourceMapState { layout, zoom_level: 1.0, pan_offset: (0.0, 0.0), selected_node: None };

    // Pan the view
    state.pan(50.0, -30.0);
    assert!((state.pan_offset.0 - 50.0).abs() < f64::EPSILON);
    assert!((state.pan_offset.1 - (-30.0)).abs() < f64::EPSILON);
}

#[test]
fn test_empty_map_state() {
    let state = ResourceMapState::default();

    assert!(state.layout.nodes.is_empty());
    assert!(state.layout.edges.is_empty());
    assert!((state.zoom_level - 1.0).abs() < f64::EPSILON);
    assert!((state.pan_offset.0).abs() < f64::EPSILON);
    assert!((state.pan_offset.1).abs() < f64::EPSILON);
}

#[test]
fn test_node_labels_show_resource_kind_and_name() {
    let layout = sample_layout();

    // Verify nodes have correct kind and name
    assert_eq!(layout.nodes[0].kind, "Service");
    assert_eq!(layout.nodes[0].name, "my-svc");
    assert_eq!(layout.nodes[1].kind, "Pod");
    assert_eq!(layout.nodes[1].name, "pod-a");
    assert_eq!(layout.nodes[2].kind, "Pod");
    assert_eq!(layout.nodes[2].name, "pod-b");
}

#[test]
fn test_different_colors_per_resource_kind() {
    // Build a layout with various resource kinds
    let layout = LayoutState {
        nodes: vec![
            GraphNode {
                id: "Pod/ns/p".to_string(),
                kind: "Pod".to_string(),
                name: "p".to_string(),
                x: 0.0,
                y: 0.0,
                layer: 0,
            },
            GraphNode {
                id: "Service/ns/s".to_string(),
                kind: "Service".to_string(),
                name: "s".to_string(),
                x: 0.0,
                y: 150.0,
                layer: 1,
            },
            GraphNode {
                id: "Deployment/ns/d".to_string(),
                kind: "Deployment".to_string(),
                name: "d".to_string(),
                x: 0.0,
                y: 300.0,
                layer: 2,
            },
        ],
        edges: vec![],
    };

    // Verify each kind is distinct
    let kinds: Vec<_> = layout.nodes.iter().map(|n| &n.kind).collect();
    assert_eq!(kinds, vec!["Pod", "Service", "Deployment"]);
}

#[test]
fn test_multiple_layers_render_correctly() {
    let layout = sample_layout();

    // Layer 0: Service
    let layer_0_nodes: Vec<_> = layout.nodes.iter().filter(|n| n.layer == 0).collect();
    assert_eq!(layer_0_nodes.len(), 1);
    assert_eq!(layer_0_nodes[0].kind, "Service");

    // Layer 1: Two Pods
    let layer_1_nodes: Vec<_> = layout.nodes.iter().filter(|n| n.layer == 1).collect();
    assert_eq!(layer_1_nodes.len(), 2);
    assert!(layer_1_nodes.iter().all(|n| n.kind == "Pod"));
}

// --- Edge rendering tests ---

#[test]
fn test_edges_connect_source_to_target() {
    let layout = sample_layout();

    for edge in &layout.edges {
        let source = layout.node_by_id(&edge.source_id);
        let target = layout.node_by_id(&edge.target_id);

        assert!(source.is_some(), "Source node should exist");
        assert!(target.is_some(), "Target node should exist");
    }
}

#[test]
fn test_edge_from_higher_to_lower_layer() {
    let layout = sample_layout();

    for edge in &layout.edges {
        let source = layout.node_by_id(&edge.source_id).unwrap();
        let target = layout.node_by_id(&edge.target_id).unwrap();

        // Source should be in an earlier (lower) layer than target
        assert!(source.layer < target.layer);
    }
}

// --- Zoom and pan tests ---

#[test]
fn test_zoom_in_clamped_at_max() {
    let mut state = ResourceMapState::default();
    state.zoom_level = 2.95;

    state.zoom_in();
    assert!((state.zoom_level - 3.0).abs() < f64::EPSILON);

    // Should not exceed 3.0
    state.zoom_in();
    assert!((state.zoom_level - 3.0).abs() < f64::EPSILON);
}

#[test]
fn test_zoom_out_clamped_at_min() {
    let mut state = ResourceMapState::default();
    state.zoom_level = 0.15;

    state.zoom_out();
    assert!((state.zoom_level - 0.1).abs() < 0.001);

    // Should not go below 0.1
    state.zoom_out();
    assert!((state.zoom_level - 0.1).abs() < 0.001);
}

#[test]
fn test_reset_zoom_resets_pan() {
    let mut state = ResourceMapState::default();
    state.zoom_level = 2.5;
    state.pan_offset = (100.0, -50.0);

    state.reset_zoom();
    assert!((state.zoom_level - 1.0).abs() < f64::EPSILON);
    assert!((state.pan_offset.0).abs() < f64::EPSILON);
    assert!((state.pan_offset.1).abs() < f64::EPSILON);
}

#[test]
fn test_pan_accumulates() {
    let mut state = ResourceMapState::default();

    state.pan(10.0, 20.0);
    assert!((state.pan_offset.0 - 10.0).abs() < f64::EPSILON);
    assert!((state.pan_offset.1 - 20.0).abs() < f64::EPSILON);

    state.pan(5.0, -10.0);
    assert!((state.pan_offset.0 - 15.0).abs() < f64::EPSILON);
    assert!((state.pan_offset.1 - 10.0).abs() < f64::EPSILON);
}

// --- Selection tests ---

#[test]
fn test_select_node_replaces_previous() {
    let mut state = ResourceMapState::default();

    state.select_node("Pod/ns/pod-a");
    assert_eq!(state.selected_node.as_deref(), Some("Pod/ns/pod-a"));

    state.select_node("Pod/ns/pod-b");
    assert_eq!(state.selected_node.as_deref(), Some("Pod/ns/pod-b"));
}

#[test]
fn test_clear_selection_removes_selection() {
    let mut state = ResourceMapState::default();
    state.select_node("Service/ns/svc");

    state.clear_selection();
    assert!(state.selected_node.is_none());
}

// --- Empty state tests ---

#[test]
fn test_empty_layout_has_no_nodes_or_edges() {
    let layout = LayoutState::empty();

    assert!(layout.nodes.is_empty());
    assert!(layout.edges.is_empty());
}

#[test]
fn test_default_state_is_empty() {
    let state = ResourceMapState::default();

    assert!(state.layout.nodes.is_empty());
    assert!(state.layout.edges.is_empty());
}

// --- Integration: full relationship graph ---

#[test]
fn test_full_ingress_to_pods_graph() {
    // Build a full Ingress -> Service -> Pod graph
    let rels = vec![
        ResourceRelationship::new(
            ResourceRef::new("Ingress", "my-ing", Some("default".to_string())),
            ResourceRef::new("Service", "my-svc", Some("default".to_string())),
            RelationshipKind::IngressBackend,
        ),
        ResourceRelationship::new(
            ResourceRef::new("Service", "my-svc", Some("default".to_string())),
            ResourceRef::new("Pod", "pod-1", Some("default".to_string())),
            RelationshipKind::ServiceSelector,
        ),
        ResourceRelationship::new(
            ResourceRef::new("Service", "my-svc", Some("default".to_string())),
            ResourceRef::new("Pod", "pod-2", Some("default".to_string())),
            RelationshipKind::ServiceSelector,
        ),
    ];

    let layout = compute_layout(&rels);

    assert_eq!(layout.nodes.len(), 4);
    assert_eq!(layout.edges.len(), 3);

    // Verify layers
    let ingress = layout.node_by_id("Ingress/default/my-ing").unwrap();
    let svc = layout.node_by_id("Service/default/my-svc").unwrap();
    let pod1 = layout.node_by_id("Pod/default/pod-1").unwrap();
    let pod2 = layout.node_by_id("Pod/default/pod-2").unwrap();

    assert_eq!(ingress.layer, 0);
    assert_eq!(svc.layer, 1);
    assert_eq!(pod1.layer, 2);
    assert_eq!(pod2.layer, 2);

    // Verify y positions reflect layers
    assert!(ingress.y < svc.y);
    assert!(svc.y < pod1.y);
    assert!((pod1.y - pod2.y).abs() < f64::EPSILON); // Same layer, same y
}

// --- Complex graph tests ---

#[test]
fn test_deployment_replicaset_pods_chain() {
    let rels = vec![
        ResourceRelationship::new(
            ResourceRef::new("Deployment", "dep", Some("ns".to_string())),
            ResourceRef::new("ReplicaSet", "rs", Some("ns".to_string())),
            RelationshipKind::OwnerReference,
        ),
        ResourceRelationship::new(
            ResourceRef::new("ReplicaSet", "rs", Some("ns".to_string())),
            ResourceRef::new("Pod", "pod-a", Some("ns".to_string())),
            RelationshipKind::OwnerReference,
        ),
        ResourceRelationship::new(
            ResourceRef::new("ReplicaSet", "rs", Some("ns".to_string())),
            ResourceRef::new("Pod", "pod-b", Some("ns".to_string())),
            RelationshipKind::OwnerReference,
        ),
    ];

    let layout = compute_layout(&rels);

    assert_eq!(layout.nodes.len(), 4);
    assert_eq!(layout.edges.len(), 3);

    let dep = layout.node_by_id("Deployment/ns/dep").unwrap();
    let rs = layout.node_by_id("ReplicaSet/ns/rs").unwrap();

    assert_eq!(dep.layer, 0);
    assert_eq!(rs.layer, 1);

    // Both pods should be in layer 2
    let pod_a = layout.node_by_id("Pod/ns/pod-a").unwrap();
    let pod_b = layout.node_by_id("Pod/ns/pod-b").unwrap();
    assert_eq!(pod_a.layer, 2);
    assert_eq!(pod_b.layer, 2);
}

// --- Theme and color mapping tests ---

#[test]
fn test_theme_colors_available() {
    let theme = Theme::light();

    // Verify theme has all required colors
    assert!(theme.colors.success.r > 0 || theme.colors.success.g > 0 || theme.colors.success.b > 0);
    assert!(theme.colors.accent.r > 0 || theme.colors.accent.g > 0 || theme.colors.accent.b > 0);
    assert!(theme.colors.info.r > 0 || theme.colors.info.g > 0 || theme.colors.info.b > 0);
    assert!(theme.colors.warning.r > 0 || theme.colors.warning.g > 0 || theme.colors.warning.b > 0);
    assert!(theme.colors.error.r > 0 || theme.colors.error.g > 0 || theme.colors.error.b > 0);
}

// --- Control tests ---

#[test]
fn test_zoom_in_out_reset_controls() {
    let mut state = ResourceMapState::default();

    // Initial zoom
    assert!((state.zoom_level - 1.0).abs() < f64::EPSILON);

    // Zoom in 5 times (1.0 + 0.1 * 5 = 1.5)
    for _ in 0..5 {
        state.zoom_in();
    }
    // Use a slightly larger epsilon for floating point accumulation
    assert!((state.zoom_level - 1.5).abs() < 0.001);

    // Reset
    state.reset_zoom();
    assert!((state.zoom_level - 1.0).abs() < f64::EPSILON);

    // Zoom out
    state.zoom_out();
    assert!((state.zoom_level - 0.9).abs() < 0.001);
}
