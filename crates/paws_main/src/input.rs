use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Write};
use std::sync::Arc;

use crossterm::cursor::{MoveToColumn, MoveUp};
use crossterm::event::{Event, EventStream, KeyCode, KeyModifiers};
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
                    match key_event.code {
                        KeyCode::Enter => {
                            // Ignore Enter if we're in a paste operation (multiple rapid
                            // characters)
                            let now = std::time::Instant::now();
                            let is_paste =
                                paste_detected && now.duration_since(paste_timer).as_millis() < 200;

                            if is_paste {
                                paste_timer = now;
                                buffer.push('\n');
                                println!("\r");
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
                        KeyCode::Up => {
                            if history_index > 0 {
                                if history_index == history.len() {
                                    temp_buffer = buffer.clone();
                                }
                                let old_buffer = buffer.clone();
                                history_index -= 1;
                                buffer = history[history_index].clone();
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
                                self.redraw_buffer(&prompt, &buffer, &old_buffer)?;
                            }
                        }
                        KeyCode::Char('c')
                            if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            buffer.clear();
                            history_index = history.len();
                            println!("\r");
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

                            // Store the char
                            buffer.push(c);

                            // Push to stdout
                            print!("{}", c);
                            io::stdout().flush()?;
                        }
                        KeyCode::Backspace => {
                            if !buffer.is_empty() {
                                buffer.pop();
                                // Move back, print space, move back
                                print!("\x08 \x08");
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
        print!("{}", buffer);

        stdout.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fake::{Fake, Faker};
    use std::fs;
    use tempfile::TempDir;

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
}
