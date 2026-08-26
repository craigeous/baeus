use crate::components::resource_map::{ResourceMapState, compute_layout};
use baeus_core::resource::{Resource, build_relationship_graph};

// ---------------------------------------------------------------------------
// T120: Namespace map view
// ---------------------------------------------------------------------------

/// State for the namespace resource map view.
/// Loads resources for a given namespace, builds the relationship graph,
/// computes the layout, and displays it in a ResourceMapState.
#[derive(Debug, Default)]
pub struct NamespaceMapState {
    pub namespace: Option<String>,
    pub resource_map: ResourceMapState,
    pub loading: bool,
    pub error: Option<String>,
}

impl NamespaceMapState {
    /// Set the namespace to display. Clears the current map.
    pub fn set_namespace(&mut self, namespace: Option<String>) {
        self.namespace = namespace;
        self.resource_map = ResourceMapState::default();
        self.error = None;
    }

    /// Process a list of resources: extract relationships, compute layout,
    /// and set it on the resource map.
    pub fn set_resources(&mut self, resources: &[Resource]) {
        let relationships = build_relationship_graph(resources);
        let layout = compute_layout(&relationships);
        self.resource_map.set_layout(layout);
        self.loading = false;
    }

    /// Select a resource node in the map by its ID.
    pub fn select_resource(&mut self, node_id: &str) {
        self.resource_map.select_node(node_id);
    }

    /// Clear the current resource selection.
    pub fn clear_selection(&mut self) {
        self.resource_map.clear_selection();
    }

    /// Set the loading state.
    pub fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
    }

    /// Set an error message.
    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
        self.loading = false;
    }

    /// Clear the error.
    pub fn clear_error(&mut self) {
        self.error = None;
    }
}

// ---------------------------------------------------------------------------
// T079: GPUI Render implementation
// ---------------------------------------------------------------------------

use crate::theme::Theme;
use gpui::{Context, ElementId, Rgba, SharedString, Window, div, prelude::*, px};

/// Precomputed colors for rendering the namespace map view.
#[allow(dead_code)]
struct NamespaceMapColors {
    background: Rgba,
    surface: Rgba,
    border: Rgba,
    accent: Rgba,
    text_primary: Rgba,
    text_secondary: Rgba,
    text_muted: Rgba,
    error: Rgba,
}

/// View wrapper for NamespaceMapState with theme for rendering.
pub struct NamespaceMapViewComponent {
    pub state: NamespaceMapState,
    pub theme: Theme,
}

impl NamespaceMapViewComponent {
    pub fn new(state: NamespaceMapState, theme: Theme) -> Self {
        Self { state, theme }
    }

    /// Toolbar: namespace label and resource type filter buttons.
    fn render_toolbar(&self, colors: &NamespaceMapColors) -> gpui::Div {
        let namespace_label = match &self.state.namespace {
            Some(ns) => SharedString::from(format!("Namespace: {ns}")),
            None => SharedString::from("Select a namespace"),
        };

        div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .px_3()
            .py_2()
            .gap(px(8.0))
            .border_b_1()
            .border_color(colors.border)
            .bg(colors.surface)
            .child(
                div()
                    .id("namespace-label")
                    .px_3()
                    .py_1()
                    .rounded(px(4.0))
                    .bg(colors.background)
                    .border_1()
                    .border_color(colors.border)
                    .text_sm()
                    .text_color(colors.text_primary)
                    .child(namespace_label),
            )
            .child(div().flex_1())
            .child(self.render_filter_btn("filter-pod", "Pods", colors))
            .child(self.render_filter_btn("filter-service", "Services", colors))
            .child(self.render_filter_btn("filter-deployment", "Deployments", colors))
    }

    /// Resource type filter button.
    fn render_filter_btn(
        &self,
        id: &str,
        label: &str,
        colors: &NamespaceMapColors,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(ElementId::Name(SharedString::from(id.to_string())))
            .px_2()
            .py_1()
            .rounded(px(3.0))
            .bg(colors.surface)
            .border_1()
            .border_color(colors.border)
            .cursor_pointer()
            .text_xs()
            .text_color(colors.text_secondary)
            .child(SharedString::from(label.to_string()))
    }

    /// Map area: embeds ResourceMapComponent.
    fn render_map_area(&self) -> gpui::Div {
        // Note: In a real GPUI app, we would render the ResourceMapComponent
        // as a child view. For now, we render a placeholder indicating the map area.
        div().flex_1().size_full().flex().items_center().justify_center().child(
            div().text_sm().text_color(self.theme.colors.text_secondary.to_gpui()).child(format!(
                "Resource Map: {} nodes, {} edges",
                self.state.resource_map.layout.nodes.len(),
                self.state.resource_map.layout.edges.len()
            )),
        )
    }

