use baeus_core::resource::{ResourceRef, ResourceRelationship};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// T118: Graph layout types
// ---------------------------------------------------------------------------

/// A node in the graph layout with computed position.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphNode {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub layer: usize,
}

/// An edge in the graph layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphEdge {
    pub source_id: String,
    pub target_id: String,
}

/// The full layout state produced by the layout algorithm.
#[derive(Debug, Clone)]
pub struct LayoutState {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

impl LayoutState {
    pub fn empty() -> Self {
        Self { nodes: Vec::new(), edges: Vec::new() }
    }

    pub fn node_by_id(&self, id: &str) -> Option<&GraphNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn edges_from(&self, source_id: &str) -> Vec<&GraphEdge> {
        self.edges.iter().filter(|e| e.source_id == source_id).collect()
    }

    pub fn edges_to(&self, target_id: &str) -> Vec<&GraphEdge> {
        self.edges.iter().filter(|e| e.target_id == target_id).collect()
    }
}

/// Sugiyama-style layered graph layout algorithm.
///
/// Steps:
/// 1. Assign layers using longest-path layering (sources at layer 0).
/// 2. Order nodes within layers using barycenter heuristic.
/// 3. Assign x/y positions based on layer and order.
pub fn compute_layout(relationships: &[ResourceRelationship]) -> LayoutState {
    if relationships.is_empty() {
        return LayoutState::empty();
    }

    // Collect all unique nodes
    let mut node_set: HashMap<String, ResourceRef> = HashMap::new();
    let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
    let mut reverse_adjacency: HashMap<String, Vec<String>> = HashMap::new();

    for rel in relationships {
        let source_key = rel.source.key();
        let target_key = rel.target.key();

        node_set.entry(source_key.clone()).or_insert_with(|| rel.source.clone());
        node_set.entry(target_key.clone()).or_insert_with(|| rel.target.clone());

        adjacency.entry(source_key.clone()).or_default().push(target_key.clone());
        reverse_adjacency.entry(target_key.clone()).or_default().push(source_key.clone());

        // Ensure entries exist even for leaf/root nodes
        adjacency.entry(target_key).or_default();
        reverse_adjacency.entry(source_key).or_default();
    }

    // Step 1: Assign layers via longest-path layering
    let layers = assign_layers(&node_set, &adjacency, &reverse_adjacency);

    // Step 2: Group nodes by layer
    let max_layer = layers.values().copied().max().unwrap_or(0);
    let mut layer_groups: Vec<Vec<String>> = vec![Vec::new(); max_layer + 1];
    for (key, &layer) in &layers {
        layer_groups[layer].push(key.clone());
    }

    // Step 3: Order within layers using barycenter heuristic
    order_layers_by_barycenter(&mut layer_groups, &reverse_adjacency);

    // Step 4: Assign positions
    let layer_spacing_y = 150.0;
    let node_spacing_x = 200.0;

    let mut nodes = Vec::new();
    for (layer_idx, layer_nodes) in layer_groups.iter().enumerate() {
        let total_width = (layer_nodes.len() as f64 - 1.0) * node_spacing_x;
        let start_x = -total_width / 2.0;

        for (order_idx, key) in layer_nodes.iter().enumerate() {
            if let Some(resource_ref) = node_set.get(key) {
                nodes.push(GraphNode {
                    id: key.clone(),
                    kind: resource_ref.kind.clone(),
                    name: resource_ref.name.clone(),
                    x: start_x + order_idx as f64 * node_spacing_x,
                    y: layer_idx as f64 * layer_spacing_y,
                    layer: layer_idx,
                });
            }
        }
    }

    // Build edges
    let mut edges = Vec::new();
    for rel in relationships {
        edges.push(GraphEdge { source_id: rel.source.key(), target_id: rel.target.key() });
    }

    LayoutState { nodes, edges }
}

/// Left-to-right Sugiyama layout for topology views.
///
/// Same algorithm as `compute_layout()` but with swapped axes:
/// - x = layer_idx * horizontal spacing (left-to-right flow)
/// - y = order_idx * vertical spacing (within each layer)
pub fn compute_layout_lr(relationships: &[ResourceRelationship]) -> LayoutState {
    if relationships.is_empty() {
        return LayoutState::empty();
    }

    let mut node_set: HashMap<String, ResourceRef> = HashMap::new();
    let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
    let mut reverse_adjacency: HashMap<String, Vec<String>> = HashMap::new();

    for rel in relationships {
        let source_key = rel.source.key();
        let target_key = rel.target.key();

        node_set.entry(source_key.clone()).or_insert_with(|| rel.source.clone());
        node_set.entry(target_key.clone()).or_insert_with(|| rel.target.clone());

        adjacency.entry(source_key.clone()).or_default().push(target_key.clone());
        reverse_adjacency.entry(target_key.clone()).or_default().push(source_key.clone());

        adjacency.entry(target_key).or_default();
        reverse_adjacency.entry(source_key).or_default();
    }

    let layers = assign_layers(&node_set, &adjacency, &reverse_adjacency);

    let max_layer = layers.values().copied().max().unwrap_or(0);
    let mut layer_groups: Vec<Vec<String>> = vec![Vec::new(); max_layer + 1];
    for (key, &layer) in &layers {
        layer_groups[layer].push(key.clone());
    }

    order_layers_by_barycenter(&mut layer_groups, &reverse_adjacency);

    // LR layout: x = layer (horizontal), y = order (vertical)
    let layer_spacing_x = 280.0;
    let node_spacing_y = 100.0;

    let mut nodes = Vec::new();
    for (layer_idx, layer_nodes) in layer_groups.iter().enumerate() {
        let total_height = (layer_nodes.len() as f64 - 1.0) * node_spacing_y;
        let start_y = -total_height / 2.0;

        for (order_idx, key) in layer_nodes.iter().enumerate() {
            if let Some(resource_ref) = node_set.get(key) {
                nodes.push(GraphNode {
                    id: key.clone(),
                    kind: resource_ref.kind.clone(),
                    name: resource_ref.name.clone(),
                    x: layer_idx as f64 * layer_spacing_x,
                    y: start_y + order_idx as f64 * node_spacing_y,
                    layer: layer_idx,
                });
            }
        }
    }

    let mut edges = Vec::new();
    for rel in relationships {
        edges.push(GraphEdge { source_id: rel.source.key(), target_id: rel.target.key() });
    }

    LayoutState { nodes, edges }
}

/// Assign layers to nodes using longest-path layering.
/// Root nodes (no incoming edges) get layer 0.
fn assign_layers(
    node_set: &HashMap<String, ResourceRef>,
    adjacency: &HashMap<String, Vec<String>>,
    reverse_adjacency: &HashMap<String, Vec<String>>,
) -> HashMap<String, usize> {
    let mut layers: HashMap<String, usize> = HashMap::new();

    // Find root nodes (no incoming edges)
    let roots: Vec<String> = node_set
        .keys()
        .filter(|key| reverse_adjacency.get(key.as_str()).map(|v| v.is_empty()).unwrap_or(true))
        .cloned()
        .collect();

    // BFS from roots to assign layers
    let mut queue: std::collections::VecDeque<String> = std::collections::VecDeque::new();

    for root in &roots {
        layers.insert(root.clone(), 0);
        queue.push_back(root.clone());
    }

    // If there are no roots (cycle), start from all nodes
    if roots.is_empty() {
        for key in node_set.keys() {
            if !layers.contains_key(key) {
                layers.insert(key.clone(), 0);
                queue.push_back(key.clone());
            }
        }
    }

    // Cap iterations to prevent infinite loops on cyclic graphs.
    // O(N + E) bound: each node visited once, plus one re-enqueue per edge.
    let total_edges: usize = adjacency.values().map(|v| v.len()).sum();
    let max_iterations = (node_set.len() + total_edges).max(1) * 2;
    let mut iterations = 0;

    while let Some(current) = queue.pop_front() {
        iterations += 1;
        if iterations > max_iterations {
            break;
        }
        let current_layer = layers[&current];
        if let Some(children) = adjacency.get(&current) {
            for child in children {
                let new_layer = current_layer + 1;
                let existing = layers.get(child).copied().unwrap_or(0);
                if new_layer > existing || !layers.contains_key(child) {
                    layers.insert(child.clone(), new_layer);
                    queue.push_back(child.clone());
                }
            }
        }
    }

    // Ensure all nodes have a layer assignment
    for key in node_set.keys() {
        layers.entry(key.clone()).or_insert(0);
    }

    layers
}

/// Order nodes within each layer using the barycenter heuristic.
/// For each node, compute the average position of its neighbors in the
/// previous layer, then sort by that average.
fn order_layers_by_barycenter(
    layer_groups: &mut [Vec<String>],
    reverse_adjacency: &HashMap<String, Vec<String>>,
) {
    // Build position index for first layer (stable sort by key for determinism)
    if !layer_groups.is_empty() {
        layer_groups[0].sort();
    }

    // Process layers top-down
    for layer_idx in 1..layer_groups.len() {
        // Build position map for the previous layer
        let prev_positions: HashMap<String, usize> = layer_groups[layer_idx - 1]
            .iter()
            .enumerate()
            .map(|(pos, key)| (key.clone(), pos))
            .collect();

        // Compute barycenter for each node in this layer
        let mut barycenters: Vec<(String, f64)> = layer_groups[layer_idx]
            .iter()
            .map(|key| {
                let parents = reverse_adjacency.get(key).cloned().unwrap_or_default();
                let positions: Vec<f64> = parents
                    .iter()
                    .filter_map(|p| prev_positions.get(p).map(|&pos| pos as f64))
                    .collect();

                let bc = if positions.is_empty() {
                    f64::MAX // No parents in previous layer; place at end
                } else {
                    positions.iter().sum::<f64>() / positions.len() as f64
                };

                (key.clone(), bc)
            })
            .collect();

        // Sort by barycenter, breaking ties with key for determinism
        barycenters.sort_by(|a, b| {
            a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal).then_with(|| a.0.cmp(&b.0))
        });

        layer_groups[layer_idx] = barycenters.into_iter().map(|(key, _)| key).collect();
    }
}

