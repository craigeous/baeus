use baeus_core::crd::{CrdSchema, CrdScope};
use gpui::prelude::FluentBuilder as _;
use gpui::{ElementId, FontWeight, Rgba, SharedString, div, prelude::*, px};
use std::collections::HashMap;

use crate::theme::Theme;

/// State for the CRD browser view.
///
/// Manages the list of discovered CRDs, group-based filtering,
/// CRD selection, and loading/error states.
#[derive(Debug, Default)]
pub struct CrdBrowserState {
    pub crds: Vec<CrdSchema>,
    pub selected_group: Option<String>,
    pub selected_crd: Option<String>,
    pub loading: bool,
    pub error: Option<String>,
}

impl CrdBrowserState {
    /// Replace the CRDs list with a new set. Clears loading and error states.
    pub fn set_crds(&mut self, crds: Vec<CrdSchema>) {
        self.crds = crds;
        self.loading = false;
        self.error = None;
    }

    /// Set the loading state. Clears any previous error when loading starts.
    pub fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
        if loading {
            self.error = None;
        }
    }

    /// Set an error message. Clears the loading state.
    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
        self.loading = false;
    }

    /// Clear the current error.
    pub fn clear_error(&mut self) {
        self.error = None;
    }

    /// Filter displayed CRDs by API group.
    /// Pass `None` to show CRDs from all groups.
    pub fn filter_by_group(&mut self, group: Option<String>) {
        self.selected_group = group;
        // Clear CRD selection when group filter changes, since the
        // previously selected CRD may no longer be visible.
        self.selected_crd = None;
    }

    /// Select a CRD by its full name (e.g., "certificates.cert-manager.io").
    pub fn select_crd(&mut self, name: &str) {
        self.selected_crd = Some(name.to_string());
    }

    /// Clear the current CRD selection.
    pub fn clear_selection(&mut self) {
        self.selected_crd = None;
    }

    /// Returns CRDs grouped by their API group.
    pub fn grouped_crds(&self) -> HashMap<String, Vec<&CrdSchema>> {
        let mut groups: HashMap<String, Vec<&CrdSchema>> = HashMap::new();
        for crd in &self.crds {
            groups.entry(crd.group.clone()).or_default().push(crd);
        }
        groups
    }

    /// Returns the CRDs filtered by the current group filter.
    pub fn filtered_crds(&self) -> Vec<&CrdSchema> {
        match &self.selected_group {
            Some(group) => self.crds.iter().filter(|c| c.group == *group).collect(),
            None => self.crds.iter().collect(),
        }
    }

    /// Returns a reference to the currently selected CRD, if any.
    pub fn selected(&self) -> Option<&CrdSchema> {
        self.selected_crd.as_ref().and_then(|name| self.crds.iter().find(|c| c.name == *name))
    }

    /// Returns the distinct API groups present in the CRDs list, sorted.
    pub fn api_groups(&self) -> Vec<&str> {
        let mut groups: Vec<&str> = self.crds.iter().map(|c| c.group.as_str()).collect();
        groups.sort();
        groups.dedup();
        groups
    }

    /// Total number of CRDs loaded.
    pub fn total_count(&self) -> usize {
        self.crds.len()
    }

    /// Number of CRDs matching the current filter.
    pub fn filtered_count(&self) -> usize {
        self.filtered_crds().len()
    }

    /// Count of namespaced CRDs in the filtered set.
    pub fn namespaced_count(&self) -> usize {
        self.filtered_crds().iter().filter(|c| c.scope == CrdScope::Namespaced).count()
    }

    /// Count of cluster-scoped CRDs in the filtered set.
    pub fn cluster_scoped_count(&self) -> usize {
        self.filtered_crds().iter().filter(|c| c.scope == CrdScope::Cluster).count()
    }
}

// ---------------------------------------------------------------------------
// GPUI Render (T075)
// ---------------------------------------------------------------------------

/// Precomputed colors for rendering the CRD browser view.
#[allow(dead_code)]
struct CrdBrowserColors {
    background: Rgba,
    surface: Rgba,
    border: Rgba,
    accent: Rgba,
    success: Rgba,
    warning: Rgba,
    error: Rgba,
    text_primary: Rgba,
    text_secondary: Rgba,
    text_muted: Rgba,
    selection: Rgba,
}

