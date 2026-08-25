use crate::components::search_bar::fuzzy_match;
use gpui::{Context, ElementId, SharedString, Window, div, prelude::*, px, rgb, rgba};

/// Categories for command palette entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandCategory {
    /// Navigate to a view or resource.
    Navigation,
    /// Perform an action (scale, restart, delete, etc.).
    Action,
    /// Jump to a specific Kubernetes resource.
    Resource,
    /// Switch or configure a view mode.
    View,
}

/// A single entry in the command palette.
#[derive(Debug, Clone)]
pub struct CommandEntry {
    pub id: String,
    pub label: String,
    pub description: String,
    pub category: CommandCategory,
    /// Optional keyboard shortcut display string (e.g., "Cmd+K").
    pub shortcut: Option<String>,
    /// Identifier for the action to execute when this entry is selected.
    pub action: String,
}

impl CommandEntry {
    /// Create a new command entry.
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        description: impl Into<String>,
        category: CommandCategory,
        action: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: description.into(),
            category,
            shortcut: None,
            action: action.into(),
        }
    }

    /// Builder method to set a keyboard shortcut.
    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }
}

/// State machine for the command palette overlay.
#[derive(Debug, Default)]
pub struct CommandPaletteState {
    /// Current search query text.
    pub query: String,
    /// The filtered and scored results based on the current query.
    pub results: Vec<ScoredCommand>,
    /// Index of the currently selected result (0-based).
    pub selected_index: usize,
    /// Whether the command palette is currently visible.
    pub visible: bool,
    /// Whether the palette is currently loading results (e.g., async search).
    pub loading: bool,
    /// The full registry of available commands.
    pub commands: Vec<CommandEntry>,
}

/// A command entry paired with its fuzzy match score.
#[derive(Debug, Clone)]
pub struct ScoredCommand {
    pub entry: CommandEntry,
    pub score: u32,
}

impl CommandPaletteState {
    /// Create a new command palette state with the given commands.
    pub fn new(commands: Vec<CommandEntry>) -> Self {
        Self { commands, ..Default::default() }
    }

    /// Open the command palette, resetting the query and results.
    pub fn open(&mut self) {
        self.visible = true;
        self.query.clear();
        self.results.clear();
        self.selected_index = 0;
        self.loading = false;
    }

    /// Close the command palette.
    pub fn close(&mut self) {
        self.visible = false;
        self.query.clear();
        self.results.clear();
        self.selected_index = 0;
        self.loading = false;
    }

    /// Toggle the command palette visibility.
    pub fn toggle(&mut self) {
        if self.visible {
            self.close();
        } else {
            self.open();
        }
    }

    /// Update the search query and recompute filtered results.
    pub fn set_query(&mut self, query: &str) {
        self.query = query.to_string();
        self.results = self.filtered_results();
        // Reset selection to the top of the new results
        self.selected_index = 0;
    }

    /// Move selection to the next result (wraps around).
    pub fn select_next(&mut self) {
        if !self.results.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.results.len();
        }
    }

    /// Move selection to the previous result (wraps around).
    pub fn select_previous(&mut self) {
        if !self.results.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.results.len() - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }

    /// Execute the currently selected command.
    ///
    /// Returns the action string of the selected command, or `None` if no
    /// result is selected.
    pub fn execute_selected(&mut self) -> Option<String> {
        let action = self.results.get(self.selected_index).map(|sc| sc.entry.action.clone());
        if action.is_some() {
            self.close();
        }
        action
    }

    /// Compute filtered results by fuzzy-matching the query against command
    /// labels and descriptions. Returns results sorted by score descending.
    pub fn filtered_results(&self) -> Vec<ScoredCommand> {
        if self.query.is_empty() {
            // When no query, return all commands with a default score of 0
            return self
                .commands
                .iter()
                .map(|entry| ScoredCommand { entry: entry.clone(), score: 0 })
                .collect();
        }

        let mut scored: Vec<ScoredCommand> = Vec::new();

        for entry in &self.commands {
            let label_score = fuzzy_match(&self.query, &entry.label);
            let desc_score = fuzzy_match(&self.query, &entry.description);

            // Take the best score from label or description
            let best_score = match (label_score, desc_score) {
                (Some(l), Some(d)) => Some(l.max(d)),
                (Some(l), None) => Some(l),
                (None, Some(d)) => Some(d),
                (None, None) => None,
            };

            if let Some(score) = best_score {
                scored.push(ScoredCommand { entry: entry.clone(), score });
            }
        }

        scored.sort_by(|a, b| b.score.cmp(&a.score));
        scored
    }
}

