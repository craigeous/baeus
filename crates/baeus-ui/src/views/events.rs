use baeus_core::EventType;
use gpui::{Context, ElementId, Rgba, SharedString, Window, div, prelude::*, px};
use serde::{Deserialize, Serialize};

use crate::theme::Theme;

/// Severity filter for the events view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventSeverityFilter {
    All,
    Normal,
    Warning,
}

impl EventSeverityFilter {
    pub fn label(&self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Normal => "Normal",
            Self::Warning => "Warning",
        }
    }

    pub fn matches(&self, event_type: &EventType) -> bool {
        match self {
            Self::All => true,
            Self::Normal => *event_type == EventType::Normal,
            Self::Warning => *event_type == EventType::Warning,
        }
    }
}

/// A single event displayed in the events view.
#[derive(Debug, Clone)]
pub struct EventRow {
    pub uid: String,
    pub event_type: EventType,
    pub reason: String,
    pub message: String,
    pub resource_kind: Option<String>,
    pub resource_name: Option<String>,
    pub namespace: Option<String>,
    pub age: String,
    pub count: u32,
}

impl EventRow {
    pub fn severity_label(&self) -> &'static str {
        match self.event_type {
            EventType::Normal => "Normal",
            EventType::Warning => "Warning",
        }
    }

    pub fn is_warning(&self) -> bool {
        self.event_type == EventType::Warning
    }

    pub fn resource_display(&self) -> String {
        match (&self.resource_kind, &self.resource_name) {
            (Some(kind), Some(name)) => format!("{kind}/{name}"),
            (Some(kind), None) => kind.clone(),
            _ => String::new(),
        }
    }
}

/// State for the events view with live event feed.
#[derive(Debug)]
pub struct EventsViewState {
    pub events: Vec<EventRow>,
    pub severity_filter: EventSeverityFilter,
    pub namespace_filter: Option<String>,
    pub resource_kind_filter: Option<String>,
    pub search_query: String,
    pub loading: bool,
    pub error: Option<String>,
    pub auto_scroll: bool,
    pub max_events: usize,
}

impl Default for EventsViewState {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            severity_filter: EventSeverityFilter::All,
            namespace_filter: None,
            resource_kind_filter: None,
            search_query: String::new(),
            loading: false,
            error: None,
            auto_scroll: true,
            max_events: 1000,
        }
    }
}

impl EventsViewState {
    pub fn set_events(&mut self, events: Vec<EventRow>) {
        self.events = events;
    }

    pub fn push_event(&mut self, event: EventRow) {
        self.events.push(event);
        // Trim to max_events
        if self.events.len() > self.max_events {
            let excess = self.events.len() - self.max_events;
            self.events.drain(0..excess);
        }
    }

    pub fn set_severity_filter(&mut self, filter: EventSeverityFilter) {
        self.severity_filter = filter;
    }

    pub fn set_namespace_filter(&mut self, namespace: Option<String>) {
        self.namespace_filter = namespace;
    }

    pub fn set_resource_kind_filter(&mut self, kind: Option<String>) {
        self.resource_kind_filter = kind;
    }

    pub fn set_search_query(&mut self, query: &str) {
        self.search_query = query.to_string();
    }

    pub fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
    }

    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
    }

    pub fn clear_error(&mut self) {
        self.error = None;
    }

    pub fn toggle_auto_scroll(&mut self) {
        self.auto_scroll = !self.auto_scroll;
    }

    /// Returns filtered events based on current filters.
    pub fn filtered_events(&self) -> Vec<&EventRow> {
        self.events
            .iter()
            .filter(|e| self.severity_filter.matches(&e.event_type))
            .filter(|e| match &self.namespace_filter {
                Some(ns) => e.namespace.as_deref() == Some(ns.as_str()),
                None => true,
            })
            .filter(|e| match &self.resource_kind_filter {
                Some(kind) => e.resource_kind.as_deref() == Some(kind.as_str()),
                None => true,
            })
            .filter(|e| {
                if self.search_query.is_empty() {
                    return true;
                }
                let query = self.search_query.to_lowercase();
                e.reason.to_lowercase().contains(&query)
                    || e.message.to_lowercase().contains(&query)
                    || e.resource_display().to_lowercase().contains(&query)
            })
            .collect()
    }

    pub fn warning_count(&self) -> usize {
        self.events.iter().filter(|e| e.is_warning()).count()
    }

    pub fn normal_count(&self) -> usize {
        self.events.iter().filter(|e| !e.is_warning()).count()
    }

    pub fn total_count(&self) -> usize {
        self.events.len()
    }

    pub fn clear_events(&mut self) {
        self.events.clear();
    }
}

