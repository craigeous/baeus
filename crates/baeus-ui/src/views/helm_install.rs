use baeus_helm::charts::ChartEntry;
use gpui::prelude::FluentBuilder as _;
use gpui::{Context, ElementId, Rgba, SharedString, Window, div, prelude::*, px};

use crate::theme::Theme;

/// State for the values editor within the Helm install view.
#[derive(Debug, Clone)]
pub struct ValuesEditorState {
    /// The raw YAML content being edited.
    pub content: String,
    /// Whether the YAML content is valid.
    pub is_valid: bool,
    /// Validation error message, if any.
    pub validation_error: Option<String>,
}

impl Default for ValuesEditorState {
    fn default() -> Self {
        Self { content: String::new(), is_valid: true, validation_error: None }
    }
}

impl ValuesEditorState {
    /// Set the editor content and validate it.
    pub fn set_content(&mut self, content: &str) {
        self.content = content.to_string();
        self.validate();
    }

    /// Validate the current content as YAML.
    pub fn validate(&mut self) -> bool {
        if self.content.is_empty() {
            self.is_valid = true;
            self.validation_error = None;
            return true;
        }

        match serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&self.content) {
            Ok(_) => {
                self.is_valid = true;
                self.validation_error = None;
                true
            }
            Err(e) => {
                self.is_valid = false;
                self.validation_error = Some(e.to_string());
                false
            }
        }
    }

    /// Clear the editor content and reset validation state.
    pub fn clear(&mut self) {
        self.content = String::new();
        self.is_valid = true;
        self.validation_error = None;
    }
}

/// State for the Helm chart install view.
///
/// Manages the chart search, selection, version picking, namespace
/// targeting, and values editing workflow for installing a new Helm chart.
#[derive(Debug)]
pub struct HelmInstallViewState {
    /// Current search query for finding charts.
    pub search_query: String,
    /// Search results matching the current query.
    pub search_results: Vec<ChartEntry>,
    /// The chart entry the user has selected from search results.
    pub selected_chart: Option<ChartEntry>,
    /// The version the user has chosen (may differ from the selected chart's version).
    pub selected_version: Option<String>,
    /// The target namespace for the installation.
    pub namespace: String,
    /// State of the values editor.
    pub values_editor: ValuesEditorState,
    /// Whether a search is in progress.
    pub searching: bool,
    /// Whether an install operation is in progress.
    pub installing: bool,
    /// Error message from search or install.
    pub error: Option<String>,
}

impl Default for HelmInstallViewState {
    fn default() -> Self {
        Self {
            search_query: String::new(),
            search_results: Vec::new(),
            selected_chart: None,
            selected_version: None,
            namespace: "default".to_string(),
            values_editor: ValuesEditorState::default(),
            searching: false,
            installing: false,
            error: None,
        }
    }
}

impl HelmInstallViewState {
    /// Set the search results from a chart index query.
    pub fn set_search_results(&mut self, results: Vec<ChartEntry>) {
        self.search_results = results;
        self.searching = false;
        self.error = None;
    }

    /// Select a chart from the search results.
    /// Also sets the selected version to the chart's version.
    pub fn select_chart(&mut self, chart: ChartEntry) {
        self.selected_version = Some(chart.version.clone());
        self.selected_chart = Some(chart);
    }

    /// Set the desired version for the selected chart.
    pub fn select_version(&mut self, version: &str) {
        self.selected_version = Some(version.to_string());
    }

    /// Set the target namespace for the install.
    pub fn set_namespace(&mut self, namespace: &str) {
        self.namespace = namespace.to_string();
    }

    /// Reset the entire view state to its defaults.
    pub fn clear(&mut self) {
        self.search_query = String::new();
        self.search_results = Vec::new();
        self.selected_chart = None;
        self.selected_version = None;
        self.namespace = "default".to_string();
        self.values_editor.clear();
        self.searching = false;
        self.installing = false;
        self.error = None;
    }

