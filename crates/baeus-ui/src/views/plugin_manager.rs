use baeus_plugins::{Plugin, PluginError, PluginPermission, PluginState};
use gpui::prelude::FluentBuilder as _;
use gpui::{Context, ElementId, Rgba, SharedString, Window, div, prelude::*, px};

use crate::theme::Theme;

/// State for the plugin manager view.
///
/// Manages the list of installed plugins with support for
/// selection, loading/error states, and install/enable/disable/uninstall
/// operations.
#[derive(Debug, Default, Clone)]
pub struct PluginManagerState {
    /// List of installed plugins
    pub plugins: Vec<Plugin>,
    /// Currently selected plugin ID
    pub selected_plugin: Option<String>,
    /// Whether the plugin list is being loaded
    pub loading: bool,
    /// Error message if something went wrong
    pub error: Option<String>,
}

impl PluginManagerState {
    /// Replace the plugins list with a new set of plugins.
    pub fn set_plugins(&mut self, plugins: Vec<Plugin>) {
        self.plugins = plugins;
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

    /// Select a plugin by ID.
    pub fn select_plugin(&mut self, plugin_id: &str) {
        self.selected_plugin = Some(plugin_id.to_string());
    }

    /// Clear the current plugin selection.
    pub fn clear_selection(&mut self) {
        self.selected_plugin = None;
    }

    /// Returns a reference to the currently selected plugin, if any.
    pub fn selected(&self) -> Option<&Plugin> {
        self.selected_plugin.as_ref().and_then(|id| self.plugins.iter().find(|p| p.id == *id))
    }

    /// Install a plugin (add to list).
    pub fn install(&mut self, plugin: Plugin) -> Result<(), PluginError> {
        if self.plugins.iter().any(|p| p.id == plugin.id) {
            return Err(PluginError::AlreadyInstalled(plugin.id));
        }
        self.plugins.push(plugin);
        Ok(())
    }

    /// Enable a plugin by ID.
    pub fn enable(&mut self, plugin_id: &str) -> Result<(), PluginError> {
        let plugin = self
            .plugins
            .iter_mut()
            .find(|p| p.id == plugin_id)
            .ok_or_else(|| PluginError::NotFound(plugin_id.to_string()))?;
        plugin.enable()
    }

    /// Disable a plugin by ID.
    pub fn disable(&mut self, plugin_id: &str) -> Result<(), PluginError> {
        let plugin = self
            .plugins
            .iter_mut()
            .find(|p| p.id == plugin_id)
            .ok_or_else(|| PluginError::NotFound(plugin_id.to_string()))?;
        plugin.disable();
        Ok(())
    }

    /// Uninstall a plugin by ID (remove from list).
    pub fn uninstall(&mut self, plugin_id: &str) -> Result<(), PluginError> {
        let index = self
            .plugins
            .iter()
            .position(|p| p.id == plugin_id)
            .ok_or_else(|| PluginError::NotFound(plugin_id.to_string()))?;

        self.plugins.remove(index);

        // Clear selection if the uninstalled plugin was selected
        if self.selected_plugin.as_deref() == Some(plugin_id) {
            self.selected_plugin = None;
        }

        Ok(())
    }

    /// Count of enabled plugins.
    pub fn enabled_count(&self) -> usize {
        self.plugins.iter().filter(|p| p.is_active()).count()
    }

    /// Count of plugins in error state.
    pub fn error_count(&self) -> usize {
        self.plugins.iter().filter(|p| p.is_error()).count()
    }

    /// Returns plugins filtered by state.
    pub fn plugins_by_state(&self, state: &PluginState) -> Vec<&Plugin> {
        self.plugins.iter().filter(|p| &p.state == state).collect()
    }
}

// ---------------------------------------------------------------------------
// GPUI Render (T082)
// ---------------------------------------------------------------------------

/// Precomputed colors for rendering the plugin manager view.
#[allow(dead_code)]
struct PluginManagerColors {
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

/// View wrapper for `PluginManagerState` with theme for rendering.
pub struct PluginManagerViewComponent {
    pub state: PluginManagerState,
    pub theme: Theme,
}

impl PluginManagerViewComponent {
    pub fn new(state: PluginManagerState, theme: Theme) -> Self {
        Self { state, theme }
    }

