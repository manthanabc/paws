use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use chrono::Utc;
use colored::Colorize;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, EventStream, KeyCode, KeyModifiers,
    MouseEventKind,
};
use crossterm::{cursor, execute, terminal};
use paws_api::{AgentId, Conversation, Environment, TextMessage, TokenCount, ToolCatalog};
use paws_app::fmt::content::FormatContent;
use paws_common::display::md::render::{MarkdownRenderer, crossterm as md_crossterm};
use paws_domain::{ChatResponseContent, ContextMessage, Role, ToolValue};
use tokio_stream::StreamExt;

use crate::info::Info;
use crate::prompt::PawsPrompt;
use crate::title_display::TitleDisplayExt;

#[derive(Debug)]
struct DrawViewportArgs<'a> {
    lines: &'a [String],
    offset: usize,
    height: usize,
    search_query: Option<&'a str>,
    match_info: Option<(usize, usize)>,
    is_searching: bool,
    show_thinking: bool,
    show_tool_outputs: bool,
}

pub struct TranscriptRenderer {
    cwd: PathBuf,
    environment: Environment,
    show_thinking: bool,
    show_tool_outputs: bool,
}

impl TranscriptRenderer {
    pub fn new(cwd: PathBuf, environment: Environment) -> Self {
        Self {
            cwd,
            environment,
            show_thinking: true,
            show_tool_outputs: false,
        }
    }

    /// Sets whether to show thinking/reasoning in the transcript
    #[allow(dead_code)]
    pub fn show_thinking(mut self, show: bool) -> Self {
        self.show_thinking = show;
        self
    }

    fn format_user_message(&self, message: &TextMessage) -> Vec<String> {
        let content = &message.content;
        let content_to_show = message
            .raw_content
            .as_ref()
            .and_then(|v| v.as_user_prompt())
            .map(|p| p.as_str().to_string())
            .unwrap_or_else(|| content.clone());

        let paws_prompt = PawsPrompt {
            cwd: self.cwd.clone(),
            agent_id: AgentId::default(),
            model: message.model.clone(),
            git_branch: None, // We could pass this in if needed, or leave None for transcript
        };
        let full_prompt = paws_prompt.render_prompt();

        let mut lines = Vec::new();
        lines.push("".to_string());
        if let Some((header, prefix)) = full_prompt.rsplit_once('\n') {
            lines.push(header.to_string());
            for line in content_to_show.lines() {
                lines.push(format!("{}{}", prefix, line));
            }
        } else {
            lines.push(full_prompt);
            for line in content_to_show.lines() {
                lines.push(format!("{} {}", "┃".white().bold(), line));
            }
        }
        lines
    }

    /// Renders a header for the transcript view showing conversation metadata
    pub fn render_header(&self, conversation: &Conversation) -> Vec<String> {
        let mut info = Info::new();

        info = info.add_title("TRANSCRIPT");

        // Conversation info
        let id = conversation.id.to_string();
        let id_display = format!("{}...", &id[..id.len().min(8)]);
        info = info.add_key_value("ID", id_display);

        if let Some(title) = &conversation.title {
            info = info.add_key_value("Title", title);
        }

        let created_at = conversation.metadata.created_at;
        let formatted = created_at.format("%Y-%m-%d %H:%M:%S UTC").to_string();
        info = info.add_key_value("Created", formatted);

        if let Some(updated_at) = conversation.metadata.updated_at {
            let formatted = updated_at.format("%Y-%m-%d %H:%M:%S UTC").to_string();
            info = info.add_key_value("Updated", formatted);
        }

        // Message count
        let msg_count = conversation
            .context
            .as_ref()
            .map(|c| c.messages.len())
            .unwrap_or(0);
        info = info.add_key_value("Messages", msg_count.to_string());

        // Usage information
        if let Some(context) = &conversation.context
            && let Some(usage) = context.accumulate_usage()
        {
            let total_tokens = match usage.total_tokens {
                TokenCount::Actual(count) => count,
                TokenCount::Approx(count) => count,
            };
            let prompt_tokens = match usage.prompt_tokens {
                TokenCount::Actual(count) => count,
                TokenCount::Approx(count) => count,
            };
            let completion_tokens = match usage.completion_tokens {
                TokenCount::Actual(count) => count,
                TokenCount::Approx(count) => count,
            };
            let cached_tokens = match usage.cached_tokens {
                TokenCount::Actual(count) => count,
                TokenCount::Approx(count) => count,
            };

            info = info.add_key_value(
                "Tokens",
                format!(
                    "{} total ({} input + {} output, {} cached)",
                    total_tokens, prompt_tokens, completion_tokens, cached_tokens
                ),
            );

            if let Some(cost) = usage.cost {
                info = info.add_key_value("Cost", format!("${:.4}", cost));
            }
        }

        // Convert Info to string and split into lines
        let info_string = info.to_string();
        info_string.lines().map(|s| s.to_string()).collect()
    }

