//! Node-specific detail rendering methods for `AppShell`.
//!
//! Follows the same pattern as `pod_detail_render.rs` — `impl AppShell` methods
//! in a separate module to keep `app_shell.rs` slim.

use gpui::*;

use crate::components::json_extract;
use crate::icons::SectionIcon;
use crate::layout::app_shell::AppShell;

impl AppShell {
    /// Render the full Node detail body with all Node-specific sections.
    pub(crate) fn render_node_detail_body(
        &self,
        cx: &mut Context<Self>,
        json: &serde_json::Value,
        text: Rgba,
        text_secondary: Rgba,
        _bg: Rgba,
    ) -> Stateful<Div> {
        let border = self.theme.colors.border.to_gpui();
        let surface = self.theme.colors.surface.to_gpui();
        let accent = self.theme.colors.accent.to_gpui();

        let mut body = div()
            .id("node-detail-body")
            .flex()
            .flex_col()
            .flex_1()
            .overflow_y_scroll()
            .p_4()
            .gap_3();

        // --- Properties (Overview) ---
        let props = json_extract::extract_detail_properties("Node", json);
        body = body.child(self.render_pod_section(
            cx,
            SectionIcon::Info,
            "node-overview",
            &props,
            text,
            text_secondary,
            border,
            accent,
            |this: &AppShell, _cx, props, text, text_secondary, border, _accent| {
                this.render_detail_properties_body(props, text, text_secondary, border)
            },
        ));

        // --- Addresses ---
        let addresses = json_extract::extract_node_addresses(json);
        if !addresses.is_empty() {
            body = body.child(self.render_pod_section(
                cx,
                SectionIcon::Addresses,
                "node-addresses",
                &addresses,
                text,
                text_secondary,
                border,
                accent,
                |_this: &AppShell, _cx, addresses, text, text_secondary, border, _accent| {
                    render_kv_table(addresses, "Type", "Address", text, text_secondary, border)
                },
            ));
        }

        // --- Capacity ---
        let capacity = json_extract::extract_node_capacity(json);
        if !capacity.is_empty() {
            body = body.child(self.render_pod_section(
                cx,
                SectionIcon::Capacity,
                "node-capacity",
                &capacity,
                text,
                text_secondary,
                border,
                accent,
                |_this: &AppShell, _cx, items, text, text_secondary, border, _accent| {
                    render_kv_table(items, "Resource", "Value", text, text_secondary, border)
                },
            ));
        }

        // --- Allocatable ---
        let allocatable = json_extract::extract_node_allocatable(json);
        if !allocatable.is_empty() {
            body = body.child(self.render_pod_section(
                cx,
                SectionIcon::Allocatable,
                "node-allocatable",
                &allocatable,
                text,
                text_secondary,
                border,
                accent,
                |_this: &AppShell, _cx, items, text, text_secondary, border, _accent| {
                    render_kv_table(items, "Resource", "Value", text, text_secondary, border)
                },
            ));
        }

        // --- Labels ---
        let labels = json_extract::extract_labels(json);
        if !labels.is_empty() {
            body = body.child(self.render_pod_section(
                cx,
                SectionIcon::Labels,
                "node-labels",
                &labels,
                text,
                text_secondary,
                border,
                accent,
                |this: &AppShell, _cx, labels, _text, _text_secondary, _border, _accent| {
                    this.render_detail_label_badges_body(labels, surface)
                },
            ));
        }

        // --- Annotations ---
        let annotations = json_extract::extract_annotations(json);
        if !annotations.is_empty() {
            body = body.child(self.render_pod_section(
                cx,
                SectionIcon::Annotations,
                "node-annotations",
                &annotations,
                text,
                text_secondary,
                border,
                accent,
                |this: &AppShell, _cx, annotations, _text, _text_secondary, _border, _accent| {
                    this.render_detail_annotations_body(annotations, surface)
                },
            ));
        }

        // --- Conditions ---
        let conditions = json_extract::extract_conditions(json);
        if !conditions.is_empty() {
            body = body.child(self.render_pod_section(
                cx,
                SectionIcon::Conditions,
                "node-conditions",
                &conditions,
                text,
                text_secondary,
                border,
                accent,
                |this: &AppShell, _cx, conditions, _text, text_secondary, _border, _accent| {
                    this.render_detail_conditions_body(conditions, text_secondary, surface, border)
                },
            ));
        }

        // --- Images ---
        let images = json_extract::extract_node_images(json);
        if !images.is_empty() {
            body = body.child(self.render_pod_section(
                cx,
                SectionIcon::Images,
                "node-images",
                &images,
                text,
                text_secondary,
                border,
                accent,
                |_this: &AppShell, _cx, images, _text, text_secondary, border, _accent| {
                    render_images_table(images, text_secondary, border)
                },
            ));
        }

        body
    }
}

