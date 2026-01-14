use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Write};
use std::sync::Arc;

use crossterm::cursor::{MoveToColumn, MoveUp};
use crossterm::event::{Event, EventStream, KeyCode, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{Clear, ClearType};
use futures::StreamExt;
use paws_api::Environment;

use crate::model::{PawsCommandManager, SlashCommand};
use crate::prompt::PawsPrompt;

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
            .filter(|line| !line.trim().is_empty())
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

        if let Ok(mut file) = OpenOptions::new()
            .append(true)
            .create(true)
            .open(history_path)
        {
            let _ = writeln!(file, "{}", line);
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
