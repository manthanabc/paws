use std::sync::{Arc, Mutex};

use paws_api::Environment;

use crate::editor::{PawsEditor, ReadResult};
use crate::model::{PawsCommandManager, SlashCommand};
use crate::prompt::PawsPrompt;

/// Console implementation for handling user input via command line.
pub struct Console {
    command: Arc<PawsCommandManager>,
    editor: Mutex<PawsEditor>,
}

impl Console {
    /// Creates a new instance of `Console`.
    pub fn new(env: Environment, command: Arc<PawsCommandManager>) -> Self {
        let editor = Mutex::new(PawsEditor::new(env, command.clone()));
        Self { command, editor }
    }
}

impl Console {
    pub async fn prompt(&self, prompt: PawsPrompt) -> anyhow::Result<SlashCommand> {
        loop {
            let mut paws_editor = self.editor.lock().unwrap();
            let user_input = paws_editor.prompt(&prompt)?;
            drop(paws_editor);
            match user_input {
                ReadResult::Continue => continue,
                ReadResult::Exit => return Ok(SlashCommand::Exit),
                ReadResult::Empty => continue,
                ReadResult::Success(text) => {
                    return self.command.parse(&text);
                }
            }
        }
    }

    /// Sets the buffer content for the next prompt
    pub fn set_buffer(&self, content: String) {
        let mut editor = self.editor.lock().unwrap();
        editor.set_buffer(content);
    }
}
