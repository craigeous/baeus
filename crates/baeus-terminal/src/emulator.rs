// Terminal emulator state management.
// Full ANSI parsing will be added with alacritty_terminal integration.

use serde::{Deserialize, Serialize};

/// Terminal size in rows and columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalSize {
    pub rows: u16,
    pub cols: u16,
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self { rows: 24, cols: 80 }
    }
}

/// A single cell in the terminal grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCell {
    pub character: char,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub fg_color: Option<TerminalColor>,
    pub bg_color: Option<TerminalColor>,
}

impl Default for TerminalCell {
    fn default() -> Self {
        Self {
            character: ' ',
            bold: false,
            italic: false,
            underline: false,
            fg_color: None,
            bg_color: None,
        }
    }
}

/// Terminal color representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalColor {
    /// 0-255 indexed color
    Indexed(u8),
    /// True color RGB
    Rgb(u8, u8, u8),
}

/// State of the terminal emulator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmulatorState {
    Idle,
    Running,
    Stopped,
    Error,
}

/// The terminal emulator: manages a grid of cells representing the terminal display.
/// Parser state for ANSI escape sequence processing.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ParserState {
    /// Normal text mode.
    Ground,
    /// Received ESC (0x1B), waiting for next byte.
    Escape,
    /// Inside a CSI sequence (ESC [), collecting params.
    Csi,
    /// Inside an OSC sequence (ESC ]), collecting string.
    Osc,
    /// Inside a DCS or other ignored sequence, skip until ST.
    Ignore,
}

/// The terminal emulator: manages a grid of cells representing the terminal display.
pub struct TerminalEmulator {
    pub size: TerminalSize,
    pub state: EmulatorState,
    pub cursor_row: u16,
    pub cursor_col: u16,
    pub cursor_visible: bool,
    grid: Vec<Vec<TerminalCell>>,
    title: String,
    scrollback: Vec<Vec<TerminalCell>>,
    scrollback_limit: usize,
    alternate_screen: bool,
    /// Current SGR attributes applied to new characters.
    current_fg: Option<TerminalColor>,
    current_bg: Option<TerminalColor>,
    current_bold: bool,
    /// ANSI parser state machine.
    parser_state: ParserState,
    /// Accumulated CSI parameter bytes.
    csi_params: Vec<u8>,
    /// Accumulated OSC string bytes.
    osc_data: Vec<u8>,
    /// Saved cursor position for DECSC/DECRC.
    saved_cursor: (u16, u16),
}

impl TerminalEmulator {
    /// Creates a new terminal emulator with the given size and an empty grid.
    pub fn new(size: TerminalSize) -> Self {
        let grid = Self::create_empty_grid(size);
        Self {
            size,
            state: EmulatorState::Idle,
            cursor_row: 0,
            cursor_col: 0,
            cursor_visible: true,
            grid,
            title: String::new(),
            scrollback: Vec::new(),
            scrollback_limit: 10_000,
            alternate_screen: false,
            current_fg: None,
            current_bg: None,
            current_bold: false,
            parser_state: ParserState::Ground,
            csi_params: Vec::new(),
            osc_data: Vec::new(),
            saved_cursor: (0, 0),
        }
    }

    /// Resizes the grid to the new terminal size.
    /// Existing content is preserved where it fits; new cells are filled with defaults.
    pub fn resize(&mut self, new_size: TerminalSize) {
        let mut new_grid = Self::create_empty_grid(new_size);

        let copy_rows = self.size.rows.min(new_size.rows) as usize;
        let copy_cols = self.size.cols.min(new_size.cols) as usize;

        for (row, new_row) in new_grid.iter_mut().enumerate().take(copy_rows) {
            for (col, new_cell) in new_row.iter_mut().enumerate().take(copy_cols) {
                *new_cell = self.grid[row][col].clone();
            }
        }

        self.grid = new_grid;
        self.size = new_size;

        // Clamp cursor to new bounds
        if self.cursor_row >= new_size.rows {
            self.cursor_row = new_size.rows.saturating_sub(1);
        }
        if self.cursor_col >= new_size.cols {
            self.cursor_col = new_size.cols.saturating_sub(1);
        }
    }

    /// Get the cell at the given row and column position.
    pub fn cell_at(&self, row: u16, col: u16) -> Option<&TerminalCell> {
        self.grid.get(row as usize).and_then(|r| r.get(col as usize))
    }

    /// Write a single character at the current cursor position and advance the cursor.
    pub fn write_char(&mut self, ch: char) {
        if self.cursor_row < self.size.rows && self.cursor_col < self.size.cols {
            self.grid[self.cursor_row as usize][self.cursor_col as usize].character = ch;
            self.cursor_col += 1;

            // Wrap to next line if past end of row
            if self.cursor_col >= self.size.cols {
                self.cursor_col = 0;
                self.advance_row();
            }
        }
    }

    /// Write a string at the current cursor position.
    pub fn write_str(&mut self, s: &str) {
        for ch in s.chars() {
            self.write_char(ch);
        }
    }

    /// Move cursor to the next line, scrolling if needed.
    pub fn newline(&mut self) {
        self.cursor_col = 0;
        self.advance_row();
    }

    /// Move cursor to column 0.
    pub fn carriage_return(&mut self) {
        self.cursor_col = 0;
    }

    /// Clear all cells on the screen.
    pub fn clear_screen(&mut self) {
        self.grid = Self::create_empty_grid(self.size);
        self.cursor_row = 0;
        self.cursor_col = 0;
    }

    /// Clear the current line.
    pub fn clear_line(&mut self) {
        if (self.cursor_row as usize) < self.grid.len() {
            let cols = self.size.cols as usize;
            self.grid[self.cursor_row as usize] = vec![TerminalCell::default(); cols];
        }
    }

