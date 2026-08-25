use gpui::{Context, ElementId, Rgba, SharedString, Window, div, prelude::*};
use serde::{Deserialize, Serialize};

use crate::components::details_panel::ResourceInfo;
use crate::theme::Theme;

/// Sort direction for table columns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortDirection {
    Ascending,
    Descending,
}

/// Definition of a single column in the resource table.
#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub id: String,
    pub label: String,
    pub sortable: bool,
    /// Relative width weight for layout calculation.
    pub width_weight: f32,
}

/// Active sort state for the table.
#[derive(Debug, Clone)]
pub struct TableSort {
    pub column_id: String,
    pub direction: SortDirection,
}

use crate::components::json_extract::ContainerBrickStatus;

/// Represents a single row in the resource table.
#[derive(Debug, Clone)]
pub struct TableRow {
    pub uid: String,
    /// Cell values in column order.
    pub cells: Vec<String>,
    pub selected: bool,
    pub kind: String,
    pub name: String,
    pub namespace: Option<String>,
    /// Per-container brick statuses for Pod rows (empty for non-Pods).
    pub container_statuses: Vec<ContainerBrickStatus>,
    /// Conditions as (type, is_true) pairs (e.g. Deployments, Nodes).
    pub conditions: Vec<(String, bool)>,
}

/// State for a virtual-scrolling resource table.
///
/// Manages columns, rows, sorting, selection, filtering, and viewport
/// scrolling for efficient rendering of potentially large resource lists.
#[derive(Debug)]
pub struct ResourceTableState {
    pub columns: Vec<ColumnDef>,
    pub rows: Vec<TableRow>,
    pub sort: Option<TableSort>,
    pub selected_uid: Option<String>,
    pub scroll_offset: usize,
    /// Number of rows that fit in the visible viewport.
    pub visible_rows: usize,
    pub filter_text: String,
    /// T341: Index-based row selection for keyboard navigation.
    pub selected_row_index: Option<usize>,
    /// T342: Per-column visibility flags (parallel to `columns`).
    pub visible_columns: Vec<bool>,
    /// T342: Per-column widths in pixels (parallel to `columns`).
    pub column_widths: Vec<f32>,
    /// T338: Whether the context menu is currently shown, and for which row index.
    pub context_menu_row: Option<usize>,
    /// T346: The resource info for the currently selected row, populated when
    /// a row is clicked to open the details panel.
    pub selected_resource: Option<ResourceInfo>,
}

impl ResourceTableState {
    /// Creates a new empty table state with the given column definitions
    /// and viewport size.
    pub fn new(columns: Vec<ColumnDef>, visible_rows: usize) -> Self {
        let col_count = columns.len();
        let default_widths: Vec<f32> = columns
            .iter()
            .map(|c| {
                // Produce sensible pixel defaults based on column label
                match c.label.as_str() {
                    "Name" => 200.0,
                    "Namespace" | "Controlled By" | "Node" => 120.0,
                    "CPU" | "Memory" | "Restarts" | "QoS" | "Age" | "Status" => 80.0,
                    "Containers" | "Ports" | "Type" => 90.0,
                    "Message" => 200.0,
                    "Conditions" => 150.0,
                    _ => (c.width_weight * 100.0).max(60.0),
                }
            })
            .collect();
        Self {
            columns,
            rows: Vec::new(),
            sort: None,
            selected_uid: None,
            scroll_offset: 0,
            visible_rows,
            filter_text: String::new(),
            selected_row_index: None,
            visible_columns: vec![true; col_count],
            column_widths: default_widths,
            context_menu_row: None,
            selected_resource: None,
        }
    }

    /// Replaces all rows in the table, resetting scroll offset.
    pub fn set_rows(&mut self, rows: Vec<TableRow>) {
        self.rows = rows;
        self.scroll_offset = 0;
    }

