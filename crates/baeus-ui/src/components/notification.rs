use gpui::{Context, ElementId, Rgba, SharedString, Window, div, prelude::*, px};
use serde::{Deserialize, Serialize};

use crate::theme::Theme;

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

/// Severity level for a notification toast.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NotificationLevel {
    Success,
    Error,
    Warning,
    Info,
}

impl NotificationLevel {
    /// Human-readable label for the notification level.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Success => "Success",
            Self::Error => "Error",
            Self::Warning => "Warning",
            Self::Info => "Info",
        }
    }

    /// Icon name string for the notification level.
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Success => "checkmark",
            Self::Error => "x-circle",
            Self::Warning => "warning-triangle",
            Self::Info => "info-circle",
        }
    }
}

/// A single notification/toast message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,
    pub level: NotificationLevel,
    pub title: String,
    pub message: Option<String>,
    pub auto_dismiss_ms: Option<u64>,
    pub dismissed: bool,
    pub created_at: u64,
}

/// Manages a collection of notifications.
pub struct NotificationState {
    pub notifications: Vec<Notification>,
    pub max_visible: usize,
}

impl Default for NotificationState {
    fn default() -> Self {
        Self::new()
    }
}

impl NotificationState {
    /// Creates a new empty notification state with max_visible = 5.
    pub fn new() -> Self {
        Self { notifications: Vec::new(), max_visible: 5 }
    }

    /// Pushes a new notification onto the stack.
    ///
    /// Generates a UUID id and sets created_at to the current UNIX timestamp
    /// (seconds). Returns the generated id.
    pub fn push(
        &mut self,
        level: NotificationLevel,
        title: impl Into<String>,
        message: Option<String>,
        auto_dismiss_ms: Option<u64>,
    ) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.notifications.push(Notification {
            id: id.clone(),
            level,
            title: title.into(),
            message,
            auto_dismiss_ms,
            dismissed: false,
            created_at,
        });
        id
    }

    /// Marks the notification with the given id as dismissed.
    pub fn dismiss(&mut self, id: &str) {
        if let Some(n) = self.notifications.iter_mut().find(|n| n.id == id) {
            n.dismissed = true;
        }
    }

    /// Marks all notifications as dismissed.
    pub fn dismiss_all(&mut self) {
        for n in &mut self.notifications {
            n.dismissed = true;
        }
    }

    /// Returns non-dismissed notifications, limited to `max_visible`.
    /// Most recent notifications are returned first.
    pub fn visible(&self) -> Vec<&Notification> {
        self.notifications.iter().filter(|n| !n.dismissed).rev().take(self.max_visible).collect()
    }

    /// Removes all dismissed notifications from the internal list (garbage collection).
    pub fn remove_dismissed(&mut self) {
        self.notifications.retain(|n| !n.dismissed);
    }

    /// Returns true if there are any undismissed Error notifications.
    pub fn has_errors(&self) -> bool {
        self.notifications.iter().any(|n| !n.dismissed && n.level == NotificationLevel::Error)
    }

    /// Returns the count of undismissed notifications.
    pub fn count(&self) -> usize {
        self.notifications.iter().filter(|n| !n.dismissed).count()
    }
}

// ---------------------------------------------------------------------------
// GPUI Render
// ---------------------------------------------------------------------------

/// Precomputed colors for rendering notifications.
#[allow(dead_code)]
struct NotificationColors {
    surface: Rgba,
    border: Rgba,
    text_primary: Rgba,
    text_secondary: Rgba,
    success: Rgba,
    error: Rgba,
    warning: Rgba,
    info: Rgba,
    dismiss_text: Rgba,
}

/// View component that renders a notification stack.
pub struct NotificationViewComponent {
    pub state: NotificationState,
    pub theme: Theme,
}

impl NotificationViewComponent {
    pub fn new(state: NotificationState, theme: Theme) -> Self {
        Self { state, theme }
    }