// ---------------------------------------------------------------------------
// Free functions for rendering Node sub-components
// ---------------------------------------------------------------------------

/// Render a two-column key-value table with custom column headers.
fn render_kv_table(
    pairs: &[(String, String)],
    key_header: &str,
    value_header: &str,
    _text: Rgba,
    text_secondary: Rgba,
    border: Rgba,
) -> Div {
    let mut table = div().flex().flex_col().gap_0();

    // Header
    table = table.child(
        div()
            .flex()
            .flex_row()
            .border_b_1()
            .border_color(border)
            .child(
                div()
                    .w(px(160.0))
                    .px_2()
                    .py_1()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(text_secondary)
                    .child(SharedString::from(key_header.to_string())),
            )
            .child(
                div()
                    .flex_1()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(text_secondary)
                    .child(SharedString::from(value_header.to_string())),
            ),
    );

    // Rows
    for (key, value) in pairs {
        table = table.child(
            div()
                .flex()
                .flex_row()
                .border_b_1()
                .border_color(border)
                .child(
                    div()
                        .w(px(160.0))
                        .px_2()
                        .py_1()
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(text_secondary)
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(SharedString::from(key.clone())),
                )
                .child(
                    div()
                        .flex_1()
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(gpui::rgb(0xE5E7EB))
                        .child(SharedString::from(value.clone())),
                ),
        );
    }

    table
}

/// Render the Node images list as a table: image name(s) + size.
fn render_images_table(
    images: &[json_extract::NodeImage],
    text_secondary: Rgba,
    border: Rgba,
) -> Div {
    let mut table = div().flex().flex_col().gap_0();

    // Header
    table = table.child(
        div()
            .flex()
            .flex_row()
            .border_b_1()
            .border_color(border)
            .child(
                div()
                    .flex_1()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(text_secondary)
                    .child("Image"),
            )
            .child(
                div()
                    .w(px(100.0))
                    .px_2()
                    .py_1()
                    .text_xs()
                    .font_weight(FontWeight::BOLD)
                    .text_color(text_secondary)
                    .child("Size"),
            ),
    );

    // Rows — show last name (short tag) for brevity
    for img in images.iter().take(50) {
        let display_name = img.names.last().cloned().unwrap_or_else(|| "<unknown>".to_string());
        let size_str = json_extract::format_bytes(img.size_bytes);

        table = table.child(
            div()
                .flex()
                .flex_row()
                .border_b_1()
                .border_color(border)
                .child(
                    div()
                        .flex_1()
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(gpui::rgb(0xE5E7EB))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(SharedString::from(display_name)),
                )
                .child(
                    div()
                        .w(px(100.0))
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(text_secondary)
                        .child(SharedString::from(size_str)),
                ),
        );
    }

    if images.len() > 50 {
        table = table.child(
            div()
                .text_xs()
                .text_color(text_secondary)
                .px_2()
                .py_1()
                .child(SharedString::from(format!("... and {} more", images.len() - 50))),
        );
    }

    table
}
