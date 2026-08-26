// Tests extracted from crates/baeus-ui/src/components/resource_table.rs

use baeus_ui::components::resource_table::*;
use baeus_ui::theme::Theme;

fn sample_columns() -> Vec<ColumnDef> {
    vec![
        ColumnDef {
            id: "name".to_string(),
            label: "Name".to_string(),
            sortable: true,
            width_weight: 2.0,
        },
        ColumnDef {
            id: "namespace".to_string(),
            label: "Namespace".to_string(),
            sortable: true,
            width_weight: 1.0,
        },
        ColumnDef {
            id: "status".to_string(),
            label: "Status".to_string(),
            sortable: false,
            width_weight: 1.0,
        },
    ]
}

fn sample_rows() -> Vec<TableRow> {
    vec![
        TableRow {
            uid: "uid-1".to_string(),
            cells: vec!["nginx-pod".to_string(), "default".to_string(), "Running".to_string()],
            selected: false,
            kind: "Pod".to_string(),
            name: "nginx-pod".to_string(),
            namespace: Some("default".to_string()),
            container_statuses: Vec::new(),
            conditions: Vec::new(),
        },
        TableRow {
            uid: "uid-2".to_string(),
            cells: vec!["redis-pod".to_string(), "cache".to_string(), "Pending".to_string()],
            selected: false,
            kind: "Pod".to_string(),
            name: "redis-pod".to_string(),
            namespace: Some("cache".to_string()),
            container_statuses: Vec::new(),
            conditions: Vec::new(),
        },
        TableRow {
            uid: "uid-3".to_string(),
            cells: vec!["api-deploy".to_string(), "production".to_string(), "Running".to_string()],
            selected: false,
            kind: "Deployment".to_string(),
            name: "api-deploy".to_string(),
            namespace: Some("production".to_string()),
            container_statuses: Vec::new(),
            conditions: Vec::new(),
        },
        TableRow {
            uid: "uid-4".to_string(),
            cells: vec!["cluster-role".to_string(), "".to_string(), "Active".to_string()],
            selected: false,
            kind: "ClusterRole".to_string(),
            name: "cluster-role".to_string(),
            namespace: None,
            container_statuses: Vec::new(),
            conditions: Vec::new(),
        },
    ]
}

// ========================================================================
// Construction tests
// ========================================================================

#[test]
fn test_new_creates_empty_state() {
    let state = ResourceTableState::new(sample_columns(), 10);
    assert_eq!(state.columns.len(), 3);
    assert!(state.rows.is_empty());
    assert!(state.sort.is_none());
    assert!(state.selected_uid.is_none());
    assert_eq!(state.scroll_offset, 0);
    assert_eq!(state.visible_rows, 10);
    assert!(state.filter_text.is_empty());
}

#[test]
fn test_new_with_zero_visible_rows() {
    let state = ResourceTableState::new(sample_columns(), 0);
    assert_eq!(state.visible_rows, 0);
}

#[test]
fn test_new_with_no_columns() {
    let state = ResourceTableState::new(vec![], 10);
    assert!(state.columns.is_empty());
}

// ========================================================================
// set_rows tests
// ========================================================================

#[test]
fn test_set_rows_populates_table() {
    let mut state = ResourceTableState::new(sample_columns(), 10);
    state.set_rows(sample_rows());
    assert_eq!(state.rows.len(), 4);
}

#[test]
fn test_set_rows_resets_scroll_offset() {
    let mut state = ResourceTableState::new(sample_columns(), 2);
    state.set_rows(sample_rows());
    state.scroll_to(2);
    assert_eq!(state.scroll_offset, 2);
    state.set_rows(sample_rows());
    assert_eq!(state.scroll_offset, 0);
}

#[test]
fn test_set_rows_replaces_existing() {
    let mut state = ResourceTableState::new(sample_columns(), 10);
    state.set_rows(sample_rows());
    assert_eq!(state.rows.len(), 4);
    state.set_rows(vec![sample_rows()[0].clone()]);
    assert_eq!(state.rows.len(), 1);
}

