// Tests for T337-T342: Resource table enhancements
//
// T337: Wire per-resource columns into ResourceListView
// T338: Wire "More Actions" context menu into ResourceTable
// T339: Wire multi-namespace filtering (FR-034, FR-073)
// T340: Implement CSV export (FR-072)
// T341: Implement table keyboard navigation (FR-074)
// T342: Implement column resize and visibility toggle (FR-006)

use baeus_ui::components::resource_table::*;
use baeus_ui::theme::Theme;
use baeus_ui::views::resource_list::{ResourceListState, ResourceListView};

// =========================================================================
// Helper: create sample rows
// =========================================================================

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
            cells: vec!["cluster-node".to_string(), "".to_string(), "Ready".to_string()],
            selected: false,
            kind: "Node".to_string(),
            name: "cluster-node".to_string(),
            namespace: None,
            container_statuses: Vec::new(),
            conditions: Vec::new(),
        },
    ]
}

fn three_col_defs() -> Vec<ColumnDef> {
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

// =========================================================================
// T337: Wire per-resource columns into ResourceListView
// =========================================================================

#[test]
fn t337_resource_list_view_uses_resource_table_columns_for_pod() {
    let state = ResourceListState::new("Pod", "v1");
    let view = ResourceListView::new(state, Theme::dark());
    let expected = columns_for_kind("Pod");
    assert_eq!(view.table_state.columns.len(), expected.len());
    for (a, b) in view.table_state.columns.iter().zip(expected.iter()) {
        assert_eq!(a.id, b.id);
        assert_eq!(a.label, b.label);
    }
}

#[test]
fn t337_resource_list_view_uses_resource_table_columns_for_deployment() {
    let state = ResourceListState::new("Deployment", "apps/v1");
    let view = ResourceListView::new(state, Theme::dark());
    let expected = columns_for_kind("Deployment");
    assert_eq!(view.table_state.columns.len(), expected.len());
    for (a, b) in view.table_state.columns.iter().zip(expected.iter()) {
        assert_eq!(a.id, b.id);
    }
}

#[test]
fn t337_resource_list_view_uses_resource_table_columns_for_node() {
    let state = ResourceListState::new("Node", "v1");
    let view = ResourceListView::new(state, Theme::dark());
    let expected = columns_for_kind("Node");
    assert_eq!(view.table_state.columns.len(), expected.len());
}

#[test]
fn t337_resource_list_view_uses_resource_table_columns_for_unknown() {
    let state = ResourceListState::new("CustomThing", "v1");
    let view = ResourceListView::new(state, Theme::dark());
    let expected = columns_for_kind("CustomThing");
    assert_eq!(view.table_state.columns.len(), expected.len());
}

// =========================================================================
// T338: Context menu state
// =========================================================================

#[test]
fn t338_context_menu_initially_closed() {
    let state = ResourceTableState::new(three_col_defs(), 10);
    assert!(state.context_menu_row.is_none());
}

#[test]
fn t338_open_context_menu() {
    let mut state = ResourceTableState::new(three_col_defs(), 10);
    state.open_context_menu(2);
    assert_eq!(state.context_menu_row, Some(2));
}

#[test]
fn t338_close_context_menu() {
    let mut state = ResourceTableState::new(three_col_defs(), 10);
    state.open_context_menu(1);
    state.close_context_menu();
    assert!(state.context_menu_row.is_none());
}

#[test]
fn t338_open_context_menu_overrides_previous() {
    let mut state = ResourceTableState::new(three_col_defs(), 10);
    state.open_context_menu(1);
    state.open_context_menu(5);
    assert_eq!(state.context_menu_row, Some(5));
}

#[test]
fn t338_actions_for_kind_pod_has_shell() {
    let actions = actions_for_kind("Pod");
    assert!(actions.iter().any(|a| a.label == "Shell"));
}

#[test]
fn t338_actions_for_kind_deployment_has_scale() {
    let actions = actions_for_kind("Deployment");
    assert!(actions.iter().any(|a| a.label == "Scale"));
}

#[test]
fn t338_actions_for_kind_unknown_has_edit_delete() {
    let actions = actions_for_kind("CustomResource");
    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0].label, "Edit");
    assert_eq!(actions[1].label, "Delete");
}

