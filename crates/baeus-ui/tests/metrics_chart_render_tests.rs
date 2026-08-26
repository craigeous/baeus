// T065: Render tests for MetricsChart (state-level, no GPUI window needed).
//
// Verifies:
// - Chart renders with data points (verify series data accessible)
// - Chart axes: Y-axis has labels, X-axis has time labels
// - Legend renders series labels and colors
// - Multiple series distinguished by label
// - Chart styles (Line/Area/Bar) tracked
// - Empty state message when no data
// - Metrics unavailable state shows guidance message
// - Time range setting affects display
// - format_bytes() and format_cpu() utilities
// - T069: Data loading integration (begin_loading, load_complete, push_data_point)
// - T070: Graceful degradation (metrics unavailable with guidance + check cluster)

use baeus_ui::components::metrics_chart::{
    ChartStyle, MetricKind, MetricSeries, MetricsChartComponent, MetricsChartState, format_bytes,
    format_cpu,
};
use baeus_ui::theme::Theme;

// ========================================================================
// Helpers
// ========================================================================

fn make_component() -> MetricsChartComponent {
    MetricsChartComponent::new(MetricsChartState::default(), Theme::dark())
}

fn make_component_with_data() -> MetricsChartComponent {
    let mut state = MetricsChartState::default();
    let mut cpu = MetricSeries::new("node-1", MetricKind::Cpu).with_capacity(4000.0);
    cpu.push(1000.0, 500.0);
    cpu.push(2000.0, 1200.0);
    cpu.push(3000.0, 800.0);
    state.add_series(cpu);

    let mut mem =
        MetricSeries::new("node-1-mem", MetricKind::Memory).with_capacity(8_589_934_592.0);
    mem.push(1000.0, 2_147_483_648.0);
    mem.push(2000.0, 3_221_225_472.0);
    mem.push(3000.0, 4_294_967_296.0);
    state.add_series(mem);

    MetricsChartComponent::new(state, Theme::dark())
}

// ========================================================================
// Chart renders with data points
// ========================================================================

#[test]
fn test_chart_series_accessible_after_creation() {
    let comp = make_component_with_data();
    assert_eq!(comp.state.series_count(), 2);
    assert!(comp.state.has_data());
}

#[test]
fn test_chart_series_labels_distinguishable() {
    let comp = make_component_with_data();
    let labels: Vec<&str> = comp.state.series.iter().map(|s| s.label.as_str()).collect();
    assert_eq!(labels, vec!["node-1", "node-1-mem"]);
}

#[test]
fn test_chart_latest_values_accessible() {
    let comp = make_component_with_data();
    let cpu = &comp.state.series[0];
    assert_eq!(cpu.latest_value(), Some(800.0));
    let mem = &comp.state.series[1];
    assert_eq!(mem.latest_value(), Some(4_294_967_296.0));
}

// ========================================================================
// Y-axis labels
// ========================================================================

#[test]
fn test_y_axis_labels_generated() {
    let comp = make_component_with_data();
    let labels = comp.y_axis_labels();
    assert_eq!(labels.len(), 5); // 0, 25%, 50%, 75%, 100% of max
}

#[test]
fn test_y_axis_labels_start_at_zero() {
    let comp = make_component_with_data();
    let labels = comp.y_axis_labels();
    // First label should represent 0
    assert_eq!(&labels[0], "0m");
}

#[test]
fn test_y_axis_labels_increase() {
    let comp = make_component_with_data();
    let labels = comp.y_axis_labels();
    // Labels should be ascending
    assert_ne!(labels[0], labels[4]);
}

// ========================================================================
// X-axis time labels
// ========================================================================

#[test]
fn test_x_axis_labels_generated() {
    let comp = make_component_with_data();
    let labels = comp.x_axis_labels();
    assert_eq!(labels.len(), 5); // 5 time ticks
}

#[test]
fn test_x_axis_labels_start_with_now() {
    let comp = make_component_with_data();
    let labels = comp.x_axis_labels();
    assert_eq!(&labels[0], "now");
}

#[test]
fn test_x_axis_labels_end_with_range() {
    let comp = make_component_with_data();
    // Default time range is 3600s = 1h
    let labels = comp.x_axis_labels();
    assert_eq!(&labels[4], "1h");
}

// ========================================================================
// Legend renders series labels and colors
// ========================================================================

#[test]
fn test_legend_visible_when_show_legend_true() {
    let comp = make_component_with_data();
    assert!(comp.state.show_legend);
}

#[test]
fn test_legend_series_colors_distinct() {
    let c0 = MetricsChartComponent::series_color(0);
    let c1 = MetricsChartComponent::series_color(1);
    assert_ne!(c0, c1);
}