    /// Sorts the table by the given column. If the table is already sorted by
    /// this column, the direction is toggled. Otherwise, sorts ascending.
    /// Does nothing if the column is not found or is not sortable.
    pub fn sort_by(&mut self, column_id: &str) {
        let is_sortable = self.columns.iter().any(|c| c.id == column_id && c.sortable);
        if !is_sortable {
            return;
        }

        let new_direction = match &self.sort {
            Some(current) if current.column_id == column_id => match current.direction {
                SortDirection::Ascending => SortDirection::Descending,
                SortDirection::Descending => SortDirection::Ascending,
            },
            _ => SortDirection::Ascending,
        };

        // Find column index for cell-based sorting
        let col_index = self.columns.iter().position(|c| c.id == column_id);

        if let Some(idx) = col_index {
            let ascending = new_direction == SortDirection::Ascending;
            self.rows.sort_by(|a, b| {
                let val_a = a.cells.get(idx).map(String::as_str).unwrap_or("");
                let val_b = b.cells.get(idx).map(String::as_str).unwrap_or("");
                if ascending { val_a.cmp(val_b) } else { val_b.cmp(val_a) }
            });
        }

        self.sort = Some(TableSort { column_id: column_id.to_string(), direction: new_direction });
    }

    /// Returns rows that match the current filter text.
    ///
    /// Matching is case-insensitive against row name, namespace, and all cell values.
    /// Returns all rows when filter_text is empty.
    pub fn filtered_rows(&self) -> Vec<&TableRow> {
        if self.filter_text.is_empty() {
            return self.rows.iter().collect();
        }
        let query = self.filter_text.to_lowercase();
        self.rows
            .iter()
            .filter(|row| {
                if row.name.to_lowercase().contains(&query) {
                    return true;
                }
                if let Some(ns) = &row.namespace {
                    if ns.to_lowercase().contains(&query) {
                        return true;
                    }
                }
                row.cells.iter().any(|cell| cell.to_lowercase().contains(&query))
            })
            .collect()
    }

    /// Returns the slice of filtered rows visible in the current viewport.
    pub fn visible_slice(&self) -> Vec<&TableRow> {
        let filtered = self.filtered_rows();
        let start = self.scroll_offset.min(filtered.len());
        let end = (self.scroll_offset + self.visible_rows).min(filtered.len());
        filtered[start..end].to_vec()
    }

    /// Sets the scroll offset, clamping to the valid range for the current
    /// filtered row count.
    pub fn scroll_to(&mut self, offset: usize) {
        let filtered_count = self.filtered_rows().len();
        if filtered_count == 0 {
            self.scroll_offset = 0;
        } else {
            self.scroll_offset = offset.min(filtered_count.saturating_sub(1));
        }
    }

    /// Selects the row with the given UID.
    pub fn select_row(&mut self, uid: &str) {
        self.selected_uid = Some(uid.to_string());
    }

    /// Returns the currently selected row, if any.
    pub fn selected_row(&self) -> Option<&TableRow> {
        self.selected_uid.as_ref().and_then(|uid| self.rows.iter().find(|r| r.uid == *uid))
    }

    /// Returns the total number of rows matching the current filter.
    pub fn total_filtered_count(&self) -> usize {
        self.filtered_rows().len()
    }

    // -------------------------------------------------------------------
    // T340: CSV export (FR-072)
    // -------------------------------------------------------------------

    /// Serializes the current visible (filtered) rows to CSV format.
    ///
    /// The first line is a header row built from the column labels.
    /// Subsequent lines contain cell values, quoted when they contain
    /// commas, double-quotes, or newlines.  Internal double-quotes are
    /// escaped by doubling them (RFC 4180).
    pub fn to_csv(&self) -> String {
        let mut out = String::new();

        // Determine which columns are visible (T342 integration).
        let visible_indices: Vec<usize> = self
            .visible_columns
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| if v { Some(i) } else { None })
            .collect();

        // Header row
        let headers: Vec<String> = visible_indices
            .iter()
            .filter_map(|&i| self.columns.get(i).map(|c| csv_escape(&c.label)))
            .collect();
        out.push_str(&headers.join(","));
        out.push('\n');

        // Data rows (use filtered_rows so CSV respects the current filter)
        for row in self.filtered_rows() {
            let cells: Vec<String> = visible_indices
                .iter()
                .map(|&i| row.cells.get(i).map(|c| csv_escape(c)).unwrap_or_default())
                .collect();
            out.push_str(&cells.join(","));
            out.push('\n');
        }