#[test]
fn t338_resource_table_view_with_kind() {
    let state = ResourceTableState::new(three_col_defs(), 10);
    let view = ResourceTableView::with_kind(state, Theme::dark(), "Pod");
    assert_eq!(view.resource_kind, "Pod");
}

// =========================================================================
// T339: Multi-namespace filtering
// =========================================================================

#[test]
fn t339_selected_namespaces_initially_empty() {
    let state = ResourceListState::new("Pod", "v1");
    assert!(state.selected_namespaces.is_empty());
}

#[test]
fn t339_set_selected_namespaces() {
    let mut state = ResourceListState::new("Pod", "v1");
    state.set_selected_namespaces(vec!["default".to_string(), "kube-system".to_string()]);
    assert_eq!(state.selected_namespaces.len(), 2);
}

#[test]
fn t339_add_namespace() {
    let mut state = ResourceListState::new("Pod", "v1");
    state.add_namespace("default");
    state.add_namespace("kube-system");
    assert_eq!(state.selected_namespaces.len(), 2);
}

#[test]
fn t339_add_namespace_no_duplicates() {
    let mut state = ResourceListState::new("Pod", "v1");
    state.add_namespace("default");
    state.add_namespace("default");
    assert_eq!(state.selected_namespaces.len(), 1);
}

#[test]
fn t339_remove_namespace() {
    let mut state = ResourceListState::new("Pod", "v1");
    state.add_namespace("default");
    state.add_namespace("cache");
    state.remove_namespace("default");
    assert_eq!(state.selected_namespaces, vec!["cache"]);
}

#[test]
fn t339_clear_namespace_filter() {
    let mut state = ResourceListState::new("Pod", "v1");
    state.add_namespace("default");
    state.clear_namespace_filter();
    assert!(state.selected_namespaces.is_empty());
}

#[test]
fn t339_is_cluster_scoped_node() {
    let state = ResourceListState::new("Node", "v1");
    assert!(state.is_cluster_scoped());
}

#[test]
fn t339_is_cluster_scoped_namespace() {
    let state = ResourceListState::new("Namespace", "v1");
    assert!(state.is_cluster_scoped());
}

#[test]
fn t339_is_cluster_scoped_persistent_volume() {
    let state = ResourceListState::new("PersistentVolume", "v1");
    assert!(state.is_cluster_scoped());
}

#[test]
fn t339_is_cluster_scoped_storage_class() {
    let state = ResourceListState::new("StorageClass", "storage.k8s.io/v1");
    assert!(state.is_cluster_scoped());
}

#[test]
fn t339_is_cluster_scoped_cluster_role() {
    let state = ResourceListState::new("ClusterRole", "rbac.authorization.k8s.io/v1");
    assert!(state.is_cluster_scoped());
}

#[test]
fn t339_is_not_cluster_scoped_pod() {
    let state = ResourceListState::new("Pod", "v1");
    assert!(!state.is_cluster_scoped());
}

#[test]
fn t339_is_not_cluster_scoped_deployment() {
    let state = ResourceListState::new("Deployment", "apps/v1");
    assert!(!state.is_cluster_scoped());
}

#[test]
fn t339_has_namespace_filter_when_namespaces_selected() {
    let mut state = ResourceListState::new("Pod", "v1");
    state.add_namespace("default");
    assert!(state.has_namespace_filter());
}

#[test]
fn t339_no_namespace_filter_when_empty() {
    let state = ResourceListState::new("Pod", "v1");
    assert!(!state.has_namespace_filter());
}

#[test]
fn t339_no_namespace_filter_for_cluster_scoped() {
    let mut state = ResourceListState::new("Node", "v1");
    state.add_namespace("default");
    // Cluster-scoped resources ignore namespace filter.
    assert!(!state.has_namespace_filter());
}

#[test]
fn t339_filter_by_namespaces_returns_all_when_empty() {
    let state = ResourceListState::new("Pod", "v1");
    let rows = sample_rows();
    let filtered = state.filter_by_namespaces(&rows);
    assert_eq!(filtered.len(), 4);
}

#[test]
fn t339_filter_by_namespaces_selects_matching() {
    let mut state = ResourceListState::new("Pod", "v1");
    state.add_namespace("default");
    let rows = sample_rows();
    let filtered = state.filter_by_namespaces(&rows);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].name, "nginx-pod");
}