// ---------------------------------------------------------------------------
// T119: Resource map component state
// ---------------------------------------------------------------------------

/// State for the interactive resource map component.
#[derive(Debug, Clone)]
pub struct ResourceMapState {
    pub layout: LayoutState,
    pub zoom_level: f64,
    pub pan_offset: (f64, f64),
    pub selected_node: Option<String>,
}

impl Default for ResourceMapState {
    fn default() -> Self {
        Self {
            layout: LayoutState::empty(),
            zoom_level: 1.0,
            pan_offset: (0.0, 0.0),
            selected_node: None,
        }
    }
}

impl ResourceMapState {
    /// Set the layout from a computed layout state.
    pub fn set_layout(&mut self, layout: LayoutState) {
        self.layout = layout;
    }

    /// Zoom in by a fixed step, clamped to max 3.0.
    pub fn zoom_in(&mut self) {
        self.zoom_level = (self.zoom_level + 0.1).min(3.0);
    }

    /// Zoom out by a fixed step, clamped to min 0.1.
    pub fn zoom_out(&mut self) {
        self.zoom_level = (self.zoom_level - 0.1).max(0.1);
    }

    /// Reset zoom to default 1.0 and pan to origin.
    pub fn reset_zoom(&mut self) {
        self.zoom_level = 1.0;
        self.pan_offset = (0.0, 0.0);
    }

