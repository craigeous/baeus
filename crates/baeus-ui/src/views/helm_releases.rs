use baeus_helm::{HelmRelease, HelmReleaseStatus};
use gpui::prelude::FluentBuilder as _;
use gpui::{Context, ElementId, Rgba, SharedString, Window, div, prelude::*, px};

use crate::theme::Theme;

/// Tracks the progress of an in-flight Helm operation (install, upgrade,
/// rollback, uninstall) so the UI can show spinners, success banners, or
/// error messages.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum HelmOperationState {
    #[default]
    Idle,
    InProgress {
        operation: String,
        release_name: String,
    },
    Success {
        message: String,
    },
    Failed {
        error: String,
    },
}

/// State for the Helm releases list view.
///
/// Manages the list of Helm releases across the connected cluster, with
/// support for namespace filtering, selection, and loading/error states.
#[derive(Debug, Default)]
pub struct HelmReleasesViewState {
    pub releases: Vec<HelmRelease>,
    pub loading: bool,
    pub error: Option<String>,
    pub namespace_filter: Option<String>,
    pub selected_release: Option<String>,
    pub operation_state: HelmOperationState,
}

impl HelmReleasesViewState {
    /// Replace the releases list with a new set of releases.
    pub fn set_releases(&mut self, releases: Vec<HelmRelease>) {
        self.releases = releases;
        self.loading = false;
        self.error = None;
    }

    /// Set the loading state. Clears any previous error.
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

    /// Filter the displayed releases by namespace.
    /// Pass `None` to show releases across all namespaces.
    pub fn filter_by_namespace(&mut self, namespace: Option<String>) {
        self.namespace_filter = namespace;
        // Clear selection when namespace filter changes, since the
        // previously selected release may no longer be visible.
        self.selected_release = None;
    }

    /// Select a release by name.
    pub fn select_release(&mut self, name: &str) {
        self.selected_release = Some(name.to_string());
    }

    /// Clear the current release selection.
    pub fn clear_selection(&mut self) {
        self.selected_release = None;
    }

    /// Returns the releases filtered by the current namespace filter.
    pub fn filtered_releases(&self) -> Vec<&HelmRelease> {
        match &self.namespace_filter {
            Some(ns) => self.releases.iter().filter(|r| r.namespace == *ns).collect(),
            None => self.releases.iter().collect(),
        }
    }

    /// Returns a reference to the currently selected release, if any.
    pub fn selected(&self) -> Option<&HelmRelease> {
        self.selected_release
            .as_ref()
            .and_then(|name| self.releases.iter().find(|r| r.name == *name))
    }

    /// Count of releases that are in a healthy (Deployed) state.
    pub fn healthy_count(&self) -> usize {
        self.filtered_releases().iter().filter(|r| r.status.is_healthy()).count()
    }

    /// Count of releases that are in a non-healthy state.
    pub fn unhealthy_count(&self) -> usize {
        self.filtered_releases().iter().filter(|r| !r.status.is_healthy()).count()
    }

    /// Returns the distinct namespaces present in the releases list.
    pub fn namespaces(&self) -> Vec<&str> {
        let mut ns: Vec<&str> = self.releases.iter().map(|r| r.namespace.as_str()).collect();
        ns.sort();
        ns.dedup();
        ns
    }

    // --- T063: Sorting & count helpers ---

    /// Sort releases by name (ascending, case-insensitive).
    pub fn sort_by_name(&mut self) {
        self.releases.sort_by_key(|a| a.name.to_lowercase());
    }

    /// Sort releases by status label (ascending alphabetical on the debug
    /// representation of the status enum).
    pub fn sort_by_status(&mut self) {
        self.releases.sort_by(|a, b| format!("{:?}", a.status).cmp(&format!("{:?}", b.status)));
    }

    /// Sort releases by last deployed timestamp (newest first).
    pub fn sort_by_last_deployed(&mut self) {
        self.releases.sort_by_key(|a| std::cmp::Reverse(a.last_deployed));
    }

    /// Returns the total number of releases (unfiltered).
    pub fn release_count(&self) -> usize {
        self.releases.len()
    }

    // --- T064: Operation state helpers ---

