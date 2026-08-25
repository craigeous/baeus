use crate::components::editor_view::EditorViewState;
use crate::theme::Theme;
use gpui::{Context, ElementId, Rgba, SharedString, Window, div, prelude::*, px};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Tabs available in the resource detail view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DetailTab {
    Overview,
    Spec,
    Status,
    Conditions,
    Events,
    Yaml,
    Logs,        // Pod only
    Terminal,    // Pod only
    PortForward, // Pod and Service
}

impl DetailTab {
    /// Returns a human-readable label for this tab.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Spec => "Spec",
            Self::Status => "Status",
            Self::Conditions => "Conditions",
            Self::Events => "Events",
            Self::Yaml => "YAML",
            Self::Logs => "Logs",
            Self::Terminal => "Terminal",
            Self::PortForward => "Port Forward",
        }
    }
}

/// Returns the available tabs for a given resource kind.
pub fn tabs_for_kind(kind: &str) -> Vec<DetailTab> {
    let mut tabs = vec![
        DetailTab::Overview,
        DetailTab::Spec,
        DetailTab::Status,
        DetailTab::Conditions,
        DetailTab::Events,
        DetailTab::Yaml,
    ];
    match kind {
        "Pod" => {
            tabs.push(DetailTab::Logs);
            tabs.push(DetailTab::Terminal);
            tabs.push(DetailTab::PortForward);
        }
        "Service" => {
            tabs.push(DetailTab::PortForward);
        }
        _ => {}
    }
    tabs
}

/// Whether an exec action requires user confirmation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecConfirmation {
    /// Exec requires confirmation before proceeding.
    Required { message: String },
    /// Already confirmed by user.
    Confirmed,
}

/// State for the exec confirmation dialog within the detail view.
#[derive(Debug)]
pub struct ExecConfirmState {
    pub pod_name: String,
    pub container_name: Option<String>,
    pub confirmation: ExecConfirmation,
}

impl ExecConfirmState {
    /// Create a new exec confirmation prompt.
    pub fn new(pod_name: &str, container_name: Option<&str>) -> Self {
        let container_info =
            container_name.map(|c| format!(" (container: {c})")).unwrap_or_default();
        Self {
            pod_name: pod_name.to_string(),
            container_name: container_name.map(|s| s.to_string()),
            confirmation: ExecConfirmation::Required {
                message: format!(
                    "Open terminal session to {pod_name}{container_info}? This grants shell access."
                ),
            },
        }
    }

    /// Confirm the exec action.
    pub fn confirm(&mut self) {
        self.confirmation = ExecConfirmation::Confirmed;
    }

    /// Returns true if exec has been confirmed.
    pub fn is_confirmed(&self) -> bool {
        self.confirmation == ExecConfirmation::Confirmed
    }
}

/// Whether a port-forward action requires user confirmation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortForwardConfirmation {
    /// Port-forward requires confirmation before proceeding.
    Required { message: String },
    /// Already confirmed by user.
    Confirmed,
}

/// State for the port-forward confirmation dialog within the detail view.
#[derive(Debug)]
pub struct PortForwardConfirmState {
    pub resource_name: String,
    pub resource_kind: String,
    pub local_port: Option<u16>,
    pub remote_port: Option<u16>,
    pub confirmation: PortForwardConfirmation,
}

impl PortForwardConfirmState {
    /// Create a new port-forward confirmation prompt.
    pub fn new(
        resource_kind: &str,
        resource_name: &str,
        local_port: Option<u16>,
        remote_port: Option<u16>,
    ) -> Self {
        let port_info = match (local_port, remote_port) {
            (Some(l), Some(r)) => format!(" (local:{l} -> remote:{r})"),
            (None, Some(r)) => format!(" (remote:{r})"),
            _ => String::new(),
        };
        Self {
            resource_name: resource_name.to_string(),
            resource_kind: resource_kind.to_string(),
            local_port,
            remote_port,
            confirmation: PortForwardConfirmation::Required {
                message: format!(
                    "Open port-forward to {resource_kind}/{resource_name}{port_info}? This exposes the target port on your local machine."
                ),
            },
        }
    }

