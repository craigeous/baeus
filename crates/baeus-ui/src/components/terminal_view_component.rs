// T049: TerminalViewComponent with impl Render
// T050: Keyboard input handling
// T051: Terminal output processing
//
// Separated from terminal_view.rs to avoid proc-macro recursion limit issues
// when the GPUI Render impl lives alongside many inline tests.

use gpui::*;

use baeus_terminal::emulator::{TerminalCell, TerminalColor, TerminalEmulator, TerminalSize};

use crate::components::terminal_view::{
    TerminalConnectionState, TerminalDisplayMode, TerminalViewState,
};
use crate::theme::Theme;

// ---------------------------------------------------------------------------
// Precomputed colors
// ---------------------------------------------------------------------------

/// Precomputed colors for rendering the terminal.
struct TerminalColors {
    bg: Rgba,
    text: Rgba,
    surface: Rgba,
    border: Rgba,
    text_secondary: Rgba,
    text_muted: Rgba,
    accent: Rgba,
    success: Rgba,
    warning: Rgba,
    error: Rgba,
}

// ---------------------------------------------------------------------------
// TerminalViewComponent
// ---------------------------------------------------------------------------

/// The GPUI renderable terminal view component.
///
/// Wraps a `TerminalViewState` and `TerminalEmulator` together, providing
/// `impl Render` for GPUI, keyboard input buffering (T050), and terminal
/// output processing (T051).
pub struct TerminalViewComponent {
    pub state: TerminalViewState,
    pub emulator: TerminalEmulator,
    pub theme: Theme,
    input_buffer: Vec<u8>,
    focus_handle: Option<FocusHandle>,
}

impl TerminalViewComponent {
    /// Creates a new terminal view component with a focus handle (production use).
    pub fn new_with_cx(state: TerminalViewState, theme: Theme, cx: &mut Context<Self>) -> Self {
        let size = TerminalSize { rows: state.rows, cols: state.cols };
        Self {
            state,
            emulator: TerminalEmulator::new(size),
            theme,
            input_buffer: Vec::new(),
            focus_handle: Some(cx.focus_handle()),
        }
    }

    /// Creates a new terminal view component without focus handle (for tests).
    pub fn new(state: TerminalViewState, theme: Theme) -> Self {
        let size = TerminalSize { rows: state.rows, cols: state.cols };
        Self {
            state,
            emulator: TerminalEmulator::new(size),
            theme,
            input_buffer: Vec::new(),
            focus_handle: None,
        }
    }

    // -- T050: Keyboard input --

    /// Handle a GPUI keystroke by mapping it to the correct terminal bytes.
    ///
    /// Priority: Ctrl+key combos first, then special keys, then `key_char`
    /// (the actual typed character including shift/option transforms), then
    /// fall back to `key` for single printable chars.
    pub fn handle_keystroke(&mut self, keystroke: &Keystroke) {
        let key = keystroke.key.as_str();

        // Ctrl+key → control character (0x01..0x1A for a..z)
        if keystroke.modifiers.control {
            if let Some(ch) = key.chars().next() {
                if key.len() == 1 && ch.is_ascii_lowercase() {
                    self.input_buffer.push(ch as u8 - b'a' + 1);
                    return;
                }
            }
            // Ctrl+special combos we don't handle — ignore
            return;
        }

        // Platform (Cmd) modifier — don't send to terminal
        if keystroke.modifiers.platform {
            return;
        }

        // Special keys (GPUI uses lowercase names)
        match key {
            "enter" => {
                self.input_buffer.push(b'\r');
                return;
            }
            "backspace" => {
                self.input_buffer.push(0x7f);
                return;
            }
            "tab" => {
                self.input_buffer.push(b'\t');
                return;
            }
            "escape" => {
                self.input_buffer.push(0x1b);
                return;
            }
            "space" => {
                self.input_buffer.push(b' ');
                return;
            }
            "up" => {
                self.input_buffer.extend_from_slice(b"\x1b[A");
                return;
            }
            "down" => {
                self.input_buffer.extend_from_slice(b"\x1b[B");
                return;
            }
            "right" => {
                self.input_buffer.extend_from_slice(b"\x1b[C");
                return;
            }
            "left" => {
                self.input_buffer.extend_from_slice(b"\x1b[D");
                return;
            }
            "home" => {
                self.input_buffer.extend_from_slice(b"\x1b[H");
                return;
            }
            "end" => {
                self.input_buffer.extend_from_slice(b"\x1b[F");
                return;
            }
            "delete" => {
                self.input_buffer.extend_from_slice(b"\x1b[3~");
                return;
            }
            "pageup" => {
                self.input_buffer.extend_from_slice(b"\x1b[5~");
                return;
            }
            "pagedown" => {
                self.input_buffer.extend_from_slice(b"\x1b[6~");
                return;
            }
            _ => {}
        }

        // Prefer key_char (handles shift, option transforms, IME)
        if let Some(ref ch) = keystroke.key_char {
            if !ch.is_empty() {
                self.input_buffer.extend_from_slice(ch.as_bytes());
                return;
            }
        }

        // Fallback: single printable character from key field
        if key.len() == 1 {
            self.input_buffer.extend_from_slice(key.as_bytes());
        }
        // Multi-char key names we don't handle (f1, f2, etc.) — ignore
    }