    /// Returns true if the view has enough state to attempt an install:
    /// a chart is selected, a version is chosen, namespace is non-empty,
    /// and the values editor content is valid.
    pub fn can_install(&self) -> bool {
        self.selected_chart.is_some()
            && self.selected_version.is_some()
            && !self.namespace.is_empty()
            && self.values_editor.is_valid
            && !self.installing
    }

    /// Returns the chart name of the selected chart, if any.
    pub fn selected_chart_name(&self) -> Option<&str> {
        self.selected_chart.as_ref().map(|c| c.name.as_str())
    }

    // --- T063: Search & install lifecycle helpers ---

    /// Mark the start of a chart search. Sets searching=true, clears error.
    pub fn begin_search(&mut self) {
        self.searching = true;
        self.error = None;
    }

    /// Finish a successful search. Sets the results and clears searching flag.
    pub fn search_complete(&mut self, results: Vec<ChartEntry>) {
        self.search_results = results;
        self.searching = false;
        self.error = None;
    }

    /// Finish a failed search. Sets the error and clears searching flag.
    pub fn search_failed(&mut self, error: String) {
        self.error = Some(error);
        self.searching = false;
    }

    /// Mark the start of an install operation. Sets installing=true, clears error.
    pub fn begin_install(&mut self) {
        self.installing = true;
        self.error = None;
    }

    /// Finish a successful install. Resets the view state back to defaults.
    pub fn install_complete(&mut self) {
        self.clear();
    }

    /// Finish a failed install. Sets the error and clears installing flag.
    pub fn install_failed(&mut self, error: String) {
        self.error = Some(error);
        self.installing = false;
    }
}

// ---------------------------------------------------------------------------
// GPUI Render (T062)
// ---------------------------------------------------------------------------

/// Precomputed colors for rendering the install view.
#[allow(dead_code)]
struct InstallViewColors {
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

/// View wrapper for `HelmInstallViewState` with theme for rendering.
pub struct HelmInstallViewComponent {
    pub state: HelmInstallViewState,
    pub theme: Theme,
}

impl HelmInstallViewComponent {
    pub fn new(state: HelmInstallViewState, theme: Theme) -> Self {
        Self { state, theme }
    }

    /// Returns a status label for the install button.
    pub fn install_button_label(&self) -> &'static str {
        if self.state.installing { "Installing..." } else { "Install" }
    }

    /// Returns whether the install button should be enabled.
    pub fn install_button_enabled(&self) -> bool {
        self.state.can_install()
    }

    /// Returns a label summarizing validation state.
    pub fn validation_label(&self) -> &str {
        if let Some(ref err) = self.state.values_editor.validation_error {
            err.as_str()
        } else if self.state.values_editor.is_valid {
            "Valid YAML"
        } else {
            "Invalid YAML"
        }
    }

    /// Search bar with query display and searching indicator.
    fn render_search_bar(&self, colors: &InstallViewColors) -> gpui::Div {
        let query_text = if self.state.search_query.is_empty() {
            SharedString::from("Search charts...")
        } else {
            SharedString::from(self.state.search_query.clone())
        };

        let tc = if self.state.search_query.is_empty() {
            colors.text_muted
        } else {
            colors.text_primary
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
                    .id("chart-search-bar")
                    .flex_1()
                    .px_2()
                    .py_1()
                    .rounded(px(4.0))
                    .bg(colors.surface)
                    .border_1()
                    .border_color(colors.border)
                    .text_sm()
                    .text_color(tc)
                    .child(query_text),
            )
            .when(self.state.searching, |el| {
                el.child(div().text_xs().text_color(colors.text_muted).child("Searching..."))
            })
    }

    /// List of chart entries from search results.
    fn render_results_list(&self, colors: &InstallViewColors) -> gpui::Div {
        if self.state.search_results.is_empty() {
            return div()
                .flex()
                .flex_col()
                .py_4()
                .items_center()
                .child(div().text_sm().text_color(colors.text_muted).child("No charts found"));
        }

        let sel_name = self.state.selected_chart_name();
        let mut list = div().flex().flex_col().overflow_hidden();

        for (i, chart) in self.state.search_results.iter().enumerate() {
            let is_sel = sel_name == Some(chart.name.as_str())
                && self.state.selected_version.as_deref() == Some(chart.version.as_str());
            list = list.child(self.render_chart_entry(chart, i, is_sel, colors));
        }

        list
    }