#[test]
fn t339_filter_by_namespaces_multi_namespace() {
    let mut state = ResourceListState::new("Pod", "v1");
    state.add_namespace("default");
    state.add_namespace("cache");
    let rows = sample_rows();
    let filtered = state.filter_by_namespaces(&rows);
    assert_eq!(filtered.len(), 2);
}

#[test]
fn t339_filter_by_namespaces_cluster_scoped_returns_all() {
    let mut state = ResourceListState::new("Node", "v1");
    state.add_namespace("default");
    let rows = sample_rows();
    // Cluster-scoped => namespace filter is ignored.
    let filtered = state.filter_by_namespaces(&rows);
    assert_eq!(filtered.len(), 4);
}

#[test]
fn t339_filter_by_namespaces_excludes_none_namespace() {
    let mut state = ResourceListState::new("Pod", "v1");
    state.add_namespace("nonexistent");
    let rows = sample_rows();
    let filtered = state.filter_by_namespaces(&rows);
    assert!(filtered.is_empty());
}

// =========================================================================
// T340: CSV export
// =========================================================================

#[test]
fn t340_csv_export_header_row() {
    let state = ResourceTableState::new(three_col_defs(), 10);
    let csv = state.to_csv();
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines[0], "Name,Namespace,Status");
}

#[test]
fn t340_csv_export_includes_data_rows() {
    let mut state = ResourceTableState::new(three_col_defs(), 10);
    state.set_rows(sample_rows());
    let csv = state.to_csv();
    let lines: Vec<&str> = csv.lines().collect();
    // 1 header + 4 data rows
    assert_eq!(lines.len(), 5);
}

#[test]
fn t340_csv_export_data_content() {
    let mut state = ResourceTableState::new(three_col_defs(), 10);
    state.set_rows(sample_rows());
    let csv = state.to_csv();
    let lines: Vec<&str> = csv.lines().collect();
    // First data row: nginx-pod, default, Running
    assert_eq!(lines[1], "nginx-pod,default,Running");
}

#[test]
fn t340_csv_export_escapes_commas() {
    let mut state = ResourceTableState::new(three_col_defs(), 10);
    state.set_rows(vec![TableRow {
        uid: "uid-esc".to_string(),
        cells: vec!["name,with,commas".to_string(), "ns".to_string(), "Running".to_string()],
        selected: false,
        kind: "Pod".to_string(),
        name: "name,with,commas".to_string(),
        namespace: Some("ns".to_string()),
        container_statuses: Vec::new(),
        conditions: Vec::new(),
    }]);
    let csv = state.to_csv();
    let lines: Vec<&str> = csv.lines().collect();
    assert!(lines[1].starts_with("\"name,with,commas\""));
}

#[test]
fn t340_csv_export_escapes_double_quotes() {
    let mut state = ResourceTableState::new(three_col_defs(), 10);
    state.set_rows(vec![TableRow {
        uid: "uid-q".to_string(),
        cells: vec!["say \"hello\"".to_string(), "ns".to_string(), "ok".to_string()],
        selected: false,
        kind: "Pod".to_string(),
        name: "say \"hello\"".to_string(),
        namespace: Some("ns".to_string()),
        container_statuses: Vec::new(),
        conditions: Vec::new(),
    }]);
    let csv = state.to_csv();
    let lines: Vec<&str> = csv.lines().collect();
    // RFC 4180: internal quotes doubled and field wrapped in quotes.
    assert_eq!(lines[1], "\"say \"\"hello\"\"\",ns,ok");
}

#[test]
fn t340_csv_export_respects_filter() {
    let mut state = ResourceTableState::new(three_col_defs(), 10);
    state.set_rows(sample_rows());
    state.filter_text = "nginx".to_string();
    let csv = state.to_csv();
    let lines: Vec<&str> = csv.lines().collect();
    // 1 header + 1 matching data row
    assert_eq!(lines.len(), 2);
    assert!(lines[1].contains("nginx-pod"));
}

#[test]
fn t340_csv_export_respects_column_visibility() {
    let mut state = ResourceTableState::new(three_col_defs(), 10);
    state.set_rows(sample_rows());
    // Hide the "namespace" column (index 1)
    state.toggle_column_visibility(1);
    let csv = state.to_csv();
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines[0], "Name,Status");
    // First data row: only name and status
    assert_eq!(lines[1], "nginx-pod,Running");
}

