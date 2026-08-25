use gpui::{Context, ElementId, Rgba, SharedString, Window, div, prelude::*, px};

use crate::theme::Theme;

/// Result of a fuzzy search match against a Kubernetes resource.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchMatch {
    pub uid: String,
    pub name: String,
    pub namespace: Option<String>,
    pub kind: String,
    /// Higher score indicates a better match.
    pub score: u32,
    /// Which field produced the match: `"name"`, `"namespace"`, or `"label"`.
    pub matched_field: String,
}

/// State for the search bar component.
///
/// Tracks the current query, focus state, and computed search results.
#[derive(Debug, Default)]
pub struct SearchBarState {
    pub query: String,
    pub is_focused: bool,
    pub results: Vec<SearchMatch>,
}

impl SearchBarState {
    /// Creates a new empty search bar state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Updates the search query text.
    pub fn set_query(&mut self, query: &str) {
        self.query = query.to_string();
    }

    /// Clears the query and results.
    pub fn clear(&mut self) {
        self.query.clear();
        self.results.clear();
    }

    /// Returns true if the search bar has an active (non-empty) query.
    pub fn is_active(&self) -> bool {
        !self.query.is_empty()
    }
}

/// Performs case-insensitive fuzzy matching of `query` against `target`.
///
/// Returns `Some(score)` if the query matches, where higher scores indicate
/// better matches. Scoring bonuses are applied for:
/// - Exact match (case-insensitive): +100
/// - Prefix match: +50
/// - Consecutive character matches: +10 per consecutive pair
///
/// Returns `None` if the query does not match (i.e., not all query characters
/// appear in order within the target).
pub fn fuzzy_match(query: &str, target: &str) -> Option<u32> {
    if query.is_empty() {
        return Some(0);
    }
    if target.is_empty() {
        return None;
    }

    let query_lower = query.to_lowercase();
    let target_lower = target.to_lowercase();

    // Check for exact match
    if query_lower == target_lower {
        return Some(200);
    }

    // Check for substring match (case-insensitive)
    let is_substring = target_lower.contains(&query_lower);

    // Check for prefix match
    let is_prefix = target_lower.starts_with(&query_lower);

    // Subsequence matching: all query chars must appear in order
    let mut target_chars = target_lower.chars().peekable();
    let mut score: u32 = 0;
    let mut matched_count: u32 = 0;
    let mut last_match_pos: Option<usize> = None;
    let mut pos: usize = 0;

    for qc in query_lower.chars() {
        let mut found = false;
        for tc in target_chars.by_ref() {
            if tc == qc {
                matched_count += 1;
                // Bonus for consecutive matches
                if let Some(last) = last_match_pos {
                    if pos == last + 1 {
                        score += 10;
                    }
                }
                last_match_pos = Some(pos);
                pos += 1;
                found = true;
                break;
            }
            pos += 1;
        }
        if !found {
            return None;
        }
    }

    // Base score for matching at all
    score += matched_count;

    // Bonus for substring match
    if is_substring {
        score += 30;
    }

    // Bonus for prefix match
    if is_prefix {
        score += 50;
    }

    Some(score)
}

/// Searches across a set of Kubernetes resources using fuzzy matching.
///
/// Each item in `items` is a tuple of:
/// `(uid, name, namespace, kind, labels)` where labels is `Vec<(key, value)>`.
///
/// Returns matches sorted by score (highest first). The best-matching field
/// is recorded in [`SearchMatch::matched_field`].
/// Type alias for a resource search item: (uid, name, namespace, kind, labels).
pub type SearchItem = (String, String, Option<String>, String, Vec<(String, String)>);

