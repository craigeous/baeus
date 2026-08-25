//! Events tab rendering for the resource detail view.
//!
//! Shows events related to the currently viewed resource, filtered from
//! the cluster's event list by involvedObject kind/name.

use gpui::{FontWeight, Rgba, SharedString, div, prelude::*, px};

use crate::components::json_extract;
use crate::layout::app_shell::AppShell;

impl AppShell {
    /// Render the Events tab content for a resource detail view.
    ///
    /// Filters the global event list for events whose `involvedObject`
    /// matches the current resource's kind and name.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_resource_events_tab(
        &self,
        kind: &str,
        name: &str,
        namespace: Option<&str>,
        _json: &serde_json::Value,
        text: Rgba,
        text_secondary: Rgba,
        bg: Rgba,
        border: Rgba,
        _accent: Rgba,
    ) -> gpui::Stateful<gpui::Div> {
        let mut body =
            div().id("events-tab-body").flex().flex_col().flex_1().overflow_y_scroll().bg(bg);

        let events = self.find_related_events(kind, name, namespace);

        if events.is_empty() {
            body = body.child(
                div()
                    .flex()
                    .justify_center()
                    .py_8()
                    .text_sm()
                    .text_color(text_secondary)
                    .child("No events found for this resource"),
            );
            return body;
        }

        // Column header row
        body = body.child(render_events_header_row(text_secondary, border));

        // Event rows
        let warning_color = self.theme.colors.warning.to_gpui();
        for event in &events {
            body = body.child(render_event_row(event, text, text_secondary, border, warning_color));
        }

        body
    }

    /// Find events related to a specific resource by matching involvedObject.
    fn find_related_events(
        &self,
        kind: &str,
        name: &str,
        namespace: Option<&str>,
    ) -> Vec<RelatedEvent> {
        let mut events = Vec::new();

        for (key, data) in &self.resource_list_data {
            if key.kind != "Event" {
                continue;
            }
            for item in data {
                let involved_kind =
                    item.pointer("/involvedObject/kind").and_then(|v| v.as_str()).unwrap_or("");
                let involved_name =
                    item.pointer("/involvedObject/name").and_then(|v| v.as_str()).unwrap_or("");
                let involved_ns =
                    item.pointer("/involvedObject/namespace").and_then(|v| v.as_str());

                if involved_kind == kind && involved_name == name {
                    if let Some(ns) = namespace {
                        if let Some(inv_ns) = involved_ns {
                            if inv_ns != ns {
                                continue;
                            }
                        }
                    }
                    let event_ns = item
                        .pointer("/metadata/namespace")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let source = item
                        .pointer("/source/component")
                        .and_then(|v| v.as_str())
                        .unwrap_or("—")
                        .to_string();
                    let involved = item
                        .pointer("/involvedObject/kind")
                        .and_then(|v| v.as_str())
                        .map(|k| {
                            let n = item
                                .pointer("/involvedObject/name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            format!("{k}/{n}")
                        })
                        .unwrap_or_else(|| "—".to_string());

                    events.push(RelatedEvent {
                        event_type: item
                            .get("type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Normal")
                            .to_string(),
                        reason: item
                            .get("reason")
                            .and_then(|v| v.as_str())
                            .unwrap_or("—")
                            .to_string(),
                        message: item
                            .get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or("—")
                            .to_string(),
                        count: item.get("count").and_then(|v| v.as_i64()).unwrap_or(1),
                        first_seen: item
                            .get("firstTimestamp")
                            .or_else(|| item.get("eventTime"))
                            .and_then(|v: &serde_json::Value| v.as_str())
                            .map(json_extract::human_age)
                            .unwrap_or_else(|| "—".to_string()),
                        last_seen: item
                            .get("lastTimestamp")
                            .or_else(|| item.get("eventTime"))
                            .and_then(|v: &serde_json::Value| v.as_str())
                            .map(json_extract::human_age)
                            .unwrap_or_else(|| "—".to_string()),
                        namespace: event_ns,
                        source,
                        involved_object: involved,
                    });
                }
            }
        }

        events.reverse();
        events
    }
}

/// Render the events table header row.
fn render_events_header_row(text_secondary: Rgba, border: Rgba) -> gpui::Div {
    div()
        .flex()
        .flex_row()
        .w_full()
        .px_4()
        .py_1()
        .border_b_1()
        .border_color(border)
        .child(
            div()
                .w(px(70.0))
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(text_secondary)
                .child("Type"),
        )
        .child(
            div()
                .w(px(80.0))
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(text_secondary)
                .child("Namespace"),
        )
        .child(
            div()
                .w(px(100.0))
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(text_secondary)
                .child("Reason"),
        )
        .child(
            div()
                .flex_1()
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(text_secondary)
                .child("Message"),
        )
        .child(
            div()
                .w(px(120.0))
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(text_secondary)
                .child("Object"),
        )
        .child(
            div()
                .w(px(90.0))
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(text_secondary)
                .child("Source"),
        )
        .child(
            div()
                .w(px(40.0))
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(text_secondary)
                .child("Count"),
        )
        .child(
            div()
                .w(px(70.0))
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(text_secondary)
                .child("First Seen"),
        )
        .child(
            div()
                .w(px(70.0))
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(text_secondary)
                .child("Last Seen"),
        )
}

/// Render a single event row.
fn render_event_row(
    event: &RelatedEvent,
    text: Rgba,
    text_secondary: Rgba,
    border: Rgba,
    warning_color: Rgba,
) -> gpui::Div {
    let type_color = if event.event_type == "Warning" { warning_color } else { text_secondary };

    let mut row = div().flex().flex_row().w_full().px_4().py_1().border_b_1().border_color(border);

    // Subtle amber tint for warning rows
    if event.event_type == "Warning" {
        row = row.bg(Rgba { r: warning_color.r, g: warning_color.g, b: warning_color.b, a: 0.08 });
    }

    row.child(
        div()
            .w(px(70.0))
            .text_xs()
            .text_color(type_color)
            .child(SharedString::from(event.event_type.clone())),
    )
    .child(
        div()
            .w(px(80.0))
            .text_xs()
            .text_color(text_secondary)
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis()
            .child(SharedString::from(event.namespace.clone())),
    )
    .child(
        div()
            .w(px(100.0))
            .text_xs()
            .text_color(text)
            .child(SharedString::from(event.reason.clone())),
    )
    .child(
        div()
            .flex_1()
            .text_xs()
            .text_color(text_secondary)
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis()
            .child(SharedString::from(event.message.clone())),
    )
    .child(
        div()
            .w(px(120.0))
            .text_xs()
            .text_color(text_secondary)
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis()
            .child(SharedString::from(event.involved_object.clone())),
    )
    .child(
        div()
            .w(px(90.0))
            .text_xs()
            .text_color(text_secondary)
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis()
            .child(SharedString::from(event.source.clone())),
    )
    .child(
        div()
            .w(px(40.0))
            .text_xs()
            .text_color(text_secondary)
            .child(SharedString::from(event.count.to_string())),
    )
    .child(
        div()
            .w(px(70.0))
            .text_xs()
            .text_color(text_secondary)
            .child(SharedString::from(event.first_seen.clone())),
    )
    .child(
        div()
            .w(px(70.0))
            .text_xs()
            .text_color(text_secondary)
            .child(SharedString::from(event.last_seen.clone())),
    )
}

/// A related event extracted from the cluster's event list.
struct RelatedEvent {
    event_type: String,
    reason: String,
    message: String,
    count: i64,
    first_seen: String,
    last_seen: String,
    namespace: String,
    source: String,
    involved_object: String,
}