    /// Returns the theme color for a given plugin state.
    pub fn state_color(&self, state: &PluginState) -> crate::theme::Color {
        match state {
            PluginState::Enabled => self.theme.colors.success,
            PluginState::Disabled => self.theme.colors.text_muted,
            PluginState::Error(_) => self.theme.colors.error,
            PluginState::Installed => self.theme.colors.accent,
        }
    }

    /// Returns a display label for a plugin state.
    pub fn state_label(state: &PluginState) -> &'static str {
        match state {
            PluginState::Enabled => "Enabled",
            PluginState::Disabled => "Disabled",
            PluginState::Error(_) => "Error",
            PluginState::Installed => "Installed",
        }
    }

    /// Returns a display label for a plugin permission.
    pub fn permission_label(permission: &PluginPermission) -> &'static str {
        match permission {
            PluginPermission::ReadResources => "Read Resources",
            PluginPermission::WriteResources => "Write Resources",
            PluginPermission::RegisterViews => "Register Views",
            PluginPermission::RegisterActions => "Register Actions",
            PluginPermission::RegisterSidebar => "Register Sidebar",
            PluginPermission::NetworkAccess => "Network Access",
        }
    }

    /// Returns a color for a permission badge.
    fn permission_color(&self, permission: &PluginPermission) -> Rgba {
        match permission {
            PluginPermission::ReadResources => self.theme.colors.info.to_gpui(),
            PluginPermission::WriteResources => self.theme.colors.warning.to_gpui(),
            PluginPermission::NetworkAccess => self.theme.colors.error.to_gpui(),
            PluginPermission::RegisterViews
            | PluginPermission::RegisterActions
            | PluginPermission::RegisterSidebar => self.theme.colors.accent.to_gpui(),
        }
    }

