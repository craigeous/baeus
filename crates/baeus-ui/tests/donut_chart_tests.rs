//! Tests for the donut chart and resource count circle components.

use baeus_ui::components::donut_chart::{DonutChart, ResourceCountCircle, resource_kind_color};
use baeus_ui::theme::Color;

#[test]
fn test_resource_kind_color_known() {
    let c = resource_kind_color("Pods");
    assert_eq!(c.r, 0x4c);
    assert_eq!(c.g, 0xaf);
    assert_eq!(c.b, 0x50);
}

#[test]
fn test_resource_kind_color_unknown() {
    let c = resource_kind_color("Unknown");
    assert_eq!(c.r, 0x9e);
}

#[test]
fn test_donut_chart_pct_clamp() {
    let chart = DonutChart {
        label: "CPU",
        value: 0.756,
        used_label: "6.0 cores".to_string(),
        total_label: "8 cores".to_string(),
        color: Color::rgb(0x00, 0xa7, 0xa0),
        bg: Color::rgb(0x1e, 0x21, 0x24),
    };
    let pct = (chart.value * 100.0).round() as u32;
    assert_eq!(pct, 76);
}

#[test]
fn test_resource_count_circle_fields() {
    let circle =
        ResourceCountCircle { kind: "Pods", count: 42, color: resource_kind_color("Pods") };
    assert_eq!(circle.count, 42);
    assert_eq!(circle.kind, "Pods");
}

#[test]
fn test_resource_kind_colors_all() {
    let kinds =
        ["Pods", "Deployments", "DaemonSets", "StatefulSets", "ReplicaSets", "Jobs", "CronJobs"];
    for kind in &kinds {
        let c = resource_kind_color(kind);
        // All should have full alpha
        assert_eq!(c.a, 255, "Kind {} should have alpha=255", kind);
    }
}

#[test]
fn test_donut_chart_zero_value() {
    let chart = DonutChart {
        label: "CPU",
        value: 0.0,
        used_label: "N/A".to_string(),
        total_label: "N/A".to_string(),
        color: Color::rgb(0x00, 0xa7, 0xa0),
        bg: Color::rgb(0x1e, 0x21, 0x24),
    };
    let pct = (chart.value * 100.0).round() as u32;
    assert_eq!(pct, 0);
}

#[test]
fn test_donut_chart_full_value() {
    let chart = DonutChart {
        label: "Memory",
        value: 1.0,
        used_label: "16 GiB".to_string(),
        total_label: "16 GiB".to_string(),
        color: Color::rgb(0x21, 0x96, 0xf3),
        bg: Color::rgb(0x1e, 0x21, 0x24),
    };
    let pct = (chart.value * 100.0).round() as u32;
    assert_eq!(pct, 100);
}
