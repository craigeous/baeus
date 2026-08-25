//! Donut chart and resource count circle components for the dashboard.

use gpui::*;

use crate::theme::Color;

/// A ring-style donut chart showing a usage percentage.
///
/// Rendered as a colored ring with a percentage label in the center.
/// Since GPUI lacks arc/gradient primitives, the ring is a solid-colored
/// circle background with a smaller inner circle cut-out showing the
/// dashboard background, giving a donut appearance.
pub struct DonutChart {
    pub label: &'static str,
    pub value: f32,          // 0.0–1.0 fraction
    pub used_label: String,  // e.g. "2.4 cores" or "45/78"
    pub total_label: String, // e.g. "8 cores" or "78 pods"
    pub color: Color,
    pub bg: Color,
}

impl DonutChart {
    /// Render the donut chart as a GPUI element.
    pub fn render(&self, text_primary: Rgba, text_secondary: Rgba) -> Div {
        let pct = (self.value * 100.0).round() as u32;
        let pct_text = SharedString::from(format!("{}%", pct));
        let used_text = SharedString::from(self.used_label.clone());
        let total_text = SharedString::from(self.total_label.clone());
        let label_text = SharedString::from(self.label.to_string());

        let ring_color = self.color.to_gpui();
        let bg_color = self.bg.to_gpui();

        // If value is 0 or N/A, show muted ring
        let outer_bg = if self.value > 0.0 {
            ring_color
        } else {
            Rgba { r: ring_color.r, g: ring_color.g, b: ring_color.b, a: 0.2 }
        };

        div()
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            // The donut ring
            .child(
                div()
                    .w(px(120.0))
                    .h(px(120.0))
                    .rounded_full()
                    .bg(outer_bg)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        // Inner circle (the "hole")
                        div()
                            .w(px(88.0))
                            .h(px(88.0))
                            .rounded_full()
                            .bg(bg_color)
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(text_primary)
                                    .child(pct_text),
                            )
                            .child(div().text_xs().text_color(text_secondary).child(used_text))
                            .child(div().text_xs().text_color(text_secondary).child(total_text)),
                    ),
            )
            // Label below
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(text_primary)
                    .child(label_text),
            )
    }
}

/// A small circle showing a resource count for the dashboard overview.
pub struct ResourceCountCircle {
    pub kind: &'static str,
    pub count: u32,
    pub color: Color,
}

/// Color palette for resource kinds.
pub fn resource_kind_color(kind: &str) -> Color {
    match kind {
        "Pods" => Color::rgb(0x4c, 0xaf, 0x50),         // green
        "Deployments" => Color::rgb(0x00, 0xa7, 0xa0),  // teal
        "DaemonSets" => Color::rgb(0xff, 0x98, 0x00),   // amber
        "StatefulSets" => Color::rgb(0xce, 0x39, 0x33), // red
        "ReplicaSets" => Color::rgb(0x7c, 0x4d, 0xff),  // purple
        "Jobs" => Color::rgb(0x21, 0x96, 0xf3),         // blue
        "CronJobs" => Color::rgb(0xff, 0x57, 0x22),     // deep orange
        _ => Color::rgb(0x9e, 0x9e, 0x9e),              // grey
    }
}

impl ResourceCountCircle {
    /// Render the resource count circle as a GPUI element.
    pub fn render(&self, text_primary: Rgba) -> Div {
        let count_text = SharedString::from(self.count.to_string());
        let kind_text = SharedString::from(self.kind.to_string());
        let circle_color = self.color.to_gpui();

        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(2.0))
            .child(
                div()
                    .w(px(56.0))
                    .h(px(56.0))
                    .rounded_full()
                    .bg(circle_color)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::BOLD)
                            .text_color(gpui::rgb(0xFFFFFF))
                            .child(count_text),
                    ),
            )
            .child(div().text_xs().text_color(text_primary).child(kind_text))
    }
}

/// A single entry in the resource distribution bar.
pub struct ResourceDistEntry {
    pub kind: &'static str,
    pub count: u32,
    pub color: Color,
}

/// A stacked horizontal bar showing the distribution of all resource types.
///
/// Each resource kind gets a colored segment proportional to its count.
/// A legend below the bar shows the color, kind, and count for each type.
pub struct ResourceDistributionBar;

impl ResourceDistributionBar {
    /// Render the distribution bar as a GPUI element.
    pub fn render(
        entries: &[ResourceDistEntry],
        text_primary: Rgba,
        text_secondary: Rgba,
        _bg: Rgba,
    ) -> Div {
        let total: u32 = entries.iter().map(|e| e.count).sum();

        let mut bar =
            div().flex().flex_row().w_full().h(px(28.0)).rounded(px(6.0)).overflow_hidden();

        if total == 0 {
            // Empty state: muted gray bar
            bar = bar.bg(Rgba { r: 0.4, g: 0.4, b: 0.4, a: 0.3 });
        } else {
            for entry in entries {
                if entry.count == 0 {
                    continue;
                }
                let fraction = entry.count as f32 / total as f32;
                let width_pct = (fraction * 100.0).round() as i32;
                let segment_color = entry.color.to_gpui();
                bar = bar.child(
                    div()
                        .h_full()
                        .bg(segment_color)
                        .flex_basis(relative(fraction))
                        .flex_grow()
                        .min_w(px(4.0)),
                );

                let _ = width_pct; // suppress unused
            }
        }

        // Legend: colored dot + kind name + count
        let mut legend = div().flex().flex_row().flex_wrap().gap_x(px(16.0)).gap_y(px(4.0)).pt_2();

        for entry in entries {
            let dot_color = entry.color.to_gpui();
            let label = SharedString::from(format!("{}: {}", entry.kind, entry.count));
            legend = legend.child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .child(div().w(px(10.0)).h(px(10.0)).rounded(px(2.0)).bg(dot_color))
                    .child(div().text_xs().text_color(text_primary).child(label)),
            );
        }

        // Total label
        let total_label = SharedString::from(format!("Total: {}", total));

        div()
            .flex()
            .flex_col()
            .w_full()
            .gap_1()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(text_primary)
                            .child("Resource Distribution"),
                    )
                    .child(div().text_xs().text_color(text_secondary).child(total_label)),
            )
            .child(bar)
            .child(legend)
    }
}

// Tests in crates/baeus-ui/tests/donut_chart_tests.rs (integration test file
// to avoid proc-macro stack overflow in lib tests).
