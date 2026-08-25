use gpui::*;
use serde::{Deserialize, Serialize};

use crate::components::status_badge::{BadgeVariant, StatusBadge};
use crate::theme::{Theme, ThemeMode};

/// The current connection state shown in the cluster list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClusterConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Error,
}

impl ClusterConnectionState {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Disconnected => "Disconnected",
            Self::Connecting => "Connecting...",
            Self::Connected => "Connected",
            Self::Reconnecting => "Reconnecting...",
            Self::Error => "Error",
        }
    }

    pub fn is_actionable(&self) -> bool {
        // Can click "Connect" only when disconnected or in error state
        matches!(self, Self::Disconnected | Self::Error)
    }
}

/// Represents a single Kubernetes cluster entry in the cluster list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClusterListItem {
    pub context_name: String,
    pub display_name: String,
    pub api_server_url: String,
    pub connected: bool,
    pub favorite: bool,
    pub auth_method: String,
    pub connection_state: ClusterConnectionState,
    pub error_message: Option<String>,
}

impl ClusterListItem {
    pub fn new(
        context_name: impl Into<String>,
        display_name: impl Into<String>,
        api_server_url: impl Into<String>,
        auth_method: impl Into<String>,
    ) -> Self {
        Self {
            context_name: context_name.into(),
            display_name: display_name.into(),
            api_server_url: api_server_url.into(),
            connected: false,
            favorite: false,
            auth_method: auth_method.into(),
            connection_state: ClusterConnectionState::Disconnected,
            error_message: None,
        }
    }

    /// Returns the button label for this cluster's primary action.
    pub fn action_label(&self) -> &'static str {
        match self.connection_state {
            ClusterConnectionState::Disconnected => "Connect",
            ClusterConnectionState::Connecting => "Connecting...",
            ClusterConnectionState::Connected => "Disconnect",
            ClusterConnectionState::Reconnecting => "Reconnecting...",
            ClusterConnectionState::Error => "Retry",
        }
    }

    /// Returns true if the connect/disconnect button should be enabled.
    pub fn is_action_enabled(&self) -> bool {
        matches!(
            self.connection_state,
            ClusterConnectionState::Disconnected
                | ClusterConnectionState::Connected
                | ClusterConnectionState::Error
        )
    }
}

/// State for the cluster list view, managing the list of clusters,
/// selection state, and text-based filtering.
#[derive(Debug, Clone, Default)]
pub struct ClusterListState {
    pub items: Vec<ClusterListItem>,
    pub selected_index: Option<usize>,
    pub filter_text: String,
}

impl ClusterListState {
    pub fn new(items: Vec<ClusterListItem>) -> Self {
        Self { items, selected_index: None, filter_text: String::new() }
    }

    /// Returns items filtered by the current `filter_text`.
    ///
    /// Matching is case-insensitive and checks both `display_name` and `context_name`.
    /// When `filter_text` is empty, all items are returned.
    pub fn filtered_items(&self) -> Vec<&ClusterListItem> {
        if self.filter_text.is_empty() {
            return self.items.iter().collect();
        }
        let query = self.filter_text.to_lowercase();
        self.items
            .iter()
            .filter(|item| {
                item.display_name.to_lowercase().contains(&query)
                    || item.context_name.to_lowercase().contains(&query)
            })
            .collect()
    }

    /// Toggles the favorite status of the item at the given index.
    ///
    /// Does nothing if the index is out of bounds.
    pub fn toggle_favorite(&mut self, index: usize) {
        if let Some(item) = self.items.get_mut(index) {
            item.favorite = !item.favorite;
        }
    }

    /// Selects the item at the given index.
    ///
    /// Sets `selected_index` to `None` if the index is out of bounds.
    pub fn select(&mut self, index: usize) {
        if index < self.items.len() {
            self.selected_index = Some(index);
        } else {
            self.selected_index = None;
        }
    }

    /// Returns a reference to the currently selected item, if any.
    pub fn selected_item(&self) -> Option<&ClusterListItem> {
        self.selected_index.and_then(|i| self.items.get(i))
    }

    /// Update the connection state of a cluster by context name.
    pub fn set_connection_state(&mut self, context_name: &str, state: ClusterConnectionState) {
        if let Some(item) = self.items.iter_mut().find(|i| i.context_name == context_name) {
            item.connection_state = state;
            item.connected = state == ClusterConnectionState::Connected;
            if state != ClusterConnectionState::Error {
                item.error_message = None;
            }
        }
    }