    /// Take all pending input bytes, clearing the internal buffer.
    /// The parent view forwards these to the PTY session.
    pub fn take_pending_input(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.input_buffer)
    }

    /// Returns true if there is pending input to send.
    pub fn has_pending_input(&self) -> bool {
        !self.input_buffer.is_empty()
    }

    // -- T051: Terminal output --

    /// Feed raw bytes from the PTY into the emulator, updating display state.
    ///
    /// After processing, the emulator title is synced to the view state.
    pub fn process_output(&mut self, data: &[u8]) {
        self.emulator.process_input(data);
        // Sync title from emulator if changed.
        let emu_title = self.emulator.title();
        if !emu_title.is_empty() && emu_title != self.state.title {
            self.state.title = emu_title.to_string();
        }
    }

    /// Resize both the view state and the underlying emulator.
    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.state.resize(rows, cols);
        self.emulator.resize(TerminalSize { rows, cols });
    }

    /// Returns the grid dimensions (rows, cols) that the emulator is using.
    pub fn grid_dimensions(&self) -> (u16, u16) {
        (self.emulator.size.rows, self.emulator.size.cols)
    }

    /// Returns the cursor position (row, col).
    pub fn cursor_position(&self) -> (u16, u16) {
        (self.emulator.cursor_row, self.emulator.cursor_col)
    }

    // -- Render helpers (each returns Div to keep chains short) --

    /// Compute colors from theme once per render.
    fn colors(&self) -> TerminalColors {
        TerminalColors {
            bg: self.theme.colors.background.to_gpui(),
            text: self.theme.colors.text_primary.to_gpui(),
            surface: self.theme.colors.surface.to_gpui(),
            border: self.theme.colors.border.to_gpui(),
            text_secondary: self.theme.colors.text_secondary.to_gpui(),
            text_muted: self.theme.colors.text_muted.to_gpui(),
            accent: self.theme.colors.accent.to_gpui(),
            success: self.theme.colors.success.to_gpui(),
            warning: self.theme.colors.warning.to_gpui(),
            error: self.theme.colors.error.to_gpui(),
        }
    }

    /// Render the header bar with connection indicator, title, and controls.
    fn render_header(&self, colors: &TerminalColors) -> Div {
        let indicator = self.render_connection_indicator(colors);
        let title_text = SharedString::from(self.state.title.clone());
        let mode_label = SharedString::from(self.display_mode_label());

        let title_el = div().text_sm().text_color(colors.text).mx_2().child(title_text);

        let mode_el = div().text_xs().text_color(colors.text_muted).child(mode_label);

        let font_label = SharedString::from(format!("{}px", self.state.settings.font_size as u32));
        let font_el = div().text_xs().text_color(colors.text_secondary).mx_2().child(font_label);

        div()
            .flex()
            .flex_row()
            .items_center()
            .px_2()
            .py_1()
            .bg(colors.surface)
            .border_b_1()
            .border_color(colors.border)
            .child(indicator)
            .child(title_el)
            .child(div().flex_grow())
            .child(font_el)
            .child(mode_el)
    }

    /// Render the connection state indicator dot.
    fn render_connection_indicator(&self, colors: &TerminalColors) -> Div {
        let color = match &self.state.connection_state {
            TerminalConnectionState::Connected => colors.success,
            TerminalConnectionState::Connecting => colors.warning,
            TerminalConnectionState::Disconnected => colors.text_muted,
            TerminalConnectionState::Error(_) => colors.error,
        };
        div().w(px(8.0)).h(px(8.0)).rounded(px(4.0)).bg(color)
    }

    /// Render the terminal grid (all visible rows).
    fn render_grid(&self, colors: &TerminalColors) -> Div {
        let grid_rows = self.emulator.grid_rows();
        let cr = self.emulator.cursor_row;
        let cc = self.emulator.cursor_col;

        let mut container = div()
            .flex()
            .flex_col()
            .flex_grow()
            .bg(colors.bg)
            .font_family("Menlo")
            .overflow_hidden();

        for (ri, row) in grid_rows.iter().enumerate() {
            let row_el = self.render_grid_row(row, ri, cr, cc, colors);
            container = container.child(row_el);
        }

        container
    }

    /// Render a single row of terminal cells.
    fn render_grid_row(
        &self,
        row: &[&TerminalCell],
        row_index: usize,
        cursor_row: u16,
        cursor_col: u16,
        colors: &TerminalColors,
    ) -> Div {
        let mut row_el = div().flex().flex_row();

        for (ci, cell) in row.iter().enumerate() {
            let is_cur = row_index == cursor_row as usize
                && ci == cursor_col as usize
                && self.emulator.cursor_visible;
            let cell_el = self.render_cell(cell, is_cur, colors);
            row_el = row_el.child(cell_el);
        }

        row_el
    }

    /// Render a single terminal cell, highlighting it if it is the cursor.
    fn render_cell(&self, cell: &TerminalCell, is_cursor: bool, colors: &TerminalColors) -> Div {
        let fg = self.map_color(&cell.fg_color, colors.text);
        let bg = self.map_color(&cell.bg_color, colors.bg);
        let ch = SharedString::from(cell.character.to_string());

        let d = div().text_xs().w(px(self.cell_width())).h(px(self.cell_height()));

        if is_cursor {
            d.bg(fg).text_color(bg).child(ch)
        } else {
            d.bg(bg).text_color(fg).child(ch)
        }
    }

    /// Render an overlay for non-connected states (error, connecting).
    fn render_connection_overlay(&self, colors: &TerminalColors) -> Div {
        let backdrop = crate::theme::Color::rgba(0, 0, 0, 160).to_gpui();
        match &self.state.connection_state {
            TerminalConnectionState::Connecting => {
                let msg = SharedString::from("Connecting...");
                self.overlay_base(backdrop)
                    .child(div().text_base().text_color(colors.warning).child(msg))
            }
            TerminalConnectionState::Error(err) => {
                let msg = SharedString::from(format!("Error: {err}"));
                self.overlay_base(backdrop)
                    .child(div().text_base().text_color(colors.error).child(msg))
            }
            _ => div(),
        }
    }

    /// Shared overlay base container.
    fn overlay_base(&self, bg_color: Rgba) -> Div {
        div()
            .absolute()
            .top_0()
            .left_0()
            .w_full()
            .h_full()
            .flex()
            .justify_center()
            .items_center()
            .bg(bg_color)
    }

    /// Render scrollback indicator when scrolled up.
    fn render_scrollback_indicator(&self, colors: &TerminalColors) -> Div {
        if !self.state.is_scrolled_up() {
            return div();
        }
        let label =
            SharedString::from(format!("Scrolled up {} lines", self.state.scrollback_offset,));
        div()
            .flex()
            .justify_center()
            .py_1()
            .bg(colors.surface)
            .border_b_1()
            .border_color(colors.border)
            .child(div().text_xs().text_color(colors.accent).child(label))
    }

    // -- Private utilities --

    /// Map a TerminalColor to an Rgba, using a default if None.
    fn map_color(&self, color: &Option<TerminalColor>, default: Rgba) -> Rgba {
        match color {
            None => default,
            Some(TerminalColor::Rgb(r, g, b)) => {
                Rgba { r: *r as f32 / 255.0, g: *g as f32 / 255.0, b: *b as f32 / 255.0, a: 1.0 }
            }
            Some(TerminalColor::Indexed(idx)) => ansi_color(*idx),
        }
    }

    /// Cell width in pixels based on font size.
    fn cell_width(&self) -> f32 {
        self.state.settings.font_size * 0.6
    }

    /// Cell height in pixels based on font size.
    fn cell_height(&self) -> f32 {
        self.state.settings.font_size * 1.2
    }

    /// Human-readable label for the current display mode.
    fn display_mode_label(&self) -> &'static str {
        match self.state.settings.display_mode {
            TerminalDisplayMode::Inline => "Inline",
            TerminalDisplayMode::Fullscreen => "Fullscreen",
            TerminalDisplayMode::Split => "Split",
        }
    }
}