#[test]
fn test_legend_color_wraps_around() {
    let c0 = MetricsChartComponent::series_color(0);
    let c6 = MetricsChartComponent::series_color(6);
    assert_eq!(c0, c6); // palette has 6 entries
}

#[test]
fn test_legend_not_shown_when_disabled() {
    let state = MetricsChartState { show_legend: false, ..Default::default() };
    let comp = MetricsChartComponent::new(state, Theme::dark());
    assert!(!comp.state.show_legend);
}

// ========================================================================
// Multiple series distinguished by label
// ========================================================================

#[test]
fn test_two_series_have_different_labels() {
    let comp = make_component_with_data();
    assert_ne!(comp.state.series[0].label, comp.state.series[1].label,);
}

#[test]
fn test_series_kinds_tracked_separately() {
    let comp = make_component_with_data();
    assert_eq!(comp.state.series[0].kind, MetricKind::Cpu,);
    assert_eq!(comp.state.series[1].kind, MetricKind::Memory,);
}

// ========================================================================
// Chart styles (Line/Area/Bar) tracked
// ========================================================================

#[test]
fn test_default_chart_style_is_area() {
    let comp = make_component();
    assert_eq!(comp.state.chart_style, ChartStyle::Area);
}

#[test]
fn test_chart_style_line() {
    let state = MetricsChartState::new(ChartStyle::Line);
    let comp = MetricsChartComponent::new(state, Theme::dark());
    assert_eq!(comp.state.chart_style, ChartStyle::Line);
}

#[test]
fn test_chart_style_bar() {
    let state = MetricsChartState::new(ChartStyle::Bar);
    let comp = MetricsChartComponent::new(state, Theme::dark());
    assert_eq!(comp.state.chart_style, ChartStyle::Bar);
}

#[test]
fn test_chart_style_label() {
    assert_eq!(ChartStyle::Line.label(), "Line");
    assert_eq!(ChartStyle::Area.label(), "Area");
    assert_eq!(ChartStyle::Bar.label(), "Bar");
}

#[test]
fn test_chart_style_all_variants() {
    let all = ChartStyle::all();
    assert_eq!(all.len(), 3);
}

// ========================================================================
// Empty state message when no data
// ========================================================================

#[test]
fn test_empty_state_shown_when_no_series() {
    let comp = make_component();
    assert!(comp.state.should_show_empty_state());
    assert_eq!(comp.state.empty_state_title(), "No Metrics Data");
}

#[test]
fn test_empty_state_not_shown_with_data() {
    let comp = make_component_with_data();
    assert!(!comp.state.should_show_empty_state());
}

#[test]
fn test_empty_state_custom_message() {
    let state = MetricsChartState {
        empty_state_message: Some("Waiting for data...".to_string()),
        ..Default::default()
    };
    let comp = MetricsChartComponent::new(state, Theme::dark());
    assert_eq!(comp.state.empty_state_message.as_deref(), Some("Waiting for data..."),);
}

// ========================================================================
// Metrics unavailable state shows guidance message
// ========================================================================

#[test]
fn test_unavailable_shows_empty_state() {
    let mut state = MetricsChartState::default();
    state.set_unavailable("not installed");
    let comp = MetricsChartComponent::new(state, Theme::dark());
    assert!(comp.state.should_show_empty_state());
    assert_eq!(comp.state.empty_state_title(), "Metrics server is not available",);
}

#[test]
fn test_unavailable_guidance_contains_instructions() {
    let mut state = MetricsChartState::default();
    state.set_unavailable("not installed");
    let comp = MetricsChartComponent::new(state, Theme::dark());
    let guidance = comp.state.setup_guidance();
    assert!(guidance.contains("metrics-server"));
    assert!(guidance.contains("kubectl apply"));
}

#[test]
fn test_available_guidance_is_empty() {
    let comp = make_component();
    assert_eq!(comp.state.setup_guidance(), "");
}

#[test]
fn test_set_available_clears_unavailable() {
    let mut state = MetricsChartState::default();
    state.set_unavailable("not installed");
    state.set_available();
    let comp = MetricsChartComponent::new(state, Theme::dark());
    assert!(comp.state.metrics_available);
    assert!(comp.state.empty_state_message.is_none());
}

// ========================================================================
// Time range setting affects display
// ========================================================================

#[test]
fn test_default_time_range_1h() {
    let comp = make_component();
    assert_eq!(comp.state.time_range_secs, 3600);
    assert_eq!(comp.time_range_label(), "1h");
}

#[test]
fn test_time_range_5m() {
    let mut state = MetricsChartState::default();
    state.set_time_range(300);
    let comp = MetricsChartComponent::new(state, Theme::dark());
    assert_eq!(comp.state.time_range_secs, 300);
    assert_eq!(comp.time_range_label(), "5m");
}

