use std::collections::HashMap;

use baeus_core::KubeClient;
use baeus_core::resource::{self, RelationshipKind, ResourceRef, ResourceRelationship};
use gpui::prelude::FluentBuilder as _;
use gpui::*;
use gpui_component::Sizable as _;

use crate::components::json_extract::json_to_table_row;
use crate::components::resource_map::{GraphNode, LayoutState, compute_layout_lr};
use crate::components::resource_table::{ResourceTableState, columns_for_kind};
use crate::icons::ResourceIcon;
use crate::layout::app_shell::{AppShell, ResourceDetailKey};
use crate::theme::Theme;

// ---------------------------------------------------------------------------
// TopologyState — stored per resource detail key on AppShell
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TopologyState {
    pub layout: LayoutState,
    pub focus_node_key: String,
    pub zoom_level: f64,
    pub pan_offset: (f64, f64),
    pub selected_node: Option<String>,
    pub loading: bool,
    pub error: Option<String>,
}

impl TopologyState {
    pub fn new_loading() -> Self {
        Self {
            layout: LayoutState::empty(),
            focus_node_key: String::new(),
            zoom_level: 1.0,
            pan_offset: (0.0, 0.0),
            selected_node: None,
            loading: true,
            error: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Color helper — maps resource kind to a color for the left stripe / edges
// ---------------------------------------------------------------------------

fn kind_color(kind: &str, theme: &Theme) -> Rgba {
    match kind {
        "Pod" => theme.colors.success.to_gpui(),
        "Service" => theme.colors.accent.to_gpui(),
        "Deployment" => theme.colors.info.to_gpui(),
        "ReplicaSet" => theme.colors.warning.to_gpui(),
        "Ingress" => theme.colors.error.to_gpui(),
        "Secret" => theme.colors.text_muted.to_gpui(),
        "ConfigMap" => Rgba { r: 0.0, g: 0.7, b: 0.65, a: 1.0 },
        "Node" => Rgba { r: 0.85, g: 0.65, b: 0.13, a: 1.0 },
        "PersistentVolumeClaim" => Rgba { r: 0.4, g: 0.35, b: 0.8, a: 1.0 },
        "PersistentVolume" => Rgba { r: 0.55, g: 0.35, b: 0.8, a: 1.0 },
        "StatefulSet" => theme.colors.info.to_gpui(),
        "DaemonSet" => theme.colors.info.to_gpui(),
        "PodDisruptionBudget" => theme.colors.warning.to_gpui(),
        _ => theme.colors.text_secondary.to_gpui(),
    }
}

/// Short display label for a kind (abbreviate long names).
fn kind_label(kind: &str) -> &str {
    match kind {
        "PersistentVolumeClaim" => "PVC",
        "PersistentVolume" => "PV",
        "PodDisruptionBudget" => "PDB",
        "ReplicaSet" => "RS",
        "StatefulSet" => "STS",
        "DaemonSet" => "DS",
        "ConfigMap" => "CM",
        _ => kind,
    }
}

// ---------------------------------------------------------------------------
// Card node dimensions
// ---------------------------------------------------------------------------

const NODE_W: f64 = 220.0;
const NODE_H: f64 = 48.0;
const INDICATOR_SIZE: f64 = 12.0;

// ---------------------------------------------------------------------------
// impl AppShell — topology rendering & data fetching
// ---------------------------------------------------------------------------

impl AppShell {
    /// Render the topology tab body.
    pub(crate) fn render_topology_tab(
        &mut self,
        cx: &mut Context<Self>,
        key: &ResourceDetailKey,
        text: Rgba,
        text_secondary: Rgba,
        bg: Rgba,
    ) -> Div {
        let Some(state) = self.topology_data.get(key) else {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .bg(bg)
                .text_color(text_secondary)
                .text_sm()
                .child("Loading topology...");
        };

        // Loading state
        if state.loading {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .bg(bg)
                .text_color(text_secondary)
                .text_sm()
                .child("Loading topology...");
        }

        // Error state
        if let Some(err) = &state.error {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .bg(bg)
                .text_color(self.theme.colors.error.to_gpui())
                .text_sm()
                .child(format!("Topology error: {err}"));
        }

        // Empty state
        if state.layout.nodes.is_empty() {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .bg(bg)
                .text_color(text_secondary)
                .text_sm()
                .child("No related resources found.");
        }

        // Build positioned card nodes and edge canvas
        let zoom = state.zoom_level;
        let pan = state.pan_offset;
        let focus_key = state.focus_node_key.clone();
        let accent = self.theme.colors.accent.to_gpui();
        let border_color = self.theme.colors.border.to_gpui();
        let surface = self.theme.colors.surface.to_gpui();
        let theme = self.theme.clone();

        // Offset all graph coordinates so the minimum node is at (PADDING, PADDING).
        // Both edges and node divs use this same coordinate system (top-left origin).
        let (min_x, _max_x, min_y, _max_y) = graph_bounds(&state.layout.nodes);
        let pad = 40.0;
        let origin_x = min_x - pad;
        let origin_y = min_y - pad;

        let key_for_zoom_in = key.clone();
        let key_for_zoom_out = key.clone();
        let key_for_reset = key.clone();
        let key_for_scroll = key.clone();
        let key_for_drag = key.clone();

        // Container needs .id() for scroll_wheel and mouse events (returns Stateful<Div>).
        // We return Div from the wrapping outer div.
        let mut inner = div().id("topology-container").flex_1().relative().overflow_hidden().bg(bg);

        // Edge canvas layer — paint bezier curves
        let edges = state.layout.edges.clone();
        let nodes_for_edges = state.layout.nodes.clone();
        let theme_for_edges = theme.clone();
        let edge_canvas = canvas(
            move |_bounds, _window, _cx| {},
            move |bounds, (), window, _cx| {
                let ox = bounds.origin.x;
                let oy = bounds.origin.y;
                let node_map: HashMap<&str, &GraphNode> =
                    nodes_for_edges.iter().map(|n| (n.id.as_str(), n)).collect();

                for edge in &edges {
                    let Some(src) = node_map.get(edge.source_id.as_str()) else {
                        continue;
                    };
                    let Some(tgt) = node_map.get(edge.target_id.as_str()) else {
                        continue;
                    };

                    // Source right-center → Target left-center (same coords as node divs)
                    let sx = ox + px(((src.x - origin_x + NODE_W) * zoom + pan.0) as f32);
                    let sy = oy + px(((src.y - origin_y + NODE_H / 2.0) * zoom + pan.1) as f32);
                    let tx = ox + px(((tgt.x - origin_x) * zoom + pan.0) as f32);
                    let ty = oy + px(((tgt.y - origin_y + NODE_H / 2.0) * zoom + pan.1) as f32);

                    let edge_color = kind_color(&src.kind, &theme_for_edges);
                    let edge_rgba: Rgba =
                        Rgba { r: edge_color.r, g: edge_color.g, b: edge_color.b, a: 0.6 };

                    // Cubic bezier: horizontal S-curve
                    let mid_x = (sx + tx) * 0.5;
                    let mut builder = PathBuilder::stroke(px(2.0));
                    builder.move_to(point(sx, sy));
                    builder.cubic_bezier_to(point(tx, ty), point(mid_x, sy), point(mid_x, ty));
                    if let Ok(path) = builder.build() {
                        window.paint_path(path, edge_rgba);
                    }

                    // Arrow head — small filled triangle
                    paint_arrow_head(window, tx, ty, edge_rgba);
                }
            },
        )
        .w_full()
        .h_full()
        .absolute()
        .top_0()
        .left_0();

        inner = inner.child(edge_canvas);

        // Node cards — we position them using a wrapping div with percentage-based centering
        for node in &state.layout.nodes {
            let is_focus = node.id == focus_key;
            let is_selected = state.selected_node.as_deref() == Some(&node.id);
            let kc = kind_color(&node.kind, &theme);

            let node_id_str = node.id.clone();
            let cluster_context = key.cluster_context.clone();
            let klabel = kind_label(&node.kind).to_string();
            let node_name_display = node.name.clone();
            // We track display offsets as f32 for px()
            let dx = ((node.x - origin_x) * zoom + pan.0) as f32;
            let dy = ((node.y - origin_y) * zoom + pan.1) as f32;

            let card = self.render_topology_card(
                cx,
                &node.id,
                &node_name_display,
                &node.kind,
                &klabel,
                kc,
                is_focus,
                is_selected,
                text,
                text_secondary,
                surface,
                border_color,
                accent,
                zoom,
                cluster_context,
                node_id_str,
            );

            // Wrap in positioning div
            // Cards use absolute positioning. The edge canvas knows the actual bounds
            // and centers via bounds.size / 2. For cards, we estimate a large container
            // center offset (the container is flex_1 so fills available space).
            // We'll use the same centering trick: add CONTAINER_CENTER_ESTIMATE.
            inner = inner.child(div().absolute().left(px(dx)).top(px(dy)).child(card));
        }

        // Zoom controls overlay — bottom right
        let controls = self.render_topology_controls(
            cx,
            key_for_zoom_in,
            key_for_zoom_out,
            key_for_reset,
            text,
            surface,
            border_color,
        );
        inner = inner.child(controls);

        // Scroll-wheel zoom
        inner = inner.on_scroll_wheel(cx.listener(
            move |this, event: &ScrollWheelEvent, _window, cx| {
                if let Some(state) = this.topology_data.get_mut(&key_for_scroll) {
                    let delta = match event.delta {
                        ScrollDelta::Pixels(p) => {
                            // p.y is Pixels; convert to f64 via arithmetic
                            let y_px: Pixels = p.y;
                            // Pixels implements Into<f32> via .0, but .0 is private
                            // Use Pixels / px(1.0) to get the raw f32
                            let raw = y_px / px(1.0);
                            raw as f64 / 200.0
                        }
                        ScrollDelta::Lines(l) => l.y as f64 * 0.1,
                    };
                    state.zoom_level = (state.zoom_level + delta).clamp(0.2, 3.0);
                    cx.notify();
                }
            },
        ));

        // Left-click drag panning — start drag, record position
        inner = inner.on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
                let x: f32 = event.position.x.into();
                let y: f32 = event.position.y.into();
                this.is_dragging_topology = true;
                this.topology_drag_last = (x, y);
                this.topology_drag_key = Some(key_for_drag.clone());
                if let Some(state) = this.topology_data.get_mut(&key_for_drag) {
                    state.selected_node = None;
                }
                cx.notify();
            }),
        );

        // Return as Div by wrapping the Stateful<Div>
        div().flex_1().flex().flex_col().child(inner)
    }