#[test]
fn t340_csv_export_empty_table() {
    let state = ResourceTableState::new(three_col_defs(), 10);
    let csv = state.to_csv();
    let lines: Vec<&str> = csv.lines().collect();
    // Just the header row
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0], "Name,Namespace,Status");
}

// =========================================================================
// T341: Table keyboard navigation
// =========================================================================

#[test]
fn t341_selected_row_index_initially_none() {
    let state = ResourceTableState::new(three_col_defs(), 10);
    assert!(state.selected_row_index.is_none());
}

#[test]
fn t341_select_next_row_from_none() {
    let mut state = ResourceTableState::new(three_col_defs(), 10);
    state.set_rows(sample_rows());
    state.select_next_row();
    assert_eq!(state.selected_row_index, Some(0));
}

#[test]
fn t341_select_next_row_increments() {
    let mut state = ResourceTableState::new(three_col_defs(), 10);
    state.set_rows(sample_rows());
    state.select_next_row(); // 0
    state.select_next_row(); // 1
    state.select_next_row(); // 2
    assert_eq!(state.selected_row_index, Some(2));
}

#[test]
fn t341_select_next_row_clamps_at_end() {
    let mut state = ResourceTableState::new(three_col_defs(), 10);
    state.set_rows(sample_rows());
    // Select last row
    state.selected_row_index = Some(3);
    state.select_next_row();
    assert_eq!(state.selected_row_index, Some(3)); // stays at 3 (4 rows total)
}

#[test]
fn t341_select_next_row_empty_table() {
    let mut state = ResourceTableState::new(three_col_defs(), 10);
    state.select_next_row();
    assert!(state.selected_row_index.is_none());
}

#[test]
fn t341_select_previous_row_from_none() {
    let mut state = ResourceTableState::new(three_col_defs(), 10);
    state.set_rows(sample_rows());
    state.select_previous_row();
    // From None, selects last row
    assert_eq!(state.selected_row_index, Some(3));
}

#[test]
fn t341_select_previous_row_decrements() {
    let mut state = ResourceTableState::new(three_col_defs(), 10);
    state.set_rows(sample_rows());
    state.selected_row_index = Some(2);
    state.select_previous_row();
    assert_eq!(state.selected_row_index, Some(1));
}

#[test]
fn t341_select_previous_row_clamps_at_zero() {
    let mut state = ResourceTableState::new(three_col_defs(), 10);
    state.set_rows(sample_rows());
    state.selected_row_index = Some(0);
    state.select_previous_row();
    assert_eq!(state.selected_row_index, Some(0)); // clamps
}

#[test]
fn t341_select_previous_row_empty_table() {
    let mut state = ResourceTableState::new(three_col_defs(), 10);
    state.select_previous_row();
    assert!(state.selected_row_index.is_none());
}

#[test]
fn t341_selected_row_by_index_returns_correct_row() {
    let mut state = ResourceTableState::new(three_col_defs(), 10);
    state.set_rows(sample_rows());
    state.selected_row_index = Some(1);
    let row = state.selected_row_by_index().unwrap();
    assert_eq!(row.name, "redis-pod");
}

#[test]
fn t341_selected_row_by_index_none_when_no_selection() {
    let mut state = ResourceTableState::new(three_col_defs(), 10);
    state.set_rows(sample_rows());
    assert!(state.selected_row_by_index().is_none());
}

#[test]
fn t341_selected_row_by_index_respects_filter() {
    let mut state = ResourceTableState::new(three_col_defs(), 10);
    state.set_rows(sample_rows());
    state.filter_text = "pod".to_string(); // matches nginx-pod, redis-pod
    state.selected_row_index = Some(1);
    let row = state.selected_row_by_index().unwrap();
    assert_eq!(row.name, "redis-pod");
}

#[test]
fn t341_keyboard_nav_round_trip() {
    let mut state = ResourceTableState::new(three_col_defs(), 10);
    state.set_rows(sample_rows());
    state.select_next_row(); // 0
    state.select_next_row(); // 1
    state.select_previous_row(); // 0
    assert_eq!(state.selected_row_index, Some(0));
    let row = state.selected_row_by_index().unwrap();
    assert_eq!(row.name, "nginx-pod");
}