    /// Returns the color for a given notification level.
    pub fn level_color(&self, level: &NotificationLevel) -> crate::theme::Color {
        match level {
            NotificationLevel::Success => self.theme.colors.success,
            NotificationLevel::Error => self.theme.colors.error,
            NotificationLevel::Warning => self.theme.colors.warning,
            NotificationLevel::Info => self.theme.colors.info,
        }
    }

    /// Render a single notification card.
    fn render_notification_card(
        &self,
        notification: &Notification,
        colors: &NotificationColors,
        index: usize,
    ) -> gpui::Div {
        let level_color = match notification.level {
            NotificationLevel::Success => colors.success,
            NotificationLevel::Error => colors.error,
            NotificationLevel::Warning => colors.warning,
            NotificationLevel::Info => colors.info,
        };

        let icon_text = SharedString::from(notification.level.icon().to_string());
        let title_text = SharedString::from(notification.title.clone());
        let dismiss_id = format!("notification-dismiss-{}", index);

        let mut card = div()
            .flex()
            .flex_row()
            .items_start()
            .gap(px(8.0))
            .w(px(320.0))
            .p_3()
            .bg(colors.surface)
            .rounded(px(8.0))
            .border_1()
            .border_color(colors.border);

        // Level icon indicator
        card = card.child(self.render_icon_indicator(level_color, icon_text));

        // Content column (title + optional message)
        let mut content = div().flex().flex_col().flex_1();
        content = content.child(self.render_title(colors.text_primary, title_text));

        if let Some(msg) = &notification.message {
            let msg_text = SharedString::from(msg.clone());
            content = content.child(self.render_message(colors.text_secondary, msg_text));
        }
        card = card.child(content);

        // Dismiss button
        card = card.child(self.render_dismiss_button(colors.dismiss_text, dismiss_id));

        card
    }

    fn render_icon_indicator(&self, level_color: Rgba, icon_text: SharedString) -> gpui::Div {
        div()
            .w(px(20.0))
            .h(px(20.0))
            .flex()
            .items_center()
            .justify_center()
            .text_sm()
            .text_color(level_color)
            .child(icon_text)
    }

    fn render_title(&self, text_color: Rgba, title: SharedString) -> gpui::Div {
        div().text_sm().font_weight(gpui::FontWeight::SEMIBOLD).text_color(text_color).child(title)
    }

    fn render_message(&self, text_color: Rgba, message: SharedString) -> gpui::Div {
        div().text_xs().text_color(text_color).mt_1().child(message)
    }

    fn render_dismiss_button(&self, text_color: Rgba, id: String) -> gpui::Stateful<gpui::Div> {
        div()
            .id(ElementId::Name(SharedString::from(id)))
            .cursor_pointer()
            .text_xs()
            .text_color(text_color)
            .child(SharedString::from("x"))
    }
}

impl Render for NotificationViewComponent {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let colors = NotificationColors {
            surface: self.theme.colors.surface.to_gpui(),
            border: self.theme.colors.border.to_gpui(),
            text_primary: self.theme.colors.text_primary.to_gpui(),
            text_secondary: self.theme.colors.text_secondary.to_gpui(),
            success: self.theme.colors.success.to_gpui(),
            error: self.theme.colors.error.to_gpui(),
            warning: self.theme.colors.warning.to_gpui(),
            info: self.theme.colors.info.to_gpui(),
            dismiss_text: self.theme.colors.text_muted.to_gpui(),
        };

        let visible = self.state.visible();

        if visible.is_empty() {
            return div();
        }

        // Collect notification data before building the div tree to avoid borrow issues
        let notification_data: Vec<Notification> = visible.into_iter().cloned().collect();

        let mut stack =
            div().absolute().top(px(16.0)).right(px(16.0)).flex().flex_col().gap(px(8.0));

        for (i, notification) in notification_data.iter().enumerate() {
            stack = stack.child(self.render_notification_card(notification, &colors, i));
        }

        stack
    }
}
