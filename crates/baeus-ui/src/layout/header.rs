use gpui::{Context, ElementId, FontWeight, SharedString, Window, div, prelude::*, px, rgb};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClusterSelector {
    pub available_contexts: Vec<ClusterOption>,
    pub active_context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterOption {
    pub context_name: String,
    pub display_name: String,
    pub connected: bool,
    pub favorite: bool,
}

impl ClusterSelector {
    pub fn new() -> Self {
        Self { available_contexts: Vec::new(), active_context: None }
    }

    pub fn set_contexts(&mut self, contexts: Vec<ClusterOption>) {
        self.available_contexts = contexts;
    }

    pub fn select_context(&mut self, context_name: &str) -> bool {
        if self.available_contexts.iter().any(|c| c.context_name == context_name) {
            self.active_context = Some(context_name.to_string());
            true
        } else {
            false
        }
    }

    pub fn active_display_name(&self) -> Option<&str> {
        self.active_context.as_ref().and_then(|active| {
            self.available_contexts
                .iter()
                .find(|c| c.context_name == *active)
                .map(|c| c.display_name.as_str())
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceSelector {
    pub available_namespaces: Vec<String>,
    pub active_namespace: Option<String>,
    pub show_all: bool,
}

impl Default for NamespaceSelector {
    fn default() -> Self {
        Self::new()
    }
}

impl NamespaceSelector {
    pub fn new() -> Self {
        Self { available_namespaces: Vec::new(), active_namespace: None, show_all: true }
    }

    pub fn set_namespaces(&mut self, namespaces: Vec<String>) {
        self.available_namespaces = namespaces;
    }

    pub fn select_namespace(&mut self, namespace: &str) {
        self.active_namespace = Some(namespace.to_string());
        self.show_all = false;
    }

    pub fn select_all(&mut self) {
        self.active_namespace = None;
        self.show_all = true;
    }

    pub fn display_label(&self) -> &str {
        if self.show_all {
            "All Namespaces"
        } else {
            self.active_namespace.as_deref().unwrap_or("All Namespaces")
        }
    }
}

/// Action emitted when the user changes the active cluster or namespace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeaderAction {
    ClusterChanged { context_name: String },
    NamespaceChanged { namespace: Option<String> },
}

#[derive(Debug)]
pub struct HeaderState {
    pub cluster_selector: ClusterSelector,
    pub namespace_selector: NamespaceSelector,
    /// Pending action to be consumed by the parent layout/app.
    pub pending_action: Option<HeaderAction>,
}

impl Default for HeaderState {
    fn default() -> Self {
        Self {
            cluster_selector: ClusterSelector::new(),
            namespace_selector: NamespaceSelector::new(),
            pending_action: None,
        }
    }
}

impl HeaderState {
    /// Switch the active cluster and emit a ClusterChanged action.
    /// Also resets the namespace selector to "All Namespaces".
    pub fn switch_cluster(&mut self, context_name: &str) -> bool {
        if self.cluster_selector.select_context(context_name) {
            self.namespace_selector = NamespaceSelector::new();
            self.pending_action =
                Some(HeaderAction::ClusterChanged { context_name: context_name.to_string() });
            true
        } else {
            false
        }
    }

    /// Switch the active namespace and emit a NamespaceChanged action.
    pub fn switch_namespace(&mut self, namespace: &str) {
        self.namespace_selector.select_namespace(namespace);
        self.pending_action =
            Some(HeaderAction::NamespaceChanged { namespace: Some(namespace.to_string()) });
    }

    /// Switch to "All Namespaces" and emit a NamespaceChanged action.
    pub fn switch_to_all_namespaces(&mut self) {
        self.namespace_selector.select_all();
        self.pending_action = Some(HeaderAction::NamespaceChanged { namespace: None });
    }

    /// Consume and return the pending action, if any.
    pub fn take_action(&mut self) -> Option<HeaderAction> {
        self.pending_action.take()
    }

    /// Populate the cluster selector from a list of context info.
    pub fn set_clusters(&mut self, clusters: Vec<ClusterOption>) {
        self.cluster_selector.set_contexts(clusters);
    }

    /// Populate the namespace selector.
    pub fn set_namespaces(&mut self, namespaces: Vec<String>) {
        self.namespace_selector.set_namespaces(namespaces);
    }
}

// --- T088: Enhanced Multi-Namespace Selector ---

/// Enhanced namespace selector that supports multi-selection of namespaces
/// with dropdown toggle and rich display labels.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnhancedNamespaceSelector {
    pub selected_namespaces: Vec<String>,
    pub available_namespaces: Vec<String>,
    pub is_dropdown_open: bool,
    #[serde(default)]
    pub search_query: String,
}

impl EnhancedNamespaceSelector {
    /// Creates a new enhanced namespace selector (empty = all namespaces).
    pub fn new() -> Self {
        Self::default()
    }

    /// Toggle a namespace: add if not selected, remove if already selected.
    pub fn toggle_namespace(&mut self, namespace: &str) {
        if let Some(idx) = self.selected_namespaces.iter().position(|n| n == namespace) {
            self.selected_namespaces.remove(idx);
        } else {
            self.selected_namespaces.push(namespace.to_string());
        }
    }

    /// Check whether a specific namespace is currently selected.
    pub fn is_namespace_selected(&self, namespace: &str) -> bool {
        if self.selected_namespaces.is_empty() {
            true // empty means "all"
        } else {
            self.selected_namespaces.iter().any(|n| n == namespace)
        }
    }

    /// Returns the number of explicitly selected namespaces.
    pub fn selected_count(&self) -> usize {
        self.selected_namespaces.len()
    }

    /// Returns a display label summarizing the current selection .
    ///
    /// - Empty selection: "All Namespaces"
    /// - Single selection: "Namespace: <name>"
    /// - Multiple selections: "Namespaces: <name1>, <name2>, ..."
    pub fn display_label(&self) -> String {
        match self.selected_namespaces.len() {
            0 => "All Namespaces".to_string(),
            1 => format!("Namespace: {}", self.selected_namespaces[0]),
            _ => format!("Namespaces: {}", self.selected_namespaces.join(", ")),
        }
    }

    /// Toggle the dropdown open/closed.
    pub fn toggle_dropdown(&mut self) {
        self.is_dropdown_open = !self.is_dropdown_open;
    }

    /// Returns true if the given namespace passes the current filter.
    /// When no namespaces are explicitly selected (i.e., "all"), any
    /// namespace matches. Otherwise, only selected namespaces match.
    pub fn matches_namespace(&self, namespace: &str) -> bool {
        if self.selected_namespaces.is_empty() {
            true
        } else {
            self.selected_namespaces.iter().any(|n| n == namespace)
        }
    }

    /// Clears the selection, reverting to "All Namespaces".
    pub fn clear_selection(&mut self) {
        self.selected_namespaces.clear();
    }

    /// Return available namespaces filtered by the current search query.
    pub fn filtered_namespaces(&self) -> Vec<&str> {
        let q = self.search_query.to_lowercase();
        self.available_namespaces
            .iter()
            .filter(|ns| q.is_empty() || ns.to_lowercase().contains(&q))
            .map(|s| s.as_str())
            .collect()
    }

    /// Set the list of available namespaces.
    pub fn set_available_namespaces(&mut self, namespaces: Vec<String>) {
        self.available_namespaces = namespaces;
    }

    /// Select all available namespaces explicitly.
    pub fn select_all_available(&mut self) {
        self.selected_namespaces = self.available_namespaces.clone();
    }
}

/// View component for the enhanced namespace selector with render support.
pub struct NamespaceSelectorViewComponent {
    pub selector: EnhancedNamespaceSelector,
}

impl NamespaceSelectorViewComponent {
    pub fn new(selector: EnhancedNamespaceSelector) -> Self {
        Self { selector }
    }

    /// Render the selector button showing the display label.
    fn render_button(&self) -> gpui::Stateful<gpui::Div> {
        let label = SharedString::from(self.selector.display_label());
        let arrow = if self.selector.is_dropdown_open {
            SharedString::from("^")
        } else {
            SharedString::from("v")
        };

        let button_id = ElementId::Name(SharedString::from("ns-selector-btn"));

        div()
            .id(button_id)
            .flex()
            .items_center()
            .gap(px(4.0))
            .px_3()
            .py_1()
            .rounded(px(6.0))
            .bg(rgb(0x374151))
            .text_sm()
            .text_color(rgb(0xD1D5DB))
            .cursor_pointer()
            .hover(|s| s.bg(rgb(0x4B5563)))
            .child(label)
            .child(div().text_xs().text_color(rgb(0x9CA3AF)).child(arrow))
    }

    /// Render a single namespace checkbox row.
    fn render_namespace_row(&self, namespace: &str, idx: usize) -> gpui::Stateful<gpui::Div> {
        let is_selected = self.selector.is_namespace_selected(namespace);
        let check_mark =
            if is_selected { SharedString::from("[x]") } else { SharedString::from("[ ]") };
        let ns_label = SharedString::from(namespace.to_string());
        let row_id = ElementId::Name(SharedString::from(format!("ns-row-{idx}")));

        div()
            .id(row_id)
            .flex()
            .items_center()
            .gap(px(8.0))
            .w_full()
            .px_3()
            .py_1()
            .cursor_pointer()
            .hover(|s| s.bg(rgb(0x374151)))
            .child(div().text_xs().text_color(rgb(0x9CA3AF)).child(check_mark))
            .child(div().text_sm().text_color(rgb(0xD1D5DB)).child(ns_label))
    }

    /// Render the action buttons at the top of the dropdown.
    fn render_action_buttons(&self) -> gpui::Div {
        let select_all_id = ElementId::Name(SharedString::from("ns-select-all"));
        let clear_id = ElementId::Name(SharedString::from("ns-clear"));

        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .px_3()
            .py_1()
            .border_b_1()
            .border_color(rgb(0x4B5563))
            .child(
                div()
                    .id(select_all_id)
                    .text_xs()
                    .text_color(rgb(0x60A5FA))
                    .cursor_pointer()
                    .child("Select All"),
            )
            .child(
                div()
                    .id(clear_id)
                    .text_xs()
                    .text_color(rgb(0x60A5FA))
                    .cursor_pointer()
                    .child("Clear"),
            )
    }

    /// Render the full dropdown.
    fn render_dropdown(&self) -> gpui::Div {
        let mut dropdown = div()
            .flex()
            .flex_col()
            .w_full()
            .bg(rgb(0x1F2937))
            .border_1()
            .border_color(rgb(0x4B5563))
            .rounded(px(6.0))
            .mt_1()
            .overflow_hidden()
            .child(self.render_action_buttons());

        for (idx, ns) in self.selector.available_namespaces.iter().enumerate() {
            dropdown = dropdown.child(self.render_namespace_row(ns, idx));
        }

        dropdown
    }
}

impl Render for NamespaceSelectorViewComponent {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let mut container = div().flex().flex_col().child(self.render_button());

        if self.selector.is_dropdown_open {
            container = container.child(self.render_dropdown());
        }

        container
    }
}

// --- T135: Multi-Namespace Selector (legacy) ---

/// Allows the user to select multiple namespaces simultaneously
/// for combined resource list views across namespaces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiNamespaceSelector {
    pub selected_namespaces: BTreeSet<String>,
    pub available_namespaces: Vec<String>,
    pub all_selected: bool,
    pub expanded: bool,
}

impl Default for MultiNamespaceSelector {
    fn default() -> Self {
        Self {
            selected_namespaces: BTreeSet::new(),
            available_namespaces: Vec::new(),
            all_selected: true,
            expanded: false,
        }
    }
}

impl MultiNamespaceSelector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Toggle a namespace: add if not selected, remove if already selected.
    /// When toggling, `all_selected` is set to false unless every available
    /// namespace ends up selected.
    pub fn toggle_namespace(&mut self, namespace: &str) {
        if self.selected_namespaces.contains(namespace) {
            self.selected_namespaces.remove(namespace);
        } else {
            self.selected_namespaces.insert(namespace.to_string());
        }
        self.sync_all_selected();
    }

    /// Select all available namespaces.
    pub fn select_all(&mut self) {
        self.selected_namespaces = self.available_namespaces.iter().cloned().collect();
        self.all_selected = true;
    }

    /// Deselect all namespaces.
    pub fn deselect_all(&mut self) {
        self.selected_namespaces.clear();
        self.all_selected = false;
    }

    /// Set the list of available namespaces. Resets selection to all.
    pub fn set_available_namespaces(&mut self, namespaces: Vec<String>) {
        self.available_namespaces = namespaces;
        self.select_all();
    }

    /// Check whether a specific namespace is currently selected.
    pub fn is_selected(&self, namespace: &str) -> bool {
        self.all_selected || self.selected_namespaces.contains(namespace)
    }

    /// Return the number of currently selected namespaces.
    pub fn selected_count(&self) -> usize {
        if self.all_selected {
            self.available_namespaces.len()
        } else {
            self.selected_namespaces.len()
        }
    }

    /// Add a single namespace to the available list (does not auto-select).
    pub fn add_namespace(&mut self, namespace: String) {
        if !self.available_namespaces.contains(&namespace) {
            self.available_namespaces.push(namespace);
            // Adding a new namespace means all_selected may no longer be true
            self.sync_all_selected();
        }
    }

    /// Remove a namespace from both the available list and the selection.
    pub fn remove_namespace(&mut self, namespace: &str) {
        self.available_namespaces.retain(|n| n != namespace);
        self.selected_namespaces.remove(namespace);
        self.sync_all_selected();
    }

    /// Synchronize the `all_selected` flag based on current state.
    fn sync_all_selected(&mut self) {
        self.all_selected = !self.available_namespaces.is_empty()
            && self.available_namespaces.iter().all(|ns| self.selected_namespaces.contains(ns));
    }
}

