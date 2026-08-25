use gpui::{Context, ElementId, Rgba, SharedString, Window, div, prelude::*, px};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::theme::Theme;

// ---------------------------------------------------------------------------
// T360: Port Forward Management View
// ---------------------------------------------------------------------------

/// Status of a port forward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortForwardStatus {
    Active,
    Stopped,
    Error,
}

impl PortForwardStatus {
    /// Returns a human-readable label for this status.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::Stopped => "Stopped",
            Self::Error => "Error",
        }
    }
}

/// A single port forward entry in the management view.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortForwardEntry {
    pub id: Uuid,
    pub name: String,
    pub namespace: String,
    pub kind: String,
    pub pod_port: u16,
    pub local_port: u16,
    pub protocol: String,
    pub status: PortForwardStatus,
}

impl PortForwardEntry {
    /// Returns the endpoint URL for this port forward (e.g. "http://localhost:8080").
    pub fn endpoint_url(&self) -> String {
        format!("http://localhost:{}", self.local_port)
    }

    /// Returns true if this entry is currently active.
    pub fn is_active(&self) -> bool {
        self.status == PortForwardStatus::Active
    }

    /// Returns true if this entry is stopped.
    pub fn is_stopped(&self) -> bool {
        self.status == PortForwardStatus::Stopped
    }

    /// Returns true if this entry is in error state.
    pub fn is_error(&self) -> bool {
        self.status == PortForwardStatus::Error
    }

    /// Returns a display string for the port mapping (e.g. "8080 -> 80").
    pub fn port_display(&self) -> String {
        format!("{} -> {}", self.local_port, self.pod_port)
    }
}

/// State for the port forward management view.
#[derive(Debug, Default)]
pub struct PortForwardState {
    pub forwards: Vec<PortForwardEntry>,
}

impl PortForwardState {
    /// Creates a new empty port forward state.
    pub fn new() -> Self {
        Self { forwards: Vec::new() }
    }

    /// Adds a port forward entry.
    pub fn add_forward(&mut self, entry: PortForwardEntry) {
        self.forwards.push(entry);
    }

    /// Stops a port forward by id (sets status to Stopped).
    /// Returns true if the entry was found and stopped.
    pub fn stop_forward(&mut self, id: Uuid) -> bool {
        if let Some(entry) = self.forwards.iter_mut().find(|e| e.id == id) {
            entry.status = PortForwardStatus::Stopped;
            true
        } else {
            false
        }
    }

    /// Removes a port forward entry by id.
    /// Returns true if the entry was found and removed.
    pub fn remove_forward(&mut self, id: Uuid) -> bool {
        if let Some(idx) = self.forwards.iter().position(|e| e.id == id) {
            self.forwards.remove(idx);
            true
        } else {
            false
        }
    }

    /// Returns all active port forwards.
    pub fn active_forwards(&self) -> Vec<&PortForwardEntry> {
        self.forwards.iter().filter(|e| e.status == PortForwardStatus::Active).collect()
    }

    /// Returns a reference to a port forward entry by id.
    pub fn get_forward(&self, id: Uuid) -> Option<&PortForwardEntry> {
        self.forwards.iter().find(|e| e.id == id)
    }

    /// Returns the total number of forwards.
    pub fn forward_count(&self) -> usize {
        self.forwards.len()
    }

    /// Returns the number of active forwards.
    pub fn active_count(&self) -> usize {
        self.active_forwards().len()
    }

    /// Returns true if the given local port is already in use by an active forward.
    pub fn is_local_port_in_use(&self, port: u16) -> bool {
        self.forwards.iter().any(|e| e.local_port == port && e.status == PortForwardStatus::Active)
    }
}

// ---------------------------------------------------------------------------
// Table column definitions
// ---------------------------------------------------------------------------

/// The columns displayed in the port forward table.
pub const PORT_FORWARD_COLUMNS: &[&str] =
    &["Name", "Namespace", "Kind", "Pod Port", "Local Port", "Protocol", "Status"];

// ---------------------------------------------------------------------------
// GPUI Render
// ---------------------------------------------------------------------------

/// Precomputed colors for rendering the port forward view.
#[allow(dead_code)]
struct PortForwardViewColors {
    background: Rgba,
    surface: Rgba,
    border: Rgba,
    accent: Rgba,
    success: Rgba,
    error: Rgba,
    text_primary: Rgba,
    text_secondary: Rgba,
    text_muted: Rgba,
}