        out
    }

    // -------------------------------------------------------------------
    // T341: Keyboard navigation (FR-074)
    // -------------------------------------------------------------------

    /// Moves the selected row index to the next row in the filtered list.
    ///
    /// If no row is selected, selects the first row.  Clamps at the last
    /// row (does not wrap).
    pub fn select_next_row(&mut self) {
        let count = self.filtered_rows().len();
        if count == 0 {
            self.selected_row_index = None;
            return;
        }
        self.selected_row_index = Some(match self.selected_row_index {
            Some(idx) => (idx + 1).min(count - 1),
            None => 0,
        });
    }

    /// Moves the selected row index to the previous row in the filtered list.
    ///
    /// If no row is selected, selects the last row.  Clamps at the first
    /// row (does not wrap).
    pub fn select_previous_row(&mut self) {
        let count = self.filtered_rows().len();
        if count == 0 {
            self.selected_row_index = None;
            return;
        }
        self.selected_row_index = Some(match self.selected_row_index {
            Some(idx) => idx.saturating_sub(1),
            None => count - 1,
        });
    }

    /// Returns a reference to the row at the current `selected_row_index`
    /// within the filtered row set, if any.
    pub fn selected_row_by_index(&self) -> Option<&TableRow> {
        let idx = self.selected_row_index?;
        self.filtered_rows().into_iter().nth(idx)
    }

    // -------------------------------------------------------------------
    // T342: Column resize & visibility toggle (FR-006)
    // -------------------------------------------------------------------

    /// Toggles the visibility of the column at `idx`.
    ///
    /// Does nothing if `idx` is out of range.
    pub fn toggle_column_visibility(&mut self, idx: usize) {
        if let Some(v) = self.visible_columns.get_mut(idx) {
            *v = !*v;
        }
    }

    /// Sets the pixel width of the column at `idx`.
    ///
    /// Does nothing if `idx` is out of range.
    pub fn set_column_width(&mut self, idx: usize, width: f32) {
        if let Some(w) = self.column_widths.get_mut(idx) {
            *w = width;
        }
    }

    /// Returns the column definitions for currently-visible columns only.
    pub fn visible_column_defs(&self) -> Vec<&ColumnDef> {
        self.columns
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                if self.visible_columns.get(i).copied().unwrap_or(true) { Some(c) } else { None }
            })
            .collect()
    }

    // -------------------------------------------------------------------
    // T338: Context menu helpers
    // -------------------------------------------------------------------

    /// Opens the context menu for the row at `idx`.
    pub fn open_context_menu(&mut self, idx: usize) {
        self.context_menu_row = Some(idx);
    }

    /// Closes the context menu.
    pub fn close_context_menu(&mut self) {
        self.context_menu_row = None;
    }

    // -------------------------------------------------------------------
    // T346: Row click -> details panel wiring
    // -------------------------------------------------------------------

    /// Sets the selected resource info, typically called when a table row
    /// is clicked. Also updates `selected_uid` to keep both in sync.
    pub fn select_resource(&mut self, info: ResourceInfo) {
        self.selected_uid = Some(info.uid.clone());
        self.selected_resource = Some(info);
    }

    /// Clears the selected resource info and the UID-based selection.
    pub fn clear_selection(&mut self) {
        self.selected_resource = None;
        self.selected_uid = None;
    }

    /// Returns a reference to the currently selected resource info, if any.
    pub fn selected_resource(&self) -> Option<&ResourceInfo> {
        self.selected_resource.as_ref()
    }

    /// Builds a `ResourceInfo` from the currently selected row (by UID).
    /// This is a convenience method that creates a minimal `ResourceInfo`
    /// from the table row data. Callers should enrich it with additional
    /// metadata (labels, annotations, conditions) from the K8s API.
    pub fn resource_info_from_selected_row(&self) -> Option<ResourceInfo> {
        let row = self.selected_row()?;
        Some(
            ResourceInfo::new(&row.name, &row.kind, &row.uid)
                .with_namespace(row.namespace.clone().unwrap_or_default()),
        )
    }
}

/// RFC 4180-compliant CSV field escaping.
fn csv_escape(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') {
        let escaped = field.replace('"', "\"\"");
        format!("\"{escaped}\"")
    } else {
        field.to_string()
    }
}

// ---------------------------------------------------------------------------
// GPUI Render
// ---------------------------------------------------------------------------

/// View wrapper for `ResourceTableState` that holds a theme for rendering.
pub struct ResourceTableView {
    pub state: ResourceTableState,
    pub theme: Theme,
    /// The resource kind displayed, used for the T338 context menu actions.
    pub resource_kind: String,
}

impl ResourceTableView {
    pub fn new(state: ResourceTableState, theme: Theme) -> Self {
        Self { state, theme, resource_kind: String::new() }
    }

    /// Creates a new view with an associated resource kind for context
    /// menu actions.
    pub fn with_kind(state: ResourceTableState, theme: Theme, kind: &str) -> Self {
        Self { state, theme, resource_kind: kind.to_string() }
    }
}