    /// Confirm the port-forward action.
    pub fn confirm(&mut self) {
        self.confirmation = PortForwardConfirmation::Confirmed;
    }

    /// Returns true if port-forward has been confirmed.
    pub fn is_confirmed(&self) -> bool {
        self.confirmation == PortForwardConfirmation::Confirmed
    }
}

/// Represents a single condition displayed in the detail view.
#[derive(Debug, Clone)]
pub struct ConditionDisplay {
    pub type_name: String,
    pub status: String,
    pub reason: Option<String>,
    pub message: Option<String>,
    pub age: String,
}

/// Represents a single event displayed in the detail view.
#[derive(Debug, Clone)]
pub struct EventDisplay {
    pub type_name: String,
    pub reason: String,
    pub message: String,
    pub age: String,
    pub count: u32,
}

/// Represents a related resource linked to the current resource.
#[derive(Debug, Clone)]
pub struct RelatedResource {
    pub kind: String,
    pub name: String,
    pub namespace: Option<String>,
    pub relationship: String,
}

/// State for the resource detail view, showing detailed information about a
/// single resource with tabs.
#[derive(Debug)]
pub struct ResourceDetailState {
    pub kind: String,
    pub name: String,
    pub namespace: Option<String>,
    pub uid: Option<String>,
    pub active_tab: DetailTab,
    pub available_tabs: Vec<DetailTab>,
    pub loading: bool,
    pub error: Option<String>,
    pub spec_json: Option<String>,
    pub status_json: Option<String>,
    pub conditions: Vec<ConditionDisplay>,
    pub events: Vec<EventDisplay>,
    pub related_resources: Vec<RelatedResource>,
    /// Exec confirmation dialog state (T075).
    pub exec_confirm: Option<ExecConfirmState>,
    /// Port-forward confirmation dialog state (T137 security hardening).
    pub port_forward_confirm: Option<PortForwardConfirmState>,
    /// Container names for this pod (populated when kind == Pod).
    pub container_names: Vec<String>,
    /// YAML editor state for the YAML tab (T084).
    pub yaml_editor: Option<EditorViewState>,
    /// The full resource YAML for serialization to the YAML editor.
    pub resource_yaml: Option<String>,
    /// The resource_version from the K8s API for optimistic concurrency.
    pub resource_version: Option<String>,
}

impl ResourceDetailState {
    /// Creates a new resource detail state for the given resource.
    pub fn new(kind: &str, name: &str, namespace: Option<String>) -> Self {
        let available_tabs = tabs_for_kind(kind);
        Self {
            kind: kind.to_string(),
            name: name.to_string(),
            namespace,
            uid: None,
            active_tab: DetailTab::Overview,
            available_tabs,
            loading: false,
            error: None,
            spec_json: None,
            status_json: None,
            conditions: Vec::new(),
            events: Vec::new(),
            related_resources: Vec::new(),
            exec_confirm: None,
            port_forward_confirm: None,
            container_names: Vec::new(),
            yaml_editor: None,
            resource_yaml: None,
            resource_version: None,
        }
    }

    /// Switches to the given tab.
    pub fn switch_tab(&mut self, tab: DetailTab) {
        self.active_tab = tab;
    }