// ========================================================================
// sort_by tests
// ========================================================================

#[test]
fn test_sort_ascending_by_name() {
    let mut state = ResourceTableState::new(sample_columns(), 10);
    state.set_rows(sample_rows());
    state.sort_by("name");

    let sort = state.sort.as_ref().unwrap();
    assert_eq!(sort.column_id, "name");
    assert_eq!(sort.direction, SortDirection::Ascending);
    assert_eq!(state.rows[0].name, "api-deploy");
    assert_eq!(state.rows[1].name, "cluster-role");
    assert_eq!(state.rows[2].name, "nginx-pod");
    assert_eq!(state.rows[3].name, "redis-pod");
}

#[test]
fn test_sort_toggle_to_descending() {
    let mut state = ResourceTableState::new(sample_columns(), 10);
    state.set_rows(sample_rows());
    state.sort_by("name");
    state.sort_by("name");

    let sort = state.sort.as_ref().unwrap();
    assert_eq!(sort.direction, SortDirection::Descending);
    assert_eq!(state.rows[0].name, "redis-pod");
    assert_eq!(state.rows[3].name, "api-deploy");
}

#[test]
fn test_sort_toggle_back_to_ascending() {
    let mut state = ResourceTableState::new(sample_columns(), 10);
    state.set_rows(sample_rows());
    state.sort_by("name");
    state.sort_by("name");
    state.sort_by("name");

    let sort = state.sort.as_ref().unwrap();
    assert_eq!(sort.direction, SortDirection::Ascending);
    assert_eq!(state.rows[0].name, "api-deploy");
}

#[test]
fn test_sort_by_different_column_resets_to_ascending() {
    let mut state = ResourceTableState::new(sample_columns(), 10);
    state.set_rows(sample_rows());
    state.sort_by("name");
    state.sort_by("name"); // now descending
    state.sort_by("namespace"); // new column -> ascending

    let sort = state.sort.as_ref().unwrap();
    assert_eq!(sort.column_id, "namespace");
    assert_eq!(sort.direction, SortDirection::Ascending);
}

#[test]
fn test_sort_by_non_sortable_column_is_noop() {
    let mut state = ResourceTableState::new(sample_columns(), 10);
    state.set_rows(sample_rows());
    let original_first = state.rows[0].uid.clone();
    state.sort_by("status"); // not sortable

    assert!(state.sort.is_none());
    assert_eq!(state.rows[0].uid, original_first);
}

#[test]
fn test_sort_by_nonexistent_column_is_noop() {
    let mut state = ResourceTableState::new(sample_columns(), 10);
    state.set_rows(sample_rows());
    state.sort_by("nonexistent");
    assert!(state.sort.is_none());
}

#[test]
fn test_sort_empty_rows() {
    let mut state = ResourceTableState::new(sample_columns(), 10);
    state.sort_by("name");
    let sort = state.sort.as_ref().unwrap();
    assert_eq!(sort.column_id, "name");
    assert_eq!(sort.direction, SortDirection::Ascending);
}

// ========================================================================
// filtered_rows tests
// ========================================================================

#[test]
fn test_filtered_rows_no_filter_returns_all() {
    let mut state = ResourceTableState::new(sample_columns(), 10);
    state.set_rows(sample_rows());
    assert_eq!(state.filtered_rows().len(), 4);
}

#[test]
fn test_filtered_rows_by_name() {
    let mut state = ResourceTableState::new(sample_columns(), 10);
    state.set_rows(sample_rows());
    state.filter_text = "nginx".to_string();
    let filtered = state.filtered_rows();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].name, "nginx-pod");
}

#[test]
fn test_filtered_rows_by_namespace() {
    let mut state = ResourceTableState::new(sample_columns(), 10);
    state.set_rows(sample_rows());
    state.filter_text = "cache".to_string();
    let filtered = state.filtered_rows();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].name, "redis-pod");
}

