// T066: Render tests for EventsView (state-level, no GPUI window needed).
//
// Verifies:
// - Events render with severity indicators (Normal=blue, Warning=yellow)
// - Severity filter reduces visible events
// - Namespace filter works
// - Resource kind filter works
// - Search query filters by message/reason
// - Empty state when no events
// - Loading indicator
// - Error state
// - Auto-scroll tracking
// - Warning/normal count badges
// - Push event auto-trims to max_events

use baeus_core::EventType;
use baeus_ui::theme::Theme;
use baeus_ui::views::events::{
    EventRow, EventSeverityFilter, EventsViewComponent, EventsViewState,
};

// ========================================================================
// Helpers
// ========================================================================

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

fn make_component() -> EventsViewComponent {
    EventsViewComponent::new(EventsViewState::default(), Theme::dark())
}

fn make_component_with_events() -> EventsViewComponent {
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
    EventsViewComponent::new(state, Theme::dark())
}

// ========================================================================
// Severity indicators (Normal=info/blue, Warning=warning/yellow)
// ========================================================================

#[test]
fn test_normal_severity_color_is_info() {
    let comp = make_component();
    let color = comp.severity_color(&EventType::Normal);
    assert_eq!(color, Theme::dark().colors.info);
}

#[test]
fn test_warning_severity_color_is_warning() {
    let comp = make_component();
    let color = comp.severity_color(&EventType::Warning);
    assert_eq!(color, Theme::dark().colors.warning);
}

#[test]
fn test_severity_colors_are_different() {
    let comp = make_component();
    let normal = comp.severity_color(&EventType::Normal);
    let warning = comp.severity_color(&EventType::Warning);
    assert_ne!(normal, warning);
}

#[test]
fn test_severity_color_with_light_theme() {
    let comp = EventsViewComponent::new(EventsViewState::default(), Theme::light());
    let color = comp.severity_color(&EventType::Normal);
    assert_eq!(color, Theme::light().colors.info);
}

// ========================================================================
// Severity filter reduces visible events
// ========================================================================

#[test]
fn test_severity_filter_all_shows_all() {
    let comp = make_component_with_events();
    assert_eq!(comp.state.severity_filter, EventSeverityFilter::All,);
    assert_eq!(comp.state.filtered_events().len(), 5);
}

#[test]
fn test_severity_filter_warning_only() {
    let mut comp = make_component_with_events();
    comp.state.set_severity_filter(EventSeverityFilter::Warning);
    let filtered = comp.state.filtered_events();
    assert_eq!(filtered.len(), 2);
    assert!(filtered.iter().all(|e| e.is_warning()));
}

#[test]
fn test_severity_filter_normal_only() {
    let mut comp = make_component_with_events();
    comp.state.set_severity_filter(EventSeverityFilter::Normal);
    let filtered = comp.state.filtered_events();
    assert_eq!(filtered.len(), 3);
    assert!(filtered.iter().all(|e| !e.is_warning()));
}

#[test]
fn test_severity_filter_label_includes_count() {
    let comp = make_component_with_events();
    let all_label = comp.severity_filter_label(EventSeverityFilter::All);
    assert!(all_label.contains("5"));

    let normal_label = comp.severity_filter_label(EventSeverityFilter::Normal);
    assert!(normal_label.contains("3"));

    let warn_label = comp.severity_filter_label(EventSeverityFilter::Warning);
    assert!(warn_label.contains("2"));
}

// ========================================================================
// Namespace filter works
// ========================================================================

#[test]
fn test_namespace_filter_default() {
    let mut comp = make_component_with_events();
    comp.state.set_namespace_filter(Some("default".to_string()));
    assert_eq!(comp.state.filtered_events().len(), 4);
}

#[test]
fn test_namespace_filter_kube_system() {
    let mut comp = make_component_with_events();
    comp.state.set_namespace_filter(Some("kube-system".to_string()));
    assert_eq!(comp.state.filtered_events().len(), 1);
}

#[test]
fn test_namespace_filter_none_shows_all() {
    let mut comp = make_component_with_events();
    comp.state.set_namespace_filter(None);
    assert_eq!(comp.state.filtered_events().len(), 5);
}

#[test]
fn test_namespace_filter_nonexistent() {
    let mut comp = make_component_with_events();
    comp.state.set_namespace_filter(Some("ghost".to_string()));
    assert_eq!(comp.state.filtered_events().len(), 0);
}

// ========================================================================
// Resource kind filter works
// ========================================================================

#[test]
fn test_resource_kind_filter_pod() {
    let mut comp = make_component_with_events();
    comp.state.set_resource_kind_filter(Some("Pod".to_string()));
    assert_eq!(comp.state.filtered_events().len(), 4);
}

#[test]
fn test_resource_kind_filter_deployment() {
    let mut comp = make_component_with_events();
    comp.state.set_resource_kind_filter(Some("Deployment".to_string()));
    assert_eq!(comp.state.filtered_events().len(), 1);
}

