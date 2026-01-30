use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Write};
use std::sync::Arc;

use crossterm::cursor::{MoveLeft, MoveRight, MoveToColumn, MoveUp};
use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{Clear, ClearType};
use futures::StreamExt;
use paws_api::Environment;
use serde::{Deserialize, Serialize};

use crate::model::{PawsCommandManager, SlashCommand};
use crate::prompt::PawsPrompt;

/// A single history entry with timestamp and command text.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct HistoryEntry {
    /// ISO 8601 timestamp when the command was entered.
    timestamp: String,
    /// The command text (may contain newlines).
    text: String,
}

/// Console implementation for handling user input via command line.
///
/// # Terminal-Native Navigation Features
///
/// The console supports the following keyboard shortcuts for a native terminal experience:
///
/// ## Cursor Movement
/// - **Left/Right Arrow**: Move cursor left/right one character
/// - **Home** or **Ctrl+A**: Move cursor to start of line
/// - **End** or **Ctrl+E**: Move cursor to end of line
///
/// ## Word Navigation
/// - **Ctrl+Left** or **Alt+B**: Move cursor to start of previous word
/// - **Ctrl+Right** or **Alt+F**: Move cursor to start of next word
///
/// ## Editing
/// - **Backspace**: Delete character before cursor
/// - **Delete**: Delete character at cursor
/// - **Ctrl+W**: Delete word before cursor
/// - **Ctrl+K**: Kill (delete) from cursor to end of line
/// - **Ctrl+U**: Kill (delete) from start of line to cursor
///
/// ## History
/// - **Up Arrow**: Navigate to previous command in history
/// - **Down Arrow**: Navigate to next command in history
///
/// ## Other
/// - **Ctrl+C**: Clear current input and start fresh
/// - **Ctrl+D**: Exit the console
/// - **Ctrl+L**: Clear screen and redraw prompt
/// - **Ctrl+O**: Open transcript
/// - **Enter**: Submit command
///
#[derive(Clone)]
pub struct Console {
    env: Environment,
    command: Arc<PawsCommandManager>,
}

impl Console {
    /// Creates a new instance of `Console`.
    pub fn new(env: Environment, command: Arc<PawsCommandManager>) -> Self {
        Self { env, command }
    }

    /// Find the start of the previous word
    fn find_prev_word_start(buffer: &str, position: usize) -> usize {
        if position == 0 {
            return 0;
        }
        let chars: Vec<char> = buffer.chars().collect();
        let mut pos = position.saturating_sub(1);
        
        // Skip any whitespace
        while pos > 0 && chars[pos].is_whitespace() {
            pos -= 1;
        }
        
        // Skip the word
        while pos > 0 && !chars[pos - 1].is_whitespace() {
            pos -= 1;
        }
        
        pos
    }
    
    /// Find the start of the next word
    fn find_next_word_start(buffer: &str, position: usize) -> usize {
        let chars: Vec<char> = buffer.chars().collect();
        let len = chars.len();
        if position >= len {
            return len;
        }
        let mut pos = position;
        
        // Skip current word
        while pos < len && !chars[pos].is_whitespace() {
            pos += 1;
        }
        
        // Skip any whitespace
        while pos < len && chars[pos].is_whitespace() {
            pos += 1;
        }
        
        pos
    }

    fn load_history(&self) -> Vec<String> {
        let history_path = self.env.history_path();
        if !history_path.exists() {
            return Vec::new();
        }

        let file = match std::fs::File::open(history_path) {
            Ok(file) => file,
            Err(_) => return Vec::new(),
        };

        let reader = io::BufReader::new(file);
        reader
            .lines()
            .map_while(Result::ok)
            .filter_map(|line| {
                let entry: HistoryEntry = serde_json::from_str(&line).ok()?;
                let trimmed = entry.text.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            })
            .collect()
    }

    fn append_history(&self, line: &str) {
        let line = line.trim();
        if line.is_empty() {
            return;
        }

        // Don't add if it's same as last entry
        if let Some(last) = self.load_history().last()
            && last == line
        {
            return;
        }

        let history_path = self.env.history_path();
        if let Some(parent) = history_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let entry = HistoryEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            text: line.to_string(),
        };

