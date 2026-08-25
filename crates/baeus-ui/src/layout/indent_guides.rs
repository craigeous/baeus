//! Navigator indent guide decoration for the sidebar tree.
//!
//! Replaces text-based tree connector characters (├─, └─, │) with
//! pixel-painted vertical indent guide lines using GPUI's `paint_quad`.

use std::ops::Range;

use gpui::*;

use super::sidebar::NavigatorFlatEntry;

/// Horizontal offset from the left edge before the first indent level.
pub const INDENT_OFFSET: f32 = 12.0;
/// Horizontal step per indent depth level.
pub const INDENT_STEP: f32 = 16.0;
/// Uniform row height for navigator tree items.
pub const ITEM_HEIGHT: f32 = 24.0;

/// A vertical guide line segment in the navigator tree.
#[derive(Debug, Clone, PartialEq)]
pub struct IndentGuideLayout {
    /// Tree depth level (1 = direct children of cluster, 2 = children of category).
    pub depth: usize,
    /// First row index (inclusive) where this guide starts.
    pub start_row: usize,
    /// Last row index (inclusive) where this guide ends.
    pub end_row: usize,
}

/// Compute indent guide line segments from a flat list of navigator entries.
///
/// Pure function — no GPUI dependency, fully testable.
///
/// Algorithm: tracks "active" guide segments per depth level.
/// A guide at depth d starts when the first entry at depth d appears,
/// and ends when an entry at depth d has `is_last_sibling = true`.
pub fn compute_indent_guides(entries: &[NavigatorFlatEntry]) -> Vec<IndentGuideLayout> {
    let mut guides = Vec::new();
    // For each depth, the start row of the currently open guide segment (if any).
    let mut active: Vec<Option<usize>> = Vec::new();

    for (row, entry) in entries.iter().enumerate() {
        let depth = entry.depth();
        let is_last = entry.is_last_sibling();

        // Ensure active has enough slots
        while active.len() <= depth {
            active.push(None);
        }

        // Close any deeper guides that are still open
        // (when we return to a shallower depth, deeper guides should have closed)
        for (d, slot) in active.iter_mut().enumerate().skip(depth + 1) {
            if let Some(start) = slot.take() {
                guides.push(IndentGuideLayout {
                    depth: d,
                    start_row: start,
                    end_row: row.saturating_sub(1),
                });
            }
        }

        // Start a guide at this depth if none is active
        if active[depth].is_none() {
            active[depth] = Some(row);
        }

        // If this is the last sibling at this depth, close the guide
        if is_last {
            if let Some(start) = active[depth].take() {
                guides.push(IndentGuideLayout { depth, start_row: start, end_row: row });
            }
        }
    }

    // Close any remaining open guides at the last row
    let last_row = entries.len().saturating_sub(1);
    for (depth, slot) in active.iter().enumerate() {
        if let Some(start) = slot {
            guides.push(IndentGuideLayout { depth, start_row: *start, end_row: last_row });
        }
    }

    guides
}

/// Decoration that paints vertical indent guide lines on a `uniform_list`.
pub struct NavigatorIndentGuideDecoration {
    entries: Vec<NavigatorFlatEntry>,
    guide_color: Hsla,
}

impl NavigatorIndentGuideDecoration {
    pub fn new(entries: Vec<NavigatorFlatEntry>, guide_color: Hsla) -> Self {
        Self { entries, guide_color }
    }
}

impl UniformListDecoration for NavigatorIndentGuideDecoration {
    fn compute(
        &self,
        _visible_range: Range<usize>,
        bounds: Bounds<Pixels>,
        scroll_offset: Point<Pixels>,
        item_height: Pixels,
        _item_count: usize,
        _window: &mut Window,
        _cx: &mut App,
    ) -> AnyElement {
        let guides = compute_indent_guides(&self.entries);
        let guide_color = self.guide_color;
        let viewport_height = bounds.size.height;
        let item_h = item_height;
        let scroll_y = scroll_offset.y;

        canvas(
            |_bounds, _window, _cx| {},
            move |bounds, (), window, _cx| {
                let zero = px(0.0);
                for guide in &guides {
                    let x = px(INDENT_OFFSET
                        + (guide.depth as f32 - 1.0) * INDENT_STEP
                        + INDENT_STEP / 2.0);

                    let y_start = item_h * guide.start_row as f32 + scroll_y;
                    let y_end = item_h * (guide.end_row as f32 + 1.0) + scroll_y;

                    // Clip to visible viewport
                    let y_start = if y_start > zero { y_start } else { zero };
                    let y_end = if y_end < viewport_height { y_end } else { viewport_height };

                    if y_end > y_start {
                        let guide_bounds = Bounds::new(
                            point(bounds.origin.x + x, bounds.origin.y + y_start),
                            size(px(1.0), y_end - y_start),
                        );
                        window.paint_quad(fill(guide_bounds, guide_color));
                    }
                }
            },
        )
        .w_full()
        .h_full()
        .into_any_element()
    }
}
