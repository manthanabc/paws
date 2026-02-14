/// Input buffer state with multi-line support
///
/// Maintains definite state of the input buffer with lines and cursor position.
/// The cursor position is tracked as (row, column) where row is the line number
/// and column is the character position within that line.
#[derive(Debug, Clone)]
pub struct InputState {
    /// Lines of text in the buffer
    lines: Vec<String>,
    /// Cursor position as (row, column)
    cursor: (usize, usize),
}

impl InputState {
    /// Creates a new empty input state
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor: (0, 0),
        }
    }

    /// Creates input state from text
    pub fn from_text(text: &str) -> Self {
        if text.is_empty() {
            return Self::new();
        }

        let lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();
        let lines = if lines.is_empty() {
            vec![String::new()]
        } else {
            lines
        };

        // Set cursor at end
        let last_line = lines.len() - 1;
        let last_col = lines[last_line].chars().count();

        Self {
            lines,
            cursor: (last_line, last_col),
        }
    }

    /// Gets the cursor position as (row, column)
    pub fn cursor(&self) -> (usize, usize) {
        self.cursor
    }

    /// Gets all lines
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Converts the buffer to a single string with newlines
    pub fn to_string(&self) -> String {
        self.lines.join("\n")
    }

    /// Clears the buffer
    pub fn clear(&mut self) {
        self.lines = vec![String::new()];
        self.cursor = (0, 0);
    }

    /// Inserts a character at the cursor position
    pub fn insert_char(&mut self, ch: char) {
        let (row, col) = self.cursor;
        let line = &mut self.lines[row];
        let char_indices: Vec<(usize, char)> = line.char_indices().collect();

        if col >= char_indices.len() {
            // Insert at end
            line.push(ch);
        } else {
            // Insert in middle
            let byte_pos = char_indices[col].0;
            line.insert(byte_pos, ch);
        }

        self.cursor.1 += 1;
    }

    /// Inserts a newline at the cursor position
    pub fn insert_newline(&mut self) {
        let (row, col) = self.cursor;
        let line = &self.lines[row];
        
        // Split current line at cursor
        let char_indices: Vec<(usize, char)> = line.char_indices().collect();
        let (before, after) = if col >= char_indices.len() {
            (line.clone(), String::new())
        } else {
            let byte_pos = char_indices[col].0;
            (line[..byte_pos].to_string(), line[byte_pos..].to_string())
        };

        // Update current line and insert new line
        self.lines[row] = before;
        self.lines.insert(row + 1, after);

        // Move cursor to start of next line
        self.cursor = (row + 1, 0);
    }

    /// Deletes the character before the cursor (backspace)
    pub fn backspace(&mut self) -> bool {
        let (row, col) = self.cursor;

        if col > 0 {
            // Delete character in current line
            let line = &mut self.lines[row];
            let char_indices: Vec<(usize, char)> = line.char_indices().collect();
            let byte_pos = char_indices[col - 1].0;
            let char_end = if col < char_indices.len() {
                char_indices[col].0
            } else {
                line.len()
            };
            line.drain(byte_pos..char_end);
            self.cursor.1 -= 1;
            true
        } else if row > 0 {
            // Merge with previous line
            let current_line = self.lines.remove(row);
            let prev_line_len = self.lines[row - 1].chars().count();
            self.lines[row - 1].push_str(&current_line);
            self.cursor = (row - 1, prev_line_len);
            true
        } else {
            false
        }
    }

    /// Deletes the character at the cursor (delete key)
    pub fn delete(&mut self) -> bool {
        let (row, col) = self.cursor;
        let line = &self.lines[row];
        let char_count = line.chars().count();

        if col < char_count {
            // Delete character in current line
            let line = &mut self.lines[row];
            let char_indices: Vec<(usize, char)> = line.char_indices().collect();
            let byte_pos = char_indices[col].0;
            let char_end = if col + 1 < char_indices.len() {
                char_indices[col + 1].0
            } else {
                line.len()
            };
            line.drain(byte_pos..char_end);
            true
        } else if row < self.lines.len() - 1 {
            // Merge with next line
            let next_line = self.lines.remove(row + 1);
            self.lines[row].push_str(&next_line);
            true
        } else {
            false
        }
    }

    /// Moves cursor left by one character
    pub fn move_left(&mut self) -> bool {
        let (row, col) = self.cursor;

        if col > 0 {
            self.cursor.1 -= 1;
            true
        } else if row > 0 {
            // Move to end of previous line
            let prev_line_len = self.lines[row - 1].chars().count();
            self.cursor = (row - 1, prev_line_len);
            true
        } else {
            false
        }
    }

    /// Moves cursor right by one character
    pub fn move_right(&mut self) -> bool {
        let (row, col) = self.cursor;
        let line_len = self.lines[row].chars().count();

        if col < line_len {
            self.cursor.1 += 1;
            true
        } else if row < self.lines.len() - 1 {
            // Move to start of next line
            self.cursor = (row + 1, 0);
            true
        } else {
            false
        }
    }

    /// Moves cursor to start of current line
    pub fn move_home(&mut self) {
        self.cursor.1 = 0;
    }

    /// Moves cursor to end of current line
    pub fn move_end(&mut self) {
        let row = self.cursor.0;
        let line_len = self.lines[row].chars().count();
        self.cursor.1 = line_len;
    }

    /// Moves cursor up by one line
    pub fn move_up(&mut self) -> bool {
        if self.cursor.0 > 0 {
            let target_row = self.cursor.0 - 1;
            let target_line_len = self.lines[target_row].chars().count();
            self.cursor.0 = target_row;
            self.cursor.1 = self.cursor.1.min(target_line_len);
            true
        } else {
            false
        }
    }

    /// Moves cursor down by one line
    pub fn move_down(&mut self) -> bool {
        if self.cursor.0 < self.lines.len() - 1 {
            let target_row = self.cursor.0 + 1;
            let target_line_len = self.lines[target_row].chars().count();
            self.cursor.0 = target_row;
            self.cursor.1 = self.cursor.1.min(target_line_len);
            true
        } else {
            false
        }
    }

    /// Finds the start of the previous word
    pub fn find_prev_word_start(&self) -> (usize, usize) {
        let (mut row, mut col) = self.cursor;

        // Move back one position first
        if col > 0 {
            col -= 1;
        } else if row > 0 {
            row -= 1;
            col = self.lines[row].chars().count();
            if col > 0 {
                col -= 1;
            }
        } else {
            return (0, 0);
        }

        let chars: Vec<char> = self.lines[row].chars().collect();

        // Skip whitespace
        while chars.get(col).map_or(false, |c| c.is_whitespace()) {
            if col > 0 {
                col -= 1;
            } else if row > 0 {
                row -= 1;
                col = self.lines[row].chars().count();
                if col > 0 {
                    col -= 1;
                }
                let chars: Vec<char> = self.lines[row].chars().collect();
                if chars.is_empty() {
                    break;
                }
            } else {
                return (0, 0);
            }
        }

        // Skip word
        let chars: Vec<char> = self.lines[row].chars().collect();
        while col > 0 && !chars.get(col - 1).map_or(true, |c| c.is_whitespace()) {
            col -= 1;
        }

        (row, col)
    }

    /// Finds the start of the next word
    pub fn find_next_word_start(&self) -> (usize, usize) {
        let (mut row, mut col) = self.cursor;
        let mut chars: Vec<char> = self.lines[row].chars().collect();

        // Skip current word
        while col < chars.len() && !chars[col].is_whitespace() {
            col += 1;
        }

        // Skip whitespace
        loop {
            while col < chars.len() && chars[col].is_whitespace() {
                col += 1;
            }

            if col < chars.len() {
                break;
            } else if row < self.lines.len() - 1 {
                row += 1;
                col = 0;
                chars = self.lines[row].chars().collect();
            } else {
                return (row, self.lines[row].chars().count());
            }
        }

        (row, col)
    }

    /// Moves cursor to previous word start
    pub fn move_word_left(&mut self) {
        let (row, col) = self.find_prev_word_start();
        self.cursor = (row, col);
    }

    /// Moves cursor to next word start
    pub fn move_word_right(&mut self) {
        let (row, col) = self.find_next_word_start();
        self.cursor = (row, col);
    }

    /// Deletes from start of line to cursor
    pub fn delete_to_start(&mut self) {
        let (row, col) = self.cursor;
        if col > 0 {
            let line = &self.lines[row];
            let char_indices: Vec<(usize, char)> = line.char_indices().collect();
            let byte_pos = if col < char_indices.len() {
                char_indices[col].0
            } else {
                line.len()
            };
            self.lines[row] = line[byte_pos..].to_string();
            self.cursor.1 = 0;
        }
    }

    /// Deletes from cursor to end of line
    pub fn delete_to_end(&mut self) {
        let (row, col) = self.cursor;
        let line = &self.lines[row];
        let char_indices: Vec<(usize, char)> = line.char_indices().collect();
        
        if col < char_indices.len() {
            let byte_pos = char_indices[col].0;
            self.lines[row] = line[..byte_pos].to_string();
        }
    }

    /// Deletes the word before the cursor
    pub fn delete_word_back(&mut self) {
        let (target_row, target_col) = self.find_prev_word_start();
        let (current_row, current_col) = self.cursor;

        if target_row == current_row {
            // Delete within same line
            let line = &self.lines[current_row];
            let char_indices: Vec<(usize, char)> = line.char_indices().collect();
            let start_byte = if target_col < char_indices.len() {
                char_indices[target_col].0
            } else {
                line.len()
            };
            let end_byte = if current_col < char_indices.len() {
                char_indices[current_col].0
            } else {
                line.len()
            };
            self.lines[current_row] = format!("{}{}", &line[..start_byte], &line[end_byte..]);
            self.cursor = (current_row, target_col);
        } else {
            // Delete across lines - merge the text
            let current_line = &self.lines[current_row];
            let char_indices: Vec<(usize, char)> = current_line.char_indices().collect();
            let keep_after = if current_col < char_indices.len() {
                &current_line[char_indices[current_col].0..]
            } else {
                ""
            };

            let target_line = &self.lines[target_row];
            let target_char_indices: Vec<(usize, char)> = target_line.char_indices().collect();
            let keep_before = if target_col < target_char_indices.len() {
                &target_line[..target_char_indices[target_col].0]
            } else {
                target_line.as_str()
            };

            // Remove lines in between and merge
            let new_line = format!("{}{}", keep_before, keep_after);
            self.lines.drain(target_row + 1..=current_row);
            self.lines[target_row] = new_line;
            self.cursor = (target_row, target_col);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_state() {
        let state = InputState::new();
        assert_eq!(state.lines(), &["".to_string()]);
        assert_eq!(state.cursor(), (0, 0));
    }

    #[test]
    fn test_from_text() {
        let state = InputState::from_text("hello\nworld");
        assert_eq!(state.lines(), &["hello".to_string(), "world".to_string()]);
        assert_eq!(state.cursor(), (1, 5));
    }

    #[test]
    fn test_insert_char() {
        let mut state = InputState::new();
        state.insert_char('h');
        state.insert_char('i');
        assert_eq!(state.to_string(), "hi");
        assert_eq!(state.cursor(), (0, 2));
    }

    #[test]
    fn test_backspace() {
        let mut state = InputState::from_text("hello");
        assert!(state.backspace());
        assert_eq!(state.to_string(), "hell");
        assert_eq!(state.cursor(), (0, 4));
    }

    #[test]
    fn test_backspace_across_lines() {
        let mut state = InputState::from_text("hello\nworld");
        state.cursor = (1, 0);
        assert!(state.backspace());
        assert_eq!(state.to_string(), "helloworld");
        assert_eq!(state.cursor(), (0, 5));
    }

    #[test]
    fn test_move_left_right() {
        let mut state = InputState::from_text("hello");
        assert!(state.move_left());
        assert_eq!(state.cursor(), (0, 4));
        assert!(state.move_right());
        assert_eq!(state.cursor(), (0, 5));
    }

    #[test]
    fn test_move_left_to_previous_line() {
        let mut state = InputState::from_text("hello\nworld");
        state.cursor = (1, 0);
        assert!(state.move_left());
        assert_eq!(state.cursor(), (0, 5));
    }

    #[test]
    fn test_move_right_to_next_line() {
        let mut state = InputState::from_text("hello\nworld");
        state.cursor = (0, 5);
        assert!(state.move_right());
        assert_eq!(state.cursor(), (1, 0));
    }

    #[test]
    fn test_word_navigation() {
        let mut state = InputState::from_text("hello world test");
        state.move_word_left();
        assert_eq!(state.cursor(), (0, 12));
        state.move_word_left();
        assert_eq!(state.cursor(), (0, 6));
        state.move_word_right();
        assert_eq!(state.cursor(), (0, 12));
    }

    #[test]
    fn test_delete_word_back() {
        let mut state = InputState::from_text("hello world");
        state.delete_word_back();
        assert_eq!(state.to_string(), "hello ");
        assert_eq!(state.cursor(), (0, 6));
    }

    #[test]
    fn test_delete_to_end() {
        let mut state = InputState::from_text("hello world");
        state.cursor = (0, 6);
        state.delete_to_end();
        assert_eq!(state.to_string(), "hello ");
    }

    #[test]
    fn test_delete_to_start() {
        let mut state = InputState::from_text("hello world");
        state.cursor = (0, 6);
        state.delete_to_start();
        assert_eq!(state.to_string(), "world");
        assert_eq!(state.cursor(), (0, 0));
    }
}