    /// Move cursor to the given position, clamped to grid bounds.
    pub fn set_cursor(&mut self, row: u16, col: u16) {
        self.cursor_row = row.min(self.size.rows.saturating_sub(1));
        self.cursor_col = col.min(self.size.cols.saturating_sub(1));
    }

    /// Get the terminal title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Set the terminal title.
    pub fn set_title(&mut self, title: &str) {
        self.title = title.to_string();
    }

    /// Get the number of lines in the scrollback buffer.
    pub fn scrollback_lines(&self) -> usize {
        self.scrollback.len()
    }

    /// Get a reference to the visible grid rows.
    pub fn grid_rows(&self) -> Vec<Vec<&TerminalCell>> {
        self.grid.iter().map(|row| row.iter().collect()).collect()
    }

    /// Toggle the alternate screen buffer.
    pub fn set_alternate_screen(&mut self, on: bool) {
        self.alternate_screen = on;
    }

    /// Check if the alternate screen buffer is active.
    pub fn is_alternate_screen(&self) -> bool {
        self.alternate_screen
    }

    /// Process raw input bytes through the ANSI escape sequence parser.
    ///
    /// Handles:
    /// - Printable characters (written to grid with current attributes)
    /// - `\n` (newline), `\r` (carriage return), `\t` (tab), `\x08` (backspace)
    /// - CSI sequences (`ESC [` params letter): cursor movement, erase, SGR colors
    /// - OSC sequences (`ESC ]` for title setting)
    /// - DEC private modes (`ESC [ ?` for cursor visibility, bracketed paste, etc.)
    pub fn process_input(&mut self, input: &[u8]) {
        self.state = EmulatorState::Running;
        for &byte in input {
            match self.parser_state {
                ParserState::Ground => self.process_ground(byte),
                ParserState::Escape => self.process_escape(byte),
                ParserState::Csi => self.process_csi(byte),
                ParserState::Osc => self.process_osc(byte),
                ParserState::Ignore => self.process_ignore(byte),
            }
        }
    }

    /// Ground state: handle printable chars and control codes.
    fn process_ground(&mut self, byte: u8) {
        match byte {
            0x1B => {
                self.parser_state = ParserState::Escape;
            }
            b'\n' => self.newline(),
            b'\r' => self.carriage_return(),
            b'\t' => {
                // Tab: advance to next multiple of 8.
                let target = (self.cursor_col / 8 + 1) * 8;
                self.cursor_col = target.min(self.size.cols.saturating_sub(1));
            }
            0x08 => {
                // Backspace: move cursor left one.
                self.cursor_col = self.cursor_col.saturating_sub(1);
            }
            0x07 => {
                // BEL: ignore (visual bell could be added).
            }
            0x20..=0x7E => {
                // Printable ASCII — write with current attributes.
                self.write_char_with_attrs(byte as char);
            }
            0xC0..=0xFF => {
                // UTF-8 lead byte — for now treat as printable (simplified).
                // Real UTF-8 decoding would buffer multi-byte sequences.
                // Just skip non-ASCII for now to avoid garbled output.
            }
            _ => {
                // Other control codes — ignore.
            }
        }
    }

    /// Escape state: received ESC, dispatch based on next byte.
    fn process_escape(&mut self, byte: u8) {
        match byte {
            b'[' => {
                self.parser_state = ParserState::Csi;
                self.csi_params.clear();
            }
            b']' => {
                self.parser_state = ParserState::Osc;
                self.osc_data.clear();
            }
            b'(' | b')' | b'*' | b'+' => {
                // Designate character set — skip the next byte.
                self.parser_state = ParserState::Ignore;
            }
            b'7' => {
                // DECSC: Save cursor position.
                self.saved_cursor = (self.cursor_row, self.cursor_col);
                self.parser_state = ParserState::Ground;
            }
            b'8' => {
                // DECRC: Restore cursor position.
                self.cursor_row = self.saved_cursor.0.min(self.size.rows.saturating_sub(1));
                self.cursor_col = self.saved_cursor.1.min(self.size.cols.saturating_sub(1));
                self.parser_state = ParserState::Ground;
            }
            b'=' | b'>' => {
                // DECKPAM / DECKPNM — keypad modes, ignore.
                self.parser_state = ParserState::Ground;
            }
            b'M' => {
                // Reverse index: move cursor up, scroll if at top.
                if self.cursor_row > 0 {
                    self.cursor_row -= 1;
                }
                // Scrolling up at row 0 not implemented yet.
                self.parser_state = ParserState::Ground;
            }
            b'c' => {
                // RIS: Full reset.
                self.clear_screen();
                self.current_fg = None;
                self.current_bg = None;
                self.current_bold = false;
                self.parser_state = ParserState::Ground;
            }
            _ => {
                // Unknown ESC sequence — return to ground.
                self.parser_state = ParserState::Ground;
            }
        }
    }

    /// CSI state: collecting params and intermediate bytes until a final byte.
    fn process_csi(&mut self, byte: u8) {
        match byte {
            // Parameter bytes: digits, semicolons, question mark, etc.
            0x30..=0x3F => {
                self.csi_params.push(byte);
            }
            // Intermediate bytes (space, !, ", etc.) — just collect.
            0x20..=0x2F => {
                self.csi_params.push(byte);
            }
            // Final byte: dispatch the CSI command.
            0x40..=0x7E => {
                self.dispatch_csi(byte);
                self.parser_state = ParserState::Ground;
            }
            // ESC inside CSI — abort and re-enter escape.
            0x1B => {
                self.parser_state = ParserState::Escape;
            }
            _ => {
                // Invalid — abort.
                self.parser_state = ParserState::Ground;
            }
        }
    }