    /// Renders the transcript of the conversation to a list of lines
    pub fn render_content(&self, conversation: &Conversation) -> Vec<String> {
        self.render_content_with_options(conversation, self.show_thinking, self.show_tool_outputs)
    }

    /// Renders the transcript of the conversation to a list of lines
    /// with the specified thinking and tool outputs visibility settings
    fn render_content_with_options(
        &self,
        conversation: &Conversation,
        show_thinking: bool,
        show_tool_outputs: bool,
    ) -> Vec<String> {
        let mut lines = Vec::new();
        let Some(context) = conversation.context.as_ref() else {
            return lines;
        };

        use md_crossterm::style::Attribute;
        let renderer = MarkdownRenderer::default();

        for message in &context.messages {
            match &**message {
                ContextMessage::Text(text_message) => {
                    match text_message.role {
                        Role::User => {
                            let user_lines = self.format_user_message(text_message);
                            lines.extend(user_lines);
                        }
                        Role::Assistant => {
                            // Show full thinking/reasoning if enabled
                            if show_thinking
                                && let Some(reasoning) = &text_message.reasoning_details
                            {
                                for detail in reasoning.iter() {
                                    // Try text field first, then decode data field if it's base64
                                    let decoded_data = detail.data.as_ref().and_then(|data| {
                                        STANDARD
                                            .decode(data)
                                            .ok()
                                            .and_then(|bytes| String::from_utf8(bytes).ok())
                                    });

                                    let reasoning_text =
                                        detail.text.as_ref().or(decoded_data.as_ref());

                                    if let Some(text) = reasoning_text {
                                        // Show reasoning type header if available
                                        if let Some(type_of) = &detail.type_of {
                                            lines.push(format!(
                                                "{}: {}",
                                                "Reasoning".dimmed(),
                                                type_of.dimmed()
                                            ));
                                        }
                                        let rendered = renderer.render(text, Some(Attribute::Dim));
                                        for line in rendered.lines() {
                                            lines.push(line.to_string());
                                        }
                                    } else if detail.data.is_some() {
                                        // Show placeholder for encrypted/undecodable reasoning
                                        if let Some(type_of) = &detail.type_of {
                                            lines.push(format!(
                                                "{}: {} (encrypted, {} bytes)",
                                                "Reasoning".dimmed(),
                                                type_of.dimmed(),
                                                detail.data.as_ref().map(|d| d.len()).unwrap_or(0)
                                            ));
                                        }
                                    }
                                }
                            }

                            if !text_message.content.is_empty() {
                                let rendered = renderer.render(&text_message.content, None);
                                for line in rendered.lines() {
                                    lines.push(line.to_string());
                                }
                            }

                            // Show tool calls using the same formatting as normal mode
                            if let Some(calls) = &text_message.tool_calls {
                                for call in calls {
                                    if let Ok(catalog) = ToolCatalog::try_from(call.clone())
                                        && let Some(content) = catalog.to_content(&self.environment)
                                    {
                                        match content {
                                            ChatResponseContent::Title(title) => {
                                                lines.push(title.display().to_string());
                                            }
                                            ChatResponseContent::PlainText(text)
                                            | ChatResponseContent::Markdown(text) => {
                                                for line in text.lines() {
                                                    lines.push(line.to_string());
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                ContextMessage::Tool(tool_result) => {
                    // Render tool results to show tool outputs in place (only if enabled)
                    if show_tool_outputs {
                        lines.push(format!("{}: {}", "Tool Result".dimmed(), tool_result.name));
                        for value in &tool_result.output.values {
                            match value {
                                ToolValue::Text(text) => {
                                    for line in text.lines() {
                                        lines.push(line.dimmed().to_string());
                                    }
                                }
                                ToolValue::Image(image) => {
                                    lines.push(
                                        format!("[Image: {}]", image.mime_type())
                                            .dimmed()
                                            .to_string(),
                                    );
                                }
                                ToolValue::AI { value, conversation_id } => {
                                    lines.push(
                                        format!("AI Result ({conversation_id}):")
                                            .dimmed()
                                            .to_string(),
                                    );
                                    for line in value.lines() {
                                        lines.push(line.dimmed().to_string());
                                    }
                                }
                                ToolValue::Empty => {}
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        lines
    }

    /// Renders a summary for the transcript view showing additional info
    pub fn render_summary(&self, conversation: &Conversation) -> Vec<String> {
        let mut info = Info::new();

        info = info.add_title("SUMMARY");

        // Add task summary from conversation
        let mut user_messages = conversation
            .context
            .iter()
            .flat_map(|ctx| ctx.messages.iter())
            .filter(|message| message.has_role(Role::User));

        let task = user_messages.next();

        if let Some(task) = task
            && let Some(task) = crate::info::format_user_message(task)
        {
            info = info.add_title("TASKS");
            info = info.add_value(task);

            for feedback in user_messages {
                if let Some(feedback) = crate::info::format_user_message(feedback) {
                    info = info.add_value(feedback);
                }
            }
        }

        // File operations from metrics
        if !conversation.metrics.file_operations.is_empty() {
            info = info.add_title("FILE OPERATIONS");

            // Collect and sort operations
            let mut operations: Vec<_> = conversation.metrics.file_operations.iter().collect();
            operations.sort_by(|(path_a, op_a), (path_b, op_b)| {
                let get_priority = |op: &paws_domain::FileOperation| match op.tool {
                    paws_domain::ToolKind::Remove => 0,
                    paws_domain::ToolKind::Patch => 1,
                    paws_domain::ToolKind::Write => 2,
                    paws_domain::ToolKind::Undo => 3,
                    paws_domain::ToolKind::Read => 4,
                    _ => 5,
                };

                let priority_a = get_priority(op_a);
                let priority_b = get_priority(op_b);

                if priority_a != priority_b {
                    priority_a.cmp(&priority_b)
                } else {
                    path_a.cmp(path_b)
                }
            });

            for (path, operation) in operations {
                let (op_letter, op_color) = match operation.tool {
                    paws_domain::ToolKind::Write => ('w', colored::Color::Green),
                    paws_domain::ToolKind::Patch => ('p', colored::Color::Yellow),
                    paws_domain::ToolKind::Remove => ('d', colored::Color::Red),
                    paws_domain::ToolKind::Undo => ('u', colored::Color::Blue),
                    paws_domain::ToolKind::Read => ('r', colored::Color::Cyan),
                    _ => ('?', colored::Color::White),
                };

                let short_path = std::path::Path::new(path)
                    .components()
                    .rev()
                    .take(2)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<PathBuf>()
                    .display()
                    .to_string();

                let line_changes = if operation.lines_added > 0 || operation.lines_removed > 0 {
                    format!(" +{}/-{}", operation.lines_added, operation.lines_removed)
                } else {
                    String::new()
                };

                let op_display = format!("[{}]", op_letter).color(op_color).bold();
                let lines_display = if !line_changes.is_empty() {
                    format!("{:<12}", line_changes).dimmed()
                } else {
                    format!("{:<12}", "").dimmed()
                };

                info = info.add_value(format!("  {} {}{}", op_display, lines_display, short_path));
            }
        }

        // Session duration
        if conversation.metrics.started_at.is_some() {
            let now = Utc::now();
            if let Some(duration) = conversation.metrics.duration(now) {
                let secs = duration.as_secs();
                let mins = secs / 60;
                let hours = mins / 60;
                let duration_str = if hours > 0 {
                    format!("{}h {}m", hours, mins % 60)
                } else if mins > 0 {
                    format!("{}m {}s", mins, secs % 60)
                } else {
                    format!("{}s", secs)
                };
                info = info.add_key_value("Session Duration", duration_str);
            }
        }

        let info_string = info.to_string();
        info_string.lines().map(|s| s.to_string()).collect()
    }

    fn draw_viewport(&self, args: DrawViewportArgs<'_>) -> Result<()> {
        let mut stdout = std::io::stdout();
        execute!(
            stdout,
            terminal::Clear(terminal::ClearType::All),
            cursor::MoveTo(0, 0)
        )?;

        let content_height = args.height.saturating_sub(1);
        let end = (args.offset + content_height).min(args.lines.len());
        for line in args.lines.iter().take(end).skip(args.offset) {
            write!(stdout, "{}\r\n", line)?;
        }

        execute!(stdout, cursor::MoveTo(0, (args.height - 1) as u16))?;

        let width = 80;
        let _separator = "─".repeat(width);

        let status = if args.is_searching {
            format!("/{}", args.search_query.unwrap_or(""))
        } else if let Some(query) = args.search_query {
            if let Some((current, total)) = args.match_info {
                format!("/{} [{}/{}] (n/N: next/prev)", query, current, total)
            } else {
                format!("/{} [0/0]", query)
            }
        } else {
            format!(
                " {}/{} | j/k: scroll | /: search | t: {} | o: {} | e: edit | q/Esc: exit ",
                args.offset + 1,
                args.lines.len(),
                if args.show_thinking {
                    "hide thinking"
                } else {
                    "show thinking"
                },
                if args.show_tool_outputs {
                    "hide tool outputs"
                } else {
                    "show tool outputs"
                }
            )
        };

        let footer_line = if status.len() > width {
            status[..width].to_string()
        } else {
            let padding = width.saturating_sub(status.len() + 1);
            format!("└{}{}", "─".repeat(padding), status.dimmed())
        };

        write!(stdout, "{}", footer_line)?;
        stdout.flush()?;
        Ok(())
    }

    /// Opens the transcript in the system editor (read-only mode)
    async fn open_in_editor(&self, lines: &[String]) -> Result<()> {
        use std::io::Write;
        use std::process::Command;

        use tempfile::NamedTempFile;

        // Create temp file
        let mut temp_file = NamedTempFile::new()?;
        for line in lines {
            writeln!(temp_file, "{}", console::strip_ansi_codes(line))?;
        }

        let path = temp_file.path().to_path_buf();
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());

        // Leave alternate screen to run editor
        execute!(
            std::io::stdout(),
            DisableMouseCapture,
            terminal::LeaveAlternateScreen,
            cursor::Show
        )?;
        terminal::disable_raw_mode()?;

        let status = Command::new(&editor)
            .arg(&path)
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::null())
            .status();

        terminal::enable_raw_mode()?;

        // Restore alternate screen
        execute!(
            std::io::stdout(),
            terminal::EnterAlternateScreen,
            EnableMouseCapture,
            cursor::Hide
        )?;

        if let Err(e) = status {
            // Maybe show an error message briefly?
            eprintln!("Failed to open editor: {}", e);
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }

        Ok(())
    }

    /// Prints the transcript to stdout (non-interactive)
    pub fn print(&self, conversation: &Conversation) -> Result<()> {
        let mut lines = self.render_content(conversation);
        let header_lines = self.render_header(conversation);
        lines.splice(0..0, header_lines);

        lines.push(String::new());
        let summary_lines = self.render_summary(conversation);
        lines.extend(summary_lines);

        let mut stdout = std::io::stdout();
        for line in lines {
            writeln!(stdout, "{}", line)?;
        }
        Ok(())
    }

    /// Runs the interactive transcript view
    pub async fn run(&mut self, conversation: Conversation) -> Result<()> {
        execute!(
            std::io::stdout(),
            terminal::EnterAlternateScreen,
            EnableMouseCapture,
            cursor::Hide
        )?;

        let mut lines = self.render_content(&conversation);

        let header_lines = self.render_header(&conversation);
        lines.splice(0..0, header_lines);

        lines.push(String::new());
        let summary_lines = self.render_summary(&conversation);
        lines.extend(summary_lines);

        let mut scroll_offset = 0;
        let (_width, mut height) = terminal::size()?;

        // Thinking visibility state
        let mut show_thinking = self.show_thinking;

        // Tool outputs visibility state
        let mut show_tool_outputs = self.show_tool_outputs;

        // Search state
        let mut is_searching = false;
        let mut search_query = String::new();
        let mut matches: Vec<usize> = Vec::new();
        let mut current_match_idx: Option<usize> = None;

        let update_matches = |query: &str, lines: &[String]| -> Vec<usize> {
            if query.is_empty() {
                return Vec::new();
            }
            let query_lower = query.to_lowercase();
            lines
                .iter()
                .enumerate()
                .filter_map(|(i, line)| {
                    if console::strip_ansi_codes(line)
                        .to_lowercase()
                        .contains(&query_lower)
                    {
                        Some(i)
                    } else {
                        None
                    }
                })
                .collect()
        };

        // Initial draw
        self.draw_viewport(DrawViewportArgs {
            lines: &lines,
            offset: scroll_offset,
            height: height as usize,
            search_query: if !search_query.is_empty() || is_searching {
                Some(&search_query)
            } else {
                None
            },
            match_info: None,
            is_searching,
            show_thinking,
            show_tool_outputs,
        })?;

        let mut reader = EventStream::new();
        loop {
            let event = reader.next().await;
            match event {
                Some(Ok(event::Event::Key(key_event))) => {
                    if is_searching {
                        match key_event.code {
                            KeyCode::Esc => {
                                is_searching = false;
                            }
                            KeyCode::Enter => {
                                is_searching = false;
                                matches = update_matches(&search_query, &lines);
                                if !matches.is_empty() {
                                    current_match_idx = Some(0);
                                    scroll_offset = matches[0].saturating_sub(2);
                                } else {
                                    current_match_idx = None;
                                }
                            }
                            KeyCode::Backspace => {
                                search_query.pop();
                            }
                            KeyCode::Char(c) => {
                                search_query.push(c);
                            }
                            _ => {}
                        }
                    } else {
                        if key_event.code == KeyCode::Esc
                            || key_event.code == KeyCode::Char('q')
                            || (key_event.code == KeyCode::Char('o')
                                && key_event.modifiers.contains(KeyModifiers::CONTROL))
                            || (key_event.code == KeyCode::Char('c')
                                && key_event.modifiers.contains(KeyModifiers::CONTROL))
                        {
                            break;
                        }

                        match key_event.code {
                            KeyCode::Char('/') => {
                                is_searching = true;
                                search_query.clear();
                                matches.clear();
                                current_match_idx = None;
                            }
                            KeyCode::Char('t') => {
                                // Toggle thinking visibility
                                show_thinking = !show_thinking;
                                // Re-render content with new thinking visibility
                                lines = self.render_content_with_options(
                                    &conversation,
                                    show_thinking,
                                    show_tool_outputs,
                                );
                                let header = self.render_header(&conversation);
                                lines.splice(0..0, header);
                                lines.push(String::new());
                                lines.extend(self.render_summary(&conversation));
                                // Adjust scroll offset if needed
                                let content_height = height.saturating_sub(1) as usize;
                                if lines.len() > content_height {
                                    scroll_offset = scroll_offset
                                        .min(lines.len().saturating_sub(content_height));
                                } else {
                                    scroll_offset = 0;
                                }
                            }
                            KeyCode::Char('o') => {
                                // Toggle tool outputs visibility
                                show_tool_outputs = !show_tool_outputs;
                                // Re-render content with new tool outputs visibility
                                lines = self.render_content_with_options(
                                    &conversation,
                                    show_thinking,
                                    show_tool_outputs,
                                );
                                let header = self.render_header(&conversation);
                                lines.splice(0..0, header);
                                lines.push(String::new());
                                lines.extend(self.render_summary(&conversation));
                                // Adjust scroll offset if needed
                                let content_height = height.saturating_sub(1) as usize;
                                if lines.len() > content_height {
                                    scroll_offset = scroll_offset
                                        .min(lines.len().saturating_sub(content_height));
                                } else {
                                    scroll_offset = 0;
                                }
                            }
                            KeyCode::Char('n') => {
                                if !matches.is_empty()
                                    && let Some(curr) = current_match_idx
                                {
                                    let next = (curr + 1) % matches.len();
                                    current_match_idx = Some(next);
                                    scroll_offset = matches[next].saturating_sub(2);
                                }
                            }
                            KeyCode::Char('N') => {
                                if !matches.is_empty()
                                    && let Some(curr) = current_match_idx
                                {
                                    let prev = if curr == 0 {
                                        matches.len() - 1
                                    } else {
                                        curr - 1
                                    };
                                    current_match_idx = Some(prev);
                                    scroll_offset = matches[prev].saturating_sub(2);
                                }
                            }
                            KeyCode::Char('p') | KeyCode::Char('P') => {
                                self.print(&conversation)?;
                            }
                            KeyCode::Char('e') => {
                                self.open_in_editor(&lines).await?;
                                // Need to redraw immediately
                                self.draw_viewport(DrawViewportArgs {
                                    lines: &lines,
                                    offset: scroll_offset,
                                    height: height as usize,
                                    search_query: if !search_query.is_empty() || is_searching {
                                        Some(&search_query)
                                    } else {
                                        None
                                    },
                                    match_info: if !matches.is_empty() {
                                        Some((current_match_idx.unwrap_or(0) + 1, matches.len()))
                                    } else {
                                        None
                                    },
                                    is_searching,
                                    show_thinking,
                                    show_tool_outputs,
                                })?;
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                scroll_offset = scroll_offset.saturating_sub(1);
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                let content_height = height.saturating_sub(1) as usize;
                                if scroll_offset + content_height < lines.len() {
                                    scroll_offset += 1;
                                }
                            }
                            KeyCode::PageUp | KeyCode::Char('u')
                                if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                            {
                                let content_height = height.saturating_sub(1) as usize;
                                scroll_offset = scroll_offset.saturating_sub(content_height / 2);
                            }
                            KeyCode::PageUp => {
                                let content_height = height.saturating_sub(1) as usize;
                                scroll_offset = scroll_offset.saturating_sub(content_height);
                            }
                            KeyCode::PageDown | KeyCode::Char('d')
                                if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                            {
                                let content_height = height.saturating_sub(1) as usize;
                                scroll_offset = (scroll_offset + content_height / 2)
                                    .min(lines.len().saturating_sub(content_height));
                            }
                            KeyCode::PageDown => {
                                let content_height = height.saturating_sub(1) as usize;
                                scroll_offset = (scroll_offset + content_height)
                                    .min(lines.len().saturating_sub(content_height));
                            }
                            KeyCode::Home | KeyCode::Char('g') => {
                                scroll_offset = 0;
                            }
                            KeyCode::End | KeyCode::Char('G') => {
                                let content_height = height.saturating_sub(1) as usize;
                                scroll_offset = lines.len().saturating_sub(content_height);
                            }
                            _ => {}
                        }
                    }

                    // Ensure scroll offset is valid
                    let content_height = height.saturating_sub(1) as usize;
                    if lines.len() > content_height {
                        scroll_offset =
                            scroll_offset.min(lines.len().saturating_sub(content_height));
                    } else {
                        scroll_offset = 0;
                    }

                    let match_info = if !matches.is_empty() {
                        Some((current_match_idx.unwrap_or(0) + 1, matches.len()))
                    } else {
                        None
                    };

                    self.draw_viewport(DrawViewportArgs {
                        lines: &lines,
                        offset: scroll_offset,
                        height: height as usize,
                        search_query: if !search_query.is_empty() || is_searching {
                            Some(&search_query)
                        } else {
                            None
                        },
                        match_info,
                        is_searching,
                        show_thinking,
                        show_tool_outputs,
                    })?;
                }
                Some(Ok(event::Event::Mouse(mouse_event))) => {
                    let content_height = height.saturating_sub(1) as usize;
                    match mouse_event.kind {
                        MouseEventKind::ScrollUp => {
                            if scroll_offset > 0 {
                                scroll_offset = scroll_offset.saturating_sub(3);
                            }
                        }
                        MouseEventKind::ScrollDown => {
                            if scroll_offset + content_height < lines.len() {
                                scroll_offset = (scroll_offset + 3)
                                    .min(lines.len().saturating_sub(content_height));
                            }
                        }
                        _ => {}
                    }

                    // Redraw after mouse scroll
                    let match_info = if !matches.is_empty() {
                        Some((current_match_idx.unwrap_or(0) + 1, matches.len()))
                    } else {
                        None
                    };

                    self.draw_viewport(DrawViewportArgs {
                        lines: &lines,
                        offset: scroll_offset,
                        height: height as usize,
                        search_query: if !search_query.is_empty() || is_searching {
                            Some(&search_query)
                        } else {
                            None
                        },
                        match_info,
                        is_searching,
                        show_thinking,
                        show_tool_outputs,
                    })?;
                }
                Some(Ok(event::Event::Resize(_w, h))) => {
                    height = h;
                    // Note: If we had to re-wrap lines based on width, we'd need to re-render here.
                    // But current rendering is not wrapping-aware (except markdown somewhat).
                    // We can re-render if needed, but for simplicity we keep lines as is for now,
                    // or call render_content again if we had stored conversation.
                    // Since we have conversation, we can re-render!

                    lines = self.render_content(&conversation);
                    let header = self.render_header(&conversation);
                    lines.splice(0..0, header);
                    lines.push(String::new());
                    lines.extend(self.render_summary(&conversation));

                    if !search_query.is_empty() {
                        matches = update_matches(&search_query, &lines);
                        if !matches.is_empty() {
                            current_match_idx = Some(0);
                        } else {
                            current_match_idx = None;
                        }
                    }

                    let content_height = height.saturating_sub(1) as usize;
                    scroll_offset = scroll_offset.min(lines.len().saturating_sub(content_height));

                    let match_info = if !matches.is_empty() {
                        Some((current_match_idx.unwrap_or(0) + 1, matches.len()))
                    } else {
                        None
                    };

                    self.draw_viewport(DrawViewportArgs {
                        lines: &lines,
                        offset: scroll_offset,
                        height: height as usize,
                        search_query: if !search_query.is_empty() || is_searching {
                            Some(&search_query)
                        } else {
                            None
                        },
                        match_info,
                        is_searching,
                        show_thinking,
                        show_tool_outputs,
                    })?;
                }
                _ => {}
            }
        }

        execute!(
            std::io::stdout(),
            DisableMouseCapture,
            terminal::LeaveAlternateScreen,
            cursor::Show
        )?;
        Ok(())
    }
}