#[test]
fn test_resource_kind_filter_none() {
    let mut comp = make_component_with_events();
    comp.state.set_resource_kind_filter(None);
    assert_eq!(comp.state.filtered_events().len(), 5);
}

// ========================================================================
// Search query filters by message/reason
// ========================================================================

#[test]
fn test_search_by_reason() {
    let mut comp = make_component_with_events();
    comp.state.set_search_query("BackOff");
    assert_eq!(comp.state.filtered_events().len(), 1);
}

#[test]
fn test_search_by_message() {
    let mut comp = make_component_with_events();
    comp.state.set_search_query("mount");
    // "FailedMount message" contains "mount"
    assert_eq!(comp.state.filtered_events().len(), 1);
}

#[test]
fn test_search_by_resource_display() {
    let mut comp = make_component_with_events();
    comp.state.set_search_query("nginx");
    // 4 events have resource_name=nginx
    assert_eq!(comp.state.filtered_events().len(), 4);
}

#[test]
fn test_search_case_insensitive() {
    let mut comp = make_component_with_events();
    comp.state.set_search_query("backoff");
    assert_eq!(comp.state.filtered_events().len(), 1);
}

#[test]
fn test_search_empty_shows_all() {
    let mut comp = make_component_with_events();
    comp.state.set_search_query("");
    assert_eq!(comp.state.filtered_events().len(), 5);
}

#[test]
fn test_search_no_match() {
    let mut comp = make_component_with_events();
    comp.state.set_search_query("nonexistent");
    assert_eq!(comp.state.filtered_events().len(), 0);
}

// ========================================================================
// Empty state when no events
// ========================================================================

#[test]
fn test_empty_state_no_events() {
    let comp = make_component();
    assert!(comp.state.events.is_empty());
    assert!(comp.state.filtered_events().is_empty());
}

#[test]
fn test_empty_after_clearing() {
    let mut comp = make_component_with_events();
    comp.state.clear_events();
    assert!(comp.state.events.is_empty());
    assert!(comp.state.filtered_events().is_empty());
}

#[test]
fn test_empty_due_to_filter() {
    let mut comp = make_component_with_events();
    comp.state.set_search_query("zzz_no_match_zzz");
    assert!(comp.state.filtered_events().is_empty());
}

// ========================================================================
// Loading indicator
// ========================================================================

#[test]
fn test_loading_false_by_default() {
    let comp = make_component();
    assert!(!comp.state.loading);
}

#[test]
fn test_loading_true() {
    let mut comp = make_component();
    comp.state.set_loading(true);
    assert!(comp.state.loading);
}

#[test]
fn test_loading_can_be_toggled() {
    let mut comp = make_component();
    comp.state.set_loading(true);
    comp.state.set_loading(false);
    assert!(!comp.state.loading);
}

// ========================================================================
// Error state
// ========================================================================

#[test]
fn test_error_none_by_default() {
    let comp = make_component();
    assert!(comp.state.error.is_none());
}

#[test]
fn test_set_error() {
    let mut comp = make_component();
    comp.state.set_error("connection timeout".to_string());
    assert_eq!(comp.state.error.as_deref(), Some("connection timeout"),);
}

#[test]
fn test_clear_error() {
    let mut comp = make_component();
    comp.state.set_error("error!".to_string());
    comp.state.clear_error();
    assert!(comp.state.error.is_none());
}

// ========================================================================
// Auto-scroll tracking
// ========================================================================

#[test]
fn test_auto_scroll_on_by_default() {
    let comp = make_component();
    assert!(comp.state.auto_scroll);
}

#[test]
fn test_auto_scroll_toggle_off() {
    let mut comp = make_component();
    comp.state.toggle_auto_scroll();
    assert!(!comp.state.auto_scroll);
}

#[test]
fn test_auto_scroll_toggle_back_on() {
    let mut comp = make_component();
    comp.state.toggle_auto_scroll();
    comp.state.toggle_auto_scroll();
    assert!(comp.state.auto_scroll);
}

// ========================================================================
// Warning/normal count badges
// ========================================================================

#[test]
fn test_warning_count() {
    let comp = make_component_with_events();
    assert_eq!(comp.state.warning_count(), 2);
}

#[test]
fn test_normal_count() {
    let comp = make_component_with_events();
    assert_eq!(comp.state.normal_count(), 3);
}

#[test]
fn test_total_count() {
    let comp = make_component_with_events();
    assert_eq!(comp.state.total_count(), 5);
}

#[test]
fn test_counts_with_no_events() {
    let comp = make_component();
    assert_eq!(comp.state.warning_count(), 0);
    assert_eq!(comp.state.normal_count(), 0);
    assert_eq!(comp.state.total_count(), 0);
}

// ========================================================================
// Push event auto-trims to max_events
// ========================================================================

#[test]
fn test_push_event_within_limit() {
    let mut comp = make_component();
    comp.state.push_event(sample_event("e1", EventType::Normal, "Started"));
    assert_eq!(comp.state.total_count(), 1);
}