/// GPUI-renderable port forward management view component.
///
/// Wraps a `PortForwardState` and a `Theme` to provide a table of active
/// port forwards with Open and Stop actions.
pub struct PortForwardView {
    pub state: PortForwardState,
    pub theme: Theme,
    /// Tracks the last URL that was "opened" (for testability).
    pub last_opened_url: Option<String>,
    /// Tracks the last forward id that was stopped (for testability).
    pub last_stopped_id: Option<Uuid>,
}

impl PortForwardView {
    pub fn new(state: PortForwardState, theme: Theme) -> Self {
        Self { state, theme, last_opened_url: None, last_stopped_id: None }
    }

    /// Simulates opening the endpoint URL for a port forward.
    pub fn open_forward(&mut self, id: Uuid) {
        if let Some(entry) = self.state.get_forward(id) {
            self.last_opened_url = Some(entry.endpoint_url());
        }
    }

    /// Stops a port forward and records the action.
    pub fn stop_forward(&mut self, id: Uuid) -> bool {
        let result = self.state.stop_forward(id);
        if result {
            self.last_stopped_id = Some(id);
        }
        result
    }

    fn colors(&self) -> PortForwardViewColors {
        PortForwardViewColors {
            background: self.theme.colors.background.to_gpui(),
            surface: self.theme.colors.surface.to_gpui(),
            border: self.theme.colors.border.to_gpui(),
            accent: self.theme.colors.accent.to_gpui(),
            success: self.theme.colors.success.to_gpui(),
            error: self.theme.colors.error.to_gpui(),
            text_primary: self.theme.colors.text_primary.to_gpui(),
            text_secondary: self.theme.colors.text_secondary.to_gpui(),
            text_muted: self.theme.colors.text_muted.to_gpui(),
        }
    }

    /// Render the view header with title and active count.
    fn render_header(&self, colors: &PortForwardViewColors) -> gpui::Div {
        let active = self.state.active_count();
        let title = SharedString::from("Port Forwards");
        let count_text = SharedString::from(format!("{active} active"));

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
                            .child(title),
                    )
                    .child(div().text_xs().text_color(colors.text_muted).child(count_text)),
            )
    }

    /// Render the table header row with column names.
    fn render_table_header(&self, colors: &PortForwardViewColors) -> gpui::Div {
        let mut row = div()
            .flex()
            .flex_row()
            .w_full()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(colors.border)
            .bg(colors.surface);

        for col in PORT_FORWARD_COLUMNS {
            row = row.child(
                div()
                    .flex_1()
                    .text_xs()
                    .text_color(colors.text_secondary)
                    .child(SharedString::from(*col)),
            );
        }

        // Actions column header
        row = row.child(
            div()
                .flex_1()
                .text_xs()
                .text_color(colors.text_secondary)
                .child(SharedString::from("Actions")),
        );

        row
    }

    /// Render a single port forward row.
    fn render_forward_row(
        &self,
        entry: &PortForwardEntry,
        idx: usize,
        colors: &PortForwardViewColors,
    ) -> gpui::Stateful<gpui::Div> {
        let row_id = ElementId::Name(SharedString::from(format!("pf-row-{idx}")));

        let status_color = match entry.status {
            PortForwardStatus::Active => colors.success,
            PortForwardStatus::Stopped => colors.text_muted,
            PortForwardStatus::Error => colors.error,
        };

        let mut row = div()
            .id(row_id)
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(colors.border)
            .child(self.render_cell(&entry.name, colors.text_primary))
            .child(self.render_cell(&entry.namespace, colors.text_secondary))
            .child(self.render_cell(&entry.kind, colors.text_secondary))
            .child(self.render_cell(&entry.pod_port.to_string(), colors.text_secondary))
            .child(self.render_cell(&entry.local_port.to_string(), colors.text_secondary))
            .child(self.render_cell(&entry.protocol, colors.text_secondary))
            .child(
                div()
                    .flex_1()
                    .text_xs()
                    .text_color(status_color)
                    .child(SharedString::from(entry.status.label())),
            );

        // Actions: Open + Stop buttons for active entries
        let actions = self.render_actions(entry, idx, colors);
        row = row.child(actions);

        row
    }

    /// Render a single text cell in a row.
    fn render_cell(&self, text: &str, color: Rgba) -> gpui::Div {
        div().flex_1().text_xs().text_color(color).child(SharedString::from(text.to_string()))
    }

    /// Render action buttons for a port forward entry.
    fn render_actions(
        &self,
        entry: &PortForwardEntry,
        idx: usize,
        colors: &PortForwardViewColors,
    ) -> gpui::Div {
        let mut actions = div().flex_1().flex().flex_row().gap(px(4.0));

        if entry.is_active() {
            let open_id = ElementId::Name(SharedString::from(format!("pf-open-{idx}")));
            let stop_id = ElementId::Name(SharedString::from(format!("pf-stop-{idx}")));

            actions = actions
                .child(
                    div()
                        .id(open_id)
                        .px_2()
                        .py_1()
                        .rounded(px(3.0))
                        .bg(colors.accent)
                        .cursor_pointer()
                        .text_xs()
                        .text_color(colors.text_primary)
                        .child("Open"),
                )
                .child(
                    div()
                        .id(stop_id)
                        .px_2()
                        .py_1()
                        .rounded(px(3.0))
                        .bg(colors.error)
                        .cursor_pointer()
                        .text_xs()
                        .text_color(colors.text_primary)
                        .child("Stop"),
                );
        }

        actions
    }

    /// Render the table body with all forward entries.
    fn render_table_body(&self, colors: &PortForwardViewColors) -> gpui::Div {
        let mut body = div().flex().flex_col().flex_1().w_full().overflow_hidden();

        if self.state.forwards.is_empty() {
            body =
                body.child(div().flex().items_center().justify_center().py(px(32.0)).child(
                    div().text_sm().text_color(colors.text_muted).child("No port forwards"),
                ));
        } else {
            for (idx, entry) in self.state.forwards.iter().enumerate() {
                body = body.child(self.render_forward_row(entry, idx, colors));
            }
        }

        body
    }
}