// ---------------------------------------------------------------------------
// T068: EventsViewComponent with impl Render
// ---------------------------------------------------------------------------

/// Precomputed colors for the events view.
#[allow(dead_code)]
struct EventsColors {
    background: Rgba,
    surface: Rgba,
    border: Rgba,
    accent: Rgba,
    text_primary: Rgba,
    text_secondary: Rgba,
    text_muted: Rgba,
    info: Rgba,
    warning: Rgba,
    error: Rgba,
}

/// GPUI-renderable events view component.
///
/// Wraps an `EventsViewState` and a `Theme` to provide the full
/// events list UI with severity filter pills, namespace/kind filters,
/// search input, scrollable event rows, and empty/loading/error states.
pub struct EventsViewComponent {
    pub state: EventsViewState,
    pub theme: Theme,
}

impl EventsViewComponent {
    pub fn new(state: EventsViewState, theme: Theme) -> Self {
        Self { state, theme }
    }

    /// Returns the theme color for a given event severity.
    pub fn severity_color(&self, event_type: &EventType) -> crate::theme::Color {
        match event_type {
            EventType::Normal => self.theme.colors.info,
            EventType::Warning => self.theme.colors.warning,
        }
    }

    /// Human-readable severity label with count for filter pill.
    pub fn severity_filter_label(&self, filter: EventSeverityFilter) -> String {
        match filter {
            EventSeverityFilter::All => {
                format!("All ({})", self.state.total_count())
            }
            EventSeverityFilter::Normal => {
                format!("Normal ({})", self.state.normal_count())
            }
            EventSeverityFilter::Warning => {
                format!("Warning ({})", self.state.warning_count())
            }
        }
    }

    // -- Precomputed colors --

    fn colors(&self) -> EventsColors {
        EventsColors {
            background: self.theme.colors.background.to_gpui(),
            surface: self.theme.colors.surface.to_gpui(),
            border: self.theme.colors.border.to_gpui(),
            accent: self.theme.colors.accent.to_gpui(),
            text_primary: self.theme.colors.text_primary.to_gpui(),
            text_secondary: self.theme.colors.text_secondary.to_gpui(),
            text_muted: self.theme.colors.text_muted.to_gpui(),
            info: self.theme.colors.info.to_gpui(),
            warning: self.theme.colors.warning.to_gpui(),
            error: self.theme.colors.error.to_gpui(),
        }
    }

    // -- Render helpers (each returns Div) --

    /// Toolbar with severity filter pills, namespace filter,
    /// resource kind filter, and search input.
    fn render_toolbar(&self, colors: &EventsColors) -> gpui::Div {
        let pills = self.render_severity_pills(colors);
        let ns = self.render_namespace_filter(colors);
        let kind = self.render_kind_filter(colors);
        let search = self.render_search_input(colors);

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
            .child(pills)
            .child(ns)
            .child(kind)
            .child(search)
    }

    /// Severity filter pills (All / Normal / Warning with counts).
    fn render_severity_pills(&self, colors: &EventsColors) -> gpui::Div {
        let filters =
            [EventSeverityFilter::All, EventSeverityFilter::Normal, EventSeverityFilter::Warning];
        let mut row = div().flex().flex_row().gap(px(4.0));

        for filter in &filters {
            let active = *filter == self.state.severity_filter;
            let tc = if active { colors.accent } else { colors.text_muted };
            let bg = if active { colors.surface } else { colors.background };
            let label = self.severity_filter_label(*filter);
            let id = format!("severity-{}", filter.label());

            let pill = div()
                .id(ElementId::Name(SharedString::from(id)))
                .px_2()
                .py_1()
                .rounded(px(4.0))
                .bg(bg)
                .cursor_pointer()
                .text_xs()
                .text_color(tc)
                .child(SharedString::from(label));
            row = row.child(pill);
        }

        row
    }

    /// Namespace filter display.
    fn render_namespace_filter(&self, colors: &EventsColors) -> gpui::Stateful<gpui::Div> {
        let label = match &self.state.namespace_filter {
            Some(ns) => format!("ns: {ns}"),
            None => "All namespaces".to_string(),
        };
        div()
            .id("events-ns-filter")
            .px_2()
            .py_1()
            .rounded(px(4.0))
            .bg(colors.surface)
            .border_1()
            .border_color(colors.border)
            .cursor_pointer()
            .text_xs()
            .text_color(colors.text_secondary)
            .child(SharedString::from(label))
    }