#[test]
fn test_filtered_rows_by_cell_value() {
    let mut state = ResourceTableState::new(sample_columns(), 10);
    state.set_rows(sample_rows());
    state.filter_text = "Pending".to_string();
    let filtered = state.filtered_rows();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].name, "redis-pod");
}

#[test]
fn test_filtered_rows_case_insensitive() {
    let mut state = ResourceTableState::new(sample_columns(), 10);
    state.set_rows(sample_rows());
    state.filter_text = "NGINX".to_string();
    let filtered = state.filtered_rows();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].name, "nginx-pod");
}

#[test]
fn test_filtered_rows_no_match() {
    let mut state = ResourceTableState::new(sample_columns(), 10);
    state.set_rows(sample_rows());
    state.filter_text = "zzzzz".to_string();
    assert!(state.filtered_rows().is_empty());
}

#[test]
fn test_filtered_rows_partial_match_multiple() {
    let mut state = ResourceTableState::new(sample_columns(), 10);
    state.set_rows(sample_rows());
    state.filter_text = "pod".to_string();
    let filtered = state.filtered_rows();
    assert_eq!(filtered.len(), 2);
}

#[test]
fn test_filtered_rows_matches_running_status_in_cells() {
    let mut state = ResourceTableState::new(sample_columns(), 10);
    state.set_rows(sample_rows());
    state.filter_text = "Running".to_string();
    let filtered = state.filtered_rows();
    assert_eq!(filtered.len(), 2); // nginx-pod and api-deploy
}

// ========================================================================
// visible_slice tests
// ========================================================================

#[test]
fn test_visible_slice_first_page() {
    let mut state = ResourceTableState::new(sample_columns(), 2);
    state.set_rows(sample_rows());
    let slice = state.visible_slice();
    assert_eq!(slice.len(), 2);
    assert_eq!(slice[0].uid, "uid-1");
    assert_eq!(slice[1].uid, "uid-2");
}

#[test]
fn test_visible_slice_with_offset() {
    let mut state = ResourceTableState::new(sample_columns(), 2);
    state.set_rows(sample_rows());
    state.scroll_to(2);
    let slice = state.visible_slice();
    assert_eq!(slice.len(), 2);
    assert_eq!(slice[0].uid, "uid-3");
    assert_eq!(slice[1].uid, "uid-4");
}

#[test]
fn test_visible_slice_at_end_partial() {
    let mut state = ResourceTableState::new(sample_columns(), 3);
    state.set_rows(sample_rows());
    state.scroll_to(2);
    let slice = state.visible_slice();
    assert_eq!(slice.len(), 2); // only 2 rows left
}

#[test]
fn test_visible_slice_empty_rows() {
    let state = ResourceTableState::new(sample_columns(), 10);
    assert!(state.visible_slice().is_empty());
}

#[test]
fn test_visible_slice_with_filter() {
    let mut state = ResourceTableState::new(sample_columns(), 10);
    state.set_rows(sample_rows());
    state.filter_text = "pod".to_string();
    let slice = state.visible_slice();
    assert_eq!(slice.len(), 2);
}

#[test]
fn test_visible_slice_zero_visible_rows() {
    let mut state = ResourceTableState::new(sample_columns(), 0);
    state.set_rows(sample_rows());
    assert!(state.visible_slice().is_empty());
}

// ========================================================================
// scroll_to tests
// ========================================================================

#[test]
fn test_scroll_to_valid_offset() {
    let mut state = ResourceTableState::new(sample_columns(), 2);
    state.set_rows(sample_rows());
    state.scroll_to(1);
    assert_eq!(state.scroll_offset, 1);
}

#[test]
fn test_scroll_to_clamped_to_max() {
    let mut state = ResourceTableState::new(sample_columns(), 2);
    state.set_rows(sample_rows());
    state.scroll_to(100);
    assert_eq!(state.scroll_offset, 3); // 4 rows, max offset is 3
}

#[test]
fn test_scroll_to_zero() {
    let mut state = ResourceTableState::new(sample_columns(), 2);
    state.set_rows(sample_rows());
    state.scroll_to(2);
    state.scroll_to(0);
    assert_eq!(state.scroll_offset, 0);
}