impl ResourceTableView {
    /// Returns the visible column indices (true entries in `visible_columns`).
    fn visible_column_indices(&self) -> Vec<usize> {
        self.state
            .visible_columns
            .iter()
            .enumerate()
            .filter_map(|(i, &v)| if v { Some(i) } else { None })
            .collect()
    }

    /// Render the column header row.
    ///
    /// Only renders columns that are currently visible (T342).
    fn render_header_row(&self, colors: &HeaderColors) -> gpui::Div {
        let mut header = div()
            .flex()
            .flex_row()
            .w_full()
            .border_b_1()
            .border_color(colors.border)
            .bg(colors.surface);

        for idx in self.visible_column_indices() {
            if let Some(col) = self.state.columns.get(idx) {
                header = header.child(self.render_header_cell(col, colors));
            }
        }

        // T338: Reserve space for the actions column header.
        header = header.child(
            div()
                .w(gpui::px(40.0))
                .px_2()
                .py_1()
                .text_xs()
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(colors.text_secondary),
        );

        header
    }

    /// Render a single column header cell.
    fn render_header_cell(&self, col: &ColumnDef, colors: &HeaderColors) -> gpui::Div {
        let label = SharedString::from(col.label.clone());
        let sort_indicator = self.sort_indicator_for(&col.id);
        let full_label = SharedString::from(format!("{}{}", label, sort_indicator));

        let mut cell = div()
            .flex_1()
            .px_2()
            .py_1()
            .text_xs()
            .font_weight(gpui::FontWeight::BOLD)
            .text_color(colors.text_secondary);

        if col.sortable {
            cell = cell.cursor_pointer();
        }

        cell.child(full_label)
    }

    /// Compute sort indicator text for a column.
    pub fn sort_indicator_for(&self, column_id: &str) -> &'static str {
        match &self.state.sort {
            Some(sort) if sort.column_id == column_id => match sort.direction {
                SortDirection::Ascending => " ^",
                SortDirection::Descending => " v",
            },
            _ => "",
        }
    }

    /// Render the body rows.
    fn render_body(&self, colors: &BodyColors) -> gpui::Div {
        let visible = self.state.visible_slice();
        let mut body = div().flex().flex_col().w_full();

        for (idx, row) in visible.iter().enumerate() {
            body = body.child(self.render_row(row, idx, colors));
        }

        if visible.is_empty() {
            body = body.child(self.render_empty_state(colors));
        }

        body
    }

    /// Render a single data row.
    ///
    /// Highlights both UID-based and index-based (T341) selection.
    /// Includes a three-dot context menu button (T338) and conditionally
    /// renders the floating actions menu.
    fn render_row(
        &self,
        row: &TableRow,
        idx: usize,
        colors: &BodyColors,
    ) -> gpui::Stateful<gpui::Div> {
        let is_uid_selected = self.state.selected_uid.as_deref() == Some(&row.uid);
        let is_idx_selected = self.state.selected_row_index == Some(idx);

        let bg_color =
            if is_uid_selected || is_idx_selected { colors.selection } else { colors.background };

        let row_id = ElementId::Name(SharedString::from(format!("row-{idx}")));

        let mut row_div = div()
            .id(row_id)
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .bg(bg_color)
            .cursor_pointer();

        // Add border bottom
        row_div = row_div.border_b_1().border_color(colors.border);

        // Only render visible columns (T342).
        let vis_indices = self.visible_column_indices();
        for col_idx in &vis_indices {
            let cell_val = row.cells.get(*col_idx).map(String::as_str).unwrap_or("");
            row_div = row_div.child(self.render_cell(cell_val, colors));
        }

        // T338: Three-dot "More Actions" button.
        let dots_id = ElementId::Name(SharedString::from(format!("row-dots-{idx}")));
        let dots_btn = div()
            .id(dots_id)
            .w(gpui::px(40.0))
            .flex()
            .justify_center()
            .items_center()
            .cursor_pointer()
            .text_sm()
            .text_color(colors.text_primary)
            .child(SharedString::from("..."));

        row_div = row_div.child(dots_btn);

        // T338: If the context menu is open for this row, render it.
        if self.state.context_menu_row == Some(idx) {
            row_div = row_div.child(self.render_context_menu(colors));
        }

        row_div
    }

    /// T338: Render the floating context menu with actions for the current
    /// resource kind.
    fn render_context_menu(&self, colors: &BodyColors) -> gpui::Div {
        let kind = if self.resource_kind.is_empty() { "Unknown" } else { &self.resource_kind };
        let actions = actions_for_kind(kind);

        let mut menu = div()
            .flex()
            .flex_col()
            .bg(colors.background)
            .border_1()
            .border_color(colors.border)
            .rounded(gpui::px(4.0))
            .py_1();

        for (ai, action) in actions.iter().enumerate() {
            let action_id = ElementId::Name(SharedString::from(format!("ctx-action-{ai}")));
            let label = SharedString::from(action.label.clone());
            let item = div()
                .id(action_id)
                .px_3()
                .py_1()
                .text_sm()
                .text_color(colors.text_primary)
                .cursor_pointer()
                .child(label);
            menu = menu.child(item);
        }

        menu
    }

    /// Render a single cell in a row.
    fn render_cell(&self, value: &str, colors: &BodyColors) -> gpui::Div {
        let text = SharedString::from(value.to_string());
        div().flex_1().px_2().py_1().text_sm().text_color(colors.text_primary).child(text)
    }

    /// Render the empty state placeholder.
    fn render_empty_state(&self, colors: &BodyColors) -> gpui::Div {
        div()
            .flex()
            .justify_center()
            .py_4()
            .text_sm()
            .text_color(colors.text_muted)
            .child("No resources found")
    }
}