    /// Pan the view by the given delta.
    pub fn pan(&mut self, dx: f64, dy: f64) {
        self.pan_offset.0 += dx;
        self.pan_offset.1 += dy;
    }

    /// Select a node by its ID.
    pub fn select_node(&mut self, node_id: &str) {
        self.selected_node = Some(node_id.to_string());
    }

    /// Clear the current node selection.
    pub fn clear_selection(&mut self) {
        self.selected_node = None;
    }
}

// ---------------------------------------------------------------------------
// T078: GPUI Render implementation
// ---------------------------------------------------------------------------

use crate::theme::Theme;
use gpui::{Context, ElementId, Rgba, SharedString, Window, div, prelude::*, px};

/// Precomputed colors for rendering the resource map.
#[allow(dead_code)]
struct MapColors {
    background: Rgba,
    surface: Rgba,
    border: Rgba,
    accent: Rgba,
    text_primary: Rgba,
    text_secondary: Rgba,
    text_muted: Rgba,
    success: Rgba,
    info: Rgba,
    warning: Rgba,
    error: Rgba,
}

/// View wrapper for ResourceMapState with theme for rendering.
pub struct ResourceMapComponent {
    pub state: ResourceMapState,
    pub theme: Theme,
}

impl ResourceMapComponent {
    pub fn new(state: ResourceMapState, theme: Theme) -> Self {
        Self { state, theme }
    }