    /// Set an error message for a cluster by context name.
    pub fn set_error(&mut self, context_name: &str, message: String) {
        if let Some(item) = self.items.iter_mut().find(|i| i.context_name == context_name) {
            item.connection_state = ClusterConnectionState::Error;
            item.connected = false;
            item.error_message = Some(message);
        }
    }

    /// Sort items: favorites first, then alphabetically by display_name.
    pub fn sort_favorites_first(&mut self) {
        self.items.sort_by(|a, b| {
            b.favorite.cmp(&a.favorite).then_with(|| a.display_name.cmp(&b.display_name))
        });
    }

    /// Count of connected clusters.
    pub fn connected_count(&self) -> usize {
        self.items.iter().filter(|i| i.connected).count()
    }
}

// ---------------------------------------------------------------------------
// ClusterListView — GPUI Render implementation
// ---------------------------------------------------------------------------

/// Maps a `ClusterConnectionState` to the corresponding `BadgeVariant`.
fn badge_variant_for(state: ClusterConnectionState) -> BadgeVariant {
    match state {
        ClusterConnectionState::Connected => BadgeVariant::Connected,
        ClusterConnectionState::Disconnected => BadgeVariant::Disconnected,
        ClusterConnectionState::Connecting => BadgeVariant::Pending,
        ClusterConnectionState::Reconnecting => BadgeVariant::Warning,
        ClusterConnectionState::Error => BadgeVariant::Error,
    }
}

/// A renderable cluster list view.
///
/// Wraps `ClusterListState` and `Theme` and renders a scrollable list of
/// cluster cards. Each card shows the cluster name, API server URL, a
/// status badge, a connect/disconnect button, and a favorite toggle star.
pub struct ClusterListView {
    pub state: ClusterListState,
    pub theme: Theme,
}

impl ClusterListView {
    pub fn new(
        state: ClusterListState,
        theme: Theme,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Self {
        Self { state, theme }
    }
}

impl Render for ClusterListView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bg = self.theme.colors.background.to_gpui();
        let border = self.theme.colors.border.to_gpui();

        let header = self.render_header(cx);
        let card_list = self.render_card_list(cx);

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(bg)
            .overflow_hidden()
            .child(div().border_b_1().border_color(border).flex_shrink_0().child(header))
            .child(card_list)
    }
}

// ---------------------------------------------------------------------------
// Rendering helpers (extracted so each method keeps a shallow type tree)
// ---------------------------------------------------------------------------

impl ClusterListView {
    /// Render the top header bar showing "Clusters" and the connected count.
    fn render_header(&self, _cx: &mut Context<Self>) -> Div {
        let text = self.theme.colors.text_primary.to_gpui();
        let text_secondary = self.theme.colors.text_secondary.to_gpui();
        let connected_count = self.state.connected_count();
        let total_count = self.state.items.len();

        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .px_4()
            .py_3()
            .child(div().font_weight(FontWeight::BOLD).text_color(text).child("Clusters"))
            .child(
                div()
                    .text_sm()
                    .text_color(text_secondary)
                    .child(format!("{connected_count} / {total_count} connected")),
            )
    }

    /// Render the scrollable card list area, or an empty-state message.
    fn render_card_list(&mut self, cx: &mut Context<Self>) -> Div {
        let text_muted = self.theme.colors.text_muted.to_gpui();

        let filtered: Vec<(usize, ClusterListItem)> = self
            .state
            .filtered_items()
            .iter()
            .filter_map(|item| {
                let idx = self.state.items.iter().position(|i| i.context_name == item.context_name);
                idx.map(|i| (i, (*item).clone()))
            })
            .collect();

        let mut list = div().flex().flex_col().flex_1().p_4().gap_3().overflow_hidden();

        if filtered.is_empty() {
            list = list.child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .flex_1()
                    .text_color(text_muted)
                    .text_sm()
                    .child("No clusters match your filter."),
            );
        } else {
            for (original_idx, item) in &filtered {
                let card = self.render_card(*original_idx, item, cx);
                list = list.child(card);
            }
        }