impl Render for PortForwardView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let colors = self.colors();

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(colors.background)
            .child(self.render_header(&colors))
            .child(self.render_table_header(&colors))
            .child(self.render_table_body(&colors))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(
        name: &str,
        ns: &str,
        kind: &str,
        pod_port: u16,
        local_port: u16,
    ) -> PortForwardEntry {
        PortForwardEntry {
            id: Uuid::new_v4(),
            name: name.to_string(),
            namespace: ns.to_string(),
            kind: kind.to_string(),
            pod_port,
            local_port,
            protocol: "TCP".to_string(),
            status: PortForwardStatus::Active,
        }
    }

    #[test]
    fn test_port_forward_status_labels() {
        assert_eq!(PortForwardStatus::Active.label(), "Active");
        assert_eq!(PortForwardStatus::Stopped.label(), "Stopped");
        assert_eq!(PortForwardStatus::Error.label(), "Error");
    }

    #[test]
    fn test_entry_endpoint_url() {
        let entry = make_entry("nginx", "default", "Pod", 80, 8080);
        assert_eq!(entry.endpoint_url(), "http://localhost:8080");
    }

    #[test]
    fn test_entry_status_checks() {
        let mut entry = make_entry("nginx", "default", "Pod", 80, 8080);
        assert!(entry.is_active());
        assert!(!entry.is_stopped());
        assert!(!entry.is_error());

        entry.status = PortForwardStatus::Stopped;
        assert!(!entry.is_active());
        assert!(entry.is_stopped());

        entry.status = PortForwardStatus::Error;
        assert!(entry.is_error());
    }

    #[test]
    fn test_state_add_forward() {
        let mut state = PortForwardState::new();
        state.add_forward(make_entry("nginx", "default", "Pod", 80, 8080));
        assert_eq!(state.forward_count(), 1);
    }

    #[test]
    fn test_state_stop_forward() {
        let mut state = PortForwardState::new();
        let entry = make_entry("nginx", "default", "Pod", 80, 8080);
        let id = entry.id;
        state.add_forward(entry);
        assert!(state.stop_forward(id));
        assert_eq!(state.get_forward(id).unwrap().status, PortForwardStatus::Stopped);
    }

    #[test]
    fn test_state_remove_forward() {
        let mut state = PortForwardState::new();
        let entry = make_entry("nginx", "default", "Pod", 80, 8080);
        let id = entry.id;
        state.add_forward(entry);
        assert!(state.remove_forward(id));
        assert_eq!(state.forward_count(), 0);
    }

    #[test]
    fn test_state_active_forwards() {
        let mut state = PortForwardState::new();
        let e1 = make_entry("nginx", "default", "Pod", 80, 8080);
        let e2 = make_entry("redis", "default", "Pod", 6379, 6379);
        let id1 = e1.id;
        state.add_forward(e1);
        state.add_forward(e2);
        assert_eq!(state.active_forwards().len(), 2);
        state.stop_forward(id1);
        assert_eq!(state.active_forwards().len(), 1);
    }

    #[test]
    fn test_columns_defined() {
        assert_eq!(PORT_FORWARD_COLUMNS.len(), 7);
        assert_eq!(PORT_FORWARD_COLUMNS[0], "Name");
        assert_eq!(PORT_FORWARD_COLUMNS[6], "Status");
    }
}