    /// Sets the loading state.
    pub fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
    }

    /// Sets an error message.
    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
    }

    /// Clears the current error.
    pub fn clear_error(&mut self) {
        self.error = None;
    }

    /// Sets the pretty-printed spec JSON.
    pub fn set_spec(&mut self, json: String) {
        self.spec_json = Some(json);
    }

    /// Sets the pretty-printed status JSON.
    pub fn set_status(&mut self, json: String) {
        self.status_json = Some(json);
    }

    /// Sets the list of conditions.
    pub fn set_conditions(&mut self, conditions: Vec<ConditionDisplay>) {
        self.conditions = conditions;
    }

    /// Sets the list of events.
    pub fn set_events(&mut self, events: Vec<EventDisplay>) {
        self.events = events;
    }

    /// Adds a related resource.
    pub fn add_related(&mut self, related: RelatedResource) {
        self.related_resources.push(related);
    }

    /// Returns true if this resource is a Pod.
    pub fn is_pod(&self) -> bool {
        self.kind == "Pod"
    }

    /// Returns true if this resource has any conditions.
    pub fn has_conditions(&self) -> bool {
        !self.conditions.is_empty()
    }

    /// Returns the number of warning events.
    pub fn warning_event_count(&self) -> usize {
        self.events.iter().filter(|e| e.type_name == "Warning").count()
    }

    // --- T075: Logs and Terminal tab wiring ---

    /// Set the container names for a pod resource.
    pub fn set_container_names(&mut self, names: Vec<String>) {
        self.container_names = names;
    }

    /// Open an exec confirmation dialog before launching terminal.
    pub fn request_exec(&mut self, container_name: Option<&str>) {
        self.exec_confirm = Some(ExecConfirmState::new(&self.name, container_name));
    }

    /// Confirm the exec action.
    pub fn confirm_exec(&mut self) {
        if let Some(ref mut confirm) = self.exec_confirm {
            confirm.confirm();
        }
    }

    /// Cancel/dismiss the exec confirmation.
    pub fn cancel_exec(&mut self) {
        self.exec_confirm = None;
    }

    /// Returns true if there is a pending exec confirmation.
    pub fn has_exec_confirm(&self) -> bool {
        self.exec_confirm.is_some()
    }

    /// Returns true if exec has been confirmed and terminal can be opened.
    pub fn is_exec_confirmed(&self) -> bool {
        self.exec_confirm.as_ref().is_some_and(|c| c.is_confirmed())
    }

    /// Switch to the Logs tab (convenience).
    pub fn open_logs(&mut self) {
        if self.available_tabs.contains(&DetailTab::Logs) {
            self.active_tab = DetailTab::Logs;
        }
    }

    /// Switch to the Terminal tab with exec confirmation (convenience).
    pub fn open_terminal(&mut self, container_name: Option<&str>) {
        if self.available_tabs.contains(&DetailTab::Terminal) {
            self.active_tab = DetailTab::Terminal;
            self.request_exec(container_name);
        }
    }

    /// Switch to the Port Forward tab with confirmation (convenience).
    pub fn open_port_forward(&mut self) {
        if self.available_tabs.contains(&DetailTab::PortForward) {
            self.active_tab = DetailTab::PortForward;
            self.request_port_forward(None, None);
        }
    }

    /// Returns true if this resource supports port forwarding (Pod or Service).
    pub fn supports_port_forward(&self) -> bool {
        self.available_tabs.contains(&DetailTab::PortForward)
    }

    /// Open a port-forward confirmation dialog before establishing connection.
    pub fn request_port_forward(&mut self, local_port: Option<u16>, remote_port: Option<u16>) {
        self.port_forward_confirm =
            Some(PortForwardConfirmState::new(&self.kind, &self.name, local_port, remote_port));
    }

    /// Confirm the port-forward action.
    pub fn confirm_port_forward(&mut self) {
        if let Some(ref mut confirm) = self.port_forward_confirm {
            confirm.confirm();
        }
    }

    /// Cancel/dismiss the port-forward confirmation.
    pub fn cancel_port_forward(&mut self) {
        self.port_forward_confirm = None;
    }

    /// Returns true if there is a pending port-forward confirmation.
    pub fn has_port_forward_confirm(&self) -> bool {
        self.port_forward_confirm.is_some()
    }

    /// Returns true if port-forward has been confirmed.
    pub fn is_port_forward_confirmed(&self) -> bool {
        self.port_forward_confirm.as_ref().is_some_and(|c| c.is_confirmed())
    }

    // --- T084: YAML tab wiring ---

    /// Set the resource YAML content and resource version for the YAML editor.
    pub fn set_resource_yaml(&mut self, yaml: String, resource_version: String) {
        self.resource_yaml = Some(yaml);
        self.resource_version = Some(resource_version);
        // Reset the editor if it was previously open so it refreshes
        self.yaml_editor = None;
    }

    /// Open the YAML tab, initializing the editor with the current resource YAML.
    pub fn open_yaml_editor(&mut self) {
        if !self.available_tabs.contains(&DetailTab::Yaml) {
            return;
        }
        self.active_tab = DetailTab::Yaml;

        // Initialize the editor if we have YAML content and it hasn't been created yet
        if self.yaml_editor.is_none() {
            if let (Some(yaml), Some(rv)) = (&self.resource_yaml, &self.resource_version) {
                self.yaml_editor = Some(EditorViewState::new(
                    yaml,
                    &self.kind,
                    &self.name,
                    self.namespace.clone(),
                    rv,
                ));
            }
        }
    }

    /// Returns true if the YAML editor is initialized and ready.
    pub fn has_yaml_editor(&self) -> bool {
        self.yaml_editor.is_some()
    }

    /// Returns a reference to the YAML editor state, if initialized.
    pub fn yaml_editor_ref(&self) -> Option<&EditorViewState> {
        self.yaml_editor.as_ref()
    }

    /// Returns a mutable reference to the YAML editor state, if initialized.
    pub fn yaml_editor_mut(&mut self) -> Option<&mut EditorViewState> {
        self.yaml_editor.as_mut()
    }

    /// Handle a successful apply from the YAML editor: update internal state.
    pub fn on_yaml_apply_success(&mut self, new_yaml: String, new_resource_version: String) {
        self.resource_yaml = Some(new_yaml);
        self.resource_version = Some(new_resource_version.clone());
        if let Some(editor) = &mut self.yaml_editor {
            editor.apply_success(&new_resource_version);
        }
    }

    /// Handle a 409 Conflict from the YAML editor: trigger conflict resolution.
    pub fn on_yaml_apply_conflict(&mut self, server_yaml: String) {
        if let Some(editor) = &mut self.yaml_editor {
            editor.apply_conflict(server_yaml);
        }
    }

    /// Handle a generic failure from the YAML editor.
    pub fn on_yaml_apply_failure(&mut self, error: String) {
        if let Some(editor) = &mut self.yaml_editor {
            editor.apply_failure(error);
        }
    }
}