    /// Render a single topology card node (card-style).
    #[allow(clippy::too_many_arguments)]
    fn render_topology_card(
        &self,
        cx: &mut Context<Self>,
        node_id: &str,
        name: &str,
        kind: &str,
        klabel: &str,
        kc: Rgba,
        is_focus: bool,
        is_selected: bool,
        text: Rgba,
        _text_secondary: Rgba,
        surface: Rgba,
        border_color: Rgba,
        accent: Rgba,
        zoom: f64,
        cluster_context: String,
        node_id_str: String,
    ) -> Stateful<Div> {
        let card_label = SharedString::from(format!("{klabel}: {name}"));

        div()
            .id(ElementId::Name(SharedString::from(format!("topo-node-{node_id}"))))
            .w(px(NODE_W as f32 * zoom as f32))
            .h(px(NODE_H as f32 * zoom as f32))
            .rounded(px(8.0))
            .bg(surface)
            .border_1()
            .border_color(if is_focus {
                accent
            } else if is_selected {
                kc
            } else {
                border_color
            })
            .when(is_focus, |d| d.border_2().border_color(accent))
            .when(is_focus, |d| d.bg(Rgba { r: accent.r, g: accent.g, b: accent.b, a: 0.08 }))
            .overflow_hidden()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_3()
            .cursor_pointer()
            .child(
                // Resource icon
                div()
                    .w(px(INDICATOR_SIZE as f32))
                    .h(px(INDICATOR_SIZE as f32))
                    .flex()
                    .items_center()
                    .justify_center()
                    .flex_shrink_0()
                    .child(
                        gpui_component::Icon::new(ResourceIcon::from_kind(kind))
                            .size(px(INDICATOR_SIZE as f32))
                            .text_color(kc),
                    ),
            )
            .child(
                // Single-line "Kind: name" label
                div()
                    .flex_1()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(text)
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .child(card_label),
            )
            .on_click(cx.listener(move |this, event: &ClickEvent, _window, cx| {
                // Double-click to navigate to the resource
                let click_count = match event {
                    ClickEvent::Mouse(m) => m.down.click_count,
                    ClickEvent::Keyboard(_) => 1,
                };
                if click_count < 2 {
                    return;
                }
                let parts: Vec<&str> = node_id_str.splitn(3, '/').collect();
                let (nk, nns, nn) = match parts.len() {
                    3 => (parts[0], Some(parts[1].to_string()), parts[2]),
                    2 => (parts[0], None, parts[1]),
                    _ => return,
                };
                let target = crate::layout::NavigationTarget::ResourceDetail {
                    cluster_context: cluster_context.clone(),
                    kind: nk.to_string(),
                    name: nn.to_string(),
                    namespace: nns,
                };
                this.workspace.open_tab(target.clone());
                this.push_navigation_history(target);
                cx.notify();
            }))
    }

    /// Render zoom control buttons for the topology view.
    #[allow(clippy::too_many_arguments)]
    fn render_topology_controls(
        &self,
        cx: &mut Context<Self>,
        key_zoom_in: ResourceDetailKey,
        key_zoom_out: ResourceDetailKey,
        key_reset: ResourceDetailKey,
        text: Rgba,
        surface: Rgba,
        border_color: Rgba,
    ) -> Div {
        let zoom_in_btn = div()
            .id("topo-zoom-in")
            .px_2()
            .py_1()
            .rounded_md()
            .bg(surface)
            .border_1()
            .border_color(border_color)
            .cursor_pointer()
            .text_sm()
            .text_color(text)
            .child("+")
            .on_click(cx.listener(move |this, _e, _w, cx| {
                if let Some(state) = this.topology_data.get_mut(&key_zoom_in) {
                    state.zoom_level = (state.zoom_level + 0.15).min(3.0);
                    cx.notify();
                }
            }));

        let zoom_out_btn = div()
            .id("topo-zoom-out")
            .px_2()
            .py_1()
            .rounded_md()
            .bg(surface)
            .border_1()
            .border_color(border_color)
            .cursor_pointer()
            .text_sm()
            .text_color(text)
            .child("\u{2212}")
            .on_click(cx.listener(move |this, _e, _w, cx| {
                if let Some(state) = this.topology_data.get_mut(&key_zoom_out) {
                    state.zoom_level = (state.zoom_level - 0.15).max(0.2);
                    cx.notify();
                }
            }));

        let zoom_reset_btn = div()
            .id("topo-zoom-reset")
            .px_2()
            .py_1()
            .rounded_md()
            .bg(surface)
            .border_1()
            .border_color(border_color)
            .cursor_pointer()
            .text_sm()
            .text_color(text)
            .child("Reset")
            .on_click(cx.listener(move |this, _e, _w, cx| {
                if let Some(state) = this.topology_data.get_mut(&key_reset) {
                    state.zoom_level = 1.0;
                    state.pan_offset = (0.0, 0.0);
                    cx.notify();
                }
            }));

        div()
            .absolute()
            .bottom_3()
            .right_3()
            .flex()
            .flex_row()
            .gap_1()
            .child(zoom_out_btn)
            .child(zoom_reset_btn)
            .child(zoom_in_btn)
    }