// ---------------------------------------------------------------------------
// GPUI View
// ---------------------------------------------------------------------------

pub struct HeaderView {
    state: HeaderState,
}

impl Default for HeaderView {
    fn default() -> Self {
        Self::new()
    }
}

impl HeaderView {
    pub fn new() -> Self {
        Self { state: HeaderState::default() }
    }

    pub fn state(&self) -> &HeaderState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut HeaderState {
        &mut self.state
    }
}

impl Render for HeaderView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let cluster_name =
            self.state.cluster_selector.active_display_name().unwrap_or("No Cluster").to_string();

        let namespace_label = self.state.namespace_selector.display_label().to_string();

        div()
            .flex()
            .items_center()
            .w_full()
            .h_10()
            .px_4()
            .bg(rgb(0x1F2937))
            .border_b_1()
            .border_color(rgb(0x4B5563))
            // Left: cluster name
            .child(
                div()
                    .flex_1()
                    .text_sm()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xF9FAFB))
                    .child(SharedString::from(cluster_name)),
            )
            // Center: namespace display
            .child(
                div()
                    .flex_1()
                    .flex()
                    .justify_center()
                    .text_sm()
                    .text_color(rgb(0xD1D5DB))
                    .child(SharedString::from(namespace_label)),
            )
            // Right: search trigger
            .child(
                div().flex_1().flex().justify_end().child(
                    div()
                        .px_3()
                        .py_1()
                        .rounded(px(6.0))
                        .bg(rgb(0x374151))
                        .text_xs()
                        .text_color(rgb(0x9CA3AF))
                        .cursor_pointer()
                        .child("Search... (Cmd+K)"),
                ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cluster_selector_select_valid() {
        let mut selector = ClusterSelector::new();
        selector.set_contexts(vec![
            ClusterOption {
                context_name: "prod".to_string(),
                display_name: "Production".to_string(),
                connected: true,
                favorite: true,
            },
            ClusterOption {
                context_name: "dev".to_string(),
                display_name: "Development".to_string(),
                connected: false,
                favorite: false,
            },
        ]);

        assert!(selector.select_context("prod"));
        assert_eq!(selector.active_display_name(), Some("Production"));
    }

    #[test]
    fn test_cluster_selector_reject_invalid() {
        let mut selector = ClusterSelector::new();
        assert!(!selector.select_context("nonexistent"));
        assert!(selector.active_context.is_none());
    }

    #[test]
    fn test_namespace_selector_select_and_all() {
        let mut selector = NamespaceSelector::new();
        selector.set_namespaces(vec!["default".to_string(), "kube-system".to_string()]);

        assert_eq!(selector.display_label(), "All Namespaces");

        selector.select_namespace("kube-system");
        assert_eq!(selector.display_label(), "kube-system");
        assert!(!selector.show_all);

        selector.select_all();
        assert_eq!(selector.display_label(), "All Namespaces");
        assert!(selector.show_all);
    }

    #[test]
    fn test_header_state_default() {
        let header = HeaderState::default();
        assert!(header.cluster_selector.available_contexts.is_empty());
        assert!(header.namespace_selector.show_all);
        assert!(header.pending_action.is_none());
    }

    // --- T049: Cluster selector wiring ---

    fn setup_header_with_clusters() -> HeaderState {
        let mut header = HeaderState::default();
        header.set_clusters(vec![
            ClusterOption {
                context_name: "prod".to_string(),
                display_name: "Production".to_string(),
                connected: true,
                favorite: true,
            },
            ClusterOption {
                context_name: "dev".to_string(),
                display_name: "Development".to_string(),
                connected: true,
                favorite: false,
            },
        ]);
        header
    }

    #[test]
    fn test_switch_cluster_emits_action() {
        let mut header = setup_header_with_clusters();
        assert!(header.switch_cluster("prod"));

        let action = header.take_action().unwrap();
        assert_eq!(action, HeaderAction::ClusterChanged { context_name: "prod".to_string() });
    }

    #[test]
    fn test_switch_cluster_resets_namespace() {
        let mut header = setup_header_with_clusters();
        header.set_namespaces(vec!["default".to_string(), "kube-system".to_string()]);
        header.switch_namespace("kube-system");

        // Switch cluster should reset namespace
        header.switch_cluster("dev");
        assert!(header.namespace_selector.show_all);
        assert!(header.namespace_selector.active_namespace.is_none());
    }

    #[test]
    fn test_switch_cluster_invalid() {
        let mut header = setup_header_with_clusters();
        assert!(!header.switch_cluster("nonexistent"));
        assert!(header.take_action().is_none());
    }

    // --- T050: Namespace selector wiring ---

    #[test]
    fn test_switch_namespace_emits_action() {
        let mut header = setup_header_with_clusters();
        header.set_namespaces(vec!["default".to_string(), "kube-system".to_string()]);

        header.switch_namespace("kube-system");
        let action = header.take_action().unwrap();
        assert_eq!(
            action,
            HeaderAction::NamespaceChanged { namespace: Some("kube-system".to_string()) }
        );
    }

    #[test]
    fn test_switch_to_all_namespaces_emits_action() {
        let mut header = setup_header_with_clusters();
        header.switch_namespace("default");
        header.take_action(); // consume

        header.switch_to_all_namespaces();
        let action = header.take_action().unwrap();
        assert_eq!(action, HeaderAction::NamespaceChanged { namespace: None });
    }

    #[test]
    fn test_take_action_consumes() {
        let mut header = setup_header_with_clusters();
        header.switch_cluster("prod");
        assert!(header.take_action().is_some());
        assert!(header.take_action().is_none()); // consumed
    }

    // --- T135: Multi-Namespace Selector ---

    fn make_multi_ns_selector() -> MultiNamespaceSelector {
        let mut sel = MultiNamespaceSelector::new();
        sel.set_available_namespaces(vec![
            "default".to_string(),
            "kube-system".to_string(),
            "monitoring".to_string(),
        ]);
        sel
    }

    #[test]
    fn test_multi_ns_default() {
        let sel = MultiNamespaceSelector::new();
        assert!(sel.selected_namespaces.is_empty());
        assert!(sel.available_namespaces.is_empty());
        assert!(sel.all_selected);
        assert!(!sel.expanded);
    }

    #[test]
    fn test_multi_ns_set_available_selects_all() {
        let sel = make_multi_ns_selector();
        assert_eq!(sel.available_namespaces.len(), 3);
        assert!(sel.all_selected);
        assert_eq!(sel.selected_count(), 3);
        assert!(sel.is_selected("default"));
        assert!(sel.is_selected("kube-system"));
        assert!(sel.is_selected("monitoring"));
    }

    #[test]
    fn test_multi_ns_toggle_namespace_deselect() {
        let mut sel = make_multi_ns_selector();
        sel.toggle_namespace("kube-system");
        assert!(!sel.is_selected("kube-system"));
        assert!(sel.is_selected("default"));
        assert!(sel.is_selected("monitoring"));
        assert!(!sel.all_selected);
        assert_eq!(sel.selected_count(), 2);
    }

    #[test]
    fn test_multi_ns_toggle_namespace_reselect() {
        let mut sel = make_multi_ns_selector();
        sel.toggle_namespace("kube-system"); // deselect
        assert!(!sel.is_selected("kube-system"));
        sel.toggle_namespace("kube-system"); // re-select
        assert!(sel.is_selected("kube-system"));
        assert!(sel.all_selected);
        assert_eq!(sel.selected_count(), 3);
    }

    #[test]
    fn test_multi_ns_select_all() {
        let mut sel = make_multi_ns_selector();
        sel.deselect_all();
        assert_eq!(sel.selected_count(), 0);
        assert!(!sel.all_selected);

        sel.select_all();
        assert!(sel.all_selected);
        assert_eq!(sel.selected_count(), 3);
    }

    #[test]
    fn test_multi_ns_deselect_all() {
        let mut sel = make_multi_ns_selector();
        sel.deselect_all();
        assert!(!sel.all_selected);
        assert_eq!(sel.selected_count(), 0);
        assert!(!sel.is_selected("default"));
        assert!(!sel.is_selected("kube-system"));
        assert!(!sel.is_selected("monitoring"));
    }

    #[test]
    fn test_multi_ns_is_selected_when_all_selected() {
        let sel = make_multi_ns_selector();
        // When all_selected is true, any namespace is considered selected
        assert!(sel.is_selected("default"));
        assert!(sel.is_selected("kube-system"));
        assert!(sel.is_selected("monitoring"));
        // Even a namespace not in the available list is "selected" when all_selected
        assert!(sel.is_selected("nonexistent"));
    }

    #[test]
    fn test_multi_ns_selected_count_partial() {
        let mut sel = make_multi_ns_selector();
        sel.toggle_namespace("default"); // deselect
        sel.toggle_namespace("monitoring"); // deselect
        assert_eq!(sel.selected_count(), 1);
        assert!(sel.is_selected("kube-system"));
    }

    #[test]
    fn test_multi_ns_add_namespace() {
        let mut sel = make_multi_ns_selector();
        sel.add_namespace("production".to_string());
        assert_eq!(sel.available_namespaces.len(), 4);
        // After adding, all_selected is no longer true because "production" is not in selected_namespaces
        assert!(!sel.all_selected);
        assert!(!sel.is_selected("production"));
    }

    #[test]
    fn test_multi_ns_add_duplicate_namespace() {
        let mut sel = make_multi_ns_selector();
        sel.add_namespace("default".to_string());
        // Should not add duplicate
        assert_eq!(sel.available_namespaces.len(), 3);
        assert!(sel.all_selected); // unchanged
    }

    #[test]
    fn test_multi_ns_remove_namespace() {
        let mut sel = make_multi_ns_selector();
        sel.remove_namespace("monitoring");
        assert_eq!(sel.available_namespaces.len(), 2);
        assert!(!sel.selected_namespaces.contains("monitoring"));
        // Should still be all_selected since remaining available are all selected
        assert!(sel.all_selected);
        assert_eq!(sel.selected_count(), 2);
    }

    #[test]
    fn test_multi_ns_remove_nonexistent_namespace() {
        let mut sel = make_multi_ns_selector();
        sel.remove_namespace("nonexistent");
        assert_eq!(sel.available_namespaces.len(), 3);
        assert!(sel.all_selected);
    }

    #[test]
    fn test_multi_ns_serialization_roundtrip() {
        let sel = make_multi_ns_selector();
        let json = serde_json::to_string_pretty(&sel).unwrap();
        let deserialized: MultiNamespaceSelector = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.available_namespaces.len(), 3);
        assert_eq!(deserialized.selected_namespaces.len(), 3);
        assert!(deserialized.all_selected);
        assert!(!deserialized.expanded);
    }
}