    /// Toolbar: plugin count badges and filter/search.
    fn render_toolbar(&self, colors: &PluginManagerColors) -> gpui::Div {
        let total = self.state.plugins.len();
        let enabled = self.state.enabled_count();
        let error = self.state.error_count();

        let total_lbl = SharedString::from(format!("Total: {total}"));
        let enabled_lbl = SharedString::from(format!("Enabled: {enabled}"));
        let error_lbl = SharedString::from(format!("Errors: {error}"));

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
                    .text_sm()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(colors.text_primary)
                    .child("Plugins"),
            )
            .child(div().flex_1())
            .child(self.render_count_badge(total_lbl, colors.accent, colors))
            .child(self.render_count_badge(enabled_lbl, colors.success, colors))
            .child(self.render_count_badge(error_lbl, colors.error, colors))
    }

    /// Small count badge.
    fn render_count_badge(
        &self,
        label: SharedString,
        dot_color: Rgba,
        colors: &PluginManagerColors,
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

    /// Plugin list: grid or list of plugin cards.
    fn render_plugin_list(&self, colors: &PluginManagerColors) -> gpui::Div {
        if self.state.plugins.is_empty() && !self.state.loading {
            return self.render_empty_state(colors);
        }

        let mut list =
            div().flex().flex_col().flex_1().w_full().overflow_hidden().gap(px(8.0)).px_3().py_3();

        for plugin in &self.state.plugins {
            let is_selected =
                self.state.selected_plugin.as_ref().map(|id| id == &plugin.id).unwrap_or(false);
            list = list.child(self.render_plugin_card(plugin, is_selected, colors));
        }

        list
    }

    /// Single plugin card with all info.
    fn render_plugin_card(
        &self,
        plugin: &Plugin,
        selected: bool,
        colors: &PluginManagerColors,
    ) -> gpui::Stateful<gpui::Div> {
        let state_color = self.state_color(&plugin.state).to_gpui();
        let state_label = SharedString::from(Self::state_label(&plugin.state));

        let card_id = format!("plugin-card-{}", plugin.id);
        let bg = if selected { colors.selection } else { colors.surface };

        div()
            .id(ElementId::Name(SharedString::from(card_id)))
            .flex()
            .flex_col()
            .w_full()
            .p_3()
            .rounded(px(8.0))
            .bg(bg)
            .border_1()
            .border_color(colors.border)
            .cursor_pointer()
            .when(selected, |el| el.border_l_4().border_color(colors.accent))
            .child(self.render_plugin_header(plugin, state_label, state_color, colors))
            .child(self.render_plugin_description(&plugin.manifest.description, colors))
            .child(self.render_plugin_permissions(&plugin.manifest.permissions, colors))
            .child(self.render_plugin_actions(plugin, colors))
    }

    /// Plugin card header: name, version, state badge.
    fn render_plugin_header(
        &self,
        plugin: &Plugin,
        state_label: SharedString,
        state_color: Rgba,
        colors: &PluginManagerColors,
    ) -> gpui::Div {
        div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .gap(px(8.0))
            .mb_2()
            .child(
                div()
                    .text_base()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(colors.text_primary)
                    .child(SharedString::from(plugin.manifest.name.clone())),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .child(SharedString::from(format!("v{}", plugin.manifest.version))),
            )
            .child(div().flex_1())
            .child(self.render_state_badge(state_label, state_color))
    }

    /// State badge: colored dot + label.
    fn render_state_badge(&self, label: SharedString, color: Rgba) -> gpui::Div {
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.0))
            .px_2()
            .py_1()
            .rounded(px(4.0))
            .bg(gpui::Rgba { r: color.r, g: color.g, b: color.b, a: 0.1 })
            .child(div().w(px(6.0)).h(px(6.0)).rounded(px(3.0)).bg(color))
            .child(div().text_xs().text_color(color).child(label))
    }

    /// Plugin description text.
    fn render_plugin_description(
        &self,
        description: &str,
        colors: &PluginManagerColors,
    ) -> gpui::Div {
        div()
            .text_sm()
            .text_color(colors.text_secondary)
            .mb_2()
            .child(SharedString::from(description.to_string()))
    }

    /// Plugin permissions as colored badges.
    fn render_plugin_permissions(
        &self,
        permissions: &[PluginPermission],
        _colors: &PluginManagerColors,
    ) -> gpui::Div {
        let mut row = div().flex().flex_row().flex_wrap().gap(px(4.0)).mb_2();

        for permission in permissions {
            row = row.child(self.render_permission_badge(permission));
        }

        row
    }

    /// Single permission badge.
    fn render_permission_badge(&self, permission: &PluginPermission) -> gpui::Div {
        let label = Self::permission_label(permission);
        let color = self.permission_color(permission);

        div()
            .px_2()
            .py_1()
            .rounded(px(4.0))
            .bg(gpui::Rgba { r: color.r, g: color.g, b: color.b, a: 0.1 })
            .border_1()
            .border_color(color)
            .text_xs()
            .text_color(color)
            .child(SharedString::from(label.to_string()))
    }

    /// Plugin action buttons: enable/disable toggle, uninstall.
    fn render_plugin_actions(&self, plugin: &Plugin, colors: &PluginManagerColors) -> gpui::Div {
        div()
            .flex()
            .flex_row()
            .gap(px(8.0))
            .child(self.render_state_toggle(&plugin.id, &plugin.state, colors))
            .child(self.render_uninstall_button(&plugin.id, colors))
    }

    /// Enable/disable toggle button.
    fn render_state_toggle(
        &self,
        plugin_id: &str,
        state: &PluginState,
        colors: &PluginManagerColors,
    ) -> gpui::Stateful<gpui::Div> {
        let (label, enabled) = match state {
            PluginState::Enabled => ("Disable", true),
            PluginState::Disabled | PluginState::Installed => ("Enable", false),
            PluginState::Error(_) => ("Enable", false),
        };

        let bg = if enabled { colors.success } else { colors.accent };

        let btn_id = format!("toggle-{plugin_id}");

        div()
            .id(ElementId::Name(SharedString::from(btn_id)))
            .px_3()
            .py_1()
            .rounded(px(4.0))
            .bg(bg)
            .cursor_pointer()
            .text_xs()
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(colors.background)
            .child(SharedString::from(label.to_string()))
    }

    /// Uninstall button.
    fn render_uninstall_button(
        &self,
        plugin_id: &str,
        colors: &PluginManagerColors,
    ) -> gpui::Stateful<gpui::Div> {
        let btn_id = format!("uninstall-{plugin_id}");

        div()
            .id(ElementId::Name(SharedString::from(btn_id)))
            .px_3()
            .py_1()
            .rounded(px(4.0))
            .bg(colors.surface)
            .border_1()
            .border_color(colors.error)
            .cursor_pointer()
            .text_xs()
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(colors.error)
            .child("Uninstall")
    }

    /// Detail panel for selected plugin.
    fn render_detail_panel(&self, colors: &PluginManagerColors) -> Option<gpui::Div> {
        let plugin = self.state.selected()?;

        let installed_at = plugin.installed_at.format("%Y-%m-%d %H:%M:%S").to_string();

        Some(
            div()
                .flex()
                .flex_col()
                .w_full()
                .p_4()
                .border_t_1()
                .border_color(colors.border)
                .child(
                    div()
                        .text_lg()
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(colors.text_primary)
                        .mb_3()
                        .child(SharedString::from(plugin.manifest.name.clone())),
                )
                .child(self.render_detail_row("ID", &plugin.id, colors))
                .child(self.render_detail_row("Version", &plugin.manifest.version, colors))
                .child(self.render_detail_row("Author", &plugin.manifest.author, colors))
                .child(self.render_detail_row("Description", &plugin.manifest.description, colors))
                .child(self.render_detail_row("Installed", &installed_at, colors))
                .child(self.render_detail_row("Library Path", &plugin.library_path, colors))
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(colors.text_primary)
                        .mt_3()
                        .mb_2()
                        .child("Permissions"),
                )
                .child(self.render_plugin_permissions(&plugin.manifest.permissions, colors)),
        )
    }

    /// Single detail row.
    fn render_detail_row(
        &self,
        label: &str,
        value: &str,
        colors: &PluginManagerColors,
    ) -> gpui::Div {
        div()
            .flex()
            .flex_row()
            .gap(px(8.0))
            .mb_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(colors.text_secondary)
                    .child(SharedString::from(format!("{label}:"))),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(colors.text_primary)
                    .child(SharedString::from(value.to_string())),
            )
    }

    /// Empty state when no plugins installed.
    fn render_empty_state(&self, colors: &PluginManagerColors) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .items_center()
            .justify_center()
            .child(div().text_sm().text_color(colors.text_muted).child("No plugins installed"))
            .child(
                div()
                    .text_xs()
                    .text_color(colors.text_muted)
                    .mt_2()
                    .child("Browse the plugin catalog to get started"),
            )
    }

    /// Loading indicator.
    fn render_loading(&self, colors: &PluginManagerColors) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .items_center()
            .justify_center()
            .child(div().text_sm().text_color(colors.text_muted).child("Loading plugins..."))
    }

    /// Error message display.
    fn render_error(&self, colors: &PluginManagerColors) -> gpui::Div {
        let msg = self.state.error.as_deref().unwrap_or("Unknown error");
        div().flex().flex_col().flex_1().items_center().justify_center().px_4().child(
            div().text_sm().text_color(colors.error).child(SharedString::from(msg.to_string())),
        )
    }
}