pub fn search_resources(query: &str, items: &[SearchItem]) -> Vec<SearchMatch> {
    if query.is_empty() {
        return Vec::new();
    }

    let mut results: Vec<SearchMatch> = Vec::new();

    for (uid, name, namespace, kind, labels) in items {
        let mut best_score: Option<u32> = None;
        let mut best_field = String::new();

        // Match against name
        if let Some(score) = fuzzy_match(query, name) {
            if best_score.is_none_or(|s| score > s) {
                best_score = Some(score);
                best_field = "name".to_string();
            }
        }

        // Match against namespace
        if let Some(ns) = namespace {
            if let Some(score) = fuzzy_match(query, ns) {
                if best_score.is_none_or(|s| score > s) {
                    best_score = Some(score);
                    best_field = "namespace".to_string();
                }
            }
        }

        // Match against labels (both keys and values)
        for (key, value) in labels {
            let label_str = format!("{key}={value}");
            if let Some(score) = fuzzy_match(query, &label_str) {
                if best_score.is_none_or(|s| score > s) {
                    best_score = Some(score);
                    best_field = "label".to_string();
                }
            }
        }

        if let Some(score) = best_score {
            results.push(SearchMatch {
                uid: uid.clone(),
                name: name.clone(),
                namespace: namespace.clone(),
                kind: kind.clone(),
                score,
                matched_field: best_field,
            });
        }
    }

    results.sort_by_key(|a| std::cmp::Reverse(a.score));
    results
}

// ---------------------------------------------------------------------------
// T087: Global Search State
// ---------------------------------------------------------------------------

/// Scope for global search: controls which resources are searched.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum SearchScope {
    CurrentNamespace,
    #[default]
    AllNamespaces,
    AllClusters,
}

/// State for the global search overlay, supporting scoped search with
/// keyboard-navigable results.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GlobalSearchState {
    pub query: String,
    pub scope: SearchScope,
    pub results: Vec<SearchMatch>,
    pub selected_result_index: Option<usize>,
    pub is_open: bool,
    pub is_searching: bool,
}

impl GlobalSearchState {
    /// Creates a new global search state with AllNamespaces scope.
    pub fn new() -> Self {
        Self { scope: SearchScope::AllNamespaces, ..Default::default() }
    }

    /// Updates the search query text.
    pub fn set_query(&mut self, query: &str) {
        self.query = query.to_string();
    }

    /// Sets the search scope.
    pub fn set_scope(&mut self, scope: SearchScope) {
        self.scope = scope;
    }

    /// Sets the results from a search operation.
    pub fn set_results(&mut self, results: Vec<SearchMatch>) {
        self.results = results;
        // Reset selection when results change
        if self.results.is_empty() {
            self.selected_result_index = None;
        } else {
            self.selected_result_index = Some(0);
        }
        self.is_searching = false;
    }

    /// Move selection to the next result, wrapping to the top.
    pub fn select_next(&mut self) {
        if self.results.is_empty() {
            self.selected_result_index = None;
            return;
        }
        self.selected_result_index = Some(match self.selected_result_index {
            Some(idx) => (idx + 1) % self.results.len(),
            None => 0,
        });
    }

    /// Move selection to the previous result, wrapping to the bottom.
    pub fn select_previous(&mut self) {
        if self.results.is_empty() {
            self.selected_result_index = None;
            return;
        }
        self.selected_result_index = Some(match self.selected_result_index {
            Some(0) => self.results.len() - 1,
            Some(idx) => idx - 1,
            None => self.results.len() - 1,
        });
    }

    /// Returns the currently selected SearchMatch, if any.
    pub fn selected_result(&self) -> Option<&SearchMatch> {
        self.selected_result_index.and_then(|idx| self.results.get(idx))
    }

    /// Opens the global search overlay.
    pub fn open(&mut self) {
        self.is_open = true;
    }

    /// Closes the global search overlay.
    pub fn close(&mut self) {
        self.is_open = false;
    }

    /// Toggles the global search overlay open/closed.
    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }

    /// Clears the query, results, and selection.
    pub fn clear(&mut self) {
        self.query.clear();
        self.results.clear();
        self.selected_result_index = None;
        self.is_searching = false;
    }

    /// Returns the number of results.
    pub fn result_count(&self) -> usize {
        self.results.len()
    }

    /// Returns the display label for the current scope.
    pub fn scope_label(&self) -> &'static str {
        match self.scope {
            SearchScope::CurrentNamespace => "Current Namespace",
            SearchScope::AllNamespaces => "All Namespaces",
            SearchScope::AllClusters => "All Clusters",
        }
    }
}

/// View wrapper for `GlobalSearchState` that holds a theme for rendering.
pub struct GlobalSearchView {
    pub state: GlobalSearchState,
    pub theme: Theme,
}