#[test]
fn test_scroll_to_empty_table() {
    let mut state = ResourceTableState::new(sample_columns(), 10);
    state.scroll_to(5);
    assert_eq!(state.scroll_offset, 0);
}

#[test]
fn test_scroll_to_respects_filter() {
    let mut state = ResourceTableState::new(sample_columns(), 2);
    state.set_rows(sample_rows());
    state.filter_text = "pod".to_string(); // 2 matching rows
    state.scroll_to(10);
    assert_eq!(state.scroll_offset, 1); // max offset for 2 filtered rows
}

// ========================================================================
// select_row / selected_row tests
// ========================================================================

#[test]
fn test_select_row_sets_uid() {
    let mut state = ResourceTableState::new(sample_columns(), 10);
    state.set_rows(sample_rows());
    state.select_row("uid-2");
    assert_eq!(state.selected_uid.as_deref(), Some("uid-2"));
}

#[test]
fn test_selected_row_returns_matching() {
    let mut state = ResourceTableState::new(sample_columns(), 10);
    state.set_rows(sample_rows());
    state.select_row("uid-3");
    let selected = state.selected_row().unwrap();
    assert_eq!(selected.name, "api-deploy");
}

#[test]
fn test_selected_row_no_selection() {
    let mut state = ResourceTableState::new(sample_columns(), 10);
    state.set_rows(sample_rows());
    assert!(state.selected_row().is_none());
}

#[test]
fn test_selected_row_invalid_uid() {
    let mut state = ResourceTableState::new(sample_columns(), 10);
    state.set_rows(sample_rows());
    state.select_row("nonexistent");
    assert!(state.selected_row().is_none());
}

#[test]
fn test_select_row_overrides_previous() {
    let mut state = ResourceTableState::new(sample_columns(), 10);
    state.set_rows(sample_rows());
    state.select_row("uid-1");
    state.select_row("uid-4");
    let selected = state.selected_row().unwrap();
    assert_eq!(selected.name, "cluster-role");
}

// ========================================================================
// total_filtered_count tests
// ========================================================================

#[test]
fn test_total_filtered_count_no_filter() {
    let mut state = ResourceTableState::new(sample_columns(), 10);
    state.set_rows(sample_rows());
    assert_eq!(state.total_filtered_count(), 4);
}

#[test]
fn test_total_filtered_count_with_filter() {
    let mut state = ResourceTableState::new(sample_columns(), 10);
    state.set_rows(sample_rows());
    state.filter_text = "pod".to_string();
    assert_eq!(state.total_filtered_count(), 2);
}

#[test]
fn test_total_filtered_count_empty() {
    let state = ResourceTableState::new(sample_columns(), 10);
    assert_eq!(state.total_filtered_count(), 0);
}

#[test]
fn test_total_filtered_count_no_match() {
    let mut state = ResourceTableState::new(sample_columns(), 10);
    state.set_rows(sample_rows());
    state.filter_text = "zzzzz".to_string();
    assert_eq!(state.total_filtered_count(), 0);
}

// ========================================================================
// Integration / edge-case tests
// ========================================================================

#[test]
fn test_sort_then_filter_then_scroll() {
    let mut state = ResourceTableState::new(sample_columns(), 1);
    state.set_rows(sample_rows());
    state.sort_by("name"); // ascending
    state.filter_text = "pod".to_string(); // nginx-pod, redis-pod
    state.scroll_to(1);
    let slice = state.visible_slice();
    assert_eq!(slice.len(), 1);
    assert_eq!(slice[0].name, "redis-pod");
}

#[test]
fn test_filter_with_namespace_none() {
    let mut state = ResourceTableState::new(sample_columns(), 10);
    state.set_rows(sample_rows());
    state.filter_text = "cluster".to_string();
    let filtered = state.filtered_rows();
    assert_eq!(filtered.len(), 1);
    assert!(filtered[0].namespace.is_none());
}