    /// Resource kind filter display.
    fn render_kind_filter(&self, colors: &EventsColors) -> gpui::Stateful<gpui::Div> {
        let label = match &self.state.resource_kind_filter {
            Some(kind) => format!("kind: {kind}"),
            None => "All kinds".to_string(),
        };
        div()
            .id("events-kind-filter")
            .px_2()
            .py_1()
            .rounded(px(4.0))
            .bg(colors.surface)
            .border_1()
            .border_color(colors.border)
            .cursor_pointer()
            .text_xs()
            .text_color(colors.text_secondary)
            .child(SharedString::from(label))
    }

    /// Search input.
    fn render_search_input(&self, colors: &EventsColors) -> gpui::Stateful<gpui::Div> {
        let ph = if self.state.search_query.is_empty() {
            "Search events...".to_string()
        } else {
            self.state.search_query.clone()
        };
        let tc = if self.state.search_query.is_empty() {
            colors.text_muted
        } else {
            colors.text_primary
        };
        div()
            .id("events-search")
            .flex_1()
            .px_2()
            .py_1()
            .rounded(px(4.0))
            .bg(colors.surface)
            .border_1()
            .border_color(colors.border)
            .text_xs()
            .text_color(tc)
            .child(SharedString::from(ph))
    }

    /// Scrollable event list from filtered_events().
    fn render_event_list(&self, colors: &EventsColors) -> gpui::Div {
        let filtered = self.state.filtered_events();
        let mut body =
            div().flex().flex_col().flex_1().w_full().overflow_hidden().bg(colors.background);

        for (i, event) in filtered.iter().enumerate() {
            body = body.child(self.render_event_row(event, i, colors));
        }

        body
    }

    /// Single event row with severity dot, age, reason, message,
    /// resource, and count badge.
    fn render_event_row(
        &self,
        event: &EventRow,
        index: usize,
        colors: &EventsColors,
    ) -> gpui::Stateful<gpui::Div> {
        let sev_color = self.severity_color(&event.event_type).to_gpui();
        let ids = format!("event-{index}");

        let dot = self.render_severity_dot(sev_color);
        let age_el = self.render_event_age(&event.age, colors);
        let reason_el = self.render_event_reason(&event.reason, colors);
        let msg_el = self.render_event_message(&event.message, colors);
        let res_el = self.render_event_resource(&event.resource_display(), colors);
        let count_el = self.render_event_count(event.count, colors);

        div()
            .id(ElementId::Name(SharedString::from(ids)))
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .px_2()
            .py_1()
            .gap(px(8.0))
            .border_b_1()
            .border_color(colors.border)
            .child(dot)
            .child(age_el)
            .child(reason_el)
            .child(msg_el)
            .child(res_el)
            .child(count_el)
    }

    /// Colored severity indicator dot.
    fn render_severity_dot(&self, color: Rgba) -> gpui::Div {
        div().w(px(8.0)).h(px(8.0)).rounded(px(4.0)).flex_shrink_0().bg(color)
    }

    /// Event age label.
    fn render_event_age(&self, age: &str, colors: &EventsColors) -> gpui::Div {
        div()
            .w(px(48.0))
            .flex_shrink_0()
            .text_xs()
            .text_color(colors.text_muted)
            .child(SharedString::from(age.to_string()))
    }

    /// Event reason label.
    fn render_event_reason(&self, reason: &str, colors: &EventsColors) -> gpui::Div {
        div()
            .w(px(100.0))
            .flex_shrink_0()
            .text_xs()
            .text_color(colors.text_primary)
            .child(SharedString::from(reason.to_string()))
    }

    /// Event message (flexible width).
    fn render_event_message(&self, message: &str, colors: &EventsColors) -> gpui::Div {
        div()
            .flex_1()
            .text_xs()
            .text_color(colors.text_secondary)
            .child(SharedString::from(message.to_string()))
    }

    /// Resource kind/name display.
    fn render_event_resource(&self, resource: &str, colors: &EventsColors) -> gpui::Div {
        div()
            .w(px(120.0))
            .flex_shrink_0()
            .text_xs()
            .text_color(colors.text_muted)
            .child(SharedString::from(resource.to_string()))
    }

    /// Event count badge.
    fn render_event_count(&self, count: u32, colors: &EventsColors) -> gpui::Div {
        if count <= 1 {
            return div();
        }
        div()
            .px_2()
            .py(px(1.0))
            .rounded(px(8.0))
            .bg(colors.surface)
            .text_xs()
            .text_color(colors.text_secondary)
            .child(SharedString::from(format!("x{count}")))
    }