// =========================================================================
// T342: Column resize and visibility toggle
// =========================================================================

#[test]
fn t342_all_columns_visible_initially() {
    let state = ResourceTableState::new(three_col_defs(), 10);
    assert_eq!(state.visible_columns, vec![true, true, true]);
}

#[test]
fn t342_column_widths_default_from_weight() {
    let state = ResourceTableState::new(three_col_defs(), 10);
    // Name -> 200 (weight*100), Namespace -> 120 (named override), Status -> 80 (named override)
    assert_eq!(state.column_widths, vec![200.0, 120.0, 80.0]);
}

#[test]
fn t342_toggle_column_visibility() {
    let mut state = ResourceTableState::new(three_col_defs(), 10);
    state.toggle_column_visibility(1);
    assert_eq!(state.visible_columns, vec![true, false, true]);
}

#[test]
fn t342_toggle_column_visibility_twice() {
    let mut state = ResourceTableState::new(three_col_defs(), 10);
    state.toggle_column_visibility(1);
    state.toggle_column_visibility(1);
    assert_eq!(state.visible_columns, vec![true, true, true]);
}

#[test]
fn t342_toggle_column_visibility_out_of_range() {
    let mut state = ResourceTableState::new(three_col_defs(), 10);
    state.toggle_column_visibility(99); // no-op
    assert_eq!(state.visible_columns, vec![true, true, true]);
}

#[test]
fn t342_set_column_width() {
    let mut state = ResourceTableState::new(three_col_defs(), 10);
    state.set_column_width(0, 250.0);
    assert_eq!(state.column_widths[0], 250.0);
}

#[test]
fn t342_set_column_width_out_of_range() {
    let mut state = ResourceTableState::new(three_col_defs(), 10);
    state.set_column_width(99, 999.0); // no-op
    assert_eq!(state.column_widths, vec![200.0, 120.0, 80.0]);
}

#[test]
fn t342_visible_column_defs_all_visible() {
    let state = ResourceTableState::new(three_col_defs(), 10);
    let vis = state.visible_column_defs();
    assert_eq!(vis.len(), 3);
}

#[test]
fn t342_visible_column_defs_one_hidden() {
    let mut state = ResourceTableState::new(three_col_defs(), 10);
    state.toggle_column_visibility(1);
    let vis = state.visible_column_defs();
    assert_eq!(vis.len(), 2);
    assert_eq!(vis[0].id, "name");
    assert_eq!(vis[1].id, "status");
}

#[test]
fn t342_visible_column_defs_all_hidden() {
    let mut state = ResourceTableState::new(three_col_defs(), 10);
    state.toggle_column_visibility(0);
    state.toggle_column_visibility(1);
    state.toggle_column_visibility(2);
    let vis = state.visible_column_defs();
    assert!(vis.is_empty());
}

#[test]
fn t342_csv_export_with_hidden_columns() {
    let mut state = ResourceTableState::new(three_col_defs(), 10);
    state.set_rows(sample_rows());
    // Hide namespace column
    state.toggle_column_visibility(1);
    let csv = state.to_csv();
    let first_line = csv.lines().next().unwrap();
    assert_eq!(first_line, "Name,Status");
}

// =========================================================================
// Integration: Keyboard nav + filter + CSV
// =========================================================================

#[test]
fn t341_t340_keyboard_nav_then_csv() {
    let mut state = ResourceTableState::new(three_col_defs(), 10);
    state.set_rows(sample_rows());
    state.filter_text = "pod".to_string();
    // Navigate to second filtered row
    state.select_next_row(); // 0
    state.select_next_row(); // 1
    let row = state.selected_row_by_index().unwrap();
    assert_eq!(row.name, "redis-pod");

    // CSV should also reflect the filter
    let csv = state.to_csv();
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines.len(), 3); // header + 2 filtered rows
}

#[test]
fn t342_t340_hidden_columns_csv() {
    let mut state = ResourceTableState::new(three_col_defs(), 10);
    state.set_rows(sample_rows());
    state.toggle_column_visibility(0); // hide Name
    state.toggle_column_visibility(2); // hide Status
    let csv = state.to_csv();
    let first_line = csv.lines().next().unwrap();
    assert_eq!(first_line, "Namespace");
    let second_line = csv.lines().nth(1).unwrap();
    assert_eq!(second_line, "default");
}
