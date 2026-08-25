use gpui::{Context, ElementId, Rgba, SharedString, Window, div, prelude::*, px};
use std::collections::HashMap;

use crate::theme::Theme;

// ---------------------------------------------------------------------------
// T344: Details Panel State
// ---------------------------------------------------------------------------

/// An owner reference linking a resource to its controller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerReference {
    pub kind: String,
    pub name: String,
    pub uid: String,
}

/// A single condition on a Kubernetes resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceCondition {
    pub condition_type: String,
    /// "True", "False", or "Unknown"
    pub status: String,
    pub reason: String,
    pub message: String,
    pub last_transition: String,
}

/// Full metadata and status information for a single Kubernetes resource,
/// used by the details panel to render a rich side panel.
#[derive(Debug, Clone)]
pub struct ResourceInfo {
    pub name: String,
    pub namespace: Option<String>,
    pub kind: String,
    pub uid: String,
    pub creation_timestamp: String,
    pub resource_version: String,
    pub labels: HashMap<String, String>,
    pub annotations: HashMap<String, String>,
    pub owner_references: Vec<OwnerReference>,
    pub conditions: Vec<ResourceCondition>,
    pub raw_json: Option<serde_json::Value>,
}

impl ResourceInfo {
    /// Creates a minimal ResourceInfo with required fields; other fields are
    /// defaulted to empty.
    pub fn new(name: impl Into<String>, kind: impl Into<String>, uid: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            namespace: None,
            kind: kind.into(),
            uid: uid.into(),
            creation_timestamp: String::new(),
            resource_version: String::new(),
            labels: HashMap::new(),
            annotations: HashMap::new(),
            owner_references: Vec::new(),
            conditions: Vec::new(),
            raw_json: None,
        }
    }

    /// Builder-style setter for namespace.
    pub fn with_namespace(mut self, ns: impl Into<String>) -> Self {
        self.namespace = Some(ns.into());
        self
    }

    /// Builder-style setter for creation timestamp.
    pub fn with_creation_timestamp(mut self, ts: impl Into<String>) -> Self {
        self.creation_timestamp = ts.into();
        self
    }

    /// Builder-style setter for resource version.
    pub fn with_resource_version(mut self, rv: impl Into<String>) -> Self {
        self.resource_version = rv.into();
        self
    }

    /// Builder-style setter for labels.
    pub fn with_labels(mut self, labels: HashMap<String, String>) -> Self {
        self.labels = labels;
        self
    }

    /// Builder-style setter for annotations.
    pub fn with_annotations(mut self, annotations: HashMap<String, String>) -> Self {
        self.annotations = annotations;
        self
    }

    /// Builder-style setter for owner references.
    pub fn with_owner_references(mut self, refs: Vec<OwnerReference>) -> Self {
        self.owner_references = refs;
        self
    }

    /// Builder-style setter for conditions.
    pub fn with_conditions(mut self, conditions: Vec<ResourceCondition>) -> Self {
        self.conditions = conditions;
        self
    }

    /// Builder-style setter for raw JSON.
    pub fn with_raw_json(mut self, json: serde_json::Value) -> Self {
        self.raw_json = Some(json);
        self
    }
}

/// State for the sliding details panel that shows metadata, conditions,
/// and actions for a selected Kubernetes resource.
#[derive(Debug)]
pub struct DetailsPanelState {
    /// Whether the panel is currently visible.
    pub open: bool,
    /// The resource currently displayed, or None if the panel is empty.
    pub resource: Option<ResourceInfo>,
    /// Panel width in logical pixels (default 400).
    pub width: f32,
}

impl Default for DetailsPanelState {
    fn default() -> Self {
        Self { open: false, resource: None, width: 400.0 }
    }
}

impl DetailsPanelState {
    /// Creates a new closed details panel with default width.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new details panel with a custom width.
    pub fn with_width(width: f32) -> Self {
        Self { open: false, resource: None, width }
    }

    /// Opens the panel with the given resource info.
    pub fn open(&mut self, resource: ResourceInfo) {
        self.resource = Some(resource);
        self.open = true;
    }