// ---------------------------------------------------------------------------
// GPUI Render
// ---------------------------------------------------------------------------

/// View wrapper for `ResourceDetailState` that holds a theme for rendering.
pub struct ResourceDetailView {
    pub state: ResourceDetailState,
    pub theme: Theme,
}

impl ResourceDetailView {
    pub fn new(state: ResourceDetailState, theme: Theme) -> Self {
        Self { state, theme }
    }

    /// Returns the active tab label.
    pub fn active_tab_label(&self) -> &'static str {
        self.state.active_tab.label()
    }

    /// Returns the tab labels for the tab bar.
    pub fn tab_labels(&self) -> Vec<&'static str> {
        self.state.available_tabs.iter().map(|t| t.label()).collect()
    }

    /// Render the tab bar.
    fn render_tab_bar(&self, colors: &DetailColors) -> gpui::Div {
        let mut bar = div()
            .flex()
            .flex_row()
            .w_full()
            .border_b_1()
            .border_color(colors.border)
            .bg(colors.surface)
            .overflow_hidden();

        for (idx, tab) in self.state.available_tabs.iter().enumerate() {
            bar = bar.child(self.render_tab(tab, idx, colors));
        }

        bar
    }

    /// Render a single tab.
    fn render_tab(
        &self,
        tab: &DetailTab,
        idx: usize,
        colors: &DetailColors,
    ) -> gpui::Stateful<gpui::Div> {
        let is_active = *tab == self.state.active_tab;
        let label = SharedString::from(tab.label().to_string());
        let tab_id = ElementId::Name(SharedString::from(format!("tab-{idx}")));

        let text_color = if is_active { colors.accent } else { colors.text_secondary };

        let mut tab_div = div()
            .id(tab_id)
            .px_3()
            .py_2()
            .cursor_pointer()
            .text_sm()
            .text_color(text_color)
            .child(label);

        if is_active {
            tab_div = tab_div.border_b_2().border_color(colors.accent);
        }

        tab_div
    }

    /// Render the overview tab content.
    fn render_overview(&self, colors: &DetailColors) -> gpui::Div {
        let name = SharedString::from(self.state.name.clone());
        let kind = SharedString::from(self.state.kind.clone());
        let ns = self.state.namespace.as_deref().unwrap_or("(cluster-scoped)");
        let ns_text = SharedString::from(ns.to_string());

        let header = self.render_overview_header(&name, &kind, &ns_text, colors);
        let mut overview = div().flex().flex_col().w_full().p_4().gap(px(8.0)).child(header);

        // Related resources
        if !self.state.related_resources.is_empty() {
            overview = overview.child(self.render_related_resources(colors));
        }

        overview
    }

    /// Render the overview header section.
    fn render_overview_header(
        &self,
        name: &SharedString,
        kind: &SharedString,
        ns_text: &SharedString,
        colors: &DetailColors,
    ) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(colors.text_primary)
                    .child(name.clone()),
            )
            .child(
                div()
                    .flex()
                    .gap(px(8.0))
                    .child(div().text_sm().text_color(colors.text_secondary).child(kind.clone()))
                    .child(div().text_sm().text_color(colors.text_muted).child(ns_text.clone())),
            )
    }

    /// Render related resources list.
    fn render_related_resources(&self, colors: &DetailColors) -> gpui::Div {
        let mut section = div().flex().flex_col().gap(px(4.0)).child(
            div()
                .text_sm()
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(colors.text_primary)
                .child("Related Resources"),
        );

        for related in &self.state.related_resources {
            let text = SharedString::from(format!(
                "{}/{} ({})",
                related.kind, related.name, related.relationship
            ));
            section = section.child(div().text_sm().text_color(colors.text_secondary).child(text));
        }

        section
    }

    /// Render the JSON spec/status tab content.
    fn render_json_content(&self, json: Option<&str>, colors: &DetailColors) -> gpui::Div {
        let text = json.unwrap_or("(not available)");
        let content = SharedString::from(text.to_string());
        div().w_full().p_4().text_xs().text_color(colors.text_secondary).child(content)
    }

    /// Render the events tab content.
    fn render_events(&self, colors: &DetailColors) -> gpui::Div {
        let mut container = div().flex().flex_col().w_full().p_4().gap(px(4.0));

        if self.state.events.is_empty() {
            container =
                container.child(div().text_sm().text_color(colors.text_muted).child("No events"));
        } else {
            for event in &self.state.events {
                container = container.child(self.render_event_row(event, colors));
            }
        }

        container
    }

    /// Render a single event row.
    fn render_event_row(&self, event: &EventDisplay, colors: &DetailColors) -> gpui::Div {
        let type_color =
            if event.type_name == "Warning" { colors.warning } else { colors.text_secondary };

        let type_text = SharedString::from(event.type_name.clone());
        let reason = SharedString::from(event.reason.clone());
        let message = SharedString::from(event.message.clone());

        div()
            .flex()
            .flex_row()
            .gap(px(8.0))
            .py_1()
            .child(div().text_xs().text_color(type_color).w(px(60.0)).child(type_text))
            .child(div().text_xs().text_color(colors.text_primary).w(px(120.0)).child(reason))
            .child(div().flex_1().text_xs().text_color(colors.text_secondary).child(message))
    }

    /// Render the active tab content.
    fn render_tab_content(&self, colors: &DetailColors) -> gpui::Div {
        match &self.state.active_tab {
            DetailTab::Overview => self.render_overview(colors),
            DetailTab::Spec => self.render_json_content(self.state.spec_json.as_deref(), colors),
            DetailTab::Status => {
                self.render_json_content(self.state.status_json.as_deref(), colors)
            }
            DetailTab::Events => self.render_events(colors),
            DetailTab::Yaml => self.render_yaml_content(colors),
            DetailTab::Conditions => self.render_conditions(colors),
            _ => {
                let label = SharedString::from(format!(
                    "{} (not yet implemented)",
                    self.state.active_tab.label()
                ));
                div().p_4().text_sm().text_color(colors.text_muted).child(label)
            }
        }
    }

    /// Render raw YAML config in a scrollable monospace view.
    fn render_yaml_content(&self, colors: &DetailColors) -> gpui::Div {
        let text = self.state.resource_yaml.as_deref().unwrap_or("(not available)");
        let content = SharedString::from(text.to_string());
        div()
            .w_full()
            .flex_1()
            .overflow_hidden()
            .p_4()
            .text_xs()
            .font_family("monospace")
            .text_color(colors.text_secondary)
            .child(content)
    }

    /// Render the conditions table.
    fn render_conditions(&self, colors: &DetailColors) -> gpui::Div {
        if self.state.conditions.is_empty() {
            let msg = SharedString::from("No conditions available");
            return div().p_4().text_sm().text_color(colors.text_muted).child(msg);
        }

        let mut table = div().flex().flex_col().w_full().p_4().gap(px(4.0));

        // Header row
        let header = div()
            .flex()
            .flex_row()
            .gap(px(8.0))
            .pb(px(4.0))
            .border_b_1()
            .border_color(colors.border)
            .text_xs()
            .text_color(colors.text_muted)
            .child(div().w(px(140.0)).child(SharedString::from("Type")))
            .child(div().w(px(80.0)).child(SharedString::from("Status")))
            .child(div().w(px(120.0)).child(SharedString::from("Reason")))
            .child(div().w(px(80.0)).child(SharedString::from("Age")))
            .child(div().flex_1().child(SharedString::from("Message")));
        table = table.child(header);

        for cond in &self.state.conditions {
            let status_color = if cond.status == "True" {
                gpui::rgb(0x34D399) // green
            } else {
                colors.warning
            };
            let row =
                div()
                    .flex()
                    .flex_row()
                    .gap(px(8.0))
                    .py(px(2.0))
                    .text_xs()
                    .child(
                        div()
                            .w(px(140.0))
                            .text_color(colors.text_primary)
                            .child(SharedString::from(cond.type_name.clone())),
                    )
                    .child(
                        div()
                            .w(px(80.0))
                            .text_color(status_color)
                            .child(SharedString::from(cond.status.clone())),
                    )
                    .child(div().w(px(120.0)).text_color(colors.text_secondary).child(
                        SharedString::from(cond.reason.as_deref().unwrap_or("-").to_string()),
                    ))
                    .child(
                        div()
                            .w(px(80.0))
                            .text_color(colors.text_secondary)
                            .child(SharedString::from(cond.age.clone())),
                    )
                    .child(div().flex_1().text_color(colors.text_secondary).child(
                        SharedString::from(cond.message.as_deref().unwrap_or("-").to_string()),
                    ));
            table = table.child(row);
        }

        table
    }
}