// ---------------------------------------------------------------------------
// impl Render
// ---------------------------------------------------------------------------

impl Render for TerminalViewComponent {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = self.colors();
        let needs_overlay = matches!(
            self.state.connection_state,
            TerminalConnectionState::Connecting | TerminalConnectionState::Error(_)
        );

        let header = self.render_header(&colors);
        let scroll_ind = self.render_scrollback_indicator(&colors);
        let grid = self.render_grid(&colors);

        let mut base = div()
            .id("terminal-view-root")
            .flex()
            .flex_col()
            .size_full()
            .bg(colors.bg)
            .border_1()
            .border_color(colors.border)
            .key_context("terminal")
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, _cx| {
                this.handle_keystroke(&event.keystroke);
            }))
            .on_click(cx.listener(|this, _event: &ClickEvent, window, _cx| {
                // Focus the terminal on click so keyboard events are received.
                if let Some(ref handle) = this.focus_handle {
                    window.focus(handle);
                }
            }))
            .child(header)
            .child(scroll_ind)
            .child(grid);

        // Track focus so on_key_down events are delivered to this view.
        if let Some(ref handle) = self.focus_handle {
            base = base.track_focus(handle);
        }

        if needs_overlay { base.child(self.render_connection_overlay(&colors)) } else { base }
    }
}