    /// Closes the panel, keeping the last resource info intact.
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Toggles the panel open/closed. If toggling open with no resource,
    /// the panel opens but shows an empty state.
    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    /// Returns true if the panel is currently open.
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Returns a reference to the currently displayed resource, if any.
    pub fn current_resource(&self) -> Option<&ResourceInfo> {
        self.resource.as_ref()
    }

    /// Sets the panel width.
    pub fn set_width(&mut self, width: f32) {
        self.width = width;
    }

    /// Clears the resource and closes the panel.
    pub fn clear(&mut self) {
        self.resource = None;
        self.open = false;
    }
}

// ---------------------------------------------------------------------------
// T345: Render Details Panel
// ---------------------------------------------------------------------------

/// Precomputed colors for rendering the details panel.
struct PanelColors {
    background: Rgba,
    surface: Rgba,
    border: Rgba,
    accent: Rgba,
    success: Rgba,
    error: Rgba,
    warning: Rgba,
    text_primary: Rgba,
    text_secondary: Rgba,
    text_muted: Rgba,
}

/// View wrapper for `DetailsPanelState` that holds a theme for rendering.
pub struct DetailsPanelView {
    pub state: DetailsPanelState,
    pub theme: Theme,
}

impl DetailsPanelView {
    pub fn new(state: DetailsPanelState, theme: Theme) -> Self {
        Self { state, theme }
    }

    // --- Action bar ---

    /// Render the universal actions bar (Copy, Edit, Delete).
    fn render_actions_bar(&self, colors: &PanelColors) -> gpui::Div {
        div()
            .flex()
            .flex_row()
            .w_full()
            .px_3()
            .py_2()
            .gap(px(8.0))
            .border_b_1()
            .border_color(colors.border)
            .bg(colors.surface)
            .child(self.render_action_button("Copy Name", "action-copy", colors))
            .child(self.render_action_button("Edit", "action-edit", colors))
            .child(self.render_action_button("Delete", "action-delete", colors))
    }

    /// Render a single action button.
    fn render_action_button(
        &self,
        label: &str,
        id_str: &str,
        colors: &PanelColors,
    ) -> gpui::Stateful<gpui::Div> {
        let btn_id = ElementId::Name(SharedString::from(id_str.to_string()));
        let label_text = SharedString::from(label.to_string());

        div()
            .id(btn_id)
            .px_2()
            .py_1()
            .rounded(px(4.0))
            .border_1()
            .border_color(colors.border)
            .cursor_pointer()
            .text_xs()
            .text_color(colors.text_secondary)
            .child(label_text)
    }

    // --- Metadata section ---

    /// Render the metadata section: name, namespace, UID, creation timestamp,
    /// labels as tag chips, and annotations.
    fn render_metadata_section(&self, resource: &ResourceInfo, colors: &PanelColors) -> gpui::Div {
        let mut section = div()
            .flex()
            .flex_col()
            .w_full()
            .px_3()
            .py_3()
            .gap(px(6.0))
            .border_b_1()
            .border_color(colors.border);

        // Section header
        section = section.child(self.render_section_header("Metadata", colors));

        // Name
        section = section.child(self.render_metadata_row("Name", &resource.name, colors));

        // Namespace
        let ns_value = resource.namespace.as_deref().unwrap_or("(cluster-scoped)");
        section = section.child(self.render_metadata_row("Namespace", ns_value, colors));

        // Kind
        section = section.child(self.render_metadata_row("Kind", &resource.kind, colors));

        // UID
        section = section.child(self.render_metadata_row("UID", &resource.uid, colors));

        // Creation timestamp
        if !resource.creation_timestamp.is_empty() {
            section = section.child(self.render_metadata_row(
                "Created",
                &resource.creation_timestamp,
                colors,
            ));
        }

        // Resource version
        if !resource.resource_version.is_empty() {
            section = section.child(self.render_metadata_row(
                "Version",
                &resource.resource_version,
                colors,
            ));
        }

        // Labels as tag chips
        if !resource.labels.is_empty() {
            section = section.child(self.render_labels_section(&resource.labels, colors));
        }

        // Annotations
        if !resource.annotations.is_empty() {
            section = section.child(self.render_annotations_section(&resource.annotations, colors));
        }

        section
    }