/// Precomputed colors for rendering the resource detail view.
struct DetailColors {
    background: Rgba,
    surface: Rgba,
    border: Rgba,
    accent: Rgba,
    warning: Rgba,
    text_primary: Rgba,
    text_secondary: Rgba,
    text_muted: Rgba,
}

impl Render for ResourceDetailView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let colors = DetailColors {
            background: self.theme.colors.background.to_gpui(),
            surface: self.theme.colors.surface.to_gpui(),
            border: self.theme.colors.border.to_gpui(),
            accent: self.theme.colors.accent.to_gpui(),
            warning: self.theme.colors.warning.to_gpui(),
            text_primary: self.theme.colors.text_primary.to_gpui(),
            text_secondary: self.theme.colors.text_secondary.to_gpui(),
            text_muted: self.theme.colors.text_muted.to_gpui(),
        };

        let mut container = div().flex().flex_col().w_full().h_full().bg(colors.background);

        if self.state.loading {
            container = container.child(
                div()
                    .flex()
                    .justify_center()
                    .py_8()
                    .text_sm()
                    .text_color(colors.text_muted)
                    .child("Loading..."),
            );
        } else if let Some(ref error) = self.state.error {
            let msg = SharedString::from(error.clone());
            container = container.child(
                div()
                    .flex()
                    .justify_center()
                    .py_8()
                    .text_sm()
                    .text_color(self.theme.colors.error.to_gpui())
                    .child(msg),
            );
        } else {
            container = container
                .child(self.render_tab_bar(&colors))
                .child(self.render_tab_content(&colors));
        }

        container
    }
}