    /// OSC state: collecting string until BEL or ST (ESC \).
    fn process_osc(&mut self, byte: u8) {
        match byte {
            0x07 => {
                // BEL terminates OSC.
                self.dispatch_osc();
                self.parser_state = ParserState::Ground;
            }
            0x1B => {
                // Could be ESC \ (ST). Check next byte.
                // For simplicity, just terminate here.
                self.dispatch_osc();
                self.parser_state = ParserState::Escape;
            }
            _ => {
                if self.osc_data.len() < 4096 {
                    self.osc_data.push(byte);
                }
            }
        }
    }

    /// Ignore state: skip bytes until a final/terminating byte.
    fn process_ignore(&mut self, byte: u8) {
        match byte {
            0x20..=0x7E => {
                // Single character after the escape — consume and return.
                self.parser_state = ParserState::Ground;
            }
            0x1B => {
                self.parser_state = ParserState::Escape;
            }
            _ => {
                self.parser_state = ParserState::Ground;
            }
        }
    }

    /// Parse CSI parameter string into a Vec of u16 values.
    fn parse_csi_params(&self) -> Vec<u16> {
        let param_str: String = self
            .csi_params
            .iter()
            .filter(|&&b| b.is_ascii_digit() || b == b';')
            .map(|&b| b as char)
            .collect();
        if param_str.is_empty() {
            return Vec::new();
        }
        param_str.split(';').map(|s| s.parse::<u16>().unwrap_or(0)).collect()
    }

    /// Check if the CSI params start with '?'.
    fn csi_is_private(&self) -> bool {
        self.csi_params.first() == Some(&b'?')
    }

    /// Dispatch a CSI final byte command.
    fn dispatch_csi(&mut self, final_byte: u8) {
        let params = self.parse_csi_params();
        let p0 = params.first().copied().unwrap_or(0);
        let p1 = params.get(1).copied().unwrap_or(0);

        if self.csi_is_private() {
            // DEC private modes: CSI ? Pn h/l
            match final_byte {
                b'h' => self.handle_dec_set(&params),
                b'l' => self.handle_dec_reset(&params),
                _ => {} // Unknown private mode command.
            }
            return;
        }

        match final_byte {
            b'A' => {
                // CUU: Cursor Up. Default 1.
                let n = if p0 == 0 { 1 } else { p0 };
                self.cursor_row = self.cursor_row.saturating_sub(n);
            }
            b'B' => {
                // CUD: Cursor Down. Default 1.
                let n = if p0 == 0 { 1 } else { p0 };
                self.cursor_row = (self.cursor_row + n).min(self.size.rows.saturating_sub(1));
            }
            b'C' => {
                // CUF: Cursor Forward. Default 1.
                let n = if p0 == 0 { 1 } else { p0 };
                self.cursor_col = (self.cursor_col + n).min(self.size.cols.saturating_sub(1));
            }
            b'D' => {
                // CUB: Cursor Back. Default 1.
                let n = if p0 == 0 { 1 } else { p0 };
                self.cursor_col = self.cursor_col.saturating_sub(n);
            }
            b'H' | b'f' => {
                // CUP: Cursor Position. Params are 1-based.
                let row = if p0 == 0 { 0 } else { p0 - 1 };
                let col = if p1 == 0 { 0 } else { p1 - 1 };
                self.set_cursor(row, col);
            }
            b'J' => {
                // ED: Erase in Display.
                match p0 {
                    0 => self.erase_below(),
                    1 => self.erase_above(),
                    2 | 3 => self.clear_screen(),
                    _ => {}
                }
            }
            b'K' => {
                // EL: Erase in Line.
                match p0 {
                    0 => self.erase_line_right(),
                    1 => self.erase_line_left(),
                    2 => self.clear_line(),
                    _ => {}
                }
            }
            b'L' => {
                // IL: Insert Lines — simplified, just clear current line.
                self.clear_line();
            }
            b'M' => {
                // DL: Delete Lines — simplified, just clear current line.
                self.clear_line();
            }
            b'P' => {
                // DCH: Delete Characters — simplified, erase from cursor.
                self.erase_line_right();
            }
            b'X' => {
                // ECH: Erase Characters — erase n chars from cursor.
                let n = if p0 == 0 { 1 } else { p0 };
                let row = self.cursor_row as usize;
                if row < self.grid.len() {
                    for c in self.cursor_col..(self.cursor_col + n).min(self.size.cols) {
                        self.grid[row][c as usize] = TerminalCell::default();
                    }
                }
            }
            b'd' => {
                // VPA: Vertical Position Absolute (1-based row).
                let row = if p0 == 0 { 0 } else { p0 - 1 };
                self.cursor_row = row.min(self.size.rows.saturating_sub(1));
            }
            b'G' | b'`' => {
                // CHA: Cursor Character Absolute (1-based col).
                let col = if p0 == 0 { 0 } else { p0 - 1 };
                self.cursor_col = col.min(self.size.cols.saturating_sub(1));
            }
            b'm' => {
                // SGR: Select Graphic Rendition.
                self.handle_sgr(&params);
            }
            b'r' => {
                // DECSTBM: Set scrolling region — ignore for now.
            }
            b'h' => {
                // SM: Set Mode — mostly ignore.
            }
            b'l' => {
                // RM: Reset Mode — mostly ignore.
            }
            b'n' => {
                // DSR: Device Status Report — ignore (would need to write response).
            }
            b'c' => {
                // DA: Device Attributes — ignore.
            }
            b't' => {
                // Window manipulation — ignore.
            }
            b'@' => {
                // ICH: Insert Characters — ignore.
            }
            b'S' => {
                // SU: Scroll Up — simplified.
                let n = if p0 == 0 { 1 } else { p0 };
                for _ in 0..n {
                    self.scroll_up();
                }
            }
            b'T' => {
                // SD: Scroll Down — ignore for now.
            }
            _ => {
                // Unknown CSI command — ignore.
            }
        }
    }