    /// Begin tracking a Helm operation (install, upgrade, rollback, uninstall).
    pub fn begin_operation(&mut self, operation: &str, release_name: &str) {
        self.operation_state = HelmOperationState::InProgress {
            operation: operation.to_string(),
            release_name: release_name.to_string(),
        };
    }

    /// Mark the current operation as successfully completed.
    pub fn operation_success(&mut self, message: String) {
        self.operation_state = HelmOperationState::Success { message };
    }

    /// Mark the current operation as failed.
    pub fn operation_failed(&mut self, error: String) {
        self.operation_state = HelmOperationState::Failed { error };
    }

    /// Reset the operation state back to Idle (dismiss banner).
    pub fn dismiss_operation_result(&mut self) {
        self.operation_state = HelmOperationState::Idle;
    }

    /// Returns `true` when there is an operation in progress.
    pub fn is_operation_in_progress(&self) -> bool {
        matches!(self.operation_state, HelmOperationState::InProgress { .. })
    }
}

// ---------------------------------------------------------------------------
// GPUI Render (T061)
// ---------------------------------------------------------------------------

/// Precomputed colors for rendering the releases view.
#[allow(dead_code)]
struct ReleasesViewColors {
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

/// View wrapper for `HelmReleasesViewState` with theme for rendering.
pub struct HelmReleasesViewComponent {
    pub state: HelmReleasesViewState,
    pub theme: Theme,
}

impl HelmReleasesViewComponent {
    pub fn new(state: HelmReleasesViewState, theme: Theme) -> Self {
        Self { state, theme }
    }

    /// Returns the theme color for a given release status.
    pub fn status_color(&self, status: &HelmReleaseStatus) -> crate::theme::Color {
        match status {
            HelmReleaseStatus::Deployed => self.theme.colors.success,
            HelmReleaseStatus::Failed => self.theme.colors.error,
            HelmReleaseStatus::PendingInstall
            | HelmReleaseStatus::PendingUpgrade
            | HelmReleaseStatus::PendingRollback => self.theme.colors.warning,
            HelmReleaseStatus::Uninstalling | HelmReleaseStatus::Superseded => {
                self.theme.colors.text_muted
            }
            HelmReleaseStatus::Unknown => self.theme.colors.text_secondary,
        }
    }

    /// Returns a display label for a release status.
    pub fn status_label(status: &HelmReleaseStatus) -> &'static str {
        match status {
            HelmReleaseStatus::Deployed => "Deployed",
            HelmReleaseStatus::Failed => "Failed",
            HelmReleaseStatus::PendingInstall => "Pending Install",
            HelmReleaseStatus::PendingUpgrade => "Pending Upgrade",
            HelmReleaseStatus::PendingRollback => "Pending Rollback",
            HelmReleaseStatus::Uninstalling => "Uninstalling",
            HelmReleaseStatus::Superseded => "Superseded",
            HelmReleaseStatus::Unknown => "Unknown",
        }
    }