// ---------------------------------------------------------------------------
// T335: Per-resource column definitions (FR-069)
// ---------------------------------------------------------------------------

fn col(id: &str, label: &str, weight: f32, sortable: bool) -> ColumnDef {
    ColumnDef { id: id.to_string(), label: label.to_string(), sortable, width_weight: weight }
}

/// Returns the column definitions for a specific resource kind (FR-069).
pub fn columns_for_kind(kind: &str) -> Vec<ColumnDef> {
    match kind {
        "Pod" => vec![
            col("name", "Name", 2.0, true),
            col("namespace", "Namespace", 1.0, true),
            col("containers", "Containers", 1.0, false),
            col("cpu", "CPU", 0.8, true),
            col("memory", "Memory", 0.8, true),
            col("restarts", "Restarts", 0.7, true),
            col("controlled_by", "Controlled By", 1.2, true),
            col("node", "Node", 1.0, true),
            col("ip", "IP", 0.8, true),
            col("qos", "QoS", 0.7, true),
            col("age", "Age", 0.8, true),
            col("status", "Status", 0.8, true),
        ],
        "Deployment" => vec![
            col("name", "Name", 2.0, true),
            col("namespace", "Namespace", 1.0, true),
            col("pods", "Pods", 0.8, true),
            col("ready", "Ready", 0.7, true),
            col("up_to_date", "Up-to-date", 0.7, true),
            col("available", "Available", 0.7, true),
            col("age", "Age", 0.8, true),
            col("conditions", "Conditions", 1.5, false),
        ],
        "StatefulSet" => vec![
            col("name", "Name", 2.0, true),
            col("namespace", "Namespace", 1.0, true),
            col("pods", "Pods", 0.8, true),
            col("replicas", "Replicas", 0.8, true),
            col("age", "Age", 0.8, true),
        ],
        "DaemonSet" => vec![
            col("name", "Name", 2.0, true),
            col("namespace", "Namespace", 1.0, true),
            col("desired", "Desired", 0.7, true),
            col("current", "Current", 0.7, true),
            col("ready", "Ready", 0.7, true),
            col("up_to_date", "Up-to-date", 0.7, true),
            col("available", "Available", 0.7, true),
            col("node_selector", "Node Selector", 1.2, false),
            col("age", "Age", 0.8, true),
        ],
        "ReplicaSet" => vec![
            col("name", "Name", 2.0, true),
            col("namespace", "Namespace", 1.0, true),
            col("desired", "Desired", 0.7, true),
            col("current", "Current", 0.7, true),
            col("ready", "Ready", 0.7, true),
            col("age", "Age", 0.8, true),
        ],
        "Job" => vec![
            col("name", "Name", 2.0, true),
            col("namespace", "Namespace", 1.0, true),
            col("completions", "Completions", 0.8, true),
            col("parallelism", "Parallelism", 0.7, true),
            col("duration", "Duration", 0.8, true),
            col("age", "Age", 0.8, true),
            col("status", "Status", 0.8, true),
            col("conditions", "Conditions", 1.5, false),
        ],
        "CronJob" => vec![
            col("name", "Name", 2.0, true),
            col("namespace", "Namespace", 1.0, true),
            col("schedule", "Schedule", 1.0, false),
            col("timezone", "Timezone", 0.8, true),
            col("suspend", "Suspend", 0.6, true),
            col("active", "Active", 0.6, true),
            col("last_schedule", "Last Schedule", 1.0, true),
            col("age", "Age", 0.8, true),
        ],
        "Service" => vec![
            col("name", "Name", 2.0, true),
            col("namespace", "Namespace", 1.0, true),
            col("type", "Type", 0.8, true),
            col("cluster_ip", "Cluster IP", 1.0, true),
            col("external_ip", "External IP", 1.0, true),
            col("ports", "Ports", 1.5, false),
            col("age", "Age", 0.8, true),
        ],
        "Ingress" => vec![
            col("name", "Name", 2.0, true),
            col("namespace", "Namespace", 1.0, true),
            col("load_balancer", "Load Balancers", 1.2, false),
            col("rules", "Rules", 1.5, false),
            col("age", "Age", 0.8, true),
        ],
        "ConfigMap" => vec![
            col("name", "Name", 2.0, true),
            col("namespace", "Namespace", 1.0, true),
            col("keys", "Keys", 1.0, false),
            col("age", "Age", 0.8, true),
        ],
        "Secret" => vec![
            col("name", "Name", 2.0, true),
            col("namespace", "Namespace", 1.0, true),
            col("type", "Type", 1.0, true),
            col("keys", "Keys", 1.0, false),
            col("age", "Age", 0.8, true),
        ],
        "Node" => vec![
            col("name", "Name", 2.0, true),
            col("cpu", "CPU", 0.8, true),
            col("memory", "Memory", 0.8, true),
            col("disk", "Disk", 0.8, true),
            col("taints", "Taints", 0.8, false),
            col("roles", "Roles", 1.0, false),
            col("internal_ip", "Internal IP", 1.0, true),
            col("schedulable", "Schedulable", 0.7, true),
            col("version", "Version", 1.0, true),
            col("age", "Age", 0.8, true),
            col("conditions", "Conditions", 1.5, false),
        ],
        "Namespace" => vec![
            col("name", "Name", 2.0, true),
            col("status", "Status", 0.8, true),
            col("age", "Age", 0.8, true),
        ],
        "PersistentVolume" => vec![
            col("name", "Name", 2.0, true),
            col("capacity", "Capacity", 0.8, true),
            col("access_modes", "Access Modes", 1.0, false),
            col("reclaim_policy", "Reclaim Policy", 1.0, true),
            col("status", "Status", 0.8, true),
            col("claim", "Claim", 1.5, true),
            col("storage_class", "Storage Class", 1.0, true),
            col("age", "Age", 0.8, true),
        ],
        "PersistentVolumeClaim" => vec![
            col("name", "Name", 2.0, true),
            col("namespace", "Namespace", 1.0, true),
            col("status", "Status", 0.8, true),
            col("volume", "Volume", 1.5, true),
            col("capacity", "Capacity", 0.8, true),
            col("storage_class", "Storage Class", 1.0, true),
            col("age", "Age", 0.8, true),
        ],
        "StorageClass" => vec![
            col("name", "Name", 2.0, true),
            col("provisioner", "Provisioner", 1.5, true),
            col("reclaim_policy", "Reclaim Policy", 1.0, true),
            col("volume_binding", "Volume Binding", 1.0, true),
            col("age", "Age", 0.8, true),
        ],
        "Event" => vec![
            col("type", "Type", 0.6, true),
            col("message", "Message", 3.0, false),
            col("namespace", "Namespace", 1.0, true),
            col("object", "Involved Object", 3.0, true),
            col("source", "Source", 1.5, true),
            col("count", "Count", 0.5, true),
            col("age", "Age", 0.4, true),
            col("last_seen", "Last Seen", 0.4, true),
        ],
        // Phase 1: RBAC + Network typed extractors
        "ServiceAccount" => vec![
            col("name", "Name", 2.0, true),
            col("namespace", "Namespace", 1.0, true),
            col("secrets", "Secrets", 0.7, true),
            col("age", "Age", 0.8, true),
        ],
        "Role" => vec![
            col("name", "Name", 2.0, true),
            col("namespace", "Namespace", 1.0, true),
            col("rules", "Rules", 0.7, true),
            col("age", "Age", 0.8, true),
        ],
        "ClusterRole" => vec![
            col("name", "Name", 2.0, true),
            col("rules", "Rules", 0.7, true),
            col("age", "Age", 0.8, true),
        ],
        "RoleBinding" => vec![
            col("name", "Name", 2.0, true),
            col("namespace", "Namespace", 1.0, true),
            col("role", "Role", 1.5, true),
            col("subjects", "Subjects", 2.0, false),
            col("age", "Age", 0.8, true),
        ],
        "ClusterRoleBinding" => vec![
            col("name", "Name", 2.0, true),
            col("role", "Role", 1.5, true),
            col("subjects", "Subjects", 2.0, false),
            col("age", "Age", 0.8, true),
        ],
        "NetworkPolicy" => vec![
            col("name", "Name", 2.0, true),
            col("namespace", "Namespace", 1.0, true),
            col("pod_selector", "Pod Selector", 1.5, false),
            col("policy_types", "Policy Types", 1.0, false),
            col("age", "Age", 0.8, true),
        ],
        "Endpoints" => vec![
            col("name", "Name", 2.0, true),
            col("namespace", "Namespace", 1.0, true),
            col("endpoints", "Endpoints", 2.0, false),
            col("age", "Age", 0.8, true),
        ],
        // Phase 2: Additional resource types
        "ResourceQuota" => vec![
            col("name", "Name", 2.0, true),
            col("namespace", "Namespace", 1.0, true),
            col("hard", "Hard", 2.0, false),
            col("used", "Used", 2.0, false),
            col("age", "Age", 0.8, true),
        ],
        "LimitRange" => vec![
            col("name", "Name", 2.0, true),
            col("namespace", "Namespace", 1.0, true),
            col("type", "Type", 0.8, true),
            col("default", "Default", 2.0, false),
            col("age", "Age", 0.8, true),
        ],
        "HorizontalPodAutoscaler" => vec![
            col("name", "Name", 2.0, true),
            col("namespace", "Namespace", 1.0, true),
            col("reference", "Reference", 1.5, true),
            col("min_max", "Min/Max", 0.8, true),
            col("current", "Current", 0.7, true),
            col("age", "Age", 0.8, true),
        ],
        "PodDisruptionBudget" => vec![
            col("name", "Name", 2.0, true),
            col("namespace", "Namespace", 1.0, true),
            col("min_available", "Min Available", 0.8, true),
            col("max_unavailable", "Max Unavailable", 0.8, true),
            col("allowed", "Allowed Disruptions", 0.8, true),
            col("age", "Age", 0.8, true),
        ],
        "PriorityClass" => vec![
            col("name", "Name", 2.0, true),
            col("value", "Value", 0.8, true),
            col("global_default", "Global Default", 0.8, true),
            col("age", "Age", 0.8, true),
        ],
        "Lease" => vec![
            col("name", "Name", 2.0, true),
            col("namespace", "Namespace", 1.0, true),
            col("holder", "Holder", 1.5, true),
            col("age", "Age", 0.8, true),
        ],
        "ValidatingWebhookConfiguration" => vec![
            col("name", "Name", 2.0, true),
            col("webhooks", "Webhooks", 0.7, true),
            col("age", "Age", 0.8, true),
        ],
        "MutatingWebhookConfiguration" => vec![
            col("name", "Name", 2.0, true),
            col("webhooks", "Webhooks", 0.7, true),
            col("age", "Age", 0.8, true),
        ],
        "EndpointSlice" => vec![
            col("name", "Name", 2.0, true),
            col("namespace", "Namespace", 1.0, true),
            col("address_type", "Address Type", 0.8, true),
            col("ports", "Ports", 1.0, false),
            col("endpoints", "Endpoints", 0.7, true),
            col("age", "Age", 0.8, true),
        ],
        "IngressClass" => vec![
            col("name", "Name", 2.0, true),
            col("controller", "Controller", 2.0, true),
            col("default", "Default", 0.7, true),
            col("age", "Age", 0.8, true),
        ],
        "Application" => vec![
            col("name", "Name", 2.0, true),
            col("namespace", "Namespace", 1.0, true),
            col("project", "Project", 0.8, true),
            col("sync_status", "Sync Status", 0.8, true),
            col("health", "Health", 0.8, true),
            col("repo", "Repo", 1.5, false),
            col("path", "Path", 1.0, true),
            col("destination", "Destination", 1.2, true),
            col("age", "Age", 0.8, true),
        ],
        "ApplicationSet" => vec![
            col("name", "Name", 2.0, true),
            col("namespace", "Namespace", 1.0, true),
            col("generators", "Generators", 0.8, true),
            col("template", "Template App", 1.2, true),
            col("age", "Age", 0.8, true),
        ],
        "AppProject" => vec![
            col("name", "Name", 2.0, true),
            col("namespace", "Namespace", 1.0, true),
            col("destinations", "Destinations", 0.8, true),
            col("sources", "Sources", 0.8, true),
            col("age", "Age", 0.8, true),
        ],
        // Default column set for any other resource type.
        _ => vec![
            col("name", "Name", 2.0, true),
            col("namespace", "Namespace", 1.0, true),
            col("age", "Age", 0.8, true),
            col("status", "Status", 0.8, true),
        ],
    }
}