    /// Single chart entry row: name, version, description.
    fn render_chart_entry(
        &self,
        chart: &ChartEntry,
        index: usize,
        selected: bool,
        colors: &InstallViewColors,
    ) -> gpui::Stateful<gpui::Div> {
        let eid = format!("chart-{index}");
        let bg = if selected { colors.selection } else { colors.background };
        let desc = chart.description.as_deref().unwrap_or("");

        div()
            .id(ElementId::Name(SharedString::from(eid)))
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
            .child(
                div()
                    .flex_1()
                    .text_sm()
                    .text_color(colors.text_primary)
                    .child(SharedString::from(chart.name.clone())),
            )
            .child(
                div()
                    .mr_2()
                    .text_xs()
                    .text_color(colors.text_secondary)
                    .child(SharedString::from(chart.version.clone())),
            )
            .child(
                div()
                    .flex_1()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child(SharedString::from(desc.to_string())),
            )
    }

    /// Detail panel for the selected chart.
    fn render_chart_detail(&self, colors: &InstallViewColors) -> gpui::Div {
        let chart = match &self.state.selected_chart {
            Some(c) => c,
            None => {
                return div().flex().flex_col().flex_1().items_center().justify_center().child(
                    div()
                        .text_sm()
                        .text_color(colors.text_muted)
                        .child("Select a chart to see details"),
                );
            }
        };

        let version_text = self.state.selected_version.as_deref().unwrap_or(&chart.version);
        let desc = chart.description.as_deref().unwrap_or("No description");
        let home = chart.home.as_deref().unwrap_or("N/A");
        let sources =
            if chart.sources.is_empty() { "None".to_string() } else { chart.sources.join(", ") };

        div()
            .flex()
            .flex_col()
            .flex_1()
            .p_3()
            .gap(px(8.0))
            .child(
                div()
                    .text_lg()
                    .text_color(colors.text_primary)
                    .child(SharedString::from(chart.name.clone())),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(colors.text_secondary)
                    .child(SharedString::from(desc.to_string())),
            )
            .child(self.render_detail_row("Version", version_text, colors))
            .child(self.render_detail_row("Home", home, colors))
            .child(self.render_detail_row("Sources", &sources, colors))
    }

    /// Key-value detail row.
    fn render_detail_row(&self, key: &str, value: &str, colors: &InstallViewColors) -> gpui::Div {
        div()
            .flex()
            .flex_row()
            .gap(px(8.0))
            .child(
                div()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child(SharedString::from(format!("{key}:"))),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(colors.text_primary)
                    .child(SharedString::from(value.to_string())),
            )
    }

    /// YAML values editor with validation status.
    fn render_values_editor(&self, colors: &InstallViewColors) -> gpui::Div {
        let border_c = if self.state.values_editor.is_valid { colors.border } else { colors.error };
        let content = if self.state.values_editor.content.is_empty() {
            SharedString::from("# Enter custom values (YAML)")
        } else {
            SharedString::from(self.state.values_editor.content.clone())
        };
        let tc = if self.state.values_editor.content.is_empty() {
            colors.text_muted
        } else {
            colors.text_primary
        };

        let mut editor = div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(div().text_xs().text_color(colors.text_secondary).child("Values (YAML)"))
            .child(
                div()
                    .id("values-editor")
                    .w_full()
                    .min_h(px(120.0))
                    .p_2()
                    .rounded(px(4.0))
                    .bg(colors.surface)
                    .border_1()
                    .border_color(border_c)
                    .text_xs()
                    .text_color(tc)
                    .overflow_hidden()
                    .child(content),
            );

        if let Some(ref err) = self.state.values_editor.validation_error {
            editor = editor.child(
                div().text_xs().text_color(colors.error).child(SharedString::from(err.clone())),
            );
        }

        editor
    }