impl GlobalSearchView {
    pub fn new(state: GlobalSearchState, theme: Theme) -> Self {
        Self { state, theme }
    }

    /// Render the scope selector bar.
    fn render_scope_bar(&self, colors: &GlobalSearchColors) -> gpui::Div {
        let label = SharedString::from(self.state.scope_label().to_string());
        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .px_3()
            .py_1()
            .child(div().text_xs().text_color(colors.text_muted).child("Scope:"))
            .child(div().text_xs().text_color(colors.accent).child(label))
    }

    /// Render the search input for global search.
    fn render_global_input(&self, colors: &GlobalSearchColors) -> gpui::Div {
        let text = if self.state.query.is_empty() {
            SharedString::from("Search all resources...")
        } else {
            SharedString::from(self.state.query.clone())
        };
        let text_color =
            if self.state.query.is_empty() { colors.text_muted } else { colors.text_primary };

        div()
            .flex()
            .items_center()
            .w_full()
            .px_3()
            .py_2()
            .rounded(px(6.0))
            .bg(colors.surface)
            .border_1()
            .border_color(colors.accent)
            .child(div().text_sm().text_color(text_color).child(text))
    }

    /// Render the results list for global search.
    fn render_global_results(&self, colors: &GlobalSearchColors) -> gpui::Div {
        let mut list = div()
            .flex()
            .flex_col()
            .w_full()
            .bg(colors.surface)
            .border_1()
            .border_color(colors.border)
            .rounded(px(6.0))
            .mt_1()
            .overflow_hidden();

        if self.state.is_searching {
            list = list.child(
                div().px_3().py_2().text_sm().text_color(colors.text_muted).child("Searching..."),
            );
        } else if self.state.results.is_empty() && !self.state.query.is_empty() {
            list = list.child(
                div()
                    .px_3()
                    .py_2()
                    .text_sm()
                    .text_color(colors.text_muted)
                    .child("No results found"),
            );
        } else {
            for (idx, result) in self.state.results.iter().enumerate() {
                let is_selected = self.state.selected_result_index == Some(idx);
                list = list.child(self.render_global_result_item(result, idx, is_selected, colors));
            }
        }

        list
    }

    /// Render a single result item with selection highlight.
    fn render_global_result_item(
        &self,
        result: &SearchMatch,
        idx: usize,
        is_selected: bool,
        colors: &GlobalSearchColors,
    ) -> gpui::Stateful<gpui::Div> {
        let name = SharedString::from(result.name.clone());
        let kind = SharedString::from(result.kind.clone());
        let ns_text = result.namespace.as_deref().unwrap_or("").to_string();
        let ns_label = SharedString::from(ns_text);
        let item_id = ElementId::Name(SharedString::from(format!("global-search-result-{idx}")));

        let bg = if is_selected { colors.surface_hover } else { colors.surface };

        let name_div = div().text_sm().text_color(colors.text_primary).child(name);

        let meta_div = div()
            .flex()
            .gap(px(8.0))
            .child(div().text_xs().text_color(colors.text_muted).child(kind))
            .child(div().text_xs().text_color(colors.text_muted).child(ns_label));

        div()
            .id(item_id)
            .flex()
            .flex_col()
            .w_full()
            .px_3()
            .py_1()
            .bg(bg)
            .cursor_pointer()
            .hover(|s| s.bg(colors.surface_hover))
            .child(name_div)
            .child(meta_div)
    }
}

/// Precomputed colors for the global search overlay.
#[allow(dead_code)]
struct GlobalSearchColors {
    surface: Rgba,
    surface_hover: Rgba,
    border: Rgba,
    accent: Rgba,
    text_primary: Rgba,
    text_muted: Rgba,
}

impl Render for GlobalSearchView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let colors = GlobalSearchColors {
            surface: self.theme.colors.surface.to_gpui(),
            surface_hover: self.theme.colors.surface_hover.to_gpui(),
            border: self.theme.colors.border.to_gpui(),
            accent: self.theme.colors.accent.to_gpui(),
            text_primary: self.theme.colors.text_primary.to_gpui(),
            text_muted: self.theme.colors.text_muted.to_gpui(),
        };

        let mut overlay = div().flex().flex_col().w_full();

        if self.state.is_open {
            overlay = overlay
                .child(self.render_scope_bar(&colors))
                .child(self.render_global_input(&colors));

            if !self.state.query.is_empty() || self.state.is_searching {
                overlay = overlay.child(self.render_global_results(&colors));
            }
        }

        overlay
    }
}