#[test]
fn test_serde_sort_direction() {
    let asc = SortDirection::Ascending;
    let json = serde_json::to_string(&asc).unwrap();
    assert_eq!(json, "\"Ascending\"");
    let deser: SortDirection = serde_json::from_str(&json).unwrap();
    assert_eq!(deser, SortDirection::Ascending);

    let desc = SortDirection::Descending;
    let json = serde_json::to_string(&desc).unwrap();
    let deser: SortDirection = serde_json::from_str(&json).unwrap();
    assert_eq!(deser, SortDirection::Descending);
}

// ========================================================================
// T030: Render-related state tests for ResourceTable
// ========================================================================

#[test]
fn test_sort_indicator_ascending() {
    let mut state = ResourceTableState::new(sample_columns(), 10);
    state.set_rows(sample_rows());
    state.sort_by("name");

    let view = ResourceTableView::new(state, Theme::dark());
    assert_eq!(view.sort_indicator_for("name"), " ^");
    assert_eq!(view.sort_indicator_for("namespace"), "");
    assert_eq!(view.sort_indicator_for("status"), "");
}

#[test]
fn test_sort_indicator_descending() {
    let mut state = ResourceTableState::new(sample_columns(), 10);
    state.set_rows(sample_rows());
    state.sort_by("name");
    state.sort_by("name"); // toggle to descending

    let view = ResourceTableView::new(state, Theme::dark());
    assert_eq!(view.sort_indicator_for("name"), " v");
}

#[test]
fn test_sort_indicator_no_sort() {
    let state = ResourceTableState::new(sample_columns(), 10);
    let view = ResourceTableView::new(state, Theme::dark());
    assert_eq!(view.sort_indicator_for("name"), "");
    assert_eq!(view.sort_indicator_for("namespace"), "");
}

#[test]
fn test_view_with_light_theme() {
    let state = ResourceTableState::new(sample_columns(), 10);
    let view = ResourceTableView::new(state, Theme::light());
    assert_eq!(view.theme.colors.background, baeus_ui::theme::Color::rgb(255, 255, 255));
}

#[test]
fn test_view_with_dark_theme() {
    let state = ResourceTableState::new(sample_columns(), 10);
    let view = ResourceTableView::new(state, Theme::dark());
    assert_eq!(view.theme.colors.background, baeus_ui::theme::Color::rgb(0x1e, 0x21, 0x24));
}

#[test]
fn test_view_selected_row_highlighted() {
    let mut state = ResourceTableState::new(sample_columns(), 10);
    state.set_rows(sample_rows());
    state.select_row("uid-2");

    let view = ResourceTableView::new(state, Theme::dark());
    // Verify selected_uid is set, which render_row uses for highlighting
    assert_eq!(view.state.selected_uid.as_deref(), Some("uid-2"));
}

#[test]
fn test_view_empty_table_shows_no_resources() {
    let state = ResourceTableState::new(sample_columns(), 10);
    let view = ResourceTableView::new(state, Theme::dark());
    assert!(view.state.visible_slice().is_empty());
}

#[test]
fn test_view_column_headers_present() {
    let state = ResourceTableState::new(sample_columns(), 10);
    let view = ResourceTableView::new(state, Theme::dark());
    assert_eq!(view.state.columns.len(), 3);
    assert_eq!(view.state.columns[0].label, "Name");
    assert_eq!(view.state.columns[1].label, "Namespace");
    assert_eq!(view.state.columns[2].label, "Status");
}

#[test]
fn test_view_visible_rows_count() {
    let mut state = ResourceTableState::new(sample_columns(), 2);
    state.set_rows(sample_rows());
    let view = ResourceTableView::new(state, Theme::dark());
    assert_eq!(view.state.visible_slice().len(), 2);
}

#[test]
fn test_view_filter_affects_visible() {
    let mut state = ResourceTableState::new(sample_columns(), 10);
    state.set_rows(sample_rows());
    state.filter_text = "nginx".to_string();
    let view = ResourceTableView::new(state, Theme::dark());
    assert_eq!(view.state.visible_slice().len(), 1);
}