    /// Kick off async topology data loading for a resource.
    pub(crate) fn start_topology_loading(
        &mut self,
        key: &ResourceDetailKey,
        cx: &mut Context<Self>,
    ) {
        // Evict oldest topology entries if cache exceeds limit.
        const MAX_TOPOLOGY_CACHE: usize = 50;
        if self.topology_data.len() >= MAX_TOPOLOGY_CACHE {
            // Remove entries not matching the current key.
            let keys_to_remove: Vec<_> = self
                .topology_data
                .keys()
                .filter(|k| *k != key)
                .take(self.topology_data.len() - MAX_TOPOLOGY_CACHE + 1)
                .cloned()
                .collect();
            for k in keys_to_remove {
                self.topology_data.remove(&k);
            }
        }

        self.topology_data.insert(key.clone(), TopologyState::new_loading());

        let cluster_context = key.cluster_context.clone();
        let focus_kind = key.kind.clone();
        let focus_name = key.name.clone();
        let focus_namespace = key.namespace.clone();
        let key_for_update = key.clone();

        let Some(client) = self.active_clients.get(&cluster_context).cloned() else {
            if let Some(state) = self.topology_data.get_mut(key) {
                state.loading = false;
                state.error = Some("No active client for this cluster".to_string());
            }
            cx.notify();
            return;
        };

        let ns = focus_namespace.clone();
        let tokio_handle = cx.global::<crate::layout::app_shell::GpuiTokioHandle>().0.clone();

        // Clone focus_kind for the update closure (the original moves into the spawn).
        let focus_kind_for_update = focus_kind.clone();
        let focus_name_for_spawn = focus_name.clone();

        cx.spawn(async move |this: WeakEntity<AppShell>, cx: &mut AsyncApp| {
            let result = tokio_handle
                .spawn(async move {
                    fetch_focused_topology(
                        &client,
                        &focus_kind,
                        &focus_name_for_spawn,
                        ns.as_deref(),
                    )
                    .await
                })
                .await;

            this.update(cx, |this, cx| {
                match result {
                    Ok(Ok(resources)) => {
                        let focus_ref =
                            ResourceRef::new(&focus_kind_for_update, &focus_name, focus_namespace);
                        let all_rels = resource::build_relationship_graph(&resources);
                        let (subgraph_rels, focus_key) =
                            resource::build_topology_subgraph(&focus_ref, &all_rels, 3);
                        let layout = compute_layout_lr(&subgraph_rels);

                        // Center graph in viewport
                        let pan = if !layout.nodes.is_empty() {
                            let (min_x, max_x, min_y, max_y) = graph_bounds(&layout.nodes);
                            let graph_cx = (min_x + max_x) / 2.0;
                            let graph_cy = (min_y + max_y) / 2.0;
                            (400.0 - graph_cx, 250.0 - graph_cy)
                        } else {
                            (0.0, 0.0)
                        };

                        this.topology_data.insert(
                            key_for_update,
                            TopologyState {
                                layout,
                                focus_node_key: focus_key,
                                zoom_level: 1.0,
                                pan_offset: pan,
                                selected_node: None,
                                loading: false,
                                error: None,
                            },
                        );
                    }
                    Ok(Err(e)) => {
                        tracing::warn!("Topology loading failed: {e:#}");
                        if let Some(state) = this.topology_data.get_mut(&key_for_update) {
                            state.loading = false;
                            state.error = Some(format!("{e}"));
                        }
                    }
                    Err(e) => {
                        if let Some(state) = this.topology_data.get_mut(&key_for_update) {
                            state.loading = false;
                            state.error = Some(format!("Task join error: {e}"));
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

// ---------------------------------------------------------------------------
// Arrow head helper
// ---------------------------------------------------------------------------

fn paint_arrow_head(window: &mut Window, tip_x: Pixels, tip_y: Pixels, color: Rgba) {
    let size = px(6.0);
    let half = px(3.0);
    let mut builder = PathBuilder::fill();
    builder.move_to(point(tip_x, tip_y));
    builder.line_to(point(tip_x - size, tip_y - half));
    builder.line_to(point(tip_x - size, tip_y + half));
    builder.close();
    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }
}

// ---------------------------------------------------------------------------
// Graph bounds helper
// ---------------------------------------------------------------------------

fn graph_bounds(nodes: &[GraphNode]) -> (f64, f64, f64, f64) {
    if nodes.is_empty() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let mut min_x = f64::MAX;
    let mut max_x = f64::MIN;
    let mut min_y = f64::MAX;
    let mut max_y = f64::MIN;
    for n in nodes {
        if n.x < min_x {
            min_x = n.x;
        }
        if n.x + NODE_W > max_x {
            max_x = n.x + NODE_W;
        }
        if n.y < min_y {
            min_y = n.y;
        }
        if n.y + NODE_H > max_y {
            max_y = n.y + NODE_H;
        }
    }
    (min_x, max_x, min_y, max_y)
}

// ---------------------------------------------------------------------------
// Async data fetching — focused topology (card-style relationship walking)
// ---------------------------------------------------------------------------

/// Map parent kind → expected child kinds for downward relationship walking.
fn expected_child_kinds(parent_kind: &str) -> &'static [&'static str] {
    match parent_kind {
        "Deployment" => &["ReplicaSet"],
        "ReplicaSet" => &["Pod"],
        "StatefulSet" => &["Pod"],
        "DaemonSet" => &["Pod"],
        "Job" => &["Pod"],
        "CronJob" => &["Job"],
        _ => &[],
    }
}

fn is_cluster_scoped(kind: &str) -> bool {
    matches!(kind, "Node" | "PersistentVolume" | "Namespace")
}

/// Convert a JSON value to a Resource, injecting `kind` if missing.
fn json_to_resource(
    json: &serde_json::Value,
    kind: &str,
    cluster_id: uuid::Uuid,
) -> Option<baeus_core::resource::Resource> {
    let mut json = json.clone();
    if json.get("kind").is_none() {
        if let Some(m) = json.as_object_mut() {
            m.insert("kind".to_string(), serde_json::json!(kind));
        }
    }
    resource::resource_from_json(&json, cluster_id)
}

/// Fetch only the resources directly related to the focused resource by walking
/// ownership chains up and down, plus cross-references (Service selectors,
/// Ingress backends, Pod→Node). Typically produces 5-15 resources with 3-7 API
/// calls instead of the old approach of bulk-listing 10+ kinds.
async fn fetch_focused_topology(
    client: &KubeClient,
    focus_kind: &str,
    focus_name: &str,
    namespace: Option<&str>,
) -> anyhow::Result<Vec<baeus_core::resource::Resource>> {
    use std::collections::HashSet;
    use uuid::Uuid;

    let cluster_id = Uuid::nil();
    let mut resources: Vec<baeus_core::resource::Resource> = Vec::new();
    let mut seen_uids: HashSet<String> = HashSet::new();

    // Helper: add a resource if not already seen (by UID).
    let add_resource = |r: baeus_core::resource::Resource, seen: &mut HashSet<String>| -> bool {
        r.uid.is_empty() || seen.insert(r.uid.clone())
    };

    // 1. Fetch the focused resource
    let focus_ns = if is_cluster_scoped(focus_kind) { None } else { namespace };
    let focus_json =
        baeus_core::client::get_resource(client, focus_kind, focus_name, focus_ns).await?;
    let focus_res = json_to_resource(&focus_json, focus_kind, cluster_id)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse focused resource"))?;

    if add_resource(focus_res.clone(), &mut seen_uids) {
        resources.push(focus_res.clone());
    }

    // 2. Walk UP via ownerReferences (max 5 hops)
    let mut current = focus_res.clone();
    for _ in 0..5 {
        if current.owner_references.is_empty() {
            break;
        }
        let owner_ref = &current.owner_references[0];
        let owner_ns =
            if is_cluster_scoped(&owner_ref.kind) { None } else { current.namespace.as_deref() };
        match baeus_core::client::get_resource(client, &owner_ref.kind, &owner_ref.name, owner_ns)
            .await
        {
            Ok(json) => {
                if let Some(r) = json_to_resource(&json, &owner_ref.kind, cluster_id) {
                    let is_new = add_resource(r.clone(), &mut seen_uids);
                    if is_new {
                        resources.push(r.clone());
                    }
                    current = r;
                } else {
                    break;
                }
            }
            Err(e) => {
                tracing::debug!("Topology: failed to fetch owner {}: {e}", owner_ref.name);
                break;
            }
        }
    }

    // 3. Walk DOWN from all collected resources (breadth-first, max depth 3)
    const MAX_CHILDREN_PER_PARENT: usize = 50;
    let mut walk_queue: Vec<(baeus_core::resource::Resource, usize)> =
        resources.iter().map(|r| (r.clone(), 0)).collect();
    let mut walk_idx = 0;

    while walk_idx < walk_queue.len() {
        let (parent, depth) = walk_queue[walk_idx].clone();
        walk_idx += 1;
        if depth >= 3 {
            continue;
        }
        let child_kinds = expected_child_kinds(&parent.kind);
        for &child_kind in child_kinds {
            let child_ns =
                if is_cluster_scoped(child_kind) { None } else { parent.namespace.as_deref() };
            match baeus_core::client::list_resources(client, child_kind, child_ns).await {
                Ok(items) => {
                    let mut count = 0;
                    for item in &items {
                        if count >= MAX_CHILDREN_PER_PARENT {
                            break;
                        }
                        if let Some(r) = json_to_resource(item, child_kind, cluster_id) {
                            // Client-side filter: only children owned by this parent
                            let is_child =
                                r.owner_references.iter().any(|oref| oref.uid == parent.uid);
                            if is_child && add_resource(r.clone(), &mut seen_uids) {
                                resources.push(r.clone());
                                walk_queue.push((r, depth + 1));
                                count += 1;
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::debug!(
                        "Topology: failed to list {child_kind} children of {}: {e}",
                        parent.name
                    );
                }
            }
        }
    }

    // 4. Cross-references

    // 4a. Pod → Node (via spec.nodeName)
    let pod_resources: Vec<_> = resources.iter().filter(|r| r.kind == "Pod").cloned().collect();
    for pod in &pod_resources {
        if let Some(node_name) = pod.spec.get("nodeName").and_then(|n| n.as_str()) {
            if seen_uids.iter().all(|uid| {
                resources
                    .iter()
                    .find(|r| &r.uid == uid)
                    .map(|r| !(r.kind == "Node" && r.name == node_name))
                    .unwrap_or(true)
            }) {
                match baeus_core::client::get_resource(client, "Node", node_name, None).await {
                    Ok(json) => {
                        if let Some(r) = json_to_resource(&json, "Node", cluster_id) {
                            if add_resource(r.clone(), &mut seen_uids) {
                                resources.push(r);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::debug!("Topology: failed to fetch Node {node_name}: {e}");
                    }
                }
            }
        }
    }

    // 4b. Service focus → Pods via selector labels
    if focus_kind == "Service" {
        if let Some(selector) = focus_res.spec.get("selector").and_then(|s| s.as_object()) {
            if !selector.is_empty() {
                let label_selector: String = selector
                    .iter()
                    .filter_map(|(k, v)| v.as_str().map(|val| format!("{k}={val}")))
                    .collect::<Vec<_>>()
                    .join(",");
                match baeus_core::client::list_resources_with_selector(
                    client,
                    "Pod",
                    namespace,
                    &label_selector,
                )
                .await
                {
                    Ok(items) => {
                        for item in items.iter().take(MAX_CHILDREN_PER_PARENT) {
                            if let Some(r) = json_to_resource(item, "Pod", cluster_id) {
                                if add_resource(r.clone(), &mut seen_uids) {
                                    resources.push(r);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::debug!("Topology: failed to list Pods for Service selector: {e}");
                    }
                }
            }
        }
    }

    // 4c. Pod/Deployment/RS focus → Services that select our Pods
    if matches!(focus_kind, "Pod" | "Deployment" | "ReplicaSet") {
        if let Ok(svc_items) =
            baeus_core::client::list_resources(client, "Service", namespace).await
        {
            let our_pods: Vec<_> = resources.iter().filter(|r| r.kind == "Pod").cloned().collect();
            for item in &svc_items {
                if let Some(svc) = json_to_resource(item, "Service", cluster_id) {
                    if seen_uids.contains(&svc.uid) {
                        continue;
                    }
                    if let Some(selector) = svc.spec.get("selector").and_then(|s| s.as_object()) {
                        if !selector.is_empty() {
                            let matches_any = our_pods.iter().any(|pod| {
                                selector.iter().all(|(k, v)| {
                                    v.as_str()
                                        .map(|val| {
                                            pod.labels.get(k).map(|l| l.as_str()) == Some(val)
                                        })
                                        .unwrap_or(false)
                                })
                            });
                            if matches_any && add_resource(svc.clone(), &mut seen_uids) {
                                resources.push(svc);
                            }
                        }
                    }
                }
            }
        }
    }

    // 4d. Ingress focus → backend Services
    if focus_kind == "Ingress" {
        if let Some(rules) = focus_res.spec.get("rules").and_then(|r| r.as_array()) {
            for rule in rules {
                if let Some(paths) =
                    rule.get("http").and_then(|h| h.get("paths")).and_then(|p| p.as_array())
                {
                    for path in paths {
                        if let Some(svc_name) = path
                            .get("backend")
                            .and_then(|b| b.get("service"))
                            .and_then(|s| s.get("name"))
                            .and_then(|n| n.as_str())
                        {
                            match baeus_core::client::get_resource(
                                client, "Service", svc_name, namespace,
                            )
                            .await
                            {
                                Ok(json) => {
                                    if let Some(r) = json_to_resource(&json, "Service", cluster_id)
                                    {
                                        if add_resource(r.clone(), &mut seen_uids) {
                                            resources.push(r);
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::debug!(
                                        "Topology: failed to fetch Service {svc_name}: {e}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 4e. Service focus → Ingresses that reference this Service
    if focus_kind == "Service" {
        if let Ok(ing_items) =
            baeus_core::client::list_resources(client, "Ingress", namespace).await
        {
            for item in &ing_items {
                if let Some(ing) = json_to_resource(item, "Ingress", cluster_id) {
                    if seen_uids.contains(&ing.uid) {
                        continue;
                    }
                    let references_us = ing
                        .spec
                        .get("rules")
                        .and_then(|r| r.as_array())
                        .map(|rules| {
                            rules.iter().any(|rule| {
                                rule.get("http")
                                    .and_then(|h| h.get("paths"))
                                    .and_then(|p| p.as_array())
                                    .map(|paths| {
                                        paths.iter().any(|path| {
                                            path.get("backend")
                                                .and_then(|b| b.get("service"))
                                                .and_then(|s| s.get("name"))
                                                .and_then(|n| n.as_str())
                                                == Some(focus_name)
                                        })
                                    })
                                    .unwrap_or(false)
                            })
                        })
                        .unwrap_or(false);
                    if references_us && add_resource(ing.clone(), &mut seen_uids) {
                        resources.push(ing);
                    }
                }
            }
        }
    }

    Ok(resources)
}

// ===========================================================================
// Cluster-Level Topology (card-style kind-level graph)
// ===========================================================================

/// Kinds displayed in the cluster-level topology graph.
const CLUSTER_TOPOLOGY_KINDS: &[&str] = &[
    "Ingress",
    "Service",
    "Deployment",
    "StatefulSet",
    "DaemonSet",
    "ReplicaSet",
    "Pod",
    "Job",
    "CronJob",
    "Node",
];

/// Build static type-level relationships between resource kinds.
fn cluster_topology_relationships() -> Vec<ResourceRelationship> {
    let edges: &[(&str, &str)] = &[
        ("Ingress", "Service"),
        ("Service", "Pod"),
        ("Deployment", "ReplicaSet"),
        ("ReplicaSet", "Pod"),
        ("StatefulSet", "Pod"),
        ("DaemonSet", "Pod"),
        ("CronJob", "Job"),
        ("Job", "Pod"),
        ("Pod", "Node"),
    ];
    edges
        .iter()
        .map(|(src, tgt)| {
            ResourceRelationship::new(
                ResourceRef::new(*src, *src, None),
                ResourceRef::new(*tgt, *tgt, None),
                RelationshipKind::OwnerReference,
            )
        })
        .collect()
}

/// State for the cluster-level topology view.
#[derive(Debug)]
pub struct ClusterTopologyState {
    pub layout: LayoutState,
    pub kind_counts: HashMap<String, usize>,
    pub selected_kind: Option<String>,
    pub zoom_level: f64,
    pub pan_offset: (f64, f64),
    pub loading: bool,
    pub error: Option<String>,
    pub kind_data: HashMap<String, Vec<serde_json::Value>>,
    pub kind_table_state: Option<ResourceTableState>,
    /// Height of the graph panel in pixels (user-resizable).
    pub graph_height: f32,
    /// Namespace filter: empty means "All Namespaces", otherwise a set of selected namespaces.
    pub selected_namespaces: std::collections::HashSet<String>,
    /// Whether the namespace dropdown panel is open.
    pub ns_dropdown_open: bool,
    /// Search query text for filtering the namespace dropdown list.
    pub ns_search_query: String,
}

impl ClusterTopologyState {
    pub fn new_loading() -> Self {
        Self {
            layout: LayoutState::empty(),
            kind_counts: HashMap::new(),
            selected_kind: None,
            zoom_level: 1.0,
            pan_offset: (0.0, 0.0),
            loading: true,
            error: None,
            kind_data: HashMap::new(),
            kind_table_state: None,
            graph_height: 350.0,
            selected_namespaces: std::collections::HashSet::new(),
            ns_dropdown_open: false,
            ns_search_query: String::new(),
        }
    }
}

/// Card dimensions for cluster topology kind nodes.
const CLUSTER_NODE_W: f64 = 220.0;
const CLUSTER_NODE_H: f64 = 60.0;

/// Build a LayoutState from kind_counts, filtering to kinds with count > 0.
fn build_cluster_topology_layout(kind_counts: &HashMap<String, usize>) -> LayoutState {
    let all_rels = cluster_topology_relationships();
    let active_kinds: std::collections::HashSet<&str> =
        kind_counts.iter().filter(|(_, count)| **count > 0).map(|(k, _)| k.as_str()).collect();
    let filtered_rels: Vec<ResourceRelationship> = all_rels
        .into_iter()
        .filter(|r| {
            active_kinds.contains(r.source.kind.as_str())
                && active_kinds.contains(r.target.kind.as_str())
        })
        .collect();
    if filtered_rels.is_empty() {
        // Create standalone nodes for active kinds with no relationships
        let nodes = active_kinds
            .iter()
            .enumerate()
            .map(|(i, kind)| GraphNode {
                id: format!("{kind}/{kind}"),
                kind: kind.to_string(),
                name: kind.to_string(),
                x: (i as f64) * 280.0,
                y: 0.0,
                layer: 0,
            })
            .collect();
        return LayoutState { nodes, edges: Vec::new() };
    }
    compute_layout_lr(&filtered_rels)
}

/// Fetch resource counts and data for all topology kinds from the cluster.
async fn fetch_cluster_topology_data(
    client: &KubeClient,
) -> (HashMap<String, usize>, HashMap<String, Vec<serde_json::Value>>) {
    let mut kind_counts = HashMap::new();
    let mut kind_data = HashMap::new();

    for &kind in CLUSTER_TOPOLOGY_KINDS {
        match baeus_core::client::list_resources(client, kind, None).await {
            Ok(items) => {
                kind_counts.insert(kind.to_string(), items.len());
                kind_data.insert(kind.to_string(), items);
            }
            Err(e) => {
                tracing::debug!("Cluster topology: failed to list {kind}: {e}");
                kind_counts.insert(kind.to_string(), 0);
            }
        }
    }

    (kind_counts, kind_data)
}

// ---------------------------------------------------------------------------
// impl AppShell — cluster topology rendering & data loading
// ---------------------------------------------------------------------------

impl AppShell {
    /// Top-level render for the cluster topology view: graph + table split.
    pub(crate) fn render_cluster_topology(
        &mut self,
        cx: &mut Context<Self>,
        cluster_context: &str,
        text: Rgba,
        text_secondary: Rgba,
        bg: Rgba,
    ) -> Div {
        let Some(state) = self.cluster_topology_states.get(cluster_context) else {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .bg(bg)
                .text_color(text_secondary)
                .text_sm()
                .child("Loading cluster topology...");
        };

        if state.loading {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .bg(bg)
                .text_color(text_secondary)
                .text_sm()
                .child("Loading cluster topology...");
        }

        if let Some(err) = &state.error {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .bg(bg)
                .text_color(self.theme.colors.error.to_gpui())
                .text_sm()
                .child(format!("Topology error: {err}"));
        }

        if state.layout.nodes.is_empty() {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .bg(bg)
                .text_color(text_secondary)
                .text_sm()
                .child("No resources found in cluster.");
        }

        // Split: graph panel (resizable) + drag handle + table panel (flex-1)
        let graph_panel =
            self.render_cluster_topology_graph(cx, cluster_context, text, text_secondary, bg);

        // Drag handle for resizing graph/table split
        let border_color = self.theme.colors.border.to_gpui();
        let accent = self.theme.colors.accent.to_gpui();
        let ctx_for_resize = cluster_context.to_string();
        let graph_height = self
            .cluster_topology_states
            .get(cluster_context)
            .map(|s| s.graph_height)
            .unwrap_or(350.0);
        let resize_handle = div()
            .id("ctopo-resize-handle")
            .w_full()
            .h(px(5.0))
            .flex_shrink_0()
            .bg(border_color)
            .cursor(CursorStyle::ResizeUpDown)
            .hover(|s| s.bg(accent))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _window, _cx| {
                    this.is_dragging_cluster_topo_resize = true;
                    this.cluster_topo_resize_start_y = event.position.y.into();
                    this.cluster_topo_resize_start_height = graph_height;
                    this.cluster_topo_resize_context = Some(ctx_for_resize.clone());
                }),
            );

        let table_panel =
            self.render_cluster_topology_table(cx, cluster_context, text, text_secondary, bg);

        let ctx_for_dropdown = cluster_context.to_string();
        let ns_dropdown = self.render_cluster_topo_ns_dropdown(cx, &ctx_for_dropdown);

        let mut table_wrapper =
            div().flex_1().flex().flex_col().relative().overflow_hidden().child(table_panel);

        if let Some(dropdown_panel) = ns_dropdown {
            table_wrapper = table_wrapper.child(
                dropdown_panel
                    .absolute()
                    .top(px(36.0)) // Below the header bar
                    .right(px(8.0)),
            );
        }

        div()
            .flex_1()
            .flex()
            .flex_col()
            .overflow_hidden()
            .child(graph_panel)
            .child(resize_handle)
            .child(table_wrapper)
    }

    /// Render the graph panel with kind nodes and relationship edges.
    fn render_cluster_topology_graph(
        &mut self,
        cx: &mut Context<Self>,
        cluster_context: &str,
        text: Rgba,
        text_secondary: Rgba,
        bg: Rgba,
    ) -> Div {
        let state = match self.cluster_topology_states.get(cluster_context) {
            Some(s) => s,
            None => return div(),
        };

        let zoom = state.zoom_level;
        let pan = state.pan_offset;
        let graph_height = state.graph_height;
        let accent = self.theme.colors.accent.to_gpui();
        let border_color = self.theme.colors.border.to_gpui();
        let surface = self.theme.colors.surface.to_gpui();
        let theme = self.theme.clone();

        let (min_x, _max_x, min_y, _max_y) = cluster_graph_bounds(&state.layout.nodes);
        let pad = 40.0;
        let origin_x = min_x - pad;
        let origin_y = min_y - pad;

        let ctx_for_scroll = cluster_context.to_string();
        let ctx_for_drag = cluster_context.to_string();

        let mut inner = div()
            .id("cluster-topo-container")
            .h(px(graph_height))
            .relative()
            .overflow_hidden()
            .bg(bg);

        // Edge canvas
        let edges = state.layout.edges.clone();
        let nodes_for_edges = state.layout.nodes.clone();
        let theme_for_edges = theme.clone();
        let edge_canvas = canvas(
            move |_bounds, _window, _cx| {},
            move |bounds, (), window, _cx| {
                let ox = bounds.origin.x;
                let oy = bounds.origin.y;
                let node_map: HashMap<&str, &GraphNode> =
                    nodes_for_edges.iter().map(|n| (n.id.as_str(), n)).collect();

                for edge in &edges {
                    let Some(src) = node_map.get(edge.source_id.as_str()) else {
                        continue;
                    };
                    let Some(tgt) = node_map.get(edge.target_id.as_str()) else {
                        continue;
                    };

                    let sx = ox + px(((src.x - origin_x + CLUSTER_NODE_W) * zoom + pan.0) as f32);
                    let sy =
                        oy + px(((src.y - origin_y + CLUSTER_NODE_H / 2.0) * zoom + pan.1) as f32);
                    let tx = ox + px(((tgt.x - origin_x) * zoom + pan.0) as f32);
                    let ty =
                        oy + px(((tgt.y - origin_y + CLUSTER_NODE_H / 2.0) * zoom + pan.1) as f32);

                    let edge_color = kind_color(&src.kind, &theme_for_edges);
                    let edge_rgba =
                        Rgba { r: edge_color.r, g: edge_color.g, b: edge_color.b, a: 0.6 };

                    let mid_x = (sx + tx) * 0.5;
                    let mut builder = PathBuilder::stroke(px(2.0));
                    builder.move_to(point(sx, sy));
                    builder.cubic_bezier_to(point(tx, ty), point(mid_x, sy), point(mid_x, ty));
                    if let Ok(path) = builder.build() {
                        window.paint_path(path, edge_rgba);
                    }

                    paint_arrow_head(window, tx, ty, edge_rgba);
                }
            },
        )
        .w_full()
        .h_full()
        .absolute()
        .top_0()
        .left_0();

        inner = inner.child(edge_canvas);

        // Kind cards
        let kind_counts = state.kind_counts.clone();
        let selected_kind = state.selected_kind.clone();
        for node in &state.layout.nodes {
            let kind_str = &node.kind;
            let count = kind_counts.get(kind_str).copied().unwrap_or(0);
            let is_selected = selected_kind.as_deref() == Some(kind_str);

            let dx = ((node.x - origin_x) * zoom + pan.0) as f32;
            let dy = ((node.y - origin_y) * zoom + pan.1) as f32;

            let card = self.render_cluster_kind_card(
                cx,
                kind_str,
                count,
                is_selected,
                cluster_context,
                text,
                text_secondary,
                surface,
                border_color,
                accent,
                zoom,
            );

            inner = inner.child(div().absolute().left(px(dx)).top(px(dy)).child(card));
        }

        // Zoom controls
        let ctx_zoom_in = cluster_context.to_string();
        let ctx_zoom_out = cluster_context.to_string();
        let ctx_reset = cluster_context.to_string();
        let controls = self.render_cluster_topo_controls(
            cx,
            ctx_zoom_in,
            ctx_zoom_out,
            ctx_reset,
            text,
            surface,
            border_color,
        );
        inner = inner.child(controls);

        // Scroll-wheel zoom
        inner = inner.on_scroll_wheel(cx.listener(
            move |this, event: &ScrollWheelEvent, _window, cx| {
                if let Some(state) = this.cluster_topology_states.get_mut(&ctx_for_scroll) {
                    let delta = match event.delta {
                        ScrollDelta::Pixels(p) => {
                            let raw = p.y / px(1.0);
                            raw as f64 / 200.0
                        }
                        ScrollDelta::Lines(l) => l.y as f64 * 0.1,
                    };
                    state.zoom_level = (state.zoom_level + delta).clamp(0.2, 3.0);
                    cx.notify();
                }
            },
        ));

        // Mouse drag start
        inner = inner.on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
                let x: f32 = event.position.x.into();
                let y: f32 = event.position.y.into();
                this.is_dragging_cluster_topology = true;
                this.cluster_topology_drag_last = (x, y);
                this.cluster_topology_drag_context = Some(ctx_for_drag.clone());
                cx.notify();
            }),
        );

        div().flex_shrink_0().h(px(graph_height)).child(inner)
    }

    /// Render the table panel below the graph.
    fn render_cluster_topology_table(
        &mut self,
        cx: &mut Context<Self>,
        cluster_context: &str,
        text: Rgba,
        text_secondary: Rgba,
        bg: Rgba,
    ) -> Div {
        let state = match self.cluster_topology_states.get(cluster_context) {
            Some(s) => s,
            None => return div().flex_1().bg(bg),
        };

        let Some(ref selected_kind) = state.selected_kind else {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .bg(bg)
                .text_color(text_secondary)
                .text_sm()
                .child("Click a resource kind to view instances");
        };

        let kind_label = selected_kind.clone();
        let accent = self.theme.colors.accent.to_gpui();
        let border_color = self.theme.colors.border.to_gpui();
        let surface = self.theme.colors.surface.to_gpui();
        let selected_ns = state.selected_namespaces.clone();

        // Header bar with kind name, count, and namespace filter
        let ctx_for_ns = cluster_context.to_string();
        let ns_display = if selected_ns.is_empty() {
            "All Namespaces".to_string()
        } else if selected_ns.len() == 1 {
            selected_ns.iter().next().unwrap().clone()
        } else {
            format!("{} namespaces", selected_ns.len())
        };

        // Count after filtering
        let filtered_count = if let Some(ref ts) = state.kind_table_state {
            ts.rows
                .iter()
                .filter(|row| {
                    if selected_ns.is_empty() {
                        return true;
                    }
                    row.namespace.as_deref().map(|ns| selected_ns.contains(ns)).unwrap_or(false)
                })
                .count()
        } else {
            0
        };

        let header = div()
            .h(px(36.0))
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_3()
            .bg(surface)
            .border_b_1()
            .border_color(border_color)
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(text)
                    .child(SharedString::from(format!("{kind_label} ({filtered_count})"))),
            )
            .child(div().flex_grow())
            .child(self.render_cluster_topo_ns_button(
                cx,
                &ctx_for_ns,
                &ns_display,
                text_secondary,
                border_color,
            ));

        // Build table rows from kind_data, filtered by namespace
        let table_content = if let Some(ref ts) = state.kind_table_state {
            let sorted_rows: Vec<&_> = ts
                .filtered_rows()
                .into_iter()
                .filter(|row| {
                    if selected_ns.is_empty() {
                        return true;
                    }
                    row.namespace.as_deref().map(|ns| selected_ns.contains(ns)).unwrap_or(false)
                })
                .collect();
            let cluster_ctx = cluster_context.to_string();

            let mut table = div().id("cluster-topo-table").flex_1().overflow_y_scroll().bg(bg);

            // Column header row
            let mut header_row = div()
                .h(px(28.0))
                .flex()
                .flex_row()
                .items_center()
                .px_2()
                .bg(surface)
                .border_b_1()
                .border_color(border_color);

            for (i, col) in ts.columns.iter().enumerate() {
                if i < ts.visible_columns.len() && !ts.visible_columns[i] {
                    continue;
                }
                let w = if i < ts.column_widths.len() { ts.column_widths[i] } else { 100.0 };
                header_row = header_row.child(
                    div()
                        .w(px(w))
                        .flex_shrink_0()
                        .px_1()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(text_secondary)
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .child(SharedString::from(col.label.clone())),
                );
            }
            table = table.child(header_row);

            // Data rows
            for row in &sorted_rows {
                let row_kind = row.kind.clone();
                let row_name = row.name.clone();
                let row_ns = row.namespace.clone();
                let ctx_click = cluster_ctx.clone();

                let mut row_div = div()
                    .id(ElementId::Name(SharedString::from(format!(
                        "ctopo-row-{}-{}",
                        row.uid, row.name,
                    ))))
                    .h(px(28.0))
                    .flex()
                    .flex_row()
                    .items_center()
                    .px_2()
                    .border_b_1()
                    .border_color(Rgba {
                        r: border_color.r,
                        g: border_color.g,
                        b: border_color.b,
                        a: 0.3,
                    })
                    .cursor_pointer()
                    .hover(|s| s.bg(Rgba { r: accent.r, g: accent.g, b: accent.b, a: 0.08 }))
                    .on_click(cx.listener(move |this, _event: &ClickEvent, _window, cx| {
                        let target = crate::layout::NavigationTarget::ResourceDetail {
                            cluster_context: ctx_click.clone(),
                            kind: row_kind.clone(),
                            name: row_name.clone(),
                            namespace: row_ns.clone(),
                        };
                        this.workspace.open_tab(target.clone());
                        this.push_navigation_history(target);
                        this.trigger_data_loading_for_active_tab(cx);
                        cx.notify();
                    }));

                for (i, cell) in row.cells.iter().enumerate() {
                    if i < ts.visible_columns.len() && !ts.visible_columns[i] {
                        continue;
                    }
                    let w = if i < ts.column_widths.len() { ts.column_widths[i] } else { 100.0 };
                    row_div = row_div.child(
                        div()
                            .w(px(w))
                            .flex_shrink_0()
                            .px_1()
                            .text_xs()
                            .text_color(text)
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .child(SharedString::from(cell.to_string())),
                    );
                }

                table = table.child(row_div);
            }

            table
        } else {
            div()
                .id("ctopo-table-empty")
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .bg(bg)
                .text_color(text_secondary)
                .text_sm()
                .child("No data available")
        };

        div().flex_1().flex().flex_col().overflow_hidden().child(header).child(table_content)
    }

    /// Render a kind card for the cluster topology graph.
    #[allow(clippy::too_many_arguments)]
    fn render_cluster_kind_card(
        &self,
        cx: &mut Context<Self>,
        kind: &str,
        count: usize,
        is_selected: bool,
        cluster_context: &str,
        text: Rgba,
        _text_secondary: Rgba,
        surface: Rgba,
        border_color: Rgba,
        accent: Rgba,
        zoom: f64,
    ) -> Stateful<Div> {
        let kc = kind_color(kind, &self.theme);
        let klabel = kind_label(kind);
        let card_text = SharedString::from(format!("{klabel} ({count})"));
        let kind_owned = kind.to_string();
        let ctx_owned = cluster_context.to_string();

        div()
            .id(ElementId::Name(SharedString::from(format!("ctopo-kind-{kind}"))))
            .w(px(CLUSTER_NODE_W as f32 * zoom as f32))
            .h(px(CLUSTER_NODE_H as f32 * zoom as f32))
            .rounded(px(8.0))
            .bg(if is_selected {
                Rgba { r: accent.r, g: accent.g, b: accent.b, a: 0.08 }
            } else {
                surface
            })
            .border_1()
            .border_color(if is_selected { accent } else { border_color })
            .when(is_selected, |d| d.border_2())
            .overflow_hidden()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .px_3()
            .cursor_pointer()
            .child(
                // Resource icon
                div()
                    .w(px(20.0))
                    .h(px(20.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .flex_shrink_0()
                    .child(
                        gpui_component::Icon::new(ResourceIcon::from_kind(kind))
                            .size(px(16.0))
                            .text_color(kc),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(text)
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .child(card_text),
            )
            .on_click(cx.listener(move |this, _event: &ClickEvent, _window, cx| {
                if let Some(state) = this.cluster_topology_states.get_mut(&ctx_owned) {
                    state.selected_kind = Some(kind_owned.clone());
                    // Build table state from pre-fetched data
                    if let Some(items) = state.kind_data.get(&kind_owned) {
                        let cols = columns_for_kind(&kind_owned);
                        let mut ts = ResourceTableState::new(cols, 50);
                        for item in items {
                            ts.rows.push(json_to_table_row(&kind_owned, item));
                        }
                        state.kind_table_state = Some(ts);
                    }
                }
                cx.notify();
            }))
    }

    /// Zoom controls for cluster topology.
    #[allow(clippy::too_many_arguments)]
    fn render_cluster_topo_controls(
        &self,
        cx: &mut Context<Self>,
        ctx_zoom_in: String,
        ctx_zoom_out: String,
        ctx_reset: String,
        text: Rgba,
        surface: Rgba,
        border_color: Rgba,
    ) -> Div {
        let zoom_in_btn = div()
            .id("ctopo-zoom-in")
            .px_2()
            .py_1()
            .rounded_md()
            .bg(surface)
            .border_1()
            .border_color(border_color)
            .cursor_pointer()
            .text_sm()
            .text_color(text)
            .child("+")
            .on_click(cx.listener(move |this, _e, _w, cx| {
                if let Some(state) = this.cluster_topology_states.get_mut(&ctx_zoom_in) {
                    state.zoom_level = (state.zoom_level + 0.15).min(3.0);
                    cx.notify();
                }
            }));

        let zoom_out_btn = div()
            .id("ctopo-zoom-out")
            .px_2()
            .py_1()
            .rounded_md()
            .bg(surface)
            .border_1()
            .border_color(border_color)
            .cursor_pointer()
            .text_sm()
            .text_color(text)
            .child("\u{2212}")
            .on_click(cx.listener(move |this, _e, _w, cx| {
                if let Some(state) = this.cluster_topology_states.get_mut(&ctx_zoom_out) {
                    state.zoom_level = (state.zoom_level - 0.15).max(0.2);
                    cx.notify();
                }
            }));

        let zoom_reset_btn = div()
            .id("ctopo-zoom-reset")
            .px_2()
            .py_1()
            .rounded_md()
            .bg(surface)
            .border_1()
            .border_color(border_color)
            .cursor_pointer()
            .text_sm()
            .text_color(text)
            .child("Reset")
            .on_click(cx.listener(move |this, _e, _w, cx| {
                if let Some(state) = this.cluster_topology_states.get_mut(&ctx_reset) {
                    state.zoom_level = 1.0;
                    state.pan_offset = (0.0, 0.0);
                    cx.notify();
                }
            }));

        div()
            .absolute()
            .bottom_3()
            .right_3()
            .flex()
            .flex_row()
            .gap_1()
            .child(zoom_out_btn)
            .child(zoom_reset_btn)
            .child(zoom_in_btn)
    }

    /// Render the namespace dropdown button for the cluster topology table header.
    fn render_cluster_topo_ns_button(
        &self,
        cx: &mut Context<Self>,
        cluster_context: &str,
        display_label: &str,
        _text_secondary: Rgba,
        _border_color: Rgba,
    ) -> Stateful<Div> {
        let ctx = cluster_context.to_string();

        // Collect unique namespaces from the selected kind's data
        let namespaces: Vec<String> =
            if let Some(state) = self.cluster_topology_states.get(cluster_context) {
                if let Some(ref ts) = state.kind_table_state {
                    let mut ns_set: std::collections::BTreeSet<String> =
                        std::collections::BTreeSet::new();
                    for row in &ts.rows {
                        if let Some(ref ns) = row.namespace {
                            ns_set.insert(ns.clone());
                        }
                    }
                    ns_set.into_iter().collect()
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            };

        // If no namespaces (cluster-scoped resources), return empty
        if namespaces.is_empty() {
            return div().id("ctopo-ns-btn-empty");
        }

        let is_open = self
            .cluster_topology_states
            .get(cluster_context)
            .map(|s| s.ns_dropdown_open)
            .unwrap_or(false);

        let arrow: SharedString = if is_open { "\u{25B2}".into() } else { "\u{25BC}".into() };
        let display = SharedString::from(display_label.to_string());

        div()
            .id(ElementId::Name(SharedString::from(format!("ctopo-ns-btn-{}", cluster_context))))
            .flex()
            .items_center()
            .gap(px(4.0))
            .px_3()
            .py_1()
            .rounded(px(6.0))
            .bg(gpui::rgb(0x374151))
            .text_sm()
            .text_color(gpui::rgb(0xD1D5DB))
            .cursor_pointer()
            .hover(|s| s.bg(gpui::rgb(0x4B5563)))
            .on_click(cx.listener(move |this, _evt, window, cx| {
                if let Some(state) = this.cluster_topology_states.get_mut(&ctx) {
                    state.ns_dropdown_open = !state.ns_dropdown_open;
                    if state.ns_dropdown_open {
                        let ctx_for_sub = ctx.clone();
                        let input = cx.new(|cx| {
                            gpui_component::input::InputState::new(window, cx)
                                .placeholder("Filter namespaces...")
                        });
                        let sub = cx.subscribe(
                            &input,
                            move |this: &mut AppShell,
                                  entity,
                                  event: &gpui_component::input::InputEvent,
                                  cx| {
                                if matches!(event, gpui_component::input::InputEvent::Change) {
                                    let val = entity.read(cx).value().to_string();
                                    if let Some(st) =
                                        this.cluster_topology_states.get_mut(&ctx_for_sub)
                                    {
                                        st.ns_search_query = val;
                                    }
                                    cx.notify();
                                }
                            },
                        );
                        let fh = input.read(cx).focus_handle(cx);
                        fh.focus(window);
                        this.topo_ns_search_input = Some(input);
                        this._topo_ns_search_subscription = Some(sub);
                    } else {
                        state.ns_search_query.clear();
                        this.topo_ns_search_input = None;
                        this._topo_ns_search_subscription = None;
                    }
                }
                cx.notify();
            }))
            .child(display)
            .child(div().text_xs().text_color(gpui::rgb(0x9CA3AF)).child(arrow))
    }

    /// Render the topology namespace dropdown overlay panel (appears below the button).
    pub(crate) fn render_cluster_topo_ns_dropdown(
        &self,
        cx: &mut Context<Self>,
        cluster_context: &str,
    ) -> Option<Stateful<Div>> {
        let state = self.cluster_topology_states.get(cluster_context)?;
        if !state.ns_dropdown_open {
            return None;
        }

        // Collect namespaces, filtered by search query
        let namespaces: Vec<String> = if let Some(ref ts) = state.kind_table_state {
            let mut ns_set: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
            for row in &ts.rows {
                if let Some(ref ns) = row.namespace {
                    ns_set.insert(ns.clone());
                }
            }
            let query = state.ns_search_query.to_lowercase();
            ns_set
                .into_iter()
                .filter(|ns| query.is_empty() || ns.to_lowercase().contains(&query))
                .collect()
        } else {
            Vec::new()
        };

        let ctx_dismiss = cluster_context.to_string();
        let ctx_all = cluster_context.to_string();
        let all_selected = state.selected_namespaces.is_empty();

        let mut panel = div()
            .id(ElementId::Name(SharedString::from(format!(
                "ctopo-ns-dropdown-{}",
                cluster_context
            ))))
            .occlude()
            .w(px(260.0))
            .max_h(px(360.0))
            .flex()
            .flex_col()
            .bg(gpui::rgb(0x1F2937))
            .border_1()
            .border_color(gpui::rgb(0x4B5563))
            .rounded(px(6.0))
            .shadow_lg()
            .overflow_hidden();

        // Search input
        if let Some(input_entity) = &self.topo_ns_search_input {
            panel = panel.child(
                div().border_b_1().border_color(gpui::rgb(0x4B5563)).px_1().py_1().child(
                    gpui_component::input::Input::new(input_entity)
                        .prefix(
                            gpui_component::Icon::new(gpui_component::IconName::Search)
                                .size(px(14.0)),
                        )
                        .cleanable(true)
                        .with_size(gpui_component::Size::Small),
                ),
            );
        }

        // "All Namespaces" row
        panel = panel.child(
            div()
                .id(ElementId::Name(SharedString::from("ctopo-ns-row-all")))
                .flex()
                .items_center()
                .gap(px(8.0))
                .w_full()
                .px_3()
                .py(px(6.0))
                .border_b_1()
                .border_color(gpui::rgb(0x374151))
                .cursor_pointer()
                .hover(|s| s.bg(gpui::rgb(0x374151)))
                .when(all_selected, |el| el.bg(Rgba { r: 0.118, g: 0.227, b: 0.373, a: 0.3 }))
                .on_click(cx.listener(move |this, _evt, _window, _cx| {
                    if let Some(st) = this.cluster_topology_states.get_mut(&ctx_all) {
                        st.selected_namespaces.clear();
                    }
                }))
                .child(
                    div()
                        .text_sm()
                        .text_color(gpui::rgb(0xD1D5DB))
                        .font_weight(FontWeight::MEDIUM)
                        .child("All Namespaces"),
                )
                .child(div().flex_grow())
                .when(all_selected, |el| {
                    el.child(div().text_color(gpui::rgb(0x60A5FA)).child(
                        gpui_component::Icon::new(gpui_component::IconName::Check).size(px(14.0)),
                    ))
                }),
        );

        // Scrollable namespace rows
        let mut rows_container = div()
            .id(ElementId::Name(SharedString::from("ctopo-ns-rows-scroll")))
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .max_h(px(260.0));

        for (idx, ns) in namespaces.iter().enumerate() {
            let ns_string = ns.to_string();
            let ctx_toggle = cluster_context.to_string();
            let is_selected = state.selected_namespaces.contains(ns);
            let ns_label = SharedString::from(ns_string.clone());
            let row_id = ElementId::Name(SharedString::from(format!("ctopo-ns-row-{idx}")));

            let row = div()
                .id(row_id)
                .flex()
                .items_center()
                .gap(px(8.0))
                .w_full()
                .px_3()
                .py(px(5.0))
                .cursor_pointer()
                .hover(|s| s.bg(gpui::rgb(0x374151)))
                .on_click(cx.listener(move |this, _evt, _window, _cx| {
                    if let Some(st) = this.cluster_topology_states.get_mut(&ctx_toggle) {
                        if st.selected_namespaces.contains(&ns_string) {
                            st.selected_namespaces.remove(&ns_string);
                        } else {
                            st.selected_namespaces.insert(ns_string.clone());
                        }
                    }
                }))
                .child(div().flex_none().text_color(gpui::rgb(0x6B7280)).child(
                    gpui_component::Icon::new(gpui_component::IconName::Folder).size(px(14.0)),
                ))
                .child(div().flex_1().text_sm().text_color(gpui::rgb(0xD1D5DB)).child(ns_label))
                .when(is_selected, |el| {
                    el.child(div().flex_none().text_color(gpui::rgb(0x60A5FA)).child(
                        gpui_component::Icon::new(gpui_component::IconName::Check).size(px(14.0)),
                    ))
                });

            rows_container = rows_container.child(row);
        }

        panel = panel.child(rows_container);
        Some(panel.on_mouse_down_out(cx.listener(
            move |this, _evt: &MouseDownEvent, _window, _cx| {
                if let Some(st) = this.cluster_topology_states.get_mut(&ctx_dismiss) {
                    st.ns_dropdown_open = false;
                    st.ns_search_query.clear();
                }
                this.topo_ns_search_input = None;
                this._topo_ns_search_subscription = None;
            },
        )))
    }

    /// Start async loading of cluster topology data.
    pub(crate) fn start_cluster_topology_loading(
        &mut self,
        cluster_context: &str,
        cx: &mut Context<Self>,
    ) {
        self.cluster_topology_states
            .insert(cluster_context.to_string(), ClusterTopologyState::new_loading());

        let Some(client) = self.active_clients.get(cluster_context).cloned() else {
            if let Some(state) = self.cluster_topology_states.get_mut(cluster_context) {
                state.loading = false;
                state.error = Some(
                    "No active client for this cluster — click Connect in the sidebar first"
                        .to_string(),
                );
            }
            cx.notify();
            return;
        };

        let ctx_key = cluster_context.to_string();
        let tokio_handle = cx.global::<crate::layout::app_shell::GpuiTokioHandle>().0.clone();

        cx.spawn(async move |this: WeakEntity<AppShell>, cx: &mut AsyncApp| {
            let result =
                tokio_handle.spawn(async move { fetch_cluster_topology_data(&client).await }).await;

            this.update(cx, |this, cx| {
                match result {
                    Ok((kind_counts, kind_data)) => {
                        let layout = build_cluster_topology_layout(&kind_counts);

                        // Center graph in viewport
                        let pan = if !layout.nodes.is_empty() {
                            let (min_x, max_x, min_y, max_y) = cluster_graph_bounds(&layout.nodes);
                            let graph_cx = (min_x + max_x) / 2.0;
                            let graph_cy = (min_y + max_y) / 2.0;
                            (400.0 - graph_cx, 150.0 - graph_cy)
                        } else {
                            (0.0, 0.0)
                        };

                        this.cluster_topology_states.insert(
                            ctx_key,
                            ClusterTopologyState {
                                layout,
                                kind_counts,
                                selected_kind: None,
                                zoom_level: 1.0,
                                pan_offset: pan,
                                loading: false,
                                error: None,
                                kind_data,
                                kind_table_state: None,
                                graph_height: 350.0,
                                selected_namespaces: std::collections::HashSet::new(),
                                ns_dropdown_open: false,
                                ns_search_query: String::new(),
                            },
                        );
                    }
                    Err(e) => {
                        if let Some(state) = this.cluster_topology_states.get_mut(&ctx_key) {
                            state.loading = false;
                            state.error = Some(format!("Task join error: {e}"));
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

/// Graph bounds helper for cluster topology nodes.
fn cluster_graph_bounds(nodes: &[GraphNode]) -> (f64, f64, f64, f64) {
    if nodes.is_empty() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let mut min_x = f64::MAX;
    let mut max_x = f64::MIN;
    let mut min_y = f64::MAX;
    let mut max_y = f64::MIN;
    for n in nodes {
        if n.x < min_x {
            min_x = n.x;
        }
        if n.x + CLUSTER_NODE_W > max_x {
            max_x = n.x + CLUSTER_NODE_W;
        }
        if n.y < min_y {
            min_y = n.y;
        }
        if n.y + CLUSTER_NODE_H > max_y {
            max_y = n.y + CLUSTER_NODE_H;
        }
    }
    (min_x, max_x, min_y, max_y)
}