/// View wrapper for `CrdBrowserState` with theme for rendering.
pub struct CrdBrowserViewComponent {
    pub state: CrdBrowserState,
    pub theme: Theme,
}

impl CrdBrowserViewComponent {
    pub fn new(state: CrdBrowserState, theme: Theme) -> Self {
        Self { state, theme }
    }

    /// Toolbar: API group filter dropdown + count badges (total/namespaced/cluster).
    fn render_toolbar(&self, colors: &CrdBrowserColors) -> gpui::Div {
        let total = self.state.total_count();
        let namespaced = self.state.namespaced_count();
        let cluster = self.state.cluster_scoped_count();

        let total_lbl = SharedString::from(format!("Total: {total}"));
        let ns_lbl = SharedString::from(format!("Namespaced: {namespaced}"));
        let cluster_lbl = SharedString::from(format!("Cluster: {cluster}"));

        let filter_lbl = match &self.state.selected_group {
            Some(group) => SharedString::from(format!("Group: {group}")),
            None => SharedString::from("All Groups"),
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
            .child(
                div()
                    .id("group-filter")
                    .px_3()
                    .py_1()
                    .rounded(px(4.0))
                    .bg(colors.surface)
                    .border_1()
                    .border_color(colors.border)
                    .cursor_pointer()
                    .text_sm()
                    .text_color(colors.text_primary)
                    .child(filter_lbl),
            )
            .child(div().flex_1())
            .child(self.render_count_badge(total_lbl, colors.accent, colors))
            .child(self.render_count_badge(ns_lbl, colors.success, colors))
            .child(self.render_count_badge(cluster_lbl, colors.warning, colors))
    }

    /// Small count badge.
    fn render_count_badge(
        &self,
        label: SharedString,
        dot_color: Rgba,
        colors: &CrdBrowserColors,
    ) -> gpui::Div {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .px_2()
            .py_1()
            .rounded(px(4.0))
            .bg(colors.surface)
            .child(div().w(px(8.0)).h(px(8.0)).rounded(px(4.0)).bg(dot_color))
            .child(div().text_xs().text_color(colors.text_primary).child(label))
    }

    /// CRD list: left panel with CRD entries grouped by API group.
    fn render_crd_list(&self, colors: &CrdBrowserColors) -> gpui::Div {
        let grouped = self.state.grouped_crds();
        let filtered = self.state.filtered_crds();

        if filtered.is_empty() && !self.state.loading {
            return self.render_empty_state(colors);
        }

        let mut list = div().flex().flex_col().flex_1().w_full().overflow_hidden();

        // Get groups in sorted order
        let mut groups: Vec<&str> = grouped.keys().map(|s| s.as_str()).collect();
        groups.sort();

        // Filter groups based on selected_group
        if let Some(ref selected) = self.state.selected_group {
            groups.retain(|g| *g == selected.as_str());
        }

        for group in groups {
            if let Some(crds) = grouped.get(group) {
                list = list.child(self.render_crd_group(group, crds, colors));
            }
        }

        list
    }

    /// Group header with count, expandable.
    fn render_crd_group(
        &self,
        group: &str,
        crds: &[&CrdSchema],
        colors: &CrdBrowserColors,
    ) -> gpui::Div {
        let count = crds.len();
        let header_lbl = SharedString::from(format!("{group} ({count})"));

        let mut group_div =
            div().flex().flex_col().w_full().border_b_1().border_color(colors.border);

        group_div = group_div.child(
            div().flex().flex_row().items_center().w_full().px_3().py_2().bg(colors.surface).child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors.text_primary)
                    .child(header_lbl),
            ),
        );

        // Add CRD entries
        for crd in crds {
            group_div = group_div.child(self.render_crd_entry(crd, colors));
        }