    /// Toolbar: namespace filter dropdown + health summary badges.
    fn render_toolbar(&self, colors: &ReleasesViewColors) -> gpui::Div {
        let healthy = self.state.healthy_count();
        let unhealthy = self.state.unhealthy_count();
        let healthy_lbl = SharedString::from(format!("Healthy: {healthy}"));
        let unhealthy_lbl = SharedString::from(format!("Unhealthy: {unhealthy}"));

        let filter_lbl = match &self.state.namespace_filter {
            Some(ns) => SharedString::from(format!("Namespace: {ns}")),
            None => SharedString::from("All Namespaces"),
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
                    .id("namespace-filter")
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
            .child(self.render_health_badge(healthy_lbl, colors.success, colors))
            .child(self.render_health_badge(unhealthy_lbl, colors.error, colors))
    }

    /// Small health summary badge.
    fn render_health_badge(
        &self,
        label: SharedString,
        dot_color: Rgba,
        colors: &ReleasesViewColors,
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

    /// Release table: header row + data rows.
    fn render_release_table(&self, colors: &ReleasesViewColors) -> gpui::Div {
        let filtered = self.state.filtered_releases();
        let sel = self.state.selected_release.as_deref();

        if filtered.is_empty() && !self.state.loading {
            return self.render_empty_state(colors);
        }

        let mut table = div().flex().flex_col().flex_1().w_full().overflow_hidden();

        table = table.child(self.render_header_row(colors));

        for release in &filtered {
            let is_sel = sel == Some(release.name.as_str());
            table = table.child(self.render_release_row(release, is_sel, colors));
        }

        table
    }

    /// Header row for the release table.
    fn render_header_row(&self, colors: &ReleasesViewColors) -> gpui::Div {
        div()
            .flex()
            .flex_row()
            .w_full()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(colors.border)
            .bg(colors.surface)
            .child(self.render_hdr_cell("Name", colors))
            .child(self.render_hdr_cell("Namespace", colors))
            .child(self.render_hdr_cell("Chart", colors))
            .child(self.render_hdr_cell("Version", colors))
            .child(self.render_hdr_cell("Status", colors))
            .child(self.render_hdr_cell("Last Deployed", colors))
            .child(self.render_hdr_cell("Actions", colors))
    }

    /// Single header cell.
    fn render_hdr_cell(&self, label: &str, colors: &ReleasesViewColors) -> gpui::Div {
        div()
            .flex_1()
            .text_xs()
            .text_color(colors.text_secondary)
            .child(SharedString::from(label.to_string()))
    }

    /// Single release row with all columns.
    fn render_release_row(
        &self,
        release: &HelmRelease,
        selected: bool,
        colors: &ReleasesViewColors,
    ) -> gpui::Stateful<gpui::Div> {
        let sc = self.status_color(&release.status).to_gpui();
        let sl = SharedString::from(Self::status_label(&release.status));
        let deployed = release.last_deployed.format("%Y-%m-%d %H:%M").to_string();

        let rid = format!("release-{}", release.name);
        let bg = if selected { colors.selection } else { colors.background };

        div()
            .id(ElementId::Name(SharedString::from(rid)))
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .px_3()
            .py_2()
            .cursor_pointer()
            .bg(bg)
            .border_b_1()
            .border_color(colors.border)
            .when(selected, |el| el.border_l_2().border_color(colors.accent))
            .child(self.render_cell(&release.name, colors.text_primary))
            .child(self.render_cell(&release.namespace, colors.text_secondary))
            .child(self.render_cell(&release.chart_name, colors.text_secondary))
            .child(self.render_cell(&release.chart_version, colors.text_secondary))
            .child(self.render_status_badge(sl, sc, colors))
            .child(self.render_cell(&deployed, colors.text_muted))
            .child(self.render_action_buttons(release, colors))
    }

    /// Single text cell in a release row.
    fn render_cell(&self, text: &str, color: Rgba) -> gpui::Div {
        div().flex_1().text_sm().text_color(color).child(SharedString::from(text.to_string()))
    }

    /// Status badge: colored dot + label.
    fn render_status_badge(
        &self,
        label: SharedString,
        color: Rgba,
        _colors: &ReleasesViewColors,
    ) -> gpui::Div {
        div()
            .flex_1()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.0))
            .child(div().w(px(8.0)).h(px(8.0)).rounded(px(4.0)).bg(color))
            .child(div().text_xs().text_color(color).child(label))
    }

    /// Action buttons (upgrade, rollback, uninstall).
    fn render_action_buttons(
        &self,
        release: &HelmRelease,
        colors: &ReleasesViewColors,
    ) -> gpui::Div {
        let n = &release.name;
        div()
            .flex_1()
            .flex()
            .flex_row()
            .gap(px(4.0))
            .child(self.render_action_btn(&format!("upgrade-{n}"), "Upgrade", colors))
            .child(self.render_action_btn(&format!("rollback-{n}"), "Rollback", colors))
            .child(self.render_action_btn(&format!("uninstall-{n}"), "Uninstall", colors))
    }

    /// Single small action button.
    fn render_action_btn(
        &self,
        id: &str,
        label: &str,
        colors: &ReleasesViewColors,
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
            .text_color(colors.text_primary)
            .child(SharedString::from(label.to_string()))
    }