#[test]
fn test_push_event_trims_oldest() {
    let mut comp = make_component();
    comp.state.max_events = 3;
    for i in 0..5 {
        comp.state.push_event(sample_event(&format!("e{i}"), EventType::Normal, "x"));
    }
    assert_eq!(comp.state.total_count(), 3);
    assert_eq!(comp.state.events[0].uid, "e2");
    assert_eq!(comp.state.events[2].uid, "e4");
}

#[test]
fn test_push_event_max_events_exact() {
    let mut comp = make_component();
    comp.state.max_events = 3;
    for i in 0..3 {
        comp.state.push_event(sample_event(&format!("e{i}"), EventType::Normal, "x"));
    }
    assert_eq!(comp.state.total_count(), 3);
    assert_eq!(comp.state.events[0].uid, "e0");
}

// ========================================================================
// Combined filters
// ========================================================================

#[test]
fn test_combined_severity_and_namespace() {
    let mut comp = make_component_with_events();
    comp.state.set_severity_filter(EventSeverityFilter::Warning);
    comp.state.set_namespace_filter(Some("default".to_string()));
    let filtered = comp.state.filtered_events();
    assert_eq!(filtered.len(), 2);
    assert!(filtered.iter().all(|e| e.is_warning()));
}

#[test]
fn test_combined_severity_and_search() {
    let mut comp = make_component_with_events();
    comp.state.set_severity_filter(EventSeverityFilter::Normal);
    comp.state.set_search_query("Scheduled");
    let filtered = comp.state.filtered_events();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].reason, "Scheduled");
}

#[test]
fn test_combined_all_filters() {
    let mut comp = make_component_with_events();
    comp.state.set_severity_filter(EventSeverityFilter::Normal);
    comp.state.set_namespace_filter(Some("default".to_string()));
    comp.state.set_resource_kind_filter(Some("Pod".to_string()));
    comp.state.set_search_query("Scheduled");
    let filtered = comp.state.filtered_events();
    assert_eq!(filtered.len(), 1);
}

// ========================================================================
// Event row helpers
// ========================================================================

#[test]
fn test_event_severity_label() {
    let normal = sample_event("e1", EventType::Normal, "Started");
    assert_eq!(normal.severity_label(), "Normal");

    let warning = sample_event("e2", EventType::Warning, "BackOff");
    assert_eq!(warning.severity_label(), "Warning");
}

#[test]
fn test_event_resource_display() {
    let event = sample_event("e1", EventType::Normal, "Started");
    assert_eq!(event.resource_display(), "Pod/nginx");
}

#[test]
fn test_event_resource_display_no_name() {
    let event = EventRow { resource_name: None, ..sample_event("e1", EventType::Normal, "x") };
    assert_eq!(event.resource_display(), "Pod");
}

#[test]
fn test_event_resource_display_no_kind() {
    let event = EventRow {
        resource_kind: None,
        resource_name: None,
        ..sample_event("e1", EventType::Normal, "x")
    };
    assert_eq!(event.resource_display(), "");
}

// ========================================================================
// Full workflow
// ========================================================================

#[test]
fn test_full_events_workflow() {
    let mut state = EventsViewState::default();

    // Start loading
    state.set_loading(true);
    assert!(state.loading);

    // Receive events
    state.set_loading(false);
    for i in 0..10 {
        let et = if i % 3 == 0 { EventType::Warning } else { EventType::Normal };
        state.push_event(sample_event(&format!("e{i}"), et, &format!("Reason{i}")));
    }
    assert_eq!(state.total_count(), 10);

    // Filter warnings
    state.set_severity_filter(EventSeverityFilter::Warning);
    let warnings = state.filtered_events();
    assert_eq!(warnings.len(), 4); // i=0,3,6,9

    // Search
    state.set_severity_filter(EventSeverityFilter::All);
    state.set_search_query("Reason5");
    assert_eq!(state.filtered_events().len(), 1);

    // Clear search
    state.set_search_query("");
    assert_eq!(state.filtered_events().len(), 10);

    // Toggle auto-scroll
    state.toggle_auto_scroll();
    assert!(!state.auto_scroll);

    // Create component
    let comp = EventsViewComponent::new(state, Theme::dark());
    assert_eq!(comp.state.warning_count(), 4);
    assert_eq!(comp.state.normal_count(), 6);
}

#[test]
fn test_component_new_defaults() {
    let comp = make_component();
    assert!(comp.state.events.is_empty());
    assert_eq!(comp.state.severity_filter, EventSeverityFilter::All,);
    assert!(comp.state.namespace_filter.is_none());
    assert!(comp.state.resource_kind_filter.is_none());
    assert!(comp.state.search_query.is_empty());
    assert!(!comp.state.loading);
    assert!(comp.state.error.is_none());
    assert!(comp.state.auto_scroll);
    assert_eq!(comp.state.max_events, 1000);
}