        if let Ok(mut file) = OpenOptions::new()
            .append(true)
            .create(true)
            .open(history_path)
        {
            let _ = writeln!(file, "{}", serde_json::to_string(&entry).unwrap());
        }
    }
}

impl Console {
    pub async fn prompt(&self, prompt: PawsPrompt) -> anyhow::Result<SlashCommand> {
        // Enable raw mode for character-by-character input
        crossterm::terminal::enable_raw_mode()?;

        // Print the prompt string
        // We need to use \r\n for newlines in raw mode
        print!("{}", prompt.render_prompt().replace('\n', "\r\n"));
        io::stdout().flush()?;

        let mut buffer = String::new();
        let mut cursor_position = 0; // Track cursor position within buffer
        let mut reader = EventStream::new();

        // History state
        let history = self.load_history();
        let mut history_index = history.len();
        let mut temp_buffer = String::new(); // Store current input when navigating history

        // Paste detection: track if we're in the middle of a paste operation
        let mut paste_detected = false;
        let mut paste_timer = std::time::Instant::now();

        loop {
            let event = reader.next().await;

            match event {
                Some(Ok(Event::Key(key_event))) => {
                    if key_event.kind == KeyEventKind::Release {
                        continue;
                    }
                    match key_event.code {
                        KeyCode::Enter => {
                            // Ignore Enter if we're in a paste operation (multiple rapid
                            // characters)
                            let now = std::time::Instant::now();
                            let is_paste =
                                paste_detected && now.duration_since(paste_timer).as_millis() < 200;

                            if is_paste {
                                paste_timer = now;
                                buffer.insert(cursor_position, '\n');
                                cursor_position += 1;
                                print!("\r\n");
                                io::stdout().flush()?;
                                continue;
                            }

                            let trimmed = buffer.trim();
                            if trimmed.is_empty() {
                                // Reprint prompt and continue
                                print!("{}", prompt.render_prompt().replace('\n', "\r\n"));
                                io::stdout().flush()?;
                                continue;
                            }
                            self.append_history(trimmed);
                            crossterm::terminal::disable_raw_mode()?;
                            return self.command.parse(trimmed);
                        }
                        KeyCode::Char('o')
                            if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            return Ok(SlashCommand::Transcript);
                        }
                        KeyCode::Char('a')
                            if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            // Move cursor to start of line
                            self.move_cursor_to(&buffer, cursor_position, 0)?;
                            cursor_position = 0;
                        }
                        KeyCode::Char('e')
                            if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            // Move cursor to end of line
                            let end = buffer.chars().count();
                            self.move_cursor_to(&buffer, cursor_position, end)?;
                            cursor_position = end;
                        }
                        KeyCode::Char('k')
                            if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            // Kill to end of line
                            let chars: Vec<char> = buffer.chars().collect();
                            buffer = chars.iter().take(cursor_position).collect();
                            // Clear from cursor to end of line
                            execute!(io::stdout(), Clear(ClearType::UntilNewLine))?;
                        }
                        KeyCode::Char('u')
                            if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            // Kill to start of line
                            let chars: Vec<char> = buffer.chars().collect();
                            let remaining: String = chars.iter().skip(cursor_position).collect();
                            
                            // Move cursor back to start
                            self.move_cursor_to(&buffer, cursor_position, 0)?;
                            
                            // Clear and redraw
                            execute!(io::stdout(), Clear(ClearType::UntilNewLine))?;
                            print!("{}", remaining.replace('\n', "\r\n"));
                            io::stdout().flush()?;
                            
                            // Move cursor back to start
                            self.move_cursor_to(&remaining, remaining.chars().count(), 0)?;
                            
                            buffer = remaining;
                            cursor_position = 0;
                        }
                        KeyCode::Char('w')
                            if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            // Delete word backwards
                            let new_pos = Self::find_prev_word_start(&buffer, cursor_position);
                            if new_pos < cursor_position {
                                let chars: Vec<char> = buffer.chars().collect();
                                let before: String = chars.iter().take(new_pos).collect();
                                let after: String = chars.iter().skip(cursor_position).collect();
                                
                                // Move cursor back
                                self.move_cursor_to(&buffer, cursor_position, new_pos)?;
                                
                                // Clear and redraw
                                execute!(io::stdout(), Clear(ClearType::UntilNewLine))?;
                                print!("{}", after.replace('\n', "\r\n"));
                                io::stdout().flush()?;
                                
                                // Move cursor back to position
                                self.move_cursor_to(&after, after.chars().count(), 0)?;
                                
                                buffer = format!("{}{}", before, after);
                                cursor_position = new_pos;
                            }
                        }
                        KeyCode::Char('l')
                            if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            // Clear screen
                            execute!(io::stdout(), Clear(ClearType::All))?;
                            execute!(io::stdout(), MoveToColumn(0))?;
                            execute!(io::stdout(), crossterm::cursor::MoveTo(0, 0))?;
                            print!("{}", prompt.render_prompt().replace('\n', "\r\n"));
                            print!("{}", buffer.replace('\n', "\r\n"));
                            io::stdout().flush()?;
                            
                            // Move cursor to correct position
                            let total_chars = buffer.chars().count();
                            if cursor_position < total_chars {
                                self.move_cursor_to(&buffer, total_chars, cursor_position)?;
                            }
                        }
                        KeyCode::Left if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                            // Move to start of previous word
                            let new_pos = Self::find_prev_word_start(&buffer, cursor_position);
                            self.move_cursor_to(&buffer, cursor_position, new_pos)?;
                            cursor_position = new_pos;
                        }
                        KeyCode::Right if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                            // Move to start of next word
                            let new_pos = Self::find_next_word_start(&buffer, cursor_position);
                            self.move_cursor_to(&buffer, cursor_position, new_pos)?;
                            cursor_position = new_pos;
                        }
                        KeyCode::Left if key_event.modifiers.contains(KeyModifiers::ALT) => {
                            // Alt+Left: Move to start of previous word
                            let new_pos = Self::find_prev_word_start(&buffer, cursor_position);
                            self.move_cursor_to(&buffer, cursor_position, new_pos)?;
                            cursor_position = new_pos;
                        }
                        KeyCode::Right if key_event.modifiers.contains(KeyModifiers::ALT) => {
                            // Alt+Right: Move to start of next word
                            let new_pos = Self::find_next_word_start(&buffer, cursor_position);
                            self.move_cursor_to(&buffer, cursor_position, new_pos)?;
                            cursor_position = new_pos;
                        }
                        KeyCode::Char('b')
                            if key_event.modifiers.contains(KeyModifiers::ALT) =>
                        {
                            // Alt+b: Move back one word (emacs-style)
                            let new_pos = Self::find_prev_word_start(&buffer, cursor_position);
                            self.move_cursor_to(&buffer, cursor_position, new_pos)?;
                            cursor_position = new_pos;
                        }
                        KeyCode::Char('f')
                            if key_event.modifiers.contains(KeyModifiers::ALT) =>
                        {
                            // Alt+f: Move forward one word (emacs-style)
                            let new_pos = Self::find_next_word_start(&buffer, cursor_position);
                            self.move_cursor_to(&buffer, cursor_position, new_pos)?;
                            cursor_position = new_pos;
                        }
                        KeyCode::Left => {
                            // Move cursor left
                            if cursor_position > 0 {
                                cursor_position -= 1;
                                execute!(io::stdout(), MoveLeft(1))?;
                            }
                        }
                        KeyCode::Right => {
                            // Move cursor right
                            if cursor_position < buffer.chars().count() {
                                cursor_position += 1;
                                execute!(io::stdout(), MoveRight(1))?;
                            }
                        }
                        KeyCode::Home => {
                            // Move to start of line
                            self.move_cursor_to(&buffer, cursor_position, 0)?;
                            cursor_position = 0;
                        }
                        KeyCode::End => {
                            // Move to end of line
                            let end = buffer.chars().count();
                            self.move_cursor_to(&buffer, cursor_position, end)?;
                            cursor_position = end;
                        }
                        KeyCode::Up => {
                            if history_index > 0 {
                                if history_index == history.len() {
                                    temp_buffer = buffer.clone();
                                }
                                let old_buffer = buffer.clone();
                                history_index -= 1;
                                buffer = history[history_index].clone();
                                cursor_position = buffer.chars().count();
                                self.redraw_buffer(&prompt, &buffer, &old_buffer)?;
                            }
                        }
                        KeyCode::Down => {
                            if history_index < history.len() {
                                let old_buffer = buffer.clone();
                                history_index += 1;
                                if history_index == history.len() {
                                    buffer = temp_buffer.clone();
                                } else {
                                    buffer = history[history_index].clone();
                                }
                                cursor_position = buffer.chars().count();
                                self.redraw_buffer(&prompt, &buffer, &old_buffer)?;
                            }
                        }
                        KeyCode::Char('c')
                            if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            buffer.clear();
                            cursor_position = 0;
                            history_index = history.len();
                            print!("\r\n");
                            print!("{}", prompt.render_prompt().replace('\n', "\r\n"));
                            io::stdout().flush()?;
                            continue;
                        }
                        KeyCode::Char('d')
                            if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            crossterm::terminal::disable_raw_mode()?;
                            return Ok(SlashCommand::Exit);
                        }
                        KeyCode::Char(c) => {
                            let now = std::time::Instant::now();
                            let elapsed = now.duration_since(paste_timer).as_millis();
                            // Detect paste: if characters arrive very rapidly (<10ms apart)
                            paste_detected = elapsed < 10;
                            paste_timer = now;

                            // Insert char at cursor position
                            let chars: Vec<char> = buffer.chars().collect();
                            let before: String = chars.iter().take(cursor_position).collect();
                            let after: String = chars.iter().skip(cursor_position).collect();
                            buffer = format!("{}{}{}", before, c, after);
                            
                            // Print character and redraw rest of line
                            print!("{}", c);
                            if !after.is_empty() {
                                print!("{}", after.replace('\n', "\r\n"));
                                // Move cursor back to position after inserted char
                                self.move_cursor_to(&after, after.chars().count(), 0)?;
                            }
                            io::stdout().flush()?;
                            cursor_position += 1;
                        }
                        KeyCode::Backspace => {
                            if cursor_position > 0 {
                                let chars: Vec<char> = buffer.chars().collect();
                                let before: String = chars.iter().take(cursor_position - 1).collect();
                                let after: String = chars.iter().skip(cursor_position).collect();
                                buffer = format!("{}{}", before, after);
                                cursor_position -= 1;
                                
                                // Move back one, clear to end, redraw, move back
                                execute!(io::stdout(), MoveLeft(1))?;
                                execute!(io::stdout(), Clear(ClearType::UntilNewLine))?;
                                print!("{}", after.replace('\n', "\r\n"));
                                
                                // Move cursor back to position
                                self.move_cursor_to(&after, after.chars().count(), 0)?;
                                io::stdout().flush()?;
                            }
                        }
                        KeyCode::Delete => {
                            if cursor_position < buffer.chars().count() {
                                let chars: Vec<char> = buffer.chars().collect();
                                let before: String = chars.iter().take(cursor_position).collect();
                                let after: String = chars.iter().skip(cursor_position + 1).collect();
                                buffer = format!("{}{}", before, after);
                                
                                // Clear to end of line and redraw
                                execute!(io::stdout(), Clear(ClearType::UntilNewLine))?;
                                print!("{}", after.replace('\n', "\r\n"));
                                
                                // Move cursor back to position
                                self.move_cursor_to(&after, after.chars().count(), 0)?;
                                io::stdout().flush()?;
                            }
                        }
                        _ => {}
                    }
                }
                Some(Ok(Event::Resize(_, _))) => {
                    crossterm::terminal::disable_raw_mode()?;
                    return Ok(SlashCommand::Resize);
                }
                Some(Err(e)) => {
                    crossterm::terminal::disable_raw_mode()?;
                    return Err(e.into());
                }
                None => break,
                _ => {}
            }
        }

        crossterm::terminal::disable_raw_mode()?;
        Ok(SlashCommand::Exit)
    }

    fn redraw_buffer(
        &self,
        prompt: &PawsPrompt,
        buffer: &str,
        old_buffer: &str,
    ) -> anyhow::Result<()> {
        let mut stdout = io::stdout();

        let rendered_prompt = prompt.render_prompt().replace('\n', "\r\n");
        let prompt_lines = rendered_prompt.matches("\r\n").count();
        let buffer_lines = old_buffer.matches('\n').count();
        let total_lines = prompt_lines + buffer_lines;

        // Move cursor up to first line of prompt
        if total_lines > 0 {
            execute!(stdout, MoveUp(total_lines as u16))?;
        }

        // Now move to beginning of line and clear everything below
        execute!(stdout, MoveToColumn(0))?;
        execute!(stdout, Clear(ClearType::FromCursorDown))?;

        // Re-render prompt + buffer
        print!("{}", rendered_prompt);
        print!("{}", buffer.replace('\n', "\r\n"));

        stdout.flush()?;
        Ok(())
    }

    fn move_cursor_to(
        &self,
        buffer: &str,
        from_pos: usize,
        to_pos: usize,
    ) -> anyhow::Result<()> {
        if from_pos == to_pos {
            return Ok(());
        }
        
        let mut stdout = io::stdout();
        let chars: Vec<char> = buffer.chars().collect();
        
        if to_pos < from_pos {
            // Move left - need to handle newlines when going backwards
            for i in (to_pos..from_pos).rev() {
                if i < chars.len() && chars[i] == '\n' {
                    // Moving back across a newline
                    execute!(stdout, crossterm::cursor::MoveUp(1))?;
                    // Find the length of the previous line
                    let mut line_len = 0;
                    for j in (0..i).rev() {
                        if chars[j] == '\n' {
                            break;
                        }
                        line_len += 1;
                    }
                    execute!(stdout, crossterm::cursor::MoveToColumn(line_len as u16))?;
                } else {
                    execute!(stdout, MoveLeft(1))?;
                }
            }
        } else {
            // Move right
            for i in from_pos..to_pos {
                if i < chars.len() && chars[i] == '\n' {
                    // Handle newline
                    execute!(stdout, MoveToColumn(0))?;
                    execute!(stdout, crossterm::cursor::MoveDown(1))?;
                } else {
                    execute!(stdout, MoveRight(1))?;
                }
            }
        }
        stdout.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use fake::{Fake, Faker};
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_history_entry_serialization() {
        let entry = HistoryEntry {
            timestamp: "2024-01-14T12:00:00Z".to_string(),
            text: "test command".to_string(),
        };

        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: HistoryEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(entry.timestamp, deserialized.timestamp);
        assert_eq!(entry.text, deserialized.text);
    }

    #[test]
    fn test_history_entry_with_newlines() {
        let entry = HistoryEntry {
            timestamp: "2024-01-14T12:00:00Z".to_string(),
            text: "line 1\nline 2\nline 3".to_string(),
        };

        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: HistoryEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(entry.text, deserialized.text);
        assert_eq!(deserialized.text.lines().count(), 3);
    }

    #[test]
    fn test_load_and_save_history() {
        let temp_dir = TempDir::new().unwrap();
        let mut env: Environment = Faker.fake();
        env.base_path = temp_dir.path().to_path_buf();
        env.custom_history_path = None;

        let command = Arc::new(PawsCommandManager::default());
        let console = Console::new(env, command);

        let history_path = temp_dir.path().join(".paws_history");

        // Write a multi-line entry
        let entry1 = HistoryEntry {
            timestamp: "2024-01-14T12:00:00Z".to_string(),
            text: "first command\nsecond line".to_string(),
        };

        let entry2 = HistoryEntry {
            timestamp: "2024-01-14T12:01:00Z".to_string(),
            text: "simple command".to_string(),
        };

        fs::write(
            &history_path,
            format!(
                "{}\n{}\n",
                serde_json::to_string(&entry1).unwrap(),
                serde_json::to_string(&entry2).unwrap()
            ),
        )
        .unwrap();

        let history = console.load_history();

        assert_eq!(history.len(), 2);
        assert_eq!(history[0], "first command\nsecond line");
        assert_eq!(history[1], "simple command");
    }

    #[test]
    fn test_append_history_creates_json() {
        let temp_dir = TempDir::new().unwrap();
        let mut env: Environment = Faker.fake();
        env.base_path = temp_dir.path().to_path_buf();
        env.custom_history_path = None;

        let command = Arc::new(PawsCommandManager::default());
        let console = Console::new(env, command);

        console.append_history("single line");
        console.append_history("line 1\nline 2");

        let history_path = temp_dir.path().join(".paws_history");
        let content = fs::read_to_string(&history_path).unwrap();

        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);

        let entry1: HistoryEntry = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(entry1.text, "single line");
        assert!(chrono::DateTime::parse_from_rfc3339(&entry1.timestamp).is_ok());

        let entry2: HistoryEntry = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(entry2.text, "line 1\nline 2");
    }

    #[test]
    fn test_duplicate_prevention() {
        let temp_dir = TempDir::new().unwrap();
        let mut env: Environment = Faker.fake();
        env.base_path = temp_dir.path().to_path_buf();
        env.custom_history_path = None;

        let command = Arc::new(PawsCommandManager::default());
        let console = Console::new(env, command);

        console.append_history("test command");
        console.append_history("test command"); // Duplicate
        console.append_history("test command"); // Another duplicate

        let history = console.load_history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0], "test command");
    }

    #[test]
    fn test_empty_commands_filtered() {
        let temp_dir = TempDir::new().unwrap();
        let mut env: Environment = Faker.fake();
        env.base_path = temp_dir.path().to_path_buf();
        env.custom_history_path = None;

        let command = Arc::new(PawsCommandManager::default());
        let console = Console::new(env, command);

        console.append_history("");
        console.append_history("   ");
        console.append_history("valid command");

        let history = console.load_history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0], "valid command");
    }

    #[test]
    fn test_multiline_history_newlines_preserved() {
        let temp_dir = TempDir::new().unwrap();
        let mut env: Environment = Faker.fake();
        env.base_path = temp_dir.path().to_path_buf();
        env.custom_history_path = None;

        let command = Arc::new(PawsCommandManager::default());
        let console = Console::new(env, command);

        let multiline = "line 1\nline 2\nline 3";
        console.append_history(multiline);

        let history = console.load_history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0], multiline);
        assert_eq!(history[0].lines().count(), 3);
    }

    #[test]
    fn test_find_prev_word_start() {
        let buffer = "hello world test";
        
        // From middle of "test"
        assert_eq!(Console::find_prev_word_start(buffer, 14), 12);
        
        // From start of "test"
        assert_eq!(Console::find_prev_word_start(buffer, 12), 6);
        
        // From middle of "world"
        assert_eq!(Console::find_prev_word_start(buffer, 8), 6);
        
        // From start of buffer
        assert_eq!(Console::find_prev_word_start(buffer, 0), 0);
        
        // From position 1
        assert_eq!(Console::find_prev_word_start(buffer, 1), 0);
    }

    #[test]
    fn test_find_prev_word_start_with_whitespace() {
        let buffer = "hello  world   test";
        
        // From "test" through whitespace
        assert_eq!(Console::find_prev_word_start(buffer, 19), 15);
        
        // From whitespace before "test"
        assert_eq!(Console::find_prev_word_start(buffer, 15), 7);
    }

    #[test]
    fn test_find_next_word_start() {
        let buffer = "hello world test";
        
        // From start
        assert_eq!(Console::find_next_word_start(buffer, 0), 6);
        
        // From middle of "hello"
        assert_eq!(Console::find_next_word_start(buffer, 2), 6);
        
        // From start of "world"
        assert_eq!(Console::find_next_word_start(buffer, 6), 12);
        
        // From middle of "world"
        assert_eq!(Console::find_next_word_start(buffer, 8), 12);
        
        // From end
        assert_eq!(Console::find_next_word_start(buffer, 16), 16);
    }

    #[test]
    fn test_find_next_word_start_with_whitespace() {
        let buffer = "hello  world   test";
        
        // From start of "hello" - skip "hello" and whitespace to start of "world"
        assert_eq!(Console::find_next_word_start(buffer, 0), 7);
        
        // From start of "world" - skip "world" and whitespace to start of "test"  
        assert_eq!(Console::find_next_word_start(buffer, 7), 15);
    }
}