// ---------------------------------------------------------------------------
// T336: Per-resource action definitions (FR-009)
// ---------------------------------------------------------------------------

/// An action that can be performed on a resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceAction {
    pub id: String,
    pub label: String,
}

impl ResourceAction {
    fn new(id: &str, label: &str) -> Self {
        Self { id: id.to_string(), label: label.to_string() }
    }
}

/// Returns the available actions for a specific resource kind (FR-009).
pub fn actions_for_kind(kind: &str) -> Vec<ResourceAction> {
    match kind {
        "Pod" => vec![
            ResourceAction::new("shell", "Shell"),
            ResourceAction::new("attach", "Attach"),
            ResourceAction::new("evict", "Evict"),
            ResourceAction::new("logs", "Logs"),
            ResourceAction::new("edit", "Edit"),
            ResourceAction::new("delete", "Delete"),
        ],
        "Deployment" => vec![
            ResourceAction::new("scale", "Scale"),
            ResourceAction::new("restart", "Restart"),
            ResourceAction::new("logs", "Logs"),
            ResourceAction::new("edit", "Edit"),
            ResourceAction::new("delete", "Delete"),
        ],
        "StatefulSet" => vec![
            ResourceAction::new("scale", "Scale"),
            ResourceAction::new("restart", "Restart"),
            ResourceAction::new("logs", "Logs"),
            ResourceAction::new("edit", "Edit"),
            ResourceAction::new("delete", "Delete"),
        ],
        "DaemonSet" => vec![
            ResourceAction::new("restart", "Restart"),
            ResourceAction::new("logs", "Logs"),
            ResourceAction::new("edit", "Edit"),
            ResourceAction::new("delete", "Delete"),
        ],
        "CronJob" => vec![
            ResourceAction::new("trigger", "Trigger"),
            ResourceAction::new("suspend", "Suspend"),
            ResourceAction::new("edit", "Edit"),
            ResourceAction::new("delete", "Delete"),
        ],
        "Job" => vec![
            ResourceAction::new("logs", "Logs"),
            ResourceAction::new("edit", "Edit"),
            ResourceAction::new("delete", "Delete"),
        ],
        "Node" => vec![
            ResourceAction::new("shell", "Shell"),
            ResourceAction::new("cordon", "Cordon"),
            ResourceAction::new("drain", "Drain"),
            ResourceAction::new("edit", "Edit"),
            ResourceAction::new("delete", "Delete"),
        ],
        // Default actions for all other resource types.
        _ => vec![ResourceAction::new("edit", "Edit"), ResourceAction::new("delete", "Delete")],
    }
}

/// Precomputed colors for rendering the header row.
struct HeaderColors {
    surface: Rgba,
    border: Rgba,
    text_secondary: Rgba,
}

/// Precomputed colors for rendering the body rows.
struct BodyColors {
    background: Rgba,
    selection: Rgba,
    border: Rgba,
    text_primary: Rgba,
    text_muted: Rgba,
}

impl Render for ResourceTableView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let header_colors = HeaderColors {
            surface: self.theme.colors.surface.to_gpui(),
            border: self.theme.colors.border.to_gpui(),
            text_secondary: self.theme.colors.text_secondary.to_gpui(),
        };

        let body_colors = BodyColors {
            background: self.theme.colors.background.to_gpui(),
            selection: self.theme.colors.selection.to_gpui(),
            border: self.theme.colors.border.to_gpui(),
            text_primary: self.theme.colors.text_primary.to_gpui(),
            text_muted: self.theme.colors.text_muted.to_gpui(),
        };

        div()
            .flex()
            .flex_col()
            .w_full()
            .overflow_hidden()
            .child(self.render_header_row(&header_colors))
            .child(self.render_body(&body_colors))
    }
}