    /// Returns the color for a given resource kind.
    /// Pod=success (green), Service=accent (blue), Deployment=info (purple), etc.
    fn kind_color(&self, kind: &str) -> Rgba {
        match kind {
            "Pod" => self.theme.colors.success.to_gpui(),
            "Service" => self.theme.colors.accent.to_gpui(),
            "Deployment" => self.theme.colors.info.to_gpui(),
            "ReplicaSet" => self.theme.colors.warning.to_gpui(),
            "Ingress" => self.theme.colors.error.to_gpui(),
            "Secret" => self.theme.colors.text_muted.to_gpui(),
            "ConfigMap" => Rgba { r: 0.0, g: 0.7, b: 0.65, a: 1.0 }, // teal
            "Node" => Rgba { r: 0.85, g: 0.65, b: 0.13, a: 1.0 },    // amber
            "PersistentVolumeClaim" => Rgba { r: 0.4, g: 0.35, b: 0.8, a: 1.0 }, // indigo
            "PersistentVolume" => Rgba { r: 0.55, g: 0.35, b: 0.8, a: 1.0 }, // violet
            "StatefulSet" => self.theme.colors.info.to_gpui(),
            "DaemonSet" => self.theme.colors.info.to_gpui(),
            "PodDisruptionBudget" => self.theme.colors.warning.to_gpui(),
            _ => self.theme.colors.text_secondary.to_gpui(),
        }
    }

    /// Main canvas container with overflow hidden and background color.
    fn render_canvas(&self, colors: &MapColors) -> gpui::Div {
        if self.state.layout.nodes.is_empty() {
            return self.render_empty_state(colors);
        }

        let mut canvas =
            div().flex().flex_col().size_full().bg(colors.background).overflow_hidden().relative();

        // Render all edges first (so they appear behind nodes)
        for edge in &self.state.layout.edges {
            canvas = canvas.child(self.render_edge(edge, colors));
        }

        // Render all nodes
        for node in &self.state.layout.nodes {
            let is_selected = self.state.selected_node.as_deref() == Some(&node.id);
            canvas = canvas.child(self.render_node(node, is_selected, colors));
        }

        // Overlay controls
        canvas = canvas.child(self.render_controls(colors));

        canvas
    }

    /// Render a single node at its position (adjusted by zoom and pan).
    fn render_node(
        &self,
        node: &GraphNode,
        selected: bool,
        colors: &MapColors,
    ) -> gpui::Stateful<gpui::Div> {
        let kind_color = self.kind_color(&node.kind);
        let node_id = format!("node-{}", node.id);

        // Apply zoom and pan transformations
        let display_x = (node.x * self.state.zoom_level) + self.state.pan_offset.0;
        let display_y = (node.y * self.state.zoom_level) + self.state.pan_offset.1;

        // Center the canvas (add offset to position nodes in the center)
        let canvas_center_x = 400.0; // Approximate canvas center
        let canvas_center_y = 300.0;

        let final_x = canvas_center_x + display_x;
        let final_y = canvas_center_y + display_y;

        let label = SharedString::from(format!("{}: {}", node.kind, node.name));

        div()
            .id(ElementId::Name(SharedString::from(node_id)))
            .absolute()
            .left(px(final_x as f32))
            .top(px(final_y as f32))
            .px_3()
            .py_2()
            .rounded(px(6.0))
            .bg(colors.surface)
            .border_2()
            .border_color(if selected { colors.accent } else { kind_color })
            .cursor_pointer()
            .child(
                div()
                    .text_xs()
                    .text_color(if selected { colors.accent } else { colors.text_primary })
                    .child(label),
            )
    }