impl Render for PluginManagerViewComponent {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let colors = PluginManagerColors {
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
            root = root.child(self.render_plugin_list(&colors));

            if let Some(detail_panel) = self.render_detail_panel(&colors) {
                root = root.child(detail_panel);
            }
        }

        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use baeus_plugins::{PluginManifest, PluginPermission};

    fn sample_manifest(id: &str, name: &str) -> PluginManifest {
        PluginManifest {
            id: id.to_string(),
            name: name.to_string(),
            version: "1.0.0".to_string(),
            description: format!("{name} plugin"),
            author: "Test Author".to_string(),
            min_app_version: "0.1.0".to_string(),
            permissions: vec![PluginPermission::ReadResources],
        }
    }

    fn sample_plugin(id: &str, name: &str) -> Plugin {
        Plugin::new(sample_manifest(id, name), format!("/plugins/{id}.dylib"))
    }

    fn sample_plugins() -> Vec<Plugin> {
        let mut p1 = sample_plugin("io.example.metrics", "Metrics");
        p1.enable().unwrap();
        let mut p2 = sample_plugin("io.example.gitops", "GitOps");
        p2.enable().unwrap();
        let p3 = sample_plugin("io.example.costview", "Cost View");
        // p3 stays in Installed state

        vec![p1, p2, p3]
    }