#[test]
fn test_time_range_24h() {
    let mut state = MetricsChartState::default();
    state.set_time_range(86400);
    let comp = MetricsChartComponent::new(state, Theme::dark());
    assert_eq!(comp.time_range_label(), "24h");
}

#[test]
fn test_time_range_custom() {
    let mut state = MetricsChartState::default();
    state.set_time_range(7200); // 2h - not a preset
    let comp = MetricsChartComponent::new(state, Theme::dark());
    assert_eq!(comp.time_range_label(), "custom");
}

#[test]
fn test_x_axis_reflects_time_range() {
    let mut state = MetricsChartState::default();
    state.set_time_range(300); // 5m
    let mut cpu = MetricSeries::new("cpu", MetricKind::Cpu);
    cpu.push(1.0, 100.0);
    state.add_series(cpu);

    let comp = MetricsChartComponent::new(state, Theme::dark());
    let labels = comp.x_axis_labels();
    assert_eq!(&labels[0], "now");
    assert_eq!(&labels[4], "5m");
}

// ========================================================================
// format_bytes() and format_cpu() utilities
// ========================================================================

#[test]
fn test_format_bytes_small() {
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(500), "500 B");
}

#[test]
fn test_format_bytes_kib() {
    assert_eq!(format_bytes(1024), "1.0 KiB");
    assert_eq!(format_bytes(1536), "1.5 KiB");
}

#[test]
fn test_format_bytes_mib() {
    assert_eq!(format_bytes(1_048_576), "1.0 MiB");
    assert_eq!(format_bytes(1_572_864), "1.5 MiB");
}

#[test]
fn test_format_bytes_gib() {
    assert_eq!(format_bytes(1_073_741_824), "1.0 GiB");
    assert_eq!(format_bytes(1_610_612_736), "1.5 GiB");
}

#[test]
fn test_format_bytes_tib() {
    assert_eq!(format_bytes(1_099_511_627_776), "1.0 TiB");
    assert_eq!(format_bytes(1_649_267_441_664), "1.5 TiB");
}

#[test]
fn test_format_cpu_millicores() {
    assert_eq!(format_cpu(0), "0m");
    assert_eq!(format_cpu(250), "250m");
    assert_eq!(format_cpu(999), "999m");
}

#[test]
fn test_format_cpu_cores() {
    assert_eq!(format_cpu(1000), "1.0 cores");
    assert_eq!(format_cpu(2500), "2.5 cores");
    assert_eq!(format_cpu(8000), "8.0 cores");
}

// ========================================================================
// Format value via MetricsChartComponent
// ========================================================================

#[test]
fn test_format_value_cpu() {
    let s = MetricsChartComponent::format_value(MetricKind::Cpu, 1500.0);
    assert_eq!(s, "1.5 cores");
}

#[test]
fn test_format_value_memory() {
    let s = MetricsChartComponent::format_value(MetricKind::Memory, 1_073_741_824.0);
    assert_eq!(s, "1.0 GiB");
}

// ========================================================================
// Max across series
// ========================================================================

#[test]
fn test_max_across_series_with_data() {
    let comp = make_component_with_data();
    let max = comp.max_across_series();
    // Memory series has values up to ~4.3 billion
    assert!(max > 4_000_000_000.0);
}

#[test]
fn test_max_across_series_empty_defaults_to_100() {
    let comp = make_component();
    assert_eq!(comp.max_across_series(), 100.0);
}

// ========================================================================
// T069: Data loading integration
// ========================================================================

#[test]
fn test_begin_loading() {
    let mut comp = make_component();
    assert!(!comp.state.is_loading());
    comp.state.begin_loading();
    assert!(comp.state.is_loading());
}

#[test]
fn test_load_complete() {
    let mut comp = make_component();
    comp.state.begin_loading();
    comp.state.load_complete();
    assert!(!comp.state.is_loading());
}

#[test]
fn test_push_data_point_creates_new_series() {
    let mut comp = make_component();
    comp.state.push_data_point("node-1", MetricKind::Cpu, 1000.0, 500.0);
    assert_eq!(comp.state.series_count(), 1);
    assert_eq!(comp.state.series[0].label, "node-1");
    assert_eq!(comp.state.series[0].latest_value(), Some(500.0),);
}

#[test]
fn test_push_data_point_appends_to_existing() {
    let mut comp = make_component();
    comp.state.push_data_point("node-1", MetricKind::Cpu, 1000.0, 500.0);
    comp.state.push_data_point("node-1", MetricKind::Cpu, 2000.0, 750.0);
    assert_eq!(comp.state.series_count(), 1);
    assert_eq!(comp.state.series[0].len(), 2);
    assert_eq!(comp.state.series[0].latest_value(), Some(750.0),);
}