// ---------------------------------------------------------------------------
// GPUI Render (SearchBarView)
// ---------------------------------------------------------------------------

/// View wrapper for `SearchBarState` that holds a theme for rendering.
pub struct SearchBarView {
    pub state: SearchBarState,
    pub theme: Theme,
}

impl SearchBarView {
    pub fn new(state: SearchBarState, theme: Theme) -> Self {
        Self { state, theme }
    }

    /// Returns the placeholder text for the search input.
    pub fn placeholder_text(&self) -> &'static str {
        "Search resources... (Cmd+K)"
    }

    /// Returns the display text for the search input.
    pub fn display_text(&self) -> &str {
        if self.state.query.is_empty() { self.placeholder_text() } else { &self.state.query }
    }

    /// Whether the results dropdown should be visible.
    pub fn should_show_results(&self) -> bool {
        self.state.is_focused && !self.state.results.is_empty()
    }

    /// Render the search input field.
    fn render_input(&self, colors: &SearchColors) -> gpui::Div {
        let text = SharedString::from(self.display_text().to_string());
        let text_color =
            if self.state.query.is_empty() { colors.text_muted } else { colors.text_primary };

        let mut input =
            div().flex().items_center().w_full().px_3().py_2().rounded(px(6.0)).bg(colors.surface);

        if self.state.is_focused {
            input = input.border_1().border_color(colors.accent);
        } else {
            input = input.border_1().border_color(colors.border);
        }

        input.child(div().text_sm().text_color(text_color).child(text))
    }

    /// Render a single search result item.
    fn render_result_item(
        &self,
        result: &SearchMatch,
        idx: usize,
        colors: &SearchColors,
    ) -> gpui::Stateful<gpui::Div> {
        let name = SharedString::from(result.name.clone());
        let kind = SharedString::from(result.kind.clone());
        let ns_text = result.namespace.as_deref().unwrap_or("").to_string();
        let ns_label = SharedString::from(ns_text);

        let item_id = ElementId::Name(SharedString::from(format!("search-result-{idx}")));

        let name_div = div().text_sm().text_color(colors.text_primary).child(name);

        let meta_div = div()
            .flex()
            .gap(px(8.0))
            .child(div().text_xs().text_color(colors.text_muted).child(kind))
            .child(div().text_xs().text_color(colors.text_muted).child(ns_label));

        div()
            .id(item_id)
            .flex()
            .flex_col()
            .w_full()
            .px_3()
            .py_1()
            .cursor_pointer()
            .hover(|s| s.bg(colors.surface_hover))
            .child(name_div)
            .child(meta_div)
    }

    /// Render the results dropdown.
    fn render_results(&self, colors: &SearchColors) -> gpui::Div {
        let mut dropdown = div()
            .flex()
            .flex_col()
            .w_full()
            .bg(colors.surface)
            .border_1()
            .border_color(colors.border)
            .rounded(px(6.0))
            .mt_1()
            .overflow_hidden();

        for (idx, result) in self.state.results.iter().enumerate() {
            dropdown = dropdown.child(self.render_result_item(result, idx, colors));
        }

        dropdown
    }
}

/// Precomputed colors for rendering the search bar.
struct SearchColors {
    surface: Rgba,
    surface_hover: Rgba,
    border: Rgba,
    accent: Rgba,
    text_primary: Rgba,
    text_muted: Rgba,
}

impl Render for SearchBarView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let colors = SearchColors {
            surface: self.theme.colors.surface.to_gpui(),
            surface_hover: self.theme.colors.surface_hover.to_gpui(),
            border: self.theme.colors.border.to_gpui(),
            accent: self.theme.colors.accent.to_gpui(),
            text_primary: self.theme.colors.text_primary.to_gpui(),
            text_muted: self.theme.colors.text_muted.to_gpui(),
        };

        let mut container = div().flex().flex_col().w_full().child(self.render_input(&colors));

        if self.should_show_results() {
            container = container.child(self.render_results(&colors));
        }

        container
    }
}
