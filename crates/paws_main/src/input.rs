use std::io::{self, Write};
use std::sync::Arc;

use crossterm::event::{Event, EventStream, KeyCode, KeyModifiers};
use futures::StreamExt;
use paws_api::Environment;

use crate::model::{PawsCommandManager, SlashCommand};
use crate::prompt::PawsPrompt;

pub enum ReadResult {
    Success(String),
    Resize,
    Exit,
}

/// Console implementation for handling user input via command line.
#[derive(Clone)]
pub struct Console {
    command: Arc<PawsCommandManager>,
}

impl Console {
    /// Creates a new instance of `Console`.
    pub fn new(_env: Environment, command: Arc<PawsCommandManager>) -> Self {
        Self { command }
    }
}

impl Console {
    pub async fn prompt(&self, prompt: PawsPrompt) -> anyhow::Result<SlashCommand> {
        // Print the prompt string
        print!("{}", prompt.render_prompt());
        io::stdout().flush()?;

        let mut buffer = String::new();
        let mut reader = EventStream::new();

        loop {
            let event = reader.next().await;

            match event {
                Some(Ok(Event::Key(key_event))) => {
                    match key_event.code {
                        KeyCode::Enter => {
                            println!(); // Move to next line
                            let trimmed = buffer.trim();
                            if trimmed.is_empty() {
                                // Reprint prompt and continue
                                print!("{}", prompt.render_prompt());
                                io::stdout().flush()?;
                                continue;
                            }
                            return self.command.parse(trimmed);
                        }
                        KeyCode::Char('c')
                            if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            return Ok(SlashCommand::Exit);
                        }
                        KeyCode::Char('d')
                            if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                        {
                            return Ok(SlashCommand::Exit);
                        }
                        KeyCode::Char(c) => {
                            buffer.push(c);
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
                    return Ok(SlashCommand::Resize);
                }
                Some(Err(e)) => return Err(e.into()),
                None => break,
                _ => {}
            }
        }

        Ok(SlashCommand::Exit)
    }

    /// Sets the buffer content for the next prompt
    pub fn set_buffer(&self, _content: String) {
        // Not implemented for simple async console yet
    }
}