    /// Install bar: namespace input, install button, progress.
    fn render_install_bar(&self, colors: &InstallViewColors) -> gpui::Div {
        let can = self.state.can_install();
        let btn_c = if can { colors.accent } else { colors.text_muted };
        let btn_label = SharedString::from(self.install_button_label());

        let ns_text = SharedString::from(self.state.namespace.clone());
        let ns_tc =
            if self.state.namespace.is_empty() { colors.text_muted } else { colors.text_primary };

        let white = crate::theme::Color::rgb(255, 255, 255).to_gpui();

        let mut bar = div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .px_3()
            .py_2()
            .gap(px(8.0))
            .border_t_1()
            .border_color(colors.border)
            .child(div().text_xs().text_color(colors.text_secondary).child("Namespace:"))
            .child(
                div()
                    .id("namespace-input")
                    .px_2()
                    .py_1()
                    .rounded(px(4.0))
                    .bg(colors.surface)
                    .border_1()
                    .border_color(colors.border)
                    .text_sm()
                    .text_color(ns_tc)
                    .child(ns_text),
            )
            .child(div().flex_1())
            .child(
                div()
                    .id("install-btn")
                    .px_4()
                    .py_1()
                    .rounded(px(4.0))
                    .bg(btn_c)
                    .cursor_pointer()
                    .text_sm()
                    .text_color(white)
                    .child(btn_label),
            );

        if self.state.installing {
            bar = bar.child(div().text_xs().text_color(colors.text_muted).child("Installing..."));
        }

        bar
    }

    /// Error message display.
    fn render_error(&self, colors: &InstallViewColors) -> gpui::Div {
        let msg = self.state.error.as_deref().unwrap_or("Unknown error");
        div().px_3().py_2().child(
            div().text_sm().text_color(colors.error).child(SharedString::from(msg.to_string())),
        )
    }
}

impl Render for HelmInstallViewComponent {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let colors = InstallViewColors {
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

        // Search bar at top
        root = root.child(self.render_search_bar(&colors));

        // Error banner if present
        if self.state.error.is_some() {
            root = root.child(self.render_error(&colors));
        }

        // Main content: left = results list, right = detail + values editor
        root = root.child(
            div()
                .flex()
                .flex_row()
                .flex_1()
                .overflow_hidden()
                .child(
                    div()
                        .w(px(320.0))
                        .border_r_1()
                        .border_color(colors.border)
                        .overflow_hidden()
                        .child(self.render_results_list(&colors)),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .overflow_hidden()
                        .child(self.render_chart_detail(&colors))
                        .child(self.render_values_editor(&colors)),
                ),
        );

        // Install bar at bottom
        root = root.child(self.render_install_bar(&colors));

        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_chart(name: &str, version: &str) -> ChartEntry {
        ChartEntry {
            name: name.to_string(),
            version: version.to_string(),
            app_version: Some("1.0.0".to_string()),
            description: Some(format!("{name} chart")),
            home: None,
            sources: vec![],
            urls: vec![format!("https://charts.example.com/{name}-{version}.tgz")],
        }
    }

    fn sample_search_results() -> Vec<ChartEntry> {
        vec![
            sample_chart("nginx", "15.4.0"),
            sample_chart("nginx", "15.3.0"),
            sample_chart("nginx-ingress", "4.8.3"),
        ]
    }

    // --- ValuesEditorState tests ---

    #[test]
    fn test_values_editor_default() {
        let editor = ValuesEditorState::default();
        assert!(editor.content.is_empty());
        assert!(editor.is_valid);
        assert!(editor.validation_error.is_none());
    }

    #[test]
    fn test_values_editor_set_content_valid() {
        let mut editor = ValuesEditorState::default();
        editor.set_content("replicaCount: 3\nimage:\n  tag: latest");
        assert_eq!(editor.content, "replicaCount: 3\nimage:\n  tag: latest");
        assert!(editor.is_valid);
        assert!(editor.validation_error.is_none());
    }