    /// Empty state when no events match.
    fn render_empty_state(&self, colors: &EventsColors) -> gpui::Div {
        div().flex().items_center().justify_center().flex_1().py(px(32.0)).child(
            div().text_sm().text_color(colors.text_muted).child(SharedString::from("No events")),
        )
    }

    /// Loading indicator.
    fn render_loading(&self, colors: &EventsColors) -> gpui::Div {
        div().flex().items_center().justify_center().flex_1().py(px(32.0)).child(
            div()
                .text_sm()
                .text_color(colors.text_muted)
                .child(SharedString::from("Loading events...")),
        )
    }

    /// Error state display.
    fn render_error(&self, message: &str, colors: &EventsColors) -> gpui::Div {
        div().flex().items_center().justify_center().flex_1().py(px(32.0)).child(
            div().text_sm().text_color(colors.error).child(SharedString::from(message.to_string())),
        )
    }
}

// ---------------------------------------------------------------------------
// impl Render
// ---------------------------------------------------------------------------

impl Render for EventsViewComponent {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let colors = self.colors();

        let mut base = div().flex().flex_col().size_full().bg(colors.background);

        base = base.child(self.render_toolbar(&colors));

        if self.state.loading {
            base = base.child(self.render_loading(&colors));
        } else if let Some(ref err) = self.state.error {
            let msg = err.clone();
            base = base.child(self.render_error(&msg, &colors));
        } else if self.state.filtered_events().is_empty() {
            base = base.child(self.render_empty_state(&colors));
        } else {
            base = base.child(self.render_event_list(&colors));
        }

        base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_event(uid: &str, event_type: EventType, reason: &str) -> EventRow {
        EventRow {
            uid: uid.to_string(),
            event_type,
            reason: reason.to_string(),
            message: format!("{reason} message"),
            resource_kind: Some("Pod".to_string()),
            resource_name: Some("nginx".to_string()),
            namespace: Some("default".to_string()),
            age: "5m".to_string(),
            count: 1,
        }
    }

    fn make_events_view() -> EventsViewState {
        let mut state = EventsViewState::default();
        state.set_events(vec![
            sample_event("e1", EventType::Normal, "Scheduled"),
            sample_event("e2", EventType::Normal, "Pulled"),
            sample_event("e3", EventType::Warning, "BackOff"),
            sample_event("e4", EventType::Warning, "FailedMount"),
            {
                let mut e = sample_event("e5", EventType::Normal, "Started");
                e.namespace = Some("kube-system".to_string());
                e.resource_kind = Some("Deployment".to_string());
                e.resource_name = Some("coredns".to_string());
                e
            },
        ]);
        state
    }

    // --- EventSeverityFilter tests ---

    #[test]
    fn test_severity_filter_labels() {
        assert_eq!(EventSeverityFilter::All.label(), "All");
        assert_eq!(EventSeverityFilter::Normal.label(), "Normal");
        assert_eq!(EventSeverityFilter::Warning.label(), "Warning");
    }

    #[test]
    fn test_severity_filter_matches() {
        assert!(EventSeverityFilter::All.matches(&EventType::Normal));
        assert!(EventSeverityFilter::All.matches(&EventType::Warning));
        assert!(EventSeverityFilter::Normal.matches(&EventType::Normal));
        assert!(!EventSeverityFilter::Normal.matches(&EventType::Warning));
        assert!(!EventSeverityFilter::Warning.matches(&EventType::Normal));
        assert!(EventSeverityFilter::Warning.matches(&EventType::Warning));
    }

    // --- EventRow tests ---

    #[test]
    fn test_event_row_severity_label() {
        let normal = sample_event("e1", EventType::Normal, "Started");
        assert_eq!(normal.severity_label(), "Normal");

        let warning = sample_event("e2", EventType::Warning, "BackOff");
        assert_eq!(warning.severity_label(), "Warning");
    }

    #[test]
    fn test_event_row_is_warning() {
        assert!(!sample_event("e1", EventType::Normal, "OK").is_warning());
        assert!(sample_event("e2", EventType::Warning, "Bad").is_warning());
    }

    #[test]
    fn test_event_row_resource_display() {
        let event = sample_event("e1", EventType::Normal, "Started");
        assert_eq!(event.resource_display(), "Pod/nginx");

        let no_name =
            EventRow { resource_name: None, ..sample_event("e2", EventType::Normal, "x") };
        assert_eq!(no_name.resource_display(), "Pod");

        let no_kind = EventRow {
            resource_kind: None,
            resource_name: None,
            ..sample_event("e3", EventType::Normal, "x")
        };
        assert_eq!(no_kind.resource_display(), "");
    }