    /// Render an edge between source and target nodes.
    /// Uses a thin horizontal/vertical div positioned between nodes.
    fn render_edge(&self, edge: &GraphEdge, colors: &MapColors) -> gpui::Div {
        // Find source and target nodes
        let source = self.state.layout.node_by_id(&edge.source_id);
        let target = self.state.layout.node_by_id(&edge.target_id);

        if source.is_none() || target.is_none() {
            return div(); // Skip rendering if nodes not found
        }

        let source = source.unwrap();
        let target = target.unwrap();

        // Apply zoom and pan
        let src_x = (source.x * self.state.zoom_level) + self.state.pan_offset.0;
        let src_y = (source.y * self.state.zoom_level) + self.state.pan_offset.1;
        let tgt_x = (target.x * self.state.zoom_level) + self.state.pan_offset.0;
        let tgt_y = (target.y * self.state.zoom_level) + self.state.pan_offset.1;

        let canvas_center_x = 400.0;
        let canvas_center_y = 300.0;

        let src_final_x = canvas_center_x + src_x;
        let src_final_y = canvas_center_y + src_y;
        let tgt_final_x = canvas_center_x + tgt_x;
        let tgt_final_y = canvas_center_y + tgt_y;

        // Calculate line dimensions
        let dx = tgt_final_x - src_final_x;
        let dy = tgt_final_y - src_final_y;
        let _length = (dx * dx + dy * dy).sqrt();

        // For simplicity, render a vertical line between the nodes
        // (More sophisticated SVG-like rendering would require custom GPUI primitives)
        let mid_x = (src_final_x + tgt_final_x) / 2.0;
        let _mid_y = (src_final_y + tgt_final_y) / 2.0;

        div()
            .absolute()
            .left(px(mid_x as f32))
            .top(px(src_final_y as f32))
            .w(px(2.0))
            .h(px((dy.abs()) as f32))
            .bg(colors.border)
    }

    /// Zoom in/out/reset buttons overlay.
    fn render_controls(&self, colors: &MapColors) -> gpui::Div {
        div()
            .absolute()
            .bottom(px(16.0))
            .right(px(16.0))
            .flex()
            .flex_row()
            .gap(px(8.0))
            .child(self.render_control_btn("zoom-in", "+", colors))
            .child(self.render_control_btn("zoom-out", "-", colors))
            .child(self.render_control_btn("zoom-reset", "Reset", colors))
    }

    /// Single control button.
    fn render_control_btn(
        &self,
        id: &str,
        label: &str,
        colors: &MapColors,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(ElementId::Name(SharedString::from(id.to_string())))
            .px_3()
            .py_2()
            .rounded(px(4.0))
            .bg(colors.surface)
            .border_1()
            .border_color(colors.border)
            .cursor_pointer()
            .text_sm()
            .text_color(colors.text_primary)
            .child(SharedString::from(label.to_string()))
    }

    /// Empty state: "No resources to display"
    fn render_empty_state(&self, colors: &MapColors) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .size_full()
            .items_center()
            .justify_center()
            .bg(colors.background)
            .child(div().text_sm().text_color(colors.text_muted).child("No resources to display"))
    }
}

impl Render for ResourceMapComponent {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let colors = MapColors {
            background: self.theme.colors.background.to_gpui(),
            surface: self.theme.colors.surface.to_gpui(),
            border: self.theme.colors.border.to_gpui(),
            accent: self.theme.colors.accent.to_gpui(),
            text_primary: self.theme.colors.text_primary.to_gpui(),
            text_secondary: self.theme.colors.text_secondary.to_gpui(),
            text_muted: self.theme.colors.text_muted.to_gpui(),
            success: self.theme.colors.success.to_gpui(),
            info: self.theme.colors.info.to_gpui(),
            warning: self.theme.colors.warning.to_gpui(),
            error: self.theme.colors.error.to_gpui(),
        };