// ---------------------------------------------------------------------------
// ANSI color palette (free function to avoid &self overhead)
// ---------------------------------------------------------------------------

/// Standard ANSI 16-color palette mapping.
fn ansi_color(idx: u8) -> Rgba {
    match idx {
        0 => Rgba { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
        1 => Rgba { r: 0.8, g: 0.0, b: 0.0, a: 1.0 },
        2 => Rgba { r: 0.0, g: 0.8, b: 0.0, a: 1.0 },
        3 => Rgba { r: 0.8, g: 0.8, b: 0.0, a: 1.0 },
        4 => Rgba { r: 0.0, g: 0.0, b: 0.8, a: 1.0 },
        5 => Rgba { r: 0.8, g: 0.0, b: 0.8, a: 1.0 },
        6 => Rgba { r: 0.0, g: 0.8, b: 0.8, a: 1.0 },
        7 => Rgba { r: 0.75, g: 0.75, b: 0.75, a: 1.0 },
        8 => Rgba { r: 0.5, g: 0.5, b: 0.5, a: 1.0 },
        9 => Rgba { r: 1.0, g: 0.0, b: 0.0, a: 1.0 },
        10 => Rgba { r: 0.0, g: 1.0, b: 0.0, a: 1.0 },
        11 => Rgba { r: 1.0, g: 1.0, b: 0.0, a: 1.0 },
        12 => Rgba { r: 0.0, g: 0.0, b: 1.0, a: 1.0 },
        13 => Rgba { r: 1.0, g: 0.0, b: 1.0, a: 1.0 },
        14 => Rgba { r: 0.0, g: 1.0, b: 1.0, a: 1.0 },
        15 => Rgba { r: 1.0, g: 1.0, b: 1.0, a: 1.0 },
        _ => {
            let v = idx as f32 / 255.0;
            Rgba { r: v, g: v, b: v, a: 1.0 }
        }
    }
}