    /// Handle DEC private set modes (CSI ? Pn h).
    fn handle_dec_set(&mut self, params: &[u16]) {
        for &p in params {
            match p {
                25 => self.cursor_visible = true,        // DECTCEM: show cursor
                1049 => self.set_alternate_screen(true), // Alternate screen buffer
                2004 => {}                               // Bracketed paste — ignore
                1 | 7 | 12 | 1000 | 1002 | 1006 => {}    // Various modes — ignore
                _ => {}
            }
        }
    }

    /// Handle DEC private reset modes (CSI ? Pn l).
    fn handle_dec_reset(&mut self, params: &[u16]) {
        for &p in params {
            match p {
                25 => self.cursor_visible = false,        // DECTCEM: hide cursor
                1049 => self.set_alternate_screen(false), // Normal screen buffer
                2004 => {}                                // Bracketed paste — ignore
                1 | 7 | 12 | 1000 | 1002 | 1006 => {}     // Various modes — ignore
                _ => {}
            }
        }
    }

    /// Handle SGR (Select Graphic Rendition) parameters.
    fn handle_sgr(&mut self, params: &[u16]) {
        if params.is_empty() {
            // ESC[m = reset.
            self.current_fg = None;
            self.current_bg = None;
            self.current_bold = false;
            return;
        }
        let mut i = 0;
        while i < params.len() {
            match params[i] {
                0 => {
                    self.current_fg = None;
                    self.current_bg = None;
                    self.current_bold = false;
                }
                1 => self.current_bold = true,
                22 => self.current_bold = false,
                30..=37 => self.current_fg = Some(TerminalColor::Indexed((params[i] - 30) as u8)),
                38 => {
                    // Extended foreground: 38;5;n or 38;2;r;g;b
                    if i + 1 < params.len() && params[i + 1] == 5 && i + 2 < params.len() {
                        self.current_fg = Some(TerminalColor::Indexed(params[i + 2] as u8));
                        i += 2;
                    } else if i + 1 < params.len() && params[i + 1] == 2 && i + 4 < params.len() {
                        self.current_fg = Some(TerminalColor::Rgb(
                            params[i + 2] as u8,
                            params[i + 3] as u8,
                            params[i + 4] as u8,
                        ));
                        i += 4;
                    }
                }
                39 => self.current_fg = None,
                40..=47 => self.current_bg = Some(TerminalColor::Indexed((params[i] - 40) as u8)),
                48 => {
                    // Extended background: 48;5;n or 48;2;r;g;b
                    if i + 1 < params.len() && params[i + 1] == 5 && i + 2 < params.len() {
                        self.current_bg = Some(TerminalColor::Indexed(params[i + 2] as u8));
                        i += 2;
                    } else if i + 1 < params.len() && params[i + 1] == 2 && i + 4 < params.len() {
                        self.current_bg = Some(TerminalColor::Rgb(
                            params[i + 2] as u8,
                            params[i + 3] as u8,
                            params[i + 4] as u8,
                        ));
                        i += 4;
                    }
                }
                49 => self.current_bg = None,
                90..=97 => {
                    self.current_fg = Some(TerminalColor::Indexed((params[i] - 90 + 8) as u8))
                }
                100..=107 => {
                    self.current_bg = Some(TerminalColor::Indexed((params[i] - 100 + 8) as u8))
                }
                // Italic, underline, strikethrough, etc. — acknowledge but don't track.
                2..=9 | 21..=29 | 50..=65 => {}
                _ => {}
            }
            i += 1;
        }
    }

    /// Dispatch an OSC sequence.
    fn dispatch_osc(&mut self) {
        let data = String::from_utf8_lossy(&self.osc_data).to_string();
        // OSC 0;title ST — set icon name and window title.
        // OSC 2;title ST — set window title.
        if let Some(rest) = data.strip_prefix("0;").or_else(|| data.strip_prefix("2;")) {
            self.title = rest.to_string();
        }
        // OSC 1;... — set icon name (ignore).
        // OSC 7;... — set working directory (ignore).
        // Other OSC sequences — ignore.
    }

    /// Write a character at the cursor with current SGR attributes.
    fn write_char_with_attrs(&mut self, ch: char) {
        if self.cursor_row < self.size.rows && self.cursor_col < self.size.cols {
            let cell = &mut self.grid[self.cursor_row as usize][self.cursor_col as usize];
            cell.character = ch;
            cell.fg_color = self.current_fg;
            cell.bg_color = self.current_bg;
            cell.bold = self.current_bold;
            self.cursor_col += 1;

            if self.cursor_col >= self.size.cols {
                self.cursor_col = 0;
                self.advance_row();
            }
        }
    }

    /// Erase from cursor to end of display.
    fn erase_below(&mut self) {
        // Erase from cursor to end of current line.
        self.erase_line_right();
        // Erase all lines below.
        for r in (self.cursor_row as usize + 1)..self.grid.len() {
            self.grid[r] = vec![TerminalCell::default(); self.size.cols as usize];
        }
    }

    /// Erase from beginning of display to cursor.
    fn erase_above(&mut self) {
        // Erase all lines above.
        for r in 0..self.cursor_row as usize {
            self.grid[r] = vec![TerminalCell::default(); self.size.cols as usize];
        }
        // Erase from beginning of current line to cursor.
        self.erase_line_left();
    }

    /// Erase from cursor to end of line.
    fn erase_line_right(&mut self) {
        let row = self.cursor_row as usize;
        if row < self.grid.len() {
            for c in self.cursor_col as usize..self.size.cols as usize {
                self.grid[row][c] = TerminalCell::default();
            }
        }
    }