        group_div
    }

    /// CRD row with kind, version(s), scope badge.
    fn render_crd_entry(
        &self,
        crd: &CrdSchema,
        colors: &CrdBrowserColors,
    ) -> gpui::Stateful<gpui::Div> {
        let selected = self.state.selected_crd.as_ref().map(|s| s == &crd.name).unwrap_or(false);

        let bg = if selected { colors.selection } else { colors.background };

        let versions_str = crd.versions.join(", ");
        let scope_str = match crd.scope {
            CrdScope::Namespaced => "Namespaced",
            CrdScope::Cluster => "Cluster",
        };
        let scope_color = match crd.scope {
            CrdScope::Namespaced => colors.success,
            CrdScope::Cluster => colors.warning,
        };

        let rid = format!("crd-{}", crd.name);

        div()
            .id(ElementId::Name(SharedString::from(rid)))
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .px_4()
            .py_2()
            .cursor_pointer()
            .bg(bg)
            .border_b_1()
            .border_color(colors.border)
            .when(selected, |el| el.border_l_2().border_color(colors.accent))
            .child(
                div()
                    .flex_1()
                    .text_sm()
                    .text_color(colors.text_primary)
                    .child(SharedString::from(crd.kind.clone())),
            )
            .child(
                div()
                    .flex_1()
                    .text_xs()
                    .text_color(colors.text_secondary)
                    .child(SharedString::from(versions_str)),
            )
            .child(self.render_scope_badge(scope_str, scope_color, colors))
    }

    /// Scope badge: colored label.
    fn render_scope_badge(&self, label: &str, color: Rgba, colors: &CrdBrowserColors) -> gpui::Div {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.0))
            .px_2()
            .py_1()
            .rounded(px(3.0))
            .bg(colors.surface)
            .border_1()
            .border_color(color)
            .child(div().text_xs().text_color(color).child(SharedString::from(label.to_string())))
    }

    /// Detail panel: right panel showing selected CRD details.
    fn render_detail_panel(&self, colors: &CrdBrowserColors) -> gpui::Div {
        if let Some(crd) = self.state.selected() {
            let name_lbl = SharedString::from(format!("Name: {}", crd.name));
            let group_lbl = SharedString::from(format!("Group: {}", crd.group));
            let kind_lbl = SharedString::from(format!("Kind: {}", crd.kind));
            let versions_lbl = SharedString::from(format!("Versions: {}", crd.versions.join(", ")));
            let scope_lbl = SharedString::from(format!(
                "Scope: {}",
                match crd.scope {
                    CrdScope::Namespaced => "Namespaced",
                    CrdScope::Cluster => "Cluster",
                }
            ));

            let mut detail = div()
                .flex()
                .flex_col()
                .flex_1()
                .w_full()
                .px_4()
                .py_3()
                .gap(px(12.0))
                .overflow_hidden();

            detail = detail
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(colors.text_primary)
                        .child("CRD Details"),
                )
                .child(self.render_detail_row(name_lbl, colors))
                .child(self.render_detail_row(group_lbl, colors))
                .child(self.render_detail_row(kind_lbl, colors))
                .child(self.render_detail_row(versions_lbl, colors))
                .child(self.render_detail_row(scope_lbl, colors));

            if let Some(ref desc) = crd.description {
                let desc_lbl = SharedString::from(format!("Description: {desc}"));
                detail = detail.child(self.render_detail_row(desc_lbl, colors));
            }

            detail
        } else {
            div().flex().flex_col().flex_1().items_center().justify_center().child(
                div().text_sm().text_color(colors.text_muted).child("Select a CRD to view details"),
            )
        }
    }

    /// Single detail row.
    fn render_detail_row(&self, label: SharedString, colors: &CrdBrowserColors) -> gpui::Div {
        div().text_sm().text_color(colors.text_secondary).child(label)
    }

    /// Empty state when no CRDs present.
    fn render_empty_state(&self, colors: &CrdBrowserColors) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .items_center()
            .justify_center()
            .child(div().text_sm().text_color(colors.text_muted).child("No CRDs found"))
    }

    /// Loading indicator.
    fn render_loading(&self, colors: &CrdBrowserColors) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .items_center()
            .justify_center()
            .child(div().text_sm().text_color(colors.text_muted).child("Loading CRDs..."))
    }

    /// Error message display.
    fn render_error(&self, colors: &CrdBrowserColors) -> gpui::Div {
        let msg = self.state.error.as_deref().unwrap_or("Unknown error");
        div().flex().flex_col().flex_1().items_center().justify_center().px_4().child(
            div().text_sm().text_color(colors.error).child(SharedString::from(msg.to_string())),
        )
    }
}