    // --- EventsViewState tests ---

    #[test]
    fn test_default_state() {
        let state = EventsViewState::default();
        assert!(state.events.is_empty());
        assert_eq!(state.severity_filter, EventSeverityFilter::All);
        assert!(state.namespace_filter.is_none());
        assert!(state.search_query.is_empty());
        assert!(state.auto_scroll);
        assert_eq!(state.max_events, 1000);
    }

    #[test]
    fn test_set_events() {
        let state = make_events_view();
        assert_eq!(state.total_count(), 5);
    }

    #[test]
    fn test_push_event() {
        let mut state = EventsViewState::default();
        state.push_event(sample_event("e1", EventType::Normal, "Started"));
        assert_eq!(state.total_count(), 1);
    }

    #[test]
    fn test_push_event_trims_to_max() {
        let mut state = EventsViewState::default();
        state.max_events = 3;
        for i in 0..5 {
            state.push_event(sample_event(&format!("e{i}"), EventType::Normal, "x"));
        }
        assert_eq!(state.total_count(), 3);
        // Oldest events should be removed
        assert_eq!(state.events[0].uid, "e2");
    }

    #[test]
    fn test_warning_and_normal_counts() {
        let state = make_events_view();
        assert_eq!(state.warning_count(), 2);
        assert_eq!(state.normal_count(), 3);
    }

    #[test]
    fn test_filter_by_severity() {
        let mut state = make_events_view();
        assert_eq!(state.filtered_events().len(), 5);

        state.set_severity_filter(EventSeverityFilter::Warning);
        assert_eq!(state.filtered_events().len(), 2);

        state.set_severity_filter(EventSeverityFilter::Normal);
        assert_eq!(state.filtered_events().len(), 3);

        state.set_severity_filter(EventSeverityFilter::All);
        assert_eq!(state.filtered_events().len(), 5);
    }

    #[test]
    fn test_filter_by_namespace() {
        let mut state = make_events_view();

        state.set_namespace_filter(Some("default".to_string()));
        assert_eq!(state.filtered_events().len(), 4);

        state.set_namespace_filter(Some("kube-system".to_string()));
        assert_eq!(state.filtered_events().len(), 1);

        state.set_namespace_filter(None);
        assert_eq!(state.filtered_events().len(), 5);
    }

    #[test]
    fn test_filter_by_resource_kind() {
        let mut state = make_events_view();

        state.set_resource_kind_filter(Some("Pod".to_string()));
        assert_eq!(state.filtered_events().len(), 4);

        state.set_resource_kind_filter(Some("Deployment".to_string()));
        assert_eq!(state.filtered_events().len(), 1);

        state.set_resource_kind_filter(None);
        assert_eq!(state.filtered_events().len(), 5);
    }

    #[test]
    fn test_filter_by_search_query() {
        let mut state = make_events_view();

        state.set_search_query("BackOff");
        assert_eq!(state.filtered_events().len(), 1);

        state.set_search_query("mount");
        assert_eq!(state.filtered_events().len(), 1);

        state.set_search_query("nginx");
        assert_eq!(state.filtered_events().len(), 4);

        state.set_search_query("");
        assert_eq!(state.filtered_events().len(), 5);
    }

    #[test]
    fn test_combined_filters() {
        let mut state = make_events_view();
        state.set_severity_filter(EventSeverityFilter::Warning);
        state.set_namespace_filter(Some("default".to_string()));

        let filtered = state.filtered_events();
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|e| e.is_warning()));
    }

    #[test]
    fn test_loading_and_error() {
        let mut state = EventsViewState::default();
        state.set_loading(true);
        assert!(state.loading);

        state.set_error("timeout".to_string());
        assert_eq!(state.error.as_deref(), Some("timeout"));

        state.clear_error();
        assert!(state.error.is_none());
    }

    #[test]
    fn test_toggle_auto_scroll() {
        let mut state = EventsViewState::default();
        assert!(state.auto_scroll);
        state.toggle_auto_scroll();
        assert!(!state.auto_scroll);
        state.toggle_auto_scroll();
        assert!(state.auto_scroll);
    }

    #[test]
    fn test_clear_events() {
        let mut state = make_events_view();
        assert!(!state.events.is_empty());
        state.clear_events();
        assert!(state.events.is_empty());
    }
}