    // --- T130: Plugin manager view tests ---

    #[test]
    fn test_default_state() {
        let state = PluginManagerState::default();
        assert!(state.plugins.is_empty());
        assert!(state.selected_plugin.is_none());
        assert!(!state.loading);
        assert!(state.error.is_none());
    }

    #[test]
    fn test_set_plugins() {
        let mut state = PluginManagerState {
            loading: true,
            error: Some("old error".to_string()),
            ..Default::default()
        };

        state.set_plugins(sample_plugins());

        assert_eq!(state.plugins.len(), 3);
        assert!(!state.loading);
        assert!(state.error.is_none());
    }

    #[test]
    fn test_set_loading() {
        let mut state = PluginManagerState { error: Some("some error".to_string()), ..Default::default() };

        state.set_loading(true);
        assert!(state.loading);
        assert!(state.error.is_none());

        state.set_loading(false);
        assert!(!state.loading);
    }

    #[test]
    fn test_set_error() {
        let mut state = PluginManagerState { loading: true, ..Default::default() };

        state.set_error("load failed".to_string());
        assert_eq!(state.error.as_deref(), Some("load failed"));
        assert!(!state.loading);
    }

    #[test]
    fn test_select_plugin() {
        let mut state = PluginManagerState::default();
        state.set_plugins(sample_plugins());

        state.select_plugin("io.example.metrics");
        assert_eq!(state.selected_plugin.as_deref(), Some("io.example.metrics"));

        let selected = state.selected().unwrap();
        assert_eq!(selected.manifest.name, "Metrics");
    }

    #[test]
    fn test_select_plugin_not_found() {
        let mut state = PluginManagerState::default();
        state.set_plugins(sample_plugins());

        state.select_plugin("nonexistent");
        assert_eq!(state.selected_plugin.as_deref(), Some("nonexistent"));
        assert!(state.selected().is_none());
    }

    #[test]
    fn test_clear_selection() {
        let mut state = PluginManagerState::default();
        state.set_plugins(sample_plugins());
        state.select_plugin("io.example.metrics");
        assert!(state.selected_plugin.is_some());

        state.clear_selection();
        assert!(state.selected_plugin.is_none());
        assert!(state.selected().is_none());
    }

    #[test]
    fn test_install_plugin() {
        let mut state = PluginManagerState::default();

        let plugin = sample_plugin("io.example.new", "New Plugin");
        state.install(plugin).unwrap();

        assert_eq!(state.plugins.len(), 1);
        assert_eq!(state.plugins[0].id, "io.example.new");
    }

    #[test]
    fn test_install_duplicate_rejected() {
        let mut state = PluginManagerState::default();
        state.install(sample_plugin("io.example.dup", "Dup")).unwrap();

        let result = state.install(sample_plugin("io.example.dup", "Dup Again"));
        assert!(result.is_err());
        match result.unwrap_err() {
            PluginError::AlreadyInstalled(id) => assert_eq!(id, "io.example.dup"),
            other => panic!("expected AlreadyInstalled, got {:?}", other),
        }
    }

    #[test]
    fn test_enable_plugin() {
        let mut state = PluginManagerState::default();
        state.set_plugins(sample_plugins());

        // costview is in Installed state
        state.enable("io.example.costview").unwrap();
        let plugin = state.plugins.iter().find(|p| p.id == "io.example.costview").unwrap();
        assert!(plugin.is_active());
    }

