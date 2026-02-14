use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Write};
use std::sync::Arc;

use crossterm::cursor::{MoveDown, MoveToColumn, MoveUp};
use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{Clear, ClearType};
use futures::StreamExt;
use paws_api::Environment;
use serde::{Deserialize, Serialize};

use crate::input_state::InputState;
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
/// The console uses a state-based input system with proper multi-line support.
///
/// ## Cursor Movement
/// - **Left/Right Arrow**: Move cursor left/right one character (across lines)
/// - **Home** or **Ctrl+A**: Move cursor to start of current line
/// - **End** or **Ctrl+E**: Move cursor to end of current line
///
/// ## Word Navigation
/// - **Ctrl+Left** or **Alt+B**: Move cursor to start of previous word
/// - **Ctrl+Right** or **Alt+F**: Move cursor to start of next word
///
/// ## Editing
/// - **Backspace**: Delete character before cursor
/// - **Delete**: Delete character at cursor
/// - **Ctrl+W**: Delete word before cursor
/// - **Ctrl+K**: Delete from cursor to end of line
/// - **Ctrl+U**: Delete from start of line to cursor
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

    /// Renders the current input state to the terminal
    ///
    /// This function clears the current display and re-renders the prompt
    /// and input buffer, positioning the cursor at the correct location.
    fn render_state(&self, prompt: &PawsPrompt, state: &InputState) -> anyhow::Result<()> {
        let mut stdout = io::stdout();

        // Calculate total lines occupied (prompt + input lines)
        let prompt_text = prompt.render_prompt().replace('\n', "\r\n");
        let prompt_lines = prompt_text.matches("\r\n").count();

        // Clear from start of prompt
        if prompt_lines + state.lines().len() > 0 {
            // Move to start of prompt
            let total_lines = prompt_lines + state.lines().len().saturating_sub(1);
            if total_lines > 0 {
                execute!(stdout, MoveUp(total_lines as u16))?;
            }
        }
        execute!(stdout, MoveToColumn(0))?;
        execute!(stdout, Clear(ClearType::FromCursorDown))?;

        // Render prompt
        print!("{}", prompt_text);

        // Render each line of input
        for (i, line) in state.lines().iter().enumerate() {
            if i > 0 {
                print!("\r\n");
            }
            print!("{}", line);
        }

        // Position cursor at the correct location
        let (cursor_row, cursor_col) = state.cursor();
        let current_row = prompt_lines + state.lines().len() - 1;
        let target_row = prompt_lines + cursor_row;

        // Move vertically to target row
        if target_row < current_row {
            execute!(stdout, MoveUp((current_row - target_row) as u16))?;
        } else if target_row > current_row {
            execute!(stdout, MoveDown((target_row - current_row) as u16))?;
        }

        // Move horizontally to target column
        execute!(stdout, MoveToColumn(cursor_col as u16))?;

        stdout.flush()?;
        Ok(())
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

        // Create input state
        let mut state = InputState::new();
        let mut reader = EventStream::new();

        // History state
        let history = self.load_history();
        let mut history_index = history.len();
        let mut temp_buffer = String::new(); // Store current input when navigating history

        // Paste detection: track if we're in the middle of a paste operation
        let mut paste_detected = false;
        let mut paste_timer = std::time::Instant::now();

        // Initial render
        print!("{}", prompt.render_prompt().replace('\n', "\r\n"));
        io::stdout().flush()?;

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
                                state.insert_newline();
                                self.render_state(&prompt, &state)?;
                                continue;
                            }

                            let text = state.to_string();
                            let trimmed = text.trim();
                            if trimmed.is_empty() {
                                // Reprint prompt and continue
                                self.render_state(&prompt, &state)?;
                                continue;
                            }
                            self.append_history(trimmed);
                            crossterm::terminal::disable_raw_mode()?;
                            return self.command.parse(trimmed);
                        }
                        KeyCode::Char('o')
                            if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            crossterm::terminal::disable_raw_mode()?;
                            return Ok(SlashCommand::Transcript);
                        }
                        KeyCode::Char('a')
                            if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            state.move_home();
                            self.render_state(&prompt, &state)?;
                        }
                        KeyCode::Char('e')
                            if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            state.move_end();
                            self.render_state(&prompt, &state)?;
                        }
                        KeyCode::Char('k')
                            if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            state.delete_to_end();
                            self.render_state(&prompt, &state)?;
                        }
                        KeyCode::Char('u')
                            if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            state.delete_to_start();
                            self.render_state(&prompt, &state)?;
                        }
                        KeyCode::Char('w')
                            if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            state.delete_word_back();
                            self.render_state(&prompt, &state)?;
                        }
                        KeyCode::Char('l')
                            if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            // Clear screen
                            execute!(io::stdout(), Clear(ClearType::All))?;
                            execute!(io::stdout(), MoveToColumn(0))?;
                            execute!(io::stdout(), crossterm::cursor::MoveTo(0, 0))?;
                            self.render_state(&prompt, &state)?;
                        }
                        KeyCode::Left if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                            state.move_word_left();
                            self.render_state(&prompt, &state)?;
                        }
                        KeyCode::Right if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                            state.move_word_right();
                            self.render_state(&prompt, &state)?;
                        }
                        KeyCode::Left if key_event.modifiers.contains(KeyModifiers::ALT) => {
                            state.move_word_left();
                            self.render_state(&prompt, &state)?;
                        }
                        KeyCode::Right if key_event.modifiers.contains(KeyModifiers::ALT) => {
                            state.move_word_right();
                            self.render_state(&prompt, &state)?;
                        }
                        KeyCode::Char('b')
                            if key_event.modifiers.contains(KeyModifiers::ALT) =>
                        {
                            state.move_word_left();
                            self.render_state(&prompt, &state)?;
                        }
                        KeyCode::Char('f')
                            if key_event.modifiers.contains(KeyModifiers::ALT) =>
                        {
                            state.move_word_right();
                            self.render_state(&prompt, &state)?;
                        }
                        KeyCode::Left => {
                            state.move_left();
                            self.render_state(&prompt, &state)?;
                        }
                        KeyCode::Right => {
                            state.move_right();
                            self.render_state(&prompt, &state)?;
                        }
                        KeyCode::Home => {
                            state.move_home();
                            self.render_state(&prompt, &state)?;
                        }
                        KeyCode::End => {
                            state.move_end();
                            self.render_state(&prompt, &state)?;
                        }
                        KeyCode::Up => {
                            if history_index > 0 {
                                if history_index == history.len() {
                                    temp_buffer = state.to_string();
                                }
                                history_index -= 1;
                                state = InputState::from_text(&history[history_index]);
                                self.render_state(&prompt, &state)?;
                            }
                        }
                        KeyCode::Down => {
                            if history_index < history.len() {
                                history_index += 1;
                                if history_index == history.len() {
                                    state = InputState::from_text(&temp_buffer);
                                } else {
                                    state = InputState::from_text(&history[history_index]);
                                }
                                self.render_state(&prompt, &state)?;
                            }
                        }
                        KeyCode::Char('c')
                            if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            state.clear();
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

                            state.insert_char(c);
                            self.render_state(&prompt, &state)?;
                        }
                        KeyCode::Backspace => {
                            state.backspace();
                            self.render_state(&prompt, &state)?;
                        }
                        KeyCode::Delete => {
                            state.delete();
                            self.render_state(&prompt, &state)?;
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
}
