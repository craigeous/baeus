use gpui::{Context, ElementId, Rgba, SharedString, Window, div, prelude::*, px};
use serde::{Deserialize, Serialize};

use crate::theme::Theme;

// ---------------------------------------------------------------------------
// T091: Port-Forward Management UI
// ---------------------------------------------------------------------------

/// Display state for a port-forward entry in the UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PortForwardDisplayState {
    Active,
    Stopped,
    Error,
    Starting,
}

impl PortForwardDisplayState {
    /// Returns a human-readable label for this state.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::Stopped => "Stopped",
            Self::Error => "Error",
            Self::Starting => "Starting",
        }
    }
}

/// A single port-forward entry displayed in the panel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortForwardEntry {
    pub id: String,
    pub pod_name: String,
    pub namespace: String,
    pub local_port: u16,
    pub remote_port: u16,
    pub state: PortForwardDisplayState,
    pub error_message: Option<String>,
}

impl PortForwardEntry {
    /// Returns the port mapping display string (e.g., "8080:80").
    pub fn port_display(&self) -> String {
        format!("{}:{}", self.local_port, self.remote_port)
    }

    /// Returns true if this entry is currently active.
    pub fn is_active(&self) -> bool {
        self.state == PortForwardDisplayState::Active
    }

    /// Returns true if this entry is in an error state.
    pub fn is_error(&self) -> bool {
        self.state == PortForwardDisplayState::Error
    }
}

/// State for the port-forward management panel.
pub struct PortForwardPanelState {
    pub entries: Vec<PortForwardEntry>,
    pub show_create_dialog: bool,
    pub new_local_port: String,
    pub new_remote_port: String,
}

impl Default for PortForwardPanelState {
    fn default() -> Self {
        Self::new()
    }
}

impl PortForwardPanelState {
    /// Creates a new empty port-forward panel state.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            show_create_dialog: false,
            new_local_port: String::new(),
            new_remote_port: String::new(),
        }
    }

    /// Adds a port-forward entry to the panel.
    pub fn add_entry(&mut self, entry: PortForwardEntry) {
        self.entries.push(entry);
    }

    /// Removes a port-forward entry by id.
    pub fn remove_entry(&mut self, id: &str) -> bool {
        if let Some(idx) = self.entries.iter().position(|e| e.id == id) {
            self.entries.remove(idx);
            true
        } else {
            false
        }
    }

    /// Stops a port-forward entry by setting its state to Stopped.
    pub fn stop_entry(&mut self, id: &str) -> bool {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.state = PortForwardDisplayState::Stopped;
            true
        } else {
            false
        }
    }

    /// Sets an error state on a port-forward entry.
    pub fn set_error(&mut self, id: &str, message: String) -> bool {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.state = PortForwardDisplayState::Error;
            entry.error_message = Some(message);
            true
        } else {
            false
        }
    }

    /// Returns the number of active port-forward entries.
    pub fn active_count(&self) -> usize {
        self.entries.iter().filter(|e| e.state == PortForwardDisplayState::Active).count()
    }

    /// Returns entries matching a specific pod name.
    pub fn entries_for_pod(&self, pod_name: &str) -> Vec<&PortForwardEntry> {
        self.entries.iter().filter(|e| e.pod_name == pod_name).collect()
    }

    /// Returns true if the given local port is already in use by an active entry.
    pub fn is_port_in_use(&self, local_port: u16) -> bool {
        self.entries
            .iter()
            .any(|e| e.local_port == local_port && e.state == PortForwardDisplayState::Active)
    }

    /// Opens the create port-forward dialog.
    pub fn open_create_dialog(&mut self) {
        self.show_create_dialog = true;
        self.new_local_port.clear();
        self.new_remote_port.clear();
    }

    /// Closes the create port-forward dialog.
    pub fn close_create_dialog(&mut self) {
        self.show_create_dialog = false;
        self.new_local_port.clear();
        self.new_remote_port.clear();
    }

    /// Validates the new port-forward inputs and returns parsed ports.
    ///
    /// Returns `Ok((local_port, remote_port))` if both ports are valid u16
    /// values greater than zero and the local port is not already in use.
    /// Returns `Err(message)` describing the validation failure.
    pub fn validate_new_forward(&self) -> Result<(u16, u16), String> {
        let local: u16 = self
            .new_local_port
            .parse()
            .map_err(|_| "Invalid local port: must be a number 1-65535".to_string())?;
        let remote: u16 = self
            .new_remote_port
            .parse()
            .map_err(|_| "Invalid remote port: must be a number 1-65535".to_string())?;

        if local == 0 {
            return Err("Local port must be greater than 0".to_string());
        }
        if remote == 0 {
            return Err("Remote port must be greater than 0".to_string());
        }
        if self.is_port_in_use(local) {
            return Err(format!("Local port {local} is already in use"));
        }

        Ok((local, remote))
    }

    /// Returns the total number of entries.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Returns an entry by id.
    pub fn get_entry(&self, id: &str) -> Option<&PortForwardEntry> {
        self.entries.iter().find(|e| e.id == id)
    }
}