    /// Render a section header.
    fn render_section_header(&self, title: &str, colors: &PanelColors) -> gpui::Div {
        div()
            .text_sm()
            .font_weight(gpui::FontWeight::BOLD)
            .text_color(colors.text_primary)
            .pb_1()
            .child(SharedString::from(title.to_string()))
    }

    /// Render a single metadata key-value row.
    fn render_metadata_row(&self, key: &str, value: &str, colors: &PanelColors) -> gpui::Div {
        let key_text = SharedString::from(format!("{key}:"));
        let value_text = SharedString::from(value.to_string());

        div()
            .flex()
            .flex_row()
            .gap(px(8.0))
            .child(
                div()
                    .text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(colors.text_secondary)
                    .w(px(80.0))
                    .child(key_text),
            )
            .child(div().flex_1().text_xs().text_color(colors.text_primary).child(value_text))
    }

    /// Render labels as tag chips.
    fn render_labels_section(
        &self,
        labels: &HashMap<String, String>,
        colors: &PanelColors,
    ) -> gpui::Div {
        let mut container = div().flex().flex_col().gap(px(4.0)).child(
            div()
                .text_xs()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(colors.text_secondary)
                .child("Labels:"),
        );

        let mut chips_row = div().flex().flex_row().flex_wrap().gap(px(4.0));

        let mut sorted_labels: Vec<_> = labels.iter().collect();
        sorted_labels.sort_by_key(|(k, _)| (*k).clone());

        for (key, value) in sorted_labels {
            let chip_text = SharedString::from(format!("{key}={value}"));
            chips_row = chips_row.child(
                div()
                    .px_2()
                    .py(px(2.0))
                    .rounded(px(4.0))
                    .bg(colors.surface)
                    .border_1()
                    .border_color(colors.border)
                    .text_xs()
                    .text_color(colors.accent)
                    .child(chip_text),
            );
        }

        container = container.child(chips_row);
        container
    }

    /// Render annotations as a collapsible list.
    fn render_annotations_section(
        &self,
        annotations: &HashMap<String, String>,
        colors: &PanelColors,
    ) -> gpui::Div {
        let mut container = div().flex().flex_col().gap(px(2.0)).child(
            div()
                .text_xs()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(colors.text_secondary)
                .child(SharedString::from(format!("Annotations ({})", annotations.len()))),
        );

        let mut sorted_annotations: Vec<_> = annotations.iter().collect();
        sorted_annotations.sort_by_key(|(k, _)| (*k).clone());

        for (key, value) in sorted_annotations {
            let annotation_text = SharedString::from(format!("{key}: {value}"));
            container = container
                .child(div().text_xs().text_color(colors.text_muted).pl_2().child(annotation_text));
        }

        container
    }

    // --- Conditions section ---

    /// Render the conditions section showing each condition as a row.
    fn render_conditions_section(
        &self,
        conditions: &[ResourceCondition],
        colors: &PanelColors,
    ) -> gpui::Div {
        let mut section = div()
            .flex()
            .flex_col()
            .w_full()
            .px_3()
            .py_3()
            .gap(px(4.0))
            .border_b_1()
            .border_color(colors.border);

        section = section.child(self.render_section_header("Conditions", colors));

        if conditions.is_empty() {
            section =
                section.child(div().text_xs().text_color(colors.text_muted).child("No conditions"));
        } else {
            for condition in conditions {
                section = section.child(self.render_condition_row(condition, colors));
            }
        }

        section
    }

    /// Render a single condition row.
    fn render_condition_row(
        &self,
        condition: &ResourceCondition,
        colors: &PanelColors,
    ) -> gpui::Div {
        let status_color = match condition.status.as_str() {
            "True" => colors.success,
            "False" => colors.error,
            _ => colors.warning,
        };

        let type_text = SharedString::from(condition.condition_type.clone());
        let status_text = SharedString::from(condition.status.clone());
        let reason_text = SharedString::from(condition.reason.clone());

        let mut row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .py_1()
            .child(div().text_xs().text_color(colors.text_primary).w(px(100.0)).child(type_text))
            .child(
                div()
                    .text_xs()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(status_color)
                    .w(px(60.0))
                    .child(status_text),
            )
            .child(div().flex_1().text_xs().text_color(colors.text_muted).child(reason_text));

        // Show message as a tooltip-like sub-row if non-empty
        if !condition.message.is_empty() {
            let msg = SharedString::from(condition.message.clone());
            row = row.child(div().text_xs().text_color(colors.text_muted).child(msg));
        }

        row
    }