    /// Erase from beginning of line to cursor.
    fn erase_line_left(&mut self) {
        let row = self.cursor_row as usize;
        if row < self.grid.len() {
            for c in 0..=self.cursor_col as usize {
                if c < self.grid[row].len() {
                    self.grid[row][c] = TerminalCell::default();
                }
            }
        }
    }

    /// Scroll the grid up by one line.
    fn scroll_up(&mut self) {
        if !self.grid.is_empty() {
            let top_row = self.grid.remove(0);
            self.scrollback.push(top_row);
            if self.scrollback.len() > self.scrollback_limit {
                let excess = self.scrollback.len() - self.scrollback_limit;
                self.scrollback.drain(0..excess);
            }
            self.grid.push(vec![TerminalCell::default(); self.size.cols as usize]);
        }
    }

    // --- Private helpers ---

    /// Create an empty grid of the given size filled with default cells.
    fn create_empty_grid(size: TerminalSize) -> Vec<Vec<TerminalCell>> {
        (0..size.rows as usize).map(|_| vec![TerminalCell::default(); size.cols as usize]).collect()
    }

    /// Advance the cursor row by one, scrolling the grid if at the bottom.
    fn advance_row(&mut self) {
        if self.cursor_row + 1 < self.size.rows {
            self.cursor_row += 1;
        } else {
            // Scroll: move top line to scrollback, shift grid up, add blank line at bottom
            if !self.grid.is_empty() {
                let top_row = self.grid.remove(0);
                self.scrollback.push(top_row);

                // Enforce scrollback limit
                if self.scrollback.len() > self.scrollback_limit {
                    let excess = self.scrollback.len() - self.scrollback_limit;
                    self.scrollback.drain(0..excess);
                }
            }
            self.grid.push(vec![TerminalCell::default(); self.size.cols as usize]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // TerminalSize tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_terminal_size_default() {
        let size = TerminalSize::default();
        assert_eq!(size.rows, 24);
        assert_eq!(size.cols, 80);
    }

    #[test]
    fn test_terminal_size_custom() {
        let size = TerminalSize { rows: 50, cols: 120 };
        assert_eq!(size.rows, 50);
        assert_eq!(size.cols, 120);
    }

    #[test]
    fn test_terminal_size_equality() {
        let a = TerminalSize { rows: 24, cols: 80 };
        let b = TerminalSize { rows: 24, cols: 80 };
        let c = TerminalSize { rows: 30, cols: 80 };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_terminal_size_clone() {
        let a = TerminalSize { rows: 24, cols: 80 };
        let b = a;
        assert_eq!(a, b);
    }

    // -----------------------------------------------------------------------
    // TerminalCell tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_terminal_cell_default() {
        let cell = TerminalCell::default();
        assert_eq!(cell.character, ' ');
        assert!(!cell.bold);
        assert!(!cell.italic);
        assert!(!cell.underline);
        assert_eq!(cell.fg_color, None);
        assert_eq!(cell.bg_color, None);
    }

    #[test]
    fn test_terminal_cell_with_attributes() {
        let cell = TerminalCell {
            character: 'A',
            bold: true,
            italic: false,
            underline: true,
            fg_color: Some(TerminalColor::Indexed(1)),
            bg_color: Some(TerminalColor::Rgb(255, 0, 0)),
        };
        assert_eq!(cell.character, 'A');
        assert!(cell.bold);
        assert!(!cell.italic);
        assert!(cell.underline);
        assert_eq!(cell.fg_color, Some(TerminalColor::Indexed(1)));
        assert_eq!(cell.bg_color, Some(TerminalColor::Rgb(255, 0, 0)));
    }

    // -----------------------------------------------------------------------
    // TerminalColor tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_terminal_color_indexed() {
        let color = TerminalColor::Indexed(42);
        assert_eq!(color, TerminalColor::Indexed(42));
        assert_ne!(color, TerminalColor::Indexed(43));
    }

    #[test]
    fn test_terminal_color_rgb() {
        let color = TerminalColor::Rgb(128, 64, 32);
        assert_eq!(color, TerminalColor::Rgb(128, 64, 32));
        assert_ne!(color, TerminalColor::Rgb(128, 64, 33));
    }

    #[test]
    fn test_terminal_color_variants_not_equal() {
        let indexed = TerminalColor::Indexed(0);
        let rgb = TerminalColor::Rgb(0, 0, 0);
        assert_ne!(indexed, rgb);
    }

    // -----------------------------------------------------------------------
    // EmulatorState tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_emulator_state_variants() {
        assert_eq!(EmulatorState::Idle, EmulatorState::Idle);
        assert_eq!(EmulatorState::Running, EmulatorState::Running);
        assert_eq!(EmulatorState::Stopped, EmulatorState::Stopped);
        assert_eq!(EmulatorState::Error, EmulatorState::Error);
        assert_ne!(EmulatorState::Idle, EmulatorState::Running);
    }