// ---------------------------------------------------------------------------
// GPUI View
// ---------------------------------------------------------------------------

pub struct CommandPaletteView {
    state: CommandPaletteState,
}

impl CommandPaletteView {
    pub fn new(commands: Vec<CommandEntry>) -> Self {
        Self { state: CommandPaletteState::new(commands) }
    }

    pub fn state(&self) -> &CommandPaletteState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut CommandPaletteState {
        &mut self.state
    }
}

impl Render for CommandPaletteView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.state.visible {
            return div();
        }

        // Clone results to avoid borrow conflicts
        let results: Vec<ScoredCommand> = self.state.results.clone();
        let query_text = SharedString::from(
            if self.state.query.is_empty() { "Type a command..." } else { &self.state.query }
                .to_string(),
        );
        let query_is_empty = self.state.query.is_empty();

        // Backdrop: semi-transparent overlay covering the full screen
        let backdrop = div().absolute().top_0().left_0().size_full().bg(rgba(0x00000080));

        // Search input display
        let search_input = div()
            .w_full()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(rgb(0x374151))
            .text_sm()
            .text_color(if query_is_empty { rgb(0x6B7280) } else { rgb(0xF9FAFB) })
            .child(query_text);

        // Results list (needs id for overflow_y_scroll)
        let mut results_list = div()
            .id("command-palette-results")
            .flex()
            .flex_col()
            .w_full()
            .max_h(px(320.0))
            .overflow_y_scroll();

        for (idx, result) in results.iter().enumerate() {
            let is_selected = idx == self.state.selected_index;
            let label_text = SharedString::from(result.entry.label.clone());
            let desc_text = SharedString::from(result.entry.description.clone());

            let row_id =
                ElementId::Name(SharedString::from(format!("command-palette-result-{idx}")));

            let action_idx = idx;
            let mut row =
                div().id(row_id).flex().items_center().px_3().py_2().cursor_pointer().text_sm();

            if is_selected {
                row = row.bg(rgb(0x374151));
            }

            // Click handler to select and execute
            row = row.on_click(cx.listener(move |this, _event, _window, _cx| {
                this.state.selected_index = action_idx;
                this.state.execute_selected();
            }));

            // Left side: label + description
            let left = div()
                .flex()
                .flex_col()
                .flex_1()
                .child(div().text_sm().text_color(rgb(0xF9FAFB)).child(label_text))
                .child(div().text_xs().text_color(rgb(0x9CA3AF)).child(desc_text));

            row = row.child(left);

            // Right side: optional shortcut
            if let Some(ref shortcut) = result.entry.shortcut {
                let shortcut_text = SharedString::from(shortcut.clone());
                let shortcut_el = div()
                    .px_2()
                    .py(px(1.0))
                    .rounded(px(4.0))
                    .bg(rgb(0x1F2937))
                    .text_xs()
                    .text_color(rgb(0x6B7280))
                    .child(shortcut_text);
                row = row.child(shortcut_el);
            }

            results_list = results_list.child(row);
        }

        // Dialog box
        let dialog = div()
            .w(px(480.0))
            .bg(rgb(0x1F2937))
            .rounded(px(8.0))
            .border_1()
            .border_color(rgb(0x4B5563))
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(search_input)
            .child(results_list);

        // Overlay container that centers the dialog
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .justify_center()
            .pt(px(80.0))
            .child(backdrop)
            .child(dialog)
    }
}