    // --- Owner references section ---

    /// Render the owner references section with clickable links.
    fn render_owner_references_section(
        &self,
        owner_refs: &[OwnerReference],
        colors: &PanelColors,
    ) -> gpui::Div {
        let mut section = div()
            .flex()
            .flex_col()
            .w_full()
            .px_3()
            .py_3()
            .gap(px(4.0))
            .border_b_1()
            .border_color(colors.border);

        section = section.child(self.render_section_header("Owner References", colors));

        if owner_refs.is_empty() {
            section = section
                .child(div().text_xs().text_color(colors.text_muted).child("No owner references"));
        } else {
            for (idx, owner) in owner_refs.iter().enumerate() {
                section = section.child(self.render_owner_reference(owner, idx, colors));
            }
        }

        section
    }

    /// Render a single owner reference as a clickable link.
    fn render_owner_reference(
        &self,
        owner: &OwnerReference,
        idx: usize,
        colors: &PanelColors,
    ) -> gpui::Stateful<gpui::Div> {
        let link_text = SharedString::from(format!("{}/{}", owner.kind, owner.name));
        let link_id = ElementId::Name(SharedString::from(format!("owner-ref-{idx}")));

        div()
            .id(link_id)
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.0))
            .cursor_pointer()
            .child(div().text_xs().text_color(colors.accent).child(link_text))
    }

    // --- Panel header ---

    /// Render the panel header with resource kind/name and close button.
    fn render_panel_header(&self, resource: &ResourceInfo, colors: &PanelColors) -> gpui::Div {
        let title = SharedString::from(format!("{}/{}", resource.kind, resource.name));

        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .w_full()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(colors.border)
            .bg(colors.surface)
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(colors.text_primary)
                    .child(title),
            )
            .child(
                div()
                    .id(ElementId::Name(SharedString::from("details-close")))
                    .cursor_pointer()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child("X"),
            )
    }

    // --- Empty state ---

    /// Render the empty state when no resource is selected.
    fn render_empty_state(&self, colors: &PanelColors) -> gpui::Div {
        div()
            .flex()
            .items_center()
            .justify_center()
            .h_full()
            .text_sm()
            .text_color(colors.text_muted)
            .child("Select a resource to view details")
    }
}

impl Render for DetailsPanelView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let colors = PanelColors {
            background: self.theme.colors.background.to_gpui(),
            surface: self.theme.colors.surface.to_gpui(),
            border: self.theme.colors.border.to_gpui(),
            accent: self.theme.colors.accent.to_gpui(),
            success: self.theme.colors.success.to_gpui(),
            error: self.theme.colors.error.to_gpui(),
            warning: self.theme.colors.warning.to_gpui(),
            text_primary: self.theme.colors.text_primary.to_gpui(),
            text_secondary: self.theme.colors.text_secondary.to_gpui(),
            text_muted: self.theme.colors.text_muted.to_gpui(),
        };

        if !self.state.is_open() {
            // When closed, render a zero-width element
            return div();
        }

        let mut panel = div()
            .flex()
            .flex_col()
            .w(px(self.state.width))
            .h_full()
            .bg(colors.background)
            .border_l_1()
            .border_color(colors.border);

        match &self.state.resource {
            Some(resource) => {
                let resource = resource.clone();

                panel = panel
                    .child(self.render_panel_header(&resource, &colors))
                    .child(self.render_actions_bar(&colors))
                    .child(self.render_metadata_section(&resource, &colors));

                if !resource.conditions.is_empty() {
                    panel =
                        panel.child(self.render_conditions_section(&resource.conditions, &colors));
                }

                if !resource.owner_references.is_empty() {
                    panel = panel.child(
                        self.render_owner_references_section(&resource.owner_references, &colors),
                    );
                }
            }
            None => {
                panel = panel.child(self.render_empty_state(&colors));
            }
        }

        panel
    }
}

