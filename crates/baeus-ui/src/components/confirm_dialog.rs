use gpui::{Context, Rgba, SharedString, Window, div, prelude::*, px};
use serde::{Deserialize, Serialize};

use crate::theme::Theme;

/// Severity level for a confirmation dialog, which determines visual styling
/// and the degree of caution conveyed to the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DialogSeverity {
    Info,
    Warning,
    Destructive,
}

/// State representation for a confirmation dialog component.
///
/// Tracks visibility, messaging, severity, button labels, and an optional
/// resource name for context. Provides builder-style configuration and
/// factory methods for common Kubernetes operations.
#[derive(Debug, Clone)]
pub struct ConfirmDialogState {
    pub visible: bool,
    pub title: String,
    pub message: String,
    pub severity: DialogSeverity,
    pub confirm_label: String,
    pub cancel_label: String,
    pub resource_name: Option<String>,
}

impl ConfirmDialogState {
    /// Creates a new confirmation dialog in the hidden state.
    ///
    /// The dialog starts with default button labels ("Confirm" / "Cancel")
    /// and no resource name. Use builder methods to customize further.
    pub fn new(title: &str, message: &str, severity: DialogSeverity) -> Self {
        Self {
            visible: false,
            title: title.to_string(),
            message: message.to_string(),
            severity,
            confirm_label: "Confirm".to_string(),
            cancel_label: "Cancel".to_string(),
            resource_name: None,
        }
    }

    /// Makes the dialog visible.
    pub fn show(&mut self) {
        self.visible = true;
    }

    /// Hides the dialog.
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Sets a custom label for the confirm button, consuming and returning self for chaining.
    pub fn with_confirm_label(mut self, label: &str) -> Self {
        self.confirm_label = label.to_string();
        self
    }

    /// Sets a custom label for the cancel button, consuming and returning self for chaining.
    pub fn with_cancel_label(mut self, label: &str) -> Self {
        self.cancel_label = label.to_string();
        self
    }

    /// Sets the resource name for additional context, consuming and returning self for chaining.
    pub fn with_resource_name(mut self, name: &str) -> Self {
        self.resource_name = Some(name.to_string());
        self
    }

    /// Returns true if the dialog severity is [`DialogSeverity::Destructive`].
    pub fn is_destructive(&self) -> bool {
        self.severity == DialogSeverity::Destructive
    }

    /// Creates a pre-configured dialog for deleting a Kubernetes resource.
    ///
    /// Uses [`DialogSeverity::Destructive`] with a "Delete" confirm label
    /// and includes the resource kind and name in the title and message.
    pub fn delete_resource(kind: &str, name: &str) -> Self {
        Self::new(
            &format!("Delete {kind}"),
            &format!(
                "Are you sure you want to delete {kind} \"{name}\"? This action cannot be undone."
            ),
            DialogSeverity::Destructive,
        )
        .with_confirm_label("Delete")
        .with_resource_name(name)
    }

    /// Creates a pre-configured dialog for scaling a Kubernetes resource.
    ///
    /// Uses [`DialogSeverity::Warning`] with a "Scale" confirm label
    /// and includes the target replica count in the message.
    pub fn scale_resource(kind: &str, name: &str, replicas: u32) -> Self {
        Self::new(
            &format!("Scale {kind}"),
            &format!("Scale {kind} \"{name}\" to {replicas} replicas?"),
            DialogSeverity::Warning,
        )
        .with_confirm_label("Scale")
        .with_resource_name(name)
    }

    /// Creates a pre-configured dialog for restarting a Kubernetes resource.
    ///
    /// Uses [`DialogSeverity::Warning`] with a "Restart" confirm label
    /// and warns about temporary downtime.
    pub fn restart_resource(kind: &str, name: &str) -> Self {
        Self::new(
            &format!("Restart {kind}"),
            &format!(
                "Are you sure you want to restart {kind} \"{name}\"? This may cause temporary downtime."
            ),
            DialogSeverity::Warning,
        )
        .with_confirm_label("Restart")
        .with_resource_name(name)
    }

    /// Creates a pre-configured dialog for exec-ing into a pod.
    ///
    /// Uses [`DialogSeverity::Warning`] with an "Open Terminal" confirm label.
    /// Includes the container name in the message when specified.
    pub fn exec_into_pod(pod_name: &str, container_name: Option<&str>) -> Self {
        let message = match container_name {
            Some(c) => format!(
                "You are about to open a terminal session in pod '{}' (container: {})",
                pod_name, c
            ),
            None => format!("You are about to open a terminal session in pod '{}'", pod_name),
        };
        Self::new("Exec into Pod", &message, DialogSeverity::Warning)
            .with_confirm_label("Open Terminal")
            .with_resource_name(pod_name)
    }