    // -----------------------------------------------------------------------
    // TerminalEmulator construction tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_emulator_new_default_size() {
        let emu = TerminalEmulator::new(TerminalSize::default());
        assert_eq!(emu.size, TerminalSize { rows: 24, cols: 80 });
        assert_eq!(emu.state, EmulatorState::Idle);
        assert_eq!(emu.cursor_row, 0);
        assert_eq!(emu.cursor_col, 0);
        assert!(emu.cursor_visible);
        assert_eq!(emu.title(), "");
        assert_eq!(emu.scrollback_lines(), 0);
        assert!(!emu.is_alternate_screen());
    }

    #[test]
    fn test_emulator_new_custom_size() {
        let size = TerminalSize { rows: 10, cols: 20 };
        let emu = TerminalEmulator::new(size);
        assert_eq!(emu.size, size);
        let rows = emu.grid_rows();
        assert_eq!(rows.len(), 10);
        assert_eq!(rows[0].len(), 20);
    }

    #[test]
    fn test_emulator_grid_initialized_with_spaces() {
        let emu = TerminalEmulator::new(TerminalSize { rows: 3, cols: 3 });
        for row in 0..3u16 {
            for col in 0..3u16 {
                let cell = emu.cell_at(row, col).unwrap();
                assert_eq!(cell.character, ' ');
            }
        }
    }

    // -----------------------------------------------------------------------
    // cell_at tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_cell_at_valid_position() {
        let emu = TerminalEmulator::new(TerminalSize { rows: 5, cols: 5 });
        assert!(emu.cell_at(0, 0).is_some());
        assert!(emu.cell_at(4, 4).is_some());
    }

    #[test]
    fn test_cell_at_out_of_bounds() {
        let emu = TerminalEmulator::new(TerminalSize { rows: 5, cols: 5 });
        assert!(emu.cell_at(5, 0).is_none());
        assert!(emu.cell_at(0, 5).is_none());
        assert!(emu.cell_at(10, 10).is_none());
    }

    // -----------------------------------------------------------------------
    // write_char tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_write_char_at_origin() {
        let mut emu = TerminalEmulator::new(TerminalSize { rows: 5, cols: 10 });
        emu.write_char('A');
        assert_eq!(emu.cell_at(0, 0).unwrap().character, 'A');
        assert_eq!(emu.cursor_row, 0);
        assert_eq!(emu.cursor_col, 1);
    }

    #[test]
    fn test_write_char_advances_cursor() {
        let mut emu = TerminalEmulator::new(TerminalSize { rows: 5, cols: 10 });
        emu.write_char('A');
        emu.write_char('B');
        emu.write_char('C');
        assert_eq!(emu.cell_at(0, 0).unwrap().character, 'A');
        assert_eq!(emu.cell_at(0, 1).unwrap().character, 'B');
        assert_eq!(emu.cell_at(0, 2).unwrap().character, 'C');
        assert_eq!(emu.cursor_col, 3);
    }

    #[test]
    fn test_write_char_wraps_at_end_of_line() {
        let mut emu = TerminalEmulator::new(TerminalSize { rows: 5, cols: 3 });
        emu.write_char('A');
        emu.write_char('B');
        emu.write_char('C'); // fills last column, wraps
        assert_eq!(emu.cursor_row, 1);
        assert_eq!(emu.cursor_col, 0);
    }

    // -----------------------------------------------------------------------
    // write_str tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_write_str_basic() {
        let mut emu = TerminalEmulator::new(TerminalSize { rows: 5, cols: 10 });
        emu.write_str("Hello");
        assert_eq!(emu.cell_at(0, 0).unwrap().character, 'H');
        assert_eq!(emu.cell_at(0, 1).unwrap().character, 'e');
        assert_eq!(emu.cell_at(0, 2).unwrap().character, 'l');
        assert_eq!(emu.cell_at(0, 3).unwrap().character, 'l');
        assert_eq!(emu.cell_at(0, 4).unwrap().character, 'o');
        assert_eq!(emu.cursor_col, 5);
    }

    #[test]
    fn test_write_str_wraps_across_lines() {
        let mut emu = TerminalEmulator::new(TerminalSize { rows: 5, cols: 3 });
        emu.write_str("ABCDE");
        // "ABC" on row 0, "DE" on row 1
        assert_eq!(emu.cell_at(0, 0).unwrap().character, 'A');
        assert_eq!(emu.cell_at(0, 2).unwrap().character, 'C');
        assert_eq!(emu.cell_at(1, 0).unwrap().character, 'D');
        assert_eq!(emu.cell_at(1, 1).unwrap().character, 'E');
        assert_eq!(emu.cursor_row, 1);
        assert_eq!(emu.cursor_col, 2);
    }

    // -----------------------------------------------------------------------
    // newline tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_newline_moves_to_next_row() {
        let mut emu = TerminalEmulator::new(TerminalSize { rows: 5, cols: 10 });
        emu.cursor_col = 5;
        emu.newline();
        assert_eq!(emu.cursor_row, 1);
        assert_eq!(emu.cursor_col, 0);
    }

    #[test]
    fn test_newline_scrolls_at_bottom() {
        let mut emu = TerminalEmulator::new(TerminalSize { rows: 3, cols: 5 });
        // Use 4 chars (< cols) to avoid auto-wrap at end of line
        emu.write_str("AAAA"); // row 0
        emu.newline();
        emu.write_str("BBBB"); // row 1
        emu.newline();
        emu.write_str("CCCC"); // row 2
        emu.newline(); // should scroll

        // Row 0 ("AAAA") should have moved to scrollback
        assert_eq!(emu.scrollback_lines(), 1);

        // After scroll, row 0 is now "BBBB ", row 1 is "CCCC ", row 2 is blank
        assert_eq!(emu.cell_at(0, 0).unwrap().character, 'B');
        assert_eq!(emu.cell_at(1, 0).unwrap().character, 'C');
        assert_eq!(emu.cell_at(2, 0).unwrap().character, ' ');

        // Cursor should be at bottom row, col 0
        assert_eq!(emu.cursor_row, 2);
        assert_eq!(emu.cursor_col, 0);
    }

    // -----------------------------------------------------------------------
    // carriage_return tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_carriage_return() {
        let mut emu = TerminalEmulator::new(TerminalSize { rows: 5, cols: 10 });
        emu.write_str("Hello");
        assert_eq!(emu.cursor_col, 5);
        emu.carriage_return();
        assert_eq!(emu.cursor_col, 0);
        assert_eq!(emu.cursor_row, 0); // row unchanged
    }

    // -----------------------------------------------------------------------
    // clear_screen tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_clear_screen() {
        let mut emu = TerminalEmulator::new(TerminalSize { rows: 5, cols: 10 });
        emu.write_str("Hello");
        emu.clear_screen();
        assert_eq!(emu.cursor_row, 0);
        assert_eq!(emu.cursor_col, 0);
        for row in 0..5u16 {
            for col in 0..10u16 {
                assert_eq!(emu.cell_at(row, col).unwrap().character, ' ');
            }
        }
    }

    // -----------------------------------------------------------------------
    // clear_line tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_clear_line() {
        let mut emu = TerminalEmulator::new(TerminalSize { rows: 5, cols: 10 });
        emu.write_str("Hello");
        emu.newline();
        emu.write_str("World");

        // Clear line 1 (where cursor is)
        emu.clear_line();
        // Line 0 should still have content
        assert_eq!(emu.cell_at(0, 0).unwrap().character, 'H');
        // Line 1 should be cleared
        for col in 0..10u16 {
            assert_eq!(emu.cell_at(1, col).unwrap().character, ' ');
        }
    }

    // -----------------------------------------------------------------------
    // set_cursor tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_set_cursor_valid() {
        let mut emu = TerminalEmulator::new(TerminalSize { rows: 10, cols: 20 });
        emu.set_cursor(5, 10);
        assert_eq!(emu.cursor_row, 5);
        assert_eq!(emu.cursor_col, 10);
    }

    #[test]
    fn test_set_cursor_clamped_to_bounds() {
        let mut emu = TerminalEmulator::new(TerminalSize { rows: 10, cols: 20 });
        emu.set_cursor(100, 200);
        assert_eq!(emu.cursor_row, 9);
        assert_eq!(emu.cursor_col, 19);
    }

    // -----------------------------------------------------------------------
    // title tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_title_default_empty() {
        let emu = TerminalEmulator::new(TerminalSize::default());
        assert_eq!(emu.title(), "");
    }

    #[test]
    fn test_set_and_get_title() {
        let mut emu = TerminalEmulator::new(TerminalSize::default());
        emu.set_title("My Terminal");
        assert_eq!(emu.title(), "My Terminal");
    }

    #[test]
    fn test_title_can_be_overwritten() {
        let mut emu = TerminalEmulator::new(TerminalSize::default());
        emu.set_title("First");
        emu.set_title("Second");
        assert_eq!(emu.title(), "Second");
    }

    // -----------------------------------------------------------------------
    // scrollback tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_scrollback_initially_empty() {
        let emu = TerminalEmulator::new(TerminalSize::default());
        assert_eq!(emu.scrollback_lines(), 0);
    }

    #[test]
    fn test_scrollback_accumulates_on_scroll() {
        let mut emu = TerminalEmulator::new(TerminalSize { rows: 2, cols: 5 });
        // Use 4 chars (< cols) to avoid auto-wrap at end of line
        emu.write_str("AAAA");
        emu.newline();
        emu.write_str("BBBB");
        emu.newline(); // scrolls row 0 to scrollback

        assert_eq!(emu.scrollback_lines(), 1);

        emu.write_str("CCCC");
        emu.newline(); // scrolls again

        assert_eq!(emu.scrollback_lines(), 2);
    }

    // -----------------------------------------------------------------------
    // resize tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_resize_preserves_content() {
        let mut emu = TerminalEmulator::new(TerminalSize { rows: 5, cols: 10 });
        emu.write_str("Hi");
        emu.resize(TerminalSize { rows: 10, cols: 20 });
        assert_eq!(emu.cell_at(0, 0).unwrap().character, 'H');
        assert_eq!(emu.cell_at(0, 1).unwrap().character, 'i');
        assert_eq!(emu.size, TerminalSize { rows: 10, cols: 20 });
    }

    #[test]
    fn test_resize_shrinks() {
        let mut emu = TerminalEmulator::new(TerminalSize { rows: 10, cols: 20 });
        emu.set_cursor(8, 15);
        emu.resize(TerminalSize { rows: 5, cols: 10 });
        assert_eq!(emu.size, TerminalSize { rows: 5, cols: 10 });
        // Cursor should be clamped
        assert_eq!(emu.cursor_row, 4);
        assert_eq!(emu.cursor_col, 9);
    }

    #[test]
    fn test_resize_grid_dimensions() {
        let mut emu = TerminalEmulator::new(TerminalSize { rows: 3, cols: 3 });
        emu.resize(TerminalSize { rows: 6, cols: 8 });
        let rows = emu.grid_rows();
        assert_eq!(rows.len(), 6);
        assert_eq!(rows[0].len(), 8);
    }

    // -----------------------------------------------------------------------
    // grid_rows tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_grid_rows_returns_all_rows() {
        let emu = TerminalEmulator::new(TerminalSize { rows: 3, cols: 4 });
        let rows = emu.grid_rows();
        assert_eq!(rows.len(), 3);
        for row in &rows {
            assert_eq!(row.len(), 4);
        }
    }

    // -----------------------------------------------------------------------
    // alternate screen tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_alternate_screen_default_off() {
        let emu = TerminalEmulator::new(TerminalSize::default());
        assert!(!emu.is_alternate_screen());
    }

    #[test]
    fn test_alternate_screen_toggle() {
        let mut emu = TerminalEmulator::new(TerminalSize::default());
        emu.set_alternate_screen(true);
        assert!(emu.is_alternate_screen());
        emu.set_alternate_screen(false);
        assert!(!emu.is_alternate_screen());
    }

    // -----------------------------------------------------------------------
    // process_input tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_process_input_printable_chars() {
        let mut emu = TerminalEmulator::new(TerminalSize { rows: 5, cols: 10 });
        emu.process_input(b"ABC");
        assert_eq!(emu.cell_at(0, 0).unwrap().character, 'A');
        assert_eq!(emu.cell_at(0, 1).unwrap().character, 'B');
        assert_eq!(emu.cell_at(0, 2).unwrap().character, 'C');
        assert_eq!(emu.state, EmulatorState::Running);
    }

    #[test]
    fn test_process_input_newline() {
        let mut emu = TerminalEmulator::new(TerminalSize { rows: 5, cols: 10 });
        emu.process_input(b"AB\nCD");
        assert_eq!(emu.cell_at(0, 0).unwrap().character, 'A');
        assert_eq!(emu.cell_at(0, 1).unwrap().character, 'B');
        assert_eq!(emu.cell_at(1, 0).unwrap().character, 'C');
        assert_eq!(emu.cell_at(1, 1).unwrap().character, 'D');
    }

    #[test]
    fn test_process_input_carriage_return() {
        let mut emu = TerminalEmulator::new(TerminalSize { rows: 5, cols: 10 });
        emu.process_input(b"ABC\rX");
        // \r moves cursor to col 0, then X overwrites A
        assert_eq!(emu.cell_at(0, 0).unwrap().character, 'X');
        assert_eq!(emu.cell_at(0, 1).unwrap().character, 'B');
        assert_eq!(emu.cell_at(0, 2).unwrap().character, 'C');
    }

    #[test]
    fn test_process_input_crlf() {
        let mut emu = TerminalEmulator::new(TerminalSize { rows: 5, cols: 10 });
        emu.process_input(b"AB\r\nCD");
        // \r goes to col 0, \n goes to next line
        assert_eq!(emu.cell_at(0, 0).unwrap().character, 'A');
        assert_eq!(emu.cell_at(0, 1).unwrap().character, 'B');
        assert_eq!(emu.cell_at(1, 0).unwrap().character, 'C');
        assert_eq!(emu.cell_at(1, 1).unwrap().character, 'D');
    }

    #[test]
    fn test_process_input_ignores_control_codes() {
        let mut emu = TerminalEmulator::new(TerminalSize { rows: 5, cols: 10 });
        // 0x07 is BEL — ignored. ESC starts an escape sequence.
        emu.process_input(&[b'A', 0x07, b'B']);
        assert_eq!(emu.cell_at(0, 0).unwrap().character, 'A');
        assert_eq!(emu.cell_at(0, 1).unwrap().character, 'B');
        assert_eq!(emu.cursor_col, 2);
    }

    #[test]
    fn test_process_input_esc_consumes_next_byte() {
        let mut emu = TerminalEmulator::new(TerminalSize { rows: 5, cols: 10 });
        // ESC followed by an unknown char is consumed as an escape sequence.
        emu.process_input(&[b'A', 0x1B, b'Z', b'B']);
        assert_eq!(emu.cell_at(0, 0).unwrap().character, 'A');
        // 'Z' after ESC is consumed, 'B' is next printable.
        assert_eq!(emu.cell_at(0, 1).unwrap().character, 'B');
        assert_eq!(emu.cursor_col, 2);
    }

    #[test]
    fn test_process_input_sets_running_state() {
        let mut emu = TerminalEmulator::new(TerminalSize::default());
        assert_eq!(emu.state, EmulatorState::Idle);
        emu.process_input(b"x");
        assert_eq!(emu.state, EmulatorState::Running);
    }

    #[test]
    fn test_process_input_scrolls_on_overflow() {
        let mut emu = TerminalEmulator::new(TerminalSize { rows: 2, cols: 3 });
        // Fill row 0: "ABC", wrap to row 1: "DEF", wrap triggers scroll, then "G"
        emu.process_input(b"ABCDEFG");
        // After writing "ABC" -> cursor wraps to row 1
        // After writing "DEF" -> cursor wraps, scrolls (row 0="ABC" to scrollback), row 0="DEF", row 1 blank
        // Then "G" written at row 1, col 0
        assert_eq!(emu.scrollback_lines(), 1);
        assert_eq!(emu.cell_at(0, 0).unwrap().character, 'D');
        assert_eq!(emu.cell_at(1, 0).unwrap().character, 'G');
    }

    #[test]
    fn test_process_input_empty() {
        let mut emu = TerminalEmulator::new(TerminalSize::default());
        emu.process_input(b"");
        // State transitions to Running even with empty input
        assert_eq!(emu.state, EmulatorState::Running);
        assert_eq!(emu.cursor_row, 0);
        assert_eq!(emu.cursor_col, 0);
    }

    // -----------------------------------------------------------------------
    // Edge case tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_write_char_on_1x1_grid() {
        let mut emu = TerminalEmulator::new(TerminalSize { rows: 1, cols: 1 });
        emu.write_char('X');
        // Writing 'X' fills the only cell, then wraps -> scroll
        assert_eq!(emu.scrollback_lines(), 1);
    }

    #[test]
    fn test_multiple_scrolls() {
        let mut emu = TerminalEmulator::new(TerminalSize { rows: 2, cols: 5 });
        for i in 0..10 {
            emu.write_str(&format!("{:>5}", i));
            emu.newline();
        }
        assert!(emu.scrollback_lines() >= 9);
    }

    #[test]
    fn test_clear_screen_after_writing() {
        let mut emu = TerminalEmulator::new(TerminalSize { rows: 5, cols: 10 });
        emu.write_str("Test data");
        emu.newline();
        emu.write_str("More data");
        emu.clear_screen();
        assert_eq!(emu.cursor_row, 0);
        assert_eq!(emu.cursor_col, 0);
        assert_eq!(emu.cell_at(0, 0).unwrap().character, ' ');
        assert_eq!(emu.cell_at(1, 0).unwrap().character, ' ');
    }
}