#[test]
fn test_push_data_point_multiple_series() {
    let mut comp = make_component();
    comp.state.push_data_point("cpu", MetricKind::Cpu, 1000.0, 500.0);
    comp.state.push_data_point("mem", MetricKind::Memory, 1000.0, 2_000_000_000.0);
    assert_eq!(comp.state.series_count(), 2);
    assert_eq!(comp.state.series[0].label, "cpu");
    assert_eq!(comp.state.series[1].label, "mem");
}

#[test]
fn test_loading_then_push_then_complete() {
    let mut comp = make_component();
    comp.state.begin_loading();
    assert!(comp.state.is_loading());

    comp.state.push_data_point("node-1", MetricKind::Cpu, 1000.0, 500.0);
    comp.state.load_complete();

    assert!(!comp.state.is_loading());
    assert!(comp.state.has_data());
    assert!(!comp.state.should_show_empty_state());
}

// ========================================================================
// T070: Graceful degradation
// ========================================================================

#[test]
fn test_graceful_degradation_unavailable_title() {
    let mut state = MetricsChartState::default();
    state.set_unavailable("not detected");
    let comp = MetricsChartComponent::new(state, Theme::dark());
    assert_eq!(comp.state.empty_state_title(), "Metrics server is not available",);
}

#[test]
fn test_graceful_degradation_guidance_text() {
    let mut state = MetricsChartState::default();
    state.set_unavailable("not detected");
    let comp = MetricsChartComponent::new(state, Theme::dark());
    let g = comp.state.setup_guidance();
    assert!(g.contains("Metrics server is not installed"));
    assert!(g.contains("kubectl apply"));
    assert!(g.contains("components.yaml"));
}

#[test]
fn test_graceful_degradation_recovery() {
    let mut state = MetricsChartState::default();
    state.set_unavailable("not detected");
    assert!(state.should_show_empty_state());

    state.set_available();
    let mut cpu = MetricSeries::new("cpu", MetricKind::Cpu);
    cpu.push(1.0, 100.0);
    state.add_series(cpu);
    assert!(!state.should_show_empty_state());
}

#[test]
fn test_graceful_degradation_available_empty_guidance() {
    let comp = make_component();
    assert_eq!(comp.state.setup_guidance(), "");
}

// ========================================================================
// Series summary values
// ========================================================================

#[test]
fn test_series_summary_current_value() {
    let comp = make_component_with_data();
    let cpu = &comp.state.series[0];
    assert_eq!(cpu.latest_value(), Some(800.0));
}

#[test]
fn test_series_summary_usage_percent() {
    let comp = make_component_with_data();
    let cpu = &comp.state.series[0];
    let pct = cpu.usage_percent().unwrap();
    assert!((pct - 20.0).abs() < 0.01);
}

#[test]
fn test_series_summary_min_max_avg() {
    let comp = make_component_with_data();
    let cpu = &comp.state.series[0];
    assert_eq!(cpu.min_value(), Some(500.0));
    assert_eq!(cpu.max_value(), Some(1200.0));
    let avg = cpu.average_value().unwrap();
    assert!((avg - 833.33).abs() < 1.0);
}

// ========================================================================
// Light theme support
// ========================================================================

#[test]
fn test_component_with_light_theme() {
    let state = MetricsChartState::default();
    let comp = MetricsChartComponent::new(state, Theme::light());
    assert_eq!(comp.theme.colors.background, Theme::light().colors.background,);
}

// ========================================================================
// Full workflow
// ========================================================================

#[test]
fn test_full_metrics_chart_workflow() {
    let mut state = MetricsChartState::default();

    // Start loading
    state.begin_loading();
    assert!(state.is_loading());

    // Receive data
    state.push_data_point("node-1-cpu", MetricKind::Cpu, 1000.0, 500.0);
    state.push_data_point("node-1-cpu", MetricKind::Cpu, 2000.0, 800.0);
    state.push_data_point("node-1-mem", MetricKind::Memory, 1000.0, 2_000_000_000.0);
    state.load_complete();

    assert!(!state.is_loading());
    assert_eq!(state.series_count(), 2);
    assert!(state.has_data());
    assert!(!state.should_show_empty_state());

    // Change time range
    state.set_time_range(300);
    assert_eq!(state.time_range_secs, 300);

    // Create component and verify rendering data
    let comp = MetricsChartComponent::new(state, Theme::dark());
    assert_eq!(comp.time_range_label(), "5m");
    assert_eq!(comp.y_axis_labels().len(), 5);
    assert_eq!(comp.x_axis_labels().len(), 5);
}