    /// Creates a pre-configured dialog for port-forwarding to a resource.
    ///
    /// Uses [`DialogSeverity::Info`] with a "Start Forwarding" confirm label.
    /// Includes port numbers in the message when specified.
    pub fn port_forward(
        resource_name: &str,
        resource_kind: &str,
        local_port: Option<u16>,
        remote_port: Option<u16>,
    ) -> Self {
        let local_str = local_port.map(|p| p.to_string()).unwrap_or_else(|| "<auto>".to_string());
        let remote_str = remote_port.map(|p| p.to_string()).unwrap_or_else(|| "<auto>".to_string());
        let message = format!(
            "Forward traffic from local port {} to {}/{} port {}",
            local_str, resource_kind, resource_name, remote_str
        );
        Self::new("Port Forward", &message, DialogSeverity::Info)
            .with_confirm_label("Start Forwarding")
            .with_resource_name(resource_name)
    }
}

// ---------------------------------------------------------------------------
// GPUI Render
// ---------------------------------------------------------------------------

/// View wrapper for `ConfirmDialogState` that holds a theme for rendering.
pub struct ConfirmDialogView {
    pub state: ConfirmDialogState,
    pub theme: Theme,
}

impl ConfirmDialogView {
    pub fn new(state: ConfirmDialogState, theme: Theme) -> Self {
        Self { state, theme }
    }

    /// Returns the confirm button color based on severity.
    pub fn confirm_button_color(&self) -> crate::theme::Color {
        match self.state.severity {
            DialogSeverity::Destructive => self.theme.colors.error,
            DialogSeverity::Warning => self.theme.colors.warning,
            DialogSeverity::Info => self.theme.colors.accent,
        }
    }

    /// Render the backdrop overlay.
    fn render_backdrop(&self, backdrop_color: Rgba) -> gpui::Div {
        div().absolute().top_0().left_0().w_full().h_full().bg(backdrop_color)
    }

    /// Render the dialog box container.
    fn render_dialog_box(&self, colors: &DialogColors) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .w(px(400.0))
            .bg(colors.surface)
            .rounded(px(8.0))
            .border_1()
            .border_color(colors.border)
            .overflow_hidden()
            .child(self.render_dialog_header(colors))
            .child(self.render_dialog_body(colors))
            .child(self.render_dialog_footer(colors))
    }

    /// Render the dialog header with title.
    fn render_dialog_header(&self, colors: &DialogColors) -> gpui::Div {
        let title = SharedString::from(self.state.title.clone());
        div().px_4().py_3().border_b_1().border_color(colors.border).child(
            div()
                .text_base()
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(colors.text_primary)
                .child(title),
        )
    }

    /// Render the dialog body with message.
    fn render_dialog_body(&self, colors: &DialogColors) -> gpui::Div {
        let message = SharedString::from(self.state.message.clone());
        div().px_4().py_3().child(div().text_sm().text_color(colors.text_secondary).child(message))
    }

    /// Render the dialog footer with buttons.
    fn render_dialog_footer(&self, colors: &DialogColors) -> gpui::Div {
        let cancel_label = SharedString::from(self.state.cancel_label.clone());
        let confirm_label = SharedString::from(self.state.confirm_label.clone());

        let cancel_btn = div()
            .id("dialog-cancel-btn")
            .px_4()
            .py_2()
            .rounded(px(6.0))
            .border_1()
            .border_color(colors.border)
            .cursor_pointer()
            .text_sm()
            .text_color(colors.text_primary)
            .child(cancel_label);

        let confirm_btn = div()
            .id("dialog-confirm-btn")
            .px_4()
            .py_2()
            .rounded(px(6.0))
            .bg(colors.confirm_button)
            .cursor_pointer()
            .text_sm()
            .text_color(colors.confirm_text)
            .child(confirm_label);

        div()
            .flex()
            .flex_row()
            .justify_end()
            .gap(px(8.0))
            .px_4()
            .py_3()
            .border_t_1()
            .border_color(colors.border)
            .child(cancel_btn)
            .child(confirm_btn)
    }
}

/// Precomputed colors for rendering the dialog.
struct DialogColors {
    surface: Rgba,
    border: Rgba,
    text_primary: Rgba,
    text_secondary: Rgba,
    confirm_button: Rgba,
    confirm_text: Rgba,
}

impl Render for ConfirmDialogView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let backdrop_color = crate::theme::Color::rgba(0, 0, 0, 128).to_gpui();

        let colors = DialogColors {
            surface: self.theme.colors.surface.to_gpui(),
            border: self.theme.colors.border.to_gpui(),
            text_primary: self.theme.colors.text_primary.to_gpui(),
            text_secondary: self.theme.colors.text_secondary.to_gpui(),
            confirm_button: self.confirm_button_color().to_gpui(),
            confirm_text: crate::theme::Color::rgb(255, 255, 255).to_gpui(),
        };

        if !self.state.visible {
            return div();
        }

        div()
            .absolute()
            .top_0()
            .left_0()
            .w_full()
            .h_full()
            .flex()
            .justify_center()
            .items_center()
            .child(self.render_backdrop(backdrop_color))
            .child(self.render_dialog_box(&colors))
    }
}