    /// Empty state when no releases present.
    fn render_empty_state(&self, colors: &ReleasesViewColors) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .items_center()
            .justify_center()
            .child(div().text_sm().text_color(colors.text_muted).child("No Helm releases found"))
    }

    /// Loading indicator.
    fn render_loading(&self, colors: &ReleasesViewColors) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .items_center()
            .justify_center()
            .child(div().text_sm().text_color(colors.text_muted).child("Loading releases..."))
    }

    /// Error message display.
    fn render_error(&self, colors: &ReleasesViewColors) -> gpui::Div {
        let msg = self.state.error.as_deref().unwrap_or("Unknown error");
        div().flex().flex_col().flex_1().items_center().justify_center().px_4().child(
            div().text_sm().text_color(colors.error).child(SharedString::from(msg.to_string())),
        )
    }
}

impl Render for HelmReleasesViewComponent {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let colors = ReleasesViewColors {
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
            root = root.child(self.render_release_table(&colors));
        }

        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use baeus_helm::HelmReleaseStatus;
    use chrono::Utc;
    use serde_json::json;
    use uuid::Uuid;

    fn sample_release(name: &str, namespace: &str, status: HelmReleaseStatus) -> HelmRelease {
        HelmRelease {
            name: name.to_string(),
            namespace: namespace.to_string(),
            chart_name: format!("{name}-chart"),
            chart_version: "1.0.0".to_string(),
            app_version: Some("1.0.0".to_string()),
            status,
            revision: 1,
            last_deployed: Utc::now(),
            values: json!({}),
            cluster_id: Uuid::new_v4(),
        }
    }

    fn sample_releases() -> Vec<HelmRelease> {
        vec![
            sample_release("nginx", "default", HelmReleaseStatus::Deployed),
            sample_release("redis", "default", HelmReleaseStatus::Deployed),
            sample_release("prometheus", "monitoring", HelmReleaseStatus::Deployed),
            sample_release("broken-app", "staging", HelmReleaseStatus::Failed),
            sample_release("upgrading-app", "staging", HelmReleaseStatus::PendingUpgrade),
        ]
    }

    #[test]
    fn test_default_state() {
        let state = HelmReleasesViewState::default();
        assert!(state.releases.is_empty());
        assert!(!state.loading);
        assert!(state.error.is_none());
        assert!(state.namespace_filter.is_none());
        assert!(state.selected_release.is_none());
    }

    #[test]
    fn test_set_releases() {
        let mut state = HelmReleasesViewState {
            loading: true,
            error: Some("old error".to_string()),
            ..Default::default()
        };

        state.set_releases(sample_releases());

        assert_eq!(state.releases.len(), 5);
        assert!(!state.loading);
        assert!(state.error.is_none());
    }

    #[test]
    fn test_set_loading() {
        let mut state =
            HelmReleasesViewState { error: Some("some error".to_string()), ..Default::default() };

        state.set_loading(true);
        assert!(state.loading);
        assert!(state.error.is_none());

        state.set_loading(false);
        assert!(!state.loading);
    }

    #[test]
    fn test_set_error() {
        let mut state = HelmReleasesViewState { loading: true, ..Default::default() };

        state.set_error("connection refused".to_string());
        assert_eq!(state.error.as_deref(), Some("connection refused"));
        assert!(!state.loading);
    }