        self.render_canvas(&colors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use baeus_core::resource::RelationshipKind;

    // ===================================================================
    // T116: Graph layout algorithm tests
    // ===================================================================

    // --- Sugiyama-style layered graph layout tests ---

    #[test]
    fn test_compute_layout_empty_relationships() {
        let layout = compute_layout(&[]);
        assert!(layout.nodes.is_empty());
        assert!(layout.edges.is_empty());
    }

    #[test]
    fn test_compute_layout_single_edge() {
        let rels = vec![ResourceRelationship::new(
            ResourceRef::new("Deployment", "deploy-a", Some("default".to_string())),
            ResourceRef::new("ReplicaSet", "rs-a", Some("default".to_string())),
            RelationshipKind::OwnerReference,
        )];

        let layout = compute_layout(&rels);

        assert_eq!(layout.nodes.len(), 2);
        assert_eq!(layout.edges.len(), 1);

        let deploy_node = layout.node_by_id("Deployment/default/deploy-a").unwrap();
        let rs_node = layout.node_by_id("ReplicaSet/default/rs-a").unwrap();

        // Deployment should be in layer 0, ReplicaSet in layer 1
        assert_eq!(deploy_node.layer, 0);
        assert_eq!(rs_node.layer, 1);
    }

    #[test]
    fn test_compute_layout_chain_three_layers() {
        // Deployment -> ReplicaSet -> Pod
        let rels = vec![
            ResourceRelationship::new(
                ResourceRef::new("Deployment", "deploy", Some("ns".to_string())),
                ResourceRef::new("ReplicaSet", "rs", Some("ns".to_string())),
                RelationshipKind::OwnerReference,
            ),
            ResourceRelationship::new(
                ResourceRef::new("ReplicaSet", "rs", Some("ns".to_string())),
                ResourceRef::new("Pod", "pod", Some("ns".to_string())),
                RelationshipKind::OwnerReference,
            ),
        ];

        let layout = compute_layout(&rels);

        assert_eq!(layout.nodes.len(), 3);
        assert_eq!(layout.edges.len(), 2);

        let deploy = layout.node_by_id("Deployment/ns/deploy").unwrap();
        let rs = layout.node_by_id("ReplicaSet/ns/rs").unwrap();
        let pod = layout.node_by_id("Pod/ns/pod").unwrap();

        assert_eq!(deploy.layer, 0);
        assert_eq!(rs.layer, 1);
        assert_eq!(pod.layer, 2);

        // Layers should have increasing y values
        assert!(deploy.y < rs.y);
        assert!(rs.y < pod.y);
    }

    #[test]
    fn test_compute_layout_node_positioning_within_layer() {
        // Service selects two Pods -> both pods should be in the same layer
        let rels = vec![
            ResourceRelationship::new(
                ResourceRef::new("Service", "svc", Some("ns".to_string())),
                ResourceRef::new("Pod", "pod-a", Some("ns".to_string())),
                RelationshipKind::ServiceSelector,
            ),
            ResourceRelationship::new(
                ResourceRef::new("Service", "svc", Some("ns".to_string())),
                ResourceRef::new("Pod", "pod-b", Some("ns".to_string())),
                RelationshipKind::ServiceSelector,
            ),
        ];

        let layout = compute_layout(&rels);

        assert_eq!(layout.nodes.len(), 3);

        let svc = layout.node_by_id("Service/ns/svc").unwrap();
        let pod_a = layout.node_by_id("Pod/ns/pod-a").unwrap();
        let pod_b = layout.node_by_id("Pod/ns/pod-b").unwrap();

        // Service in layer 0, both pods in layer 1
        assert_eq!(svc.layer, 0);
        assert_eq!(pod_a.layer, 1);
        assert_eq!(pod_b.layer, 1);

        // Pods in same layer should have same y but different x
        assert!((pod_a.y - pod_b.y).abs() < f64::EPSILON);
        assert!((pod_a.x - pod_b.x).abs() > f64::EPSILON);
    }

    #[test]
    fn test_compute_layout_edge_routing() {
        let rels = vec![ResourceRelationship::new(
            ResourceRef::new("Ingress", "ing", Some("ns".to_string())),
            ResourceRef::new("Service", "svc", Some("ns".to_string())),
            RelationshipKind::IngressBackend,
        )];

        let layout = compute_layout(&rels);

        assert_eq!(layout.edges.len(), 1);
        assert_eq!(layout.edges[0].source_id, "Ingress/ns/ing");
        assert_eq!(layout.edges[0].target_id, "Service/ns/svc");

        // Source should be in a higher (lower y) layer than target
        let source_node = layout.node_by_id(&layout.edges[0].source_id).unwrap();
        let target_node = layout.node_by_id(&layout.edges[0].target_id).unwrap();
        assert!(source_node.y < target_node.y);
    }

    #[test]
    fn test_compute_layout_full_ingress_to_pods() {
        // Ingress -> Service -> Pod1, Pod2
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

        let ing = layout.node_by_id("Ingress/default/my-ing").unwrap();
        let svc = layout.node_by_id("Service/default/my-svc").unwrap();
        let pod1 = layout.node_by_id("Pod/default/pod-1").unwrap();
        let pod2 = layout.node_by_id("Pod/default/pod-2").unwrap();

        // Three distinct layers
        assert_eq!(ing.layer, 0);
        assert_eq!(svc.layer, 1);
        assert_eq!(pod1.layer, 2);
        assert_eq!(pod2.layer, 2);
    }

    #[test]
    fn test_compute_layout_node_names_and_kinds() {
        let rels = vec![ResourceRelationship::new(
            ResourceRef::new("Service", "my-svc", Some("default".to_string())),
            ResourceRef::new("Pod", "my-pod", Some("default".to_string())),
            RelationshipKind::ServiceSelector,
        )];

        let layout = compute_layout(&rels);

        let svc = layout.node_by_id("Service/default/my-svc").unwrap();
        assert_eq!(svc.kind, "Service");
        assert_eq!(svc.name, "my-svc");

        let pod = layout.node_by_id("Pod/default/my-pod").unwrap();
        assert_eq!(pod.kind, "Pod");
        assert_eq!(pod.name, "my-pod");
    }

    #[test]
    fn test_layout_edges_from() {
        let rels = vec![
            ResourceRelationship::new(
                ResourceRef::new("Service", "svc", Some("ns".to_string())),
                ResourceRef::new("Pod", "pod-a", Some("ns".to_string())),
                RelationshipKind::ServiceSelector,
            ),
            ResourceRelationship::new(
                ResourceRef::new("Service", "svc", Some("ns".to_string())),
                ResourceRef::new("Pod", "pod-b", Some("ns".to_string())),
                RelationshipKind::ServiceSelector,
            ),
        ];

        let layout = compute_layout(&rels);

        let outgoing = layout.edges_from("Service/ns/svc");
        assert_eq!(outgoing.len(), 2);

        let incoming = layout.edges_to("Pod/ns/pod-a");
        assert_eq!(incoming.len(), 1);
    }

    #[test]
    fn test_layer_spacing() {
        let rels = vec![
            ResourceRelationship::new(
                ResourceRef::new("Deployment", "d", Some("ns".to_string())),
                ResourceRef::new("ReplicaSet", "rs", Some("ns".to_string())),
                RelationshipKind::OwnerReference,
            ),
            ResourceRelationship::new(
                ResourceRef::new("ReplicaSet", "rs", Some("ns".to_string())),
                ResourceRef::new("Pod", "p", Some("ns".to_string())),
                RelationshipKind::OwnerReference,
            ),
        ];

        let layout = compute_layout(&rels);

        let d = layout.node_by_id("Deployment/ns/d").unwrap();
        let rs = layout.node_by_id("ReplicaSet/ns/rs").unwrap();
        let p = layout.node_by_id("Pod/ns/p").unwrap();

        // Layers should be evenly spaced (150.0 apart)
        assert!((rs.y - d.y - 150.0).abs() < f64::EPSILON);
        assert!((p.y - rs.y - 150.0).abs() < f64::EPSILON);
    }

    // ===================================================================
    // T119: Resource map component state tests
    // ===================================================================

    #[test]
    fn test_resource_map_state_default() {
        let state = ResourceMapState::default();
        assert!(state.layout.nodes.is_empty());
        assert!(state.layout.edges.is_empty());
        assert!((state.zoom_level - 1.0).abs() < f64::EPSILON);
        assert!((state.pan_offset.0).abs() < f64::EPSILON);
        assert!((state.pan_offset.1).abs() < f64::EPSILON);
        assert!(state.selected_node.is_none());
    }

    #[test]
    fn test_set_layout() {
        let mut state = ResourceMapState::default();
        let layout = LayoutState {
            nodes: vec![GraphNode {
                id: "Pod/ns/p".to_string(),
                kind: "Pod".to_string(),
                name: "p".to_string(),
                x: 0.0,
                y: 0.0,
                layer: 0,
            }],
            edges: Vec::new(),
        };

        state.set_layout(layout);
        assert_eq!(state.layout.nodes.len(), 1);
    }

    #[test]
    fn test_zoom_in() {
        let mut state = ResourceMapState::default();
        state.zoom_in();
        assert!((state.zoom_level - 1.1).abs() < f64::EPSILON);
    }

    #[test]
    fn test_zoom_in_clamped() {
        let mut state = ResourceMapState::default();
        state.zoom_level = 2.95;
        state.zoom_in();
        assert!((state.zoom_level - 3.0).abs() < f64::EPSILON);

        // Should not exceed 3.0
        state.zoom_in();
        assert!((state.zoom_level - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_zoom_out() {
        let mut state = ResourceMapState::default();
        state.zoom_out();
        assert!((state.zoom_level - 0.9).abs() < f64::EPSILON);
    }

    #[test]
    fn test_zoom_out_clamped() {
        let mut state = ResourceMapState::default();
        state.zoom_level = 0.15;
        state.zoom_out();
        // 0.15 - 0.1 = 0.05, clamped to 0.1
        assert!((state.zoom_level - 0.1).abs() < 0.001);
    }

    #[test]
    fn test_reset_zoom() {
        let mut state = ResourceMapState::default();
        state.zoom_level = 2.5;
        state.pan_offset = (100.0, -50.0);

        state.reset_zoom();
        assert!((state.zoom_level - 1.0).abs() < f64::EPSILON);
        assert!((state.pan_offset.0).abs() < f64::EPSILON);
        assert!((state.pan_offset.1).abs() < f64::EPSILON);
    }

    #[test]
    fn test_pan() {
        let mut state = ResourceMapState::default();
        state.pan(10.0, -20.0);
        assert!((state.pan_offset.0 - 10.0).abs() < f64::EPSILON);
        assert!((state.pan_offset.1 - (-20.0)).abs() < f64::EPSILON);

        state.pan(5.0, 5.0);
        assert!((state.pan_offset.0 - 15.0).abs() < f64::EPSILON);
        assert!((state.pan_offset.1 - (-15.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_select_node() {
        let mut state = ResourceMapState::default();
        state.select_node("Pod/ns/my-pod");
        assert_eq!(state.selected_node.as_deref(), Some("Pod/ns/my-pod"));
    }

    #[test]
    fn test_clear_selection() {
        let mut state = ResourceMapState::default();
        state.select_node("Pod/ns/my-pod");
        state.clear_selection();
        assert!(state.selected_node.is_none());
    }

    #[test]
    fn test_select_node_replaces_previous() {
        let mut state = ResourceMapState::default();
        state.select_node("Pod/ns/pod-a");
        state.select_node("Pod/ns/pod-b");
        assert_eq!(state.selected_node.as_deref(), Some("Pod/ns/pod-b"));
    }
}