impl Render for CrdBrowserViewComponent {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let colors = CrdBrowserColors {
            background: self.theme.colors.background.to_gpui(),
            surface: self.theme.colors.surface.to_gpui(),
            border: self.theme.colors.border.to_gpui(),
            accent: self.theme.colors.accent.to_gpui(),
            success: self.theme.colors.success.to_gpui(),
            warning: self.theme.colors.warning.to_gpui(),
            error: self.theme.colors.error.to_gpui(),
            text_primary: self.theme.colors.text_primary.to_gpui(),
            text_secondary: self.theme.colors.text_secondary.to_gpui(),
            text_muted: self.theme.colors.text_muted.to_gpui(),
            selection: self.theme.colors.selection.to_gpui(),
        };

        let mut root = div().flex().flex_col().size_full().bg(colors.background);

        root = root.child(self.render_toolbar(&colors));

        if self.state.loading {
            root = root.child(self.render_loading(&colors));
        } else if self.state.error.is_some() {
            root = root.child(self.render_error(&colors));
        } else {
            // Two-panel layout: CRD list on left, detail on right
            let content = div()
                .flex()
                .flex_row()
                .flex_1()
                .w_full()
                .overflow_hidden()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .w(px(400.0))
                        .border_r_1()
                        .border_color(colors.border)
                        .child(self.render_crd_list(&colors)),
                )
                .child(self.render_detail_panel(&colors));