// ---------------------------------------------------------------------------
// GPUI Render
// ---------------------------------------------------------------------------

/// View component for the port-forward management panel.
pub struct PortForwardPanelComponent {
    pub state: PortForwardPanelState,
    pub theme: Theme,
}

impl PortForwardPanelComponent {
    pub fn new(state: PortForwardPanelState, theme: Theme) -> Self {
        Self { state, theme }
    }

    /// Render the panel header with title and "New Forward" button.
    fn render_header(&self, colors: &PortForwardColors) -> gpui::Div {
        let active_count = self.state.active_count();
        let count_text = SharedString::from(format!("{active_count} active"));
        let new_btn_id = ElementId::Name(SharedString::from("pf-new-btn"));

        div()
            .flex()
            .items_center()
            .w_full()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(colors.border)
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(colors.text_primary)
                            .child("Port Forwards"),
                    )
                    .child(div().text_xs().text_color(colors.text_muted).child(count_text)),
            )
            .child(
                div()
                    .id(new_btn_id)
                    .px_2()
                    .py_1()
                    .rounded(px(4.0))
                    .bg(colors.accent)
                    .text_xs()
                    .text_color(colors.text_primary)
                    .cursor_pointer()
                    .child("+ New Forward"),
            )
    }

    /// Render a single port-forward entry row.
    fn render_entry_row(
        &self,
        entry: &PortForwardEntry,
        idx: usize,
        colors: &PortForwardColors,
    ) -> gpui::Stateful<gpui::Div> {
        let row_id = ElementId::Name(SharedString::from(format!("pf-entry-{idx}")));
        let stop_id = ElementId::Name(SharedString::from(format!("pf-stop-{idx}")));

        let port_text = SharedString::from(entry.port_display());
        let pod_text = SharedString::from(format!("{}/{}", entry.namespace, entry.pod_name));
        let state_label = SharedString::from(entry.state.label().to_string());

        let state_color = match entry.state {
            PortForwardDisplayState::Active => colors.success,
            PortForwardDisplayState::Error => colors.error,
            PortForwardDisplayState::Stopped => colors.text_muted,
            PortForwardDisplayState::Starting => colors.accent,
        };

        let info_div = self.render_entry_info(&port_text, &pod_text, colors);
        let status_div = self.render_entry_status(&state_label, state_color);
        let error_div = self.render_entry_error(entry, colors);

        let mut row = div()
            .id(row_id)
            .flex()
            .items_center()
            .w_full()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(colors.border)
            .child(info_div)
            .child(status_div);

        // Show stop button for active/starting entries
        if entry.state == PortForwardDisplayState::Active
            || entry.state == PortForwardDisplayState::Starting
        {
            row = row.child(
                div()
                    .id(stop_id)
                    .px_2()
                    .py_1()
                    .rounded(px(4.0))
                    .bg(colors.error)
                    .text_xs()
                    .text_color(colors.text_primary)
                    .cursor_pointer()
                    .child("Stop"),
            );
        }

        // Show error message if present
        if entry.is_error() {
            row = row.child(error_div);
        }

        row
    }

    /// Render the info portion of an entry (port + pod name).
    fn render_entry_info(
        &self,
        port_text: &SharedString,
        pod_text: &SharedString,
        colors: &PortForwardColors,
    ) -> gpui::Div {
        div()
            .flex_1()
            .flex()
            .flex_col()
            .gap(px(2.0))
            .child(div().text_sm().text_color(colors.text_primary).child(port_text.clone()))
            .child(div().text_xs().text_color(colors.text_muted).child(pod_text.clone()))
    }

    /// Render the status indicator for an entry.
    fn render_entry_status(&self, state_label: &SharedString, state_color: Rgba) -> gpui::Div {
        div().px_2().text_xs().text_color(state_color).child(state_label.clone())
    }

    /// Render the error message for an entry.
    fn render_entry_error(
        &self,
        entry: &PortForwardEntry,
        colors: &PortForwardColors,
    ) -> gpui::Div {
        let msg = entry.error_message.as_deref().unwrap_or("Unknown error");
        let error_text = SharedString::from(msg.to_string());

        div().w_full().px_3().py_1().text_xs().text_color(colors.error).child(error_text)
    }

    /// Render the entries list.
    fn render_entries_list(&self, colors: &PortForwardColors) -> gpui::Div {
        let mut list = div().flex().flex_col().w_full();

        if self.state.entries.is_empty() {
            list = list.child(
                div()
                    .flex()
                    .justify_center()
                    .py_4()
                    .text_sm()
                    .text_color(colors.text_muted)
                    .child("No active port forwards"),
            );
        } else {
            for (idx, entry) in self.state.entries.iter().enumerate() {
                list = list.child(self.render_entry_row(entry, idx, colors));
            }
        }

        list
    }

    /// Render the create dialog.
    fn render_create_dialog(&self, colors: &PortForwardColors) -> gpui::Div {
        let local_text = if self.state.new_local_port.is_empty() {
            SharedString::from("Local port")
        } else {
            SharedString::from(self.state.new_local_port.clone())
        };
        let remote_text = if self.state.new_remote_port.is_empty() {
            SharedString::from("Remote port")
        } else {
            SharedString::from(self.state.new_remote_port.clone())
        };

        let start_id = ElementId::Name(SharedString::from("pf-start-btn"));
        let cancel_id = ElementId::Name(SharedString::from("pf-cancel-btn"));

        let local_input = self.render_dialog_input(&local_text, colors);
        let remote_input = self.render_dialog_input(&remote_text, colors);

        div()
            .flex()
            .flex_col()
            .w_full()
            .p_3()
            .gap(px(8.0))
            .bg(colors.surface)
            .border_1()
            .border_color(colors.border)
            .rounded(px(6.0))
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(colors.text_primary)
                    .child("New Port Forward"),
            )
            .child(
                div()
                    .flex()
                    .gap(px(8.0))
                    .child(local_input)
                    .child(div().text_sm().text_color(colors.text_muted).child(":"))
                    .child(remote_input),
            )
            .child(
                div()
                    .flex()
                    .gap(px(8.0))
                    .justify_end()
                    .child(
                        div()
                            .id(cancel_id)
                            .px_3()
                            .py_1()
                            .rounded(px(4.0))
                            .bg(colors.border)
                            .text_xs()
                            .text_color(colors.text_primary)
                            .cursor_pointer()
                            .child("Cancel"),
                    )
                    .child(
                        div()
                            .id(start_id)
                            .px_3()
                            .py_1()
                            .rounded(px(4.0))
                            .bg(colors.accent)
                            .text_xs()
                            .text_color(colors.text_primary)
                            .cursor_pointer()
                            .child("Start"),
                    ),
            )
    }

    /// Render a single input field in the create dialog.
    fn render_dialog_input(&self, text: &SharedString, colors: &PortForwardColors) -> gpui::Div {
        div()
            .flex_1()
            .px_2()
            .py_1()
            .rounded(px(4.0))
            .bg(colors.background)
            .border_1()
            .border_color(colors.border)
            .text_sm()
            .text_color(colors.text_muted)
            .child(text.clone())
    }
}

/// Precomputed colors for rendering the port-forward panel.
#[allow(dead_code)]
struct PortForwardColors {
    background: Rgba,
    surface: Rgba,
    border: Rgba,
    accent: Rgba,
    success: Rgba,
    error: Rgba,
    text_primary: Rgba,
    text_muted: Rgba,
}

impl Render for PortForwardPanelComponent {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let colors = PortForwardColors {
            background: self.theme.colors.background.to_gpui(),
            surface: self.theme.colors.surface.to_gpui(),
            border: self.theme.colors.border.to_gpui(),
            accent: self.theme.colors.accent.to_gpui(),
            success: self.theme.colors.success.to_gpui(),
            error: self.theme.colors.error.to_gpui(),
            text_primary: self.theme.colors.text_primary.to_gpui(),
            text_muted: self.theme.colors.text_muted.to_gpui(),
        };

        let mut panel = div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .bg(colors.background)
            .child(self.render_header(&colors))
            .child(self.render_entries_list(&colors));

        if self.state.show_create_dialog {
            panel = panel.child(self.render_create_dialog(&colors));
        }

        panel
    }
}