// ---------------------------------------------------------------------------
// T344/T345: Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_resource() -> ResourceInfo {
        let mut labels = HashMap::new();
        labels.insert("app".to_string(), "nginx".to_string());
        labels.insert("env".to_string(), "production".to_string());

        let mut annotations = HashMap::new();
        annotations
            .insert("kubernetes.io/change-cause".to_string(), "initial deployment".to_string());

        ResourceInfo::new("nginx-abc123", "Pod", "f47ac10b-58cc-4372-a567-0e02b2c3d479")
            .with_namespace("default")
            .with_creation_timestamp("2026-02-20T10:30:00Z")
            .with_resource_version("12345")
            .with_labels(labels)
            .with_annotations(annotations)
            .with_owner_references(vec![OwnerReference {
                kind: "ReplicaSet".to_string(),
                name: "nginx-abc123-rs".to_string(),
                uid: "a1b2c3d4-e5f6-7890-abcd-ef1234567890".to_string(),
            }])
            .with_conditions(vec![ResourceCondition {
                condition_type: "Ready".to_string(),
                status: "True".to_string(),
                reason: "PodReady".to_string(),
                message: "All containers are ready".to_string(),
                last_transition: "2026-02-20T10:31:00Z".to_string(),
            }])
    }

    // --- DetailsPanelState tests ---

    #[test]
    fn test_default_state_is_closed() {
        let state = DetailsPanelState::new();
        assert!(!state.is_open());
        assert!(state.resource.is_none());
        assert_eq!(state.width, 400.0);
    }

    #[test]
    fn test_with_custom_width() {
        let state = DetailsPanelState::with_width(500.0);
        assert_eq!(state.width, 500.0);
        assert!(!state.is_open());
    }

    #[test]
    fn test_open_sets_resource_and_opens() {
        let mut state = DetailsPanelState::new();
        state.open(sample_resource());
        assert!(state.is_open());
        assert!(state.resource.is_some());
        assert_eq!(state.resource.as_ref().unwrap().name, "nginx-abc123");
    }

    #[test]
    fn test_close_keeps_resource() {
        let mut state = DetailsPanelState::new();
        state.open(sample_resource());
        state.close();
        assert!(!state.is_open());
        assert!(state.resource.is_some());
    }

    #[test]
    fn test_toggle_open_close() {
        let mut state = DetailsPanelState::new();
        assert!(!state.is_open());

        state.toggle();
        assert!(state.is_open());

        state.toggle();
        assert!(!state.is_open());

        state.toggle();
        assert!(state.is_open());
    }

    #[test]
    fn test_clear_removes_resource_and_closes() {
        let mut state = DetailsPanelState::new();
        state.open(sample_resource());
        state.clear();
        assert!(!state.is_open());
        assert!(state.resource.is_none());
    }

    #[test]
    fn test_current_resource() {
        let mut state = DetailsPanelState::new();
        assert!(state.current_resource().is_none());

        state.open(sample_resource());
        let r = state.current_resource().unwrap();
        assert_eq!(r.kind, "Pod");
        assert_eq!(r.name, "nginx-abc123");
    }

    #[test]
    fn test_set_width() {
        let mut state = DetailsPanelState::new();
        state.set_width(600.0);
        assert_eq!(state.width, 600.0);
    }

    #[test]
    fn test_open_replaces_previous_resource() {
        let mut state = DetailsPanelState::new();
        state.open(sample_resource());
        assert_eq!(state.resource.as_ref().unwrap().name, "nginx-abc123");

        let new_resource = ResourceInfo::new("api-server", "Deployment", "uid-2");
        state.open(new_resource);
        assert_eq!(state.resource.as_ref().unwrap().name, "api-server");
        assert_eq!(state.resource.as_ref().unwrap().kind, "Deployment");
    }

    // --- ResourceInfo tests ---

    #[test]
    fn test_resource_info_builder() {
        let r = ResourceInfo::new("test", "Service", "uid-1")
            .with_namespace("kube-system")
            .with_creation_timestamp("2026-01-01T00:00:00Z")
            .with_resource_version("999");

        assert_eq!(r.name, "test");
        assert_eq!(r.kind, "Service");
        assert_eq!(r.uid, "uid-1");
        assert_eq!(r.namespace.as_deref(), Some("kube-system"));
        assert_eq!(r.creation_timestamp, "2026-01-01T00:00:00Z");
        assert_eq!(r.resource_version, "999");
    }

    #[test]
    fn test_resource_info_defaults() {
        let r = ResourceInfo::new("test", "Pod", "uid-1");
        assert!(r.namespace.is_none());
        assert!(r.creation_timestamp.is_empty());
        assert!(r.resource_version.is_empty());
        assert!(r.labels.is_empty());
        assert!(r.annotations.is_empty());
        assert!(r.owner_references.is_empty());
        assert!(r.conditions.is_empty());
        assert!(r.raw_json.is_none());
    }

    #[test]
    fn test_resource_info_with_raw_json() {
        let json = serde_json::json!({"kind": "Pod", "metadata": {"name": "test"}});
        let r = ResourceInfo::new("test", "Pod", "uid-1").with_raw_json(json.clone());
        assert_eq!(r.raw_json.unwrap(), json);
    }

    #[test]
    fn test_resource_info_with_labels() {
        let mut labels = HashMap::new();
        labels.insert("app".to_string(), "web".to_string());
        let r = ResourceInfo::new("test", "Pod", "uid-1").with_labels(labels);
        assert_eq!(r.labels.len(), 1);
        assert_eq!(r.labels.get("app").unwrap(), "web");
    }

    #[test]
    fn test_owner_reference_fields() {
        let owner = OwnerReference {
            kind: "ReplicaSet".to_string(),
            name: "nginx-rs".to_string(),
            uid: "owner-uid-1".to_string(),
        };
        assert_eq!(owner.kind, "ReplicaSet");
        assert_eq!(owner.name, "nginx-rs");
        assert_eq!(owner.uid, "owner-uid-1");
    }

    #[test]
    fn test_resource_condition_fields() {
        let cond = ResourceCondition {
            condition_type: "Ready".to_string(),
            status: "True".to_string(),
            reason: "PodReady".to_string(),
            message: "All containers started".to_string(),
            last_transition: "2026-02-20T10:31:00Z".to_string(),
        };
        assert_eq!(cond.condition_type, "Ready");
        assert_eq!(cond.status, "True");
        assert_eq!(cond.reason, "PodReady");
        assert_eq!(cond.message, "All containers started");
        assert_eq!(cond.last_transition, "2026-02-20T10:31:00Z");
    }

    #[test]
    fn test_condition_status_values() {
        for status in ["True", "False", "Unknown"] {
            let cond = ResourceCondition {
                condition_type: "Test".to_string(),
                status: status.to_string(),
                reason: String::new(),
                message: String::new(),
                last_transition: String::new(),
            };
            assert_eq!(cond.status, status);
        }
    }

    #[test]
    fn test_sample_resource_has_all_fields() {
        let r = sample_resource();
        assert_eq!(r.name, "nginx-abc123");
        assert_eq!(r.namespace.as_deref(), Some("default"));
        assert_eq!(r.kind, "Pod");
        assert_eq!(r.uid, "f47ac10b-58cc-4372-a567-0e02b2c3d479");
        assert_eq!(r.creation_timestamp, "2026-02-20T10:30:00Z");
        assert_eq!(r.resource_version, "12345");
        assert_eq!(r.labels.len(), 2);
        assert_eq!(r.annotations.len(), 1);
        assert_eq!(r.owner_references.len(), 1);
        assert_eq!(r.conditions.len(), 1);
    }

    #[test]
    fn test_panel_open_close_sequence() {
        let mut state = DetailsPanelState::new();

        // Start closed
        assert!(!state.is_open());

        // Open with resource
        state.open(sample_resource());
        assert!(state.is_open());
        assert_eq!(state.current_resource().unwrap().name, "nginx-abc123");

        // Close
        state.close();
        assert!(!state.is_open());
        // Resource is preserved
        assert_eq!(state.current_resource().unwrap().name, "nginx-abc123");

        // Toggle open
        state.toggle();
        assert!(state.is_open());

        // Clear
        state.clear();
        assert!(!state.is_open());
        assert!(state.current_resource().is_none());
    }
}