    #[test]
    fn test_enable_not_found() {
        let mut state = PluginManagerState::default();
        let result = state.enable("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_disable_plugin() {
        let mut state = PluginManagerState::default();
        state.set_plugins(sample_plugins());

        // metrics is Enabled
        state.disable("io.example.metrics").unwrap();
        let plugin = state.plugins.iter().find(|p| p.id == "io.example.metrics").unwrap();
        assert_eq!(plugin.state, PluginState::Disabled);
    }

    #[test]
    fn test_disable_not_found() {
        let mut state = PluginManagerState::default();
        let result = state.disable("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_uninstall_plugin() {
        let mut state = PluginManagerState::default();
        state.set_plugins(sample_plugins());
        assert_eq!(state.plugins.len(), 3);

        state.uninstall("io.example.gitops").unwrap();
        assert_eq!(state.plugins.len(), 2);
        assert!(state.plugins.iter().all(|p| p.id != "io.example.gitops"));
    }

    #[test]
    fn test_uninstall_clears_selection() {
        let mut state = PluginManagerState::default();
        state.set_plugins(sample_plugins());
        state.select_plugin("io.example.gitops");

        state.uninstall("io.example.gitops").unwrap();
        assert!(state.selected_plugin.is_none());
    }

    #[test]
    fn test_uninstall_preserves_other_selection() {
        let mut state = PluginManagerState::default();
        state.set_plugins(sample_plugins());
        state.select_plugin("io.example.metrics");

        state.uninstall("io.example.gitops").unwrap();
        // Metrics selection should still be there
        assert_eq!(state.selected_plugin.as_deref(), Some("io.example.metrics"));
    }

    #[test]
    fn test_uninstall_not_found() {
        let mut state = PluginManagerState::default();
        let result = state.uninstall("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_enabled_count() {
        let mut state = PluginManagerState::default();
        state.set_plugins(sample_plugins());

        // metrics and gitops are enabled, costview is installed
        assert_eq!(state.enabled_count(), 2);
    }

    #[test]
    fn test_error_count() {
        let mut state = PluginManagerState::default();
        let mut plugins = sample_plugins();
        plugins[2].set_error("load failure".to_string());
        state.set_plugins(plugins);

        assert_eq!(state.error_count(), 1);
    }

    #[test]
    fn test_plugins_by_state() {
        let mut state = PluginManagerState::default();
        state.set_plugins(sample_plugins());

        let enabled = state.plugins_by_state(&PluginState::Enabled);
        assert_eq!(enabled.len(), 2);

        let installed = state.plugins_by_state(&PluginState::Installed);
        assert_eq!(installed.len(), 1);
    }

    #[test]
    fn test_full_lifecycle() {
        let mut state = PluginManagerState::default();

        // Start loading
        state.set_loading(true);
        assert!(state.loading);

        // Receive plugins
        state.set_plugins(sample_plugins());
        assert!(!state.loading);
        assert_eq!(state.plugins.len(), 3);

        // Install new plugin
        state.install(sample_plugin("io.example.new", "New")).unwrap();
        assert_eq!(state.plugins.len(), 4);

        // Enable it
        state.enable("io.example.new").unwrap();
        assert_eq!(state.enabled_count(), 3);

        // Select it
        state.select_plugin("io.example.new");
        let selected = state.selected().unwrap();
        assert_eq!(selected.manifest.name, "New");

        // Disable it
        state.disable("io.example.new").unwrap();
        assert_eq!(state.enabled_count(), 2);

        // Uninstall it
        state.uninstall("io.example.new").unwrap();
        assert_eq!(state.plugins.len(), 3);
        assert!(state.selected_plugin.is_none());
    }

    #[test]
    fn test_error_workflow() {
        let mut state = PluginManagerState::default();

        state.set_loading(true);
        assert!(state.loading);

        state.set_error("failed to scan directory".to_string());
        assert!(!state.loading);
        assert_eq!(state.error.as_deref(), Some("failed to scan directory"));
        assert!(state.plugins.is_empty());
    }

    #[test]
    fn test_set_loading_false_does_not_clear_error() {
        let mut state = PluginManagerState::default();
        state.set_error("some error".to_string());

        state.set_loading(false);
        assert!(!state.loading);
        assert!(state.error.is_some());
    }
}
