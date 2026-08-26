use ropey::Rope;

#[derive(Debug)]
pub struct TextBuffer {
    rope: Rope,
    undo_stack: Vec<BufferOp>,
    redo_stack: Vec<BufferOp>,
}

#[derive(Debug, Clone)]
enum BufferOp {
    Insert { pos: usize, text: String },
    Delete { pos: usize, text: String },
}

impl TextBuffer {
    pub fn new() -> Self {
        Self { rope: Rope::new(), undo_stack: Vec::new(), redo_stack: Vec::new() }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(text: &str) -> Self {
        Self { rope: Rope::from_str(text), undo_stack: Vec::new(), redo_stack: Vec::new() }
    }

    pub fn insert(&mut self, char_idx: usize, text: &str) {
        let clamped = char_idx.min(self.rope.len_chars());
        self.rope.insert(clamped, text);
        self.undo_stack.push(BufferOp::Insert { pos: clamped, text: text.to_string() });
        self.redo_stack.clear();
    }

    pub fn delete(&mut self, start: usize, end: usize) {
        let start = start.min(self.rope.len_chars());
        let end = end.min(self.rope.len_chars());
        if start >= end {
            return;
        }
        let deleted: String = self.rope.slice(start..end).chars().collect();
        self.rope.remove(start..end);
        self.undo_stack.push(BufferOp::Delete { pos: start, text: deleted });
        self.redo_stack.clear();
    }

    pub fn undo(&mut self) -> bool {
        let Some(op) = self.undo_stack.pop() else {
            return false;
        };
        match &op {
            BufferOp::Insert { pos, text } => {
                let end = *pos + text.chars().count();
                self.rope.remove(*pos..end);
            }
            BufferOp::Delete { pos, text } => {
                self.rope.insert(*pos, text);
            }
        }
        self.redo_stack.push(op);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(op) = self.redo_stack.pop() else {
            return false;
        };
        match &op {
            BufferOp::Insert { pos, text } => {
                self.rope.insert(*pos, text);
            }
            BufferOp::Delete { pos, text } => {
                let end = *pos + text.chars().count();
                self.rope.remove(*pos..end);
            }
        }
        self.undo_stack.push(op);
        true
    }

    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn line(&self, line_idx: usize) -> Option<String> {
        if line_idx < self.rope.len_lines() {
            Some(self.rope.line(line_idx).to_string())
        } else {
            None
        }
    }

    pub fn is_empty(&self) -> bool {
        self.rope.len_chars() == 0
    }
}

impl Default for TextBuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_buffer_is_empty() {
        let buf = TextBuffer::new();
        assert!(buf.is_empty());
        assert_eq!(buf.len_chars(), 0);
    }

    #[test]
    fn test_from_str() {
        let buf = TextBuffer::from_str("hello world");
        assert_eq!(buf.text(), "hello world");
        assert_eq!(buf.len_chars(), 11);
    }

    #[test]
    fn test_insert() {
        let mut buf = TextBuffer::new();
        buf.insert(0, "hello");
        assert_eq!(buf.text(), "hello");

        buf.insert(5, " world");
        assert_eq!(buf.text(), "hello world");
    }

    #[test]
    fn test_insert_middle() {
        let mut buf = TextBuffer::from_str("helloworld");
        buf.insert(5, " ");
        assert_eq!(buf.text(), "hello world");
    }

    #[test]
    fn test_delete() {
        let mut buf = TextBuffer::from_str("hello world");
        buf.delete(5, 11);
        assert_eq!(buf.text(), "hello");
    }

    #[test]
    fn test_delete_empty_range() {
        let mut buf = TextBuffer::from_str("hello");
        buf.delete(3, 3);
        assert_eq!(buf.text(), "hello");
    }

    #[test]
    fn test_undo_insert() {
        let mut buf = TextBuffer::new();
        buf.insert(0, "hello");
        assert_eq!(buf.text(), "hello");

        assert!(buf.undo());
        assert_eq!(buf.text(), "");
    }

    #[test]
    fn test_undo_delete() {
        let mut buf = TextBuffer::from_str("hello world");
        buf.delete(5, 11);
        assert_eq!(buf.text(), "hello");

        assert!(buf.undo());
        assert_eq!(buf.text(), "hello world");
    }

    #[test]
    fn test_redo() {
        let mut buf = TextBuffer::new();
        buf.insert(0, "hello");
        buf.undo();
        assert_eq!(buf.text(), "");

        assert!(buf.redo());
        assert_eq!(buf.text(), "hello");
    }

    #[test]
    fn test_undo_nothing() {
        let mut buf = TextBuffer::new();
        assert!(!buf.undo());
    }

    #[test]
    fn test_redo_nothing() {
        let mut buf = TextBuffer::new();
        assert!(!buf.redo());
    }

    #[test]
    fn test_new_edit_clears_redo() {
        let mut buf = TextBuffer::new();
        buf.insert(0, "a");
        buf.undo();
        buf.insert(0, "b");
        assert!(!buf.redo()); // redo stack cleared
    }

    #[test]
    fn test_line_access() {
        let buf = TextBuffer::from_str("line1\nline2\nline3");
        assert_eq!(buf.len_lines(), 3);
        assert_eq!(buf.line(0).unwrap(), "line1\n");
        assert_eq!(buf.line(1).unwrap(), "line2\n");
        assert_eq!(buf.line(2).unwrap(), "line3");
        assert!(buf.line(3).is_none());
    }
}