    #[test]
    fn test_filter_by_namespace() {
        let mut state = HelmReleasesViewState::default();
        state.set_releases(sample_releases());
        state.select_release("nginx");
        assert!(state.selected_release.is_some());

        state.filter_by_namespace(Some("monitoring".to_string()));
        assert_eq!(state.namespace_filter.as_deref(), Some("monitoring"));
        // Selection cleared when filter changes
        assert!(state.selected_release.is_none());

        let filtered = state.filtered_releases();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "prometheus");
    }

    #[test]
    fn test_filter_by_namespace_none_shows_all() {
        let mut state = HelmReleasesViewState::default();
        state.set_releases(sample_releases());
        state.filter_by_namespace(Some("default".to_string()));
        assert_eq!(state.filtered_releases().len(), 2);

        state.filter_by_namespace(None);
        assert_eq!(state.filtered_releases().len(), 5);
    }

    #[test]
    fn test_select_release() {
        let mut state = HelmReleasesViewState::default();
        state.set_releases(sample_releases());

        state.select_release("redis");
        assert_eq!(state.selected_release.as_deref(), Some("redis"));

        let selected = state.selected().unwrap();
        assert_eq!(selected.name, "redis");
        assert_eq!(selected.namespace, "default");
    }

    #[test]
    fn test_select_release_not_found() {
        let mut state = HelmReleasesViewState::default();
        state.set_releases(sample_releases());

        state.select_release("nonexistent");
        assert_eq!(state.selected_release.as_deref(), Some("nonexistent"));
        assert!(state.selected().is_none());
    }

    #[test]
    fn test_clear_selection() {
        let mut state = HelmReleasesViewState::default();
        state.set_releases(sample_releases());
        state.select_release("nginx");
        assert!(state.selected_release.is_some());

        state.clear_selection();
        assert!(state.selected_release.is_none());
        assert!(state.selected().is_none());
    }

    #[test]
    fn test_filtered_releases_no_filter() {
        let mut state = HelmReleasesViewState::default();
        state.set_releases(sample_releases());

        let filtered = state.filtered_releases();
        assert_eq!(filtered.len(), 5);
    }

    #[test]
    fn test_filtered_releases_by_namespace() {
        let mut state = HelmReleasesViewState::default();
        state.set_releases(sample_releases());
        state.filter_by_namespace(Some("staging".to_string()));

        let filtered = state.filtered_releases();
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|r| r.namespace == "staging"));
    }

    #[test]
    fn test_filtered_releases_empty_namespace() {
        let mut state = HelmReleasesViewState::default();
        state.set_releases(sample_releases());
        state.filter_by_namespace(Some("nonexistent".to_string()));

        assert!(state.filtered_releases().is_empty());
    }

    #[test]
    fn test_healthy_count() {
        let mut state = HelmReleasesViewState::default();
        state.set_releases(sample_releases());

        // All namespaces: nginx, redis, prometheus are Deployed (3 healthy)
        assert_eq!(state.healthy_count(), 3);
    }

    #[test]
    fn test_unhealthy_count() {
        let mut state = HelmReleasesViewState::default();
        state.set_releases(sample_releases());

        // broken-app (Failed) + upgrading-app (PendingUpgrade) = 2 unhealthy
        assert_eq!(state.unhealthy_count(), 2);
    }

    #[test]
    fn test_healthy_count_with_namespace_filter() {
        let mut state = HelmReleasesViewState::default();
        state.set_releases(sample_releases());
        state.filter_by_namespace(Some("staging".to_string()));

        assert_eq!(state.healthy_count(), 0);
        assert_eq!(state.unhealthy_count(), 2);
    }

    #[test]
    fn test_namespaces() {
        let mut state = HelmReleasesViewState::default();
        state.set_releases(sample_releases());

        let ns = state.namespaces();
        assert_eq!(ns, vec!["default", "monitoring", "staging"]);
    }

    #[test]
    fn test_namespaces_empty() {
        let state = HelmReleasesViewState::default();
        assert!(state.namespaces().is_empty());
    }

    #[test]
    fn test_full_workflow() {
        let mut state = HelmReleasesViewState::default();

        // Start loading
        state.set_loading(true);
        assert!(state.loading);
        assert!(state.error.is_none());

        // Receive releases
        state.set_releases(sample_releases());
        assert!(!state.loading);
        assert_eq!(state.releases.len(), 5);

        // Filter to default namespace
        state.filter_by_namespace(Some("default".to_string()));
        assert_eq!(state.filtered_releases().len(), 2);

        // Select a release
        state.select_release("nginx");
        let selected = state.selected().unwrap();
        assert_eq!(selected.name, "nginx");
        assert!(selected.status.is_healthy());

        // Clear filter
        state.filter_by_namespace(None);
        assert_eq!(state.filtered_releases().len(), 5);

        // Check health counts
        assert_eq!(state.healthy_count(), 3);
        assert_eq!(state.unhealthy_count(), 2);
    }

    #[test]
    fn test_error_workflow() {
        let mut state = HelmReleasesViewState::default();

        state.set_loading(true);
        assert!(state.loading);

        state.set_error("cluster unreachable".to_string());
        assert!(!state.loading);
        assert_eq!(state.error.as_deref(), Some("cluster unreachable"));
        assert!(state.releases.is_empty());
    }

    #[test]
    fn test_set_loading_false_does_not_clear_error() {
        let mut state = HelmReleasesViewState::default();
        state.set_error("some error".to_string());

        // Setting loading to false should not clear the error
        state.set_loading(false);
        assert!(!state.loading);
        assert!(state.error.is_some());
    }
}