    /// Loading indicator.
    fn render_loading(&self, colors: &NamespaceMapColors) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .items_center()
            .justify_center()
            .bg(colors.background)
            .child(div().text_sm().text_color(colors.text_muted).child("Loading resources..."))
    }

    /// Error message.
    fn render_error(&self, colors: &NamespaceMapColors) -> gpui::Div {
        let msg = self.state.error.as_deref().unwrap_or("Unknown error");
        div()
            .flex()
            .flex_col()
            .flex_1()
            .items_center()
            .justify_center()
            .bg(colors.background)
            .px_4()
            .child(
                div().text_sm().text_color(colors.error).child(SharedString::from(msg.to_string())),
            )
    }

    /// Empty state: "Select a namespace" or "No resources found".
    fn render_empty(&self, colors: &NamespaceMapColors) -> gpui::Div {
        let message = if self.state.namespace.is_none() {
            "Select a namespace to view resource relationships"
        } else {
            "No resources found in this namespace"
        };

        div()
            .flex()
            .flex_col()
            .flex_1()
            .items_center()
            .justify_center()
            .bg(colors.background)
            .child(div().text_sm().text_color(colors.text_muted).child(message))
    }
}

impl Render for NamespaceMapViewComponent {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let colors = NamespaceMapColors {
            background: self.theme.colors.background.to_gpui(),
            surface: self.theme.colors.surface.to_gpui(),
            border: self.theme.colors.border.to_gpui(),
            accent: self.theme.colors.accent.to_gpui(),
            text_primary: self.theme.colors.text_primary.to_gpui(),
            text_secondary: self.theme.colors.text_secondary.to_gpui(),
            text_muted: self.theme.colors.text_muted.to_gpui(),
            error: self.theme.colors.error.to_gpui(),
        };

        let mut root = div().flex().flex_col().size_full().bg(colors.background);

        root = root.child(self.render_toolbar(&colors));

        if self.state.loading {
            root = root.child(self.render_loading(&colors));
        } else if self.state.error.is_some() {
            root = root.child(self.render_error(&colors));
        } else if self.state.resource_map.layout.nodes.is_empty() {
            root = root.child(self.render_empty(&colors));
        } else {
            root = root.child(self.render_map_area());
        }

        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use baeus_core::resource::{OwnerReference, Resource};
    use uuid::Uuid;

    fn test_cluster_id() -> Uuid {
        Uuid::new_v4()
    }

    fn make_resource_with_uid(
        uid: &str,
        name: &str,
        namespace: &str,
        kind: &str,
        api_version: &str,
        cluster_id: Uuid,
    ) -> Resource {
        let mut r = Resource::new(name, namespace, kind, api_version, cluster_id);
        r.uid = uid.to_string();
        r
    }

    // --- Default state ---

    #[test]
    fn test_default_state() {
        let state = NamespaceMapState::default();
        assert!(state.namespace.is_none());
        assert!(!state.loading);
        assert!(state.error.is_none());
        assert!(state.resource_map.layout.nodes.is_empty());
        assert!(state.resource_map.selected_node.is_none());
    }

    // --- set_namespace ---

    #[test]
    fn test_set_namespace() {
        let mut state = NamespaceMapState::default();
        state.set_namespace(Some("production".to_string()));
        assert_eq!(state.namespace.as_deref(), Some("production"));
    }

    #[test]
    fn test_set_namespace_clears_map() {
        let mut state = NamespaceMapState::default();
        let cluster = test_cluster_id();

        // Build some resources to populate the map
        let svc = make_resource_with_uid("svc-uid", "svc", "default", "Service", "v1", cluster)
            .with_spec(serde_json::json!({"selector": {"app": "web"}}));
        let pod = make_resource_with_uid("pod-uid", "pod", "default", "Pod", "v1", cluster)
            .with_label("app", "web");
        state.set_resources(&[svc, pod]);
        assert!(!state.resource_map.layout.nodes.is_empty());

        // Changing namespace should clear the layout
        state.set_namespace(Some("other".to_string()));
        assert!(state.resource_map.layout.nodes.is_empty());
    }

    #[test]
    fn test_set_namespace_clears_error() {
        let mut state = NamespaceMapState::default();
        state.set_error("connection failed".to_string());
        assert!(state.error.is_some());

        state.set_namespace(Some("default".to_string()));
        assert!(state.error.is_none());
    }

    // --- set_resources ---