            root = root.child(content);
        }

        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn sample_crd(name: &str, group: &str, kind: &str, scope: CrdScope) -> CrdSchema {
        CrdSchema {
            name: format!("{}.{group}", name.to_lowercase()),
            group: group.to_string(),
            kind: kind.to_string(),
            versions: vec!["v1".to_string()],
            scope,
            description: None,
            schema_properties: None,
            schema: None,
            cluster_id: Uuid::new_v4(),
        }
    }

    fn sample_crds() -> Vec<CrdSchema> {
        vec![
            sample_crd("certificates", "cert-manager.io", "Certificate", CrdScope::Namespaced),
            sample_crd("issuers", "cert-manager.io", "Issuer", CrdScope::Namespaced),
            sample_crd("clusterissuers", "cert-manager.io", "ClusterIssuer", CrdScope::Cluster),
            sample_crd("virtualmachines", "kubevirt.io", "VirtualMachine", CrdScope::Namespaced),
            sample_crd(
                "ingressroutes",
                "traefik.containo.us",
                "IngressRoute",
                CrdScope::Namespaced,
            ),
        ]
    }

    // --- Default state tests ---

    #[test]
    fn test_default_state() {
        let state = CrdBrowserState::default();
        assert!(state.crds.is_empty());
        assert!(state.selected_group.is_none());
        assert!(state.selected_crd.is_none());
        assert!(!state.loading);
        assert!(state.error.is_none());
    }

    // --- set_crds tests ---

    #[test]
    fn test_set_crds() {
        let mut state = CrdBrowserState {
            loading: true,
            error: Some("old error".to_string()),
            ..Default::default()
        };

        state.set_crds(sample_crds());

        assert_eq!(state.crds.len(), 5);
        assert!(!state.loading);
        assert!(state.error.is_none());
    }

    #[test]
    fn test_set_crds_replaces_existing() {
        let mut state = CrdBrowserState::default();
        state.set_crds(sample_crds());
        assert_eq!(state.crds.len(), 5);

        state.set_crds(vec![sample_crd("single", "test.io", "Single", CrdScope::Namespaced)]);
        assert_eq!(state.crds.len(), 1);
    }

    // --- Loading and error state tests ---

    #[test]
    fn test_set_loading() {
        let mut state =
            CrdBrowserState { error: Some("some error".to_string()), ..Default::default() };

        state.set_loading(true);
        assert!(state.loading);
        assert!(state.error.is_none());

        state.set_loading(false);
        assert!(!state.loading);
    }

    #[test]
    fn test_set_loading_false_does_not_clear_error() {
        let mut state = CrdBrowserState::default();
        state.set_error("some error".to_string());

        state.set_loading(false);
        assert!(!state.loading);
        assert!(state.error.is_some());
    }

    #[test]
    fn test_set_error() {
        let mut state = CrdBrowserState { loading: true, ..Default::default() };

        state.set_error("connection refused".to_string());
        assert_eq!(state.error.as_deref(), Some("connection refused"));
        assert!(!state.loading);
    }

    #[test]
    fn test_clear_error() {
        let mut state = CrdBrowserState::default();
        state.set_error("timeout".to_string());
        assert!(state.error.is_some());

        state.clear_error();
        assert!(state.error.is_none());
    }

    // --- filter_by_group tests ---

    #[test]
    fn test_filter_by_group() {
        let mut state = CrdBrowserState::default();
        state.set_crds(sample_crds());
        state.select_crd("certificates.cert-manager.io");
        assert!(state.selected_crd.is_some());

        state.filter_by_group(Some("cert-manager.io".to_string()));
        assert_eq!(state.selected_group.as_deref(), Some("cert-manager.io"));
        // Selection cleared when group filter changes
        assert!(state.selected_crd.is_none());

        let filtered = state.filtered_crds();
        assert_eq!(filtered.len(), 3);
        assert!(filtered.iter().all(|c| c.group == "cert-manager.io"));
    }

    #[test]
    fn test_filter_by_group_none_shows_all() {
        let mut state = CrdBrowserState::default();
        state.set_crds(sample_crds());
        state.filter_by_group(Some("cert-manager.io".to_string()));
        assert_eq!(state.filtered_crds().len(), 3);

        state.filter_by_group(None);
        assert_eq!(state.filtered_crds().len(), 5);
    }

    #[test]
    fn test_filter_by_group_nonexistent() {
        let mut state = CrdBrowserState::default();
        state.set_crds(sample_crds());
        state.filter_by_group(Some("nonexistent.io".to_string()));
        assert!(state.filtered_crds().is_empty());
    }

    // --- select_crd tests ---

    #[test]
    fn test_select_crd() {
        let mut state = CrdBrowserState::default();
        state.set_crds(sample_crds());

        state.select_crd("certificates.cert-manager.io");
        assert_eq!(state.selected_crd.as_deref(), Some("certificates.cert-manager.io"));

        let selected = state.selected().unwrap();
        assert_eq!(selected.kind, "Certificate");
        assert_eq!(selected.group, "cert-manager.io");
    }

    #[test]
    fn test_select_crd_not_found() {
        let mut state = CrdBrowserState::default();
        state.set_crds(sample_crds());

        state.select_crd("nonexistent.test.io");
        assert_eq!(state.selected_crd.as_deref(), Some("nonexistent.test.io"));
        assert!(state.selected().is_none());
    }

    #[test]
    fn test_clear_selection() {
        let mut state = CrdBrowserState::default();
        state.set_crds(sample_crds());
        state.select_crd("certificates.cert-manager.io");
        assert!(state.selected_crd.is_some());

        state.clear_selection();
        assert!(state.selected_crd.is_none());
        assert!(state.selected().is_none());
    }

    // --- grouped_crds tests ---

    #[test]
    fn test_grouped_crds() {
        let mut state = CrdBrowserState::default();
        state.set_crds(sample_crds());

        let grouped = state.grouped_crds();
        assert_eq!(grouped.len(), 3);
        assert_eq!(grouped["cert-manager.io"].len(), 3);
        assert_eq!(grouped["kubevirt.io"].len(), 1);
        assert_eq!(grouped["traefik.containo.us"].len(), 1);
    }

    #[test]
    fn test_grouped_crds_empty() {
        let state = CrdBrowserState::default();
        let grouped = state.grouped_crds();
        assert!(grouped.is_empty());
    }

    // --- filtered_crds tests ---

    #[test]
    fn test_filtered_crds_no_filter() {
        let mut state = CrdBrowserState::default();
        state.set_crds(sample_crds());
        assert_eq!(state.filtered_crds().len(), 5);
    }

    #[test]
    fn test_filtered_crds_with_group_filter() {
        let mut state = CrdBrowserState::default();
        state.set_crds(sample_crds());
        state.filter_by_group(Some("kubevirt.io".to_string()));

        let filtered = state.filtered_crds();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].kind, "VirtualMachine");
    }

    // --- api_groups tests ---

    #[test]
    fn test_api_groups() {
        let mut state = CrdBrowserState::default();
        state.set_crds(sample_crds());

        let groups = state.api_groups();
        assert_eq!(groups.len(), 3);
        assert!(groups.contains(&"cert-manager.io"));
        assert!(groups.contains(&"kubevirt.io"));
        assert!(groups.contains(&"traefik.containo.us"));
    }

    #[test]
    fn test_api_groups_sorted() {
        let mut state = CrdBrowserState::default();
        state.set_crds(sample_crds());

        let groups = state.api_groups();
        let mut sorted = groups.clone();
        sorted.sort();
        assert_eq!(groups, sorted);
    }

    #[test]
    fn test_api_groups_empty() {
        let state = CrdBrowserState::default();
        assert!(state.api_groups().is_empty());
    }

    // --- Count tests ---

    #[test]
    fn test_total_count() {
        let mut state = CrdBrowserState::default();
        state.set_crds(sample_crds());
        assert_eq!(state.total_count(), 5);
    }

    #[test]
    fn test_filtered_count() {
        let mut state = CrdBrowserState::default();
        state.set_crds(sample_crds());
        assert_eq!(state.filtered_count(), 5);

        state.filter_by_group(Some("cert-manager.io".to_string()));
        assert_eq!(state.filtered_count(), 3);
    }

    #[test]
    fn test_namespaced_count() {
        let mut state = CrdBrowserState::default();
        state.set_crds(sample_crds());
        // 4 namespaced: Certificate, Issuer, VirtualMachine, IngressRoute
        assert_eq!(state.namespaced_count(), 4);
    }

    #[test]
    fn test_cluster_scoped_count() {
        let mut state = CrdBrowserState::default();
        state.set_crds(sample_crds());
        // 1 cluster-scoped: ClusterIssuer
        assert_eq!(state.cluster_scoped_count(), 1);
    }

    #[test]
    fn test_scope_counts_with_filter() {
        let mut state = CrdBrowserState::default();
        state.set_crds(sample_crds());
        state.filter_by_group(Some("cert-manager.io".to_string()));

        // cert-manager.io: Certificate (NS), Issuer (NS), ClusterIssuer (Cluster)
        assert_eq!(state.namespaced_count(), 2);
        assert_eq!(state.cluster_scoped_count(), 1);
    }

    // --- Full workflow test ---

    #[test]
    fn test_full_workflow() {
        let mut state = CrdBrowserState::default();

        // Start loading
        state.set_loading(true);
        assert!(state.loading);
        assert!(state.error.is_none());

        // Receive CRDs
        state.set_crds(sample_crds());
        assert!(!state.loading);
        assert_eq!(state.total_count(), 5);

        // Check groups
        let groups = state.api_groups();
        assert_eq!(groups.len(), 3);

        // Filter to cert-manager.io
        state.filter_by_group(Some("cert-manager.io".to_string()));
        assert_eq!(state.filtered_count(), 3);

        // Select a CRD
        state.select_crd("certificates.cert-manager.io");
        let selected = state.selected().unwrap();
        assert_eq!(selected.kind, "Certificate");
        assert!(selected.is_namespaced());

        // Clear filter
        state.filter_by_group(None);
        assert_eq!(state.filtered_count(), 5);

        // Check scope counts
        assert_eq!(state.namespaced_count(), 4);
        assert_eq!(state.cluster_scoped_count(), 1);
    }

    #[test]
    fn test_error_workflow() {
        let mut state = CrdBrowserState::default();

        state.set_loading(true);
        assert!(state.loading);

        state.set_error("cluster unreachable".to_string());
        assert!(!state.loading);
        assert_eq!(state.error.as_deref(), Some("cluster unreachable"));
        assert!(state.crds.is_empty());
    }
}