    #[test]
    fn test_values_editor_set_content_invalid() {
        let mut editor = ValuesEditorState::default();
        editor.set_content("key: [unclosed");
        assert!(!editor.is_valid);
        assert!(editor.validation_error.is_some());
    }

    #[test]
    fn test_values_editor_set_content_empty() {
        let mut editor = ValuesEditorState::default();
        editor.set_content("");
        assert!(editor.is_valid);
        assert!(editor.validation_error.is_none());
    }

    #[test]
    fn test_values_editor_validate_empty() {
        let mut editor = ValuesEditorState::default();
        assert!(editor.validate());
        assert!(editor.is_valid);
    }

    #[test]
    fn test_values_editor_clear() {
        let mut editor = ValuesEditorState::default();
        editor.set_content("key: [invalid");
        assert!(!editor.is_valid);

        editor.clear();
        assert!(editor.content.is_empty());
        assert!(editor.is_valid);
        assert!(editor.validation_error.is_none());
    }

    // --- HelmInstallViewState tests ---

    #[test]
    fn test_default_state() {
        let state = HelmInstallViewState::default();
        assert!(state.search_query.is_empty());
        assert!(state.search_results.is_empty());
        assert!(state.selected_chart.is_none());
        assert!(state.selected_version.is_none());
        assert_eq!(state.namespace, "default");
        assert!(state.values_editor.content.is_empty());
        assert!(!state.searching);
        assert!(!state.installing);
        assert!(state.error.is_none());
    }

    #[test]
    fn test_set_search_results() {
        let mut state = HelmInstallViewState::default();
        state.searching = true;
        state.error = Some("old error".to_string());

        state.set_search_results(sample_search_results());

        assert_eq!(state.search_results.len(), 3);
        assert!(!state.searching);
        assert!(state.error.is_none());
    }

    #[test]
    fn test_set_search_results_empty() {
        let mut state = HelmInstallViewState::default();
        state.set_search_results(vec![]);
        assert!(state.search_results.is_empty());
    }

    #[test]
    fn test_select_chart() {
        let mut state = HelmInstallViewState::default();
        let chart = sample_chart("nginx", "15.4.0");

        state.select_chart(chart);

        assert!(state.selected_chart.is_some());
        assert_eq!(state.selected_chart.as_ref().unwrap().name, "nginx");
        assert_eq!(state.selected_version.as_deref(), Some("15.4.0"));
    }

    #[test]
    fn test_select_chart_sets_version() {
        let mut state = HelmInstallViewState::default();
        let chart = sample_chart("redis", "18.5.0");

        state.select_chart(chart);

        assert_eq!(state.selected_version.as_deref(), Some("18.5.0"));
    }

    #[test]
    fn test_select_version() {
        let mut state = HelmInstallViewState::default();
        let chart = sample_chart("nginx", "15.4.0");
        state.select_chart(chart);

        state.select_version("15.3.0");

        assert_eq!(state.selected_version.as_deref(), Some("15.3.0"));
        // The selected chart should still be the same
        assert_eq!(state.selected_chart.as_ref().unwrap().name, "nginx");
    }

    #[test]
    fn test_set_namespace() {
        let mut state = HelmInstallViewState::default();
        assert_eq!(state.namespace, "default");

        state.set_namespace("production");
        assert_eq!(state.namespace, "production");
    }

    #[test]
    fn test_set_namespace_empty() {
        let mut state = HelmInstallViewState::default();
        state.set_namespace("");
        assert_eq!(state.namespace, "");
    }