        list
    }

    /// Render an individual cluster card.
    fn render_card(
        &self,
        original_idx: usize,
        item: &ClusterListItem,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let is_dark = matches!(self.theme.mode, ThemeMode::Dark | ThemeMode::System);
        let surface = self.theme.colors.surface.to_gpui();
        let surface_hover = self.theme.colors.surface_hover.to_gpui();
        let border = self.theme.colors.border.to_gpui();
        let accent = self.theme.colors.accent.to_gpui();
        let error_color = self.theme.colors.error.to_gpui();
        let selected_index = self.state.selected_index;
        let is_selected = selected_index == Some(original_idx);

        let card_id =
            ElementId::Name(SharedString::from(format!("cluster-card-{}", item.context_name)));

        let mut card = div()
            .id(card_id)
            .flex()
            .flex_col()
            .p_3()
            .rounded(px(8.0))
            .border_1()
            .border_color(border)
            .bg(surface)
            .cursor_pointer()
            .on_click(cx.listener(move |this, _event, _window, _cx| {
                this.state.select(original_idx);
            }));

        if is_selected {
            card = card.border_color(accent).bg(surface_hover);
        }

        // Row 1: Name + Favorite star
        let name_row = self.render_name_row(original_idx, item, cx);
        card = card.child(name_row);

        // Row 2: API server URL
        let url_row = self.render_url_row(item);
        card = card.child(url_row);

        // Row 3: Status badge + Action button
        let status_row = self.render_status_row(original_idx, item, is_dark, cx);
        card = card.child(status_row);

        // Optional error message
        if let Some(err) = &item.error_message {
            card = card.child(div().text_xs().text_color(error_color).mt_1().child(err.clone()));
        }

        card
    }

    /// Render the cluster name (bold) and the favorite toggle star.
    fn render_name_row(
        &self,
        original_idx: usize,
        item: &ClusterListItem,
        cx: &mut Context<Self>,
    ) -> Div {
        let text = self.theme.colors.text_primary.to_gpui();
        let text_muted = self.theme.colors.text_muted.to_gpui();
        let accent = self.theme.colors.accent.to_gpui();
        let favorite = item.favorite;
        let fav_star = if favorite { "★" } else { "☆" };
        let fav_btn_id =
            ElementId::Name(SharedString::from(format!("cluster-fav-{}", item.context_name)));

        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .child(
                div()
                    .font_weight(FontWeight::BOLD)
                    .text_color(text)
                    .text_sm()
                    .child(item.display_name.clone()),
            )
            .child(
                div()
                    .id(fav_btn_id)
                    .cursor_pointer()
                    .text_color(if favorite { accent } else { text_muted })
                    .on_click(cx.listener(move |this, _event, _window, _cx| {
                        this.state.toggle_favorite(original_idx);
                    }))
                    .child(fav_star),
            )
    }

    /// Render the API server URL line.
    fn render_url_row(&self, item: &ClusterListItem) -> Div {
        let text_muted = self.theme.colors.text_muted.to_gpui();

        div().text_xs().text_color(text_muted).mt_1().child(item.api_server_url.clone())
    }

    /// Render the status badge (dot + label) and connect/disconnect button.
    fn render_status_row(
        &self,
        original_idx: usize,
        item: &ClusterListItem,
        is_dark: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let badge = StatusBadge::new(badge_variant_for(item.connection_state));
        let badge_color = badge.color(is_dark).to_gpui();
        let badge_label = badge.label.clone();

        let action_btn = self.render_action_button(original_idx, item, cx);

        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .mt_2()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(div().w(px(8.0)).h(px(8.0)).rounded_full().bg(badge_color))
                    .child(div().text_xs().text_color(badge_color).child(badge_label)),
            )
            .child(action_btn)
    }

    /// Render the connect / disconnect / retry action button.
    fn render_action_button(
        &self,
        original_idx: usize,
        item: &ClusterListItem,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let surface_hover = self.theme.colors.surface_hover.to_gpui();
        let text_muted = self.theme.colors.text_muted.to_gpui();
        let accent = self.theme.colors.accent.to_gpui();
        let action_label = item.action_label();
        let action_enabled = item.is_action_enabled();

        let action_btn_id =
            ElementId::Name(SharedString::from(format!("cluster-action-{}", item.context_name)));

        let mut btn = div()
            .id(action_btn_id)
            .px_2()
            .py(px(2.0))
            .rounded(px(4.0))
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .cursor_pointer();

        if action_enabled {
            btn = btn.bg(accent).text_color(gpui::rgb(0xFFFFFF)).on_click(cx.listener(
                move |this, _event, _window, _cx| {
                    let current = this.state.items[original_idx].connection_state;
                    match current {
                        ClusterConnectionState::Disconnected | ClusterConnectionState::Error => {
                            this.state.set_connection_state(
                                &this.state.items[original_idx].context_name.clone(),
                                ClusterConnectionState::Connecting,
                            );
                        }
                        ClusterConnectionState::Connected => {
                            this.state.set_connection_state(
                                &this.state.items[original_idx].context_name.clone(),
                                ClusterConnectionState::Disconnected,
                            );
                        }
                        _ => {}
                    }
                },
            ));
        } else {
            btn = btn.bg(surface_hover).text_color(text_muted);
        }

        btn.child(action_label)
    }
}