// ---------------------------------------------------------------------------
// T106: Service-specific detail types
// ---------------------------------------------------------------------------

/// The type of a Kubernetes Service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceType {
    ClusterIP,
    NodePort,
    LoadBalancer,
    ExternalName,
}

/// A single port exposed by a Kubernetes Service.
#[derive(Debug, Clone)]
pub struct ServicePort {
    pub name: Option<String>,
    pub protocol: String,
    pub port: u16,
    pub target_port: String,
    pub node_port: Option<u16>,
}

/// Service-specific detail information for the resource detail view.
#[derive(Debug, Clone)]
pub struct ServiceDetail {
    pub service_type: ServiceType,
    pub cluster_ip: Option<String>,
    pub external_ips: Vec<String>,
    pub ports: Vec<ServicePort>,
    pub selectors: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// T107: Ingress-specific detail types
// ---------------------------------------------------------------------------

/// A single path rule within an Ingress rule.
#[derive(Debug, Clone)]
pub struct IngressPath {
    pub path: String,
    pub path_type: String,
    pub backend_service: String,
    pub backend_port: u16,
}

/// A single Ingress rule for a given host.
#[derive(Debug, Clone)]
pub struct IngressRule {
    pub host: Option<String>,
    pub paths: Vec<IngressPath>,
}

/// TLS configuration for an Ingress resource.
#[derive(Debug, Clone)]
pub struct TlsConfig {
    pub hosts: Vec<String>,
    pub secret_name: Option<String>,
}

/// Ingress-specific detail information for the resource detail view.
#[derive(Debug, Clone)]
pub struct IngressDetail {
    pub rules: Vec<IngressRule>,
    pub default_backend: Option<String>,
    pub tls: Vec<TlsConfig>,
}

// ---------------------------------------------------------------------------
// T108: PVC-specific detail types
// ---------------------------------------------------------------------------

/// The status of a PersistentVolumeClaim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PvcStatus {
    Bound,
    Pending,
    Lost,
}

/// Access mode for a PersistentVolumeClaim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PvcAccessMode {
    ReadWriteOnce,
    ReadOnlyMany,
    ReadWriteMany,
}

/// PVC-specific detail information for the resource detail view.
#[derive(Debug, Clone)]
pub struct PvcDetail {
    pub status: PvcStatus,
    pub capacity: Option<String>,
    pub access_modes: Vec<PvcAccessMode>,
    pub storage_class_name: Option<String>,
    pub volume_name: Option<String>,
}