    #[test]
    fn test_set_resources_builds_layout() {
        let mut state = NamespaceMapState::default();
        state.set_namespace(Some("default".to_string()));
        state.set_loading(true);

        let cluster = test_cluster_id();
        let deploy = make_resource_with_uid(
            "deploy-uid",
            "my-deploy",
            "default",
            "Deployment",
            "apps/v1",
            cluster,
        );
        let mut rs =
            make_resource_with_uid("rs-uid", "my-rs", "default", "ReplicaSet", "apps/v1", cluster);
        rs.owner_references.push(OwnerReference {
            uid: "deploy-uid".to_string(),
            kind: "Deployment".to_string(),
            name: "my-deploy".to_string(),
            api_version: "apps/v1".to_string(),
            controller: true,
        });

        state.set_resources(&[deploy, rs]);

        assert!(!state.loading);
        assert_eq!(state.resource_map.layout.nodes.len(), 2);
        assert_eq!(state.resource_map.layout.edges.len(), 1);
    }

    #[test]
    fn test_set_resources_with_empty_list() {
        let mut state = NamespaceMapState::default();
        state.set_resources(&[]);

        assert!(state.resource_map.layout.nodes.is_empty());
        assert!(state.resource_map.layout.edges.is_empty());
    }

    #[test]
    fn test_set_resources_with_no_relationships() {
        let mut state = NamespaceMapState::default();
        let cluster = test_cluster_id();

        let pod1 = make_resource_with_uid("p1", "pod-1", "default", "Pod", "v1", cluster);
        let pod2 = make_resource_with_uid("p2", "pod-2", "default", "Pod", "v1", cluster);

        state.set_resources(&[pod1, pod2]);

        // No relationships, so no nodes or edges in the layout
        assert!(state.resource_map.layout.nodes.is_empty());
    }

    // --- select_resource / clear_selection ---

    #[test]
    fn test_select_resource() {
        let mut state = NamespaceMapState::default();
        state.select_resource("Pod/default/my-pod");
        assert_eq!(state.resource_map.selected_node.as_deref(), Some("Pod/default/my-pod"));
    }

    #[test]
    fn test_clear_selection() {
        let mut state = NamespaceMapState::default();
        state.select_resource("Pod/default/my-pod");
        state.clear_selection();
        assert!(state.resource_map.selected_node.is_none());
    }

    // --- loading / error ---

    #[test]
    fn test_loading_state() {
        let mut state = NamespaceMapState::default();
        assert!(!state.loading);
        state.set_loading(true);
        assert!(state.loading);
        state.set_loading(false);
        assert!(!state.loading);
    }

    #[test]
    fn test_error_state() {
        let mut state = NamespaceMapState::default();
        assert!(state.error.is_none());

        state.set_error("something went wrong".to_string());
        assert_eq!(state.error.as_deref(), Some("something went wrong"));
        assert!(!state.loading); // error clears loading

        state.clear_error();
        assert!(state.error.is_none());
    }

    #[test]
    fn test_error_clears_loading() {
        let mut state = NamespaceMapState::default();
        state.set_loading(true);
        state.set_error("failed".to_string());
        assert!(!state.loading);
    }

    // --- Integration: full pipeline ---

    #[test]
    fn test_full_pipeline_namespace_map() {
        let mut state = NamespaceMapState::default();
        let cluster = test_cluster_id();

        state.set_namespace(Some("production".to_string()));
        state.set_loading(true);

        // Build Ingress -> Service -> Pod chain
        let ingress = make_resource_with_uid(
            "ing-uid",
            "my-ingress",
            "production",
            "Ingress",
            "networking.k8s.io/v1",
            cluster,
        )
        .with_spec(serde_json::json!({
            "rules": [{
                "host": "app.example.com",
                "http": {
                    "paths": [{
                        "path": "/",
                        "backend": {
                            "service": { "name": "my-service", "port": { "number": 80 } }
                        }
                    }]
                }
            }]
        }));

        let svc =
            make_resource_with_uid("svc-uid", "my-service", "production", "Service", "v1", cluster)
                .with_spec(serde_json::json!({ "selector": { "app": "web" } }));

        let pod = make_resource_with_uid("pod-uid", "web-pod", "production", "Pod", "v1", cluster)
            .with_label("app", "web");

        state.set_resources(&[ingress, svc, pod]);

        // Verify state
        assert!(!state.loading);
        assert!(state.error.is_none());
        assert_eq!(state.resource_map.layout.nodes.len(), 3);
        assert_eq!(state.resource_map.layout.edges.len(), 2);

        // Select a node
        state.select_resource("Service/production/my-service");
        assert_eq!(
            state.resource_map.selected_node.as_deref(),
            Some("Service/production/my-service")
        );

        // Clear
        state.clear_selection();
        assert!(state.resource_map.selected_node.is_none());
    }
}