    #[test]
    fn test_clear() {
        let mut state = HelmInstallViewState::default();
        state.search_query = "nginx".to_string();
        state.set_search_results(sample_search_results());
        state.select_chart(sample_chart("nginx", "15.4.0"));
        state.set_namespace("production");
        state.values_editor.set_content("replicaCount: 3");
        state.installing = true;
        state.error = Some("timeout".to_string());

        state.clear();

        assert!(state.search_query.is_empty());
        assert!(state.search_results.is_empty());
        assert!(state.selected_chart.is_none());
        assert!(state.selected_version.is_none());
        assert_eq!(state.namespace, "default");
        assert!(state.values_editor.content.is_empty());
        assert!(!state.searching);
        assert!(!state.installing);
        assert!(state.error.is_none());
    }

    #[test]
    fn test_can_install_true() {
        let mut state = HelmInstallViewState::default();
        state.select_chart(sample_chart("nginx", "15.4.0"));
        state.set_namespace("default");

        assert!(state.can_install());
    }

    #[test]
    fn test_can_install_false_no_chart() {
        let state = HelmInstallViewState::default();
        assert!(!state.can_install());
    }

    #[test]
    fn test_can_install_false_no_version() {
        let mut state = HelmInstallViewState::default();
        state.select_chart(sample_chart("nginx", "15.4.0"));
        state.selected_version = None;

        assert!(!state.can_install());
    }

    #[test]
    fn test_can_install_false_empty_namespace() {
        let mut state = HelmInstallViewState::default();
        state.select_chart(sample_chart("nginx", "15.4.0"));
        state.set_namespace("");

        assert!(!state.can_install());
    }

    #[test]
    fn test_can_install_false_invalid_values() {
        let mut state = HelmInstallViewState::default();
        state.select_chart(sample_chart("nginx", "15.4.0"));
        state.values_editor.set_content("key: [invalid");

        assert!(!state.can_install());
    }

    #[test]
    fn test_can_install_false_when_installing() {
        let mut state = HelmInstallViewState::default();
        state.select_chart(sample_chart("nginx", "15.4.0"));
        state.installing = true;

        assert!(!state.can_install());
    }

    #[test]
    fn test_selected_chart_name() {
        let mut state = HelmInstallViewState::default();
        assert!(state.selected_chart_name().is_none());

        state.select_chart(sample_chart("nginx", "15.4.0"));
        assert_eq!(state.selected_chart_name(), Some("nginx"));
    }

    #[test]
    fn test_full_install_workflow() {
        let mut state = HelmInstallViewState::default();

        // 1. Search
        state.search_query = "nginx".to_string();
        state.searching = true;
        assert!(!state.can_install());

        // 2. Receive results
        state.set_search_results(sample_search_results());
        assert!(!state.searching);
        assert_eq!(state.search_results.len(), 3);

        // 3. Select a chart
        state.select_chart(sample_chart("nginx", "15.4.0"));
        assert!(state.can_install());

        // 4. Pick a different version
        state.select_version("15.3.0");
        assert_eq!(state.selected_version.as_deref(), Some("15.3.0"));
        assert!(state.can_install());

        // 5. Set namespace
        state.set_namespace("production");
        assert!(state.can_install());

        // 6. Edit values
        state.values_editor.set_content("replicaCount: 3\nimage:\n  tag: stable");
        assert!(state.values_editor.is_valid);
        assert!(state.can_install());

        // 7. Start install
        state.installing = true;
        assert!(!state.can_install()); // locked while installing

        // 8. Install complete, clear for next install
        state.clear();
        assert!(!state.can_install()); // no chart selected
        assert_eq!(state.namespace, "default");
    }

    #[test]
    fn test_values_editor_workflow() {
        let mut state = HelmInstallViewState::default();
        state.select_chart(sample_chart("nginx", "15.4.0"));

        // Empty values are fine
        assert!(state.can_install());

        // Set valid values
        state.values_editor.set_content("service:\n  type: LoadBalancer");
        assert!(state.can_install());

        // Set invalid values
        state.values_editor.set_content("invalid: yaml: [broken");
        assert!(!state.can_install());

        // Fix values
        state.values_editor.set_content("service:\n  type: ClusterIP");
        assert!(state.can_install());

        // Clear values
        state.values_editor.clear();
        assert!(state.can_install());
    }
}
