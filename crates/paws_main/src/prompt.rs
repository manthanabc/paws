use std::fmt::Write;
use std::path::PathBuf;
use std::process::Command;

use convert_case::{Case, Casing};
use derive_setters::Setters;
use nu_ansi_term::{Color, Style};
use paws_api::{AgentId, ModelId};

use crate::display_constants::markers;

// Constants
const VERTICLE_LINE: &str = "┃";

/// Very Specialized Prompt for the Agent Chat
#[derive(Clone, Setters)]
#[setters(strip_option, borrow_self)]
pub struct PawsPrompt {
    pub cwd: PathBuf,
    pub agent_id: AgentId,
    pub model: Option<ModelId>,
}

impl PawsPrompt {
    pub fn render_prompt(&self) -> String {
        let mode_style = Style::new().fg(Color::White).bold();
        let branch_style = Style::new().fg(Color::Cyan);

        // Get current directory
        let current_dir = self
            .cwd
            .file_name()
            .and_then(|name| name.to_str())
            .map(String::from)
            .unwrap_or_else(|| markers::EMPTY.to_string());

        let branch_opt = get_git_branch();
        let mut result = String::new();

        write!(
            result,
            "{} ",
            mode_style.paint(self.agent_id.as_str().to_case(Case::UpperSnake)),
        )
        .unwrap();

        // Append model if available
        if let Some(model) = self.model.as_ref() {
            let model_str = model.to_string();
            let formatted_model = model_str
                .split('/')
                .next_back()
                .unwrap_or_else(|| model.as_str());
            write!(result, "[{formatted_model}]").unwrap();
        }

        // Only append branch info if present
        if let Some(branch) = branch_opt
            && branch != current_dir
        {
            write!(result, " Git:{} ", branch_style.paint(branch)).unwrap();
        }

        write!(result, "\n{} ", mode_style.paint(VERTICLE_LINE)).unwrap();

        result
    }
}

/// Gets the current git branch name if available
fn get_git_branch() -> Option<String> {
    // First check if we're in a git repository
    let git_check = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .ok()?;

    if !git_check.status.success() || git_check.stdout.is_empty() {
        return None;
    }

    // If we are in a git repo, get the branch
    let output = Command::new("git")
        .args(["branch", "--show-current"])
        .output()
        .ok()?;

    if output.status.success() {
        String::from_utf8(output.stdout)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    } else {
        None
    }
}
